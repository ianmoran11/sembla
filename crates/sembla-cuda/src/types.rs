//! Shared plain-data types for the CUDA host surface.
//!
//! These types are compiled with or without the `cuda` feature so that the
//! real backend (`backend.rs`) and the feature-off request surface
//! (`backend_stub.rs`) expose one definition of each type instead of mirrored
//! copies. Behavioural types (NVRTC compilation, device execution) remain
//! feature-gated; only the data contracts live here.

use sembla_runtime::state::StateStore;

/// Which state hashes a CUDA run records.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HashMode {
    #[default]
    FinalOnly,
    EveryTick,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaRunResult {
    pub final_state_hash: [u8; 32],
    pub per_tick_state_hashes: Vec<[u8; 32]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaDeviceIdentity {
    pub gpu_model: String,
    pub driver_version: String,
}

/// One tick of CUDA execution observed on the host: the mirrored state plus
/// the same fired/deferred shapes the CPU `TickReport` exposes.
#[derive(Clone, Debug)]
pub struct CudaTickObservation {
    pub tick: u32,
    pub state: StateStore,
    pub fired_per_box: Vec<(String, Vec<(u32, usize)>)>,
    pub deferred_per_resource_table: Vec<(String, usize)>,
}
