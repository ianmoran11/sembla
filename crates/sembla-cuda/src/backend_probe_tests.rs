use super::classify_device_count;
use crate::CudaError;
use cudarc::driver::{sys::CUresult, DriverError};

#[test]
fn production_device_probe_maps_cuda_no_device() {
    assert_eq!(
        classify_device_count(Err(DriverError(CUresult::CUDA_ERROR_NO_DEVICE))),
        Err(CudaError::NoDevice)
    );
}
