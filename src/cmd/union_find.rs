use std::collections::{hash_map::Entry, HashMap};

use csv;

use config::{Config, Delimiter};
use select::SelectColumns;
use util;
use CliResult;

#[derive(Debug)]
struct UnionFindEntry {
    parent: usize,
    size: usize,
}

#[derive(Debug)]
struct UnionFind {
    entries: Vec<UnionFindEntry>,
}

impl UnionFind {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn make_set(&mut self) -> usize {
        let i = self.entries.len();

        self.entries.push(UnionFindEntry { parent: i, size: 1 });

        i
    }

    fn find(&mut self, mut x: usize) -> usize {
        let mut root = x;

        loop {
            let parent = self.entries[root].parent;

            if parent == root {
                break;
            }

            root = parent;
        }

        // Path compression
        loop {
            let entry = &mut self.entries[x];

            if entry.parent == root {
                break;
            }

            let parent = entry.parent;
            entry.parent = root;
            x = parent;
        }

        root
    }

    fn union(&mut self, mut x: usize, mut y: usize) {
        x = self.find(x);
        y = self.find(y);

        if x == y {
            return;
        }

        let x_size = self.entries[x].size;
        let y_size = self.entries[y].size;

        if x_size > y_size {
            self.entries[y].parent = x;
            self.entries[x].size += y_size;
        } else {
            self.entries[x].parent = y;
            self.entries[y].size += x_size;
        }
    }

    fn leaders(&self) -> impl Iterator<Item = &UnionFindEntry> {
        self.entries.iter().enumerate().filter_map(|(i, entry)| {
            if i != entry.parent {
                None
            } else {
                Some(entry)
            }
        })
    }

    fn largest(&self) -> Option<usize> {
        let mut max: Option<&UnionFindEntry> = None;

        for entry in self.leaders() {
            match max {
                None => {
                    max = Some(entry);
                }
                Some(current_entry) => {
                    if entry.size > current_entry.size {
                        max = Some(entry);
                    }
                }
            }
        }

        max.map(|entry| entry.parent)
    }

    fn sizes(&self) -> impl Iterator<Item = usize> + '_ {
        self.leaders().map(|entry| entry.size)
    }
}

type Bytes = Vec<u8>;

#[derive(Debug)]
struct UnionFindHashMap {
    inner: UnionFind,
    map: HashMap<Bytes, usize>,
}

impl UnionFindHashMap {
    fn new() -> Self {
        Self {
            inner: UnionFind::new(),
            map: HashMap::new(),
        }
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn get(&mut self, node: Bytes) -> usize {
        match self.map.entry(node) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => *entry.insert(self.inner.make_set()),
        }
    }

    fn union(&mut self, source: Bytes, target: Bytes) {
        let x = self.get(source);
        let y = self.get(target);

        self.inner.union(x, y);
    }

    fn nodes(self) -> impl Iterator<Item = (Bytes, usize)> {
        let mut inner = self.inner;

        self.map.into_iter().map(move |(node, i)| {
            let label = inner.find(i);

            (node, label)
        })
    }

    fn largest_component(self) -> impl Iterator<Item = Bytes> {
        let largest = self.inner.largest().unwrap();
        let mut inner = self.inner;

        self.map.into_iter().flat_map(move |(node, i)| {
            if inner.find(i) == largest {
                Some(node)
            } else {
                None
            }
        })
    }

    fn sizes(&self) -> impl Iterator<Item = usize> + '_ {
        self.inner.sizes()
    }
}

static USAGE: &str = "
Apply the union-find algorithm on a CSV file representing a graph's
edge list (one column for source nodes, one column for target nodes) in
order to return a CSV of nodes with a component label.

The command can also return only the nodes belonging to the largest connected
component using the -L/--largest flag or the sizes of all the connected
components of the graph using the -S/--sizes flag.

Usage:
    xan union-find <source> <target> [options] [<input>]
    xan union-find --help

union-find options:
    -L, --largest  Only return nodes belonging to the largest component.
                   The output CSV file will only contain a 'node' column in
                   this case.
    -S, --sizes    Return a single CSV column containing the sizes of the graph's
                   various connected components.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_source: SelectColumns,
    arg_target: SelectColumns,
    flag_largest: bool,
    flag_sizes: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
    flag_no_headers: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut rdr = conf.reader()?;

    let headers = rdr.byte_headers()?;

    let source_index = Config::new(&None)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_source)
        .single_selection(headers)?;

    let target_index = Config::new(&None)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_target)
        .single_selection(headers)?;

    let mut wtr = Config::new(&args.flag_output).writer()?;
    let mut record = csv::ByteRecord::new();

    let mut union_find = UnionFindHashMap::new();

    while rdr.read_byte_record(&mut record)? {
        let source = record[source_index].to_vec();
        let target = record[target_index].to_vec();

        union_find.union(source, target);
    }

    record.clear();

    if args.flag_sizes {
        record.push_field(b"size");
    } else {
        record.push_field(b"node");

        if !args.flag_largest {
            record.push_field(b"component");
        }
    }

    wtr.write_byte_record(&record)?;

    if args.flag_largest {
        if union_find.is_empty() {
            return Ok(wtr.flush()?);
        }

        for node in union_find.largest_component() {
            record.clear();
            record.push_field(&node);

            wtr.write_byte_record(&record)?;
        }
    } else if args.flag_sizes {
        for size in union_find.sizes() {
            record.clear();
            record.push_field(size.to_string().as_bytes());

            wtr.write_byte_record(&record)?;
        }
    } else {
        for (node, label) in union_find.nodes() {
            record.clear();
            record.push_field(&node);
            record.push_field(label.to_string().as_bytes());

            wtr.write_byte_record(&record)?;
        }
    }

    Ok(wtr.flush()?)
}
