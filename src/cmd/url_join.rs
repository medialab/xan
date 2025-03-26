use std::str;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::urls::LRUTrieMultiMap;
use crate::util;
use crate::CliResult;

fn prefix_header(headers: &csv::ByteRecord, prefix: &String) -> csv::ByteRecord {
    let mut prefixed_headers = csv::ByteRecord::new();

    for column in headers.iter() {
        prefixed_headers.push_field(&[prefix.as_bytes(), column].concat());
    }

    prefixed_headers
}

static USAGE: &str = "
Join a CSV file containing a column of url prefixes with another CSV file.

The default behavior of this command is to be an 'inner join', which
means only matched rows will be written in the output. Use the --left
flag if you want to perform a 'left join' and keep every row of the searched
file in the output.

The file containing urls will always be completely read in memory
while the second one will always be streamed.

You can of course work on gzipped files if needed and feed one of both
files from stdin by using `-` instead of a path.

Not that this command indexes the hierarchical reordering of a bunch of urls
into a prefix tree. This reordering scheme is named LRUs and you can read about
it here: https://github.com/medialab/ural#about-lrus

If you only need to filter rows of the second file and don't
actually need to join columns from the urls file, you should
probably use `xan search --url-prefix --patterns` instead.

Usage:
    xan url-join [options] <column> <input> <url-column> <urls>
    xan url-join --help

join options:
    --left                       Write every row from input file in the output, with empty
                                 padding cells on the right when no url from the second
                                 file produced any match.
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
    arg_column: SelectColumns,
    arg_input: String,
    arg_url_column: SelectColumns,
    arg_urls: String,
    flag_left: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_prefix_left: Option<String>,
    flag_prefix_right: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let inner = !args.flag_left;

    let urls_rconf = Config::new(&Some(args.arg_urls.clone()))
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_url_column);

    let mut urls_reader = urls_rconf.reader()?;
    let mut urls_headers = urls_reader.byte_headers()?.clone();
    let url_cell_index = urls_rconf.single_selection(&urls_headers)?;

    let padding = vec![b""; urls_headers.len()];

    if let Some(prefix) = &args.flag_prefix_right {
        urls_headers = prefix_header(&urls_headers, prefix);
    }

    let rconf = Config::new(&Some(args.arg_input.clone()))
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column);

    let mut reader = rconf.reader()?;
    let mut headers = reader.byte_headers()?.clone();
    let pos = rconf.single_selection(reader.byte_headers()?)?;

    if let Some(prefix) = &args.flag_prefix_left {
        headers = prefix_header(&headers, prefix);
    }

    let mut writer = Config::new(&args.flag_output).writer()?;

    if !args.flag_no_headers {
        let mut full_headers = csv::ByteRecord::new();
        full_headers.extend(headers.iter());
        full_headers.extend(urls_headers.iter());

        writer.write_record(&full_headers)?;
    }

    // Indexing the urls
    let mut trie: LRUTrieMultiMap<csv::ByteRecord> = LRUTrieMultiMap::new();

    for result in urls_reader.into_byte_records() {
        let record = result?;
        let url =
            String::from_utf8(record[url_cell_index].to_vec()).expect("invalid utf-8 encoding");

        trie.insert(&url, record)?;
    }

    // Peforming join
    let mut record = csv::ByteRecord::new();

    while reader.read_byte_record(&mut record)? {
        let url = std::str::from_utf8(&record[pos]).expect("invalid utf-8 encoding");

        if let Ok(matches) = trie.longest_matching_prefix_values(url) {
            if !inner && !matches.has_next() {
                record.extend(&padding);
                writer.write_byte_record(&record)?;
                continue;
            }

            for matched_record in matches {
                let mut record_to_write = record.clone();
                record_to_write.extend(matched_record);
                writer.write_byte_record(&record_to_write)?;
            }
        }
    }

    Ok(writer.flush()?)
}
