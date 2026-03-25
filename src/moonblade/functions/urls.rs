use crate::urls::LRUStems;

use super::FunctionResult;
use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArguments, DynamicValue};

pub fn urljoin(args: BoundArguments) -> FunctionResult {
    let mut url = args.get(0).unwrap().try_as_url()?;
    let addendum = args.get(1).unwrap().try_as_str()?;

    url = url
        .join(&addendum)
        .map_err(|_| EvaluationError::Custom("invalid url part to join".to_string()))?;

    // TODO: canonicalize
    Ok(DynamicValue::from(url.to_string()))
}

pub fn lru(args: BoundArguments) -> FunctionResult {
    let tagged_url = args.get1().try_as_tagged_url()?;

    Ok(DynamicValue::from(LRUStems::from(&tagged_url).to_string()))
}
