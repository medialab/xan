use std::collections::BTreeSet;
use std::sync::Arc;

use aho_corasick::AhoCorasick;
use pariter::IteratorExt;
use regex::bytes::{RegexSet, RegexSetBuilder};
use simd_csv::ByteRecord;

use crate::config::{Config, Delimiter};
use crate::select::{SelectedColumns, Selection};
use crate::urls::LRUTrieMultiMap;
use crate::util;
use crate::CliResult;

fn prefix_header(headers: &ByteRecord, prefix: &String) -> ByteRecord {
    let mut prefixed_headers = ByteRecord::new();

    for column in headers.iter() {
        prefixed_headers.push_field(&[prefix.as_bytes(), column].concat());
    }

    prefixed_headers
}

#[derive(Deserialize, Clone, Copy)]
enum DropKey {
    None,
    Left,
    Right,
    Both,
}

enum Index {
    Substring(AhoCorasick),
    Regex(RegexSet),
    Url(LRUTrieMultiMap<usize>),
}

impl Index {
    fn matches(&self, cell: &[u8], matches: &mut BTreeSet<usize>) {
        match self {
            Self::Substring(inner) => {
                for m in inner.find_iter(cell) {
                    matches.insert(m.pattern().as_usize());
                }
            }
            Self::Regex(inner) => {
                for m in inner.matches(cell) {
                    matches.insert(m);
                }
            }
            Self::Url(inner) => {
                if let Ok(url) = std::str::from_utf8(cell) {
                    if let Ok(ids) = inner.longest_matching_prefix_values(url) {
                        for id in ids {
                            matches.insert(*id);
                        }
                    }
                }
            }
        }
    }
}

struct Joiner {
    index: Index,
    headers: ByteRecord,
    records: Vec<ByteRecord>,
}

impl Joiner {
    fn matches(&self, cell: &[u8], matches: &mut BTreeSet<usize>) {
        self.index.matches(cell, matches);
    }

    fn matched_records<'a, 'b>(
        &'a self,
        matches: &'b BTreeSet<usize>,
    ) -> impl Iterator<Item = &'a ByteRecord> + 'b
    where
        'a: 'b,
    {
        matches.iter().copied().map(|i| &self.records[i])
    }
}

static USAGE: &str = "
Join a CSV file containing a column of patterns that will be matched with rows
of another CSV file.

This command has several flags to select the way to perform matches:

    * (default): matching a substring (e.g. \"john\" in \"My name is john\")
    * -r, --regex: using a regular expression
    * -u, --url-prefix: matching by url prefix (e.g. \"lemonde.fr/business\")

The default behavior of this command is to do an 'inner join', which
means only matched rows will be written in the output. Use the --left
flag if you want to perform a 'left join' and keep every row of the searched
file in the output.

The file containing patterns will always be completely read in memory
while the second one will always be streamed.

You can of course work on gzipped files if needed and feed one of both
files from stdin by using `-` instead of a path.

Fuzzy-join is a costly operation, especially when testing a large number of patterns,
so a -p/--parallel and -t/--threads flag can be used to use multiple CPUs and
speed up the search.

A typical use-case for this command is to fuzzy search family
names, using regex patterns, in some text column of a CSV file, all while
keeping any match-related column from the pattern file.

This said, if you only need to filter rows of the second file and don't
actually need to join columns from the patterns file, you should
probably use `xan search --regex --patterns` instead.

Usage:
    xan fuzzy-join [options] <columns> <input> <pattern-column> <patterns>
    xan fuzzy-join --help

fuzzy-join options:
    -r, --regex                  Join by regex patterns.
    -u, --url-prefix             Join by url prefix, i.e. cells must contain urls
                                 matching the searched url prefix. Urls are first
                                 reordered using a scheme called a LRU, that you can
                                 read about here:
                                 https://github.com/medialab/ural?tab=readme-ov-file#about-lrus
    -i, --ignore-case            Make the patterns case-insensitive.
    -S, --simplified             When using -u/--url-prefix, drop irrelevant parts of the urls,
                                 like the scheme, `www.` subdomains etc. to facilitate matches.
    --left                       Write every row from input file in the output, with empty
                                 padding cells on the right when no regex pattern from the second
                                 file produced any match.
    -p, --parallel               Whether to use parallelization to speed up computations.
                                 Will automatically select a suitable number of threads to use
                                 based on your number of cores. Use -t, --threads if you want to
                                 indicate the number of threads yourself.
    -t, --threads <threads>      Parellize computations using this many threads. Use -p, --parallel
                                 if you want the number of threads to be automatically chosen instead.
    -D, --drop-key <mode>        Indicate whether to drop columns representing the join key
                                 in `left` (i.e. input file) or `right` file (i.e. pattern file),
                                 or `none`, or `both`.
                                 [default: none]
    -L, --prefix-left <prefix>   Add a prefix to the names of the columns in the
                                 searched file.
    -R, --prefix-right <prefix>  Add a prefix to the names of the columns in the
                                 patterns file.

Common options:
    -h, --help                  Display this message
    -o, --output <file>         Write output to <file> instead of stdout.
    -n, --no-headers            When set, the first row will not be interpreted
                                as headers. (i.e., They are not searched, analyzed,
                                sliced, etc.)
    -d, --delimiter <arg>       The field delimiter for reading CSV data.
                                Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_columns: SelectedColumns,
    arg_input: String,
    arg_pattern_column: SelectedColumns,
    arg_patterns: String,
    flag_regex: bool,
    flag_url_prefix: bool,
    flag_left: bool,
    flag_simplified: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_ignore_case: bool,
    flag_delimiter: Option<Delimiter>,
    flag_drop_key: DropKey,
    flag_prefix_left: Option<String>,
    flag_prefix_right: Option<String>,
    flag_parallel: bool,
    flag_threads: Option<usize>,
}

impl Args {
    fn build_joiner(&self) -> CliResult<Joiner> {
        let rconf = Config::new(&Some(self.arg_patterns.clone()))
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.arg_pattern_column.clone());

        let mut reader = rconf.simd_reader()?;
        let headers = reader.byte_headers()?.clone();
        let pattern_cell_index = rconf.single_selection(&headers)?;
        let selection: Selection = match self.flag_drop_key {
            DropKey::Right | DropKey::Both => {
                Selection::without_indices(headers.len(), &[pattern_cell_index])
            }
            _ => Selection::full(headers.len()),
        };
        let dropped_headers: ByteRecord = selection.select(&headers).collect();

        let mut patterns = Vec::new();
        let mut records = Vec::new();
        let mut url_trie_opt: Option<LRUTrieMultiMap<usize>> = self.flag_url_prefix.then(|| {
            if self.flag_simplified {
                LRUTrieMultiMap::new_simplified()
            } else {
                LRUTrieMultiMap::new()
            }
        });

        for (i, record) in reader.into_byte_records().enumerate() {
            let record = record?;
            let pattern = String::from_utf8(record[pattern_cell_index].to_vec()).unwrap();

            if let Some(url_trie) = &mut url_trie_opt {
                url_trie.insert(&pattern, i)?;
            } else {
                patterns.push(pattern);
            }
            let dropped_reccord: ByteRecord = selection.select(&record).collect();
            records.push(dropped_reccord);
        }

        let index = if let Some(url_trie) = url_trie_opt {
            Index::Url(url_trie)
        } else if self.flag_regex {
            Index::Regex(
                RegexSetBuilder::new(patterns)
                    .case_insensitive(self.flag_ignore_case)
                    .build()?,
            )
        } else {
            Index::Substring(AhoCorasick::new(patterns)?)
        };

        Ok(Joiner {
            index,
            headers: dropped_headers,
            records,
        })
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let inner = !args.flag_left;

    let parallelization = match (args.flag_parallel, args.flag_threads) {
        (true, None) => Some(None),
        (_, Some(count)) => Some(Some(count)),
        _ => None,
    };

    let joiner = args.build_joiner()?;
    let mut patterns_headers = joiner.headers.clone();

    if let Some(prefix) = &args.flag_prefix_right {
        patterns_headers = prefix_header(&patterns_headers, prefix);
    }

    let rconf = Config::new(&Some(args.arg_input.clone()))
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns.clone());

    let mut reader = rconf.simd_reader()?;
    let mut headers = reader.byte_headers()?.clone();
    let sel = rconf.selection(reader.byte_headers()?)?;

    let inverted_sel: Selection = match args.flag_drop_key {
        DropKey::Left | DropKey::Both => sel.inverse(headers.len()),
        _ => Selection::full(headers.len()),
    };

    if let Some(prefix) = &args.flag_prefix_left {
        headers = prefix_header(&headers, prefix);
    }

    let dropped_headers: ByteRecord = inverted_sel.select(&headers).collect();

    let padding = vec![b""; patterns_headers.len()];

    let mut writer = Config::new(&args.flag_output).simd_writer()?;

    if !rconf.no_headers {
        writer.write_record(dropped_headers.iter().chain(patterns_headers.iter()))?;
    }

    // Parallel
    if let Some(threads) = parallelization {
        let joiner = Arc::new(joiner);
        let joiner_handle = joiner.clone();

        reader
            .into_byte_records()
            .parallel_map_custom(
                |o| o.threads(threads.unwrap_or_else(crate::util::default_num_cpus)),
                move |result| -> CliResult<(ByteRecord, BTreeSet<usize>)> {
                    let record = result?;

                    let mut matches = BTreeSet::new();

                    for cell in sel.select(&record) {
                        joiner.matches(cell, &mut matches);
                    }

                    Ok((record, matches))
                },
            )
            .try_for_each(|result| -> CliResult<()> {
                let (mut record, matches) = result?;

                for pattern_record in joiner_handle.matched_records(&matches) {
                    writer
                        .write_record(inverted_sel.select(&record).chain(pattern_record.iter()))?;
                }

                if !inner && matches.is_empty() {
                    record.extend(&padding);
                    writer.write_byte_record(&record)?;
                }

                Ok(())
            })?;

        return Ok(writer.flush()?);
    }

    // Single-threaded
    let mut record = ByteRecord::new();
    let mut matches = BTreeSet::new();

    while reader.read_byte_record(&mut record)? {
        matches.clear();

        for cell in sel.select(&record) {
            joiner.matches(cell, &mut matches);
        }

        for pattern_record in joiner.matched_records(&matches) {
            writer.write_record(inverted_sel.select(&record).chain(pattern_record.iter()))?;
        }

        if !inner && matches.is_empty() {
            record.extend(&padding);
            writer.write_byte_record(&record)?;
        }
    }

    Ok(writer.flush()?)
}
