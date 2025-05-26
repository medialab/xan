use std::borrow::Cow;
use std::env;
use std::fs::File;
use std::io::{self, IsTerminal};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use bstr::ByteSlice;
use colored::{ColoredString, Colorize};
use flate2::{write::GzEncoder, Compression};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::{prelude::*, ThreadPoolBuilder};

use crate::cmd::progress::get_progress_style;
use crate::collections::Counter;
use crate::config::{Config, Delimiter};
use crate::moonblade::{AggregationProgram, GroupAggregationProgram, Stats};
use crate::read::{read_byte_record_up_to, segment_csv_file, SegmentationOptions};
use crate::select::SelectColumns;
use crate::util::{self, FilenameTemplate};
use crate::CliResult;

fn get_spinner_style(path: ColoredString, unspecified: bool) -> ProgressStyle {
    ProgressStyle::with_template(
        &(if unspecified {
            format!(
                "{{spinner}} {{decimal_bytes:>11}} of {} in {{elapsed}} ({{decimal_bytes_per_sec}})",
                path
            )
        } else {
            format!(
                "{{spinner}} {{human_pos:>11}} rows of {} in {{elapsed}} ({{per_sec}})",
                path
            )
        }),
    )
    .unwrap()
    .tick_chars("⠁⠁⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈⣿")
}

struct Bars {
    main: ProgressBar,
    multi: MultiProgress,
    bars: Mutex<Vec<(String, ProgressBar)>>,
    total: u64,
    chunked: bool,
    unspecified: bool,
}

impl Bars {
    fn new(total: usize, threads: usize, chunked: bool, unspecified: bool) -> Self {
        let main = ProgressBar::new(total as u64);

        let multi = MultiProgress::new();
        multi.add(main.clone());

        main.set_prefix(format!("(t={}) ", threads));

        let bars = Bars {
            main,
            multi,
            bars: Mutex::new(Vec::new()),
            total: total as u64,
            chunked,
            unspecified,
        };

        bars.set_color("blue");

        bars
    }

    fn set_color(&self, color: &str) {
        self.main.set_style(get_progress_style(
            Some(self.total),
            color,
            false,
            if self.chunked { "chunks" } else { "files" },
        ));
        self.main.tick();
    }

    fn start(&self, name: &str) -> ProgressBar {
        let bar = ProgressBar::new_spinner();
        bar.set_style(get_spinner_style(name.cyan(), self.unspecified));

        self.bars.lock().unwrap().push((
            name.to_string(),
            self.multi.insert_before(&self.main, bar.clone()),
        ));

        self.main.tick();

        bar
    }

    fn stop(&self, name: &str) {
        self.bars.lock().unwrap().retain_mut(|(p, b)| {
            if p != name {
                true
            } else {
                b.set_style(get_spinner_style(p.green(), self.unspecified));
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

    // fn interrupt(&self) {
    //     for (path, bar) in self.bars.lock().unwrap().iter() {
    //         bar.set_style(get_spinner_style(path.yellow()));
    //         bar.tick();
    //         bar.abandon();
    //     }

    //     self.set_color("yellow");
    //     self.main.abandon();
    // }
}

struct ParallelProgressBar {
    bars: Option<Bars>,
}

impl ParallelProgressBar {
    fn hidden() -> Self {
        Self { bars: None }
    }

    fn new(total: usize, threads: usize, chunked: bool, unspecified: bool) -> Self {
        Self {
            bars: Some(Bars::new(total, threads, chunked, unspecified)),
        }
    }

    fn start(&self, path: &str) -> Option<ProgressBar> {
        self.bars.as_ref().map(|bars| bars.start(path))
    }

    fn stop(&self, name: &str) {
        if let Some(bars) = &self.bars {
            bars.stop(name);
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

type TotalAndItems = (u64, Vec<(Vec<u8>, u64)>);

struct FrequencyTables {
    counters: Vec<(Vec<u8>, Counter<Vec<u8>>)>,
}

impl FrequencyTables {
    fn new() -> Self {
        Self {
            counters: Vec::new(),
        }
    }

    fn with_capacity(selected_headers: Vec<Vec<u8>>, approx_capacity: Option<usize>) -> Self {
        let mut freq_counters = Self {
            counters: Vec::with_capacity(selected_headers.len()),
        };

        for header in selected_headers {
            freq_counters
                .counters
                .push((header, Counter::new(approx_capacity)));
        }

        freq_counters
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut Counter<Vec<u8>>> {
        self.counters.iter_mut().map(|(_, t)| t)
    }

    fn merge(&mut self, other: Self) -> Result<(), &str> {
        // First time merge
        if self.counters.is_empty() {
            self.counters = other.counters;
            return Ok(());
        }

        let error_msg = "inconsistent column selection across files!";

        if self.counters.len() != other.counters.len() {
            return Err(error_msg);
        }

        for ((_, self_counter), (_, other_counter)) in
            self.counters.iter_mut().zip(other.counters.into_iter())
        {
            self_counter.merge(other_counter);
        }

        Ok(())
    }

    fn into_total_and_items(
        self,
        limit: Option<usize>,
    ) -> impl Iterator<Item = (Vec<u8>, TotalAndItems)> {
        self.counters
            .into_iter()
            .map(move |(name, counter)| (name, counter.into_total_and_items(limit, true)))
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
        // First time merge
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

#[derive(Debug)]
struct FileChunk {
    file_path: String,
    from: u64,
    to: u64,
    position: usize,
    headers: csv::ByteRecord,
}

impl FileChunk {
    fn name(&self) -> String {
        format!("{}@chunk-{}", self.file_path, self.position)
    }
}

#[derive(Debug)]
enum Input {
    Path(String),
    FileChunk(FileChunk),
}

impl Input {
    fn name(&self) -> Cow<str> {
        match self {
            Self::Path(p) => Cow::Borrowed(p),
            Self::FileChunk(chunk) => Cow::Owned(chunk.name()),
        }
    }

    fn path(&self) -> &str {
        match self {
            Self::Path(p) => p,
            Self::FileChunk(chunk) => &chunk.file_path,
        }
    }

    fn headers(&self) -> Option<csv::ByteRecord> {
        match self {
            Self::FileChunk(chunk) => Some(chunk.headers.clone()),
            _ => None,
        }
    }
}

struct InputReader {
    config: Config,
    reader: Box<dyn io::Read + Send>,
    headers: Option<csv::ByteRecord>,
    _children: Option<Children>,
    up_to: Option<u64>,
    bar: Option<ProgressBar>,
}

impl InputReader {
    fn into_csv_reader(self) -> CliResult<CsvInputReader> {
        let mut csv_reader = self.config.csv_reader_from_reader(self.reader);
        let headers = match self.headers {
            None => csv_reader.byte_headers()?.clone(),
            Some(h) => h,
        };

        Ok(CsvInputReader {
            csv_reader,
            headers,
            _children: self._children,
            up_to: self.up_to,
            bar: self.bar,
        })
    }
}

struct CsvInputReader {
    csv_reader: BoxedReader,
    headers: csv::ByteRecord,
    _children: Option<Children>,
    up_to: Option<u64>,
    bar: Option<ProgressBar>,
}

impl CsvInputReader {
    fn read_byte_record(&mut self, record: &mut csv::ByteRecord) -> Result<bool, csv::Error> {
        read_byte_record_up_to(&mut self.csv_reader, record, self.up_to)
    }

    fn tick(&self) {
        if let Some(bar) = &self.bar {
            bar.inc(1);
        }
    }
}

static USAGE: &str = "
Parallel processing of CSV data.

This command can either process a single CSV file by splitting it into one
chunk per working thread, or a dataset comprised of multiple files on disk.

When processing a single file, this command cannot work with streams (stdin)
nor gzipped data, unless it was compressed with `bgzip -i` and a `.gzi` index
file can be found beside the file.

To process a single CSV file in parallel, use the -F/--single-file flag:

    $ xan parallel count -F docs.csv

To process multiple files at once, you must give their paths as multiple
arguments to the command or give them through stdin with one path
per line or in a CSV column when using the --path-column flag:

    Multiple arguments through shell glob:
    $ xan parallel count data/**/docs.csv

    One path per line, fed through stdin:
    $ ls data/**/docs.csv | xan parallel count

    Paths from a CSV column through stdin:
    $ cat filelist.csv | xan parallel count --path-column path

Note that you can use the `split` or `partition` command to preemptively
split a large file into manageable chunks, if you can spare the disk space.

This command has multiple subcommands that each perform some typical
parallel reduce operation:

    - `count`: counts the number of rows in the whole dataset.
    - `cat`: preprocess the files and redirect the concatenated
        rows to your output (e.g. searching all the files in parallel and
        retrieving the results).
    - `freq`: builds frequency tables in parallel. See \"xan freq -h\" for
        an example of output.
    - `stats`: computes well-known statistics in parallel. See \"xan stats -h\" for
        an example of output.
    - `agg`: parallelize a custom aggregation. See \"xan agg -h\" for more details.
    - `groupby`: parallelize a custom grouped aggregation. See \"xan groupby -h\"
        for more details.
    - `map`: writes the result of given preprocessing in a new
        file besides the original one. This subcommand takes a filename template
        where `{}` will be replaced by the name of each target file without any
        extension (`.csv` or `.csv.gz` would be stripped for instance), or by
        a chunk id, when using -F/--single-file.

For instance, the following command:

    $ xan parallel map '{}_freq.csv' -P 'freq -s Category' *.csv

Will create a file suffixed \"_freq.csv\" for each CSV file in current directory
containing its frequency table for the \"Category\" command.

Note also that the `xan parallel map` subcommand, used with the -F/--single-file
flag, is basically a parallel split.

Finally, preprocessing on each file can be done using two different methods:

1. Using only xan subcommands with -P, --preprocess:
    $ xan parallel count -P \"search -s name John | slice -l 10\" file.csv

2. Using a shell subcommand passed to \"$SHELL -c\" with -H, --shell-preprocess:
    $ xan parallel count -H \"xan search -s name John | xan slice -l 10\" file.csv

The second preprocessing option will of course not work in DOS-based shells and Powershell
on Windows.

Usage:
    xan parallel count [options] [<inputs>...]
    xan parallel cat [options] [<inputs>...]
    xan parallel freq [options] [<inputs>...]
    xan parallel stats [options] [<inputs>...]
    xan parallel agg [options] <expr> [<inputs>...]
    xan parallel groupby [options] <group> <expr> [<inputs>...]
    xan parallel map <template> [options] [<inputs>...]
    xan parallel --help
    xan p count [options] [<inputs>...]
    xan p cat [options] [<inputs>...]
    xan p freq [options] [<inputs>...]
    xan p stats [options] [<inputs>...]
    xan p agg [options] <expr> [<inputs>...]
    xan p groupby [options] <group> <expr> [<inputs>...]
    xan p map <template> [options] [<inputs>...]
    xan p --help

parallel options:
    -F, --single-file            Parallelize computation over a single uncompressed
                                 CSV file on disk instead of processing multiple
                                 files in parallel.
    -P, --preprocess <op>        Preprocessing, only able to use xan subcommands.
    -H, --shell-preprocess <op>  Preprocessing commands that will run directly in your
                                 own shell using the -c flag. Will not work on windows.
    --progress                   Display a progress bar for the parallel tasks. The
                                 per file/chunk bars will tick once per CSV row only
                                 AFTER pre-processing!
    -t, --threads <n>            Number of threads to use. Will default to a sensible
                                 number based on the available CPUs.
    --path-column <name>         Name of the path column if stdin is given as a CSV file
                                 instead of one path per line.

parallel count options:
    -S, --source-column <name>  If given, will return a CSV file containing a column with
                                the source file being counted and a column with the count itself.

parallel cat options:
    -B, --buffer-size <n>       Number of rows a thread is allowed to keep in memory
                                before flushing to the output. Set <= 0 to flush only once per
                                processed file. Keep in mind this could cost a lot of memory.
                                [default: 1024]
    -S, --source-column <name>  Name of a column to prepend in the output of indicating the
                                path to source file.

parallel freq options:
    -s, --select <cols>  Columns for which to build frequency tables.
    --sep <char>         Split the cell into multiple values to count using the
                         provided separator.
    -A, --all            Remove the limit.
    -l, --limit <arg>    Limit the frequency table to the N most common
                         items. Use -A, -all or set to 0 to disable the limit.
                         [default: 10]
    -a, --approx         If set, return the items most likely having the top counts,
                         as per given --limit. Won't work if --limit is 0 or
                         with -A, --all. Accuracy of results increases with the given
                         limit.
    -N, --no-extra       Don't include empty cells & remaining counts.

parallel stats options:
    -s, --select <cols>    Columns for which to build statistics.
    -A, --all              Shorthand for -cq.
    -c, --cardinality      Show cardinality and modes.
                           This requires storing all CSV data in memory.
    -q, --quartiles        Show quartiles.
                           This requires storing all CSV data in memory.
    -a, --approx           Show approximated statistics.
    --nulls                Include empty values in the population size for computing
                           mean and standard deviation.

parallel map options:
    -z, --compress  Use this flag to gzip the processed files.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Note that this has no effect when
                           concatenating columns.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize, Default)]
pub struct Args {
    pub cmd_count: bool,
    cmd_cat: bool,
    pub cmd_freq: bool,
    pub cmd_stats: bool,
    pub cmd_agg: bool,
    pub cmd_groupby: bool,
    cmd_map: bool,
    arg_inputs: Vec<String>,
    pub arg_expr: Option<String>,
    pub arg_group: Option<SelectColumns>,
    arg_template: Option<FilenameTemplate>,
    flag_single_file: bool,
    flag_preprocess: Option<String>,
    flag_shell_preprocess: Option<String>,
    flag_progress: bool,
    flag_threads: Option<NonZeroUsize>,
    flag_path_column: Option<SelectColumns>,
    flag_buffer_size: isize,
    flag_source_column: Option<String>,
    pub flag_select: SelectColumns,
    pub flag_sep: Option<String>,
    pub flag_limit: usize,
    pub flag_no_extra: bool,
    pub flag_all: bool,
    pub flag_cardinality: bool,
    pub flag_quartiles: bool,
    pub flag_approx: bool,
    pub flag_nulls: bool,
    flag_compress: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

type BoxedReader = csv::Reader<Box<dyn io::Read + Send>>;

impl Args {
    pub fn single_file(path: &Option<String>, threads: Option<NonZeroUsize>) -> CliResult<Self> {
        match path {
            Some(p) => Ok(Self {
                flag_threads: threads,
                flag_single_file: true,
                flag_buffer_size: 1024,
                flag_limit: 10,
                arg_inputs: vec![p.to_string()],
                ..Default::default()
            }),
            None => Err("cannot parallelize over stdin! You must provide a file path.")?,
        }
    }

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

        if self.flag_approx {
            stats.compute_approx();
        }

        stats
    }

    fn inputs(&self) -> CliResult<(Vec<Input>, Option<usize>)> {
        let inputs = if !self.arg_inputs.is_empty() {
            self.arg_inputs.clone()
        } else if io::stdin().is_terminal() {
            vec![]
        } else {
            Config::stdin()
                .lines(&self.flag_path_column)?
                .collect::<Result<Vec<_>, _>>()?
        };

        if inputs.is_empty() {
            Err("no files to process!\nDid you forget stdin or arguments?")?;
        }

        if self.flag_single_file {
            if inputs.len() > 1 {
                Err("-F/--single-file can only work with a single path as input!")?;
            }

            let target = inputs.first().unwrap();
            let config = Config::new(&Some(target.clone()))
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            if target.ends_with(".gz") && !config.is_indexed_gzip() {
                Err("cannot parallelize over single gzipped file without .gzi index!")?;
            }

            let mut reader = config.seekable_reader()?;
            let headers = reader.byte_headers()?.clone();

            match segment_csv_file(
                &mut reader,
                SegmentationOptions::chunks(
                    self.flag_threads
                        .map(|t| t.get())
                        .unwrap_or_else(num_cpus::get),
                ),
            )? {
                None => Err("could not segment file correctly!")?,
                Some(segments) => {
                    let chunks_hint = Some(segments.len());

                    return Ok((
                        segments
                            .into_iter()
                            .enumerate()
                            .map(|(i, (from, to))| {
                                Input::FileChunk(FileChunk {
                                    file_path: target.clone(),
                                    from,
                                    to,
                                    position: i,
                                    headers: headers.clone(),
                                })
                            })
                            .collect(),
                        chunks_hint,
                    ));
                }
            }
        }

        Ok((inputs.into_iter().map(Input::Path).collect(), None))
    }

    fn io_reader(
        &self,
        input: &Input,
        progress_bar: &ParallelProgressBar,
    ) -> CliResult<InputReader> {
        let bar = progress_bar.start(&input.name());

        // Shell preprocessing
        if let Some(preprocessing) = &self.flag_shell_preprocess {
            if preprocessing.trim().is_empty() {
                Err("-H, --shell-preprocess cannot be an empty command!")?;
            }

            let config = Config::stdin()
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            let shell = env::var("SHELL").expect("$SHELL is not set!");

            // NOTE: here we are relying on cat to avoid spinning a thread.
            // I haven't benchmarked this but I suspect `cat` does a better job
            // than what I could do regarding buffering etc. It's true we could
            // save up a process though, so we might want to benchmark this.
            let mut cat = match input {
                Input::Path(p) => Command::new("cat")
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .arg(p)
                    .spawn()
                    .expect("could not spawn \"cat\""),
                Input::FileChunk(file_chunk) => Command::new(env::current_exe()?)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .arg("slice")
                    .arg("--byte-offset")
                    .arg(file_chunk.from.to_string())
                    .arg("--end-byte")
                    .arg(file_chunk.to.to_string())
                    .arg(&file_chunk.file_path)
                    .spawn()
                    .expect("could not spawn \"xan slice\""),
            };

            let mut child = Command::new(shell)
                .stdin(cat.stdout.take().expect("could not consume cat stdout"))
                .stdout(Stdio::piped())
                .args(["-c", preprocessing])
                .spawn()
                .expect("could not spawn shell preprocessing");

            let reader = Box::new(child.stdout.take().expect("cannot read child stdout"));

            // NOTE: this must happen before reading headers to ensure correct drop
            let _children = Some(Children::from(vec![cat, child]));

            Ok(InputReader {
                config,
                reader,
                headers: input.headers(),
                _children,
                up_to: None,
                bar,
            })
        }
        // Standard preprocessing
        else if let Some(preprocessing) = &self.flag_preprocess {
            if preprocessing.trim().is_empty() {
                Err("-P, --preprocess cannot be an empty command!")?;
            }

            let exe = env::current_exe()?;

            let raw_preprocessing = shlex::split(preprocessing).ok_or_else(|| {
                format!("could not parse shell expression: {}", preprocessing.cyan())
            })?;

            let mut preprocessing = Vec::with_capacity(raw_preprocessing.len());

            // NOTE: renormalizing tokens around pipes (e.g. when given a pipe
            // that is not separated by a space `progress |search -es Category`).
            for token in raw_preprocessing.into_iter() {
                if token == "|" {
                    preprocessing.push(token);
                } else if let Some(rest) = token.strip_prefix("|") {
                    preprocessing.push("|".to_string());
                    preprocessing.push(rest.trim().to_string());
                } else if let Some(rest) = token.strip_suffix("|") {
                    preprocessing.push(rest.trim().to_string());
                    preprocessing.push("|".to_string());
                } else {
                    preprocessing.push(token);
                }
            }

            let mut children: Vec<Child> = Vec::new();

            if let Input::FileChunk(file_chunk) = input {
                children.push(
                    Command::new(exe.clone())
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .arg("slice")
                        .arg("--byte-offset")
                        .arg(file_chunk.from.to_string())
                        .arg("--end-byte")
                        .arg(file_chunk.to.to_string())
                        .arg(&file_chunk.file_path)
                        .spawn()
                        .expect("could not spawn \"xan slice\""),
                );
            }

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
                    command.arg(input.path());
                }

                children.push(command.spawn().expect("could not spawn preprocessing"));
            }

            let config = Config::stdin()
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            Ok(InputReader {
                config,
                reader: Box::new(
                    children
                        .last_mut()
                        .unwrap()
                        .stdout
                        .take()
                        .expect("cannot read child stdout"),
                ),
                headers: input.headers(),
                _children: Some(Children::from(children)),
                up_to: None,
                bar,
            })
        }
        // No preprocessing
        else {
            match input {
                Input::Path(p) => {
                    let config = Config::new(&Some(p.to_string()))
                        .delimiter(self.flag_delimiter)
                        .no_headers(self.flag_no_headers);

                    let reader = config.io_reader()?;

                    Ok(InputReader {
                        config,
                        reader,
                        headers: None,
                        _children: None,
                        up_to: None,
                        bar,
                    })
                }
                Input::FileChunk(file_chunk) => {
                    let config = Config::new(&Some(file_chunk.file_path.to_string()))
                        .delimiter(self.flag_delimiter)
                        .no_headers(true);

                    let reader = config.io_reader_at_position(file_chunk.from)?;

                    Ok(InputReader {
                        config,
                        reader,
                        headers: Some(file_chunk.headers.clone()),
                        _children: None,
                        up_to: Some(file_chunk.to - file_chunk.from),
                        bar,
                    })
                }
            }
        }
    }

    fn reader(
        &self,
        input: &Input,
        progress_bar: &ParallelProgressBar,
    ) -> CliResult<CsvInputReader> {
        self.io_reader(input, progress_bar)
            .and_then(InputReader::into_csv_reader)
    }

    fn progress_bar(&self, total: usize) -> ParallelProgressBar {
        if self.flag_progress {
            console::set_colors_enabled(true);
            colored::control::set_override(true);

            ParallelProgressBar::new(
                total,
                self.flag_threads
                    .expect("at that point, threads cannot be None")
                    .get(),
                self.flag_single_file,
                self.cmd_map,
            )
        } else {
            ParallelProgressBar::hidden()
        }
    }

    fn count(self, inputs: Vec<Input>) -> CliResult<()> {
        let progress_bar = self.progress_bar(inputs.len());

        if let Some(source_column_name) = &self.flag_source_column {
            let writer_mutex = {
                let mut writer = Config::new(&self.flag_output).writer()?;

                let mut output_headers = csv::ByteRecord::new();
                output_headers.push_field(source_column_name.as_bytes());
                output_headers.push_field(b"count");

                writer.write_byte_record(&output_headers)?;

                Mutex::new(writer)
            };

            inputs.par_iter().try_for_each(|input| -> CliResult<()> {
                let mut input_reader = self.reader(input, &progress_bar)?;
                let name = input.name();

                let mut record = csv::ByteRecord::new();
                let mut count: usize = 0;

                while input_reader.read_byte_record(&mut record)? {
                    count += 1;

                    input_reader.tick();
                }

                let mut output_record = csv::ByteRecord::new();
                output_record.push_field(name.as_bytes());
                output_record.push_field(count.to_string().as_bytes());

                writer_mutex
                    .lock()
                    .unwrap()
                    .write_byte_record(&output_record)?;

                progress_bar.stop(&input.name());

                Ok(())
            })?;

            progress_bar.succeed();

            writer_mutex.into_inner().unwrap().flush()?;
        } else {
            let total_count = AtomicUsize::new(0);

            inputs.par_iter().try_for_each(|input| -> CliResult<()> {
                let mut input_reader = self.reader(input, &progress_bar)?;

                let mut record = csv::ByteRecord::new();
                let mut count: usize = 0;

                while input_reader.read_byte_record(&mut record)? {
                    count += 1;

                    input_reader.tick();
                }

                total_count.fetch_add(count, Ordering::Relaxed);

                progress_bar.stop(&input.name());

                Ok(())
            })?;

            progress_bar.succeed();

            println!("{}", total_count.into_inner());
        }

        Ok(())
    }

    fn cat(self, inputs: Vec<Input>) -> CliResult<()> {
        if self.flag_preprocess.is_none() && self.flag_shell_preprocess.is_none() {
            Err("`xan parallel cat` without -P/--preprocess or -H/--shell-preprocess is counterproductive!\n`xan cat rows` will be faster.")?
        }

        let progress_bar = self.progress_bar(inputs.len());

        // NOTE: the bool tracks whether headers were already written
        let writer_mutex = Arc::new(Mutex::new((
            false,
            Config::new(&self.flag_output).writer()?,
        )));

        let buffer_size_opt = if self.flag_buffer_size <= 0 {
            None
        } else {
            Some(self.flag_buffer_size as usize)
        };

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

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.reader(input, &progress_bar)?;
            let name = input.name();

            if let Some(source_column) = &self.flag_source_column {
                input_reader.headers.push_field(source_column.as_bytes());
            }

            let mut buffer: Vec<csv::ByteRecord> = if let Some(buffer_size) = buffer_size_opt {
                Vec::with_capacity(buffer_size)
            } else {
                Vec::new()
            };

            let mut record = csv::ByteRecord::new();

            while input_reader.read_byte_record(&mut record)? {
                if matches!(buffer_size_opt, Some(buffer_size) if buffer.len() == buffer_size) {
                    flush(&input_reader.headers, &buffer)?;

                    buffer.clear();
                }

                if self.flag_source_column.is_some() {
                    record.push_field(name.as_bytes());
                }

                buffer.push(record.clone());

                input_reader.tick();
            }

            if !buffer.is_empty() {
                flush(&input_reader.headers, &buffer)?;
            }

            progress_bar.stop(&input.name());

            Ok(())
        })?;

        progress_bar.succeed();

        Arc::into_inner(writer_mutex)
            .unwrap()
            .into_inner()
            .unwrap()
            .1
            .flush()?;

        Ok(())
    }

    fn freq(mut self, inputs: Vec<Input>) -> CliResult<()> {
        if self.flag_all {
            self.flag_limit = 0;
        }

        if self.flag_approx && self.flag_limit == 0 {
            Err("-a, --approx cannot work with --limit=0 or -A, --all!")?;
        }

        let approx_capacity = self.flag_approx.then_some(self.flag_limit);

        let progress_bar = self.progress_bar(inputs.len());

        let total_freq_tables_mutex = Arc::new(Mutex::new(FrequencyTables::new()));

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.reader(input, &progress_bar)?;

            let sel = self.flag_select.selection(&input_reader.headers, true)?;

            let mut freq_tables =
                FrequencyTables::with_capacity(sel.collect(&input_reader.headers), approx_capacity);

            let mut record = csv::ByteRecord::new();

            while input_reader.read_byte_record(&mut record)? {
                for (counter, cell) in freq_tables.iter_mut().zip(sel.select(&record)) {
                    if let Some(sep) = &self.flag_sep {
                        for subcell in cell.split_str(sep) {
                            counter.add(subcell.to_vec());
                        }
                    } else {
                        counter.add(cell.to_vec());
                    }
                }

                input_reader.tick();
            }

            total_freq_tables_mutex.lock().unwrap().merge(freq_tables)?;

            progress_bar.stop(&input.name());

            Ok(())
        })?;

        let mut writer = Config::new(&self.flag_output).writer()?;

        let mut output_record = csv::ByteRecord::new();
        output_record.extend([b"field", b"value", b"count"]);

        writer.write_byte_record(&output_record)?;

        let total_freq_tables = Arc::into_inner(total_freq_tables_mutex)
            .unwrap()
            .into_inner()
            .unwrap();

        for (field, (total, items)) in
            total_freq_tables.into_total_and_items(if self.flag_limit == 0 {
                None
            } else {
                Some(self.flag_limit)
            })
        {
            let mut emitted: u64 = 0;

            for (value, count) in items {
                emitted += count;

                output_record.clear();
                output_record.push_field(&field);
                output_record.push_field(&value);
                output_record.push_field(count.to_string().as_bytes());

                writer.write_byte_record(&output_record)?;
            }

            let remaining = total - emitted;

            if !self.flag_no_extra && remaining > 0 {
                output_record.clear();
                output_record.push_field(&field);
                output_record.push_field(b"<rest>");
                output_record.push_field(remaining.to_string().as_bytes());

                writer.write_byte_record(&output_record)?;
            }
        }

        progress_bar.succeed();
        writer.flush()?;

        Ok(())
    }

    fn stats(self, inputs: Vec<Input>) -> CliResult<()> {
        let progress_bar = self.progress_bar(inputs.len());

        let mut writer = Config::new(&self.flag_output).writer()?;
        writer.write_byte_record(&self.new_stats().headers())?;

        let total_stats_mutex = Mutex::new(StatsTables::new());

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.reader(input, &progress_bar)?;

            let sel = self.flag_select.selection(&input_reader.headers, true)?;

            let mut local_stats =
                StatsTables::with_capacity(sel.collect(&input_reader.headers), || self.new_stats());
            let mut record = csv::ByteRecord::new();

            while input_reader.read_byte_record(&mut record)? {
                for (cell, stats) in sel.select(&record).zip(local_stats.iter_mut()) {
                    stats.process(cell);
                }

                input_reader.tick();
            }

            total_stats_mutex.lock().unwrap().merge(local_stats)?;

            progress_bar.stop(&input.name());

            Ok(())
        })?;

        for (name, stats) in total_stats_mutex.into_inner().unwrap().into_iter() {
            writer.write_byte_record(&stats.results(&name))?;
        }

        progress_bar.succeed();
        writer.flush()?;

        Ok(())
    }

    fn agg(self, inputs: Vec<Input>) -> CliResult<()> {
        let progress_bar = self.progress_bar(inputs.len());

        let total_program_mutex: Mutex<Option<AggregationProgram>> = Mutex::new(None);

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.reader(input, &progress_bar)?;

            let mut record = csv::ByteRecord::new();
            let mut program =
                AggregationProgram::parse(self.arg_expr.as_ref().unwrap(), &input_reader.headers)?;

            let mut index: usize = 0;

            while input_reader.read_byte_record(&mut record)? {
                program.run_with_record(index, &record)?;
                index += 1;

                input_reader.tick();
            }

            let mut total_program_opt = total_program_mutex.lock().unwrap();

            match total_program_opt.as_mut() {
                Some(current_program) => current_program.merge(program),
                None => *total_program_opt = Some(program),
            };

            progress_bar.stop(&input.name());

            Ok(())
        })?;

        if let Some(mut total_program) = total_program_mutex.into_inner().unwrap() {
            let mut writer = Config::new(&self.flag_output).writer()?;
            writer.write_record(total_program.headers())?;
            writer.write_byte_record(&total_program.finalize(true)?)?;
        }

        progress_bar.succeed();

        Ok(())
    }

    fn groupby(self, inputs: Vec<Input>) -> CliResult<()> {
        let progress_bar = self.progress_bar(inputs.len());

        let total_program_mutex: Mutex<Option<(Vec<Vec<u8>>, GroupAggregationProgram)>> =
            Mutex::new(None);

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.reader(input, &progress_bar)?;

            let sel = self
                .arg_group
                .clone()
                .unwrap()
                .selection(&input_reader.headers, true)?;

            let mut record = csv::ByteRecord::new();
            let mut program = GroupAggregationProgram::parse(
                self.arg_expr.as_ref().unwrap(),
                &input_reader.headers,
            )?;

            let mut index: usize = 0;

            while input_reader.read_byte_record(&mut record)? {
                let group = sel.collect(&record);

                program.run_with_record(group, index, &record)?;
                index += 1;

                input_reader.tick();
            }

            let mut total_program_opt = total_program_mutex.lock().unwrap();

            match total_program_opt.as_mut() {
                Some((_, current_program)) => current_program.merge(program),
                None => *total_program_opt = Some((sel.collect(&input_reader.headers), program)),
            };

            progress_bar.stop(&input.name());

            Ok(())
        })?;

        if let Some((group_headers, total_program)) = total_program_mutex.into_inner().unwrap() {
            let mut writer = Config::new(&self.flag_output).writer()?;
            let mut output_record = csv::ByteRecord::new();
            output_record.extend(group_headers);
            output_record.extend(total_program.headers());

            writer.write_record(&output_record)?;

            for result in total_program.into_byte_records(true) {
                let (group, values) = result?;

                output_record.clear();
                output_record.extend(group);
                output_record.extend(values.into_iter());

                writer.write_byte_record(&output_record)?;
            }
        }

        progress_bar.succeed();

        Ok(())
    }

    fn map(mut self, inputs: Vec<Input>) -> CliResult<()> {
        if !self.flag_single_file
            && self.flag_preprocess.is_none()
            && self.flag_shell_preprocess.is_none()
        {
            Err("`xan parallel map` without -F/--single-file and -P/--preprocess or -H/--shell-preprocess is pointless ;).")?;
        }

        // NOTE: this work only for CSV output, which has to be the case with -F/--single-file
        // If we don't do this, the output chunk do not have proper headers, and the chunking
        // is not done properly.
        if self.flag_single_file
            && self.flag_preprocess.is_none()
            && self.flag_shell_preprocess.is_none()
        {
            self.flag_preprocess = Some("slice".to_string());
        }

        let progress_bar = self.progress_bar(inputs.len());
        let template = self.arg_template.clone().unwrap();

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.io_reader(input, &progress_bar)?;

            let err = || format!("Could not extract file base from path {}", input.path());

            let absolute_path = Path::new(input.path()).canonicalize().map_err(|_| err())?;

            let file_id = match input {
                Input::Path(_) => Cow::Borrowed(
                    absolute_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .and_then(|name| name.split(".").next())
                        .ok_or_else(err)?,
                ),
                Input::FileChunk(file_chunk) => Cow::Owned(format!("{:0>4}", file_chunk.position)),
            };

            let file_name = template.filename(&file_id);
            let mut file_path = PathBuf::from(absolute_path.parent().ok_or_else(err)?);
            file_path.push(file_name);

            let output_file = File::create(file_path)?;

            let mut output: Box<dyn io::Write> = if self.flag_compress {
                Box::new(GzEncoder::new(output_file, Compression::default()))
            } else {
                Box::new(output_file)
            };

            if let Some(bar) = &input_reader.bar {
                io::copy(&mut bar.wrap_read(&mut input_reader.reader), &mut output)?;
            } else {
                io::copy(&mut input_reader.reader, &mut output)?;
            }

            progress_bar.stop(&input.name());

            Ok(())
        })?;

        progress_bar.succeed();

        Ok(())
    }

    pub fn run(mut self) -> CliResult<()> {
        let (inputs, chunks_hint) = self.inputs()?;

        if inputs.len() == 1 && !self.flag_single_file {
            eprintln!(
                "{}",
                "warning: processing a single file. Did you forget -F/--single-file!".yellow()
            );
        }

        if self.flag_threads.is_none() {
            self.flag_threads = Some(NonZeroUsize::new(num_cpus::get()).unwrap());
        }

        let mut threads = self.flag_threads.unwrap().get();

        match chunks_hint {
            Some(t) => {
                threads = threads.min(t);
            }
            None => {
                threads = threads.min(inputs.len());
            }
        };

        ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .expect("could not build thread pool!");

        self.flag_threads = Some(NonZeroUsize::new(threads).unwrap());

        if self.cmd_count {
            self.count(inputs)?;
        } else if self.cmd_cat {
            self.cat(inputs)?;
        } else if self.cmd_freq {
            self.freq(inputs)?;
        } else if self.cmd_stats {
            self.stats(inputs)?;
        } else if self.cmd_agg {
            self.agg(inputs)?;
        } else if self.cmd_groupby {
            self.groupby(inputs)?;
        } else if self.cmd_map {
            self.map(inputs)?;
        } else {
            unreachable!()
        }

        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    args.run()
}
