use std::borrow::Cow;
use std::convert::TryFrom;

use pariter::IteratorExt;

use crate::config::{Config, Delimiter};
use crate::moonblade::{DynamicValue, Program, SpecifiedEvaluationError};
use crate::select::SelectColumns;
use crate::util::ImmutableRecordHelpers;
use crate::CliError;
use crate::CliResult;

#[derive(Default)]
pub enum MoonbladeMode {
    #[default]
    Map,
    Foreach,
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

    fn should_not_emit_headers(&self) -> bool {
        matches!(self, Self::Foreach)
    }

    fn cannot_report(&self) -> bool {
        matches!(self, Self::Filter(_) | Self::Flatmap | Self::Foreach)
    }
}

#[derive(Default)]
pub enum MoonbladeErrorPolicy {
    #[default]
    Panic,
    Report,
    Ignore,
    Log,
}

impl MoonbladeErrorPolicy {
    pub fn try_from_restricted(value: &str) -> Result<Self, CliError> {
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

    pub fn handle_row_error(
        &self,
        index: usize,
        error: SpecifiedEvaluationError,
    ) -> Result<(), SpecifiedEvaluationError> {
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

    pub fn handle_error<T: Default>(
        &self,
        result: Result<T, SpecifiedEvaluationError>,
    ) -> Result<T, SpecifiedEvaluationError> {
        match result {
            Ok(value) => Ok(value),
            Err(err) => match self {
                MoonbladeErrorPolicy::Panic => Err(err)?,
                MoonbladeErrorPolicy::Ignore => Ok(T::default()),
                MoonbladeErrorPolicy::Log => {
                    eprintln!("{}", err);
                    Ok(T::default())
                }
                _ => unreachable!(),
            },
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

#[derive(Default)]
pub struct MoonbladeCmdArgs {
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
    pub limit: Option<usize>,
}

pub fn handle_eval_result<'b>(
    args: &MoonbladeCmdArgs,
    index: usize,
    record: &'b mut csv::ByteRecord,
    eval_result: Result<DynamicValue, SpecifiedEvaluationError>,
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
            MoonbladeMode::Foreach => {}
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
            must_write_headers = !args.mode.should_not_emit_headers();

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

    let program = Program::parse(&map_expr, &headers)?;

    if must_write_headers {
        wtr.write_byte_record(&modified_headers)?;
    }

    if let Some(threads) = args.parallelization {
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
                    Result<DynamicValue, SpecifiedEvaluationError>,
                )> {
                    let record = record?;

                    let eval_result = program.run_with_record(i, &record);

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
    let mut emitted: usize = 0;

    while rdr.read_byte_record(&mut record)? {
        let eval_result = program.run_with_record(i, &record);

        let records_to_emit =
            handle_eval_result(&args, i, &mut record, eval_result, column_to_replace)?;

        for record_to_emit in records_to_emit {
            emitted += 1;
            wtr.write_byte_record(&record_to_emit)?;
        }

        i += 1;

        if let Some(limit) = args.limit {
            if emitted >= limit {
                break;
            }
        }
    }

    Ok(wtr.flush()?)
}
