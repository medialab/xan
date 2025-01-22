use csv::ByteRecord;

use super::error::{ConcretizationError, SpecifiedEvaluationError};
use super::interpreter::{concretize_expression, eval_expression, ConcreteExpr, EvaluationContext};
use super::parser::parse_named_expressions;

#[derive(Clone, Debug)]
enum MergeColumn {
    Existing(usize),
    New(usize),
}

impl MergeColumn {
    fn index(&self) -> usize {
        match self {
            Self::Existing(i) => *i,
            Self::New(i) => *i,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MergeProgram {
    exprs: Vec<(ConcreteExpr, MergeColumn)>,
    context: EvaluationContext,
    input_headers: ByteRecord,
    output_headers: ByteRecord,
    full_buffer: ByteRecord,
}

impl MergeProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let mut full_headers = csv::ByteRecord::new();
        let mut output_headers = headers.clone();

        for h in headers.iter() {
            full_headers.push_field(&[b"current_", h].concat());
        }

        let named_exprs = parse_named_expressions(code)
            .map_err(|_| ConcretizationError::ParseError(code.to_string()))?;

        let merge_columns = named_exprs
            .iter()
            .map(|(_, name)| {
                if let Some(index) = headers.iter().position(|h| h == name.as_bytes()) {
                    MergeColumn::Existing(index)
                } else {
                    full_headers.push_field(&[b"current_", name.as_bytes()].concat());
                    output_headers.push_field(name.as_bytes());
                    MergeColumn::New(output_headers.len() - 1)
                }
            })
            .collect::<Vec<_>>();

        for h in headers.iter() {
            full_headers.push_field(&[b"new_", h].concat());
        }

        let exprs = named_exprs
            .iter()
            .zip(merge_columns.into_iter())
            .map(|((expr, _), merge_column)| {
                concretize_expression(expr.clone(), &full_headers)
                    .map(|concrete_expr| (concrete_expr, merge_column))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            exprs,
            input_headers: headers.clone(),
            output_headers,
            context: EvaluationContext::new(&full_headers),
            full_buffer: ByteRecord::new(),
        })
    }

    pub fn headers(&self) -> &ByteRecord {
        &self.output_headers
    }

    pub fn allocate(&self, record: &ByteRecord) -> Vec<Vec<u8>> {
        let mut owned = Vec::with_capacity(self.output_headers.len());

        for cell in record.iter() {
            owned.push(cell.to_vec());
        }

        for (_, merge_column) in self.exprs.iter() {
            if matches!(merge_column, MergeColumn::New(_)) {
                owned.push(vec![]);
            }
        }

        owned
    }

    pub fn run_with_record(
        &mut self,
        index: usize,
        current_owned_record: &mut Vec<Vec<u8>>,
        new_record: &ByteRecord,
    ) -> Result<(), SpecifiedEvaluationError> {
        let working_record = &mut self.full_buffer;

        working_record.clear();

        for cell in current_owned_record.iter() {
            working_record.push_field(&cell);
        }

        working_record.extend(new_record.iter());

        let values = self
            .exprs
            .iter()
            .map(|(expr, _)| eval_expression(expr, Some(index), &working_record, &self.context))
            .collect::<Result<Vec<_>, _>>()?;

        for ((_, merge_column), value) in self.exprs.iter().zip(values.into_iter()) {
            current_owned_record[merge_column.index()] = value.serialize_as_bytes().to_vec();
        }

        Ok(())
    }
}
