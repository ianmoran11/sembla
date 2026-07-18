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
mod tests {
    use super::CudaAvailability;
    use crate::CudaError;

    #[test]
    fn no_device_has_the_frozen_diagnostic_and_never_falls_back() {
        let error = CudaAvailability {
            driver_library: true,
            device_count: 0,
            nvrtc_library: true,
        }
        .require()
        .unwrap_err();
        assert_eq!(error, CudaError::NoDevice);
        assert_eq!(
            error.to_string(),
            "cuda backend unavailable: no CUDA device found"
        );
    }

    #[test]
    fn missing_driver_and_toolkit_are_distinct() {
        assert_eq!(
            CudaAvailability {
                driver_library: false,
                device_count: 0,
                nvrtc_library: false,
            }
            .require(),
            Err(CudaError::DriverMissing)
        );
        assert_eq!(
            CudaAvailability {
                driver_library: true,
                device_count: 1,
                nvrtc_library: false,
            }
            .require(),
            Err(CudaError::ToolkitMissing)
        );
    }
}
