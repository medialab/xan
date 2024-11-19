use std::collections::{hash_map::Entry, HashMap, HashSet};

// NOTE: keep this library in check: https://github.com/sweet-security/candystore
use csv;
use dlv_list::{Index, VecList};
use transient_btree_index::{BtreeConfig, BtreeIndex};

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Deduplicate the rows of a CSV file. Runs in O(n) time, consuming O(c) memory, c being
the distinct number of row identities.

If your file is already sorted on the deduplication selection, use the -S/--sorted flag
to run in O(1) memory instead.

Note that it will be the first row having a specific identity that will be emitted in
the output and not any subsequent one.

Usage:
    xan dedup [options] [<input>]
    xan dedup --help

dedup options:
    --check             Verify whether the selection has any duplicates, i.e. whether
                        the selected columns satisfy a uniqueness constraint.
    -s, --select <arg>  Select a subset of columns to on which to deduplicate.
                        See 'xan select --help' for the format details.
    -S, --sorted        Use if you know your file is already sorted on the deduplication
                        selection to avoid storing unique values in memory.
    -l, --keep-last     Keep the last row having a specific identiy, rather than
                        the first one. Note that it will cost more memory and that
                        no rows will be flushed before the whole file has been read
                        if -S/--sorted is not used.
    -e, --external      Use an external btree index to keep the index on disk and avoid
                        overflowing RAM. Does not work with -l/--keep-last.

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
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_check: bool,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_sorted: bool,
    flag_keep_last: bool,
    flag_external: bool,
}

type DeduplicationKey = Vec<Vec<u8>>;

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;

    if args.flag_external {
        if args.flag_keep_last {
            Err("-l/--keep-last does not work with -e/--external!")?;
        }

        if args.flag_check {
            Err("--check does not work with -e/--external yet!")?;
        }
    }

    if args.flag_sorted {
        args.flag_external = false;
    }

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);

    let mut rdr = rconf.reader()?;
    let sel = rconf.selection(rdr.byte_headers()?)?;

    if args.flag_check {
        let mut record = csv::ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            let mut already_seen = HashSet::<DeduplicationKey>::new();

            while rdr.read_byte_record(&mut record)? {
                let key = sel.collect(&record);

                if !already_seen.insert(key) {
                    Err("selection is NOT unique!")?;
                }
            }
        }

        println!("selection is unique!");

        return Ok(());
    }

    let mut wtr = Config::new(&args.flag_output).writer()?;

    rconf.write_headers(&mut rdr, &mut wtr)?;

    if args.flag_external {
        let mut record = csv::ByteRecord::new();

        let mut btree_index = BtreeIndex::<Vec<Vec<u8>>, ()>::with_capacity(
            BtreeConfig::default().fixed_value_size(0),
            1024,
        )?;

        while rdr.read_byte_record(&mut record)? {
            let key = sel.collect(&record);

            if btree_index.insert(key, ())?.is_none() {
                wtr.write_byte_record(&record)?;
            }
        }
    }

    match (args.flag_sorted, args.flag_keep_last) {
        // Unsorted, keep first
        (false, false) => {
            let mut record = csv::ByteRecord::new();
            let mut already_seen = HashSet::<DeduplicationKey>::new();

            while rdr.read_byte_record(&mut record)? {
                let key = sel.collect(&record);

                if already_seen.insert(key) {
                    wtr.write_byte_record(&record)?;
                }
            }
        }

        // Unsorted, keep last
        (false, true) => {
            let mut set = KeepLastSet::new();

            for result in rdr.byte_records() {
                let record = result?;
                let key = sel.collect(&record);
                set.push(key, record);
            }

            for record in set.into_iter() {
                wtr.write_byte_record(&record)?;
            }
        }

        // Sorted, keep first
        (true, false) => {
            let mut record = csv::ByteRecord::new();
            let mut current: Option<DeduplicationKey> = None;

            while rdr.read_byte_record(&mut record)? {
                let key = sel.collect(&record);

                match current {
                    None => {
                        wtr.write_byte_record(&record)?;
                        current = Some(key);
                    }
                    Some(current_key) if current_key != key => {
                        wtr.write_byte_record(&record)?;
                        current = Some(key);
                    }
                    _ => (),
                };
            }
        }

        // Sorted, keep last
        (true, true) => {
            let mut current: Option<(DeduplicationKey, csv::ByteRecord)> = None;

            for result in rdr.byte_records() {
                let record = result?;
                let key = sel.collect(&record);

                match current {
                    Some((current_key, record_to_flush)) if current_key != key => {
                        wtr.write_byte_record(&record_to_flush)?;
                    }
                    _ => (),
                }

                current = Some((key, record));
            }

            if let Some((_, record_to_flush)) = current {
                wtr.write_byte_record(&record_to_flush)?;
            }
        }
    }

    Ok(wtr.flush()?)
}

struct KeepLastSet {
    map: HashMap<DeduplicationKey, Index<csv::ByteRecord>>,
    list: VecList<csv::ByteRecord>,
}

impl KeepLastSet {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            list: VecList::new(),
        }
    }

    fn push(&mut self, key: DeduplicationKey, record: csv::ByteRecord) {
        match self.map.entry(key) {
            Entry::Occupied(mut entry) => {
                let current_index = entry.get_mut();

                self.list.remove(*current_index);
                *current_index = self.list.push_back(record);
            }
            Entry::Vacant(entry) => {
                entry.insert(self.list.push_back(record));
            }
        };
    }

    fn into_iter(self) -> impl Iterator<Item = csv::ByteRecord> {
        self.list.into_iter()
    }
}
