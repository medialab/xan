use std::io::Write;

use pariter::IteratorExt;

use crate::config::{Config, Delimiter};
use crate::moonblade::{DynamicValue, Program, SpecifiedEvaluationError};
use crate::select::SelectColumns;
use crate::util::ImmutableRecordHelpers;
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

    fn process(&mut self, _mode: &MoonbladeMode, value: &DynamicValue) {
        self.push(value);
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub enum MoonbladeMode {
    #[default]
    Transform,
}

impl MoonbladeMode {
    fn is_transform(&self) -> bool {
        matches!(self, Self::Transform)
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
            MoonbladeMode::Transform => {
                let record = record.replace_at(replace.unwrap(), &value.unwrap());

                writer.write_byte_record(&record)?;
                written_count += 1;
            }
        },
        Err(err) => {
            Err(format!("Row n°{}: {}", index + 1, err))?;
        }
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
    }

    if !args.no_headers {
        modified_headers = headers.clone();

        if !headers.is_empty() {
            must_write_headers = true;

            if args.mode.is_transform() {
                if let Some(renamed) = &args.rename_column {
                    modified_headers =
                        modified_headers.replace_at(column_to_replace.unwrap(), renamed.as_bytes());
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
