use std::cell::RefCell;
use std::collections::{btree_map::Entry as BTreeMapEntry, BTreeMap, HashMap};
use std::ops::Not;
use std::rc::Rc;

use indexmap::{map::Entry as IndexMapEntry, IndexMap};
use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::Value;

use crate::collections::UnionFind;

#[derive(Default)]
struct AttributeNameInterner {
    strings: Vec<Rc<String>>,
    map: BTreeMap<Rc<String>, usize>,
}

impl AttributeNameInterner {
    fn register(&mut self, name: String) -> usize {
        use BTreeMapEntry::*;

        let name = Rc::new(name);
        let next_id = self.strings.len();

        match self.map.entry(name.clone()) {
            Occupied(entry) => *entry.get(),
            Vacant(entry) => {
                self.strings.push(name.clone());
                entry.insert(next_id);
                next_id
            }
        }
    }

    fn get(&self, id: usize) -> &str {
        &self.strings[id]
    }
}

thread_local! {
    static INTERNER: RefCell<AttributeNameInterner> = RefCell::new(AttributeNameInterner::default());
}

#[derive(Debug)]
pub struct Attributes {
    entries: Vec<(usize, Value)>,
}

impl Attributes {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, key: &str, value: Value) {
        let attr_name_id = INTERNER.with_borrow_mut(|interner| interner.register(key.to_string()));
        self.entries.push((attr_name_id, value));
    }
}

impl Serialize for Attributes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.entries.len()))?;

        for (k, v) in self.entries.iter() {
            INTERNER.with_borrow(|interner| map.serialize_entry(interner.get(*k), v))?;
        }

        map.end()
    }
}

#[derive(Serialize)]
struct Node {
    key: Rc<String>,
}

#[derive(Serialize)]
struct Edge {
    source: Rc<String>,
    target: Rc<String>,
    #[serde(skip_serializing_if = "Not::not")]
    undirected: bool,
    attributes: Attributes,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "lowercase")]
enum GraphType {
    #[default]
    Directed,
    Undirected,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphOptions {
    allow_self_loops: bool,
    multi: bool,
    graph_type: GraphType,
}

#[derive(Default, Serialize)]
pub struct Graph {
    options: GraphOptions,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

#[derive(Default)]
pub struct GraphBuilder {
    options: GraphOptions,
    disjoint_sets: Option<UnionFind>,
    nodes: IndexMap<Rc<String>, Node>,
    edges: HashMap<(usize, usize), Edge>,
}

impl GraphBuilder {
    pub fn mark_as_undirected(&mut self) {
        self.options.graph_type = GraphType::Undirected;
    }

    pub fn keep_largest_component(&mut self) {
        self.disjoint_sets = Some(UnionFind::new());
    }

    pub fn add_node(&mut self, key: String) -> usize {
        use IndexMapEntry::*;

        let rc_key = Rc::new(key);
        let next_id = self.nodes.len();

        match self.nodes.entry(rc_key.clone()) {
            Occupied(entry) => entry.index(),
            Vacant(entry) => {
                entry.insert(Node { key: rc_key });

                if let Some(sets) = self.disjoint_sets.as_mut() {
                    sets.make_set();
                }

                next_id
            }
        }
    }

    pub fn add_edge(
        &mut self,
        source: usize,
        target: usize,
        attributes: Attributes,
        undirected: bool,
    ) {
        let (source, target) = if source == target {
            self.options.allow_self_loops = true;
            (source, target)
        } else if undirected && source > target {
            (target, source)
        } else {
            (source, target)
        };

        let source_node = self.nodes.get_index(source).unwrap().1;
        let target_node = self.nodes.get_index(target).unwrap().1;

        let edge = Edge {
            source: source_node.key.clone(),
            target: target_node.key.clone(),
            undirected,
            attributes,
        };

        if self.edges.insert((source, target), edge).is_some() {
            self.options.multi = true;
        }

        if let Some(sets) = self.disjoint_sets.as_mut() {
            sets.union(source, target);
        }
    }

    pub fn build(self) -> Graph {
        let (nodes, edges) = if let Some(sets) = self.disjoint_sets {
            let largest_component = sets.largest();

            (
                self.nodes
                    .into_values()
                    .enumerate()
                    .filter_map(|(i, node)| {
                        if matches!(largest_component, Some(c) if c != sets.find(i)) {
                            None
                        } else {
                            Some(node)
                        }
                    })
                    .collect(),
                self.edges
                    .into_iter()
                    .filter_map(|((source_id, _), edge)| {
                        if matches!(largest_component, Some(c) if c != sets.find(source_id)) {
                            None
                        } else {
                            Some(edge)
                        }
                    })
                    .collect(),
            )
        } else {
            (
                self.nodes.into_values().collect(),
                self.edges.into_values().collect(),
            )
        };

        Graph {
            options: self.options,
            nodes,
            edges,
        }
    }
}
