use pariter::IteratorExt;

use crate::config::{Config, Delimiter};
use crate::moonblade::Program;
use crate::select::SelectColumns;
use crate::util::{self, ImmutableRecordHelpers};
use crate::CliResult;

static USAGE: &str = r#"
The flatmap command evaluates an expression for each row of the given CSV file.
This expression is expected to return a potentially iterable value (e.g. a list).

If said value is falsey, then no row will be written in the output of the input
row.

Then, for each nested value yielded by the expression, one row of CSV will be
written to the output.

This row will have the same columns as the input with an additional one
containing the nested value or replacing the value of a column of your choice,
using the -r/--replace flag.

For instance, given the following CSV file:

name,colors
John,blue
Mary,yellow|red

The following command:

    $ xan flatmap 'split(colors, "|")' color -r colors

Will produce the following result:

name,color
John,blue
Mary,yellow
Mary,red

Note that this example is voluntarily simplistic and you should probably rely on
the `explode` command instead, if what you need is just to split cells by a
separator.

Also, when using the -r/--replace flag, the given expression will be considered
as a pipe being fed with target column so you can use `_` as a convenience. This
means the above command can be rewritten thusly:

    $ xan flatmap 'split(_, "|")' color -r colors

Finally, if the expression returns an empty list or a falsey value, no row will
be written in the output for the current input row. This means one can use the
`flatmap` command as a sort of `filter` + `map` in a single pass over the CSV file.

For instance, given the following CSV file:

name,age
John Mayer,34
Mary Sue,45

The following command:

    $ xan flatmap 'if(age >= 40, last(split(name, " ")))' surname

Will produce the following result:

name,age,surname
Mary Sue,45,Sue

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Usage:
    xan flatmap [options] <expression> <column> [<input>]
    xan flatmap --help

flatmap options:
    -r, --replace <column>     Name of the column that will be replaced by the mapped values.
    -p, --parallel             Whether to use parallelization to speed up computations.
                               Will automatically select a suitable number of threads to use
                               based on your number of cores. Use -t, --threads if you want to
                               indicate the number of threads yourself.
    -t, --threads <threads>    Parellize computations using this many threads. Use -p, --parallel
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
    arg_column: String,
    arg_expression: String,
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_parallel: bool,
    flag_threads: Option<usize>,
    flag_replace: Option<SelectColumns>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .delimiter(args.flag_delimiter);

    let parallelization = match (args.flag_parallel, args.flag_threads) {
        (true, None) => Some(None),
        (_, Some(count)) => Some(Some(count)),
        _ => None,
    };

    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    let replaced_index_opt = args
        .flag_replace
        .as_ref()
        .map(|s| s.single_selection(&headers, !args.flag_no_headers))
        .transpose()?;

    if let Some(i) = replaced_index_opt {
        args.arg_expression = format!("col({}) | {}", i, &args.arg_expression);
    }

    let program = Program::parse(&args.arg_expression, &headers)?;

    if !args.flag_no_headers {
        let output_headers = if let Some(i) = replaced_index_opt {
            headers.replace_at(i, args.arg_column.as_bytes())
        } else {
            let mut h = headers.clone();
            h.push_field(args.arg_column.as_bytes());
            h
        };

        wtr.write_record(&output_headers)?;
    }

    if let Some(threads) = parallelization {
        for records in rdr.into_byte_records().enumerate().parallel_map_custom(
            |o| o.threads(threads.unwrap_or_else(num_cpus::get)),
            move |(index, record)| -> CliResult<Vec<csv::ByteRecord>> {
                let record = record?;

                let mut output = Vec::new();

                let values = program.run_with_record(index, &record)?;

                for value in values.flat_iter() {
                    if value.is_falsey() {
                        continue;
                    }

                    output.push(if let Some(i) = replaced_index_opt {
                        record.replace_at(i, &value.serialize_as_bytes())
                    } else {
                        record.append(&value.serialize_as_bytes())
                    });
                }

                Ok(output)
            },
        ) {
            for record in records? {
                wtr.write_byte_record(&record)?;
            }
        }
    } else {
        let mut record = csv::ByteRecord::new();
        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let values = program.run_with_record(index, &record)?;

            for value in values.flat_iter() {
                if value.is_falsey() {
                    continue;
                }

                let output_record = if let Some(i) = replaced_index_opt {
                    record.replace_at(i, &value.serialize_as_bytes())
                } else {
                    record.append(&value.serialize_as_bytes())
                };

                wtr.write_byte_record(&output_record)?;
            }

            index += 1;
        }
    }

    Ok(wtr.flush()?)
}
