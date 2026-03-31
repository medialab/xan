use lazy_static::lazy_static;
use paltoquet::tokenizers::FingerprintTokenizer;

use crate::moonblade::types::{BoundArguments, DynamicValue};

use super::FunctionResult;

lazy_static! {
    static ref FINGERPRINT_TOKENIZER: FingerprintTokenizer = FingerprintTokenizer::default();
}

pub fn fingerprint(args: BoundArguments) -> FunctionResult {
    let string = args.get1().try_as_str()?;

    Ok(DynamicValue::from(
        FINGERPRINT_TOKENIZER.key(string.as_ref()),
    ))
}

pub fn apply_unidecode(args: BoundArguments) -> FunctionResult {
    let arg = args.get1_str()?;

    Ok(DynamicValue::from(unidecode::unidecode(&arg)))
}
