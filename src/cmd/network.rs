use std::collections::HashMap;
use std::mem::swap;
use std::ops::Not;
use std::rc::Rc;

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

static USAGE: &str = "
TODO...

Usage:
    xan network edgelist [options] <source> <target> [<input>]
    xan network --help

xan network edgelist options:
    -U, --undirected  Whether the graph is undirected.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize, Debug)]
struct Args {
    arg_input: Option<String>,
    arg_source: Option<SelectColumns>,
    arg_target: Option<SelectColumns>,
    flag_undirected: bool,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
}

impl Args {
    fn edgelist(self) -> CliResult<Graph> {
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

        let mut graph_options = GraphOptions::default();

        if self.flag_undirected {
            graph_options.graph_type = GraphType::Undirected;
        }

        let mut nodes: HashMap<Rc<String>, Node> = HashMap::new();
        let mut edges: HashMap<(Rc<String>, Rc<String>), Edge> = HashMap::new();

        while edge_reader.read_byte_record(&mut record)? {
            let mut source =
                Rc::new(String::from_utf8(record[source_column_index].to_vec()).unwrap());
            let mut target =
                Rc::new(String::from_utf8(record[target_column_index].to_vec()).unwrap());

            if source == target {
                graph_options.allow_self_loops = true;
            } else if self.flag_undirected && source > target {
                swap(&mut source, &mut target);
            }

            let source = nodes
                .entry(source.clone())
                .or_insert_with(|| Node {
                    key: source.clone(),
                })
                .key
                .clone();
            let target = nodes
                .entry(target.clone())
                .or_insert_with(|| Node {
                    key: target.clone(),
                })
                .key
                .clone();

            let edge = Edge {
                source: source.clone(),
                target: target.clone(),
                undirected: false,
            };

            if edges.insert((source, target), edge).is_some() {
                graph_options.multi = true;
            }
        }

        let graph = Graph {
            options: graph_options,
            nodes: nodes.into_values().collect(),
            edges: edges.into_values().collect(),
        };

        Ok(graph)
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let mut writer = Config::new(&args.flag_output).io_writer()?;

    let graph = args.edgelist()?;

    serde_json::to_writer_pretty(&mut writer, &graph)?;
    writeln!(&mut writer)?;

    Ok(())
}
