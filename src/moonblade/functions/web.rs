use base64::prelude::*;
use bstr::ByteSlice;
use mime2ext::mime2ext;

use crate::urls::LRUStems;

use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArguments, DynamicValue};

use super::FunctionResult;

pub fn parse_dataurl(args: BoundArguments) -> FunctionResult {
    let bytes = args.get1().try_as_bytes()?;

    if !bytes.starts_with(b"data:") {
        return Err(EvaluationError::Custom(
            "data url does not start with \"data:\"".to_string(),
        ));
    }

    match bytes[5..].split_once_str(b",") {
        Some((spec, data)) => {
            if spec.ends_with(b";base64") {
                let mime = &spec[..spec.len() - 7];

                Ok(DynamicValue::from(vec![
                    DynamicValue::from(std::str::from_utf8(mime)?),
                    DynamicValue::from_owned_bytes(BASE64_STANDARD.decode(data).map_err(|_| {
                        EvaluationError::Custom("data url contains invalid base64".to_string())
                    })?),
                ]))
            } else {
                Err(EvaluationError::NotImplemented(
                    "url-encoded data url is not implemented yet".to_string(),
                ))
            }
        }
        None => Err(EvaluationError::Custom(
            "data url is misformatted".to_string(),
        )),
    }
}

pub fn mime_ext(args: BoundArguments) -> FunctionResult {
    let target = args.get1_str()?;

    match mime2ext(target) {
        Some(ext) => Ok(DynamicValue::from(ext)),
        None => Err(EvaluationError::Custom("unknown MIME type".to_string())),
    }
}

pub fn html_unescape(args: BoundArguments) -> FunctionResult {
    let string = args.get1_str()?;

    Ok(DynamicValue::from(html_escape::decode_html_entities(
        &string,
    )))
}

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
