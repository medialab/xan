pub mod aggregators;
mod program;
mod stats;

pub use aggregators::{CovarianceWelford, Welford};
pub use program::{AggregationProgram, GroupAggregationProgram};
pub use stats::Stats;
