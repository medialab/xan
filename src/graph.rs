use std::borrow::Cow;
use std::io::Write;
use std::ops::Not;
use std::rc::Rc;

use ahash::RandomState;
use indexmap::{map::Entry as IndexMapEntry, IndexMap};
use jiff::Zoned;
use serde_json::Value;

use crate::collections::{HashMap, UnionFind};
use crate::config::Config;
use crate::json::{Attributes, JSONType, INTERNER};
use crate::xml::XMLWriter;
use crate::CliResult;

fn serialize_value_to_csv(value: &Value) -> Cow<str> {
    match value {
        Value::String(string) => Cow::Borrowed(string),
        Value::Bool(v) => Cow::Borrowed(if *v { "true" } else { "false" }),
        Value::Null => Cow::Borrowed(""),
        Value::Number(v) => Cow::Owned(v.to_string()),
        _ => unreachable!(),
    }
}

impl JSONType {
    fn as_gexf_type(&self) -> &str {
        match self {
            Self::Float => "double",
            Self::Integer => "long",
            Self::String => "string",
            Self::Null => "string",
        }
    }
}

struct GexfNamespace {
    version: &'static str,
    xmlns: &'static str,
    schema_location: &'static str,
}

impl GexfNamespace {
    fn one_point_two() -> Self {
        Self {
            version: "1.2",
            xmlns: "http://www.gexf.net/1.2draft",
            schema_location: "http://www.gexf.net/1.2draft http://www.gexf.net/1.2draft/gexf.xsd",
        }
    }

    fn one_point_three() -> Self {
        Self {
            version: "1.3",
            xmlns: "http://gexf.net/1.3",
            schema_location: "http://gexf.net/1.3 http://gexf.net/1.3/gexf.xsd",
        }
    }
}

#[derive(Serialize)]
struct Node {
    key: Rc<String>,
    #[serde(skip_serializing_if = "Attributes::is_empty")]
    attributes: Attributes,
}

#[derive(Serialize)]
struct Edge {
    source: Rc<String>,
    target: Rc<String>,
    #[serde(skip_serializing_if = "Not::not")]
    undirected: bool,
    #[serde(skip_serializing_if = "Attributes::is_empty")]
    attributes: Attributes,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphType {
    #[default]
    Directed,
    Undirected,
}

impl GraphType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Directed => "directed",
            Self::Undirected => "undirected",
        }
    }
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphOptions {
    pub allow_self_loops: bool,
    pub multi: bool,
    #[serde(rename = "type")]
    pub graph_type: GraphType,
}

#[derive(Default, Serialize)]
pub struct Graph {
    pub options: GraphOptions,
    #[serde(skip_serializing)]
    node_model: Vec<ModelAttribute>,
    #[serde(skip_serializing)]
    edge_model: Vec<ModelAttribute>,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

#[derive(Debug)]
pub struct GraphStats {
    pub nodes: usize,
    pub edges: usize,
    pub density: f64,
}

impl Graph {
    pub fn compute_stats(&self) -> GraphStats {
        let nodes = self.nodes.len();
        let edges = self.edges.len();

        let density = match self.options.graph_type {
            GraphType::Directed => edges as f64 / (nodes * nodes.saturating_sub(1)) as f64,
            GraphType::Undirected => edges as f64 / (nodes * nodes.saturating_sub(1) / 2) as f64,
        };

        GraphStats {
            nodes,
            edges,
            density,
        }
    }
}

#[derive(Debug)]
struct ModelAttribute {
    interner_id: usize,
    name: String,
    json_type: JSONType,
}

#[derive(Debug)]
pub enum DegreeMap {
    Undirected(Vec<usize>),
    Directed(Vec<(usize, usize)>),
}

impl DegreeMap {
    fn new_undirected(capacity: usize) -> Self {
        Self::Undirected(vec![0; capacity])
    }

    fn new_directed(capacity: usize) -> Self {
        Self::Directed(vec![(0, 0); capacity])
    }

    fn new(undirected: bool, capacity: usize) -> Self {
        if undirected {
            Self::new_undirected(capacity)
        } else {
            Self::new_directed(capacity)
        }
    }

    fn is_undirected(&self) -> bool {
        matches!(self, Self::Undirected(_))
    }

    fn add(&mut self, source: usize, target: usize) {
        match self {
            Self::Undirected(map) => {
                map[source] += 1;
                map[target] += 1;
            }
            Self::Directed(map) => {
                map[source].1 += 1;
                map[target].0 += 1;
            }
        }
    }
}

#[derive(Default)]
pub struct GraphBuilder {
    options: GraphOptions,
    disjoint_sets: Option<UnionFind>,
    node_model: Vec<ModelAttribute>,
    edge_model: Vec<ModelAttribute>,
    nodes: IndexMap<Rc<String>, Node, RandomState>,
    edges: HashMap<(usize, usize), Edge>,
}

impl GraphBuilder {
    pub fn new(track_largest_component: bool) -> Self {
        let mut builder = Self::default();

        if track_largest_component {
            builder.disjoint_sets = Some(UnionFind::new());
        }

        builder
    }

    fn is_undirected(&self) -> bool {
        matches!(self.options.graph_type, GraphType::Undirected)
    }

    pub fn mark_as_undirected(&mut self) {
        self.options.graph_type = GraphType::Undirected;
    }

    pub fn set_node_model<'a>(
        &mut self,
        headers: impl Iterator<Item = &'a str>,
        model: impl Iterator<Item = JSONType>,
    ) {
        self.node_model = headers
            .zip(model)
            .map(|(header, json_type)| ModelAttribute {
                name: header.to_string(),
                interner_id: INTERNER
                    .with_borrow_mut(|interner| interner.register(header.to_string())),
                json_type,
            })
            .collect();
    }

    pub fn set_edge_model<'a>(
        &mut self,
        headers: impl Iterator<Item = &'a str>,
        model: impl Iterator<Item = JSONType>,
    ) {
        self.edge_model = headers
            .zip(model)
            .map(|(header, json_type)| ModelAttribute {
                name: header.to_string(),
                interner_id: INTERNER
                    .with_borrow_mut(|interner| interner.register(header.to_string())),
                json_type,
            })
            .collect();
    }

    pub fn add_node(&mut self, key: String, attributes: Attributes) -> usize {
        use IndexMapEntry::*;

        let rc_key = Rc::new(key);
        let next_id = self.nodes.len();

        match self.nodes.entry(rc_key.clone()) {
            Occupied(entry) => entry.index(),
            Vacant(entry) => {
                entry.insert(Node {
                    key: rc_key,
                    attributes,
                });

                if let Some(sets) = self.disjoint_sets.as_mut() {
                    sets.make_set();
                }

                next_id
            }
        }
    }

    pub fn add_edge(&mut self, source: usize, target: usize, attributes: Attributes) {
        let undirected = self.is_undirected();

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

    pub fn compute_degrees(&self) -> DegreeMap {
        let mut degree_map = DegreeMap::new(self.is_undirected(), self.nodes.len());

        for (source, target) in self.edges.keys().copied() {
            degree_map.add(source, target);
        }

        degree_map
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
            node_model: self.node_model,
            edge_model: self.edge_model,
            nodes,
            edges,
        }
    }
}

impl Graph {
    pub fn write_json<W: Write>(&self, mut writer: W) -> CliResult<()> {
        serde_json::to_writer_pretty(&mut writer, &self)?;
        writeln!(&mut writer)?;

        Ok(())
    }

    pub fn write_gexf<W: Write>(&self, writer: W, version: &str) -> CliResult<()> {
        let mut xml_writer = XMLWriter::new(writer);

        xml_writer.write_declaration()?;

        let gexf_namespace = match version {
            "1.3" => GexfNamespace::one_point_three(),
            "1.2" => GexfNamespace::one_point_two(),
            _ => panic!("unsupported gexf version"),
        };

        let today = Zoned::now().strftime("%F").to_string();
        let node_model = &self.node_model;
        let edge_model = &self.edge_model;
        let graph = self;

        xml_writer.open(
            "gexf",
            [
                ("xmlns", gexf_namespace.xmlns),
                ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
                ("xsi:schemaLocation", gexf_namespace.schema_location),
                ("version", gexf_namespace.version),
            ],
        )?;

        // Meta
        xml_writer.open("meta", [("lastmodifieddate", today.as_str())])?;

        xml_writer.open_no_attributes("creator")?;
        xml_writer.write_text("xan")?;
        xml_writer.close("creator")?;

        xml_writer.close("meta")?;

        // Graph data
        xml_writer.open(
            "graph",
            [("defaultedgetype", graph.options.graph_type.as_str())],
        )?;

        // Node model
        let mut node_label_attr: Option<usize> = None;

        xml_writer.open("attributes", [("class", "node")])?;
        for (i, model_attr) in node_model.iter().enumerate() {
            if model_attr.name == "label" {
                node_label_attr = Some(model_attr.interner_id);
                continue;
            }

            xml_writer.open_empty(
                "attribute",
                [
                    ("id", i.to_string().as_str()),
                    ("title", model_attr.name.as_str()),
                    ("type", model_attr.json_type.as_gexf_type()),
                ],
            )?;
        }
        xml_writer.close("attributes")?;

        // Edge model
        xml_writer.open("attributes", [("class", "edge")])?;
        for (i, model_attr) in edge_model.iter().enumerate() {
            xml_writer.open_empty(
                "attribute",
                [
                    ("id", i.to_string().as_str()),
                    ("title", model_attr.name.as_str()),
                    ("type", model_attr.json_type.as_gexf_type()),
                ],
            )?;
        }
        xml_writer.close("attributes")?;

        fn serialize_value(value: &Value) -> String {
            match value {
                Value::Bool(b) => b.to_string(),
                Value::Null => "".to_string(),
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.to_string(),
                _ => unreachable!(),
            }
        }

        // Node data
        xml_writer.open_no_attributes("nodes")?;
        for node in graph.nodes.iter() {
            let node_label = if let Some(id) = node_label_attr {
                match node.attributes.get(id) {
                    None => Cow::Borrowed(node.key.as_str()),
                    Some(v) => Cow::Owned(serialize_value(v)),
                }
            } else {
                Cow::Borrowed(node.key.as_str())
            };

            if node.attributes.is_empty() {
                xml_writer
                    .open_empty("node", [("id", node.key.as_str()), ("label", &node_label)])?;
            } else {
                xml_writer.open("node", [("id", node.key.as_str()), ("label", &node_label)])?;

                xml_writer.open_no_attributes("attvalues")?;
                for (i, (interner_id, value)) in node.attributes.iter().enumerate() {
                    if matches!(node_label_attr, Some(id) if id == *interner_id) {
                        continue;
                    }

                    xml_writer.open_empty(
                        "attvalue",
                        [
                            ("for", i.to_string().as_str()),
                            ("value", &serialize_value(value)),
                        ],
                    )?;
                }
                xml_writer.close("attvalues")?;

                xml_writer.close("node")?;
            }
        }
        xml_writer.close("nodes")?;

        // Edge data
        xml_writer.open_no_attributes("edges")?;
        for edge in graph.edges.iter() {
            if edge.attributes.is_empty() {
                xml_writer.open_empty(
                    "edge",
                    [
                        ("source", edge.source.as_str()),
                        ("target", edge.target.as_str()),
                    ],
                )?;
            } else {
                xml_writer.open(
                    "edge",
                    [
                        ("source", edge.source.as_str()),
                        ("target", edge.target.as_str()),
                    ],
                )?;

                xml_writer.open_no_attributes("attvalues")?;
                for (i, (_, value)) in edge.attributes.iter().enumerate() {
                    xml_writer.open_empty(
                        "attvalue",
                        [
                            ("for", i.to_string().as_str()),
                            ("value", &serialize_value(value)),
                        ],
                    )?;
                }
                xml_writer.close("attvalues")?;

                xml_writer.close("edge")?;
            }
        }
        xml_writer.close("edges")?;

        xml_writer.close("graph")?;

        xml_writer.close("gexf")?;
        xml_writer.finish()?;

        Ok(())
    }

    pub fn write_csv_nodelist<W: Write>(
        &self,
        writer: W,
        degree_map: Option<DegreeMap>,
    ) -> CliResult<()> {
        let mut writer = Config::new(&None).csv_writer_from_writer(writer);

        let mut record = csv::ByteRecord::new();
        record.push_field(b"node");

        for attr in self.node_model.iter() {
            record.push_field(attr.name.as_bytes());
        }

        if let Some(map) = &degree_map {
            record.push_field(b"degree");

            if !map.is_undirected() {
                record.push_field(b"in_degree");
                record.push_field(b"out_degree");
            }
        }

        writer.write_byte_record(&record)?;

        for (i, node) in self.nodes.iter().enumerate() {
            record.clear();
            record.push_field(node.key.as_bytes());

            if !node.attributes.is_empty() {
                for (_, attr_value) in node.attributes.iter() {
                    record.push_field(serialize_value_to_csv(attr_value).as_bytes());
                }
            } else {
                for _ in self.node_model.iter() {
                    record.push_field(b"");
                }
            }

            if let Some(map) = &degree_map {
                match map {
                    DegreeMap::Directed(degrees) => {
                        let degree = degrees[i];
                        record.push_field((degree.0 + degree.1).to_string().as_bytes());
                        record.push_field(degree.0.to_string().as_bytes());
                        record.push_field(degree.1.to_string().as_bytes());
                    }
                    DegreeMap::Undirected(degrees) => {
                        record.push_field(degrees[i].to_string().as_bytes());
                    }
                }
            }

            writer.write_byte_record(&record)?;
        }

        Ok(())
    }
}
