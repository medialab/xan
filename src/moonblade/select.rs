use simd_csv::ByteRecord;

use super::error::{ConcretizationError, SpecifiedEvaluationError};
use super::interpreter::{concretize_expression, eval_expression, ConcreteExpr};
use super::parser::parse_named_expressions;
use super::types::HeadersIndex;

#[derive(Clone, Debug)]
pub struct SelectionProgram {
    exprs: Vec<(ConcreteExpr, String, bool)>,
    headers_index: HeadersIndex,
    mask: Vec<Option<usize>>,
    rest: Vec<usize>,
}

impl SelectionProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let headers_index = HeadersIndex::from_headers(headers);
        let mut mask = vec![None; headers.len()];
        let mut rest = vec![];

        let exprs = match parse_named_expressions(code) {
            Err(err) => return Err(ConcretizationError::ParseError(err)),
            Ok(parsed_exprs) => parsed_exprs
                .into_iter()
                .enumerate()
                .map(|(expr_i, (expr, expr_name))| {
                    concretize_expression(expr, headers, None).map(|c| {
                        let expr_name = expr_name.unwrap();

                        // TODO: what to do in case of plural?
                        let pos = headers_index.get_first_by_name(&expr_name);

                        if let Some(i) = pos {
                            mask[i] = Some(expr_i);
                        } else {
                            mask.push(None);
                            rest.push(expr_i);
                        }

                        (c, expr_name, pos.is_some())
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
        }?;

        Ok(Self {
            exprs,
            headers_index,
            mask,
            rest,
        })
    }

    pub fn len(&self) -> usize {
        self.exprs.len()
    }

    pub fn headers(&self) -> impl Iterator<Item = &[u8]> {
        self.exprs.iter().map(|(_, name, _)| name.as_bytes())
    }

    pub fn new_headers(&self) -> impl Iterator<Item = &[u8]> {
        self.exprs
            .iter()
            .filter_map(|(_, name, already_exists)| (!already_exists).then_some(name.as_bytes()))
    }

    pub fn has_something_to_overwrite(&self) -> bool {
        self.rest.len() < self.exprs.len()
    }

    pub fn extend_into(
        &self,
        index: usize,
        record: &ByteRecord,
        output_record: &mut ByteRecord,
    ) -> Result<(), SpecifiedEvaluationError> {
        for (expr, _, _) in self.exprs.iter() {
            let value = eval_expression(expr, Some(index), record, &self.headers_index)?;
            output_record.push_field(&value.serialize_as_bytes());
        }

        Ok(())
    }

    pub fn extend(
        &self,
        index: usize,
        record: &mut ByteRecord,
    ) -> Result<bool, SpecifiedEvaluationError> {
        let mut truthy = true;

        for (expr, _, _) in self.exprs.iter() {
            let value = eval_expression(expr, Some(index), record, &self.headers_index)?;
            truthy &= value.is_truthy();
            record.push_field(&value.serialize_as_bytes());
        }

        Ok(truthy)
    }

    pub fn overwrite(
        &self,
        index: usize,
        record: &mut ByteRecord,
    ) -> Result<(bool, ByteRecord), SpecifiedEvaluationError> {
        let mut new_record = ByteRecord::new();
        let mut truthy = true;

        for (expr_i_opt, cell) in self.mask.iter().copied().zip(record.iter()) {
            if let Some(expr_i) = expr_i_opt {
                let expr = &self.exprs[expr_i].0;

                let value = eval_expression(expr, Some(index), record, &self.headers_index)?;
                truthy &= value.is_truthy();
                new_record.push_field(&value.serialize_as_bytes());
            } else {
                new_record.push_field(cell);
            }
        }

        for expr_i in self.rest.iter().copied() {
            let expr = &self.exprs[expr_i].0;

            let value = eval_expression(expr, Some(index), record, &self.headers_index)?;
            truthy &= value.is_truthy();
            new_record.push_field(&value.serialize_as_bytes());
        }

        Ok((truthy, new_record))
    }
}
