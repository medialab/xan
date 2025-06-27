use std::convert::TryFrom;
use std::io::Write;

use pariter::IteratorExt;

use crate::config::{Config, Delimiter};
use crate::moonblade::{DynamicValue, Program, SpecifiedEvaluationError};
use crate::select::SelectColumns;
use crate::util::ImmutableRecordHelpers;
use crate::CliError;
use crate::CliResult;

#[derive(Default)]
enum MoonbladeOutputValue {
    #[default]
    None,
    Some(Vec<u8>),
    Multiple(Vec<Vec<u8>>),
}

impl MoonbladeOutputValue {
    fn of(mode: &MoonbladeMode, value: &DynamicValue) -> Self {
        let mut output_value = Self::default();
        output_value.process(mode, value);
        output_value
    }

    fn unwrap(self) -> Vec<u8> {
        match self {
            Self::Some(bytes) => bytes,
            _ => panic!("cannot unwrap"),
        }
    }

    fn into_iter(self) -> Box<dyn Iterator<Item = Vec<u8>>> {
        match self {
            Self::None => Box::new(std::iter::empty()),
            Self::Some(bytes) => Box::new(std::iter::once(bytes)),
            Self::Multiple(list) => Box::new(list.into_iter()),
        }
    }

    fn push(&mut self, value: &DynamicValue) {
        let bytes = value.serialize_as_bytes().into_owned();

        match self {
            Self::None => {
                *self = Self::Some(bytes);
            }
            Self::Some(other) => {
                let other = std::mem::take(other);
                *self = Self::Multiple(vec![other, bytes]);
            }
            Self::Multiple(values) => {
                values.push(bytes);
            }
        };
    }

    fn process(&mut self, mode: &MoonbladeMode, value: &DynamicValue) {
        match mode {
            MoonbladeMode::Flatmap => {
                if value.is_truthy() {
                    for subvalue in value.flat_iter() {
                        self.push(subvalue);
                    }
                }
            }
            _ => {
                self.push(value);
            }
        }
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub enum MoonbladeMode {
    #[default]
    Map,
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
        matches!(self, Self::Flatmap)
    }
}

#[derive(Default, Debug, Deserialize, Clone, Copy)]
#[serde(try_from = "String")]
pub enum MoonbladeErrorPolicy {
    #[default]
    Panic,
    Ignore,
    Log,
}

impl TryFrom<String> for MoonbladeErrorPolicy {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(match value.as_str() {
            "panic" => Self::Panic,
            "ignore" => Self::Ignore,
            "log" => Self::Log,
            _ => {
                return Err(format!(
                    "unknown moonblade error policy given to -E/--errors \"{}\"!",
                    value
                ))
            }
        })
    }
}

#[derive(Default, Debug)]
pub enum LegacyMoonbladeErrorPolicy {
    #[default]
    Panic,
    Report,
    Ignore,
    Log,
}

impl LegacyMoonbladeErrorPolicy {
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

    pub fn handle_error<T: Default>(
        &self,
        result: Result<T, SpecifiedEvaluationError>,
    ) -> Result<T, SpecifiedEvaluationError> {
        match result {
            Ok(value) => Ok(value),
            Err(err) => match self {
                LegacyMoonbladeErrorPolicy::Panic => Err(err)?,
                LegacyMoonbladeErrorPolicy::Ignore => Ok(T::default()),
                LegacyMoonbladeErrorPolicy::Log => {
                    eprintln!("{}", err);
                    Ok(T::default())
                }
                _ => unreachable!(),
            },
        }
    }
}

impl TryFrom<String> for LegacyMoonbladeErrorPolicy {
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

#[derive(Default, Debug)]
pub struct MoonbladeCmdArgs {
    pub target_column: Option<String>,
    pub rename_column: Option<String>,
    pub map_expr: String,
    pub input: Option<String>,
    pub output: Option<String>,
    pub no_headers: bool,
    pub delimiter: Option<Delimiter>,
    pub parallelization: Option<Option<usize>>,
    pub error_policy: LegacyMoonbladeErrorPolicy,
    pub error_column_name: Option<String>,
    pub mode: MoonbladeMode,
    pub limit: Option<usize>,
}

fn handle_moonblade_output<W: Write>(
    writer: &mut csv::Writer<W>,
    args: &MoonbladeCmdArgs,
    index: usize,
    record: &mut csv::ByteRecord,
    eval_result: Result<MoonbladeOutputValue, SpecifiedEvaluationError>,
    replace: Option<usize>,
) -> CliResult<usize> {
    let mut written_count: usize = 0;

    match eval_result {
        Ok(value) => match args.mode {
            MoonbladeMode::Map => {
                record.push_field(&value.unwrap());

                if args.error_policy.will_report() {
                    record.push_field(b"");
                }

                writer.write_byte_record(record)?;
                written_count += 1;
            }
            MoonbladeMode::Transform => {
                let mut record = record.replace_at(replace.unwrap(), &value.unwrap());

                if args.error_policy.will_report() {
                    record.push_field(b"");
                }

                writer.write_byte_record(&record)?;
                written_count += 1;
            }
            MoonbladeMode::Flatmap => {
                for cell in value.into_iter() {
                    let new_record = if let Some(idx) = replace {
                        record.replace_at(idx, &cell)
                    } else {
                        record.append(&cell)
                    };

                    writer.write_byte_record(&new_record)?;
                    written_count += 1;
                }
            }
        },
        Err(err) => match args.error_policy {
            LegacyMoonbladeErrorPolicy::Ignore => {
                if args.mode.is_map() {
                    record.push_field(b"");
                    writer.write_byte_record(record)?;
                    written_count += 1;
                } else if args.mode.is_transform() {
                    let record = record.replace_at(replace.unwrap(), b"");
                    writer.write_byte_record(&record)?;
                    written_count += 1;
                }
            }
            LegacyMoonbladeErrorPolicy::Report => {
                if args.mode.cannot_report() {
                    unreachable!();
                }

                if args.mode.is_map() {
                    record.push_field(b"");
                    record.push_field(err.to_string().as_bytes());
                    writer.write_byte_record(record)?;
                    written_count += 1;
                } else if args.mode.is_transform() {
                    let mut record = record.replace_at(replace.unwrap(), b"");
                    record.push_field(err.to_string().as_bytes());
                    writer.write_byte_record(&record)?;
                    written_count += 1;
                }
            }
            LegacyMoonbladeErrorPolicy::Log => {
                eprintln!("Row n°{}: {}", index + 1, err);

                if args.mode.is_map() {
                    record.push_field(b"");
                    writer.write_byte_record(record)?;
                    written_count += 1;
                } else if args.mode.is_transform() {
                    let record = record.replace_at(replace.unwrap(), b"");
                    writer.write_byte_record(&record)?;
                    written_count += 1;
                }
            }
            LegacyMoonbladeErrorPolicy::Panic => {
                Err(format!("Row n°{}: {}", index + 1, err))?;
            }
        },
    };

    Ok(written_count)
}

pub fn run_moonblade_cmd(args: MoonbladeCmdArgs) -> CliResult<()> {
    let mut rconfig = Config::new(&args.input)
        .delimiter(args.delimiter)
        .no_headers(args.no_headers);

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.output).writer()?;

    let headers = rdr.byte_headers()?.clone();
    let mut modified_headers = csv::ByteRecord::new();
    let mut must_write_headers = false;
    let mut column_to_replace: Option<usize> = None;
    let mut map_expr = args.map_expr.clone();

    if args.mode.is_transform() {
        rconfig = rconfig.select(SelectColumns::parse(&args.target_column.clone().unwrap())?);
        let idx = rconfig.single_selection(&headers)?;

        // NOTE: binding implicit last value to target column value
        map_expr = format!("col({}) | {}", idx, map_expr);
        column_to_replace = Some(idx);
    } else if args.mode.is_flatmap() {
        if let Some(replaced) = &args.rename_column {
            rconfig = rconfig.select(SelectColumns::parse(replaced)?);
            let idx = rconfig.single_selection(&headers)?;

            // NOTE: binding implicit last value to target column value
            map_expr = format!("col({}) | {}", idx, map_expr);
            column_to_replace = Some(idx);
        }
    }

    if !args.no_headers {
        modified_headers = headers.clone();

        if !headers.is_empty() {
            must_write_headers = true;

            if args.mode.is_map() {
                if let Some(target_column) = &args.target_column {
                    modified_headers.push_field(target_column.as_bytes());
                }
            } else if args.mode.is_transform() {
                if let Some(renamed) = &args.rename_column {
                    modified_headers =
                        modified_headers.replace_at(column_to_replace.unwrap(), renamed.as_bytes());
                }
            } else if args.mode.is_flatmap() {
                if args.rename_column.is_some() {
                    if let Some(renamed) = &args.target_column {
                        modified_headers = modified_headers
                            .replace_at(column_to_replace.unwrap(), renamed.as_bytes());
                    }
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
                |o| o.threads(threads.unwrap_or_else(num_cpus::get)),
                move |(i, record)| -> CliResult<(
                    usize,
                    csv::ByteRecord,
                    Result<MoonbladeOutputValue, SpecifiedEvaluationError>,
                )> {
                    let record = record?;

                    let eval_result = program
                        .run_with_record(i, &record)
                        .map(|value| MoonbladeOutputValue::of(&args.mode, &value));

                    Ok((i, record, eval_result))
                },
            )
            .try_for_each(|result| -> CliResult<()> {
                let (i, mut record, eval_result) = result?;
                handle_moonblade_output(
                    &mut wtr,
                    &args,
                    i,
                    &mut record,
                    eval_result,
                    column_to_replace,
                )?;

                Ok(())
            })?;

        return Ok(wtr.flush()?);
    }

    let mut record = csv::ByteRecord::new();
    let mut i: usize = 0;
    let mut emitted: usize = 0;

    while rdr.read_byte_record(&mut record)? {
        let eval_result = program
            .run_with_record(i, &record)
            .map(|value| MoonbladeOutputValue::of(&args.mode, &value));

        emitted += handle_moonblade_output(
            &mut wtr,
            &args,
            i,
            &mut record,
            eval_result,
            column_to_replace,
        )?;

        i += 1;

        if let Some(limit) = args.limit {
            if emitted >= limit {
                break;
            }
        }
    }

    Ok(wtr.flush()?)
}
