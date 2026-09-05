use sembla_ir::ValidatedModel;
use sembla_runtime::core::{ParamEnv, TableInit};

use crate::types::{CudaDeviceIdentity, CudaRunResult, CudaTickObservation, HashMode};
use crate::{CudaAvailability, CudaError, PhiloxCoordinate};

/// Feature-off request surface. It returns an explicit diagnostic and cannot
/// construct or hide a CPU executor.
#[derive(Debug)]
pub struct CudaBackend;

impl CudaBackend {
    /// Applies the same explicit availability gate exposed by the CUDA build.
    pub fn check_availability(availability: CudaAvailability) -> Result<(), CudaError> {
        availability.require()
    }

    pub fn new(
        _model: &ValidatedModel,
        _initial_tables: Vec<TableInit>,
        _params: &ParamEnv,
        _seed: u64,
        _hash_mode: HashMode,
    ) -> Result<Self, CudaError> {
        Err(CudaError::FeatureDisabled)
    }

    pub fn device_identity(&self) -> &CudaDeviceIdentity {
        unreachable!("feature-off CUDA backend cannot be constructed")
    }

    pub fn run_tick_observed(&mut self) -> Result<CudaTickObservation, CudaError> {
        Err(CudaError::FeatureDisabled)
    }

    pub fn philox_vectors(
        &self,
        _coordinates: &[PhiloxCoordinate],
    ) -> Result<Vec<[u32; 4]>, CudaError> {
        Err(CudaError::FeatureDisabled)
    }

    pub fn run(&mut self, _ticks: u32) -> Result<CudaRunResult, CudaError> {
        Err(CudaError::FeatureDisabled)
    }
}

#[cfg(test)]
#[path = "backend_stub_tests.rs"]
mod tests;
