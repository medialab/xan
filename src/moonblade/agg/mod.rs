pub mod aggregators;
mod program;
mod stats;

pub use aggregators::CovarianceWelford;
pub use program::{AggregationProgram, GroupAggregationProgram};
pub use stats::Stats;
