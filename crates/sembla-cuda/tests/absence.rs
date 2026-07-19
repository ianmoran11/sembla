use sembla_cuda::{CudaAvailability, CudaBackend, CudaError};

#[test]
fn requesting_cuda_without_a_device_has_the_frozen_diagnostic() {
    let error = CudaBackend::check_availability(CudaAvailability {
        driver_library: true,
        device_count: 0,
        nvrtc_library: true,
    })
    .unwrap_err();
    assert_eq!(error, CudaError::NoDevice);
    assert_eq!(
        error.to_string(),
        "cuda backend unavailable: no CUDA device found"
    );
}

#[cfg(not(feature = "cuda"))]
#[test]
fn feature_off_constructor_never_substitutes_the_cpu_oracle() {
    let source = include_str!("../../../examples/two_state.json");
    let model = sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap();
    let params = sembla_runtime::eval::ParamEnv::defaults(&model);
    let error = CudaBackend::new(
        &model,
        Vec::new(),
        &params,
        0,
        sembla_cuda::HashMode::FinalOnly,
    )
    .unwrap_err();
    assert_eq!(error, CudaError::FeatureDisabled);
}
