use std::borrow::Cow;
use std::cell::RefCell;
use std::io::Write;
use std::ops::Not;

use ahash::RandomState;
use indexmap::{map::Entry as IndexMapEntry, IndexMap};
use jiff::Zoned;
use serde::ser::{SerializeMap, SerializeSeq, Serializer as _};
use serde_json::{
    ser::{Formatter, Serializer},
    Value,
};

use crate::collections::UnionFind;
use crate::config::Config;
use crate::json::{Attributes, JSONType, INTERNER};
use crate::xml::XMLWriter;
use crate::CliResult;

fn density(graph_type: GraphType, order: usize, size: usize) -> f64 {
    match graph_type {
        GraphType::Directed => size as f64 / (order * order.saturating_sub(1)) as f64,
        GraphType::Undirected => size as f64 / (order * order.saturating_sub(1) / 2) as f64,
    }
}

fn serialize_value_to_csv(value: &Value) -> Cow<'_, str> {
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

#[derive(Default, Serialize, Clone, Copy)]
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

enum EdgeStore {
    Hash(IndexMap<(usize, usize), Attributes, RandomState>),
    Linear(Vec<(usize, usize, Attributes)>),
}

impl EdgeStore {
    #[inline]
    fn len(&self) -> usize {
        match self {
            Self::Hash(index) => index.len(),
            Self::Linear(list) => list.len(),
        }
    }

    fn insert(&mut self, source: usize, target: usize, attributes_opt: Option<Attributes>) -> bool {
        match self {
            Self::Hash(index) => match index.entry((source, target)) {
                IndexMapEntry::Occupied(mut entry) => {
                    if let Some(attributes) = attributes_opt {
                        entry.insert(attributes);
                    }
                    true
                }
                IndexMapEntry::Vacant(entry) => {
                    entry.insert(attributes_opt.unwrap_or_default());
                    false
                }
            },
            Self::Linear(list) => {
                list.push((source, target, attributes_opt.unwrap_or_default()));
                false
            }
        }
    }

    fn pairs(&self) -> Box<dyn Iterator<Item = (usize, usize)> + '_> {
        match self {
            Self::Hash(index) => Box::new(index.keys().copied()),
            Self::Linear(list) => {
                Box::new(list.iter().map(|(source, target, _)| (*source, *target)))
            }
        }
    }

    // fn into_values(self) -> Box<dyn Iterator<Item = Attributes>> {
    //     match self {
    //         Self::Hash(index) => Box::new(index.into_values()),
    //         Self::Linear(list) => Box::new(list.into_iter().map(|(_, _, attributes)| attributes)),
    //     }
    // }

    fn iter(&self) -> Box<dyn Iterator<Item = ((usize, usize), &Attributes)> + '_> {
        match self {
            Self::Hash(index) => Box::new(
                index
                    .iter()
                    .map(|((source, target), attributes)| ((*source, *target), attributes)),
            ),
            Self::Linear(list) => Box::new(
                list.iter()
                    .map(|(source, target, attributes)| ((*source, *target), attributes)),
            ),
        }
    }

    // fn into_iter(self) -> Box<dyn Iterator<Item = ((usize, usize), Attributes)>> {
    //     match self {
    //         Self::Hash(index) => Box::new(index.into_iter()),
    //         Self::Linear(list) => Box::new(
    //             list.into_iter()
    //                 .map(|(source, target, attributes)| ((source, target), attributes)),
    //         ),
    //     }
    // }
}

enum NodeExtremityType {
    None,
    Source,
    Target,
}

#[derive(Default)]
pub struct GraphBuilderOptions {
    pub linear_edge_store: bool,
    pub undirected: bool,
}

pub struct GraphBuilder {
    options: GraphOptions,
    last_source_index: Option<usize>,
    last_target_index: Option<usize>,
    node_model: Vec<ModelAttribute>,
    edge_model: Vec<ModelAttribute>,
    nodes: IndexMap<String, Attributes, RandomState>,
    edges: EdgeStore,
}

impl GraphBuilder {
    pub fn new(options: GraphBuilderOptions) -> Self {
        let mut graph_options = GraphOptions::default();

        if options.undirected {
            graph_options.graph_type = GraphType::Undirected;
        }

        Self {
            options: graph_options,
            last_source_index: None,
            last_target_index: None,
            node_model: Vec::new(),
            edge_model: Vec::new(),
            nodes: IndexMap::with_hasher(RandomState::new()),
            edges: if options.linear_edge_store {
                EdgeStore::Linear(Vec::new())
            } else {
                EdgeStore::Hash(IndexMap::with_hasher(RandomState::new()))
            },
        }
    }

    #[inline(always)]
    fn is_undirected(&self) -> bool {
        matches!(self.options.graph_type, GraphType::Undirected)
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

    fn add_node_impl(
        &mut self,
        extremity_type: NodeExtremityType,
        key: String,
        attributes_opt: Option<Attributes>,
    ) -> usize {
        use IndexMapEntry::*;

        let cache = match extremity_type {
            NodeExtremityType::None => None,
            NodeExtremityType::Source => self.last_source_index,
            NodeExtremityType::Target => self.last_target_index,
        };

        if let Some(cached_index) = cache {
            let (node, current_attributes) = self.nodes.get_index_mut(cached_index).unwrap();

            if key == **node {
                if let Some(attributes) = attributes_opt {
                    *current_attributes = attributes;
                }

                return cached_index;
            }
        }

        let next_id = self.nodes.len();

        let node_id = match self.nodes.entry(key) {
            Occupied(mut entry) => {
                if let Some(attributes) = attributes_opt {
                    entry.insert(attributes);
                }

                entry.index()
            }
            Vacant(entry) => {
                entry.insert(attributes_opt.unwrap_or_default());

                next_id
            }
        };

        match extremity_type {
            NodeExtremityType::None => (),
            NodeExtremityType::Source => {
                self.last_source_index = Some(node_id);
            }
            NodeExtremityType::Target => {
                self.last_target_index = Some(node_id);
            }
        };

        node_id
    }

    #[inline(always)]
    pub fn add_node(&mut self, key: String, attributes: Attributes) -> usize {
        self.add_node_impl(NodeExtremityType::None, key, Some(attributes))
    }

    #[inline(always)]
    pub fn get_source_node_id(&mut self, key: String) -> usize {
        self.add_node_impl(NodeExtremityType::Source, key, None)
    }

    #[inline(always)]
    pub fn get_target_node_id(&mut self, key: String) -> usize {
        self.add_node_impl(NodeExtremityType::Target, key, None)
    }

    pub fn add_edge_impl(
        &mut self,
        source: usize,
        target: usize,
        attributes_opt: Option<Attributes>,
    ) {
        let undirected = self.is_undirected();

        let (source, target) = if source == target {
            self.options.allow_self_loops = true;
            (source, target)
        } else if undirected && source > target {
            (target, source)
        } else {
            (source, target)
        };

        if self.edges.insert(source, target, attributes_opt) {
            // TODO: merge attributes here?
            self.options.multi = true;
        }
    }

    #[inline(always)]
    pub fn add_edge(&mut self, source: usize, target: usize) {
        self.add_edge_impl(source, target, None);
    }

    #[inline(always)]
    pub fn add_edge_with_attributes(
        &mut self,
        source: usize,
        target: usize,
        attributes: Attributes,
    ) {
        self.add_edge_impl(source, target, Some(attributes));
    }

    pub fn compute_degrees(&self) -> DegreeMap {
        let mut degree_map = DegreeMap::new(self.is_undirected(), self.nodes.len());

        for (source, target) in self.edges.pairs() {
            degree_map.add(source, target);
        }

        degree_map
    }

    pub fn compute_union_find(&self) -> UnionFind {
        let mut sets = UnionFind::with_capacity(self.nodes.len());

        for (source, target) in self.edges.pairs() {
            if source == target {
                continue;
            }

            sets.union(source, target);
        }

        sets
    }

    pub fn compute_union_find_with_largest(&self) -> (UnionFind, usize) {
        let sets = self.compute_union_find();
        let largest = sets.largest().unwrap();
        (sets, largest)
    }

    // fn filter_nodes<F>(&self, predicate: F) -> impl Iterator<Item = (&Rc<String>, &Attributes)>
    // where
    //     F: Fn(&(&Rc<String>, &Attributes)) -> bool,
    // {
    //     self.nodes.iter().filter(predicate)
    // }

    pub fn write_csv_stats(
        &self,
        writer_config: &Config,
        only_largest_component: bool,
    ) -> CliResult<()> {
        let mut writer = writer_config.simd_writer()?;

        let sets = self.compute_union_find();

        let (order, size, components, max_component_size) = if only_largest_component {
            let largest_component = sets.largest().unwrap();

            let order = (0..self.nodes.len())
                .filter(|i| sets.find(*i) == largest_component)
                .count();

            let size = self
                .edges
                .pairs()
                .filter(|(source, _)| sets.find(*source) == largest_component)
                .count();

            (order, size, 1, sets.size(largest_component))
        } else {
            let mut components: usize = 0;
            let mut max_component_size: usize = 0;

            for size in sets.sizes() {
                components += 1;

                if size > max_component_size {
                    max_component_size = size;
                }
            }

            (
                self.nodes.len(),
                self.edges.len(),
                components,
                max_component_size,
            )
        };

        writer.write_record([
            "type",
            "nodes",
            "edges",
            "is_multi",
            "has_self_loops",
            "density",
            "connected_components",
            "largest_connected_component",
        ])?;

        writer.write_record([
            self.options.graph_type.as_str(),
            &order.to_string(),
            &size.to_string(),
            if self.options.multi { "yes" } else { "no" },
            if self.options.allow_self_loops {
                "yes"
            } else {
                "no"
            },
            &density(self.options.graph_type, order, size).to_string(),
            &components.to_string(),
            &max_component_size.to_string(),
        ])?;

        Ok(writer.flush()?)
    }

    pub fn write_csv_components(&self, writer_config: &Config) -> CliResult<()> {
        let mut writer = writer_config.simd_writer()?;

        let sets = self.compute_union_find();

        writer.write_record(["component_size", "arbitrary_node"])?;

        for leader in sets.leaders() {
            writer.write_record([
                leader.size.to_string().as_bytes(),
                self.nodes.get_index(leader.parent).unwrap().0.as_bytes(),
            ])?;
        }

        Ok(writer.flush()?)
    }

    pub fn write_csv_nodelist(
        &self,
        writer_config: &Config,
        only_largest_component: bool,
        compute_degrees: bool,
        union_find: bool,
    ) -> CliResult<()> {
        let sets_opt =
            (only_largest_component || union_find).then(|| self.compute_union_find_with_largest());

        let degree_map_opt = compute_degrees.then(|| self.compute_degrees());

        let mut writer = writer_config.simd_writer()?;

        let mut record = simd_csv::ByteRecord::new();
        record.push_field(b"node");

        for attr in self.node_model.iter() {
            record.push_field(attr.name.as_bytes());
        }

        if let Some(map) = &degree_map_opt {
            record.push_field(b"degree");

            if !map.is_undirected() {
                record.push_field(b"in_degree");
                record.push_field(b"out_degree");
            }
        }

        if union_find {
            record.push_field(b"component");
        }

        writer.write_byte_record(&record)?;

        for (i, (key, attributes)) in self.nodes.iter().enumerate() {
            if only_largest_component {
                let (sets, largest) = sets_opt.as_ref().unwrap();

                if sets.find(i) != *largest {
                    continue;
                }
            }

            record.clear();
            record.push_field(key.as_bytes());

            if !attributes.is_empty() {
                for (_, attr_value) in attributes.iter() {
                    record.push_field(serialize_value_to_csv(attr_value).as_bytes());
                }
            } else {
                for _ in self.node_model.iter() {
                    record.push_field(b"");
                }
            }

            if let Some(map) = &degree_map_opt {
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

            if union_find {
                let (sets, _) = sets_opt.as_ref().unwrap();
                record.push_field(sets.find(i).to_string().as_bytes());
            }

            writer.write_byte_record(&record)?;
        }

        Ok(writer.flush()?)
    }

    pub fn write_gexf(
        &self,
        writer_config: &Config,
        version: &str,
        minify: bool,
        only_largest_component: bool,
    ) -> CliResult<()> {
        let sets_opt = only_largest_component.then(|| self.compute_union_find_with_largest());

        let writer = writer_config.buf_io_writer()?;

        let mut xml_writer = if minify {
            XMLWriter::new_minified(writer)
        } else {
            XMLWriter::new(writer)
        };

        xml_writer.write_declaration()?;

        let gexf_namespace = match version {
            "1.3" => GexfNamespace::one_point_three(),
            "1.2" => GexfNamespace::one_point_two(),
            _ => panic!("unsupported gexf version"),
        };

        let today = Zoned::now().strftime("%F").to_string();
        let node_model = &self.node_model;
        let edge_model = &self.edge_model;

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
            [("defaultedgetype", self.options.graph_type.as_str())],
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
        for (i, (key, attributes)) in self.nodes.iter().enumerate() {
            if let Some((sets, largest)) = &sets_opt {
                if sets.find(i) != *largest {
                    continue;
                }
            }

            let node_label = if let Some(id) = node_label_attr {
                match attributes.get(id) {
                    None => Cow::Borrowed(key.as_str()),
                    Some(v) => Cow::Owned(serialize_value(v)),
                }
            } else {
                Cow::Borrowed(key.as_str())
            };

            if attributes.is_empty() {
                xml_writer.open_empty("node", [("id", key.as_str()), ("label", &node_label)])?;
            } else {
                xml_writer.open("node", [("id", key.as_str()), ("label", &node_label)])?;

                xml_writer.open_no_attributes("attvalues")?;
                for (i, (interner_id, value)) in attributes.iter().enumerate() {
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
        for ((source, target), attributes) in self.edges.iter() {
            if let Some((sets, largest)) = &sets_opt {
                if sets.find(source) != *largest {
                    continue;
                }
            }

            let source_key = self.nodes.get_index(source).unwrap().0;
            let target_key = self.nodes.get_index(target).unwrap().0;

            if attributes.is_empty() {
                xml_writer.open_empty(
                    "edge",
                    [
                        ("source", source_key.as_str()),
                        ("target", target_key.as_str()),
                    ],
                )?;
            } else {
                xml_writer.open(
                    "edge",
                    [
                        ("source", source_key.as_str()),
                        ("target", target_key.as_str()),
                    ],
                )?;

                xml_writer.open_no_attributes("attvalues")?;
                for (i, (_, value)) in attributes.iter().enumerate() {
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

    fn write_json_impl<W: Write, F: Formatter>(
        &self,
        mut serializer: Serializer<W, F>,
        only_largest_component: bool,
    ) -> CliResult<()> {
        struct IteratorSerializer<I>(RefCell<I>, Option<usize>);

        impl<I> IteratorSerializer<I> {
            fn new(inner: I, size_hint: Option<usize>) -> Self {
                Self(RefCell::new(inner), size_hint)
            }
        }

        impl<I, T> serde::Serialize for IteratorSerializer<I>
        where
            I: Iterator<Item = T>,
            T: serde::Serialize,
        {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut seq = serializer.serialize_seq(self.1)?;

                let mut iter = self.0.borrow_mut();

                for item in iter.by_ref() {
                    seq.serialize_element(&item)?;
                }

                seq.end()
            }
        }

        #[derive(Serialize)]
        struct GraphologyNode<'a> {
            key: &'a str,
            #[serde(skip_serializing_if = "Attributes::is_empty")]
            attributes: &'a Attributes,
        }

        #[derive(Serialize)]
        struct GraphologyEdge<'s, 't, 'a> {
            source: &'s str,
            target: &'t str,
            #[serde(skip_serializing_if = "Not::not")]
            undirected: bool,
            #[serde(skip_serializing_if = "Attributes::is_empty")]
            attributes: &'a Attributes,
        }

        let sets_opt = only_largest_component.then(|| self.compute_union_find_with_largest());

        let mut root_map = serializer.serialize_map(Some(3))?;
        root_map.serialize_entry("options", &self.options)?;

        if let Some((sets, largest)) = &sets_opt {
            root_map.serialize_entry(
                "nodes",
                &IteratorSerializer::new(
                    self.nodes
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| sets.find(*i) == *largest)
                        .map(|(_, (key, attributes))| GraphologyNode {
                            key: key.as_ref(),
                            attributes,
                        }),
                    None,
                ),
            )?;

            root_map.serialize_entry(
                "edges",
                &IteratorSerializer::new(
                    self.edges
                        .iter()
                        .filter(|((source, _), _)| sets.find(*source) == *largest)
                        .map(|((source, target), attributes)| {
                            let source_key = self.nodes.get_index(source).unwrap().0;
                            let target_key = self.nodes.get_index(target).unwrap().0;

                            GraphologyEdge {
                                source: source_key.as_ref(),
                                target: target_key.as_ref(),
                                undirected: self.is_undirected(),
                                attributes,
                            }
                        }),
                    Some(self.edges.len()),
                ),
            )?;
        } else {
            root_map.serialize_entry(
                "nodes",
                &IteratorSerializer::new(
                    self.nodes.iter().map(|(key, attributes)| GraphologyNode {
                        key: key.as_ref(),
                        attributes,
                    }),
                    Some(self.nodes.len()),
                ),
            )?;

            root_map.serialize_entry(
                "edges",
                &IteratorSerializer::new(
                    self.edges.iter().map(|((source, target), attributes)| {
                        let source_key = self.nodes.get_index(source).unwrap().0;
                        let target_key = self.nodes.get_index(target).unwrap().0;

                        GraphologyEdge {
                            source: source_key.as_ref(),
                            target: target_key.as_ref(),
                            undirected: self.is_undirected(),
                            attributes,
                        }
                    }),
                    Some(self.edges.len()),
                ),
            )?;
        }

        SerializeMap::end(root_map)?;

        Ok(())
    }

    pub fn write_json(
        &self,
        writer_config: &Config,
        minify: bool,
        only_largest_component: bool,
    ) -> CliResult<()> {
        let mut writer = writer_config.buf_io_writer()?;

        if minify {
            self.write_json_impl(Serializer::new(&mut writer), only_largest_component)?;
        } else {
            self.write_json_impl(Serializer::pretty(&mut writer), only_largest_component)?;
        };

        writeln!(&mut writer)?;

        Ok(writer.flush()?)
    }
}
