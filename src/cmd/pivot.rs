use crate::config::{Config, Delimiter};
use crate::moonblade::PivotAggregationProgram;
use crate::select::{SelectColumns, Selection};
use crate::util;
use crate::CliResult;

// TODO: optimize when names are known beforehand
// TODO: -S/--sorted
// TODO: multiselections

static USAGE: &str = r#"
Pivot a CSV file by allowing distinct values from a column to be separated into
their own column.

For instance, given the following data:

country,name,year,population
NL,Amsterdam,2000,1005
NL,Amsterdam,2010,1065
NL,Amsterdam,2020,1158
US,Seattle,2000,564
US,Seattle,2010,608
US,Seattle,2020,738
US,New York City,2000,8015
US,New York City,2010,8175
US,New York City,2020,8772

The following command:

    $ xan pivot year 'first(population)' file.csv

Will produce the following result:

country,name,2000,2010,2020
NL,Amsterdam,1005,1065,1158
US,Seattle,564,608,738
US,New York City,8015,8175,8772

By default, rows will be grouped and aggregated together using all columns that
are not the pivoted column nor present in the aggregation clause. If you want
to group rows differently, you can use the -g/--groupby flag instead so that
the following command:

    $ xan pivot year 'sum(population)' -g country file.csv

Will produce:

country,2000,2010,2020
NL,1005,1065,1158
US,564,608,738

Usage:
    xan pivot [-P...] [options] <column> <expr> [<input>]
    xan pivot --help

pivot options:
    -g, --groupby <columns>  Group results by given selection of columns instead
                             of grouping by columns not used to pivot nor in
                             aggregation.

pivotal options:
    -P  Use at least three times to get help from your friends!

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
"#;

#[derive(Deserialize, Debug)]
struct Args {
    arg_input: Option<String>,
    arg_column: SelectColumns,
    arg_expr: String,
    flag_groupby: Option<SelectColumns>,
    #[serde(rename = "flag_P")]
    flag_p: usize,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_p >= 3 {
        println!("{}", include_str!("../moonblade/doc/pivot.txt"));
        return Ok(());
    }

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .delimiter(args.flag_delimiter)
        .select(args.arg_column);

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();
    let pivot_col_index = rconf.single_selection(&headers)?;
    let mut program = PivotAggregationProgram::parse(&args.arg_expr, &headers)?;

    let column_indices_used_in_aggregation = program.used_column_indices();

    if column_indices_used_in_aggregation.contains(&pivot_col_index) {
        Err("aggregation cannot work on the pivot column!")?;
    }

    let mut disappearing_columns = vec![pivot_col_index];
    disappearing_columns.extend(column_indices_used_in_aggregation);

    let groupby_sel = if let Some(cols) = args.flag_groupby.as_ref() {
        let sel = cols.selection(&headers, !args.flag_no_headers)?;

        if sel.iter().any(|i| disappearing_columns.contains(i)) {
            Err(
                "-g/--groupby cannot contain columns used to pivot or used in aggregation clause!",
            )?;
        }

        sel
    } else {
        Selection::without_indices(headers.len(), &disappearing_columns)
    };

    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut index: usize = 0;
    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        let group = groupby_sel.collect(&record);
        let pivot = record[pivot_col_index].to_vec();

        program.run_with_record(group, pivot, index, &record)?;

        index += 1;
    }

    let pivoted_column_names = program.pivoted_column_names();

    if !rconf.no_headers {
        let mut output_headers = groupby_sel.select(&headers).collect::<csv::ByteRecord>();

        for name in pivoted_column_names.iter() {
            output_headers.push_field(name);
        }

        wtr.write_byte_record(&output_headers)?;
    }

    program.flush(&pivoted_column_names, |output_record| -> CliResult<()> {
        wtr.write_byte_record(output_record)?;

        Ok(())
    })?;

    Ok(wtr.flush()?)
}
