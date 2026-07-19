use sembla_ir::ValidatedModel;
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::state::{StateStore, TableInit};

use crate::{CudaAvailability, CudaError, PhiloxCoordinate};

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

#[derive(Clone, Debug)]
pub struct CudaTickObservation {
    pub tick: u32,
    pub state: StateStore,
    pub fired_per_box: Vec<(String, Vec<(u32, usize)>)>,
    pub deferred_per_resource_table: Vec<(String, usize)>,
}

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
mod tests {
    use super::CudaBackend;
    use crate::CudaError;

    #[test]
    fn feature_off_request_fails_explicitly() {
        let error = std::mem::size_of::<CudaBackend>();
        assert_eq!(error, 0);
        assert_eq!(
            CudaError::FeatureDisabled.to_string(),
            "cuda backend unavailable: crate built without the 'cuda' feature"
        );
    }
}
