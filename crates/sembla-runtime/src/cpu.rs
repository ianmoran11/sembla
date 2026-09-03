//! Deterministic CPU execution and observation entry points.

pub use crate::error::{EvalError, TickError};
pub use crate::eval::{
    eval_column, eval_gather, eval_ref_column, eval_typed_ref_column, eval_typed_ref_gather,
    AggCache, EvalTable, RefColumn, ValueColumn,
};
pub use crate::executor::{
    observe_grouped_views, observe_views, run, run_tick, run_tick_with_features,
    run_tick_with_features_timed, run_with_features, summarize, RunReport, SaturationWarning,
    TickPhaseDurations, TickReport, TimedTickReport,
};
pub use crate::observation::{GroupedViewValue, ObservationValue, SummaryValue, ViewValue};
