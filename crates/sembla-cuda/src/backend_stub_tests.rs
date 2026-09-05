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
