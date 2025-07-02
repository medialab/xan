use std::io;
use std::num::NonZeroUsize;

use bstr::ByteSlice;
use csv::ByteRecord;

use crate::collections::{hash_map::Entry, HashMap, HashSet};
use crate::config::{Config, Delimiter};
use crate::select::{SelectColumns, Selection};
use crate::util;
use crate::CliResult;

type IndexKey = Vec<Vec<u8>>;

fn get_row_key(sel: &Selection, row: &ByteRecord, case_insensitive: bool) -> IndexKey {
    sel.select(row)
        .map(|v| transform(v, case_insensitive))
        .collect()
}

fn transform(bs: &[u8], case_insensitive: bool) -> Vec<u8> {
    if !case_insensitive {
        bs.to_vec()
    } else {
        bs.to_lowercase()
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

fn get_padding(headers: &ByteRecord) -> ByteRecord {
    (0..headers.len()).map(|_| b"").collect()
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
    map: HashMap<IndexKey, (usize, usize)>,
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
        reader: &mut csv::Reader<R>,
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
Join two sets of CSV data on the specified columns.

The default join operation is an \"inner\" join. This corresponds to the
intersection of rows on the keys specified. The command is also able to
perform a left outer join with --left, a right outer join with --right,
a full outer join with --full, a semi join with --semi, an antin join with --anti
and finally a cartesian product/cross join with --cross.

By default, joins are done case sensitively, but this can be disabled using
the -i, --ignore-case flag.

The column arguments specify the columns to join for each input. Columns can
be selected using the same syntax as the \"xan select\" command. Both selections
must return a same number of columns, for the join keys to be properly aligned.

Note that this command is able to consume streams such as stdin (in which case
the file name must be \"-\" to indicate which file will be read from stdin) and
gzipped files out of the box.

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
    xan join [options] --cross <input1> <input2>
    xan join --help

join options:
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
    arg_columns1: SelectColumns,
    arg_input1: String,
    arg_columns2: SelectColumns,
    arg_input2: String,
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
    flag_delimiter: Option<Delimiter>,
    flag_prefix_left: Option<String>,
    flag_prefix_right: Option<String>,
}

type BoxedReader = csv::Reader<Box<dyn io::Read + Send>>;

impl Args {
    fn readers_and_selections(
        &self,
    ) -> CliResult<((BoxedReader, Selection), (BoxedReader, Selection))> {
        let left = Config::new(&Some(self.arg_input1.clone()))
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.arg_columns1.clone());

        let right = Config::new(&Some(self.arg_input2.clone()))
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.arg_columns2.clone());

        let mut left_reader = left.reader()?;
        let mut right_reader = right.reader()?;

        let left_sel = left.selection(left_reader.byte_headers()?)?;
        let right_sel = right.selection(right_reader.byte_headers()?)?;

        if !self.flag_cross && left_sel.len() != right_sel.len() {
            Err("not the same number of columns selected on left & right!")?;
        }

        Ok(((left_reader, left_sel), (right_reader, right_sel)))
    }

    fn wconf(&self) -> Config {
        Config::new(&self.flag_output)
    }

    fn index(&self, reader: &mut BoxedReader, sel: &Selection) -> CliResult<Index> {
        Index::from_csv_reader(reader, sel, self.flag_ignore_case, self.flag_nulls)
    }

    fn write_headers<W: io::Write>(
        &self,
        writer: &mut csv::Writer<W>,
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

    fn inner_join(self) -> CliResult<()> {
        let ((mut left_reader, left_sel), (mut right_reader, right_sel)) =
            self.readers_and_selections()?;

        let mut writer = self.wconf().writer()?;

        self.write_headers(
            &mut writer,
            left_reader.byte_headers()?,
            right_reader.byte_headers()?,
        )?;

        let mut index = self.index(&mut left_reader, &left_sel)?;

        let mut right_record = csv::ByteRecord::new();

        while right_reader.read_byte_record(&mut right_record)? {
            index.for_each_record(&right_sel, &right_record, |left_record| {
                writer.write_record(left_record.iter().chain(right_record.iter()))
            })?;
        }

        Ok(writer.flush()?)
    }

    fn full_outer_join(self) -> CliResult<()> {
        let ((mut left_reader, left_sel), (mut right_reader, right_sel)) =
            self.readers_and_selections()?;

        let mut writer = self.wconf().writer()?;

        let left_headers = left_reader.byte_headers()?.clone();
        let right_headers = right_reader.byte_headers()?.clone();

        let left_padding = get_padding(&left_headers);
        let right_padding = get_padding(&right_headers);

        self.write_headers(&mut writer, &left_headers, &right_headers)?;

        let mut index = self.index(&mut left_reader, &left_sel)?;

        let mut right_record = csv::ByteRecord::new();

        while right_reader.read_byte_record(&mut right_record)? {
            let mut something_was_written: bool = false;

            index.for_each_node_mut(&right_sel, &right_record, |left_node| {
                something_was_written = true;
                left_node.written = true;
                writer.write_record(left_node.record.iter().chain(right_record.iter()))
            })?;

            if !something_was_written {
                writer.write_record(left_padding.iter().chain(right_record.iter()))?;
            }
        }

        for left_record in index.records_not_written() {
            writer.write_record(left_record.iter().chain(right_padding.iter()))?;
        }

        Ok(writer.flush()?)
    }

    fn left_join(self) -> CliResult<()> {
        let ((mut left_reader, left_sel), (mut right_reader, right_sel)) =
            self.readers_and_selections()?;

        let mut writer = self.wconf().writer()?;

        let left_headers = left_reader.byte_headers()?.clone();
        let right_headers = right_reader.byte_headers()?.clone();

        let right_padding = get_padding(&right_headers);

        self.write_headers(&mut writer, &left_headers, &right_headers)?;

        let mut index = self.index(&mut right_reader, &right_sel)?;

        let mut left_record = csv::ByteRecord::new();

        while left_reader.read_byte_record(&mut left_record)? {
            let mut something_was_written: bool = false;

            index.for_each_record(&left_sel, &left_record, |right_record| {
                something_was_written = true;
                writer.write_record(left_record.iter().chain(right_record.iter()))
            })?;

            if !something_was_written {
                writer.write_record(left_record.iter().chain(right_padding.iter()))?;
            }
        }

        Ok(writer.flush()?)
    }

    fn right_join(self) -> CliResult<()> {
        let ((mut left_reader, left_sel), (mut right_reader, right_sel)) =
            self.readers_and_selections()?;

        let mut writer = self.wconf().writer()?;

        let left_headers = left_reader.byte_headers()?.clone();
        let right_headers = right_reader.byte_headers()?.clone();

        let left_padding = get_padding(&left_headers);

        self.write_headers(&mut writer, &left_headers, &right_headers)?;

        let mut index = self.index(&mut left_reader, &left_sel)?;

        let mut right_record = csv::ByteRecord::new();

        while right_reader.read_byte_record(&mut right_record)? {
            let mut something_was_written: bool = false;

            index.for_each_record(&right_sel, &right_record, |left_record| {
                something_was_written = true;
                writer.write_record(left_record.iter().chain(right_record.iter()))
            })?;

            if !something_was_written {
                writer.write_record(left_padding.iter().chain(right_record.iter()))?;
            }
        }

        Ok(writer.flush()?)
    }

    fn semi_join(self, anti: bool) -> CliResult<()> {
        let ((mut left_reader, left_sel), (mut right_reader, right_sel)) =
            self.readers_and_selections()?;

        let mut writer = self.wconf().writer()?;

        if !self.flag_no_headers {
            writer.write_byte_record(left_reader.byte_headers()?)?;
        }

        let mut index: HashSet<IndexKey> = HashSet::new();

        let mut right_record = csv::ByteRecord::new();

        while right_reader.read_byte_record(&mut right_record)? {
            let key = get_row_key(&right_sel, &right_record, self.flag_ignore_case);

            if !self.flag_nulls && key.iter().all(|c| c.is_empty()) {
                continue;
            }

            index.insert(key);
        }

        let mut left_record = csv::ByteRecord::new();

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

    fn cross_join(self) -> CliResult<()> {
        let ((mut left_reader, _), (mut right_reader, _)) = self.readers_and_selections()?;

        let mut writer = self.wconf().writer()?;

        self.write_headers(
            &mut writer,
            left_reader.byte_headers()?,
            right_reader.byte_headers()?,
        )?;

        let index = right_reader
            .into_byte_records()
            .collect::<Result<Vec<_>, _>>()?;

        let mut left_record = csv::ByteRecord::new();

        while left_reader.read_byte_record(&mut left_record)? {
            for right_record in index.iter() {
                writer.write_record(left_record.iter().chain(right_record.iter()))?;
            }
        }

        Ok(writer.flush()?)
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if [
        args.flag_left,
        args.flag_right,
        args.flag_full,
        args.flag_semi,
        args.flag_anti,
        args.flag_cross,
    ]
    .iter()
    .filter(|flag| **flag)
    .count()
        > 1
    {
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
