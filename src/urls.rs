// use std::collections::BTreeMap;
use std::fmt::{self, Display};
use std::ops::Deref;
use std::str::FromStr;

use bstr::ByteSlice;
use url::{Host, ParseError, Url};

#[derive(Debug, PartialEq)]
pub struct TaggedUrl {
    has_scheme: bool,
    url: Url,
}

impl TaggedUrl {
    pub fn into_inner(self) -> Url {
        self.url
    }
}

impl FromStr for TaggedUrl {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with("https://") && !s.starts_with("http://") {
            Url::parse(&format!("https://{}", s)).map(|url| Self {
                has_scheme: false,
                url,
            })
        } else {
            Url::parse(s).map(|url| TaggedUrl {
                has_scheme: true,
                url,
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LRUStemKind {
    Scheme,
    Port,
    User,
    Password,
    Host,
    Path,
    Query,
    Fragment,
}

impl LRUStemKind {
    fn as_str(&self) -> &str {
        match self {
            Self::Scheme => "s",
            Self::Port => "t",
            Self::Host => "h",
            Self::User => "u",
            Self::Path => "p",
            Self::Query => "q",
            Self::Fragment => "f",
            Self::Password => "w",
        }
    }
}

#[derive(PartialEq)]
pub struct LRUStem {
    string: String,
    kind: LRUStemKind,
}

pub struct LRUStems(Vec<LRUStem>);

impl LRUStems {
    pub fn from_tagged_url(value: &TaggedUrl, simplified: bool) -> Self {
        let mut stems = Vec::new();

        let url = &value.url;

        // Scheme
        if !simplified && value.has_scheme {
            stems.push(LRUStem {
                string: url.scheme().to_string(),
                kind: LRUStemKind::Scheme,
            })
        }

        // Port
        if let Some(port) = url.port() {
            stems.push(LRUStem {
                string: port.to_string(),
                kind: LRUStemKind::Port,
            })
        }

        // Host
        if let Some(host) = url.host() {
            match host {
                Host::Ipv4(_) | Host::Ipv6(_) => stems.push(LRUStem {
                    string: url.host_str().unwrap().to_string(),
                    kind: LRUStemKind::Host,
                }),
                Host::Domain(mut domain) => {
                    if simplified && domain.starts_with("www.") {
                        domain = &domain[4..];
                    }

                    for part in domain.split('.').rev() {
                        stems.push(LRUStem {
                            string: part.to_string(),
                            kind: LRUStemKind::Host,
                        })
                    }
                }
            }
        }

        // Path
        if url.path() != "/" {
            if let Some(segments) = url.path_segments() {
                for part in segments {
                    stems.push(LRUStem {
                        string: part.to_string(),
                        kind: LRUStemKind::Path,
                    })
                }
            }
        }

        // Query
        if let Some(query) = url.query() {
            stems.push(LRUStem {
                string: query.to_string(),
                kind: LRUStemKind::Query,
            })
        }

        // Fragment
        if let Some(fragment) = url.fragment() {
            stems.push(LRUStem {
                string: fragment.to_string(),
                kind: LRUStemKind::Fragment,
            })
        }

        // User
        if !simplified && !url.username().is_empty() {
            stems.push(LRUStem {
                string: url.username().to_string(),
                kind: LRUStemKind::User,
            })
        }

        // Password
        if !simplified {
            if let Some(password) = url.password() {
                stems.push(LRUStem {
                    string: password.to_string(),
                    kind: LRUStemKind::Password,
                })
            }
        }

        Self(stems)
    }

    pub fn is_simplified_match(&self, target: &str) -> bool {
        if let Ok(tagged_url) = target.parse::<TaggedUrl>() {
            let stems = Self::from_tagged_url(&tagged_url, true);
            stems.starts_with(self)
        } else {
            false
        }
    }
}

impl Deref for LRUStems {
    type Target = [LRUStem];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for LRUStems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for stem in self.0.iter() {
            write!(f, "{}:{}|", stem.kind.as_str(), &stem.string)?;
        }
        Ok(())
    }
}

impl<'a> From<&'a TaggedUrl> for LRUStems {
    fn from(value: &'a TaggedUrl) -> Self {
        Self::from_tagged_url(value, false)
    }
}

pub fn should_follow_href<T: AsRef<[u8]>>(href: T) -> bool {
    let href = href.as_ref().trim();

    if href.is_empty() || href.starts_with(b"#") {
        return false;
    }

    if href.contains_str(b":") {
        let start = &href[..8].to_lowercase();
        return start.starts_with(b"https://") || start.starts_with(b"http://");
    }

    true
}

// #[derive(Default)]
// struct LRUTrieMapNode<V> {
//     value: Option<V>,
//     children: BTreeMap<LRUStem, LRUTrieMapNode<V>>,
//     descendants_count: usize,
// }

// impl<V> LRUTrieMapNode<V> {
//     pub fn empty() -> Self {
//         Self {
//             value: None,
//             children: BTreeMap::new(),
//             descendants_count: 0,
//         }
//     }
// }

// pub struct LRUTrieMap<V> {
//     root: LRUTrieMapNode<V>,
// }

// impl<V> LRUTrieMap<V> {
//     pub fn new() -> Self {
//         Self {
//             root: LRUTrieMapNode::empty(),
//         }
//     }
// }

// TODO: LRUTrieMultiMap later on

#[cfg(test)]
mod tests {
    use super::*;

    fn lru(url: &str) -> String {
        LRUStems::from(&url.parse::<TaggedUrl>().unwrap()).to_string()
    }

    fn simplified_lru(url: &str) -> String {
        LRUStems::from_tagged_url(&url.parse::<TaggedUrl>().unwrap(), true).to_string()
    }

    #[test]
    fn test_tagged_url() {
        assert!(!"lemonde.fr".parse::<TaggedUrl>().unwrap().has_scheme);
        assert!("http://lemonde.fr".parse::<TaggedUrl>().unwrap().has_scheme);
    }

    #[test]
    fn test_lru_stems() {
        assert_eq!(lru("http://lemonde.fr"), "s:http|h:fr|h:lemonde|");
        assert_eq!(lru("http://lemonde.fr/"), "s:http|h:fr|h:lemonde|");
        assert_eq!(lru("lemonde.fr"), "h:fr|h:lemonde|");
        assert_eq!(
            lru("http://lemonde.fr?test"),
            "s:http|h:fr|h:lemonde|q:test|"
        );
        assert_eq!(
            lru("http://user:password@lemonde.fr/path?test#frag"),
            "s:http|h:fr|h:lemonde|p:path|q:test|f:frag|u:user|w:password|"
        );
        assert_eq!(
            simplified_lru("http://user:password@www.lemonde.fr/path?test#frag"),
            "h:fr|h:lemonde|p:path|q:test|f:frag|"
        );
    }

    #[test]
    fn test_should_follow_href() {
        let tests = vec![
            ("#top", false),
            ("  #strip", false),
            ("magnet:uri-xIOhoug", false),
            ("home.html", true),
            ("/home.html", true),
            ("./home.html", true),
            ("https://www.lemonde.fr", true),
            ("HTTP://www.lemonde.fr", true),
            ("http:www.lemonde", false),
            ("mailto:whatever@gmail.com", false),
            ("tel:053775175743", false),
            ("javascript:alert(\"hello\")", false),
            ("file:///home/test/ok", false),
            ("ftp:whatever", false),
            ("", false),
        ];

        for (url, expected) in tests {
            assert_eq!(should_follow_href(url), expected, "{}", url);
        }
    }
}
