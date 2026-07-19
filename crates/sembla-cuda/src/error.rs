use std::error::Error;
use std::fmt;

/// A deterministic CUDA construction, compilation, or execution failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CudaError {
    FeatureDisabled,
    DriverMissing,
    NoDevice,
    ToolkitMissing,
    Codegen(String),
    InvalidInput(String),
    Compilation(String),
    Driver(String),
    DeviceExecution(String),
    Dump(String),
}

impl fmt::Display for CudaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FeatureDisabled => {
                f.write_str("cuda backend unavailable: crate built without the 'cuda' feature")
            }
            Self::DriverMissing => {
                f.write_str("cuda backend unavailable: CUDA driver library not found")
            }
            Self::NoDevice => f.write_str("cuda backend unavailable: no CUDA device found"),
            Self::ToolkitMissing => {
                f.write_str("cuda backend unavailable: NVRTC library not found")
            }
            Self::Codegen(message) => write!(f, "cuda code generation failed: {message}"),
            Self::InvalidInput(message) => write!(f, "cuda backend input is invalid: {message}"),
            Self::Compilation(message) => write!(f, "cuda model compilation failed: {message}"),
            Self::Driver(message) => write!(f, "cuda driver operation failed: {message}"),
            Self::DeviceExecution(message) => {
                write!(f, "cuda model execution failed: {message}")
            }
            Self::Dump(message) => write!(f, "cuda source dump failed: {message}"),
        }
    }
}

impl Error for CudaError {}
