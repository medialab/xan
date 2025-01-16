use crate::config::{Config, Delimiter};
use crate::graph::GraphBuilder;
use crate::json::{Attributes, JSONEmptyMode, JSONTypeInferrenceBuffer};
use crate::select::{SelectColumns, Selection};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Convert CSV data to graph data.

Supported formats:
    json - Graphology JSON serialization format
           ref: https://graphology.github.io/serialization.html
    gexf - Graph eXchange XML Format
           ref: https://gexf.net/

Supported modes:
    edgelist: converts a CSV of edges with a column representing
              sources and another column targets.

Usage:
    xan network edgelist [options] <source> <target> [<input>]
    xan network --help

xan network options:
    -f, --format <format>     One of \"json\" or \"gexf\".
                              [default: json]
    --gexf-version <version>  GEXF version to output. Can be one of \"1.2\"
                              or \"1.3\".
                              [default: 1.2]
    -L, --largest-component   Only keep the largest connected component
                              in the resulting graph.

network edgelist options:
    -U, --undirected       Whether the graph is undirected.
    --nodes <path>         Path to a CSV file containing node metadata
                           (use \"-\" to feed the file from stdin).
    --node-column <name>   Name of the column containing node keys.
                           [default: node]

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
    flag_format: String,
    flag_gexf_version: String,
    flag_largest_component: bool,
    flag_undirected: bool,
    flag_nodes: Option<String>,
    flag_node_column: SelectColumns,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
}

impl Args {
    fn edgelist(&self) -> CliResult<GraphBuilder> {
        let edges_rconf = Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers);

        let mut graph_builder = GraphBuilder::default();
        let mut record = csv::StringRecord::new();

        if let Some(nodes_path) = &self.flag_nodes {
            let nodes_rconf = Config::new(&Some(nodes_path.clone()))
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            let mut node_reader = nodes_rconf.reader()?;
            let node_headers = node_reader.byte_headers()?.clone();

            let node_column_index = self
                .flag_node_column
                .single_selection(&node_headers, !self.flag_no_headers)?;

            let node_attr_sel =
                Selection::without_indices(node_headers.len(), &[node_column_index]);

            let mut node_attr_inferrence =
                JSONTypeInferrenceBuffer::new(node_attr_sel.clone(), 512, JSONEmptyMode::Omit);

            node_attr_inferrence.read(&mut node_reader)?;

            let node_headers = node_reader.headers()?.clone();

            graph_builder.set_node_model(
                node_attr_sel.select_string_record(&node_headers),
                node_attr_inferrence.types(),
            );

            let mut process_node_record = |record: &csv::StringRecord| {
                let key = record[node_column_index].to_string();

                let mut attributes = Attributes::with_capacity(node_attr_sel.len());

                for (k, v) in node_attr_inferrence.cast(&node_headers, record).flatten() {
                    attributes.insert(k, v);
                }

                graph_builder.add_node(key, attributes);
            };

            for buffered_record in node_attr_inferrence.records() {
                process_node_record(buffered_record);
            }

            while node_reader.read_record(&mut record)? {
                process_node_record(&record);
            }
        }

        let mut edge_reader = edges_rconf.reader()?;
        let edge_headers = edge_reader.byte_headers()?.clone();

        let source_column_index = self
            .arg_source
            .as_ref()
            .unwrap()
            .single_selection(&edge_headers, !self.flag_no_headers)?;
        let target_column_index = self
            .arg_target
            .as_ref()
            .unwrap()
            .single_selection(&edge_headers, !self.flag_no_headers)?;

        let edge_attr_sel = Selection::without_indices(
            edge_headers.len(),
            &[source_column_index, target_column_index],
        );

        let mut edge_attr_inferrence =
            JSONTypeInferrenceBuffer::new(edge_attr_sel.clone(), 512, JSONEmptyMode::Omit);

        edge_attr_inferrence.read(&mut edge_reader)?;

        if self.flag_undirected {
            graph_builder.mark_as_undirected();
        }

        if self.flag_largest_component {
            graph_builder.keep_largest_component();
        }

        let edge_headers = edge_reader.headers()?.clone();

        graph_builder.set_edge_model(
            edge_attr_sel.select_string_record(&edge_headers),
            edge_attr_inferrence.types(),
        );

        let mut process_edge_record = |record: &csv::StringRecord| {
            let source = record[source_column_index].to_string();
            let target = record[target_column_index].to_string();

            let source_id = graph_builder.add_node(source, Attributes::default());
            let target_id = graph_builder.add_node(target, Attributes::default());

            let mut attributes = Attributes::with_capacity(edge_attr_sel.len());

            for (k, v) in edge_attr_inferrence.cast(&edge_headers, record).flatten() {
                attributes.insert(k, v);
            }

            graph_builder.add_edge(source_id, target_id, attributes, self.flag_undirected);
        };

        for buffered_record in edge_attr_inferrence.records() {
            process_edge_record(buffered_record);
        }

        while edge_reader.read_record(&mut record)? {
            process_edge_record(&record);
        }

        Ok(graph_builder)
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let mut writer = Config::new(&args.flag_output).io_writer()?;

    if !["1.2", "1.3"].contains(&args.flag_gexf_version.as_str()) {
        Err(format!(
            "unsupported gexf version: {}",
            args.flag_gexf_version
        ))?;
    }

    let graph_builder = args.edgelist()?;

    match args.flag_format.as_str() {
        "gexf" => graph_builder.write_gexf(&mut writer, &args.flag_gexf_version),
        "json" => graph_builder.write_json(&mut writer),
        _ => Err(format!("unsupported format: {}!", &args.flag_format))?,
    }
}
