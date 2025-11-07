use std::fs;

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

Expression clauses can also return more than one item at once to avoid repeating
computations, for instance:

Splitting a full name:

    $ xan map 'full_name.split(" ") as (first_name, last_name)' file.csv > result.csv

Extracting data from a JSON cell:

    $ xan map 'data.parse_json() | [_.name, _.meta[2].age] as (name, age)' file.csv > result.csv

You can also use the -O/--overwrite flag to overwrite already existing columns:

    $ xan map -O 'b * 10 as b, a * b as c' file.csv > result.csv

Will produce:

a,b,c
1,40,4
5,20,10

The expression can optionally be read from a file using the -f/--evaluate-file flag:

    $ xan map -f expr.moonblade file.csv > result.csv

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Miscellaneous tricks:

1. Copying a column:

    $ xan map 'column_name as copy_name' file.csv > result.csv

2. Create a column containing a constant value:

    $ xan map '"john" as from' file.csv > result.csv

Usage:
    xan map [options] <expression> [<input>]
    xan map --help

map options:
    -f, --evaluate-file        Read evaluation expression from a file instead.
    -O, --overwrite            If set, expressions named with a column already existing
                               in the file will be overwritten with the result of the
                               expression instead of adding a new column at the end.
                               This means you can both transform and add columns at the
                               same time.
    -F, --filter               If given, will not write rows in the output if all results
                               of evaluated expression are falsey.
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
    flag_filter: bool,
    flag_parallel: bool,
    flag_threads: Option<usize>,
    flag_evaluate_file: bool,
    flag_overwrite: bool,
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

    let parallelization = match (args.flag_parallel, args.flag_threads) {
        (true, None) => Some(None),
        (_, Some(count)) => Some(Some(count)),
        _ => None,
    };

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let mut rdr = rconf.simd_reader()?;
    let headers = rdr.byte_headers()?.clone();

    let program = SelectionProgram::parse(&args.arg_expression, &headers)?;

    if args.flag_overwrite && program.has_any_plural_expr() {
        Err("-O/--overwrite does not work with clauses yielding multiple columns yet!")?;
    }

    let actually_overwriting = args.flag_overwrite && program.has_something_to_overwrite();

    if !rconf.no_headers {
        if actually_overwriting {
            wtr.write_record(headers.iter().chain(program.new_headers()))?;
        } else {
            wtr.write_record(headers.iter().chain(program.headers()))?;
        }
    }

    if let Some(threads) = parallelization {
        for result in rdr.into_byte_records().enumerate().parallel_map_custom(
            |o| o.threads(threads.unwrap_or_else(crate::util::default_num_cpus)),
            move |(index, record)| -> CliResult<(bool, simd_csv::ByteRecord)> {
                let mut record = record?;

                let is_truthy;

                if actually_overwriting {
                    (is_truthy, record) = program.overwrite(index, &mut record)?;
                } else {
                    is_truthy = program.extend(index, &mut record)?;
                }

                Ok((is_truthy, record))
            },
        ) {
            let (is_truthy, record) = result?;

            if !args.flag_filter || is_truthy {
                wtr.write_byte_record(&record)?;
            }
        }
    } else {
        let mut record = simd_csv::ByteRecord::new();
        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let is_truthy;

            if actually_overwriting {
                (is_truthy, record) = program.overwrite(index, &mut record)?;
            } else {
                is_truthy = program.extend(index, &mut record)?;
            }

            if !args.flag_filter || is_truthy {
                wtr.write_byte_record(&record)?;
            }

            index += 1;
        }
    }

    Ok(wtr.flush()?)
}
