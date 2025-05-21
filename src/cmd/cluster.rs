use std::io::{self, Write};

use serde::ser::{Serialize, SerializeStruct, Serializer};

use crate::collections::{hash_map::Entry, HashMap};
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
    let mut clustering: Box<dyn ClusteringAlgorithm> = Box::<KeyCollision>::default();

    let mut record = csv::ByteRecord::new();
    let mut index: usize = 0;

    while rdr.read_byte_record(&mut record)? {
        let value = String::from_utf8(record[sel_index].to_vec()).unwrap();
        let key = program.generate_key(index, &record)?;

        clustering.process(index, key, value);

        index += 1;
    }

    let mut clusters = clustering.into_clusters();

    clusters.sort_by(|a, b| {
        b.values
            .len()
            .cmp(&a.values.len())
            .then_with(|| b.rows.len().cmp(&a.rows.len()))
            .then_with(|| a.best().cmp(b.best()))
    });

    let mut writer = Config::new(&args.flag_output).io_writer()?;

    for cluster in clusters {
        cluster.write_toml(&mut writer)?;
    }

    Ok(())
}

#[derive(Debug)]
struct Cluster {
    id: usize,
    key: String,
    rows: Vec<usize>,
    values: Vec<(String, usize)>,
}

impl Cluster {
    fn write_toml<W: Write>(&self, mut writer: W) -> io::Result<()> {
        writeln!(&mut writer, "[[cluster]]")?;
        writeln!(&mut writer, "id = {}", self.id)?;
        writeln!(&mut writer, "key = \"{}\"", self.key)?;
        writeln!(&mut writer, "nb_values = {}", self.values.len())?;
        writeln!(&mut writer, "nb_rows = {}", self.rows.len())?;
        writeln!(
            &mut writer,
            "rows = \"{}\"",
            self.rows
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )?;
        writeln!(&mut writer, "replace_with = {:?}", self.best())?;
        writeln!(&mut writer, "values = [")?;

        for (value, count) in self.values.iter() {
            writeln!(
                &mut writer,
                "  {{ value = {:?}, count = {} }},",
                value, count
            )?;
        }

        writeln!(&mut writer, "]")?;
        writeln!(&mut writer, "harmonize = false")?;
        writeln!(&mut writer)?;

        Ok(())
    }
}

impl Serialize for Cluster {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Cluster", 8)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("key", &self.key)?;
        state.serialize_field("nb_values", &self.values.len())?;
        state.serialize_field("nb_rows", &self.rows.len())?;
        state.serialize_field(
            "rows",
            &self
                .rows
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(","),
        )?;
        state.serialize_field("replace_with", self.best())?;
        state.serialize_field("values", &self.values)?;
        state.serialize_field("harmonize", &false)?;
        state.end()
    }
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

        let mut values = values.into_iter().collect::<Vec<_>>();
        values.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0)));

        Cluster {
            id,
            key,
            rows,
            values,
        }
    }

    fn best(&self) -> &String {
        &self.values[0].0
    }
}

trait ClusteringAlgorithm {
    fn process(&mut self, index: usize, key: String, value: String);
    fn into_clusters(self: Box<Self>) -> Vec<Cluster>;
}

#[derive(Default)]
struct KeyCollision {
    collisions: HashMap<String, Vec<(usize, String)>>,
}

impl ClusteringAlgorithm for KeyCollision {
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

    fn into_clusters(self: Box<Self>) -> Vec<Cluster> {
        self.collisions
            .into_iter()
            .enumerate()
            .map(|(id, (key, entries))| Cluster::from_entries(id, key, entries))
            .filter(|cluster| cluster.values.len() > 1)
            .collect()
    }
}
