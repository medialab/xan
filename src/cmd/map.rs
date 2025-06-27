use pariter::IteratorExt;

use crate::config::{Config, Delimiter};
use crate::moonblade::SelectionProgram;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
The map command evaluates an expression for each row of the given CSV file and
output the same row with added columns containing the results of beforementioned
expression.

For instance, given the following CSV file:

a,b
1,4
5,2

The following command:

    $ xan map 'a + b as c' file.csv > result.csv

Will produce the following result:

a,b,c
1,4,5
5,2,7

You can also create multiple columns at once:

    $ xan map 'a + b as c, a * b as d' file.csv > result.csv

Will produce the following result:

a,b,c,d
1,4,5,4
5,2,7,10

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

If you want to create multiple columns in a single pass, take a look
at `xan select --append --evaluate` instead.

Miscellaneous tricks:

1. Copying a column:

    $ xan map 'column_name as copy_name' file.csv > result.csv

2. Create a column containing a constant value:

    $ xan map '"john" as from' file.csv > result.csv

Usage:
    xan map [options] <expression> [<input>]
    xan map --help

map options:
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
    arg_expression: String,
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_parallel: bool,
    flag_threads: Option<usize>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
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

    let program = SelectionProgram::parse(&args.arg_expression, &headers)?;

    if !args.flag_no_headers {
        wtr.write_record(headers.iter().chain(program.headers()))?;
    }

    if let Some(threads) = parallelization {
        for result in rdr.into_byte_records().enumerate().parallel_map_custom(
            |o| o.threads(threads.unwrap_or_else(num_cpus::get)),
            move |(index, record)| -> CliResult<csv::ByteRecord> {
                let mut record = record?;

                program.mutate_record(index, &mut record)?;

                Ok(record)
            },
        ) {
            wtr.write_byte_record(&result?)?;
        }
    } else {
        let mut record = csv::ByteRecord::new();
        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            program.mutate_record(index, &mut record)?;

            wtr.write_byte_record(&record)?;

            index += 1;
        }
    }

    Ok(wtr.flush()?)
}
