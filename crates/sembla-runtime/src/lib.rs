//! Deterministic runtime for Sembla simulations.

pub mod core;
#[doc(hidden)]
pub mod engine;
mod error;
mod observation;
mod params;
pub mod population;
pub mod prior;
pub mod rng;
pub mod state;
pub mod state_artifact;

/// The version of the Sembla runtime crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
