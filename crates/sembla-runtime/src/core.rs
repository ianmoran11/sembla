//! Backend-neutral runtime contracts shared by CPU and accelerator backends.

pub use crate::error::EvalError;
pub use crate::observation::{
    device_observation_eligibility, DeviceObservationEligibility, DeviceViewEligibility,
    GroupedViewValue, ObservationValue, SummaryValue, ViewValue,
};
pub use crate::params::{ParamEnv, ParamOverride};
pub use crate::state::{
    ColumnData, ColumnInit, InputTable, Snapshot, StateError, StateStore, TableInit, WriteBuffer,
};
