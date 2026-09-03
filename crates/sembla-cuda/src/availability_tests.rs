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
