use csv::ByteRecord;

use super::error::{ConcretizationError, SpecifiedEvaluationError};
use super::Program;

#[derive(Clone, Debug)]
pub struct ChooseProgram {
    header_len: usize,
    program: Program,
    full_buffer: ByteRecord,
}

impl ChooseProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let mut full_headers = csv::ByteRecord::new();

        for h in headers.iter() {
            full_headers.push_field(&[b"current_", h].concat());
        }
        for h in headers.iter() {
            full_headers.push_field(&[b"new_", h].concat());
        }

        Ok(Self {
            header_len: headers.len(),
            program: Program::parse(code, &full_headers)?,
            full_buffer: ByteRecord::new(),
        })
    }

    pub fn prepare_current_record(&mut self, current_record: &ByteRecord) {
        let working_record = &mut self.full_buffer;
        working_record.clear();
        working_record.extend(current_record.iter());
    }

    pub fn run_with_record(
        &mut self,
        index: usize,
        new_record: &ByteRecord,
    ) -> Result<bool, SpecifiedEvaluationError> {
        let working_record = &mut self.full_buffer;

        working_record.truncate(self.header_len);
        working_record.extend(new_record.iter());

        let value = self.program.run_with_record(index, working_record)?;

        Ok(value.is_truthy())
    }
}
