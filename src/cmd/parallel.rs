use std::borrow::Cow;
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, stderr, stdout, IsTerminal, Write};
use std::iter::once;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use bstr::ByteSlice;
use colored::{ColoredString, Colorize};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::{prelude::*, ThreadPoolBuilder};
use simd_csv::ByteRecord;

use crate::cmd::progress::get_progress_style;
use crate::cmd::top::Value as TopValue;
use crate::collections::{Counter, DynamicOrd, TopKHeapMapWithTies};
use crate::config::{Compression, Config, Delimiter};
use crate::moonblade::{AggregationProgram, GroupAggregationProgram, Stats};
use crate::processing::{parse_pipeline, Children};
use crate::select::SelectedColumns;
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

#[derive(Clone)]
struct Bars {
    main: ProgressBar,
    multi: MultiProgress,
    bars: Arc<Mutex<Vec<(String, ProgressBar)>>>,
    total: u64,
    unspecified: bool,
}

impl Bars {
    fn new(total: usize, threads: usize, unspecified: bool) -> Self {
        let main = ProgressBar::new(total as u64);

        let multi = MultiProgress::new();
        multi.add(main.clone());

        main.set_prefix(format!("(t={}) ", threads));

        let bars = Bars {
            main,
            multi,
            bars: Arc::new(Mutex::new(Vec::new())),
            total: total as u64,
            unspecified,
        };

        bars.set_color("blue");

        bars.main.enable_steady_tick(Duration::from_millis(200));

        bars
    }

    fn set_color(&self, color: &str) {
        self.main.set_style(get_progress_style(
            Some(self.total),
            color,
            false,
            "chunks/files",
        ));
    }

    fn start(&self, name: &str) -> ProgressBar {
        let bar = ProgressBar::new_spinner();
        bar.set_style(get_spinner_style(name.cyan(), self.unspecified));

        self.bars.lock().unwrap().push((
            name.to_string(),
            self.multi.insert_before(&self.main, bar.clone()),
        ));

        // NOTE: bar must be inserted into the multibar before first
        // tick, or weirdness will ensue.
        bar.enable_steady_tick(Duration::from_millis(200));

        bar
    }

    fn stop(&self, name: &str, errored: bool) {
        self.bars.lock().unwrap().retain_mut(|(p, b)| {
            if p != name {
                true
            } else {
                b.set_style(get_spinner_style(
                    if errored { p.red() } else { p.green() },
                    self.unspecified,
                ));
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

    fn abort(&self) {
        self.set_color("red");
        self.main.tick();
        self.abandon();
    }
}

struct OptionalProgressBar(Option<ProgressBar>);

impl OptionalProgressBar {
    #[inline(always)]
    fn inc(&self, delta: u64) {
        if let Some(bar) = &self.0 {
            bar.inc(delta);
        }
    }

    #[inline(always)]
    fn tick(&self) {
        self.inc(1);
    }
}

// TODO: we could manage a pool with an index handle instead of a BTreeMap
struct ProcessManager {
    bars: Option<Bars>,
    children_map: Arc<Mutex<BTreeMap<String, Children>>>,
}

impl ProcessManager {
    fn new() -> Self {
        Self {
            bars: None,
            children_map: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    fn with_progress_bar(total: usize, threads: usize, unspecified: bool) -> Self {
        let mut manager = Self::new();
        manager.bars = Some(Bars::new(total, threads, unspecified));

        manager
    }

    fn spawn_checker_thread(&self) {
        let children_map_handle = self.children_map.clone();
        let bars_handle = self.bars.clone();

        thread::spawn(move || loop {
            let mut children_map = children_map_handle.lock().unwrap();
            let must_abort = check_running_processes(&mut children_map, &bars_handle);

            if must_abort {
                std::process::exit(1);
            }

            std::mem::drop(children_map);

            thread::sleep(Duration::from_millis(500));
        });
    }

    fn start(&self, name: &str, children_opt: Option<Children>) -> OptionalProgressBar {
        if let Some(children) = children_opt {
            self.children_map
                .lock()
                .unwrap()
                .insert(name.to_string(), children);
        }

        OptionalProgressBar(self.bars.as_ref().map(|bars| bars.start(name)))
    }

    fn stop(&self, name: &str) {
        let mut children_map = self.children_map.lock().unwrap();

        let must_abort = if let Some(mut dropped_children) = children_map.remove(name) {
            check_running_process(name, &mut dropped_children, &self.bars)
        } else {
            false
        };

        if must_abort {
            for children in children_map.values_mut() {
                children.kill().unwrap();
            }
            children_map.clear();

            std::process::exit(1);
        }

        if let Some(bars) = &self.bars {
            bars.stop(name, false);
        }
    }

    fn succeed(self) {
        assert!(self.children_map.lock().unwrap().is_empty());

        if let Some(bars) = &self.bars {
            bars.succeed();
        }
    }
}

fn check_running_process(name: &str, children: &mut Children, bars_handle: &Option<Bars>) -> bool {
    children.check(|stderr_contents| {
        if let Some(bars) = bars_handle {
            bars.stop(name, true);
            bars.abort();
        }

        let stderr_msg = stderr_contents.trim();

        if stderr_msg.is_empty() {
            eprintln!(
                "Processing failed for {} without captured stderr (stderr was already closed).",
                name.cyan()
            );
        } else {
            eprintln!(
                "Processing failed for {} with captured stderr:\n{}",
                name.cyan(),
                stderr_msg.red()
            );
        }
    })
}

fn check_running_processes(
    children_map: &mut BTreeMap<String, Children>,
    bars_handle: &Option<Bars>,
) -> bool {
    let mut must_abort = false;

    for (name, children) in children_map.iter_mut() {
        must_abort = check_running_process(name, children, bars_handle);
    }

    if must_abort {
        for children in children_map.values_mut() {
            children.kill().unwrap();
        }
        children_map.clear();
    }

    must_abort
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        check_running_processes(&mut self.children_map.lock().unwrap(), &self.bars);
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

    fn with_capacity(selected_headers: ByteRecord, approx_capacity: Option<usize>) -> Self {
        let mut freq_counters = Self {
            counters: Vec::with_capacity(selected_headers.len()),
        };

        for header in selected_headers.into_iter() {
            freq_counters
                .counters
                .push((header.to_vec(), Counter::new(approx_capacity)));
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

    fn with_capacity<F>(selected_headers: ByteRecord, new: F) -> Self
    where
        F: Fn() -> Stats,
    {
        let mut stats_tables = Self {
            tables: Vec::with_capacity(selected_headers.len()),
        };

        for header in selected_headers.into_iter() {
            stats_tables.tables.push((header.to_vec(), new()));
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
    headers: ByteRecord,
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
    fn name(&self) -> Cow<'_, str> {
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
}

struct InputReader {
    config: Config,
    reader: Option<Box<dyn io::Read + Send>>,
    headers: Option<ByteRecord>,
    children: Option<Children>,
}

impl InputReader {
    fn take_children(&mut self) -> Option<Children> {
        self.children.take()
    }

    fn take(&mut self) -> Box<dyn io::Read + Send> {
        self.reader.take().unwrap()
    }

    fn take_simd_csv_reader(&mut self) -> BoxedReader {
        let io_reader = self.take();
        self.config.simd_csv_reader_from_reader(io_reader)
    }

    fn take_simd_csv_splitter(&mut self) -> BoxedSplitter {
        let io_reader = self.take();
        self.config.simd_csv_splitter_from_reader(io_reader)
    }

    fn headers(&self, fallback: &ByteRecord) -> ByteRecord {
        if let Some(h) = &self.headers {
            h.clone()
        } else {
            fallback.clone()
        }
    }
}

static USAGE: &str = "
Parallel processing of CSV data.

This command usually parallelizes computation over multiple files, but is also
able to automatically chunk CSV files and bgzipped CSV files (when a `.gzi` index
can be found) when the number of available threads is greater than the number
of files to read.

This means this command is quite capable of parallelizing over a single CSV file.

To process a single CSV file in parallel:

    $ xan parallel count docs.csv

To process multiple files at once, you must give their paths as multiple
arguments to the command or give them through stdin with one path
per line or in a CSV column when using the --path-column flag:

    Multiple arguments through shell glob:
    $ xan parallel count data/**/docs.csv

    One path per line, fed through stdin:
    $ ls data/**/docs.csv | xan parallel count

    Paths from a CSV column through stdin:
    $ cat filelist.csv | xan parallel count --path-column path

Note that sometimes you might find useful to use the `split` or `partition`
command to preemptively split a large file into manageable chunks, if you can
spare the disk space.

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
    - `top`: return top 10 rows (or any count using the -l/--limit flag) maximizing
        given <column>.
    - `map`: writes the result of given preprocessing in a new
        file besides the original one. This subcommand takes a filename template
        where `{}` will be replaced by the name of each target file without any
        extension (`.csv` or `.csv.gz` would be stripped for instance). This
        command is unable to leverage CSV file chunking.

For instance, the following command:

    $ xan parallel map '{}_freq.csv' -P 'freq -s Category' *.csv

Will create a file suffixed \"_freq.csv\" for each CSV file in current directory
containing its frequency table for the \"Category\" command.

Finally, preprocessing on each file can be done using two different methods:

1. Using only xan subcommands with -P, --preprocess:
    $ xan parallel count -P \"search -s name John | slice -l 10\" file.csv

2. Using a subcommand passed to \"$SHELL -c\" or \"cmd /C\" with -H, --shell-preprocess:
    $ xan parallel count -H \"rg john | xan from -f ndjson\" data.ndjson

Usage:
    xan parallel count [options] [<inputs>...]
    xan parallel cat [options] [<inputs>...]
    xan parallel freq [options] [<inputs>...]
    xan parallel stats [options] [<inputs>...]
    xan parallel agg [options] <expr> [<inputs>...]
    xan parallel groupby [options] <group> <expr> [<inputs>...]
    xan parallel top [options] <column> [<inputs>...]
    xan parallel map [options] <template> [<inputs>...]
    xan parallel --help
    xan p count [options] [<inputs>...]
    xan p cat [options] [<inputs>...]
    xan p freq [options] [<inputs>...]
    xan p stats [options] [<inputs>...]
    xan p agg [options] <expr> [<inputs>...]
    xan p groupby [options] <group> <expr> [<inputs>...]
    xan p top [options] <column> [<inputs>...]
    xan p map [options] <template> [<inputs>...]
    xan p --help

parallel options:
    -P, --preprocess <op>        Preprocessing using only `xan` subcommands.
    -H, --shell-preprocess <op>  Preprocessing commands that will run directly in your
                                 own shell using the -c flag.
    --run <path>                 Run xan script at given <path> as preprocessing.
                                 See `xan run -h` for more information.
    --progress                   Display a progress bar for the parallel tasks. The
                                 per file/chunk bars will tick once per CSV row only
                                 AFTER pre-processing!
    -t, --threads <n>            Number of threads to use. Will default to a sensible
                                 number based on the available CPUs.
    --path-column <name>         Name of the path column if stdin is given as a CSV file
                                 instead of one path per line.
    --dont-chunk                 Tell the command not to attempt to split CSV inputs into
                                 chunks when the number of available threads is larger
                                 than the number of files to process. This can be useful
                                 when preprocessing needs to deal with non-standard
                                 CSV files such as those dealt with by `xan input`.

parallel count options:
    -S, --source-column <name>  If given, will return a CSV file containing a column with
                                the source file being counted and a column with the count itself.

parallel cat options:
    -B, --buffer-size <n>       Number of rows a thread is allowed to keep in memory
                                before flushing to the output. Set to -1 for infinite buffer size,
                                which means flushing only once per processed file. This can be
                                useful to ensure resulting rows are grouped by input file in the output.
                                But keep in mind this could also cost a lot of memory.
                                [default: 1024]
    -S, --source-column <name>  Name of a column to prepend in the output of indicating the
                                path to source file.

parallel freq options:
    -s, --select <cols>  Columns for which to build frequency tables.
    --sep <char>         Split the cell into multiple values to count using the
                         provided separator.
    -A, --all            Remove the limit.
    -l, --limit <n>      Limit the frequency table to the N most common
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

parallel top options:
    -l, --limit <n>       Number of top items to return. Cannot be < 1.
                          [default: 10]
    -R, --reverse         Reverse order.
    -L, --lexicographic   Rank values lexicographically instead of considering
                          them as numbers.
    -r, --rank <col>      Name of a rank column to prepend.
    -T, --ties            Keep all rows tied for last. Will therefore
                          consume O(k + t) memory, t being the number of ties.

parallel map options:
    -z, --compress <kind>  Compress created files using either \"gz|gzip\" or \"zst|zstd\"
                           compression.

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
    pub cmd_cat: bool,
    pub cmd_freq: bool,
    pub cmd_stats: bool,
    pub cmd_agg: bool,
    pub cmd_groupby: bool,
    pub cmd_top: bool,
    cmd_map: bool,
    pub arg_inputs: Vec<String>,
    pub arg_expr: Option<String>,
    pub arg_group: Option<SelectedColumns>,
    pub arg_column: Option<SelectedColumns>,
    arg_template: Option<FilenameTemplate>,
    pub flag_preprocess: Option<String>,
    pub flag_shell_preprocess: Option<String>,
    pub flag_run: Option<String>,
    flag_progress: bool,
    pub flag_threads: Option<NonZeroUsize>,
    flag_path_column: Option<SelectedColumns>,
    flag_dont_chunk: bool,
    flag_buffer_size: isize,
    pub flag_source_column: Option<String>,
    pub flag_select: SelectedColumns,
    pub flag_sep: Option<String>,
    pub flag_limit: usize,
    pub flag_no_extra: bool,
    pub flag_all: bool,
    pub flag_cardinality: bool,
    pub flag_quartiles: bool,
    pub flag_approx: bool,
    pub flag_nulls: bool,
    flag_compress: Option<Compression>,
    pub flag_output: Option<String>,
    pub flag_no_headers: bool,
    pub flag_delimiter: Option<Delimiter>,
    pub flag_reverse: bool,
    pub flag_lexicographic: bool,
    pub flag_rank: Option<String>,
    pub flag_ties: bool,
}

type BoxedReader = simd_csv::Reader<Box<dyn io::Read + Send>>;
type BoxedSplitter = simd_csv::Splitter<Box<dyn io::Read + Send>>;

impl Args {
    fn resolve(&mut self) -> CliResult<()> {
        let preprocessing_flags_count = self.flag_preprocess.is_some() as u8
            + self.flag_shell_preprocess.is_some() as u8
            + self.flag_run.is_some() as u8;

        if preprocessing_flags_count > 1 {
            Err("only one of -P/--preprocess, -H/--shell-preprocess or --run can be given!")?;
        }

        if let Some(path) = &self.flag_run {
            self.flag_preprocess = Some(fs::read_to_string(path)?);
        }

        Ok(())
    }

    fn has_preprocessing(&self) -> bool {
        self.flag_preprocess.is_some() || self.flag_shell_preprocess.is_some()
    }

    pub fn single_file(path: &Option<String>, threads: Option<NonZeroUsize>) -> CliResult<Self> {
        match path {
            Some(p) => Ok(Self {
                flag_threads: threads,
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

    fn inputs(&self) -> CliResult<(Vec<Input>, usize)> {
        let mut inputs = if !self.arg_inputs.is_empty() {
            self.arg_inputs.clone()
        } else if io::stdin().is_terminal() {
            vec![]
        } else {
            Config::std()
                .lines(&self.flag_path_column)?
                .collect::<Result<Vec<_>, _>>()?
        };

        if inputs.is_empty() {
            Err("no files to process!\nDid you forget stdin or arguments?")?;
        }

        for p in inputs.iter() {
            if !Path::new(p).is_file() {
                Err(format!("{} does not exist!", p.cyan()))?;
            }
        }

        let threads = self
            .flag_threads
            .unwrap_or_else(|| NonZeroUsize::new(crate::util::default_num_cpus()).unwrap())
            .get();

        // One thread per input or more inputs than threads
        if inputs.len() >= threads {
            return Ok((inputs.into_iter().map(Input::Path).collect(), threads));
        }

        // If we are using `map` or if inputs are not all chunkable
        if self.flag_dont_chunk || self.cmd_map || !inputs.iter().all(|p| Config::is_chunkable(p)) {
            let actual_threads = inputs.len();

            return Ok((
                inputs.into_iter().map(Input::Path).collect(),
                actual_threads,
            ));
        }

        // TODO: we could also weight the number of allocated threads by the size
        // TODO: we could also artificially chunk more to distribute load more evenly
        // in skewed contexts

        // We sort input by size
        // TODO: apply some factor on size when file is gzipped
        inputs.sort_by_key(|p| Path::new(p).metadata().map(|m| m.len()).unwrap_or(0));
        inputs.reverse();

        let mut threads_per_input = vec![0; inputs.len()];

        // Allocating threads
        let mut t: usize = 0;

        for _ in 0..threads {
            threads_per_input[t] += 1;
            t = (t + 1) % inputs.len();
        }

        let mut chunked_inputs = Vec::new();
        let mut actual_threads: usize = 0;

        for (p, t) in inputs.iter().zip(threads_per_input) {
            let config = Config::new(&Some(p.clone()))
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            let mut seeker = config.simd_seeker()?.ok_or("could not sample file!")?;
            let segments = seeker.segments(t)?;

            actual_threads += segments.len();

            // NOTE: if file was too short for segmentation, we fallback to
            // a path input instead
            if segments.len() == 1 {
                chunked_inputs.push(Input::Path(p.clone()));
                continue;
            }

            for (i, (from, to)) in segments.into_iter().enumerate() {
                chunked_inputs.push(Input::FileChunk(FileChunk {
                    file_path: p.clone(),
                    from,
                    to,
                    position: i,
                    headers: seeker.byte_headers().clone(),
                }));
            }
        }

        Ok((chunked_inputs, actual_threads))
    }

    fn io_reader(&self, input: &Input) -> CliResult<InputReader> {
        // Shell preprocessing
        if let Some(preprocessing) = &self.flag_shell_preprocess {
            if preprocessing.trim().is_empty() {
                Err("-H, --shell-preprocess cannot be an empty command!")?;
            }

            let config = Config::std()
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            let shell = if cfg!(target_os = "windows") {
                "cmd"
            } else {
                &env::var("SHELL").map_err(|_| "$SHELL is not set!")?
            };

            let mut cmd = Command::new(shell);
            let mut children: Vec<Child> = Vec::new();

            match input {
                Input::Path(path) => {
                    cmd.stdin(File::open(path)?);
                }
                Input::FileChunk(file_chunk) => {
                    let mut slice_child = Command::new(env::current_exe()?)
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .arg("slice")
                        .arg("--byte-offset")
                        .arg(file_chunk.from.to_string())
                        .arg("--end-byte")
                        .arg(file_chunk.to.to_string())
                        .arg("--raw")
                        .arg(&file_chunk.file_path)
                        .spawn()?;

                    cmd.stdin(
                        slice_child
                            .stdout
                            .take()
                            .expect("could not consume chunk slice stdout"),
                    );

                    children.push(slice_child);
                }
            };

            let mut child = cmd
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .args([
                    if cfg!(target_os = "windows") {
                        "/C"
                    } else {
                        "-c"
                    },
                    preprocessing,
                ])
                .spawn()?;

            let reader = Box::new(child.stdout.take().expect("cannot read child stdout"));

            children.push(child);

            Ok(InputReader {
                config,
                reader: Some(reader),
                headers: None,
                children: Some(Children::from(children)),
            })
        }
        // Standard preprocessing
        else if let Some(preprocessing) = &self.flag_preprocess {
            if preprocessing.trim().is_empty() {
                Err("-P, --preprocess cannot be an empty command!")?;
            }

            let exe = env::current_exe()?;

            let mut children: Vec<Child> = Vec::new();

            if let Input::FileChunk(file_chunk) = input {
                children.push(
                    Command::new(exe.clone())
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .arg("slice")
                        .arg("--byte-offset")
                        .arg(file_chunk.from.to_string())
                        .arg("--end-byte")
                        .arg(file_chunk.to.to_string())
                        .arg("--raw")
                        .arg(&file_chunk.file_path)
                        .spawn()?,
                );
            }

            for step in parse_pipeline(preprocessing)? {
                let mut command = Command::new(exe.clone());
                command.stdout(Stdio::piped()).stderr(Stdio::piped());

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

                children.push(command.spawn()?);
            }

            let config = Config::std()
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            Ok(InputReader {
                config,
                reader: Some(Box::new(
                    children
                        .last_mut()
                        .unwrap()
                        .stdout
                        .take()
                        .expect("cannot read child stdout"),
                )),
                headers: None,
                children: Some(Children::from(children)),
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
                        reader: Some(reader),
                        headers: None,
                        children: None,
                    })
                }
                Input::FileChunk(file_chunk) => {
                    let config = Config::new(&Some(file_chunk.file_path.to_string()))
                        .delimiter(self.flag_delimiter)
                        .no_headers(true);

                    let reader = config.io_reader_at_position_with_limit(
                        file_chunk.from,
                        file_chunk.to - file_chunk.from,
                    )?;

                    Ok(InputReader {
                        config,
                        reader: Some(reader),
                        headers: Some(file_chunk.headers.clone()),
                        children: None,
                    })
                }
            }
        }
    }

    fn process_manager(&self, total: usize) -> ProcessManager {
        let manager = if self.flag_progress {
            console::set_colors_enabled_stderr(true);
            console::set_colors_enabled(true);
            colored::control::set_override(true);

            ProcessManager::with_progress_bar(
                total,
                self.flag_threads
                    .expect("at that point, threads cannot be None")
                    .get(),
                self.cmd_map,
            )
        } else {
            ProcessManager::new()
        };

        if self.has_preprocessing() {
            manager.spawn_checker_thread();
        }

        manager
    }

    fn count(self, inputs: Vec<Input>) -> CliResult<()> {
        let process_manager = self.process_manager(inputs.len());

        if let Some(source_column_name) = &self.flag_source_column {
            let counters_mutex = Mutex::new(BTreeMap::<String, u64>::new());

            inputs.par_iter().try_for_each(|input| -> CliResult<()> {
                let mut input_reader = self.io_reader(input)?;
                let progress_bar =
                    process_manager.start(&input.name(), input_reader.take_children());
                let mut csv_splitter = input_reader.take_simd_csv_splitter();

                let count = csv_splitter.count_records()?;

                progress_bar.inc(count);

                counters_mutex
                    .lock()
                    .unwrap()
                    .entry(input.path().to_string())
                    .and_modify(|c| *c += count)
                    .or_insert(count);

                process_manager.stop(&input.name());

                Ok(())
            })?;

            let mut writer = Config::new(&self.flag_output).simd_writer()?;

            let mut output_record = ByteRecord::new();
            output_record.push_field(source_column_name.as_bytes());
            output_record.push_field(b"count");

            writer.write_byte_record(&output_record)?;

            for (path, count) in counters_mutex.into_inner().unwrap().into_iter() {
                output_record.clear();
                output_record.push_field(path.as_bytes());
                output_record.push_field(count.to_string().as_bytes());

                writer.write_byte_record(&output_record)?;
            }

            writer.flush()?;

            process_manager.succeed();
        } else {
            let total_count = AtomicU64::new(0);

            inputs.par_iter().try_for_each(|input| -> CliResult<()> {
                let mut input_reader = self.io_reader(input)?;
                let progress_bar =
                    process_manager.start(&input.name(), input_reader.take_children());
                let mut csv_splitter = input_reader.take_simd_csv_splitter();

                let count = csv_splitter.count_records()?;

                total_count.fetch_add(count, Ordering::Relaxed);

                progress_bar.inc(count);

                process_manager.stop(&input.name());

                Ok(())
            })?;

            process_manager.succeed();

            writeln!(&mut stdout(), "{}", total_count.into_inner())?;
        }

        Ok(())
    }

    fn cat(self, inputs: Vec<Input>) -> CliResult<()> {
        if !self.has_preprocessing() {
            Err("`xan parallel cat` without -P/--preprocess or -H/--shell-preprocess is counterproductive!\n`xan cat rows` will be faster.")?
        }

        let process_manager = self.process_manager(inputs.len());

        #[inline(always)]
        fn check_headers<'b>(
            no_headers: bool,
            path: &str,
            expected: &mut Option<ByteRecord>,
            headers: &'b ByteRecord,
        ) -> CliResult<Option<&'b ByteRecord>> {
            if !no_headers {
                match expected {
                    Some(expected_headers) if headers != expected_headers => Err(format!(
                        "found inconsistent headers as soon as \"{}\"!\nExpected: {:?}\nGot: {:?}",
                        path, headers, expected_headers
                    ))?,
                    None => {
                        *expected = Some(headers.clone());
                        return Ok(Some(headers));
                    }
                    _ => (),
                }
            } else {
                match expected {
                    Some(expected_headers) if headers.len() != expected_headers.len() => Err(format!("found inconsistent column count as soon as \"{}\"!\nExpected: {}\nGot: {}", path, headers.len(), expected_headers.len()))?,
                    None => {
                        *expected = Some(headers.clone());
                    }
                    _ => ()
                }
            }

            Ok(None)
        }

        // Faster, single-threaded path (still managing preprocessing subprocesses)
        if self.flag_threads.unwrap().get() == 1 {
            let mut headers_opt: Option<ByteRecord> = None;

            let mut writer = Config::new(&self.flag_output).simd_writer()?;

            for input in inputs {
                let mut input_reader = self.io_reader(&input)?;
                let progress_bar =
                    process_manager.start(&input.name(), input_reader.take_children());
                let mut csv_reader = input_reader.take_simd_csv_reader();
                let mut headers = input_reader.headers(csv_reader.byte_headers()?);

                let path = input.path();

                if let Some(source_column) = &self.flag_source_column {
                    headers.push_field(source_column.as_bytes());
                }

                if let Some(headers_to_write) =
                    check_headers(self.flag_no_headers, path, &mut headers_opt, &headers)?
                {
                    writer.write_byte_record(headers_to_write)?;
                }

                let mut record = ByteRecord::new();

                while csv_reader.read_byte_record(&mut record)? {
                    if self.flag_source_column.is_some() {
                        record.push_field(path.as_bytes());
                    }

                    writer.write_byte_record(&record)?;

                    progress_bar.tick();
                }

                process_manager.stop(&input.name());
            }

            return Ok(writer.flush()?);
        }

        // NOTE: the option tracks whether headers were already written
        let writer_mutex = Mutex::new((None, Config::new(&self.flag_output).simd_writer()?));

        let buffer_size_opt = if self.flag_buffer_size <= 0 {
            None
        } else {
            Some(self.flag_buffer_size as usize)
        };

        let flush = |path: &str, headers: &ByteRecord, records: &[ByteRecord]| -> CliResult<()> {
            let mut guard = writer_mutex.lock().unwrap();

            if let Some(headers_to_write) =
                check_headers(self.flag_no_headers, path, &mut guard.0, headers)?
            {
                guard.1.write_byte_record(headers_to_write)?;
            }

            for record in records.iter() {
                guard.1.write_byte_record(record)?;
            }

            guard.1.flush()?;

            Ok(())
        };

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.io_reader(input)?;
            let progress_bar = process_manager.start(&input.name(), input_reader.take_children());
            let mut csv_reader = input_reader.take_simd_csv_reader();
            let mut headers = input_reader.headers(csv_reader.byte_headers()?);

            let path = input.path();

            if let Some(source_column) = &self.flag_source_column {
                headers.push_field(source_column.as_bytes());
            }

            let mut buffer: Vec<ByteRecord> = if let Some(buffer_size) = buffer_size_opt {
                Vec::with_capacity(buffer_size)
            } else {
                Vec::new()
            };

            let mut record = ByteRecord::new();

            while csv_reader.read_byte_record(&mut record)? {
                if matches!(buffer_size_opt, Some(buffer_size) if buffer.len() == buffer_size) {
                    flush(path, &headers, &buffer)?;

                    buffer.clear();
                }

                if self.flag_source_column.is_some() {
                    record.push_field(path.as_bytes());
                }

                buffer.push(record.clone());

                progress_bar.tick();
            }

            if !buffer.is_empty() {
                flush(path, &headers, &buffer)?;
            }

            process_manager.stop(&input.name());

            Ok(())
        })?;

        process_manager.succeed();

        writer_mutex.into_inner().unwrap().1.flush()?;

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

        let process_manager = self.process_manager(inputs.len());

        let total_freq_tables_mutex = Mutex::new(FrequencyTables::new());

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.io_reader(input)?;
            let progress_bar = process_manager.start(&input.name(), input_reader.take_children());
            let mut csv_reader = input_reader.take_simd_csv_reader();
            let headers = input_reader.headers(csv_reader.byte_headers()?);

            let sel = self.flag_select.selection(&headers, true)?;

            let mut freq_tables =
                FrequencyTables::with_capacity(sel.select(&headers).collect(), approx_capacity);

            let mut record = ByteRecord::new();

            while csv_reader.read_byte_record(&mut record)? {
                for (counter, cell) in freq_tables.iter_mut().zip(sel.select(&record)) {
                    if let Some(sep) = &self.flag_sep {
                        for subcell in cell.split_str(sep) {
                            counter.add(subcell.to_vec());
                        }
                    } else {
                        counter.add(cell.to_vec());
                    }
                }

                progress_bar.tick();
            }

            total_freq_tables_mutex.lock().unwrap().merge(freq_tables)?;

            process_manager.stop(&input.name());

            Ok(())
        })?;

        let mut writer = Config::new(&self.flag_output).simd_writer()?;

        let mut output_record = ByteRecord::new();
        output_record.extend([b"field", b"value", b"count"]);

        writer.write_byte_record(&output_record)?;

        let total_freq_tables = total_freq_tables_mutex.into_inner().unwrap();

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

        process_manager.succeed();
        writer.flush()?;

        Ok(())
    }

    fn stats(self, inputs: Vec<Input>) -> CliResult<()> {
        let process_manager = self.process_manager(inputs.len());

        let mut writer = Config::new(&self.flag_output).simd_writer()?;
        writer.write_byte_record(&self.new_stats().headers())?;

        let total_stats_mutex = Mutex::new(StatsTables::new());

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.io_reader(input)?;
            let progress_bar = process_manager.start(&input.name(), input_reader.take_children());
            let mut csv_reader = input_reader.take_simd_csv_reader();
            let headers = input_reader.headers(csv_reader.byte_headers()?);

            let sel = self.flag_select.selection(&headers, true)?;

            let mut local_stats =
                StatsTables::with_capacity(sel.select(&headers).collect(), || self.new_stats());
            let mut record = ByteRecord::new();

            while csv_reader.read_byte_record(&mut record)? {
                for (cell, stats) in sel.select(&record).zip(local_stats.iter_mut()) {
                    stats.process(cell);
                }

                progress_bar.tick();
            }

            total_stats_mutex.lock().unwrap().merge(local_stats)?;

            process_manager.stop(&input.name());

            Ok(())
        })?;

        for (name, stats) in total_stats_mutex.into_inner().unwrap().into_iter() {
            writer.write_byte_record(&stats.results(&name))?;
        }

        process_manager.succeed();
        writer.flush()?;

        Ok(())
    }

    fn agg(self, inputs: Vec<Input>) -> CliResult<()> {
        let process_manager = self.process_manager(inputs.len());

        let total_program_mutex: Mutex<Option<AggregationProgram>> = Mutex::new(None);

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.io_reader(input)?;
            let progress_bar = process_manager.start(&input.name(), input_reader.take_children());
            let mut csv_reader = input_reader.take_simd_csv_reader();
            let headers = input_reader.headers(csv_reader.byte_headers()?);

            let mut record = ByteRecord::new();
            let mut program = AggregationProgram::parse(
                self.arg_expr.as_ref().unwrap(),
                &headers,
                !csv_reader.has_headers(),
            )?;

            let mut index: usize = 0;

            while csv_reader.read_byte_record(&mut record)? {
                program.run_with_record(index, &record)?;
                index += 1;

                progress_bar.tick();
            }

            let mut total_program_opt = total_program_mutex.lock().unwrap();

            match total_program_opt.as_mut() {
                Some(current_program) => current_program.merge(program),
                None => *total_program_opt = Some(program),
            };

            process_manager.stop(&input.name());

            Ok(())
        })?;

        if let Some(mut total_program) = total_program_mutex.into_inner().unwrap() {
            let mut writer = Config::new(&self.flag_output).simd_writer()?;
            writer.write_record(total_program.headers())?;
            writer.write_byte_record(&total_program.finalize(true)?)?;
        }

        process_manager.succeed();

        Ok(())
    }

    fn groupby(self, inputs: Vec<Input>) -> CliResult<()> {
        let process_manager = self.process_manager(inputs.len());

        let total_program_mutex: Mutex<Option<(ByteRecord, GroupAggregationProgram<ByteRecord>)>> =
            Mutex::new(None);

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.io_reader(input)?;
            let progress_bar = process_manager.start(&input.name(), input_reader.take_children());
            let mut csv_reader = input_reader.take_simd_csv_reader();
            let headers = input_reader.headers(csv_reader.byte_headers()?);

            let sel = self.arg_group.clone().unwrap().selection(&headers, true)?;

            let mut record = ByteRecord::new();
            let mut program = GroupAggregationProgram::parse(
                self.arg_expr.as_ref().unwrap(),
                &headers,
                !csv_reader.has_headers(),
            )?;

            let mut index: usize = 0;

            while csv_reader.read_byte_record(&mut record)? {
                let group = sel.select(&record).collect();

                program.run_with_record(group, index, &record)?;
                index += 1;

                progress_bar.tick();
            }

            let mut total_program_opt = total_program_mutex.lock().unwrap();

            match total_program_opt.as_mut() {
                Some((_, current_program)) => current_program.merge(program),
                None => *total_program_opt = Some((sel.select(&headers).collect(), program)),
            };

            process_manager.stop(&input.name());

            Ok(())
        })?;

        if let Some((group_headers, total_program)) = total_program_mutex.into_inner().unwrap() {
            let mut writer = Config::new(&self.flag_output).simd_writer()?;
            let mut output_record = ByteRecord::new();
            output_record.extend(&group_headers);
            output_record.extend(total_program.headers());

            writer.write_record(&output_record)?;

            for result in total_program.into_byte_records(true) {
                let (group, values) = result?;

                output_record.clear();
                output_record.extend(&group);
                output_record.extend(values.into_iter());

                writer.write_byte_record(&output_record)?;
            }
        }

        process_manager.succeed();

        Ok(())
    }

    fn map(self, inputs: Vec<Input>) -> CliResult<()> {
        if !self.has_preprocessing() {
            Err("`xan parallel map` without -P/--preprocess or -H/--shell-preprocess is pointless ;).")?;
        }

        // NOTE: xan p map on chunked file is basically a parallel xan split, but with some caveats
        // NOTE: this work only for CSV output, which has to be the case with -F/--single-file
        // If we don't do this, the output chunk do not have proper headers, and the chunking
        // is not done properly.
        // if self.flag_single_file
        //     && self.flag_preprocess.is_none()
        //     && self.flag_shell_preprocess.is_none()
        // {
        //     self.flag_preprocess = Some("slice".to_string());
        // }

        let process_manager = self.process_manager(inputs.len());
        let template = self.arg_template.clone().unwrap();

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.io_reader(input)?;
            let progress_bar = process_manager.start(&input.name(), input_reader.take_children());

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

            let mut output: Box<dyn io::Write> = if let Some(compression) = self.flag_compress {
                compression.wrap_writer(output_file)?
            } else {
                Box::new(output_file)
            };

            let mut inner = input_reader.take();

            if let Some(bar) = &progress_bar.0 {
                io::copy(&mut bar.wrap_read(&mut inner), &mut output)?;
            } else {
                io::copy(&mut inner, &mut output)?;
            }

            process_manager.stop(&input.name());

            Ok(())
        })?;

        process_manager.succeed();

        Ok(())
    }

    fn top(self, inputs: Vec<Input>) -> CliResult<()> {
        let process_manager = self.process_manager(inputs.len());

        let total_heap_mutex: Mutex<(Option<ByteRecord>, _)> = Mutex::new((
            None,
            TopKHeapMapWithTies::<DynamicOrd<TopValue>, ByteRecord>::with_capacity(
                self.flag_limit,
                self.flag_ties,
            ),
        ));

        let new_value = |cell: &[u8]| -> Option<TopValue> {
            if self.flag_lexicographic {
                TopValue::new_string(cell)
            } else {
                TopValue::new_float(cell)
            }
        };

        inputs.par_iter().try_for_each(|input| -> CliResult<()> {
            let mut input_reader = self.io_reader(input)?;
            let progress_bar = process_manager.start(&input.name(), input_reader.take_children());
            let mut csv_reader = input_reader.take_simd_csv_reader();

            let headers = input_reader.headers(csv_reader.byte_headers()?);
            let score_column = self
                .arg_column
                .as_ref()
                .unwrap()
                .single_selection(&headers, true)?;

            let mut local_heap =
                TopKHeapMapWithTies::<DynamicOrd<TopValue>, ByteRecord>::with_capacity(
                    self.flag_limit,
                    self.flag_ties,
                );

            let mut record = ByteRecord::new();

            while csv_reader.read_byte_record(&mut record)? {
                if let Some(score) = new_value(&record[score_column]) {
                    local_heap
                        .push_with(DynamicOrd::new(score, self.flag_reverse), || record.clone());
                }

                progress_bar.tick();
            }

            let mut total_heap = total_heap_mutex.lock().unwrap();

            total_heap.1.merge(local_heap);

            if total_heap.0.is_none() && !self.flag_no_headers {
                total_heap.0 = Some(headers);
            }

            process_manager.stop(&input.name());

            Ok(())
        })?;

        let (headers_opt, total_heap) = total_heap_mutex.into_inner().unwrap();

        let mut writer = Config::new(&self.flag_output).simd_writer()?;

        if let Some(headers) = headers_opt {
            writer.write_byte_record(&headers)?;
        }

        for (i, (_, record)) in total_heap.into_sorted_vec().into_iter().enumerate() {
            if self.flag_rank.is_some() {
                writer.write_record(once((i + 1).to_string().as_bytes()).chain(record.iter()))?;
            } else {
                writer.write_byte_record(&record)?;
            }
        }

        process_manager.succeed();

        Ok(())
    }

    pub fn run(mut self) -> CliResult<()> {
        self.resolve()?;

        let (inputs, actual_threads) = self.inputs()?;

        if inputs.len() == 1 {
            writeln!(
                &mut stderr(),
                "{}",
                "nothing is actually parallelized!".yellow()
            )?;
        }

        ThreadPoolBuilder::new()
            .num_threads(actual_threads)
            .build_global()
            .expect("could not build thread pool!");

        self.flag_threads = Some(NonZeroUsize::new(actual_threads).unwrap());

        if self.cmd_count {
            self.count(inputs)
        } else if self.cmd_cat {
            self.cat(inputs)
        } else if self.cmd_freq {
            self.freq(inputs)
        } else if self.cmd_stats {
            self.stats(inputs)
        } else if self.cmd_agg {
            self.agg(inputs)
        } else if self.cmd_groupby {
            self.groupby(inputs)
        } else if self.cmd_top {
            self.top(inputs)
        } else if self.cmd_map {
            self.map(inputs)
        } else {
            unreachable!()
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    args.run()
}
