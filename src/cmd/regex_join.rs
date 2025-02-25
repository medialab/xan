use std::cmp::Ordering;
use std::str;

use pariter::IteratorExt;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

fn prefix_header(headers: &csv::ByteRecord, prefix: &String) -> csv::ByteRecord {
    let mut prefixed_headers = csv::ByteRecord::new();

    for column in headers.iter() {
        prefixed_headers.push_field(&[prefix.as_bytes(), column].concat());
    }

    prefixed_headers
}

fn union_of_sorted_lists(a: &[usize], b: &[usize]) -> Vec<usize> {
    let n = a.len();
    let m = b.len();

    let mut r: Vec<usize> = Vec::with_capacity(n.max(m));

    let mut i: usize = 0;
    let mut j: usize = 0;

    while i < n && j < m {
        let a_item = a[i];
        let b_item = b[j];

        match a_item.cmp(&b_item) {
            Ordering::Less => {
                r.push(a_item);
                i += 1;
            }
            Ordering::Greater => {
                r.push(b_item);
                j += 1;
            }
            Ordering::Equal => {
                r.push(a_item);
                i += 1;
                j += 1;
            }
        };
    }

    while i < n {
        r.push(a[i]);
        i += 1;
    }

    while j < m {
        r.push(b[j]);
        j += 1;
    }

    r
}

static USAGE: &str = "
Join a CSV file containing a column of regex patterns with another CSV file.

The default behavior of this command is to be an 'inner join', which
means only matched rows will be written in the output. Use the --left
flag if you want to perform a 'left join' and keep every row of the searched
file in the output.

The file containing patterns will always be completely read in memory
while the second one will always be streamed.

You can of course work on gzipped files if needed and feed one of both
files from stdin by using `-` instead of a path.

Note that this commands relies on a regexset under the hood and is
more performant than just testing every regex pattern for each row
of the other CSV file.

This remains a costly operation, especially when testing a large
number of regex patterns, so a -p/--parallel and -t/--threads
flag can be used to use multiple CPUs and speed up the search.

A typical use-case for this command is to fuzzy search family
names, using regex patterns, in some text column of a CSV file, all while
keeping any match-related column from the pattern file.

This said, if you only need to filter rows of the second file and don't
actually need to join columns from the patterns file, you should
probably use `xan search --patterns` instead.

Usage:
    xan regex-join [options] <columns> <input> <pattern-column> <patterns>
    xan regex-join --help

join options:
    -i, --ignore-case            Make the regex patterns case-insensitive.
    --left                       Write every row from input file in the output, with empty
                                 padding cells on the right when no regex pattern from the second
                                 file produced any match.
    -p, --parallel               Whether to use parallelization to speed up computations.
                                 Will automatically select a suitable number of threads to use
                                 based on your number of cores. Use -t, --threads if you want to
                                 indicate the number of threads yourself.
    -t, --threads <threads>      Parellize computations using this many threads. Use -p, --parallel
                                 if you want the number of threads to be automatically chosen instead.
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
    arg_columns: SelectColumns,
    arg_input: String,
    arg_pattern_column: SelectColumns,
    arg_patterns: String,
    flag_left: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_ignore_case: bool,
    flag_delimiter: Option<Delimiter>,
    flag_prefix_left: Option<String>,
    flag_prefix_right: Option<String>,
    flag_parallel: bool,
    flag_threads: Option<usize>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let inner = !args.flag_left;

    let parallelization = match (args.flag_parallel, args.flag_threads) {
        (true, None) => Some(None),
        (_, Some(count)) => Some(Some(count)),
        _ => None,
    };

    let patterns_rconf = Config::new(&Some(args.arg_patterns.clone()))
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_pattern_column);

    let mut patterns_reader = patterns_rconf.reader()?;
    let mut patterns_headers = patterns_reader.byte_headers()?.clone();
    let pattern_cell_index = patterns_rconf.single_selection(&patterns_headers)?;

    let padding = vec![b""; patterns_headers.len()];

    if let Some(prefix) = &args.flag_prefix_right {
        patterns_headers = prefix_header(&patterns_headers, prefix);
    }

    let rconf = Config::new(&Some(args.arg_input.clone()))
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns);

    let mut reader = rconf.reader()?;
    let mut headers = reader.byte_headers()?.clone();
    let sel = rconf.selection(reader.byte_headers()?)?;

    if let Some(prefix) = &args.flag_prefix_left {
        headers = prefix_header(&headers, prefix);
    }

    let mut writer = Config::new(&args.flag_output).writer()?;

    if !args.flag_no_headers {
        let mut full_headers = csv::ByteRecord::new();
        full_headers.extend(headers.iter());
        full_headers.extend(patterns_headers.iter());

        writer.write_record(&full_headers)?;
    }

    // Indexing the patterns
    let mut patterns: Vec<String> = Vec::new();
    let mut regex_rows: Vec<csv::ByteRecord> = Vec::new();

    for row in patterns_reader.into_byte_records() {
        let row = row?;

        patterns.push(String::from_utf8(row[pattern_cell_index].to_vec()).unwrap());
        regex_rows.push(row);
    }

    let regex_set = regex::bytes::RegexSetBuilder::new(&patterns)
        .case_insensitive(args.flag_ignore_case)
        .build()?;

    // Peforming join
    if let Some(threads) = parallelization {
        reader
            .into_byte_records()
            .parallel_map_custom(
                |o| {
                    if let Some(count) = threads {
                        o.threads(count)
                    } else {
                        o
                    }
                },
                move |record| -> CliResult<Vec<csv::ByteRecord>> {
                    let mut row = record?;

                    let mut rows_to_emit: Vec<csv::ByteRecord> = Vec::new();

                    let mut total_matches: Vec<usize> = Vec::new();

                    for cell in sel.select(&row) {
                        let matches = regex_set.matches(cell).into_iter().collect::<Vec<_>>();

                        if total_matches.is_empty() {
                            total_matches = matches;
                        } else {
                            total_matches = union_of_sorted_lists(&total_matches, &matches);
                        }
                    }

                    for i in total_matches.iter() {
                        let mut row_to_write = row.clone();
                        row_to_write.extend(&regex_rows[*i]);
                        rows_to_emit.push(row_to_write);
                    }

                    if !inner && total_matches.is_empty() {
                        row.extend(&padding);
                        rows_to_emit.push(row);
                    }

                    Ok(rows_to_emit)
                },
            )
            .try_for_each(|result| -> CliResult<()> {
                let rows_to_emit = result?;

                for row in rows_to_emit {
                    writer.write_byte_record(&row)?;
                }

                Ok(())
            })?;
    } else {
        let mut row = csv::ByteRecord::new();

        while reader.read_byte_record(&mut row)? {
            let mut total_matches: Vec<usize> = Vec::new();

            for cell in sel.select(&row) {
                let matches = regex_set.matches(cell).into_iter().collect::<Vec<_>>();

                if total_matches.is_empty() {
                    total_matches = matches;
                } else {
                    total_matches = union_of_sorted_lists(&total_matches, &matches);
                }
            }

            for i in total_matches.iter() {
                let mut row_to_write = row.clone();
                row_to_write.extend(&regex_rows[*i]);
                writer.write_byte_record(&row_to_write)?;
            }

            if !inner && total_matches.is_empty() {
                row.extend(&padding);
                writer.write_byte_record(&row)?;
            }
        }
    }

    Ok(writer.flush()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_union_of_sorted_lists() {
        assert_eq!(
            union_of_sorted_lists(&[1, 2, 3], &[2, 5, 7]),
            vec![1, 2, 3, 5, 7]
        );
    }
}
