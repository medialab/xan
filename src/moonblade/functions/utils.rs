use rand::Rng;
use regex::Regex;
use uuid::Uuid;

use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArguments, DynamicValue};

use super::FunctionResult;

pub fn md5(args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(format!(
        "{:x}",
        md5::compute(args.get1().try_as_bytes()?)
    )))
}

pub fn uuid(_args: BoundArguments) -> FunctionResult {
    let id = Uuid::new_v4().to_string();

    Ok(DynamicValue::from(id))
}

pub fn random(_args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(rand::rng().random::<f64>()))
}

pub fn type_of(mut args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(args.pop1().type_of()))
}

pub fn err(args: BoundArguments) -> FunctionResult {
    let arg = args.get1_str()?;

    Err(EvaluationError::Custom(arg.to_string()))
}

pub fn parse_regex(args: BoundArguments) -> FunctionResult {
    let string = args.get1_str()?;

    Ok(DynamicValue::from(Regex::new(&string).map_err(|_| {
        EvaluationError::Custom(format!("could not parse \"{}\" as regex", string))
    })?))
}
