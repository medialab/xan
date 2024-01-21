use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::sync::Arc;

use csv;
use pariter::IteratorExt;
use thread_local::ThreadLocal;

use config::{Config, Delimiter};
use moonblade::{DynamicValue, EvaluationError, PipelineProgram};
use select::SelectColumns;
use util::ImmutableRecordHelpers;
use CliError;
use CliResult;

pub fn get_moonblade_cheatsheet() -> &'static str {
    "
xsv script language cheatsheet (use --functions for comprehensive list of
available functions):

  . Indexing a column by name:
        'trim(col)'

  . Indexing a column by name even with spaces:
        'trim(Name of film)'

  . Indexing column with characters forbidden in identifies (e.g. commas):
        'trim(col(\"Name, of film\"))'

  . Indexing column by index (0-based):
        'trim(col(2))'

  . Indexing a column by name and 0-based nth (for duplicate headers):
        'trim(col(\"col\", 1))'

  . Integer literals:
        'add(1, count)'

  . Boolean literals (true or false):
        'coalesce(count, true)'

  . Null literals:
        'coalesce(null, count)'

  . Float literals:
        'mul(0.5, count)'

  . String literals (can use single or double quotes):
        'concat(name, \"-\", surname)'

  . Regex literals:
        'match(name, /john/)'

  . Case-insensitive regex literals:
        'match(name, /john/i)'

  . Accessing current row index:
        'add(%index, 1)'

  . Nesting function calls:
        'add(sub(col1, col2), mul(col3, col4))'

  . Basic branching (also consider using the \"coalesce\" function for simple cases):
        'if(lt(count, 4), trim(name), trim(surname))'

  . Piping (underscore \"_\" becomes a reference to previous result):
        'trim(name) | lower(_) | add(count, len(_))'

        is the same as:

        'add(count, len(lower(trim(name))))'

  . Piping shorthand for unary functions:
        'trim(name) | lower'

        is the same as:

        'trim(name) | lower(_)'

Misc notes:

  . This is a minimal interpreted language with dynamic typing,
    which means functions will usually cast values around to
    make them fit expectations. Use the `typeof` function if
    you feel lost.
"
}

pub fn get_moonblade_functions_help() -> &'static str {
    "
# Available functions

(use --cheatsheet for a reminder of how the scripting language works)

## Arithmetics

    - abs(x) -> number
        Return absolute value of number.

    - add(x, y) -> number
        Add two numbers.

    - dec(x) -> number
        Decrement x, subtracting 1.

    - div(x, y) -> number
        Divide two numbers.

    - idiv(x, y) -> number
        Integer division of two numbers.

    - inc(x) -> number
        Increment x, adding 1.

    - mul(x, y) -> number
        Multiply x & y.

    - neg(x) -> Number
        Return -x.

    - sub(x, y) -> number
        Subtract x & y.

## Boolean operations & branching

    - and(a, b) -> bool
        Perform boolean AND operation.

    - if(cond, then, else?) -> T
        Evaluate condition and switch to correct branch.

    - unless(cond, then, else?) -> T
        Shorthand for `if(not(cond), then, else?)`.

    - not(a) -> bool
        Perform boolean NOT operation.

    - or(a, b) -> bool
        Perform boolean OR operation.

## Comparison

    - eq(x, y) -> bool
        Test numerical equality.

    - gt(x, y) -> bool
        Test numerical x > y.

    - gte(x, y) -> bool
        Test numerical x >= y.

    - lt(x, y)
        Test numerical x < y.

    - lte(x, y)
        Test numerical x > y.

    - neq(x, y) -> bool
        Test numerical x != y.

    - s_eq(s1, s2) -> bool
        Test sequence equality.

    - s_gt(s1, s2) -> bool
        Test sequence s1 > s2.

    - s_gte(s1, s2) -> bool
        Test sequence s1 >= s2.

    - s_lt(s1, s2) -> bool
        Test sequence s1 < s2.

    - s_gte(s1, s2) -> bool
        Test sequence s1 <= s2.

    - s_neq(s1, s2) -> bool
        Test sequence s1 != s2.

## String & sequence helpers

    - compact(list) -> list
        Drop all falsey values from given list.

    - concat(string, *strings) -> string
        Concatenate given strings into a single one.

    - contains(seq, subseq) -> bool
        Find if subseq can be found in seq.

    - count(seq, pattern) -> int
        Count number of times pattern appear in seq.

    - endswith(string, pattern) -> bool
        Test if string ends with pattern.

    - first(seq) -> T
        Get first element of sequence.

    - get(seq, index) -> T
        Get nth element of sequence (can use negative indexing).

    - join(seq, sep) -> string
        Join sequence by separator.

    - last(seq) -> T
        Get last element of sequence.

    - len(seq) -> int
        Get length of sequence.

    - ltrim(string, pattern?) -> string
        Trim string of leading whitespace or
        provided characters.

    - lower(string) -> string
        Lowercase string.

    - match(string, regex) -> bool
        Return whether regex pattern matches string.

    - replace(string, pattern, replacement) -> string
        Replace pattern in string. Can use a regex.

    - rtrim(string, pattern?) -> string
        Trim string of trailing whitespace or
        provided characters.

    - slice(seq, start, end?) -> seq
        Return slice of sequence.

    - split(string, sep, max?) -> list
        Split a string by separator.

    - startswith(string, pattern) -> bool
        Test if string starts with pattern.

    - trim(string, pattern?) -> string
        Trim string of leading & trailing whitespace or
        provided characters.

    - unidecode(string) -> string
        Convert string to ascii as well as possible.

    - upper(string) -> string
        Uppercase string.

## Utils

    - coalesce(*args) -> T
        Return first truthy value.

    - col(name_or_pos, nth?) -> string
        Return value of cell for given column, by name, by position or by
        name & nth, in case of duplicate header names.

    - err(msg) -> error
        Make the expression return a custom error.

    - typeof(value) -> string
        Return type of value.

    - val(value) -> T
        Return a value as-is. Useful to return constants.

## IO & path wrangling

    - abspath(string) -> string
        Return absolute & canonicalized path.

    - filesize(string) -> int
        Return the size of given file in bytes.

    - isfile(string) -> bool
        Return whether the given path is an existing file on disk.

    - pathjoin(string, *strings) -> string
        Join multiple paths correctly.

    - read(path, encoding?, errors?) -> string
        Read file at path. Default encoding is \"utf-8\".
        Default error handling policy is \"replace\", and can be
        one of \"replace\", \"ignore\" or \"strict\".

## Random

    - uuid() -> string
        Return a uuid v4.

"
}

pub fn get_moonblade_aggregations_function_help() -> &'static str {
    "
# Available aggregation functions

(use --cheatsheet for a reminder of how the scripting language works)

    - avg(<expr>) -> number
        Average of numerical values. Same as `mean`.

    - count(<expr>?) -> number
        Count the numbers of row. Works like in SQL in that `count(<expr>)`
        will count all non-null values returned by given expression, while
        `count()` without any expression will count every matching row.

    - max(<expr>) -> number | string
        Maximum value.

    - mean(<expr>) -> number
        Mean of numerical values. Same as `avg`.

    - median(<expr>) -> number
        Median of numerical values, interpolating on even counts.

    - median_high(<expr>) -> number
        Median of numerical values, returning higher value on even counts.

    - median_low(<expr>) -> number
        Median of numerical values, returning lower value on even counts.

    - min(<expr>) -> number | string
        Minimum value.

    - stddev(<expr>) -> number
        Population standard deviation. Same as `stddev_pop`.

    - stddev_pop(<expr>) -> number
        Population standard deviation. Same as `stddev`.

    - stddev_sample(<expr>) -> number
        Sample standard deviation (i.e. using Bessel's correction).

    - sum(<expr>) -> number
        Sum of numerical values.

    - var(<expr>) -> number
        Population variance. Same as `var_pop`.

    - var_pop(<expr>) -> number
        Population variance. Same as `var`.

    - var_sample(<expr>) -> number
        Sample variance (i.e. using Bessel's correction).
"
}

pub enum MoonbladeMode {
    Map,
    Filter,
    Transform,
    Flatmap,
}

impl MoonbladeMode {
    fn is_map(&self) -> bool {
        match self {
            Self::Map => true,
            _ => false,
        }
    }

    fn is_flatmap(&self) -> bool {
        match self {
            Self::Flatmap => true,
            _ => false,
        }
    }

    fn is_transform(&self) -> bool {
        match self {
            Self::Transform => true,
            _ => false,
        }
    }

    fn cannot_report(&self) -> bool {
        match self {
            Self::Filter | Self::Flatmap => true,
            _ => false,
        }
    }
}

pub enum MoonbladeErrorPolicy {
    Panic,
    Report,
    Ignore,
    Log,
}

impl MoonbladeErrorPolicy {
    fn will_report(&self) -> bool {
        match self {
            Self::Report => true,
            _ => false,
        }
    }

    pub fn from_restricted(value: &str) -> Result<Self, CliError> {
        Ok(match value {
            "panic" => Self::Panic,
            "ignore" => Self::Ignore,
            "log" => Self::Log,
            _ => {
                return Err(CliError::Other(format!(
                    "unknown error policy \"{}\"",
                    value
                )))
            }
        })
    }
}

impl TryFrom<String> for MoonbladeErrorPolicy {
    type Error = CliError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(match value.as_str() {
            "panic" => Self::Panic,
            "report" => Self::Report,
            "ignore" => Self::Ignore,
            "log" => Self::Log,
            _ => {
                return Err(CliError::Other(format!(
                    "unknown error policy \"{}\"",
                    value
                )))
            }
        })
    }
}

pub struct MoonbladeCmdArgs {
    pub print_cheatsheet: bool,
    pub print_functions: bool,
    pub target_column: Option<String>,
    pub rename_column: Option<String>,
    pub map_expr: String,
    pub input: Option<String>,
    pub output: Option<String>,
    pub no_headers: bool,
    pub delimiter: Option<Delimiter>,
    pub threads: Option<usize>,
    pub error_policy: MoonbladeErrorPolicy,
    pub error_column_name: Option<String>,
    pub mode: MoonbladeMode,
}

pub fn handle_eval_result<'a, 'b>(
    args: &'a MoonbladeCmdArgs,
    index: usize,
    record: &'b mut csv::ByteRecord,
    eval_result: Result<DynamicValue, EvaluationError>,
    replace: Option<usize>,
) -> Result<Vec<Cow<'b, csv::ByteRecord>>, String> {
    let mut records_to_emit: Vec<Cow<csv::ByteRecord>> = Vec::new();

    match eval_result {
        Ok(value) => match args.mode {
            MoonbladeMode::Filter => {
                if value.is_truthy() {
                    records_to_emit.push(Cow::Borrowed(record));
                }
            }
            MoonbladeMode::Map => {
                record.push_field(&value.serialize_as_bytes(b"|"));

                if args.error_policy.will_report() {
                    record.push_field(b"");
                }

                records_to_emit.push(Cow::Borrowed(record));
            }
            MoonbladeMode::Transform => {
                let mut record =
                    record.replace_at(replace.unwrap(), &value.serialize_as_bytes(b"|"));

                if args.error_policy.will_report() {
                    record.push_field(b"");
                }

                records_to_emit.push(Cow::Owned(record));
            }
            MoonbladeMode::Flatmap => 'm: {
                if value.is_falsey() {
                    break 'm;
                }

                for subvalue in value.flat_iter() {
                    let cell = subvalue.serialize_as_bytes(b"|");

                    let new_record = if let Some(idx) = replace {
                        record.replace_at(idx, &cell)
                    } else {
                        record.append(&cell)
                    };

                    records_to_emit.push(Cow::Owned(new_record));
                }
            }
        },
        Err(err) => match args.error_policy {
            MoonbladeErrorPolicy::Ignore => {
                let value = DynamicValue::None.serialize_as_bytes(b"|");

                if args.mode.is_map() {
                    record.push_field(&value);
                    records_to_emit.push(Cow::Borrowed(record));
                } else if args.mode.is_transform() {
                    let record = record.replace_at(replace.unwrap(), &value);
                    records_to_emit.push(Cow::Owned(record));
                }
            }
            MoonbladeErrorPolicy::Report => {
                if args.mode.cannot_report() {
                    unreachable!();
                }

                let value = DynamicValue::None.serialize_as_bytes(b"|");

                if args.mode.is_map() {
                    record.push_field(&value);
                    record.push_field(err.to_string().as_bytes());
                    records_to_emit.push(Cow::Borrowed(record));
                } else if args.mode.is_transform() {
                    let mut record = record.replace_at(replace.unwrap(), &value);
                    record.push_field(err.to_string().as_bytes());
                    records_to_emit.push(Cow::Owned(record));
                }
            }
            MoonbladeErrorPolicy::Log => {
                eprintln!("Row n°{}: {}", index + 1, err);

                let value = DynamicValue::None.serialize_as_bytes(b"|");

                if args.mode.is_map() {
                    record.push_field(&value);
                    records_to_emit.push(Cow::Borrowed(record));
                } else if args.mode.is_transform() {
                    let record = record.replace_at(replace.unwrap(), &value);
                    records_to_emit.push(Cow::Owned(record));
                }
            }
            MoonbladeErrorPolicy::Panic => {
                return Err(format!("Row n°{}: {}", index + 1, err));
            }
        },
    };

    Ok(records_to_emit)
}

pub fn run_moonblade_cmd(args: MoonbladeCmdArgs) -> CliResult<()> {
    if args.print_cheatsheet {
        println!("{}", get_moonblade_cheatsheet());
        return Ok(());
    }

    if args.print_functions {
        println!("{}", get_moonblade_functions_help());
        return Ok(());
    }

    let mut rconfig = Config::new(&args.input)
        .delimiter(args.delimiter)
        .no_headers(args.no_headers);

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.output).writer()?;

    let mut headers = csv::ByteRecord::new();
    let mut modified_headers = csv::ByteRecord::new();
    let mut must_write_headers = false;
    let mut column_to_replace: Option<usize> = None;
    let mut map_expr = args.map_expr.clone();

    if !args.no_headers {
        headers = rdr.byte_headers()?.clone();
        modified_headers = headers.clone();

        if !headers.is_empty() {
            must_write_headers = true;

            if args.mode.is_map() {
                if let Some(target_column) = &args.target_column {
                    modified_headers.push_field(target_column.as_bytes());
                }
            } else if args.mode.is_transform() {
                if let Some(name) = &args.target_column {
                    rconfig = rconfig.select(SelectColumns::parse(name)?);
                    let idx = rconfig.single_selection(&headers)?;

                    if let Some(renamed) = &args.rename_column {
                        modified_headers = modified_headers.replace_at(idx, renamed.as_bytes());
                    }

                    column_to_replace = Some(idx);

                    // NOTE: binding implicit last value to target column value
                    map_expr = format!("col({}) | {}", idx, map_expr);
                }
            } else if args.mode.is_flatmap() {
                if let Some(replaced) = &args.rename_column {
                    rconfig = rconfig.select(SelectColumns::parse(replaced)?);
                    let idx = rconfig.single_selection(&headers)?;

                    if let Some(renamed) = &args.target_column {
                        modified_headers = modified_headers.replace_at(idx, renamed.as_bytes());
                    }

                    column_to_replace = Some(idx);
                } else if let Some(target_column) = &args.target_column {
                    modified_headers.push_field(target_column.as_bytes());
                }
            }

            if args.error_policy.will_report() {
                if let Some(error_column_name) = &args.error_column_name {
                    modified_headers.push_field(error_column_name.as_bytes());
                }
            }
        }
    }

    let mut program = PipelineProgram::parse(&map_expr, &headers)?;

    if must_write_headers {
        wtr.write_byte_record(&modified_headers)?;
    }

    if let Some(threads) = args.threads {
        // NOTE: this could be a OnceCell but it is very new in rust
        let local: Arc<ThreadLocal<RefCell<PipelineProgram>>> =
            Arc::new(ThreadLocal::with_capacity(threads));

        rdr.into_byte_records()
            .enumerate()
            .parallel_map_custom(
                |o| o.threads(threads),
                move |(i, record)| -> CliResult<(
                    usize,
                    csv::ByteRecord,
                    Result<DynamicValue, EvaluationError>,
                )> {
                    let record = record?;

                    let mut local_program =
                        local.get_or(|| RefCell::new(program.clone())).borrow_mut();

                    local_program.set("index", DynamicValue::Integer(i as i64));

                    let eval_result = local_program.run_with_record(&record);

                    Ok((i, record, eval_result))
                },
            )
            .try_for_each(|result| -> CliResult<()> {
                let (i, mut record, eval_result) = result?;
                let records_to_emit =
                    handle_eval_result(&args, i, &mut record, eval_result, column_to_replace)?;

                for record_to_emit in records_to_emit {
                    wtr.write_byte_record(&record_to_emit)?;
                }
                Ok(())
            })?;

        return Ok(wtr.flush()?);
    }

    let mut record = csv::ByteRecord::new();
    let mut i: usize = 0;

    while rdr.read_byte_record(&mut record)? {
        program.set("index", DynamicValue::Integer(i as i64));

        let eval_result = program.run_with_record(&record);

        let records_to_emit =
            handle_eval_result(&args, i, &mut record, eval_result, column_to_replace)?;

        for record_to_emit in records_to_emit {
            wtr.write_byte_record(&record_to_emit)?;
        }

        i += 1;
    }

    Ok(wtr.flush()?)
}