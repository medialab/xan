use csv::ByteRecord;

use super::error::{ConcretizationError, SpecifiedEvaluationError};
use super::interpreter::{concretize_expression, eval_expression, ConcreteExpr, EvaluationContext};
use super::parser::parse_named_expressions;

#[derive(Clone)]
enum MergeColumn {
    Existing(usize),
    New(String),
}

#[derive(Clone)]
pub struct MergeProgram {
    exprs: Vec<(ConcreteExpr, MergeColumn)>,
    context: EvaluationContext,
    headers: ByteRecord,
    full_buffer: ByteRecord,
}

impl MergeProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let mut full_headers = csv::ByteRecord::new();

        for h in headers {
            full_headers.push_field(&[b"current_", h].concat());
        }
        for h in headers {
            full_headers.push_field(&[b"next_", h].concat());
        }

        let exprs = match parse_named_expressions(code) {
            Err(_) => return Err(ConcretizationError::ParseError(code.to_string())),
            Ok(parsed_exprs) => parsed_exprs
                .into_iter()
                .map(|e| concretize_expression(e.0.clone(), &full_headers).map(|c| (c, e.1)))
                .collect::<Result<Vec<_>, _>>(),
        }?;

        Ok(Self {
            exprs: exprs
                .into_iter()
                .map(|(expr, name)| {
                    (
                        expr,
                        if let Some(index) = headers.iter().position(|h| h == name.as_bytes()) {
                            MergeColumn::Existing(index)
                        } else {
                            MergeColumn::New(name)
                        },
                    )
                })
                .collect(),
            headers: headers.clone(),
            context: EvaluationContext::new(&full_headers),
            full_buffer: ByteRecord::new(),
        })
    }

    pub fn headers(&self) -> ByteRecord {
        let mut record = self.headers.clone();

        for (_, kind) in self.exprs.iter() {
            if let MergeColumn::New(name) = kind {
                record.push_field(name.as_bytes());
            }
        }

        record
    }

    pub fn run_with_record(
        &mut self,
        index: usize,
        current_record: &mut ByteRecord,
        next_record: &ByteRecord,
    ) -> Result<(), SpecifiedEvaluationError> {
        let working_record = &mut self.full_buffer;

        working_record.clear();
        working_record.extend(current_record.iter());
        working_record.extend(next_record.iter());

        // let mut output_record = csv::ByteRecord::new();

        // for (expr, _) in self.exprs.iter() {
        //     let value = eval_expression(expr, Some(index), record, &self.context)?;
        //     output_record.push_field(&value.serialize_as_bytes());
        // }

        // Ok(output_record)

        Ok(())
    }
}
