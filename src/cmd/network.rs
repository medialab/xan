use crate::collections::IncrementalId;
use crate::config::{Config, Delimiter};
use crate::graph::{GraphBuilder, GraphBuilderOptions};
use crate::json::{Attributes, JSONEmptyMode, JSONTypeInferrenceBuffer};
use crate::select::{SelectedColumns, Selection};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Process CSV data to build a network (nodes & edges) so you can produce a variety
of output ranging from graph data formats (e.g. json or gexf) or other CSV
outputs that can be useful to interpret network information easily when piped
into other xan commands.

Supported input modes:
    `edgelist`:  converts a CSV of edges with a column representing
                 sources and another column targets.
    `bipartite`: converts a CSV with two columns representing the
                 edges between both parts of a bipartite graph.

Supported output formats (-f, --format):
    `json`     - Graphology JSON serialization format
                 ref: https://graphology.github.io/serialization.html
    `gexf`     - Graph eXchange XML Format
                 ref: https://gexf.net/
    `nodelist` - CSV nodelist, with optional degrees if using -D/--degrees
    `stats`    - Single CSV row of useful graph statistics (number of nodes, edges,
                 graph type, density etc.)

Usage:
    xan network edgelist [options] <source> <target> [<input>]
    xan network bipartite [options] <part1> <part2> [<input>]
    xan network --help

output format options:
    -f, --format <format>     One of \"json\", \"gexf\", \"stats\" or \"nodelist\".
                              [default: json]
    --gexf-version <version>  GEXF version to output. Can be one of \"1.2\"
                              or \"1.3\".
                              [default: 1.2]
    --minify                  Whether to minify json or gexf output.

xan network options:
    -L, --largest-component   Only keep the largest connected component
                              in the resulting graph.
    -S, --simple              Use to indicate you know beforehand that processed
                              graph is simple, i.e. it does not contains multiple
                              edges for a same (source, target) pair. This can
                              improve performance of the overall process.
    -D, --degrees             Whether to compute node degrees so it can be added
                              to relevant outputs. Currently only relevant
                              when using -f \"nodelist\".

edgelist options:
    -U, --undirected       Whether the graph is undirected.
    --nodes <path>         Path to a CSV file containing node metadata
                           (use \"-\" to feed the file from stdin).
    --node-column <name>   Name of the column containing node keys.
                           [default: node]

bipartite options:
    --disjoint-keys  Pass this if you know both partitions of the graph
                         use disjoint sets of keys (i.e. if you know they share
                         no common keys at all). Incorrect graphs will be produced
                         if some keys are used by both partitions!

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
    cmd_edgelist: bool,
    cmd_bipartite: bool,
    arg_input: Option<String>,
    arg_source: Option<SelectedColumns>,
    arg_target: Option<SelectedColumns>,
    arg_part1: Option<SelectedColumns>,
    arg_part2: Option<SelectedColumns>,
    flag_format: String,
    flag_gexf_version: String,
    flag_minify: bool,
    flag_largest_component: bool,
    flag_simple: bool,
    flag_undirected: bool,
    flag_nodes: Option<String>,
    flag_node_column: SelectedColumns,
    flag_disjoint_keys: bool,
    flag_degrees: bool,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
}

impl Args {
    fn graph_builder(&self) -> GraphBuilder {
        let options = GraphBuilderOptions {
            undirected: self.flag_undirected,
            linear_edge_store: self.flag_simple,
        };

        GraphBuilder::new(options)
    }

    fn edgelist(&self) -> CliResult<GraphBuilder> {
        let edges_rconf = Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers);

        let mut graph_builder = self.graph_builder();

        let mut record = csv::StringRecord::new();

        if let Some(nodes_path) = &self.flag_nodes {
            let nodes_rconf = Config::new(&Some(nodes_path.clone()))
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            let mut node_reader = nodes_rconf.reader()?;
            let node_headers = node_reader.byte_headers()?.clone();

            let node_column_index = self
                .flag_node_column
                .single_selection(&node_headers, !nodes_rconf.no_headers)?;

            let node_attr_sel =
                Selection::without_indices(node_headers.len(), &[node_column_index]);

            let mut node_attr_inferrence = JSONTypeInferrenceBuffer::new(
                node_attr_sel.clone(),
                Some(512),
                JSONEmptyMode::Empty,
            );

            node_attr_inferrence.read(&mut node_reader)?;

            let node_headers = node_reader.headers()?.clone();

            graph_builder.set_node_model(
                node_attr_sel.select(&node_headers),
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
            .single_selection(&edge_headers, !edges_rconf.no_headers)?;
        let target_column_index = self
            .arg_target
            .as_ref()
            .unwrap()
            .single_selection(&edge_headers, !edges_rconf.no_headers)?;

        let edge_attr_sel = Selection::without_indices(
            edge_headers.len(),
            &[source_column_index, target_column_index],
        );

        let mut edge_attr_inferrence =
            JSONTypeInferrenceBuffer::new(edge_attr_sel.clone(), Some(512), JSONEmptyMode::Empty);

        edge_attr_inferrence.read(&mut edge_reader)?;

        let edge_headers = edge_reader.headers()?.clone();

        graph_builder.set_edge_model(
            edge_attr_sel.select(&edge_headers),
            edge_attr_inferrence.types(),
        );

        let mut process_edge_record = |record: &csv::StringRecord| {
            let source = record[source_column_index].to_string();
            let target = record[target_column_index].to_string();

            let source_id = graph_builder.add_source_node(source, Attributes::default());
            let target_id = graph_builder.add_target_node(target, Attributes::default());

            let mut attributes = Attributes::with_capacity(edge_attr_sel.len());

            for (k, v) in edge_attr_inferrence.cast(&edge_headers, record).flatten() {
                attributes.insert(k, v);
            }

            graph_builder.add_edge(source_id, target_id, attributes);
        };

        for buffered_record in edge_attr_inferrence.records() {
            process_edge_record(buffered_record);
        }

        while edge_reader.read_record(&mut record)? {
            process_edge_record(&record);
        }

        Ok(graph_builder)
    }

    fn bipartite(&self) -> CliResult<GraphBuilder> {
        let rconf = Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers);

        let mut graph_builder = self.graph_builder();

        let mut reader = rconf.reader()?;
        let mut record = csv::StringRecord::new();

        let headers = reader.byte_headers()?.clone();

        let first_part_index = self
            .arg_part1
            .as_ref()
            .unwrap()
            .single_selection(&headers, !rconf.no_headers)?;

        let second_part_index = self
            .arg_part2
            .as_ref()
            .unwrap()
            .single_selection(&headers, !rconf.no_headers)?;

        let mut incremental_id =
            (!self.flag_disjoint_keys).then(IncrementalId::<(usize, String)>::new);

        while reader.read_record(&mut record)? {
            let mut first_part_node = record[first_part_index].to_string();
            let mut second_part_node = record[second_part_index].to_string();

            if let Some(id) = incremental_id.as_mut() {
                first_part_node = id.get((0, first_part_node)).to_string();
                second_part_node = id.get((1, second_part_node)).to_string();
            }

            let first_part_node_id =
                graph_builder.add_source_node(first_part_node, Attributes::default());

            let second_part_node_id =
                graph_builder.add_target_node(second_part_node, Attributes::default());

            graph_builder.add_edge(
                first_part_node_id,
                second_part_node_id,
                Attributes::default(),
            );
        }

        Ok(graph_builder)
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_degrees && args.flag_format != "nodelist" {
        Err("-D/--degrees is only relevant with -f nodelist!")?;
    }

    if args.flag_minify && !(args.flag_format == "json" || args.flag_format == "gexf") {
        Err("--minify is only relevant with -f (json|gexf)!")?;
    }

    let wconf = Config::new(&args.flag_output);

    if !["1.2", "1.3"].contains(&args.flag_gexf_version.as_str()) {
        Err(format!(
            "unsupported gexf version: {}",
            args.flag_gexf_version
        ))?;
    }

    let builder = (if args.cmd_edgelist {
        args.edgelist()
    } else if args.cmd_bipartite {
        args.bipartite()
    } else {
        unreachable!()
    })?;

    match args.flag_format.as_str() {
        "stats" => builder.write_csv_stats(&wconf, args.flag_largest_component),
        "nodelist" => {
            builder.write_csv_nodelist(&wconf, args.flag_largest_component, args.flag_degrees)
        }
        "gexf" => builder.write_gexf(
            &wconf,
            &args.flag_gexf_version,
            args.flag_minify,
            args.flag_largest_component,
        ),
        "json" => builder.write_json(&wconf, args.flag_minify, args.flag_largest_component),
        _ => Err(format!("unsupported output format: {}!", &args.flag_format))?,
    }
}
