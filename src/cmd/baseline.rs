use regex::bytes::RegexBuilder;
use std::str::from_utf8;

use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Annotate numeric columns with a comparison to a baseline row.

Finds a row matching the given pattern and treats it as the baseline.
For every other row, numeric cells are formatted as \"value (±N%)\"
showing the relative difference from the corresponding baseline cell.
The baseline row itself shows plain values.

Non-numeric cells are passed through unchanged.

Usage:
    xan baseline [options] <pattern> [<input>]
    xan baseline --help

baseline options:
    -s, --select <cols>    Column(s) to search for the baseline pattern.
                           Defaults to the first column.
    -c, --compare <cols>   Only compare these columns. By default, all
                           columns with numeric values in the baseline
                           row are compared.
    -i, --ignore-case      Case-insensitive pattern matching.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_pattern: String,
    arg_input: Option<String>,
    flag_select: Option<SelectedColumns>,
    flag_compare: Option<SelectedColumns>,
    flag_ignore_case: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

fn try_parse_f64(s: &[u8]) -> Option<f64> {
    from_utf8(s).ok().and_then(|s| s.trim().parse::<f64>().ok())
}

fn format_comparison(value: f64, baseline: f64) -> String {
    if value == baseline {
        if value == value.round() {
            return format!("{} (=)", value as i64);
        }
        return format!("{} (=)", value);
    }
    if baseline == 0.0 {
        let sign = if value > 0.0 { "+" } else { "-" };
        if value == value.round() {
            return format!("{} ({}inf%)", value as i64, sign);
        }
        return format!("{} ({}inf%)", value, sign);
    }
    let pct = (value - baseline) / baseline * 100.0;
    let sign = if pct >= 0.0 { "+" } else { "" };
    if value == value.round() {
        return format!("{} ({}{:.0}%)", value as i64, sign, pct);
    }
    format!("{} ({}{:.0}%)", value, sign, pct)
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut reader = rconfig.simd_reader()?;
    let headers = reader.byte_headers()?.clone();

    // determine which columns to search for the pattern
    let search_sel: Vec<usize> = if let Some(ref sel) = args.flag_select {
        Config::new(&args.arg_input)
            .delimiter(args.flag_delimiter)
            .no_headers(args.flag_no_headers)
            .select(sel.clone())
            .selection(&headers)?
            .to_vec()
    } else {
        // default: first column
        vec![0]
    };

    // read all records
    let records: Vec<_> = reader.byte_records().collect::<Result<Vec<_>, _>>()?;

    // find baseline row
    let pattern = RegexBuilder::new(&args.arg_pattern)
        .case_insensitive(args.flag_ignore_case)
        .unicode(false)
        .build()?;

    let mut baseline_idx: Option<usize> = None;
    for (i, record) in records.iter().enumerate() {
        for &col in &search_sel {
            if col < record.len() && pattern.is_match(&record[col]) {
                baseline_idx = Some(i);
                break;
            }
        }
        if baseline_idx.is_some() {
            break;
        }
    }

    let baseline_idx = match baseline_idx {
        Some(i) => i,
        None => {
            return Err(format!(
                "No row matching pattern '{}' found",
                args.arg_pattern
            )
            .into());
        }
    };

    let baseline_record = &records[baseline_idx];

    // determine which columns to compare
    let compare_cols: Vec<usize> = if let Some(ref sel) = args.flag_compare {
        Config::new(&args.arg_input)
            .delimiter(args.flag_delimiter)
            .no_headers(args.flag_no_headers)
            .select(sel.clone())
            .selection(&headers)?
            .to_vec()
    } else {
        // auto-detect: all columns where baseline has a numeric value
        (0..baseline_record.len())
            .filter(|&i| try_parse_f64(&baseline_record[i]).is_some())
            .collect()
    };

    // parse baseline values for compare columns
    let mut baseline_vals: Vec<Option<f64>> = vec![None; headers.len()];
    for &col in &compare_cols {
        if col < baseline_record.len() {
            baseline_vals[col] = try_parse_f64(&baseline_record[col]);
        }
    }

    // write output
    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    if !rconfig.no_headers {
        wtr.write_byte_record(&headers)?;
    }

    let mut out_record = simd_csv::ByteRecord::new();

    for (i, record) in records.iter().enumerate() {
        out_record.clear();

        for col in 0..record.len() {
            if i == baseline_idx || !compare_cols.contains(&col) {
                // baseline row or non-compared column: pass through
                out_record.push_field(&record[col]);
            } else if let (Some(val), Some(base)) =
                (try_parse_f64(&record[col]), baseline_vals[col])
            {
                let formatted = format_comparison(val, base);
                out_record.push_field(formatted.as_bytes());
            } else {
                // non-numeric: pass through
                out_record.push_field(&record[col]);
            }
        }

        wtr.write_byte_record(&out_record)?;
    }

    Ok(wtr.flush()?)
}
