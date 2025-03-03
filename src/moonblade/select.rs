use csv::ByteRecord;

use super::error::{ConcretizationError, SpecifiedEvaluationError};
use super::interpreter::{concretize_expression, eval_expression, ConcreteExpr, EvaluationContext};
use super::parser::parse_named_expressions;

#[derive(Clone)]
pub struct SelectionProgram {
    exprs: Vec<(ConcreteExpr, String)>,
    context: EvaluationContext,
}

impl SelectionProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let exprs = match parse_named_expressions(code) {
            Err(_) => return Err(ConcretizationError::ParseError(code.to_string())),
            Ok(parsed_exprs) => parsed_exprs
                .into_iter()
                .map(|e| concretize_expression(e.0.clone(), headers, None).map(|c| (c, e.1)))
                .collect::<Result<Vec<_>, _>>(),
        }?;

        Ok(Self {
            exprs,
            context: EvaluationContext::new(headers),
        })
    }

    pub fn headers(&self) -> impl Iterator<Item = &[u8]> {
        self.exprs.iter().map(|(_, name)| name.as_bytes())
    }

    pub fn run_with_record(
        &self,
        index: usize,
        record: &ByteRecord,
    ) -> Result<ByteRecord, SpecifiedEvaluationError> {
        let mut output_record = csv::ByteRecord::new();

        for (expr, _) in self.exprs.iter() {
            let value = eval_expression(expr, Some(index), record, &self.context)?;
            output_record.push_field(&value.serialize_as_bytes());
        }

        Ok(output_record)
    }
}
