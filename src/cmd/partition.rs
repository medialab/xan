use std::fs;
use std::io;
use std::path::Path;

use regex::Regex;

use crate::collections::{hash_map::Entry, HashMap, HashSet};
use crate::config::{Config, Delimiter};
use crate::record::Record;
use crate::select::SelectColumns;
use crate::util::{self, FilenameTemplate};
use crate::CliResult;

static USAGE: &str = "
Partition the given CSV data into chunks based on the values of a column.

The files are written to the output directory with filenames based on the
values in the partition column and the `--filename` flag.

By default, this command will consider it works in a case-insensitive filesystem
(e.g. on macOS). This can have an impact on the names of the create files. If you
know beforehand that your filesystem is case-sensitive and want filenames to be
better aligned with the original values use the -C/--case-sensitive flag.

Note that most operating systems avoid opening more than 1024 files at once,
so if you know the cardinality of the partitioned column is very high, please
sort the file on this column beforehand and use the -S/--sorted flag.

Usage:
    xan partition [options] <column> [<input>]
    xan partition --help

partition options:
    -O, --out-dir <dir>        Where to write the chunks. Defaults to current working
                               directory.
    -f, --filename <filename>  A filename template to use when constructing
                               the names of the output files.  The string '{}'
                               will be replaced by a value based on the value
                               of the field, but sanitized for shell safety.
                               [default: {}.csv]
    -p, --prefix-length <n>    Truncate the partition column after the
                               specified number of bytes when creating the
                               output file.
    -S, --sorted               Use this flag if you know the file is sorted
                               on the partition column in advance, so the command
                               can run faster and with less memory and resources
                               opened.
    --drop                     Drop the partition column from results.
    -C, --case-sensitive       Don't perform case normalization to assess whether a
                               new file has to be created when seeing a new value.
                               Only use on case-sensitive filesystems or this can have
                               adverse effects!

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Otherwise, the first row will
                           appear in all chunks as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Clone, Deserialize)]
struct Args {
    arg_column: SelectColumns,
    arg_input: Option<String>,
    flag_out_dir: Option<String>,
    flag_filename: FilenameTemplate,
    flag_prefix_length: Option<usize>,
    flag_drop: bool,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_sorted: bool,
    flag_case_sensitive: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if let Some(dir) = &args.flag_out_dir {
        fs::create_dir_all(dir)?;
    }

    // It would be nice to support efficient parallel partitions, but doing
    // do would involve more complicated inter-thread communication, with
    // multiple readers and writers, and some way of passing buffers
    // between them.
    args.sequential_partition()
}

impl Args {
    /// Configuration for our reader.
    fn rconfig(&self) -> Config {
        Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.arg_column.clone())
    }

    /// Get the column to use as a key.
    fn key_column(&self, rconfig: &Config, headers: &csv::ByteRecord) -> CliResult<usize> {
        let select_cols = rconfig.selection(headers)?;
        if select_cols.len() == 1 {
            Ok(select_cols[0])
        } else {
            Err("can only partition on one column")?
        }
    }

    /// A basic sequential partition.
    fn sequential_partition(&self) -> CliResult<()> {
        let rconfig = self.rconfig();
        let mut rdr = rconfig.reader()?;
        let mut headers = rdr.byte_headers()?.clone();
        let key_col = self.key_column(&rconfig, &headers)?;
        let mut generator =
            WriterGenerator::new(self.flag_filename.clone(), self.flag_case_sensitive);
        let out_dir = match &self.flag_out_dir {
            Some(dir) => Path::new(dir),
            None => Path::new(""),
        };

        if self.flag_drop {
            headers = headers.remove(key_col);
        }

        let mut row = csv::ByteRecord::new();

        if self.flag_sorted {
            let mut current: Option<(Vec<u8>, BoxedWriter)> = None;

            while rdr.read_byte_record(&mut row)? {
                // Decide what file to put this in.
                let column = &row[key_col];
                let key = match self.flag_prefix_length {
                    // We exceed --prefix-length, so ignore the extra bytes.
                    Some(len) if len < column.len() => &column[0..len],
                    _ => column,
                };

                match current {
                    Some((ref k, _)) if k == key => {}
                    _ => {
                        let mut wtr = generator.writer(out_dir, key)?;

                        if !rconfig.no_headers {
                            wtr.write_record(&headers)?;
                        }

                        current = Some((key.to_vec(), wtr));
                    }
                };

                let wtr = &mut current.as_mut().unwrap().1;

                if self.flag_drop {
                    wtr.write_record(&row.remove(key_col))?;
                } else {
                    wtr.write_byte_record(&row)?;
                }
            }
        } else {
            let mut writers: HashMap<Vec<u8>, BoxedWriter> = HashMap::new();

            while rdr.read_byte_record(&mut row)? {
                // Decide what file to put this in.
                let column = &row[key_col];
                let key = match self.flag_prefix_length {
                    // We exceed --prefix-length, so ignore the extra bytes.
                    Some(len) if len < column.len() => &column[0..len],
                    _ => column,
                };

                let mut entry = writers.entry(key.to_vec());
                let wtr = match entry {
                    Entry::Occupied(ref mut occupied) => occupied.get_mut(),
                    Entry::Vacant(vacant) => {
                        // We have a new key, so make a new writer.
                        let mut wtr = generator.writer(out_dir, key)?;
                        if !rconfig.no_headers {
                            wtr.write_record(&headers)?;
                        }
                        vacant.insert(wtr)
                    }
                };

                if self.flag_drop {
                    wtr.write_record(&row.remove(key_col))?;
                } else {
                    wtr.write_byte_record(&row)?;
                }
            }
        }

        Ok(())
    }
}

type BoxedWriter = csv::Writer<Box<dyn io::Write + 'static>>;

/// Generates unique filenames based on CSV values.
struct WriterGenerator {
    template: FilenameTemplate,
    counter: usize,
    used: HashSet<String>,
    non_word_char: Regex,
    case_sensitive: bool,
}

impl WriterGenerator {
    fn new(template: FilenameTemplate, case_sensitive: bool) -> WriterGenerator {
        WriterGenerator {
            template,
            counter: 1,
            used: HashSet::new(),
            non_word_char: Regex::new(r"[^\w_.\-]").unwrap(),
            case_sensitive,
        }
    }

    /// Create a CSV writer for `key`.  Does not add headers.
    fn writer<P>(&mut self, path: P, key: &[u8]) -> io::Result<BoxedWriter>
    where
        P: AsRef<Path>,
    {
        let unique_value = self.unique_value(key);
        self.template.writer(path.as_ref(), &unique_value)
    }

    /// Generate a unique value for `key`, suitable for use in a
    /// "shell-safe" filename.  If you pass `key` twice, you'll get two
    /// different values.
    fn unique_value(&mut self, key: &[u8]) -> String {
        // Sanitize our key.
        let utf8 = String::from_utf8_lossy(key);
        let mut safe = self.non_word_char.replace_all(&utf8, "").into_owned();
        safe = if safe.is_empty() {
            "empty".to_owned()
        } else {
            safe
        };

        let mut base = safe.clone();

        if !self.case_sensitive {
            base = base.to_lowercase();
        }

        // Now check for collisions.
        if !self.used.contains(&base) {
            self.used.insert(base.clone());
            safe
        } else {
            loop {
                let candidate = format!("{}_{}", &safe, self.counter);
                self.counter = self.counter.checked_add(1).unwrap_or_else(|| {
                    // We'll run out of other things long before we ever
                    // reach this, but we'll check just for correctness and
                    // completeness.
                    panic!("Cannot generate unique value")
                });
                if !self.used.contains(&candidate) {
                    self.used.insert(candidate.clone());
                    return candidate;
                }
            }
        }
    }
}
