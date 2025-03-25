use std::collections::BTreeMap;
use std::fmt::{self, Display};
use std::num::NonZeroUsize;
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

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord)]
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

#[derive(PartialEq, PartialOrd, Ord, Eq, Debug)]
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

    pub fn into_iter(self) -> impl Iterator<Item = LRUStem> {
        self.0.into_iter()
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

#[derive(Debug)]
struct LRUTrieMapNode<V> {
    value: Option<V>,
    children: BTreeMap<LRUStem, LRUTrieMapNode<V>>,
}

impl<V> LRUTrieMapNode<V> {
    pub fn empty() -> Self {
        Self {
            value: None,
            children: BTreeMap::new(),
        }
    }

    fn add_child(&mut self, stem: LRUStem) -> &mut LRUTrieMapNode<V> {
        self.children
            .entry(stem)
            .or_insert_with(LRUTrieMapNode::empty)
    }

    fn get_child(&self, stem: &LRUStem) -> Option<&LRUTrieMapNode<V>> {
        self.children.get(stem)
    }
}

#[derive(Debug)]
pub struct LRUTrieMap<V> {
    root: LRUTrieMapNode<V>,
}

impl<V> LRUTrieMap<V> {
    pub fn new() -> Self {
        Self {
            root: LRUTrieMapNode::empty(),
        }
    }

    pub fn insert(&mut self, url: &str, value: V) -> Result<(), ParseError> {
        let tagged_url = url.parse::<TaggedUrl>()?;
        let stems = LRUStems::from_tagged_url(&tagged_url, true);

        let mut current_node = &mut self.root;

        for stem in stems.into_iter() {
            current_node = current_node.add_child(stem);
        }

        current_node.value = Some(value);

        Ok(())
    }

    pub fn longest_matching_prefix_value(&self, url: &str) -> Result<Option<&V>, ParseError> {
        let tagged_url = url.parse::<TaggedUrl>()?;
        let stems = LRUStems::from_tagged_url(&tagged_url, true);

        let mut matching_value = None;
        let mut current_node = &self.root;

        for stem in stems.into_iter() {
            if let Some(child) = current_node.get_child(&stem) {
                current_node = child;
            } else {
                break;
            }
        }

        if let Some(value) = &current_node.value {
            matching_value = Some(value);
        }

        Ok(matching_value)
    }

    pub fn is_match(&self, url: &str) -> Result<bool, ParseError> {
        self.longest_matching_prefix_value(url)
            .map(|value| value.is_some())
    }
}

impl LRUTrieMap<()> {
    pub fn add(&mut self, url: &str) -> Result<(), ParseError> {
        self.insert(url, ())
    }
}

pub type LRUTrie = LRUTrieMap<()>;

struct LRUTrieMultiMapNode<V> {
    value: V,
    next: Option<NonZeroUsize>,
}

impl<V> LRUTrieMultiMapNode<V> {
    fn new(value: V) -> Self {
        Self { value, next: None }
    }
}

pub struct LongestMatchingPrefixValues<'a, V> {
    nodes: &'a Vec<LRUTrieMultiMapNode<V>>,
    current_node: Option<usize>,
}

impl<'a, V> Iterator for LongestMatchingPrefixValues<'a, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        self.current_node.map(|i| {
            let node = &self.nodes[i - 1];

            if let Some(previous_index) = &node.next {
                self.current_node = Some(previous_index.get());
            } else {
                self.current_node = None;
            }

            &node.value
        })
    }
}

pub struct LRUTrieMultiMap<V> {
    trie: LRUTrieMap<(usize, usize)>,
    nodes: Vec<LRUTrieMultiMapNode<V>>,
}

impl<V> LRUTrieMultiMap<V> {
    pub fn new() -> Self {
        Self {
            trie: LRUTrieMap::new(),
            nodes: Vec::new(),
        }
    }

    pub fn insert(&mut self, url: &str, value: V) -> Result<(), ParseError> {
        let tagged_url = url.parse::<TaggedUrl>()?;
        let stems = LRUStems::from_tagged_url(&tagged_url, true);

        let mut current_node = &mut self.trie.root;
        let next_id = self.nodes.len() + 1;

        for stem in stems.into_iter() {
            current_node = current_node.add_child(stem);
        }

        if let Some((_, tail)) = &mut current_node.value {
            let new_node = LRUTrieMultiMapNode::new(value);
            self.nodes[*tail - 1].next = Some(NonZeroUsize::new(next_id).unwrap());
            *tail = next_id;
            self.nodes.push(new_node);
        } else {
            self.nodes.push(LRUTrieMultiMapNode::new(value));
            current_node.value = Some((next_id, next_id));
        }

        Ok(())
    }

    pub fn longest_matching_prefix_values(
        &self,
        url: &str,
    ) -> Result<LongestMatchingPrefixValues<V>, ParseError> {
        self.trie
            .longest_matching_prefix_value(url)
            .map(|found| LongestMatchingPrefixValues {
                nodes: &self.nodes,
                current_node: found.map(|(head, _)| *head),
            })
    }
}

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

    #[test]
    fn test_lru_trie() {
        let mut trie = LRUTrie::new();
        trie.add("http://www.lemonde.fr").unwrap();
        trie.add("http://lefigaro.fr/business").unwrap();

        assert_eq!(trie.is_match("http://lemonde.fr").unwrap(), true);
        assert_eq!(
            trie.is_match("http://lemonde.fr/path/to.html").unwrap(),
            true
        );
        assert_eq!(trie.is_match("http://lefigaro.fr").unwrap(), false);
        assert_eq!(
            trie.is_match("http://lefigaro.fr/business/article.html")
                .unwrap(),
            true
        );
        assert_eq!(trie.is_match("http://liberation.fr").unwrap(), false);
    }

    #[test]
    fn test_lru_trie_multimap() {
        let mut trie: LRUTrieMultiMap<usize> = LRUTrieMultiMap::new();

        trie.insert("http://www.lemonde.fr", 1).unwrap();
        trie.insert("http://lefigaro.fr/business", 2).unwrap();
        trie.insert("http://www.lemonde.fr", 3).unwrap();

        assert_eq!(
            trie.longest_matching_prefix_values("http://lemonde.fr/path/to.html")
                .unwrap()
                .copied()
                .collect::<Vec<_>>(),
            vec![1, 3]
        );

        assert_eq!(
            trie.longest_matching_prefix_values("http://lefigaro.fr/business/path.html")
                .unwrap()
                .copied()
                .collect::<Vec<_>>(),
            vec![2]
        );

        assert_eq!(
            trie.longest_matching_prefix_values("http://liberation.fr")
                .unwrap()
                .copied()
                .collect::<Vec<_>>(),
            Vec::<usize>::new()
        );
    }
}
