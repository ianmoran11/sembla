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

#[cfg(feature = "cuda")]
pub use backend::{CudaBackend, CudaRunResult, HashMode};
#[cfg(not(feature = "cuda"))]
pub use backend_stub::{CudaBackend, CudaRunResult, HashMode};

/// The version of the Sembla CUDA crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
