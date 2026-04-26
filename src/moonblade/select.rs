use simd_csv::ByteRecord;

use super::error::{ConcretizationError, EvaluationError, SpecifiedEvaluationError};
use super::interpreter::{concretize_expression, ConcreteExpr, EvaluationContext};
use super::parser::{parse_named_expressions, ExprName};
use super::types::{DynamicValue, HeadersIndex};

#[derive(Clone, Debug)]
pub struct SelectionProgram {
    exprs: Vec<(ConcreteExpr, ExprName, bool)>,
    headers_index: HeadersIndex,
    mask: Vec<Option<usize>>,
    rest: Vec<usize>,
}

impl SelectionProgram {
    pub fn parse(
        code: &str,
        headers: &ByteRecord,
        headless: bool,
    ) -> Result<Self, ConcretizationError> {
        let headers_index = HeadersIndex::new(headers, headless);
        let mut mask = vec![None; headers.len()];
        let mut rest = vec![];

        let exprs = match parse_named_expressions(code) {
            Err(err) => return Err(ConcretizationError::ParseError(err)),
            Ok(parsed_exprs) => parsed_exprs
                .into_iter()
                .enumerate()
                .map(|(expr_i, (expr, expr_name))| {
                    concretize_expression(expr, &headers_index, None).map(|c| match &expr_name {
                        ExprName::Singular(name) => {
                            let pos = headers_index.first_by_name(name);

                            if let Some(i) = pos {
                                mask[i] = Some(expr_i);
                            } else {
                                rest.push(expr_i);
                            }

                            (c, expr_name, pos.is_some())
                        }
                        ExprName::Plural(_names) => {
                            rest.push(expr_i);

                            (c, expr_name, false)
                        }
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

    pub fn headers(&self) -> impl Iterator<Item = &[u8]> {
        self.exprs
            .iter()
            .flat_map(|(_, expr_name, _)| expr_name.iter())
            .map(|name| name.as_bytes())
    }

    pub fn new_headers(&self) -> impl Iterator<Item = &[u8]> {
        self.exprs
            .iter()
            .filter_map(|(_, expr_name, already_exists)| {
                (!already_exists).then_some(match expr_name {
                    ExprName::Singular(name) => name.as_bytes(),
                    _ => unimplemented!(),
                })
            })
    }

    pub fn has_any_plural_expr(&self) -> bool {
        self.exprs
            .iter()
            .any(|(_, expr_name, _)| matches!(expr_name, ExprName::Plural(_)))
    }

    pub fn has_something_to_overwrite(&self) -> bool {
        self.rest.len() < self.exprs.len()
    }

    pub fn run_with_record<'a>(
        &'a self,
        row_index: usize,
        col_index: usize,
        record: &'a ByteRecord,
    ) -> impl Iterator<Item = Result<DynamicValue, SpecifiedEvaluationError>> + 'a {
        self.exprs.iter().map(move |(expr, _, _)| {
            EvaluationContext::new_with_col_index(
                Some(row_index),
                Some(col_index),
                record,
                &self.headers_index,
            )
            .evaluate(expr)
        })
    }

    pub fn extend_into(
        &self,
        row_index: usize,
        record: &ByteRecord,
        output_record: &mut ByteRecord,
    ) -> Result<bool, SpecifiedEvaluationError> {
        let mut truthy = false;

        for (expr, expr_name, _) in self.exprs.iter() {
            let value = EvaluationContext::new(Some(row_index), record, &self.headers_index)
                .evaluate(expr)?;

            truthy |= value.is_truthy();

            match expr_name {
                ExprName::Singular(_) => {
                    value.push_field_to_record(output_record);
                }
                ExprName::Plural(names) => {
                    let mut count: usize = 0;

                    for sub_value in value.flat_iter() {
                        sub_value.push_field_to_record(output_record);
                        count += 1;
                    }

                    if names.len() != count {
                        return Err(
                            EvaluationError::plural_clause_misalignment(names, count).anonymous()
                        );
                    }
                }
            }
        }

        Ok(truthy)
    }

    pub fn extend(
        &self,
        row_index: usize,
        record: &mut ByteRecord,
    ) -> Result<bool, SpecifiedEvaluationError> {
        let mut truthy = false;

        for (expr, expr_name, _) in self.exprs.iter() {
            let value = EvaluationContext::new(Some(row_index), record, &self.headers_index)
                .evaluate(expr)?;
            truthy |= value.is_truthy();

            match expr_name {
                ExprName::Singular(_) => {
                    value.push_field_to_record(record);
                }
                ExprName::Plural(names) => {
                    let mut count: usize = 0;

                    for sub_value in value.flat_iter() {
                        sub_value.push_field_to_record(record);
                        count += 1;
                    }

                    if names.len() != count {
                        return Err(
                            EvaluationError::plural_clause_misalignment(names, count).anonymous()
                        );
                    }
                }
            }
        }

        Ok(truthy)
    }

    // NOTE: I could make this work for ExprName::Plural, but to avoid allocating an
    // aligned Vec<Option<DynamicValue>> for each row, the function will need to take
    // a mutable reference to it (no need to even clear it).
    // I won't do it as of yet because it is a very niche use-case and would complexify
    // multithreaded code significantly, requiring mutable thread-local state.
    // What's more, this would mean you need to have owned DynamicValue and any
    // future optimization related to raw column copy could be hampered by this pattern.
    pub fn overwrite(
        &self,
        row_index: usize,
        record: &mut ByteRecord,
    ) -> Result<(bool, ByteRecord), SpecifiedEvaluationError> {
        let mut new_record = ByteRecord::new();
        let mut truthy = false;

        for (expr_i_opt, cell) in self.mask.iter().copied().zip(record.iter()) {
            if let Some(expr_i) = expr_i_opt {
                let expr = &self.exprs[expr_i].0;

                let value = EvaluationContext::new(Some(row_index), record, &self.headers_index)
                    .evaluate(expr)?;
                truthy |= value.is_truthy();
                value.push_field_to_record(&mut new_record);
            } else {
                new_record.push_field(cell);
            }
        }

        for expr_i in self.rest.iter().copied() {
            let expr = &self.exprs[expr_i].0;

            let value = EvaluationContext::new(Some(row_index), record, &self.headers_index)
                .evaluate(expr)?;
            truthy |= value.is_truthy();
            value.push_field_to_record(&mut new_record);
        }

        Ok((truthy, new_record))
    }
}
