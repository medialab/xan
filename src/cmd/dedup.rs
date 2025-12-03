use std::io::{stdout, Write};
use std::rc::Rc;
use std::str;

use ahash::RandomState;
use colored::Colorize;
use dlv_list::{Index, VecList};
use indexmap::{map::Entry as IndexMapEntry, IndexMap};
use simd_csv::ByteRecord;
use transient_btree_index::{BtreeConfig, BtreeIndex};
use unicode_width::UnicodeWidthStr;

use crate::collections::{hash_map::Entry, HashMap, HashSet};
use crate::config::{Config, Delimiter};
use crate::moonblade::ChooseProgram;
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Deduplicate the rows of a CSV file. Runs in O(n) time, consuming O(c) memory, c being
the distinct number of row identities.

If your file is already sorted on the deduplication selection, use the -S/--sorted flag
to run in O(1) memory instead.

Note that, by default, this command will write the first row having
a specific identity to the output, unless you use -l/--keep-last.

The command can also write only the duplicated rows with --keep-duplicates.

You are also given the option to add a column indicating whether each row is
a duplicate or not, as per selected method, using -f/--flag <name>. You can
even pipe the result into `xan partition` to split the file into a deduplicated
one and another containing only discarded duplicates:

    $ xan dedup -f duplicated file.csv | xan partition -s duplicated

Finally, it is also possible to specify which rows to keep by evaluating
an expression (see `xan help cheatsheet` and `xan help functions` for
the documentation of the expression language).

For instance, if you want to deduplicate a CSV of events on the `id`
column but want to keep the row having the maximum value in the `count`
column instead of the first row found with any given identity:

    $ xan dedup -s id --choose 'new_count > current_count' events.csv > deduped.csv

Notice how the column names of the currently kept row were prefixed
with \"current_\", while the ones of the new row were prefixed
with \"new_\" instead.

Note that if you need to aggregate cell values from duplicated
rows, you should probably check out `xan groupby` instead, that can
be used for this very purpose, especially with the --keep flag.

Usage:
    xan dedup [options] [<input>]
    xan dedup --help

dedup options:
    --check                Verify whether the selection has any duplicates, i.e. whether
                           the selected columns satisfy a uniqueness constraint.
    -s, --select <arg>     Select a subset of columns to on which to deduplicate.
                           See 'xan select --help' for the format details.
    -S, --sorted           Use if you know your file is already sorted on the deduplication
                           selection to avoid needing to keep a hashmap of values
                           in memory.
    -l, --keep-last        Keep the last row having a specific identity, rather than
                           the first one. Note that it will cost more memory and that
                           no rows will be flushed before the whole file has been read
                           if -S/--sorted is not used.
    -e, --external         Use an external btree index to keep the index on disk and avoid
                           overflowing RAM. Does not work with -l/--keep-last and -k/--keep-duplicates.
    -k, --keep-duplicates  Emit only the duplicated rows.
    -C, --choose <expr>    Evaluate an expression that must return whether to
                           keep a newly seen row or not. Column name in the given
                           expression will be prefixed with \"current_\" for the
                           currently kept row and \"new_\" for the new row to consider.
    -f, --flag <name>      Instead of filtering duplicated rows, add a column with given <name>
                           indicating whether a row is duplicated. File order might get
                           modified to keep proper performance when -l/--keep-last
                           or -C/--choose is used.

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
    flag_select: SelectedColumns,
    flag_check: bool,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_sorted: bool,
    flag_keep_last: bool,
    flag_external: bool,
    flag_keep_duplicates: bool,
    flag_choose: Option<String>,
    flag_flag: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;

    if args.flag_external {
        if args.flag_keep_last {
            Err("-l/--keep-last does not work with -e/--external!")?;
        }

        if args.flag_check {
            Err("--check does not work with -e/--external yet!")?;
        }

        if args.flag_keep_duplicates {
            Err("--keep-duplicates does not work with -e/--external!")?;
        }

        if args.flag_choose.is_some() {
            Err("--choose does not work with -e/--external!")?;
        }
    }

    let mut mutually_exclusive_count: usize = 0;

    if args.flag_keep_last {
        mutually_exclusive_count += 1;
    }
    if args.flag_keep_duplicates {
        mutually_exclusive_count += 1;
    }
    if args.flag_choose.is_some() {
        mutually_exclusive_count += 1;
    }

    if mutually_exclusive_count > 1 {
        Err("must select only one of --choose, -l/--keep-last, --keep-duplicates")?;
    }

    if args.flag_sorted {
        args.flag_external = false;
    }

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);

    let mut rdr = rconf.simd_reader()?;
    let headers = rdr.byte_headers()?.clone();
    let sel = rconf.selection(&headers)?;

    if args.flag_check {
        if args.flag_flag.is_some() {
            Err("-f/--flag does not make sense with --check!")?;
        }

        let mut already_seen = HashSet::<ByteRecord>::new();
        let mut record = ByteRecord::new();

        let mut count: u64 = 0;

        while rdr.read_byte_record(&mut record)? {
            let key = sel.select(&record).collect();

            if !already_seen.insert(key) {
                let max_len_of_head_sel = sel
                    .select(&headers)
                    .map(|h| str::from_utf8(h).unwrap().width())
                    .max()
                    .unwrap();

                let msg = format!(
                    "selection is NOT unique!\nFirst duplicate record found at index {}:\n\n{}",
                    count,
                    sel.select(&headers)
                        .map(|h| {
                            let head_to_print = str::from_utf8(h).unwrap();
                            format!(
                                "{}{}",
                                head_to_print,
                                " ".repeat(max_len_of_head_sel - head_to_print.width())
                            )
                        })
                        .zip(
                            sel.select(&record)
                                .map(|cell| { str::from_utf8(cell).unwrap().red().bold() })
                        )
                        .map(|(h, v)| format!("{} {}", h, v))
                        .collect::<Vec<_>>()
                        .join("\n"),
                );

                Err(msg)?;
            }

            count += 1;
        }

        writeln!(&mut stdout(), "selection is unique!")?;

        return Ok(());
    }

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    if !rconf.no_headers {
        if let Some(column_name) = &args.flag_flag {
            wtr.write_record(headers.iter().chain([column_name.as_bytes()]))?;
        } else {
            wtr.write_byte_record(&headers)?;
        }
    }

    let mut record_writer = RecordWriter {
        mode: if args.flag_flag.is_some() {
            RecordWriterMode::Flag
        } else {
            RecordWriterMode::Filter
        },
        writer: &mut wtr,
        inverted: args.flag_keep_duplicates,
    };

    // External
    if args.flag_external {
        let mut record = ByteRecord::new();

        let mut btree_index = BtreeIndex::<Vec<Vec<u8>>, ()>::with_capacity(
            BtreeConfig::default().fixed_value_size(0),
            1024,
        )?;

        while rdr.read_byte_record(&mut record)? {
            let key = sel.collect(&record);

            if btree_index.insert(key, ())?.is_none() {
                record_writer.emit_record(&record)?;
            } else {
                record_writer.discard_record(&record)?;
            }
        }

        return Ok(wtr.flush()?);
    }

    enum DedupMode {
        KeepFirst,
        KeepLast,
        KeepDuplicates,
        Choose(String),
    }

    let dedup_mode = if args.flag_keep_last {
        DedupMode::KeepLast
    } else if args.flag_keep_duplicates {
        DedupMode::KeepDuplicates
    } else if let Some(expr) = args.flag_choose.take() {
        DedupMode::Choose(expr)
    } else {
        DedupMode::KeepFirst
    };

    match (args.flag_sorted, dedup_mode) {
        // Unsorted, keep first
        (false, DedupMode::KeepFirst) => {
            let mut record = ByteRecord::new();
            let mut already_seen = HashSet::<ByteRecord>::new();

            while rdr.read_byte_record(&mut record)? {
                let key = sel.select(&record).collect();

                if already_seen.insert(key) {
                    record_writer.emit_record(&record)?;
                } else {
                    record_writer.discard_record(&record)?;
                }
            }
        }

        // Unsorted, keep last
        (false, DedupMode::KeepLast) => {
            let mut set = KeepLastSet::new();

            for result in rdr.byte_records() {
                let record = result?;
                let key = sel.select(&record).collect();

                if let Some(discarded_record) = set.push(key, record) {
                    record_writer.discard_record(&discarded_record)?;
                }
            }

            for record in set.into_iter() {
                record_writer.emit_record(&record)?;
            }
        }

        // Sorted, keep first
        (true, DedupMode::KeepFirst) => {
            let mut record = ByteRecord::new();
            let mut current: Option<ByteRecord> = None;

            while rdr.read_byte_record(&mut record)? {
                let key = sel.select(&record).collect();

                match current {
                    None => {
                        record_writer.emit_record(&record)?;
                        current = Some(key);
                    }
                    Some(current_key) if current_key != key => {
                        record_writer.emit_record(&record)?;
                        current = Some(key);
                    }
                    _ => {
                        record_writer.discard_record(&record)?;
                    }
                };
            }
        }

        // Sorted, keep last
        (true, DedupMode::KeepLast) => {
            let mut current: Option<(ByteRecord, ByteRecord)> = None;

            for result in rdr.byte_records() {
                let record = result?;
                let key = sel.select(&record).collect();

                match current {
                    Some((current_key, record_to_flush)) if current_key != key => {
                        record_writer.emit_record(&record_to_flush)?;
                    }
                    Some((_, record_to_flush)) => {
                        record_writer.discard_record(&record_to_flush)?;
                    }
                    _ => {}
                }

                current = Some((key, record));
            }

            if let Some((_, record_to_flush)) = current {
                record_writer.emit_record(&record_to_flush)?;
            }
        }

        // Unsorted, keep duplicates
        (false, DedupMode::KeepDuplicates) => {
            let mut map: HashMap<ByteRecord, Option<(usize, Rc<ByteRecord>)>> = HashMap::new();
            let mut records: Vec<(bool, Rc<ByteRecord>)> = Vec::new();
            let mut record = ByteRecord::new();
            let mut index: usize = 0;

            while rdr.read_byte_record(&mut record)? {
                let key = sel.select(&record).collect();
                let record_rc = Rc::new(record.clone());

                match map.entry(key) {
                    Entry::Occupied(mut entry) => {
                        if let Some((ind, _)) = entry.get_mut().take() {
                            records[ind].0 = true;
                        }
                        records.push((true, record_rc.clone()));
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(Some((index, record_rc.clone())));
                        records.push((false, record_rc.clone()));
                    }
                }
                index += 1;
            }

            for (keep, record) in records.into_iter() {
                if keep {
                    record_writer.emit_record(&record)?;
                } else {
                    record_writer.discard_record(&record)?;
                }
            }
        }

        // Sorted, keep duplicates
        (true, DedupMode::KeepDuplicates) => {
            let mut record = ByteRecord::new();

            struct PreviousEntry {
                key: ByteRecord,
                record: ByteRecord,
                already_emitted: bool,
            }

            let mut previous_entry_opt: Option<PreviousEntry> = None;

            while rdr.read_byte_record(&mut record)? {
                let key = sel.select(&record).collect();

                match previous_entry_opt.as_mut() {
                    None => {
                        previous_entry_opt = Some(PreviousEntry {
                            key,
                            record: record.clone(),
                            already_emitted: false,
                        })
                    }
                    Some(previous_entry) => {
                        if previous_entry.key == key {
                            if !previous_entry.already_emitted {
                                record_writer.emit_record(&previous_entry.record)?;
                                previous_entry.already_emitted = true;
                            }

                            record_writer.emit_record(&record)?;
                        } else {
                            if !previous_entry.already_emitted {
                                record_writer.discard_record(&previous_entry.record)?;
                            }

                            previous_entry_opt = Some(PreviousEntry {
                                key,
                                record: record.clone(),
                                already_emitted: false,
                            })
                        }
                    }
                };
            }

            if let Some(previous_entry) = previous_entry_opt {
                if !previous_entry.already_emitted {
                    record_writer.discard_record(&previous_entry.record)?;
                }
            }
        }

        // Unsorted choose
        (false, DedupMode::Choose(expr)) => {
            let mut map: IndexMap<ByteRecord, ByteRecord, RandomState> =
                IndexMap::with_hasher(RandomState::new());
            let mut program = ChooseProgram::parse(&expr, &headers)?;
            let mut record = ByteRecord::new();
            let mut index: usize = 0;

            while rdr.read_byte_record(&mut record)? {
                match map.entry(sel.select(&record).collect()) {
                    IndexMapEntry::Vacant(entry) => {
                        entry.insert(record.clone());
                    }
                    IndexMapEntry::Occupied(mut entry) => {
                        program.prepare_current_record(entry.get());

                        if program.run_with_record(index, &record)? {
                            record_writer.discard_record(entry.get())?;
                            record.clone_into(entry.get_mut());
                        } else {
                            record_writer.discard_record(&record)?;
                        }
                    }
                }

                index += 1;
            }

            for output_record in map.into_values() {
                record_writer.emit_record(&output_record)?;
            }
        }

        // Sorted choose
        (true, DedupMode::Choose(expr)) => {
            let mut current_opt: Option<(ByteRecord, ByteRecord)> = None;
            let mut program = ChooseProgram::parse(&expr, &headers)?;
            let mut record = ByteRecord::new();
            let mut index: usize = 0;

            while rdr.read_byte_record(&mut record)? {
                let key = sel.select(&record).collect();

                match current_opt.as_mut() {
                    None => {
                        program.prepare_current_record(&record);
                        current_opt = Some((key, record.clone()));
                    }
                    Some((current_key, current_record)) => {
                        if &key == current_key {
                            if program.run_with_record(index, &record)? {
                                record_writer.discard_record(current_record)?;
                                // Swap
                                record.clone_into(current_record);
                                program.prepare_current_record(current_record);
                            } else {
                                record_writer.discard_record(&record)?;
                            }
                        } else {
                            // Flush
                            record_writer.emit_record(current_record)?;
                            program.prepare_current_record(&record);
                            *current_key = key;
                            record.clone_into(current_record);
                        }
                    }
                }

                index += 1;
            }

            // Flush
            if let Some((_, current_record)) = current_opt {
                record_writer.emit_record(&current_record)?;
            }
        }
    }

    Ok(wtr.flush()?)
}

enum RecordWriterMode {
    Filter,
    Flag,
}

impl RecordWriterMode {
    fn is_flag(&self) -> bool {
        matches!(self, Self::Flag)
    }
}

struct RecordWriter<'w, W: Write> {
    mode: RecordWriterMode,
    writer: &'w mut simd_csv::Writer<W>,
    inverted: bool,
}

impl<W: Write> RecordWriter<'_, W> {
    #[inline(always)]
    fn label(&self, mut emit: bool) -> &'static [u8] {
        if self.inverted {
            emit = !emit;
        }

        if emit {
            b"false"
        } else {
            b"true"
        }
    }

    fn emit_record(&mut self, record: &ByteRecord) -> CliResult<()> {
        use RecordWriterMode::*;

        match self.mode {
            Filter => {
                self.writer.write_byte_record(record)?;
            }
            Flag => {
                self.writer
                    .write_record(record.iter().chain([self.label(true)]))?;
            }
        };

        Ok(())
    }

    fn discard_record(&mut self, record: &ByteRecord) -> CliResult<()> {
        if self.mode.is_flag() {
            self.writer
                .write_record(record.iter().chain([self.label(false)]))?;
        }

        Ok(())
    }
}

struct KeepLastSet {
    map: HashMap<ByteRecord, Index<ByteRecord>>,
    list: VecList<ByteRecord>,
}

impl KeepLastSet {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            list: VecList::new(),
        }
    }

    #[must_use]
    fn push(&mut self, key: ByteRecord, record: ByteRecord) -> Option<ByteRecord> {
        match self.map.entry(key) {
            Entry::Occupied(mut entry) => {
                let current_index = entry.get_mut();

                let removed = self.list.remove(*current_index);
                *current_index = self.list.push_back(record);
                removed
            }
            Entry::Vacant(entry) => {
                entry.insert(self.list.push_back(record));
                None
            }
        }
    }

    fn into_iter(self) -> impl Iterator<Item = ByteRecord> {
        self.list.into_iter()
    }
}
