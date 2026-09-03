use crate::CudaError;

/// CUDA capability facts used to make backend selection explicit and testable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CudaAvailability {
    pub driver_library: bool,
    pub device_count: usize,
    pub nvrtc_library: bool,
}

impl CudaAvailability {
    /// Requires the one production CUDA path. This function never substitutes
    /// the CPU oracle or any other backend.
    pub fn require(self) -> Result<(), CudaError> {
        if !self.driver_library {
            return Err(CudaError::DriverMissing);
        }
        if self.device_count == 0 {
            return Err(CudaError::NoDevice);
        }
        if !self.nvrtc_library {
            return Err(CudaError::ToolkitMissing);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "availability_tests.rs"]
mod tests;
