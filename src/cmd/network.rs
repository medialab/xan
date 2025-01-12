use std::collections::HashMap;
use std::ops::Not;
use std::rc::Rc;

use indexmap::{map::Entry, IndexMap};

use crate::collections::UnionFind;
use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

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
struct Graph {
    options: GraphOptions,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

#[derive(Default)]
struct GraphBuilder {
    options: GraphOptions,
    disjoint_sets: Option<UnionFind>,
    nodes: IndexMap<Rc<String>, Node>,
    edges: HashMap<(usize, usize), Edge>,
}

impl GraphBuilder {
    fn mark_as_undirected(&mut self) {
        self.options.graph_type = GraphType::Undirected;
    }

    fn keep_largest_component(&mut self) {
        self.disjoint_sets = Some(UnionFind::new());
    }

    fn add_node(&mut self, key: String) -> usize {
        let rc_key = Rc::new(key);
        let next_id = self.nodes.len();

        match self.nodes.entry(rc_key.clone()) {
            Entry::Occupied(entry) => entry.index(),
            Entry::Vacant(entry) => {
                entry.insert(Node { key: rc_key });

                if let Some(sets) = self.disjoint_sets.as_mut() {
                    sets.make_set();
                }

                next_id
            }
        }
    }

    fn add_edge(&mut self, source: usize, target: usize, undirected: bool) {
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
        };

        if self.edges.insert((source, target), edge).is_some() {
            self.options.multi = true;
        }

        if let Some(sets) = self.disjoint_sets.as_mut() {
            sets.union(source, target);
        }
    }

    fn build(self) -> Graph {
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

static USAGE: &str = "
TODO...

Usage:
    xan network edgelist [options] <source> <target> [<input>]
    xan network --help

xan network options:
    -L, --largest-component  Only keep the largest connected component
                             in the resulting graph.

xan network edgelist options:
    -U, --undirected  Whether the graph is undirected.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter foDirectedr reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize, Debug)]
struct Args {
    arg_input: Option<String>,
    arg_source: Option<SelectColumns>,
    arg_target: Option<SelectColumns>,
    flag_largest_component: bool,
    flag_undirected: bool,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
}

impl Args {
    fn edgelist(self) -> CliResult<GraphBuilder> {
        let rconf = Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers);

        let mut edge_reader = rconf.reader()?;
        let edge_headers = edge_reader.byte_headers()?.clone();

        let source_column_index = self
            .arg_source
            .unwrap()
            .single_selection(&edge_headers, !self.flag_no_headers)?;
        let target_column_index = self
            .arg_target
            .unwrap()
            .single_selection(&edge_headers, !self.flag_no_headers)?;

        let mut record = csv::ByteRecord::new();
        let mut graph_builder = GraphBuilder::default();

        if self.flag_undirected {
            graph_builder.mark_as_undirected();
        }

        if self.flag_largest_component {
            graph_builder.keep_largest_component();
        }

        while edge_reader.read_byte_record(&mut record)? {
            let source = String::from_utf8(record[source_column_index].to_vec()).unwrap();
            let target = String::from_utf8(record[target_column_index].to_vec()).unwrap();

            let source_id = graph_builder.add_node(source);
            let target_id = graph_builder.add_node(target);

            graph_builder.add_edge(source_id, target_id, self.flag_undirected);
        }

        Ok(graph_builder)
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let mut writer = Config::new(&args.flag_output).io_writer()?;

    let graph_builder = args.edgelist()?;

    serde_json::to_writer_pretty(&mut writer, &graph_builder.build())?;
    writeln!(&mut writer)?;

    Ok(())
}
