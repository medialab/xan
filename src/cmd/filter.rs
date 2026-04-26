use std::fs;
use std::num::NonZeroUsize;

use pariter::IteratorExt;
use simd_csv::ByteRecord;

use crate::collections::ContextBuffer;
use crate::config::{Config, Delimiter};
use crate::moonblade::Program;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
The filter command evaluates an expression for each row of the given CSV file and
only output the row if the result of beforementioned expression is truthy.

For instance, given the following CSV file:

a
1
2
3

The following command:

    $ xan filter 'a > 1'

Will produce the following result:

a
2
3

The expression can optionally be read from a file using the -f/--evaluate-file flag:

    $ xan filter -f expr.moonblade file.csv > result.csv

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Usage:
    xan filter [options] <expression> [<input>]
    xan filter --help

filter options:
    -f, --evaluate-file       Read evaluation expression from a file instead.
    -v, --invert-match        If set, will invert the evaluated value.
    -l, --limit <n>           Maximum number of rows to return. Useful to avoid downstream
                              buffering some times (e.g. when searching for very few
                              rows in a big file before piping to `view` or `flatten`).
    -B, --before-context <n>  Number of rows to keep before a matching one.
    -A, --after-context <n>   Number of rows to keep after a matching one.
    -p, --parallel            Whether to use parallelization to speed up computations.
                              Will automatically select a suitable number of threads to use
                              based on your number of cores. Use -t, --threads if you want to
                              indicate the number of threads yourself.
    -t, --threads <threads>   Parellize computations using this many threads. Use -p, --parallel
                              if you want the number of threads to be automatically chosen instead.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
"#;

#[derive(Deserialize)]
struct Args {
    arg_expression: String,
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_parallel: bool,
    flag_limit: Option<usize>,
    flag_before_context: Option<NonZeroUsize>,
    flag_after_context: Option<NonZeroUsize>,
    flag_threads: Option<usize>,
    flag_invert_match: bool,
    flag_evaluate_file: bool,
}

impl Args {
    fn resolve(&mut self) -> CliResult<()> {
        if self.flag_evaluate_file {
            self.arg_expression = fs::read_to_string(&self.arg_expression)?;
        }

        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve()?;

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .delimiter(args.flag_delimiter);

    let threads = util::parallelization(args.flag_parallel, args.flag_threads);

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let mut rdr = rconf.simd_reader()?;
    let headers = rdr.byte_headers()?.clone();

    if !rconf.no_headers {
        wtr.write_byte_record(&headers)?;
    }

    let mut context_buffer = ContextBuffer::new(args.flag_before_context, args.flag_after_context);

    let program = Program::parse(&args.arg_expression, &headers, rconf.no_headers)?;
    let mut matches: usize = 0;

    if let Some(t) = threads {
        for result in rdr.into_byte_records().enumerate().parallel_map_custom(
            |o| o.threads(t),
            move |(index, record)| -> CliResult<(bool, ByteRecord)> {
                let record = record?;

                let value = program.run_with_record(index, &record)?;

                let mut is_match = value.is_truthy();

                if args.flag_invert_match {
                    is_match = !is_match;
                }

                Ok((is_match, record))
            },
        ) {
            let (is_match, record) = result?;

            if is_match {
                matches += 1;
            }

            context_buffer.try_process_owned(is_match, record, |r| wtr.write_byte_record(r))?;

            if let Some(limit) = args.flag_limit {
                if matches >= limit {
                    break;
                }
            }
        }
    } else {
        let mut record = ByteRecord::new();
        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let value = program.run_with_record(index, &record)?;

            let mut is_match = value.is_truthy();

            if args.flag_invert_match {
                is_match = !is_match;
            }

            if is_match {
                matches += 1;
            }

            context_buffer.try_process(is_match, &record, |r| wtr.write_byte_record(r))?;

            if let Some(limit) = args.flag_limit {
                if matches >= limit {
                    break;
                }
            }

            index += 1;
        }
    }

    Ok(wtr.flush()?)
}
