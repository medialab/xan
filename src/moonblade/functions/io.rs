use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use bytesize::ByteSize;
use encoding::{label::encoding_from_whatwg_label, DecoderTrap};
use flate2::read::MultiGzDecoder;
use lazy_static::lazy_static;
use namedlock::{AutoCleanup, LockSpace};

use crate::collections::HashMap;

use super::FunctionResult;
use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArguments, DynamicValue};

pub fn abspath(args: BoundArguments) -> FunctionResult {
    let arg = args.get1_str()?;
    let mut path = PathBuf::new();
    path.push(arg.as_ref());
    let path = path.canonicalize().unwrap();
    let path = String::from(path.to_str().ok_or(EvaluationError::InvalidPath)?);

    Ok(DynamicValue::from(path))
}

pub fn pathjoin(args: BoundArguments) -> FunctionResult {
    let mut path = PathBuf::new();

    for arg in args {
        path.push(arg.try_as_str()?.as_ref());
    }

    let path = String::from(path.to_str().ok_or(EvaluationError::InvalidPath)?);

    Ok(DynamicValue::from(path))
}

fn decoder_trap_from_str(name: &str) -> Result<DecoderTrap, EvaluationError> {
    Ok(match name {
        "strict" => DecoderTrap::Strict,
        "replace" => DecoderTrap::Replace,
        "ignore" => DecoderTrap::Ignore,
        _ => return Err(EvaluationError::UnsupportedDecoderTrap(name.to_string())),
    })
}

pub fn isfile(args: BoundArguments) -> FunctionResult {
    let path = args.get1_str()?;
    let path = Path::new(path.as_ref());

    Ok(DynamicValue::Boolean(path.is_file()))
}

fn abstract_read(
    path: &DynamicValue,
    encoding: Option<&DynamicValue>,
    errors: Option<&DynamicValue>,
) -> Result<String, EvaluationError> {
    let path = path.try_as_str()?;

    let mut file = match File::open(path.as_ref()) {
        Err(_) => return Err(EvaluationError::IO(format!("cannot read file {}", path))),
        Ok(f) => f,
    };

    let contents = match encoding {
        Some(encoding_value) => {
            let encoding_name = encoding_value.try_as_str()?.replace('_', "-");
            let encoding = encoding_from_whatwg_label(&encoding_name);
            let encoding = encoding
                .ok_or_else(|| EvaluationError::UnsupportedEncoding(encoding_name.to_string()))?;

            let decoder_trap = match errors {
                Some(trap) => decoder_trap_from_str(&trap.try_as_str()?)?,
                None => DecoderTrap::Replace,
            };

            let mut buffer: Vec<u8> = Vec::new();

            if path.ends_with(".gz") {
                let mut gz = MultiGzDecoder::new(file);
                gz.read_to_end(&mut buffer)
                    .map_err(|_| EvaluationError::IO(format!("cannot read file {}", path)))?;
            } else {
                file.read_to_end(&mut buffer)
                    .map_err(|_| EvaluationError::IO(format!("cannot read file {}", path)))?;
            }

            encoding
                .decode(&buffer, decoder_trap)
                .map_err(|_| EvaluationError::DecodeError)?
        }
        None => {
            let mut buffer = String::new();

            if path.ends_with(".gz") {
                let mut gz = MultiGzDecoder::new(file);
                gz.read_to_string(&mut buffer)
                    .map_err(|_| EvaluationError::IO(format!("cannot read file {}", path)))?;
            } else {
                file.read_to_string(&mut buffer)
                    .map_err(|_| EvaluationError::IO(format!("cannot read file {}", path)))?;
            }

            buffer
        }
    };

    Ok(contents)
}

pub fn read(args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(abstract_read(
        args.get1(),
        args.get_not_none(1),
        args.get_not_none(2),
    )?))
}

pub fn read_json(args: BoundArguments) -> FunctionResult {
    let contents = abstract_read(args.get1(), None, None)?;
    serde_json::from_str(&contents)
        .map_err(|_| EvaluationError::JSONParseError(format!("{:?}", contents)))
}

pub fn read_csv(args: BoundArguments) -> FunctionResult {
    let contents = abstract_read(args.get1(), None, None)?;

    let mut reader = csv::Reader::from_reader(contents.as_bytes());
    let headers = reader
        .headers()
        .map_err(|_| EvaluationError::IO("error while reading CSV header row".to_string()))?
        .clone();

    let mut record = csv::StringRecord::new();
    let mut rows: Vec<DynamicValue> = Vec::new();

    loop {
        match reader.read_record(&mut record) {
            Err(_) => {
                return Err(EvaluationError::IO(
                    "error while reading CSV row".to_string(),
                ))
            }
            Ok(has_row) => {
                if !has_row {
                    break;
                }

                let mut map: HashMap<String, DynamicValue> = HashMap::with_capacity(headers.len());

                for (cell, header) in record.iter().zip(headers.iter()) {
                    map.insert(header.to_string(), DynamicValue::from(cell));
                }

                rows.push(DynamicValue::from(map));
            }
        }
    }

    Ok(DynamicValue::from(rows))
}

lazy_static! {
    static ref WRITE_FILE_LOCKS: LockSpace<PathBuf, ()> = LockSpace::new(AutoCleanup);
}

pub fn write(args: BoundArguments) -> FunctionResult {
    let data = args.get1();
    let path = PathBuf::from(args.get(1).unwrap().try_as_str()?.as_ref());

    // mkdir -p
    if let Some(dir) = path.parent() {
        // NOTE: fs::create_dir_all is threadsafe
        fs::create_dir_all(dir).map_err(|_| {
            EvaluationError::IO(format!("cannot create dir {}", dir.to_string_lossy()))
        })?;
    }

    WRITE_FILE_LOCKS
        .lock(path.clone(), || ())
        .map_err(|_| EvaluationError::Custom("write file lock is poisoned".to_string()))?;

    fs::write(&path, data.try_as_bytes()?).map_err(|_| {
        EvaluationError::IO(format!("cannot write file {}", path.to_string_lossy()))
    })?;

    Ok(DynamicValue::from(path.to_string_lossy()))
}

pub fn move_file(args: BoundArguments) -> FunctionResult {
    let (source, target) = args.get2_str()?;

    let source_path = PathBuf::from(source.as_ref());
    let target_path = PathBuf::from(target.as_ref());

    // mkdir -p
    if let Some(dir) = target_path.parent() {
        // NOTE: fs::create_dir_all is threadsafe
        fs::create_dir_all(dir).map_err(|_| {
            EvaluationError::IO(format!("cannot create dir {}", dir.to_string_lossy()))
        })?;
    }

    fs::rename(&source_path, &target_path).map_err(|_| {
        EvaluationError::IO(format!(
            "cannot move from {} to {}",
            source_path.to_string_lossy(),
            target_path.to_string_lossy()
        ))
    })?;

    Ok(DynamicValue::from(target_path.to_string_lossy()))
}

pub fn copy_file(args: BoundArguments) -> FunctionResult {
    let (source, target) = args.get2_str()?;

    let source_path = PathBuf::from(source.as_ref());
    let target_path = PathBuf::from(target.as_ref());

    // mkdir -p
    if let Some(dir) = target_path.parent() {
        // NOTE: fs::create_dir_all is threadsafe
        fs::create_dir_all(dir).map_err(|_| {
            EvaluationError::IO(format!("cannot create dir {}", dir.to_string_lossy()))
        })?;
    }

    fs::copy(&source_path, &target_path).map_err(|_| {
        EvaluationError::IO(format!(
            "cannot copy {} to {}",
            source_path.to_string_lossy(),
            target_path.to_string_lossy()
        ))
    })?;

    Ok(DynamicValue::from(target_path.to_string_lossy()))
}

pub fn ext(args: BoundArguments) -> FunctionResult {
    let string = args.get1_str()?;
    let path = Path::new(string.as_ref());

    Ok(DynamicValue::from(
        path.extension().and_then(|e| e.to_str()),
    ))
}

pub fn dirname(args: BoundArguments) -> FunctionResult {
    let string = args.get1_str()?;
    let path = Path::new(string.as_ref());

    Ok(DynamicValue::from(path.parent().and_then(|p| p.to_str())))
}

pub fn basename(args: BoundArguments) -> FunctionResult {
    let string = args.get1_str()?;
    let path = Path::new(string.as_ref());

    let name = path.file_name().and_then(|p| p.to_str());

    if args.len() == 2 {
        let suffix = args.get(1).unwrap().try_as_str()?;

        Ok(DynamicValue::from(
            name.and_then(|n| n.strip_suffix(suffix.as_ref()).or(Some(n))),
        ))
    } else {
        Ok(DynamicValue::from(name))
    }
}

pub fn filesize(args: BoundArguments) -> FunctionResult {
    let path = args.get1_str()?;

    match fs::metadata(path.as_ref()) {
        Ok(size) => Ok(DynamicValue::from(size.len() as i64)),
        Err(_) => Err(EvaluationError::IO(format!(
            "cannot access file metadata for {}",
            path
        ))),
    }
}

pub fn bytesize(args: BoundArguments) -> FunctionResult {
    let bytes = args.get1().try_as_usize()? as u64;
    let human_readable = ByteSize::b(bytes).display().si().to_string();

    Ok(DynamicValue::from(human_readable))
}

pub fn shlex_split(args: BoundArguments) -> FunctionResult {
    let string = args.get1_str()?;

    if let Some(splitted) = shlex::split(&string) {
        Ok(DynamicValue::from(
            splitted
                .into_iter()
                .map(DynamicValue::from)
                .collect::<Vec<_>>(),
        ))
    } else {
        Err(EvaluationError::Custom(format!(
            "could not split {:?}",
            args.get1()
        )))
    }
}

pub fn cmd(mut args: BoundArguments) -> FunctionResult {
    let (command_name_arg, command_args) = args.pop2();

    let command_name = command_name_arg.try_as_str()?;

    let mut command = Command::new(command_name.as_ref());

    for command_arg in command_args.try_as_list()? {
        command.arg(command_arg.try_as_str()?.as_ref());
    }

    if let Ok(mut output) = command.output() {
        if output.status.success() {
            let result = &mut output.stdout;
            result.truncate(result.trim_ascii_end().len());

            Ok(DynamicValue::from_owned_bytes(output.stdout))
        } else {
            Err(EvaluationError::Custom(format!(
                "\"{}\" failed!",
                command_name
            )))
        }
    } else {
        Err(EvaluationError::Custom(format!(
            "error while spawning \"{}\"",
            command_name
        )))
    }
}

pub fn shell(args: BoundArguments) -> FunctionResult {
    let pipeline = args.get1_str()?;

    let mut command = if cfg!(target_os = "windows") {
        let mut command = Command::new("cmd");
        command.args(["/C", pipeline.as_ref()]);
        command
    } else {
        let mut command = Command::new(std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string()));
        command.args(["-c", pipeline.as_ref()]);
        command
    };

    if let Ok(mut output) = command.output() {
        if output.status.success() {
            let result = &mut output.stdout;
            result.truncate(result.trim_ascii_end().len());

            Ok(DynamicValue::from_owned_bytes(output.stdout))
        } else {
            Err(EvaluationError::Custom(format!(
                "shell pipeline \"{}\" failed!",
                pipeline
            )))
        }
    } else {
        Err(EvaluationError::Custom(format!(
            "error while running shell pipeline \"{}\"",
            pipeline
        )))
    }
}

pub fn parse_json(args: BoundArguments) -> FunctionResult {
    let arg = args.get1();

    serde_json::from_slice(arg.try_as_bytes()?)
        .map_err(|_| EvaluationError::JSONParseError(format!("{:?}", args.get1())))
}

pub fn parse_py_literal(args: BoundArguments) -> FunctionResult {
    let parsed: py_literal::Value = args
        .get1_str()?
        .parse()
        .map_err(|err: py_literal::ParseError| EvaluationError::Custom(err.to_string()))?;

    fn map_to_dynamic_value(value: py_literal::Value) -> FunctionResult {
        Ok(match value {
            py_literal::Value::None => DynamicValue::None,
            py_literal::Value::Boolean(v) => DynamicValue::Boolean(v),
            py_literal::Value::Float(f) => DynamicValue::Float(f),
            py_literal::Value::Integer(bi) => match bi.try_into() {
                Ok(i) => DynamicValue::Integer(i),
                Err(err) => return Err(EvaluationError::Custom(err.to_string())),
            },
            py_literal::Value::Bytes(b) => DynamicValue::from_owned_bytes(b),
            py_literal::Value::String(s) => DynamicValue::from(s),
            py_literal::Value::List(l)
            | py_literal::Value::Tuple(l)
            | py_literal::Value::Set(l) => {
                let mut list = Vec::new();

                for item in l {
                    list.push(map_to_dynamic_value(item)?);
                }

                DynamicValue::from(list)
            }
            py_literal::Value::Dict(d) => {
                let mut dict = HashMap::new();

                for (key, value) in d {
                    dict.insert(
                        map_to_dynamic_value(key)?.try_as_str()?.into_owned(),
                        map_to_dynamic_value(value)?,
                    );
                }

                DynamicValue::from(dict)
            }
            py_literal::Value::Complex(c) => {
                DynamicValue::from(vec![DynamicValue::Float(c.re), DynamicValue::Float(c.im)])
            }
        })
    }

    map_to_dynamic_value(parsed)
}
