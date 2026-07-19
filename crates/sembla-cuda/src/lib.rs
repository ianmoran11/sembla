//! Native-`f64` CUDA execution for validated Sembla models.
//!
//! Pure kernel generation is always compiled. The `cuda` build feature gates
//! only CUDA/NVRTC bindings, so the default workspace remains toolkit-free.

mod availability;
mod codegen;
mod error;

#[cfg(feature = "cuda")]
mod backend;
#[cfg(not(feature = "cuda"))]
mod backend_stub;

pub use availability::CudaAvailability;
pub use codegen::{generate, GeneratedCuda, DUMP_ENV};
pub use error::CudaError;

/// One coordinate in the shared PRD-0003 Philox namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhiloxCoordinate {
    pub seed: u64,
    pub tick: u32,
    pub rule_id: u32,
    pub entity_id: u32,
    pub draw_index: u32,
}

impl PhiloxCoordinate {
    pub const fn new(seed: u64, tick: u32, rule_id: u32, entity_id: u32, draw_index: u32) -> Self {
        Self {
            seed,
            tick,
            rule_id,
            entity_id,
            draw_index,
        }
    }
}

#[cfg(feature = "cuda")]
pub use backend::{CudaBackend, CudaRunResult, HashMode};
#[cfg(not(feature = "cuda"))]
pub use backend_stub::{CudaBackend, CudaRunResult, HashMode};

/// The version of the Sembla CUDA crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
