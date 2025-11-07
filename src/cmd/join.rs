use std::borrow::Cow;
use std::io;
use std::num::NonZeroUsize;

use bstr::ByteSlice;
use simd_csv::ByteRecord;

use crate::collections::{hash_map::Entry, HashMap, HashSet};
use crate::config::{Config, Delimiter};
use crate::select::{SelectedColumns, Selection};
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

fn transform(bs: &[u8], case_insensitive: bool) -> Cow<[u8]> {
    if !case_insensitive {
        Cow::Borrowed(bs)
    } else {
        Cow::Owned(bs.to_lowercase())
    }
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

# Memory considerations

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

Usage:
    xan join [options] <columns1> <input1> <columns2> <input2>
    xan join [options] <columns> <input1> <input2>
    xan join [options] --cross <input1> <input2>
    xan join --help

join options:
    --inner                      Do an \"inner\" join. This only returns rows where
                                 a match can be found between both data sets. This
                                 is the command's default, so this flag can be omitted,
                                 or used for clarity.
    --left                       Do an \"outer left\" join. This returns all rows in
                                 first CSV data set, including rows with no
                                 corresponding row in the second data set. When no
                                 corresponding row exists, it is padded out with
                                 empty fields. This is the reverse of --right.
    --right                      Do an \"outer right\" join. This returns all rows in
                                 second CSV data set, including rows with no
                                 corresponding row in the first data set. When no
                                 corresponding row exists, it is padded out with
                                 empty fields. This is the reverse of --left.
    --full                       Do a \"full outer\" join. This returns all rows in
                                 both data sets with matching records joined. If
                                 there is no match, the missing side will be padded
                                 out with empty fields.
    --semi                       Only keep rows of left file matching a row in right file.
    --anti                       Only keep rows of left file not matching a row in right file.
    --cross                      This returns the cartesian product of the given CSV
                                 files. The number of rows emitted will be equal to N * M,
                                 where N and M correspond to the number of rows in the given
                                 data sets, respectively.
    -i, --ignore-case            When set, joins are done case insensitively.
    --nulls                      When set, joins will work on empty fields.
                                 Otherwise, empty keys are completely ignored, i.e. when
                                 column selection yield only empty cells.
    -D, --drop-key <mode>        Indicate whether to drop columns representing the join key
                                 in `left` or `right` file, or `none`, or `both`.
                                 Defaults to `none` unless joined columns are named the same
                                 and -i, --ignore-case is not set.
    -L, --prefix-left <prefix>   Add a prefix to the names of the columns in the
                                 first dataset.
    -R, --prefix-right <prefix>  Add a prefix to the names of the columns in the
                                 second dataset.

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
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve();

    let operation = args.flag_inner as u8
        + args.flag_left as u8
        + args.flag_right as u8
        + args.flag_full as u8
        + args.flag_semi as u8
        + args.flag_anti as u8
        + args.flag_cross as u8;

    if operation == 0 {
        args.flag_inner = true;
    } else if operation > 1 {
        Err("Please pick exactly one join operation.")?;
    }

    if args.flag_left {
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
