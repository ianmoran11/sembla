//! Deterministic CPU evaluation and execution for Sembla simulations.

mod error;
mod eval;
mod executor;

pub use error::TickError;
pub use eval::{
    eval_column, eval_gather, eval_ref_column, eval_typed_ref_column, eval_typed_ref_gather,
    AggCache, EvalTable, RefColumn, ValueColumn,
};
pub use executor::{
    observe_grouped_views, observe_views, run, run_tick, run_tick_with_features,
    run_tick_with_features_timed, run_with_features, summarize, RunReport, SaturationWarning,
    TickPhaseDurations, TickReport, TimedTickReport,
};
pub use sembla_runtime::core::{
    EvalError, GroupedViewValue, ObservationValue, SummaryValue, ViewValue,
};

/// The version of the Sembla CPU crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_matches_package_version() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    }
}
