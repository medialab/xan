use std::collections::{hash_map::Entry, HashMap};

use crate::config::{Config, Delimiter};
use crate::moonblade::Program;
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
TODO...

Usage:
    xan cluster <column> [options] [<input>]
    xan cluster --help

cluster options:
    -k, --key <expr>  An expression to evaluate to generate a key
                      for each row by transforming the selected cell.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_column: SelectColumns,
    arg_input: Option<String>,
    flag_key: Option<String>,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column);

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?;

    let sel_index = rconf.single_selection(headers)?;

    let key_expr = match &args.flag_key {
        Some(expr) => format!("col({}) | {}", sel_index, expr),
        None => format!("col({})", sel_index),
    };

    let program = Program::parse(&key_expr, headers)?;
    let mut clustering = ClusteringAlgorithm::KeyCollision(KeyCollision::default());

    let mut record = csv::ByteRecord::new();
    let mut index: usize = 0;

    while rdr.read_byte_record(&mut record)? {
        let value = String::from_utf8(record[sel_index].to_vec()).unwrap();
        let key = program.generate_key(index, &record)?;

        clustering.process(index, key, value);

        index += 1;
    }

    for cluster in clustering.into_clusters() {
        dbg!(cluster);
    }

    Ok(())
}

#[derive(Debug)]
struct Cluster {
    id: usize,
    key: String,
    rows: Vec<usize>,
    values: HashMap<String, usize>,
}

impl Cluster {
    fn from_entries(id: usize, key: String, entries: Vec<(usize, String)>) -> Self {
        let mut rows = Vec::new();
        let mut values = HashMap::new();

        for (row_index, row_value) in entries {
            rows.push(row_index);
            values
                .entry(row_value)
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }

        Cluster {
            id,
            key,
            rows,
            values,
        }
    }
}

#[derive(Default)]
struct KeyCollision {
    collisions: HashMap<String, Vec<(usize, String)>>,
}

impl KeyCollision {
    fn process(&mut self, index: usize, key: String, value: String) {
        match self.collisions.entry(key) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().push((index, value));
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![(index, value)]);
            }
        };
    }

    fn into_clusters(self) -> impl Iterator<Item = Cluster> {
        self.collisions
            .into_iter()
            .enumerate()
            .map(|(id, (key, entries))| Cluster::from_entries(id, key, entries))
            .filter(|cluster| cluster.values.len() > 1)
    }
}
macro_rules! build_clustering_algorithm_enum {
    ($($variant: ident,)+) => {
        enum ClusteringAlgorithm {
            $(
                $variant($variant),
            )+
        }

        impl ClusteringAlgorithm {
            fn process(&mut self, index: usize,key: String, value: String) {
                match self {
                    $(
                        Self::$variant(inner) => inner.process(index, key, value),
                    )+
                };
            }

            fn into_clusters(self) -> impl Iterator<Item=Cluster> {
                match self {
                    $(
                        Self::$variant(inner) => inner.into_clusters(),
                    )+
                }
            }
        }
    };
}

build_clustering_algorithm_enum!(KeyCollision,);
