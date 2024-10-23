use std::collections::HashMap;
use std::env;
use std::io;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use bstr::ByteSlice;
use colored::{ColoredString, Colorize};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::{prelude::*, ThreadPoolBuilder};

use crate::cmd::progress::get_progress_style;
use crate::config::{Config, Delimiter};
use crate::moonblade::Stats;
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

// TODO: groupby, agg

fn get_spinner_style(path: ColoredString) -> ProgressStyle {
    ProgressStyle::with_template(&format!(
        "{{spinner}} {{human_pos:>11}} rows of {} in {{elapsed}} ({{per_sec}})",
        path
    ))
    .unwrap()
    .tick_chars("⠁⠁⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈⣿")
}

struct Bars {
    main: ProgressBar,
    multi: MultiProgress,
    bars: Mutex<Vec<(String, ProgressBar)>>,
    total: u64,
}

impl Bars {
    fn new(total: usize) -> Self {
        let main = ProgressBar::new(total as u64);
        let multi = MultiProgress::new();
        multi.add(main.clone());

        let bars = Bars {
            main,
            multi,
            bars: Mutex::new(Vec::new()),
            total: total as u64,
        };

        bars.set_color("blue");

        bars
    }

    fn set_color(&self, color: &str) {
        self.main
            .set_style(get_progress_style(Some(self.total), color, false, "files"));
        self.main.tick();
    }

    fn start(&self, path: &str) -> ProgressBar {
        let bar = ProgressBar::new_spinner();
        bar.set_style(get_spinner_style(path.cyan()));

        self.bars.lock().unwrap().push((
            path.to_string(),
            self.multi.insert_before(&self.main, bar.clone()),
        ));

        bar
    }

    fn stop(&self, path: &str) {
        self.bars.lock().unwrap().retain_mut(|(p, b)| {
            if p != path {
                true
            } else {
                b.set_style(get_spinner_style(path.green()));
                b.abandon();
                false
            }
        });
        self.main.inc(1);
    }

    fn abandon(&self) {
        for (_, bar) in self.bars.lock().unwrap().iter() {
            bar.abandon();
        }

        self.main.abandon();
    }

    fn succeed(&self) {
        self.set_color("green");
        self.main.tick();
        self.abandon();
    }

    fn interrupt(&self) {
        for (path, bar) in self.bars.lock().unwrap().iter() {
            bar.set_style(get_spinner_style(path.yellow()));
            bar.tick();
            bar.abandon();
        }

        self.set_color("yellow");
        self.main.abandon();
    }
}

struct ParallelProgressBar {
    bars: Option<Arc<Bars>>,
}

impl ParallelProgressBar {
    fn hidden() -> Self {
        Self { bars: None }
    }

    fn new(total: usize) -> Self {
        let bars = Arc::new(Bars::new(total));

        let handle = bars.clone();

        ctrlc::set_handler(move || {
            handle.interrupt();
            std::process::exit(1);
        })
        .expect("Could not setup ctrl+c handler!");

        Self { bars: Some(bars) }
    }

    fn start(&self, path: &str) -> Option<ProgressBar> {
        self.bars.as_ref().map(|bars| bars.start(path))
    }

    fn tick(bar_opt: &Option<ProgressBar>) {
        if let Some(bar) = bar_opt {
            bar.inc(1);
        }
    }

    fn stop(&self, path: &str) {
        if let Some(bars) = &self.bars {
            bars.stop(path);
        }
    }

    fn succeed(&self) {
        if let Some(bars) = &self.bars {
            bars.succeed();
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

impl From<Vec<Child>> for Children {
    fn from(children: Vec<Child>) -> Self {
        Self { children }
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

struct StatsTables {
    tables: Vec<(Vec<u8>, Stats)>,
}

impl StatsTables {
    fn new() -> Self {
        Self { tables: Vec::new() }
    }

    fn with_capacity<F>(selected_headers: Vec<Vec<u8>>, new: F) -> Self
    where
        F: Fn() -> Stats,
    {
        let mut stats_tables = Self {
            tables: Vec::with_capacity(selected_headers.len()),
        };

        for header in selected_headers {
            stats_tables.tables.push((header, new()));
        }

        stats_tables
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut Stats> {
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

            current_table.merge(table);
        }

        Ok(())
    }

    fn into_iter(self) -> impl Iterator<Item = (Vec<u8>, Stats)> {
        self.tables.into_iter()
    }
}

static USAGE: &str = "
Process CSV datasets split into multiple files, in parallel.

The CSV files composing said dataset can be given as multiple arguments to the
command, or given through stdin, one path per line or in a CSV column when
using --path-column:

    Multiple arguments through shell glob:
    $ xan parallel count data/**/docs.csv

    One path per line, fed through stdin:
    $ ls data/**/docs.csv | xan parallel count

    Paths from a CSV column through stdin:
    $ xan glob 'data/**/docs.csv' | xan parallel count --path-column path

Note that you can use the `split` or `partition` command to preemptively
split a large file into manageable chunks, if you can spare the disk space.

This command has multiple subcommands that each perform some typical
parallel reduce operation:

    - `count`: counts the number of rows in the whole dataset.
    - `cat`: preprocess the files and redirect the concatenated
        rows to your output (e.g. searching all the files in parallel and
        retrieving the results).
    - `freq`: builds frequency tables in parallel.
    - `stats`: computes well-known statistics in parallel.

Finally, preprocessing on each file can be done using two different methods:

1. Using only xan subcommands with -P, --preprocess:
    $ xan parallel count -P \"search -s name John | slice -l 10\" file.csv

2. Using a shell subcommand passed to \"$SHELL -c\" with -S, --shell-preprocess:
    $ xan parallel count -S \"xan search -s name John | xan slice -l 10\" file.csv

The second preprocessing option will of course not work in DOS-based shells and Powershell
on Windows.

Usage:
    xan parallel count [options] [<inputs>...]
    xan parallel cat [options] [<inputs>...]
    xan parallel freq [options] [<inputs>...]
    xan parallel stats [options] [<inputs>...]
    xan p count [options] [<inputs>...]
    xan p cat [options] [<inputs>...]
    xan p freq [options] [<inputs>...]
    xan p stats [options] [<inputs>...]
    xan parallel --help

parallel options:
    -P, --preprocess <op>        Preprocessing, only able to use xan subcommands.
    -S, --shell-preprocess <op>  Preprocessing commands that will run directly in your
                                 own shell using the -c flag. Will not work on windows.
    --progress                   Display a progress bar for the parallel tasks.
    -t, --threads <n>            Number of threads to use. Will default to a sensible
                                 number based on the available CPUs.
    --path-column <name>         Name of the path column if stdin is given as a CSV file
                                 instead of one path per line.

parallel cat options:
    -B, --buffer-size <n>       Number of rows a thread is allowed to keep in memory
                                before flushing to the output.
                                [default: 1024]
    -I, --input-dir <dir>       When concatenating rows, root directory to resolve
                                relative paths contained in the -i/--input file column.
    -S, --source-column <name>  Name of a column to prepend in the output of indicating the
                                path to source file.

parallel freq options:
    -s, --select <cols>  Columns for which to build frequency tables.
    --sep <char>         Split the cell into multiple values to count using the
                         provided separator.

parallel stats options:
    -s, --select <cols>  Columns for which to build statistics.
    -A, --all            Show all statistics available.
    -c, --cardinality    Show cardinality and modes.
                         This requires storing all CSV data in memory.
    -q, --quartiles      Show quartiles.
                         This requires storing all CSV data in memory.
    --nulls              Include empty values in the population size for computing
                         mean and standard deviation.

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
    cmd_stats: bool,
    flag_preprocess: Option<String>,
    flag_shell_preprocess: Option<String>,
    flag_progress: bool,
    flag_threads: Option<NonZeroUsize>,
    flag_path_column: Option<SelectColumns>,
    flag_buffer_size: NonZeroUsize,
    flag_input_dir: Option<PathBuf>,
    flag_source_column: Option<String>,
    flag_select: SelectColumns,
    flag_sep: Option<String>,
    flag_all: bool,
    flag_cardinality: bool,
    flag_quartiles: bool,
    flag_nulls: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

type Reader = csv::Reader<Box<dyn io::Read + Send>>;

impl Args {
    fn new_stats(&self) -> Stats {
        let mut stats = Stats::new();

        if self.flag_nulls {
            stats.include_nulls();
        }

        if self.flag_all || self.flag_cardinality {
            stats.compute_frequencies();
        }

        if self.flag_all || self.flag_quartiles {
            stats.compute_numbers();
        }

        stats
    }

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
        Ok(if let Some(preprocessing) = &self.flag_shell_preprocess {
            if preprocessing.trim().is_empty() {
                Err("-S, --shell-preprocess cannot be an empty command!")?;
            }

            let config = Config::new(&None)
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            let shell = env::var("SHELL").expect("$SHELL is not set!");

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
                .expect("could not spawn shell preprocessing");

            (
                config.csv_reader_from_reader(Box::new(
                    child.stdout.take().expect("cannot read child stdout"),
                )),
                Some(Children::pair(cat, child)),
            )
        } else if let Some(preprocessing) = &self.flag_preprocess {
            if preprocessing.trim().is_empty() {
                Err("-P, --preprocess cannot be an empty command!")?;
            }

            let exe = env::current_exe()?;

            let preprocessing = shlex::split(preprocessing).expect("could not shlex");

            let mut children: Vec<Child> = Vec::new();

            for mut step in preprocessing.split(|token| token == "|") {
                let mut command = Command::new(exe.clone());
                command.stdout(Stdio::piped());

                if let Some(first) = step.first() {
                    if first == "xan" {
                        step = &step[1..];
                    }
                }

                for arg in step {
                    command.arg(arg);
                }

                if let Some(last_child) = children.last_mut() {
                    // Piping last command into the next
                    command.stdin(
                        last_child
                            .stdout
                            .take()
                            .expect("could not consume last child stdout"),
                    );
                } else {
                    // First command in pipeline must read the file
                    command.stdin(Stdio::null());
                    command.arg(path);
                }

                children.push(command.spawn().expect("could not spawn preprocessing"));
            }

            let config = Config::new(&None)
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            (
                config.csv_reader_from_reader(Box::new(
                    children
                        .last_mut()
                        .unwrap()
                        .stdout
                        .take()
                        .expect("cannot read child stdout"),
                )),
                Some(Children::from(children)),
            )
        } else {
            let config = Config::new(&Some(path.to_string()))
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            (config.reader()?, None)
        })
    }

    fn try_for_each_path<F>(&self, callback: F) -> CliResult<()>
    where
        F: Fn(&String) -> CliResult<()> + Send + Sync,
    {
        self.inputs()?.par_iter().try_for_each(callback)
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.cmd_cat && args.flag_preprocess.is_none() && args.flag_shell_preprocess.is_none() {
        Err("`xan parallel cat` without -P/--preprocess or -S/--shell-preprocess is counterproductive!\n`xan cat rows` will be faster.")?
    }

    if let Some(threads) = args.flag_threads {
        ThreadPoolBuilder::new()
            .num_threads(threads.get())
            .build_global()
            .expect("could not build thread pool!");
    }

    let inputs_count = args.arg_inputs.len();
    let progress_bar = if args.flag_progress {
        console::set_colors_enabled(true);
        colored::control::set_override(true);

        ParallelProgressBar::new(inputs_count)
    } else {
        ParallelProgressBar::hidden()
    };

    // Count
    if args.cmd_count {
        let total_count = AtomicUsize::new(0);

        args.try_for_each_path(|path| {
            let (mut reader, _children_guard) = args.reader(path)?;

            let bar = progress_bar.start(path);

            let mut record = csv::ByteRecord::new();
            let mut count: usize = 0;

            while reader.read_byte_record(&mut record)? {
                count += 1;

                ParallelProgressBar::tick(&bar);
            }

            total_count.fetch_add(count, Ordering::Relaxed);
            progress_bar.stop(path);

            Ok(())
        })?;

        progress_bar.succeed();

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

        args.try_for_each_path(|path| {
            let (mut reader, _children_guard) = args.reader(path)?;

            let bar = progress_bar.start(path);

            let mut headers = reader.byte_headers()?.clone();

            if let Some(source_column) = &args.flag_source_column {
                headers.push_field(source_column.as_bytes());
            }

            let mut buffer: Vec<csv::ByteRecord> = Vec::with_capacity(buffer_size);

            for result in reader.byte_records() {
                if buffer.len() == buffer_size {
                    flush(&headers, &buffer)?;

                    buffer.clear();
                }

                let mut record = result?;

                if args.flag_source_column.is_some() {
                    if let Some(root_dir) = &args.flag_input_dir {
                        let mut buf = root_dir.clone();
                        buf.push(path);
                        record.push_field(buf.to_string_lossy().as_bytes());
                    } else {
                        record.push_field(path.as_bytes());
                    }
                }

                buffer.push(record);

                ParallelProgressBar::tick(&bar);
            }

            if !buffer.is_empty() {
                flush(&headers, &buffer)?;
            }

            progress_bar.stop(path);

            Ok(())
        })?;

        progress_bar.succeed();

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

        args.try_for_each_path(|path| {
            let (mut reader, _children_guard) = args.reader(path)?;

            let bar = progress_bar.start(path);

            let headers = reader.byte_headers()?.clone();
            let sel = Config::new(&None)
                .select(args.flag_select.clone())
                .selection(&headers)?;

            let mut freq_tables = FrequencyTables::with_capacity(sel.collect(&headers));

            let mut record = csv::ByteRecord::new();

            while reader.read_byte_record(&mut record)? {
                for (table, cell) in freq_tables.iter_mut().zip(sel.select(&record)) {
                    if let Some(sep) = &args.flag_sep {
                        for subcell in cell.split_str(sep) {
                            table.inc(subcell.to_vec());
                        }
                    } else {
                        table.inc(cell.to_vec());
                    }
                }

                ParallelProgressBar::tick(&bar);
            }

            total_freq_tables_mutex.lock().unwrap().merge(freq_tables)?;

            progress_bar.stop(path);

            Ok(())
        })?;

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

        progress_bar.succeed();
        writer.flush()?;
    }
    // Stats
    else if args.cmd_stats {
        let mut writer = Config::new(&args.flag_output).writer()?;
        writer.write_byte_record(&args.new_stats().headers())?;

        let total_stats = Mutex::new(StatsTables::new());

        args.try_for_each_path(|path| {
            let (mut reader, _children_guard) = args.reader(path)?;

            let bar = progress_bar.start(path);

            let headers = reader.byte_headers()?.clone();
            let sel = Config::new(&None)
                .select(args.flag_select.clone())
                .selection(&headers)?;

            let mut local_stats =
                StatsTables::with_capacity(sel.collect(&headers), || args.new_stats());
            let mut record = csv::ByteRecord::new();

            while reader.read_byte_record(&mut record)? {
                for (cell, stats) in sel.select(&record).zip(local_stats.iter_mut()) {
                    stats.process(cell);
                }

                ParallelProgressBar::tick(&bar);
            }

            total_stats.lock().unwrap().merge(local_stats)?;

            progress_bar.stop(path);

            Ok(())
        })?;

        progress_bar.succeed();

        for (name, stats) in total_stats.into_inner().unwrap().into_iter() {
            writer.write_byte_record(&stats.results(&name))?;
        }

        writer.flush()?;
    }

    Ok(())
}
