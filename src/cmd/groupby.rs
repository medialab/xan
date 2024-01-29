use csv;

use config::{Config, Delimiter};
use select::SelectColumns;
use util;
use CliResult;

use moonblade::GroupAggregationProgram;

use cmd::moonblade::{
    get_moonblade_aggregations_function_help, get_moonblade_cheatsheet,
    get_moonblade_functions_help,
};

static USAGE: &str = "
Group a CSV file by values contained in a given column then aggregate data per
group using a custom aggregation expression.

The result of running the command will be a CSV file containing a \"group\"
column containing the value representing each group and additional columns for
each computed aggregation.

You can, for instance, compute the sum of a column per group:

    $ xsv groupby user_name 'sum(retweet_count)' > groups.csv

You can use dynamic expressions to mangle the data before aggregating it:

    $ xsv groupby user_name 'sum(add(retweet_count, replies_count))' > groups.csv

You can perform multiple aggregations at once:

    $ xsv groupby user_name 'sum(retweet_count), mean(retweet_count), max(replies_count)' > groups.csv

You can rename the output columns using the 'as' syntax:

    $ xsv groupby user_name 'sum(n) as sum, max(replies_count) as \"Max Replies\"' > groups.csv

For a quick review of the capabilities of the script language, use
the --cheatsheet flag.

For a list of available aggregation functions, use the --aggs flag.

If you want to list available functions, use the --functions flag.

Usage:
    xsv groupby [options] <column> <expression> [<input>]
    xsv groupby --help
    xsv groupby --cheatsheet
    xsv groupby --aggs
    xsv groupby --functions

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character. [default: ,]
";

#[derive(Deserialize)]
struct Args {
    arg_column: SelectColumns,
    arg_expression: String,
    arg_input: Option<String>,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_aggs: bool,
    flag_cheatsheet: bool,
    flag_functions: bool,
    flag_group_column : String,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_aggs {
        println!("{}", get_moonblade_aggregations_function_help());
        return Ok(());
    }

    if args.flag_cheatsheet {
        println!("{}", get_moonblade_cheatsheet());
        return Ok(());
    }

    if args.flag_functions {
        println!("{}", get_moonblade_functions_help());
        return Ok(());
    }

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column);

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;
    let headers = rdr.byte_headers()?;

    let column_index = rconf.single_selection(headers)?;

    let mut program = GroupAggregationProgram::parse(&args.arg_expression, headers)?;

    let mut record = csv::ByteRecord::new();

    wtr.write_byte_record(&program.headers(args.flag_group_column))?;

    while rdr.read_byte_record(&mut record)? {
        let group = record[column_index].to_vec();
        program.run_with_record(group, &record)?;
    }

    program.finalize(|output_record| wtr.write_byte_record(output_record))?;

    Ok(wtr.flush()?)
}
