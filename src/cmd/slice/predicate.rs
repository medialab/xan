use crate::CliResult;
use crate::moonblade::Program;

pub(super) enum Selection {
    Range { start: usize, end: usize },
    Indices(Vec<usize>),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RowDecision {
    pub yield_row: bool,
    pub stop: bool,
}

pub(super) struct RowPredicate {
    selection: Selection,
    start: Option<Program>,
    end: Option<Program>,
    has_started: bool,
    eligible_index: usize,
    next_index: usize,
    done: bool,
}

impl RowPredicate {
    pub fn new(mut selection: Selection, start: Option<Program>, end: Option<Program>) -> Self {
        let done = match &mut selection {
            Selection::Range { start, end } => start >= end,
            Selection::Indices(indices) => {
                indices.sort_unstable();
                indices.dedup();
                indices.is_empty()
            }
        };

        Self {
            selection,
            has_started: start.is_none(),
            start,
            end,
            eligible_index: 0,
            next_index: 0,
            done,
        }
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn process(
        &mut self,
        input_index: usize,
        record: &simd_csv::ByteRecord,
    ) -> CliResult<RowDecision> {
        if self.done {
            return Ok(RowDecision {
                yield_row: false,
                stop: true,
            });
        }

        if !self.has_started {
            if !self
                .start
                .as_ref()
                .unwrap()
                .run_with_record(input_index, record)?
                .is_truthy()
            {
                return Ok(RowDecision {
                    yield_row: false,
                    stop: false,
                });
            }

            self.has_started = true;
        }

        if let Some(program) = &self.end
            && program.run_with_record(input_index, record)?.is_truthy()
        {
            self.done = true;
            return Ok(RowDecision {
                yield_row: false,
                stop: true,
            });
        }

        let decision = match &self.selection {
            Selection::Range { start, end } => RowDecision {
                yield_row: self.eligible_index >= *start && self.eligible_index < *end,
                stop: self.eligible_index >= end - 1,
            },
            Selection::Indices(indices) => {
                let yield_row = indices[self.next_index] == self.eligible_index;

                if yield_row {
                    self.next_index += 1;
                }

                RowDecision {
                    yield_row,
                    stop: self.next_index == indices.len(),
                }
            }
        };

        self.eligible_index = self.eligible_index.saturating_add(1);
        self.done = decision.stop;

        Ok(decision)
    }
}

#[cfg(test)]
mod tests {
    use super::{RowDecision, RowPredicate, Selection};
    use crate::moonblade::Program;

    fn program(expression: &str) -> Program {
        Program::parse(expression, &simd_csv::ByteRecord::from(vec!["n"]), false).unwrap()
    }

    fn record(value: &str) -> simd_csv::ByteRecord {
        simd_csv::ByteRecord::from(vec![value])
    }

    fn range(start: usize, end: usize) -> Selection {
        Selection::Range { start, end }
    }

    fn decision(yield_row: bool, stop: bool) -> RowDecision {
        RowDecision { yield_row, stop }
    }

    fn selected(predicate: &mut RowPredicate, values: &[&str]) -> Vec<usize> {
        let mut selected = Vec::new();
        for (input_index, value) in values.iter().enumerate() {
            if predicate.is_done() {
                break;
            }
            let result = predicate.process(input_index, &record(value)).unwrap();
            if result.yield_row {
                selected.push(input_index);
            }
            if result.stop {
                break;
            }
        }
        selected
    }

    #[test]
    fn range_is_half_open_and_stops_after_last_yield() {
        let mut predicate = RowPredicate::new(range(1, 3), None, None);
        assert_eq!(
            predicate.process(0, &record("0")).unwrap(),
            decision(false, false)
        );
        assert_eq!(
            predicate.process(1, &record("1")).unwrap(),
            decision(true, false)
        );
        assert_eq!(
            predicate.process(2, &record("2")).unwrap(),
            decision(true, true)
        );
        assert!(predicate.is_done());
    }

    #[test]
    fn indices_are_normalized_and_keep_input_order() {
        let mut predicate = RowPredicate::new(Selection::Indices(vec![4, 1, 1, 0]), None, None);
        assert_eq!(
            selected(&mut predicate, &["0", "1", "2", "3", "4", "5"]),
            vec![0, 1, 4]
        );
        assert!(predicate.is_done());
    }

    #[test]
    fn empty_selections_do_not_evaluate_conditions() {
        for selection in [range(0, 0), range(3, 3), Selection::Indices(vec![])] {
            let mut predicate = RowPredicate::new(selection, Some(program("int(n)")), None);
            assert!(predicate.is_done());
            assert_eq!(
                predicate.process(0, &record("invalid")).unwrap(),
                decision(false, true)
            );
        }
    }

    #[test]
    fn start_latches_and_selection_is_relative_to_first_match() {
        let mut predicate = RowPredicate::new(range(1, 3), Some(program("n == 2")), None);
        assert_eq!(
            selected(&mut predicate, &["0", "1", "2", "0", "1", "2"]),
            vec![3, 4]
        );
    }

    #[test]
    fn end_condition_is_exclusive() {
        let mut predicate = RowPredicate::new(range(0, usize::MAX), None, Some(program("n >= 2")));
        assert_eq!(selected(&mut predicate, &["0", "1", "2", "3"]), vec![0, 1]);
        assert!(predicate.is_done());
    }

    #[test]
    fn conditions_and_indices_follow_documented_order() {
        let mut predicate = RowPredicate::new(
            Selection::Indices(vec![2, 0]),
            Some(program("n >= 2")),
            Some(program("n >= 5")),
        );
        assert_eq!(
            selected(&mut predicate, &["0", "1", "2", "3", "4", "5"]),
            vec![2, 4]
        );
    }

    #[test]
    fn end_condition_runs_before_numeric_selection() {
        let mut predicate = RowPredicate::new(range(3, 5), None, Some(program("n >= 2")));
        assert!(selected(&mut predicate, &["0", "1", "2", "3", "4"]).is_empty());
        assert!(predicate.is_done());
    }

    #[test]
    fn expressions_use_input_index_before_and_after_selection_skips() {
        let mut predicate = RowPredicate::new(
            range(1, 4),
            Some(program("row_index() >= 2")),
            Some(program("row_index() >= 4")),
        );
        assert_eq!(
            selected(&mut predicate, &["same", "same", "same", "same", "same"]),
            vec![3]
        );
        assert!(predicate.is_done());
    }

    #[test]
    fn matching_start_and_end_excludes_that_row() {
        let mut predicate = RowPredicate::new(
            range(0, 1),
            Some(program("n == 1")),
            Some(program("n == 1")),
        );
        assert_eq!(
            predicate.process(0, &record("0")).unwrap(),
            decision(false, false)
        );
        assert_eq!(
            predicate.process(1, &record("1")).unwrap(),
            decision(false, true)
        );
    }

    #[test]
    fn unmatched_start_does_not_evaluate_end() {
        let mut predicate =
            RowPredicate::new(range(0, 1), Some(program("false")), Some(program("int(n)")));
        assert_eq!(
            predicate.process(0, &record("invalid")).unwrap(),
            decision(false, false)
        );
        assert!(!predicate.is_done());
    }

    #[test]
    fn condition_errors_propagate() {
        let mut start = RowPredicate::new(range(0, 1), Some(program("int(n)")), None);
        assert!(start.process(0, &record("invalid")).is_err());
        let mut end = RowPredicate::new(range(0, 1), None, Some(program("int(n)")));
        assert!(end.process(0, &record("invalid")).is_err());
    }

    #[test]
    fn done_predicate_never_evaluates_another_row() {
        let mut predicate = RowPredicate::new(range(0, 1), None, Some(program("int(n) > 10")));
        assert_eq!(
            predicate.process(0, &record("1")).unwrap(),
            decision(true, true)
        );
        assert_eq!(
            predicate.process(1, &record("invalid")).unwrap(),
            decision(false, true)
        );
    }
}
