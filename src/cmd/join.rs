use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::sync::Arc;

use aho_corasick::AhoCorasick;
use bstr::ByteSlice;
use pariter::IteratorExt;
use regex::bytes::{RegexSet, RegexSetBuilder};
use simd_csv::{ByteRecord, Writer};

use crate::cmd::sort::{iter_cmp, iter_cmp_num};
use crate::collections::{hash_map::Entry, HashMap, HashSet};
use crate::config::{Config, Delimiter};
use crate::select::{SelectedColumns, Selection};
use crate::urls::LRUTrieMultiMap;
use crate::util;
use crate::CliResult;

#[derive(Deserialize, Clone, Copy)]
enum DropKey {
    None,
    Left,
    Right,
    Both,
}

fn get_row_key(sel: &Selection, row: &ByteRecord, case_insensitive: bool) -> ByteRecord {
    sel.select(row)
        .map(|v| transform(v, case_insensitive))
        .collect()
}

fn transform(bs: &[u8], case_insensitive: bool) -> Cow<'_, [u8]> {
    if !case_insensitive {
        Cow::Borrowed(bs)
    } else {
        Cow::Owned(bs.to_lowercase())
    }
}

fn prefix_header(headers: &ByteRecord, prefix: &String) -> ByteRecord {
    let mut prefixed_headers = ByteRecord::new();

    for column in headers.iter() {
        prefixed_headers.push_field(&[prefix.as_bytes(), column].concat());
    }

    prefixed_headers
}

fn build_headers(
    left_headers: &ByteRecord,
    right_headers: &ByteRecord,
    left_prefix: &Option<String>,
    right_prefix: &Option<String>,
) -> ByteRecord {
    let mut headers = ByteRecord::new();

    for column in left_headers.iter() {
        if let Some(prefix) = left_prefix {
            headers.push_field(&[prefix.as_bytes(), column].concat());
        } else {
            headers.push_field(column);
        }
    }

    for column in right_headers.iter() {
        if let Some(prefix) = right_prefix {
            headers.push_field(&[prefix.as_bytes(), column].concat());
        } else {
            headers.push_field(column);
        }
    }

    headers
}

fn get_padding(len: usize) -> ByteRecord {
    (0..len).map(|_| b"").collect()
}

#[derive(Debug)]
struct IndexNode {
    record: ByteRecord,
    written: bool,
    next: Option<NonZeroUsize>,
}

impl IndexNode {
    fn new(record: ByteRecord) -> Self {
        Self {
            record,
            written: false,
            next: None,
        }
    }
}

// NOTE: I keep both head & tail to keep insertion order easily
// It is possible to keep only the tail instead of course to
// save up more memory, but the output is less understandable
// for the user and not aligned with usual affordances.
#[derive(Debug)]
struct Index {
    case_insensitive: bool,
    nulls: bool,
    map: HashMap<ByteRecord, (usize, usize)>,
    nodes: Vec<IndexNode>,
}

impl Index {
    fn new(case_insensitive: bool, nulls: bool) -> Self {
        Self {
            case_insensitive,
            nulls,
            map: HashMap::new(),
            nodes: Vec::new(),
        }
    }

    fn from_csv_reader<R: io::Read>(
        reader: &mut simd_csv::Reader<R>,
        sel: &Selection,
        case_insensitive: bool,
        nulls: bool,
    ) -> CliResult<Self> {
        let mut index = Index::new(case_insensitive, nulls);

        for result in reader.byte_records() {
            let record = result?;

            index.add(sel, record);
        }

        Ok(index)
    }

    fn add(&mut self, sel: &Selection, record: ByteRecord) {
        let key = get_row_key(sel, &record, self.case_insensitive);

        if !self.nulls && key.iter().all(|c| c.is_empty()) {
            return;
        }

        let next_id = self.nodes.len() + 1;

        match self.map.entry(key) {
            Entry::Occupied(mut entry) => {
                let (_, tail) = entry.get_mut();
                let new_node = IndexNode::new(record);
                self.nodes[*tail - 1].next = Some(NonZeroUsize::new(next_id).unwrap());
                *tail = next_id;
                self.nodes.push(new_node);
            }
            Entry::Vacant(entry) => {
                entry.insert((next_id, next_id));
                self.nodes.push(IndexNode::new(record));
            }
        };
    }

    fn for_each_node_mut<F, E>(
        &mut self,
        sel: &Selection,
        record: &ByteRecord,
        mut callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(&mut IndexNode) -> Result<(), E>,
    {
        let key = get_row_key(sel, record, self.case_insensitive);

        if !self.nulls && key.iter().all(|c| c.is_empty()) {
            return Ok(());
        }

        if let Some((i, _)) = self.map.get(&key) {
            let mut current_node = &mut self.nodes[i - 1];

            callback(current_node)?;

            while let Some(previous_index) = current_node.next {
                current_node = &mut self.nodes[previous_index.get() - 1];
                callback(current_node)?;
            }
        }

        Ok(())
    }

    fn for_each_record<F, E>(
        &mut self,
        sel: &Selection,
        record: &ByteRecord,
        mut callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(&ByteRecord) -> Result<(), E>,
    {
        self.for_each_node_mut(sel, record, |node| callback(&node.record))
    }

    fn records_not_written(&self) -> impl Iterator<Item = &ByteRecord> {
        self.nodes.iter().filter_map(|node| {
            if !node.written {
                Some(&node.record)
            } else {
                None
            }
        })
    }
}

enum FuzzyIndex {
    Substring(AhoCorasick),
    Regex(RegexSet),
    Url(LRUTrieMultiMap<usize>),
}

impl FuzzyIndex {
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

struct FuzzyJoiner {
    index: FuzzyIndex,
    headers: ByteRecord,
    records: Vec<ByteRecord>,
}

impl FuzzyJoiner {
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
Join two CSV files on the specified columns.

The default join operation is an \"inner\" join. This corresponds to the
intersection of rows on the keys specified. The command is also able to
perform a left outer join with --left, a right outer join with --right,
a full outer join with --full, a semi join with --semi, an anti join with --anti
and finally a cartesian product/cross join with --cross.

By default, joins are done case sensitively, but this can be changed using
the -i, --ignore-case flag.

The column arguments specify the columns to join for each input. Columns can
be selected using the same syntax as the \"xan select\" command. Both selections
must return a same number of columns, for the join keys to be properly aligned.

Note that when it is obviously safe to drop the joined columns from one of the files
the command will do so automatically. Else you can tweak the command's behavior
using the -D/--drop-key flag.

Note that this command is able to consume streams such as stdin (in which case
the file name must be \"-\" to indicate which file will be read from stdin).

# Examples

Inner join of two files on a column named differently:

    $ xan join user_id tweets.csv id accounts.csv > joined.csv
join
The same, but with columns named the same:

    $ xan join user_id tweets.csv accounts.csv > joined.csv

Left join:

    $ xan join --left user_id tweets.csv id accounts.csv > joined.csv

Joining on multiple columns:

    $ xan join media,month per-query.csv totals.csv > joined.csv

One file from stdin:

    $ xan filter 'retweets > 10' tweets.csv | xan join user_id - id accounts.csv > joined.csv

Prefixing right column names:

    $ xan join -R user_ user_id tweets.csv id accounts.csv > joined.csv

# Sorted inputs

This command performs what is usually called a \"hash join\". That is to say one
of the files is indexed into an in-memory hashmap for the join operation to work.

Now if you know your input files are sorted in a similar fashion, you can use
the -S/--sorted flag to perform a \"merge join\" instead and perform the operation
while using only constant memory (unless you have many duplicates).

# Fuzzy join

This command is also able to perform a so-called \"fuzzy\" join using the
following flags:

    * -c, --contains: matching a substring (e.g. \"john\" in \"My name is john\")
    * -r, --regex: using a regular expression
    * -u, --url-prefix: matching by url prefix (e.g. \"lemonde.fr/business\")

The file containing patterns has to be, by convention, given on the right, while
the left one should contain values that will be tested against those patterns.

This means --left can still be used to emit rows without any match.

Fuzzy-join is a costly operation, especially when testing a large number of patterns,
so a -p/--parallel and -t/--threads flag can be used to use multiple CPUs and
speed up the search.

A typical use-case for this command is to fuzzy search family
names, using regex patterns, in some text column of a CSV file, all while
keeping any match-related column from the pattern file.

This said, if you only need to filter rows of the first file and don't actually
need the columns from the patterns file (i.e. performing a fuzzy --semi or --anti
join), you should probably use `xan search --patterns` instead.

# Memory considerations (without -S/--sorted)

    - `inner join`: the command does not try to be clever and
                    always indexes the left file, while the right
                    file is streamed. Prefer placing the smaller file
                    on the left.
    - `left join`:  the command always indexes the right file and streams
                    the left file.
    - `right join`: the command always indexes the left file and streams
                    the right file.
    - `full join`:  the command does not try to be clever and
                    always indexes the left file, while the right
                    file is streamed. Prefer placing the smaller file
                    on the left.
    - `semi join`:  the command always indexes the right file and streams the
                    left file.
    - `anti join`:  the command always indexes the right file and streams the
                    left file.
    - `cross join`: the command does not try to be clever and
                    always indexes the left file, while the right
                    file is streamed. Prefer placing the smaller file
                    on the left.
    - `fuzzy join`: the command always indexes patterns of the right file and
                    streams the file on the left.

Usage:
    xan join [options] <columns1> <input1> <columns2> <input2>
    xan join [options] <columns> <input1> <input2>
    xan join [options] --cross <input1> <input2>
    xan join --help

join mode options:
    --inner  Do an \"inner\" join. This only returns rows where
             a match can be found between both data sets. This
             is the command's default, so this flag can be omitted,
             or used for clarity.
    --left   Do an \"outer left\" join. This returns all rows in
             first CSV data set, including rows with no
             corresponding row in the second data set. When no
             corresponding row exists, it is padded out with
             empty fields. This is the reverse of --right.
             Can be used in fuzzy joins.
    --right  Do an \"outer right\" join. This returns all rows in
             second CSV data set, including rows with no
             corresponding row in the first data set. When no
             corresponding row exists, it is padded out with
             empty fields. This is the reverse of --left.
    --full   Do a \"full outer\" join. This returns all rows in
             both data sets with matching records joined. If
             there is no match, the missing side will be padded
             out with empty fields.
    --semi   Only keep rows of left file matching a row in right file.
    --anti   Only keep rows of left file not matching a row in right file.
    --cross  This returns the cartesian product of the given CSV
             files. The number of rows emitted will be equal to N * M,
             where N and M correspond to the number of rows in the given
             data sets, respectively.

fuzzy join mode options:
    -c, --contains    Join by matching substrings.
    -r, --regex       Join by regex patterns.
    -u, --url-prefix  Join by url prefix, i.e. cells must contain urls
                      matching the searched url prefix. Urls are first
                      reordered using a scheme called a LRU, that you can
                      read about here:
                      https://github.com/medialab/ural?tab=readme-ov-file#about-lrus

join options:
    -i, --ignore-case            When set, joins are done case insensitively.
    --nulls                      When set, joins will work on empty fields.
                                 Otherwise, empty keys are completely ignored, i.e. when
                                 column selection yield only empty cells.
    -D, --drop-key <mode>        Indicate whether to drop columns representing the join key
                                 in `left` or `right` file, or `none`, or `both`.
                                 Defaults to `none` or some relevant automatic choice when
                                 obviously convenient (e.g. not when using --full nor -i/--ignore-case
                                 nor fuzzy matching).
    -l, --prefix-left <prefix>   Add a prefix to the names of the columns in the
                                 first dataset.
    -r, --prefix-right <prefix>  Add a prefix to the names of the columns in the
                                 second dataset.

sorted input options:
    -S, --sorted   Use this flag to indicate both inputs are sorted in a
                   similar fashion to speed up computation.
    -R, --reverse  Reverse sort order, i.e. descending order.
    -N, --numeric  Compare keys according to their numerical values instead of
                   the default lexicographic order.

fuzzy join options:
    --simplified-urls        When using -u/--url-prefix, drop irrelevant parts of the urls,
                             like the scheme, `www.` subdomains etc. to facilitate matches.
    -p, --parallel           Whether to use parallelization to speed up computations.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

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
    arg_columns: Option<SelectedColumns>,
    arg_columns1: SelectedColumns,
    arg_input1: String,
    arg_columns2: SelectedColumns,
    arg_input2: String,
    flag_inner: bool,
    flag_left: bool,
    flag_right: bool,
    flag_full: bool,
    flag_semi: bool,
    flag_anti: bool,
    flag_cross: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_ignore_case: bool,
    flag_nulls: bool,
    flag_drop_key: Option<DropKey>,
    flag_delimiter: Option<Delimiter>,
    flag_prefix_left: Option<String>,
    flag_prefix_right: Option<String>,
    flag_contains: bool,
    flag_regex: bool,
    flag_url_prefix: bool,
    flag_simplified_urls: bool,
    flag_parallel: bool,
    flag_threads: Option<usize>,
    flag_sorted: bool,
    flag_reverse: bool,
    flag_numeric: bool,
}

type BoxedReader = simd_csv::Reader<Box<dyn io::Read + Send>>;
type ReaderHandle = (BoxedReader, ByteRecord, Selection);

impl Args {
    fn resolve(&mut self) {
        if let Some(sel) = &self.arg_columns {
            self.arg_columns1 = sel.clone();
            self.arg_columns2 = sel.clone();
        }
    }

    fn build_fuzzy_joiner(&self) -> CliResult<FuzzyJoiner> {
        let rconf = Config::new(&Some(self.arg_input2.clone()))
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.arg_columns2.clone());

        let mut reader = rconf.simd_reader()?;
        let headers = reader.byte_headers()?.clone();
        let pattern_cell_index = rconf.single_selection(&headers)?;
        let selection: Selection = match self.flag_drop_key.unwrap_or(DropKey::None) {
            DropKey::Right | DropKey::Both => {
                Selection::without_indices(headers.len(), &[pattern_cell_index])
            }
            _ => Selection::full(headers.len()),
        };
        let dropped_headers: ByteRecord = selection.select(&headers).collect();

        let mut patterns = Vec::new();
        let mut records = Vec::new();
        let mut url_trie_opt: Option<LRUTrieMultiMap<usize>> = self.flag_url_prefix.then(|| {
            if self.flag_simplified_urls {
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
            FuzzyIndex::Url(url_trie)
        } else if self.flag_regex {
            FuzzyIndex::Regex(
                RegexSetBuilder::new(patterns)
                    .case_insensitive(self.flag_ignore_case)
                    .build()?,
            )
        } else {
            FuzzyIndex::Substring(AhoCorasick::new(patterns)?)
        };

        Ok(FuzzyJoiner {
            index,
            headers: dropped_headers,
            records,
        })
    }

    fn readers(&mut self) -> CliResult<(ReaderHandle, ReaderHandle)> {
        let left = Config::new(&Some(self.arg_input1.clone()))
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.arg_columns1.clone());

        let right = Config::new(&Some(self.arg_input2.clone()))
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.arg_columns2.clone());

        self.flag_no_headers = left.no_headers;

        let mut left_reader = left.simd_reader()?;
        let mut right_reader = right.simd_reader()?;

        let left_headers = left_reader.byte_headers()?.clone();
        let right_headers = right_reader.byte_headers()?.clone();

        let left_sel = left.selection(&left_headers)?;
        let right_sel = right.selection(&right_headers)?;

        if !self.flag_cross && left_sel.len() != right_sel.len() {
            Err("not the same number of columns selected on left & right!")?;
        }

        Ok((
            (left_reader, left_headers, left_sel),
            (right_reader, right_headers, right_sel),
        ))
    }

    fn wconf(&self) -> Config {
        Config::new(&self.flag_output)
    }

    fn index(&self, reader: &mut BoxedReader, sel: &Selection) -> CliResult<Index> {
        Index::from_csv_reader(reader, sel, self.flag_ignore_case, self.flag_nulls)
    }

    fn inverted_selections(
        &mut self,
        left_headers: &ByteRecord,
        left_sel: &Selection,
        right_headers: &ByteRecord,
        right_sel: &Selection,
    ) -> (Selection, Selection) {
        let drop_key = match self.flag_drop_key {
            Some(d) => d,
            None => {
                if !self.flag_no_headers
                    && !self.flag_ignore_case
                    && !self.flag_full
                    && left_sel.select(left_headers).collect::<ByteRecord>()
                        == right_sel.select(right_headers).collect::<ByteRecord>()
                {
                    if self.flag_inner || self.flag_left || self.flag_full {
                        DropKey::Right
                    } else if self.flag_right {
                        DropKey::Left
                    } else {
                        DropKey::None
                    }
                } else {
                    DropKey::None
                }
            }
        };

        let left_inverse_sel = match drop_key {
            DropKey::Left | DropKey::Both => left_sel.inverse(left_headers.len()),
            _ => Selection::full(left_headers.len()),
        };

        let right_inverse_sel = match drop_key {
            DropKey::Right | DropKey::Both => right_sel.inverse(right_headers.len()),
            _ => Selection::full(right_headers.len()),
        };

        (left_inverse_sel, right_inverse_sel)
    }

    fn write_headers<W: io::Write>(
        &self,
        writer: &mut simd_csv::Writer<W>,
        left_headers: &ByteRecord,
        right_headers: &ByteRecord,
    ) -> CliResult<()> {
        if !self.flag_no_headers {
            writer.write_byte_record(&build_headers(
                left_headers,
                right_headers,
                &self.flag_prefix_left,
                &self.flag_prefix_right,
            ))?;
        }

        Ok(())
    }

    fn inner_join(mut self) -> CliResult<()> {
        let (
            (mut left_reader, left_headers, left_sel),
            (mut right_reader, right_headers, right_sel),
        ) = self.readers()?;

        let (inverted_left_sel, inverted_right_sel) =
            self.inverted_selections(&left_headers, &left_sel, &right_headers, &right_sel);

        let mut writer = self.wconf().simd_writer()?;

        self.write_headers(
            &mut writer,
            &inverted_left_sel.select(&left_headers).collect(),
            &inverted_right_sel.select(&right_headers).collect(),
        )?;

        let mut index = self.index(&mut left_reader, &left_sel)?;

        let mut right_record = simd_csv::ByteRecord::new();

        while right_reader.read_byte_record(&mut right_record)? {
            index.for_each_record(&right_sel, &right_record, |left_record| {
                writer.write_record(
                    inverted_left_sel
                        .select(left_record)
                        .chain(inverted_right_sel.select(&right_record)),
                )
            })?;
        }

        Ok(writer.flush()?)
    }

    fn full_outer_join(mut self) -> CliResult<()> {
        let (
            (mut left_reader, left_headers, left_sel),
            (mut right_reader, right_headers, right_sel),
        ) = self.readers()?;

        let (inverted_left_sel, inverted_right_sel) =
            self.inverted_selections(&left_headers, &left_sel, &right_headers, &right_sel);

        let mut writer = self.wconf().simd_writer()?;

        self.write_headers(
            &mut writer,
            &inverted_left_sel.select(&left_headers).collect(),
            &inverted_right_sel.select(&right_headers).collect(),
        )?;

        let left_padding = get_padding(inverted_left_sel.len());
        let right_padding = get_padding(inverted_right_sel.len());

        let mut index = self.index(&mut left_reader, &left_sel)?;

        let mut right_record = simd_csv::ByteRecord::new();

        while right_reader.read_byte_record(&mut right_record)? {
            let mut something_was_written: bool = false;

            index.for_each_node_mut(&right_sel, &right_record, |left_node| {
                something_was_written = true;
                left_node.written = true;
                writer.write_record(
                    inverted_left_sel
                        .select(&left_node.record)
                        .chain(inverted_right_sel.select(&right_record)),
                )
            })?;

            if !something_was_written {
                writer.write_record(
                    left_padding
                        .iter()
                        .chain(inverted_right_sel.select(&right_record)),
                )?;
            }
        }

        for left_record in index.records_not_written() {
            writer.write_record(
                inverted_left_sel
                    .select(left_record)
                    .chain(right_padding.iter()),
            )?;
        }

        Ok(writer.flush()?)
    }

    fn left_join(mut self) -> CliResult<()> {
        let (
            (mut left_reader, left_headers, left_sel),
            (mut right_reader, right_headers, right_sel),
        ) = self.readers()?;

        let (inverted_left_sel, inverted_right_sel) =
            self.inverted_selections(&left_headers, &left_sel, &right_headers, &right_sel);

        let mut writer = self.wconf().simd_writer()?;

        self.write_headers(
            &mut writer,
            &inverted_left_sel.select(&left_headers).collect(),
            &inverted_right_sel.select(&right_headers).collect(),
        )?;

        let right_padding = get_padding(inverted_right_sel.len());

        let mut index = self.index(&mut right_reader, &right_sel)?;

        let mut left_record = simd_csv::ByteRecord::new();

        while left_reader.read_byte_record(&mut left_record)? {
            let mut something_was_written: bool = false;

            index.for_each_record(&left_sel, &left_record, |right_record| {
                something_was_written = true;
                writer.write_record(
                    inverted_left_sel
                        .select(&left_record)
                        .chain(inverted_right_sel.select(right_record)),
                )
            })?;

            if !something_was_written {
                writer.write_record(
                    inverted_left_sel
                        .select(&left_record)
                        .chain(right_padding.iter()),
                )?;
            }
        }

        Ok(writer.flush()?)
    }

    fn right_join(mut self) -> CliResult<()> {
        let (
            (mut left_reader, left_headers, left_sel),
            (mut right_reader, right_headers, right_sel),
        ) = self.readers()?;

        let (inverted_left_sel, inverted_right_sel) =
            self.inverted_selections(&left_headers, &left_sel, &right_headers, &right_sel);

        let mut writer = self.wconf().simd_writer()?;

        self.write_headers(
            &mut writer,
            &inverted_left_sel.select(&left_headers).collect(),
            &inverted_right_sel.select(&right_headers).collect(),
        )?;

        let left_padding = get_padding(inverted_left_sel.len());

        let mut index = self.index(&mut left_reader, &left_sel)?;

        let mut right_record = simd_csv::ByteRecord::new();

        while right_reader.read_byte_record(&mut right_record)? {
            let mut something_was_written: bool = false;

            index.for_each_record(&right_sel, &right_record, |left_record| {
                something_was_written = true;
                writer.write_record(
                    inverted_left_sel
                        .select(left_record)
                        .chain(inverted_right_sel.select(&right_record)),
                )
            })?;

            if !something_was_written {
                writer.write_record(
                    left_padding
                        .iter()
                        .chain(inverted_right_sel.select(&right_record)),
                )?;
            }
        }

        Ok(writer.flush()?)
    }

    fn semi_join(mut self, anti: bool) -> CliResult<()> {
        let ((mut left_reader, left_headers, left_sel), (mut right_reader, _, right_sel)) =
            self.readers()?;

        let mut writer = self.wconf().simd_writer()?;

        if !self.flag_no_headers {
            writer.write_byte_record(&left_headers)?;
        }

        let mut index: HashSet<ByteRecord> = HashSet::new();

        let mut right_record = simd_csv::ByteRecord::new();

        while right_reader.read_byte_record(&mut right_record)? {
            let key = get_row_key(&right_sel, &right_record, self.flag_ignore_case);

            if !self.flag_nulls && key.iter().all(|c| c.is_empty()) {
                continue;
            }

            index.insert(key);
        }

        let mut left_record = simd_csv::ByteRecord::new();

        while left_reader.read_byte_record(&mut left_record)? {
            let key = get_row_key(&left_sel, &left_record, self.flag_ignore_case);
            let mut is_match = index.contains(&key);

            if anti {
                is_match = !is_match;
            }

            if is_match {
                writer.write_byte_record(&left_record)?;
            }
        }

        Ok(writer.flush()?)
    }

    fn merge_join(mut self) -> CliResult<()> {
        let (
            (mut left_reader, left_headers, left_sel),
            (mut right_reader, right_headers, right_sel),
        ) = self.readers()?;

        let (inverted_left_sel, mut inverted_right_sel) =
            self.inverted_selections(&left_headers, &left_sel, &right_headers, &right_sel);

        if self.flag_anti || self.flag_semi {
            inverted_right_sel = Selection::empty();
        }

        let mut writer = self.wconf().simd_writer()?;

        self.write_headers(
            &mut writer,
            &inverted_left_sel.select(&left_headers).collect(),
            &inverted_right_sel.select(&right_headers).collect(),
        )?;

        let cmp = |a: &ByteRecord, b: &ByteRecord| -> Ordering {
            let a_sel = left_sel.select(a);
            let b_sel = right_sel.select(b);

            let ordering = if self.flag_numeric {
                iter_cmp_num(a_sel, b_sel)
            } else {
                iter_cmp(a_sel, b_sel)
            };

            if self.flag_reverse {
                ordering.reverse()
            } else {
                ordering
            }
        };

        let left_padding = get_padding(inverted_left_sel.len());
        let right_padding = get_padding(inverted_right_sel.len());

        let write_only_left = |writer: &mut Writer<Box<dyn Write + Send>>,
                               record: &ByteRecord|
         -> simd_csv::Result<()> {
            writer.write_record(inverted_left_sel.select(record).chain(&right_padding))
        };

        let write_only_right = |writer: &mut Writer<Box<dyn Write + Send>>,
                                record: &ByteRecord|
         -> simd_csv::Result<()> {
            writer.write_record(left_padding.iter().chain(inverted_right_sel.select(record)))
        };

        let mut left_buffer: Vec<ByteRecord> = Vec::new();
        let mut right_buffer: Vec<ByteRecord> = Vec::new();

        let mut left_records = left_reader.byte_records();
        let mut right_records = right_reader.byte_records();

        let mut left_record_opt = left_records.next().transpose()?;
        let mut right_record_opt = right_records.next().transpose()?;

        while let (Some(left_record), Some(right_record)) = (&left_record_opt, &right_record_opt) {
            match cmp(left_record, right_record) {
                Ordering::Equal => {
                    // Collecting left records
                    left_buffer.clear();
                    left_buffer.push(left_record_opt.take().unwrap());
                    left_record_opt = left_records.next().transpose()?;

                    while matches!(&left_record_opt, Some(next_left_record) if cmp(&left_buffer[0], next_left_record).is_eq())
                    {
                        left_buffer.push(left_record_opt.take().unwrap());
                        left_record_opt = left_records.next().transpose()?;
                    }

                    // Collecting right records
                    right_buffer.clear();
                    right_buffer.push(right_record_opt.take().unwrap());
                    right_record_opt = right_records.next().transpose()?;

                    while matches!(&right_record_opt, Some(next_right_record) if cmp(&right_buffer[0], next_right_record).is_eq())
                    {
                        if !self.flag_semi && !self.flag_anti {
                            right_buffer.push(right_record_opt.take().unwrap());
                        }
                        right_record_opt = right_records.next().transpose()?;
                    }

                    if self.flag_semi {
                        for l in left_buffer.iter() {
                            writer.write_byte_record(l)?;
                        }
                    } else if !self.flag_anti {
                        // Cross-product
                        for l in left_buffer.iter() {
                            for r in right_buffer.iter() {
                                writer.write_record(
                                    inverted_left_sel
                                        .select(l)
                                        .chain(inverted_right_sel.select(r)),
                                )?;
                            }
                        }
                    }
                }
                Ordering::Less => {
                    if self.flag_left || self.flag_full || self.flag_anti {
                        write_only_left(&mut writer, left_record)?;
                    }

                    left_record_opt = left_records.next().transpose()?;
                }
                Ordering::Greater => {
                    if self.flag_right || self.flag_full {
                        write_only_right(&mut writer, right_record)?;
                    }

                    right_record_opt = right_records.next().transpose()?;
                }
            }
        }

        let mut remaining_record = ByteRecord::new();

        if self.flag_left || self.flag_full || self.flag_anti {
            if let Some(left_record) = &left_record_opt {
                write_only_left(&mut writer, left_record)?;
            }

            while left_reader.read_byte_record(&mut remaining_record)? {
                write_only_left(&mut writer, &remaining_record)?;
            }
        }

        if self.flag_right || self.flag_full {
            if let Some(right_record) = &right_record_opt {
                write_only_right(&mut writer, right_record)?;
            }

            while right_reader.read_byte_record(&mut remaining_record)? {
                write_only_right(&mut writer, &remaining_record)?;
            }
        }

        Ok(writer.flush()?)
    }

    fn cross_join(mut self) -> CliResult<()> {
        let ((mut left_reader, left_headers, _), (right_reader, right_headers, _)) =
            self.readers()?;

        let mut writer = self.wconf().simd_writer()?;

        self.write_headers(&mut writer, &left_headers, &right_headers)?;

        let index = right_reader
            .into_byte_records()
            .collect::<Result<Vec<_>, _>>()?;

        let mut left_record = simd_csv::ByteRecord::new();

        while left_reader.read_byte_record(&mut left_record)? {
            for right_record in index.iter() {
                writer.write_record(left_record.iter().chain(right_record.iter()))?;
            }
        }

        Ok(writer.flush()?)
    }

    fn fuzzy_join(self) -> CliResult<()> {
        let inner = !self.flag_left;

        let threads = util::parallelization(self.flag_parallel, self.flag_threads);

        let joiner = self.build_fuzzy_joiner()?;
        let mut patterns_headers = joiner.headers.clone();

        if let Some(prefix) = &self.flag_prefix_right {
            patterns_headers = prefix_header(&patterns_headers, prefix);
        }

        let rconf = Config::new(&Some(self.arg_input1.clone()))
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.arg_columns1.clone());

        let mut reader = rconf.simd_reader()?;
        let mut headers = reader.byte_headers()?.clone();
        let sel = rconf.selection(reader.byte_headers()?)?;

        let inverted_sel: Selection = match self.flag_drop_key.unwrap_or(DropKey::None) {
            DropKey::Left | DropKey::Both => sel.inverse(headers.len()),
            _ => Selection::full(headers.len()),
        };

        if let Some(prefix) = &self.flag_prefix_left {
            headers = prefix_header(&headers, prefix);
        }

        let dropped_headers: ByteRecord = inverted_sel.select(&headers).collect();

        let padding = vec![b""; patterns_headers.len()];

        let mut writer = Config::new(&self.flag_output).simd_writer()?;

        if !rconf.no_headers {
            writer.write_record(dropped_headers.iter().chain(patterns_headers.iter()))?;
        }

        // Parallel
        if let Some(t) = threads {
            let joiner = Arc::new(joiner);
            let joiner_handle = joiner.clone();

            reader
                .into_byte_records()
                .parallel_map_custom(
                    |o| o.threads(t),
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
                        writer.write_record(
                            inverted_sel.select(&record).chain(pattern_record.iter()),
                        )?;
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
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve();

    let normal_operations = args.flag_inner as u8
        + args.flag_left as u8
        + args.flag_right as u8
        + args.flag_full as u8
        + args.flag_semi as u8
        + args.flag_anti as u8
        + args.flag_cross as u8;

    let fuzzy_operations =
        args.flag_contains as u8 + args.flag_regex as u8 + args.flag_url_prefix as u8;

    match normal_operations + fuzzy_operations {
        0 => args.flag_inner = true,
        1 => (),
        2 if fuzzy_operations == 1 && args.flag_left => (),
        _ => Err("Please pick exactly one join operation!")?,
    }

    if fuzzy_operations > 1 {
        Err("Please exactly one fuzzy join (-c, -r, -u) operation!")?;
    }

    if fuzzy_operations == 1 && normal_operations == 1 && !args.flag_left {
        Err("fuzzy join (-c, -r, -u) is only compatible with --left!")?;
    }

    if fuzzy_operations == 0 && (args.flag_parallel || args.flag_threads.is_some()) {
        Err("-p/--parallel or -t/--threads only work with fuzzy joins (-c, -r, -u)!")?;
    }

    if fuzzy_operations == 1 && args.flag_sorted {
        Err("-S/--sorted does not work with fuzzy joins (-c, -r, -u)!")?;
    }

    if args.flag_sorted && args.flag_cross {
        Err("-S/--sorted does not make sense with --cross!")?;
    }

    if fuzzy_operations == 1 {
        args.fuzzy_join()
    } else if args.flag_sorted {
        args.merge_join()
    } else if args.flag_left {
        args.left_join()
    } else if args.flag_right {
        args.right_join()
    } else if args.flag_full {
        args.full_outer_join()
    } else if args.flag_cross {
        args.cross_join()
    } else if args.flag_semi {
        args.semi_join(false)
    } else if args.flag_anti {
        args.semi_join(true)
    } else {
        args.inner_join()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(values: &[&str]) -> ByteRecord {
        let mut record = ByteRecord::new();

        for v in values.iter() {
            record.push_field(v.as_bytes());
        }

        record
    }

    impl Index {
        fn test_vec(&mut self, sel: &Selection, record: &ByteRecord) -> Vec<ByteRecord> {
            let mut v = Vec::new();

            self.for_each_node_mut(sel, record, |node| -> Result<(), ()> {
                v.push(node.record.clone());
                Ok(())
            })
            .unwrap();

            v
        }
    }

    #[test]
    fn test_index_linked_lists() {
        let mut index = Index::new(false, false);
        let sel = Selection::full(1);

        index.add(&sel, ByteRecord::from(rec(&["a", "one"])));
        index.add(&sel, ByteRecord::from(rec(&["b", "one"])));
        index.add(&sel, ByteRecord::from(rec(&["a", "two"])));
        index.add(&sel, ByteRecord::from(rec(&["a", "three"])));
        index.add(&sel, ByteRecord::from(rec(&["b", "two"])));
        index.add(&sel, ByteRecord::from(rec(&["c", "one"])));

        assert_eq!(
            index.test_vec(&sel, &rec(&["d", "one"])),
            Vec::<ByteRecord>::new()
        );
        assert_eq!(
            index.test_vec(&sel, &rec(&["a", "one"])),
            vec![rec(&["a", "one"]), rec(&["a", "two"]), rec(&["a", "three"])]
        );
        assert_eq!(
            index.test_vec(&sel, &rec(&["b", "one"])),
            vec![rec(&["b", "one"]), rec(&["b", "two"])]
        );
        assert_eq!(
            index.test_vec(&sel, &rec(&["c", "one"])),
            vec![rec(&["c", "one"])]
        );
    }
}
