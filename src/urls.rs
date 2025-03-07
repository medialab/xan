use std::fmt::{self, Display};
use std::str::FromStr;

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

#[derive(Debug, Clone, Copy)]
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

struct LRUStem {
    string: String,
    kind: LRUStemKind,
}

// impl LRUStem {
//     fn to_string(&self) -> String {
//         format!("{}:{}", self.kind.as_str(), self.string)
//     }
// }

pub struct LRUStems(Vec<LRUStem>);

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
        let mut stems = Vec::new();

        let url = &value.url;

        // Scheme
        if value.has_scheme {
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
                Host::Domain(domain) => {
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
        if !url.username().is_empty() {
            stems.push(LRUStem {
                string: url.username().to_string(),
                kind: LRUStemKind::User,
            })
        }

        // Password
        if let Some(password) = url.password() {
            stems.push(LRUStem {
                string: password.to_string(),
                kind: LRUStemKind::Password,
            })
        }

        Self(stems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lru(url: &str) -> String {
        LRUStems::from(&url.parse::<TaggedUrl>().unwrap()).to_string()
    }

    #[test]
    fn test_tagged_url() {
        assert!(!"lemonde.fr".parse::<TaggedUrl>().unwrap().has_scheme);
        assert!("http://lemonde.fr".parse::<TaggedUrl>().unwrap().has_scheme);
    }

    #[test]
    fn test_lru_stems() {
        assert_eq!(lru("http://lemonde.fr"), "s:http|h:fr|h:lemonde|");
        assert_eq!(lru("lemonde.fr"), "h:fr|h:lemonde|");
        assert_eq!(
            lru("http://lemonde.fr?test"),
            "s:http|h:fr|h:lemonde|q:test|"
        );
        assert_eq!(
            lru("http://user:password@lemonde.fr/path?test#frag"),
            "s:http|h:fr|h:lemonde|p:path|q:test|f:frag|u:user|w:password|"
        );
    }
}
