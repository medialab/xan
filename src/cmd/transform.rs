use pariter::IteratorExt;

use crate::config::{Config, Delimiter};
use crate::moonblade::Program;
use crate::select::SelectColumns;
use crate::util::{self, ImmutableRecordHelpers};
use crate::CliResult;

static USAGE: &str = r#"
The transform command evaluates an expression for each row of the given CSV file
and use the result to edit a target column that can optionally be renamed.

For instance, given the following CSV file:

name,surname
john,davis
mary,sue

The following command:

    $ xan transform surname 'upper(surname)'

Will produce the following result:

name,surname
john,DAVIS
mary,SUE

Note that the given expression will be given the target column as its implicit
value, which means that the latter command can also be written as:

    $ xan transform surname 'upper(_)'

Or even shorter:

    $ xan transfrom surname upper

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Usage:
    xan transform [options] <column> <expression> [<input>]
    xan transform --help

transform options:
    -r, --rename <name>        New name for the transformed column.
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
    arg_column: SelectColumns,
    arg_expression: String,
    arg_input: Option<String>,
    flag_rename: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_parallel: bool,
    flag_threads: Option<usize>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .delimiter(args.flag_delimiter)
        .select(args.arg_column);

    let parallelization = match (args.flag_parallel, args.flag_threads) {
        (true, None) => Some(None),
        (_, Some(count)) => Some(Some(count)),
        _ => None,
    };

    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    let transformed_index = rconf.single_selection(&headers)?;

    args.arg_expression = format!("col({}) | {}", transformed_index, &args.arg_expression);

    let program = Program::parse(&args.arg_expression, &headers)?;

    if !args.flag_no_headers {
        let output_headers = if let Some(new_name) = &args.flag_rename {
            headers.replace_at(transformed_index, new_name.as_bytes())
        } else {
            headers.clone()
        };

        wtr.write_record(&output_headers)?;
    }

    if let Some(threads) = parallelization {
        for result in rdr.into_byte_records().enumerate().parallel_map_custom(
            |o| o.threads(threads.unwrap_or_else(num_cpus::get)),
            move |(index, record)| -> CliResult<csv::ByteRecord> {
                let record = record?;

                let value = program.run_with_record(index, &record)?;

                let record = record.replace_at(transformed_index, &value.serialize_as_bytes());

                Ok(record)
            },
        ) {
            wtr.write_byte_record(&result?)?;
        }
    } else {
        let mut record = csv::ByteRecord::new();
        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let value = program.run_with_record(index, &record)?;

            wtr.write_byte_record(
                &record.replace_at(transformed_index, &value.serialize_as_bytes()),
            )?;

            index += 1;
        }
    }

    Ok(wtr.flush()?)
}
