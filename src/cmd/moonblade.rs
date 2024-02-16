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
xan script language cheatsheet (use --functions for comprehensive list of
available functions & operators):

  . Indexing a column by name:
        'name'

  . Indexing column with forbidden characters (e.g. spaces, commas etc.):
        'col(\"Name of film\")'

  . Indexing column by index (0-based):
        'col(2)'

  . Indexing a column by name and 0-based nth (for duplicate headers):
        'col(\"col\", 1)'

  . Applying functions:
        'trim(name)'
        'trim(concat(name, \" \", surname))'

  . Using operators (unary & binary):
        '-nb1'
        'nb1 + nb2'
        '(nb1 > 1) || nb2'

  . Integer literals:
        '1'

  . Float literals:
        '0.5'

  . Boolean literals:
        'true'
        'false'

  . Null literals:
        'null'

  . String literals (can use single or double quotes):
        '\"hello\"'
        \"'hello'\"

  . Regex literals:
        '/john/'
        '/john/i' (case-insensitive)

  . Special variables:
        '%index' -> current row index

  . Nesting function calls:
        'add(sub(col1, col2), mul(col3, col4))'

  . Basic branching (also consider using the \"coalesce\" function for simple cases):
        'if(count < 4, trim(name), trim(surname))'

  . Piping (underscore \"_\" becomes a reference to previous result):
        'trim(name) | lower(_) | add(count, len(_))'

        is the same as:

        'add(count, len(lower(trim(name))))'

  . Piping shorthand for unary functions:
        'trim(name) | lower'

        is the same as:

        'trim(name) | lower(_)'
"
}

pub fn get_moonblade_functions_help() -> &'static str {
    "
# Available functions & operators

(use --cheatsheet for a reminder of the expression language's basics)

## Operators

### Unary operators

    !x - boolean negation
    -x - numerical negation,

### Numerical comparison

Warning: those operators will always consider operands as numbers and will
try to cast them around as such. For string/sequence comparison, use the
operators in the next section.

    x == y - numerical equality
    x != y - numerical inequality
    x <  y - numerical less than
    x <= y - numerical less than or equal
    x >  y - numerical greater than
    x >= y - numerical greater than or equal

### String/sequence comparison

Warning: those operators will always consider operands as strings or
sequences and will try to cast them around as such. For numerical comparison,
use the operators in the previous section.

    x eq y - string equality
    x ne y - string inequality
    x lt y - string less than
    x le y - string less than or equal
    x gt y - string greater than
    x ge y - string greater than or equal

### Arithmetic operators

    x + y  - numerical addition
    x - y  - numerical subtraction
    x * y  - numerical multiplication
    x / y  - numerical division
    x % y  - numerical remainder

    x // y - numerical integer division
    x ** y - numerical exponentiation

## String operators

    x . y - string concatenation

## Logical operators

    x &&  y - logical and
    x and y
    x ||  y - logical or
    x or  y

    x in y
    x not in y

## Arithmetics

    - abs(x) -> number
        Return absolute value of number.

    - add(x, y, *n) -> number
        Add two or more numbers.

    - ceil(x) -> number
        Return the smallest integer greater than or equal to x.

    - div(x, y, *n) -> number
        Divide two or more numbers.

    - floor(x) -> number
        Return the smallest integer lower than or equal to x.

    - idiv(x, y) -> number
        Integer division of two numbers.

    - log(x) -> number
        Return the natural logarithm of x.

    - mod(x, y) -> number
        Return the remainder of x divided by y.

    - mul(x, y, *n) -> number
        Multiply two or more numbers.

    - neg(x) -> number
        Return -x.

    - pow(x, y) -> number
        Raise x to the power of y.

    - round(x) -> number
        Return x rounded to the nearest integer.

    - sqrt(x) -> number
        Return the square root of x.

    - sub(x, y, *n) -> number
        Subtract two or more numbers.

## Boolean operations & branching

    - and(a, b, *x) -> bool
        Perform boolean AND operation on two or more values.

    - if(cond, then, else?) -> T
        Evaluate condition and switch to correct branch.

    - unless(cond, then, else?) -> T
        Shorthand for `if(not(cond), then, else?)`.

    - not(a) -> bool
        Perform boolean NOT operation.

    - or(a, b, *x) -> bool
        Perform boolean OR operation on two or more values.

## Comparison

    - eq(s1, s2) -> bool
        Test string or sequence equality.

    - ne(s1, s2) -> bool
        Test string or sequence inequality.

    - gt(s1, s2) -> bool
        Test that string or sequence s1 > s2.

    - ge(s1, s2) -> bool
        Test that string or sequence s1 >= s2.

    - lt(s1, s2) -> bool
        Test that string or sequence s1 < s2.

    - ge(s1, s2) -> bool
        Test that string or sequence s1 <= s2.

## String & sequence helpers

    - compact(list) -> list
        Drop all falsey values from given list.

    - concat(string, *strings) -> string
        Concatenate given strings into a single one.

    - contains(seq, subseq) -> bool
        Find if subseq can be found in seq. Subseq can
        be a regular expression.

    - count(seq, pattern) -> int
        Count number of times pattern appear in seq. Pattern
        can be a regular expression.

    - endswith(string, pattern) -> bool
        Test if string ends with pattern.

    - escape_regex(string) -> string
        Escape a string so it can be used safely in a regular expression.

    - first(seq) -> T
        Get first element of sequence.

    - fmt(string, *replacements):
        Format a string by replacing \"{}\" occurrences by subsequent
        arguments.

        Example: `fmt(\"Hello {} {}\", name, surname)` will replace
        the first \"{}\" by the value of the name column, then the
        second one by the value of the surname column.

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

    - md5(string) -> string
        Return the md5 hash of string in hexadecimal representation.

    - uuid() -> string
        Return a uuid v4.

"
}

pub fn get_moonblade_aggregations_function_help() -> &'static str {
    "
# Available aggregation functions

(use --cheatsheet for a reminder of how the scripting language works)

    - all(<expr>) -> bool
        Returns true if all elements returned by given expression are truthy.

    - any(<expr>) -> bool
        Returns true if one of the elements returned by given expression is truthy.

    - avg(<expr>) -> number
        Average of numerical values. Same as `mean`.

    - cardinality(<expr>) -> number
        Number of distinct values returned by given expression.

    - count(<expr>?) -> number
        Count the numbers of row. Works like in SQL in that `count(<expr>)`
        will count all non-null values returned by given expression, while
        `count()` without any expression will count every matching row.

    - first(<expr>) -> string
        Return first seen non nullish element of the values returned by the given expression.

    - last(<expr>) -> string
        Return last seen non nullish element of the values returned by the given expression.

    - lex_first(<expr>) -> string
        Return first string in lexicographical order.

    - lex_last(<expr>) -> string
        Return last string in lexicographical order.

    - max(<expr>) -> number | string
        Maximum numerical value.

    - mean(<expr>) -> number
        Mean of numerical values. Same as `avg`.

    - median(<expr>) -> number
        Median of numerical values, interpolating on even counts.

    - median_high(<expr>) -> number
        Median of numerical values, returning higher value on even counts.

    - median_low(<expr>) -> number
        Median of numerical values, returning lower value on even counts.

    - min(<expr>) -> number | string
        Minimum numerical value.

    - mode(<expr>) - string
        Value appearing the most, breaking ties arbitrarily in favor of the
        first value in lexicographical order.

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
    Filter(bool),
    Transform,
    Flatmap,
}

impl MoonbladeMode {
    fn is_map(&self) -> bool {
        matches!(self, Self::Map)
    }

    fn is_flatmap(&self) -> bool {
        matches!(self, Self::Flatmap)
    }

    fn is_transform(&self) -> bool {
        matches!(self, Self::Transform)
    }

    fn cannot_report(&self) -> bool {
        matches!(self, Self::Filter(_) | Self::Flatmap)
    }
}

pub enum MoonbladeErrorPolicy {
    Panic,
    Report,
    Ignore,
    Log,
}

impl MoonbladeErrorPolicy {
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

    fn will_report(&self) -> bool {
        matches!(self, Self::Report)
    }

    pub fn handle_error(
        &self,
        index: usize,
        error: EvaluationError,
    ) -> Result<(), EvaluationError> {
        match self {
            MoonbladeErrorPolicy::Panic => Err(error)?,
            MoonbladeErrorPolicy::Ignore => Ok(()),
            MoonbladeErrorPolicy::Log => {
                eprintln!("Row n°{}: {}", index, error);
                Ok(())
            }
            _ => unreachable!(),
        }
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
    pub parallelization: Option<Option<usize>>,
    pub error_policy: MoonbladeErrorPolicy,
    pub error_column_name: Option<String>,
    pub mode: MoonbladeMode,
}

pub fn handle_eval_result<'b>(
    args: &MoonbladeCmdArgs,
    index: usize,
    record: &'b mut csv::ByteRecord,
    eval_result: Result<DynamicValue, EvaluationError>,
    replace: Option<usize>,
) -> Result<Vec<Cow<'b, csv::ByteRecord>>, String> {
    let mut records_to_emit: Vec<Cow<csv::ByteRecord>> = Vec::new();

    match eval_result {
        Ok(value) => match args.mode {
            MoonbladeMode::Filter(invert) => {
                let mut should_emit = value.is_truthy();

                if invert {
                    should_emit = !should_emit;
                }

                if should_emit {
                    records_to_emit.push(Cow::Borrowed(record));
                }
            }
            MoonbladeMode::Map => {
                record.push_field(&value.serialize_as_bytes());

                if args.error_policy.will_report() {
                    record.push_field(b"");
                }

                records_to_emit.push(Cow::Borrowed(record));
            }
            MoonbladeMode::Transform => {
                let mut record = record.replace_at(replace.unwrap(), &value.serialize_as_bytes());

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
                    let cell = subvalue.serialize_as_bytes();

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
                if args.mode.is_map() {
                    record.push_field(b"");
                    records_to_emit.push(Cow::Borrowed(record));
                } else if args.mode.is_transform() {
                    let record = record.replace_at(replace.unwrap(), b"");
                    records_to_emit.push(Cow::Owned(record));
                }
            }
            MoonbladeErrorPolicy::Report => {
                if args.mode.cannot_report() {
                    unreachable!();
                }

                if args.mode.is_map() {
                    record.push_field(b"");
                    record.push_field(err.to_string().as_bytes());
                    records_to_emit.push(Cow::Borrowed(record));
                } else if args.mode.is_transform() {
                    let mut record = record.replace_at(replace.unwrap(), b"");
                    record.push_field(err.to_string().as_bytes());
                    records_to_emit.push(Cow::Owned(record));
                }
            }
            MoonbladeErrorPolicy::Log => {
                eprintln!("Row n°{}: {}", index + 1, err);

                if args.mode.is_map() {
                    record.push_field(b"");
                    records_to_emit.push(Cow::Borrowed(record));
                } else if args.mode.is_transform() {
                    let record = record.replace_at(replace.unwrap(), b"");
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

    if let Some(threads) = args.parallelization {
        // NOTE: this could be a OnceCell but it is very new in rust
        let local: Arc<ThreadLocal<RefCell<PipelineProgram>>> = Arc::new(match threads {
            None => ThreadLocal::new(),
            Some(count) => ThreadLocal::with_capacity(count),
        });

        rdr.into_byte_records()
            .enumerate()
            .parallel_map_custom(
                |o| {
                    if let Some(count) = threads {
                        o.threads(count)
                    } else {
                        o
                    }
                },
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
