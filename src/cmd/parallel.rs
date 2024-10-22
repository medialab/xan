use std::collections::HashMap;
use std::io;
use std::num::NonZeroUsize;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use indicatif::ProgressBar;
use rayon::{prelude::*, ThreadPoolBuilder};

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

// TODO: finish progress bar
// TODO: cat -S/--source-column
// TODO: examples in the help
// TODO: document in main
// TODO: can we chunk a single file?
// TODO: raw preprocessing
// TODO: freq --sep
// TODO: groupby, agg, stats
// TODO: unit tests

struct ParallelProgressBar {
    bar: Option<ProgressBar>,
}

impl ParallelProgressBar {
    fn hidden() -> Self {
        Self { bar: None }
    }

    fn new(total: usize) -> Self {
        Self {
            bar: Some(ProgressBar::new(total as u64)),
        }
    }

    fn abandon(&self) {
        if let Some(bar) = &self.bar {
            bar.abandon();
        }
    }

    fn tick(&self) {
        if let Some(bar) = &self.bar {
            bar.inc(1);
        }
    }
}

struct Children {
    children: Vec<Child>,
}

impl Children {
    fn pair(one: Child, two: Child) -> Self {
        Self {
            children: vec![one, two],
        }
    }

    fn wait(&mut self) -> io::Result<()> {
        for child in self.children.iter_mut() {
            child.wait()?;
        }

        Ok(())
    }

    fn kill(&mut self) -> io::Result<()> {
        for child in self.children.iter_mut() {
            child.kill()?;
        }

        Ok(())
    }
}

impl Drop for Children {
    fn drop(&mut self) {
        if std::thread::panicking() {
            let _ = self.kill();
        } else {
            let _ = self.wait();
        }
    }
}

#[derive(Default)]
struct FrequencyTable {
    map: HashMap<Vec<u8>, u64>,
}

impl FrequencyTable {
    fn inc(&mut self, key: Vec<u8>) {
        self.add(key, 1);
    }

    fn add(&mut self, key: Vec<u8>, count: u64) {
        self.map
            .entry(key)
            .and_modify(|current_count| *current_count += count)
            .or_insert(count);
    }
}

struct FrequencyTables {
    tables: Vec<(Vec<u8>, FrequencyTable)>,
}

impl FrequencyTables {
    fn new() -> Self {
        Self { tables: Vec::new() }
    }

    fn with_capacity(selected_headers: Vec<Vec<u8>>) -> Self {
        let mut freq_tables = Self {
            tables: Vec::with_capacity(selected_headers.len()),
        };

        for header in selected_headers {
            freq_tables.tables.push((header, FrequencyTable::default()));
        }

        freq_tables
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut FrequencyTable> {
        self.tables.iter_mut().map(|(_, t)| t)
    }

    fn merge(&mut self, other: Self) -> Result<(), &str> {
        if self.tables.is_empty() {
            self.tables = other.tables;
            return Ok(());
        }

        let error_msg = "inconsistent column selection across files!";

        if self.tables.len() != other.tables.len() {
            return Err(error_msg);
        }

        for (i, (name, table)) in other.tables.into_iter().enumerate() {
            let (current_name, current_table) = &mut self.tables[i];

            if current_name != &name {
                return Err(error_msg);
            }

            for (key, count) in table.map {
                current_table.add(key, count);
            }
        }

        Ok(())
    }

    fn into_sorted(self) -> impl Iterator<Item = (Vec<u8>, Vec<(Vec<u8>, u64)>)> {
        self.tables.into_iter().map(|(name, table)| {
            let mut items: Vec<_> = table.map.into_iter().collect();
            items.par_sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0)));

            (name, items)
        })
    }
}

static USAGE: &str = "
Process CSV datasets split into multiple files, in parallel.

The CSV files composing said dataset can be given as variadic arguments to the
command, or given through stdin, one path per line or in a CSV column when
using --path-column.

`xan parallel count` counts the number of rows in the whole dataset.

`xan parallel cat` preprocess the files and redirect the concatenated
rows to your output (e.g. searching all the files in parallel and
retrieving the results).

`xan parallel freq` build frequency tables in parallel.

Note that you can use the `split` or `partition` command to preemptively
split a large file into manageable chunks, if you can spare the disk space.

Usage:
    xan parallel count [options] [<inputs>...]
    xan parallel cat [options] [<inputs>...]
    xan parallel freq [options] [<inputs>...]
    xan p count [options] [<inputs>...]
    xan p cat [options] [<inputs>...]
    xan p freq [options] [<inputs>...]
    xan parallel --help

parallel options:
    -P, --preprocess <op>  Preprocessing command that will run on every
                           file to process.
    --progress             Display a progress bar for the parallel tasks.
    -t, --threads <n>      Number of threads to use. Will default to a sensible
                           number based on the available CPUs.
    --path-column <name>   Name of the path column if stdin is given as a CSV file
                           instead of one path per line.

parallel cat options:
    -B, --buffer-size <n>  Number of rows a thread is allowed to keep in memory
                           before flushing to the output.
                           [default: 1024]

parallel freq options:
    -s, --select <cols>  Columns for which to build frequency tables.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Note that this has no effect when
                           concatenating columns.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_inputs: Vec<String>,
    cmd_count: bool,
    cmd_cat: bool,
    cmd_freq: bool,
    flag_preprocess: Option<String>,
    flag_progress: bool,
    flag_threads: Option<NonZeroUsize>,
    flag_path_column: Option<SelectColumns>,
    flag_buffer_size: NonZeroUsize,
    flag_select: SelectColumns,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

type Reader = csv::Reader<Box<dyn io::Read + Send>>;

impl Args {
    fn inputs(&self) -> CliResult<Vec<String>> {
        if !self.arg_inputs.is_empty() {
            Ok(self.arg_inputs.clone())
        } else if let Some(col_name) = &self.flag_path_column {
            let config = Config::new(&None).select(col_name.clone());
            let mut reader = config.reader()?;
            let headers = reader.byte_headers()?;
            let path_column_index = config.single_selection(headers)?;

            let mut paths = Vec::new();
            let mut record = csv::ByteRecord::new();

            while reader.read_byte_record(&mut record)? {
                let path = String::from_utf8(record[path_column_index].to_vec())
                    .expect("could not decode path column as utf8");

                paths.push(path);
            }

            Ok(paths)
        } else {
            Ok(io::stdin()
                .lines()
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter_map(|line| {
                    let line = line.trim();

                    if !line.is_empty() {
                        Some(line.to_string())
                    } else {
                        None
                    }
                })
                .collect())
        }
    }

    fn reader(&self, path: &str) -> CliResult<(Reader, Option<Children>)> {
        Ok(if let Some(preprocessing) = &self.flag_preprocess {
            let config = Config::new(&None)
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            let shell = std::env::var("SHELL").expect("$SHELL is not set!");

            let mut cat = Command::new("cat")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .arg(path)
                .spawn()
                .expect("could not spawn \"cat\"");

            let mut child = Command::new(shell)
                .stdin(cat.stdout.take().expect("could not consume cat stdout"))
                .stdout(Stdio::piped())
                .args(["-c", preprocessing])
                .spawn()
                .expect("could not spawn preprocessing");

            (
                config.csv_reader_from_reader(Box::new(
                    child.stdout.take().expect("cannot read child stdout"),
                )),
                Some(Children::pair(cat, child)),
            )
        } else {
            let config = Config::new(&Some(path.to_string()))
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            (config.reader()?, None)
        })
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.cmd_cat && args.flag_preprocess.is_none() {
        Err("`xan parallel cat` without -p/--preprocess is counterproductive!\n`xan cat rows` will be faster.")?
    }

    if let Some(threads) = args.flag_threads {
        ThreadPoolBuilder::new()
            .num_threads(threads.get())
            .build_global()
            .expect("could not build thread pool!");
    }

    let inputs_count = args.arg_inputs.len();
    let progress_bar = if args.flag_progress {
        ParallelProgressBar::new(inputs_count)
    } else {
        ParallelProgressBar::hidden()
    };

    // Count
    if args.cmd_count {
        let total_count = AtomicUsize::new(0);

        args.inputs()?
            .par_iter()
            .try_for_each(|path| -> CliResult<()> {
                let (mut reader, _children_guard) = args.reader(path)?;

                let mut record = csv::ByteRecord::new();
                let mut count: usize = 0;

                while reader.read_byte_record(&mut record)? {
                    count += 1;
                }

                total_count.fetch_add(count, Ordering::Relaxed);
                progress_bar.tick();

                Ok(())
            })?;

        progress_bar.abandon();

        println!("{}", total_count.into_inner());
    }
    // Cat
    else if args.cmd_cat {
        // NOTE: the bool tracks whether headers were already written
        let writer_mutex = Arc::new(Mutex::new((
            false,
            Config::new(&args.flag_output).writer()?,
        )));
        let buffer_size = args.flag_buffer_size.get();

        let flush = |headers: &csv::ByteRecord, records: &[csv::ByteRecord]| -> CliResult<()> {
            let mut guard = writer_mutex.lock().unwrap();

            if !guard.0 {
                guard.1.write_byte_record(headers)?;
                guard.0 = true;
            }

            for record in records.iter() {
                guard.1.write_byte_record(record)?;
            }

            Ok(())
        };

        args.inputs()?
            .par_iter()
            .try_for_each(|path| -> CliResult<()> {
                let (mut reader, _children_guard) = args.reader(path)?;
                let headers = reader.byte_headers()?.clone();

                let mut buffer: Vec<csv::ByteRecord> = Vec::with_capacity(buffer_size);

                for result in reader.byte_records() {
                    if buffer.len() == buffer_size {
                        flush(&headers, &buffer)?;

                        buffer.clear();
                    }

                    buffer.push(result?);
                }

                if !buffer.is_empty() {
                    flush(&headers, &buffer)?;
                }

                progress_bar.tick();

                Ok(())
            })?;

        progress_bar.abandon();

        Arc::into_inner(writer_mutex)
            .unwrap()
            .into_inner()
            .unwrap()
            .1
            .flush()?;
    }
    // Freq
    else if args.cmd_freq {
        let total_freq_tables_mutex = Arc::new(Mutex::new(FrequencyTables::new()));

        args.inputs()?
            .par_iter()
            .try_for_each(|path| -> CliResult<()> {
                let (mut reader, _children_guard) = args.reader(path)?;

                let headers = reader.byte_headers()?.clone();
                let sel = Config::new(&None)
                    .select(args.flag_select.clone())
                    .selection(&headers)?;

                let mut freq_tables = FrequencyTables::with_capacity(sel.collect(&headers));

                let mut record = csv::ByteRecord::new();

                while reader.read_byte_record(&mut record)? {
                    for (table, cell) in freq_tables.iter_mut().zip(sel.select(&record)) {
                        table.inc(cell.to_vec());
                    }
                }

                total_freq_tables_mutex.lock().unwrap().merge(freq_tables)?;

                progress_bar.tick();

                Ok(())
            })?;

        progress_bar.abandon();

        let mut writer = Config::new(&args.flag_output).writer()?;

        let mut output_record = csv::ByteRecord::new();
        output_record.extend([b"field", b"value", b"count"]);

        writer.write_byte_record(&output_record)?;

        let total_freq_tables = Arc::into_inner(total_freq_tables_mutex)
            .unwrap()
            .into_inner()
            .unwrap();

        for (field, items) in total_freq_tables.into_sorted() {
            for (value, count) in items {
                output_record.clear();
                output_record.push_field(&field);
                output_record.push_field(&value);
                output_record.push_field(count.to_string().as_bytes());

                writer.write_byte_record(&output_record)?;
            }
        }

        writer.flush()?;
    }

    Ok(())
}
