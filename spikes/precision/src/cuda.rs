//! Optional CUDA `double` reference.
//!
//! The public status API always exists. With `--features cuda` but no `nvcc`,
//! build.rs leaves `sembla_cuda_toolkit` unset and this module contains no FFI
//! references, producing the required documented no-op.

use std::{error::Error, fmt};

use crate::{
    f64_mirror::run_f64_mirror,
    fp64::Fp64Throughput,
    gpu::{accuracy_workload_config, ACCURACY_TICK},
    native_f64::{score_native, NativeF64AccuracyReport, NativeF64Device, NativeF64TickResult},
    oracle::run_oracle,
    workload::Workload,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CudaStatus {
    FeatureDisabled,
    ToolkitAbsent,
    DeviceUnavailable(String),
    Available(Fp64Throughput),
}

impl CudaStatus {
    #[must_use]
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available(_))
    }
}

impl fmt::Display for CudaStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FeatureDisabled => formatter.write_str("cuda: feature-disabled"),
            Self::ToolkitAbsent => formatter.write_str("cuda: toolkit-absent"),
            Self::DeviceUnavailable(reason) => {
                write!(formatter, "cuda: device-unavailable; reason={reason}")
            }
            Self::Available(profile) => write!(formatter, "cuda: available; {profile}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaError(String);

impl CudaError {
    #[cfg(all(feature = "cuda", sembla_cuda_toolkit))]
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for CudaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for CudaError {}

pub enum CudaF64Outcome {
    Unavailable(CudaStatus),
    Executed(NativeF64AccuracyReport),
}

impl fmt::Display for CudaF64Outcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(status) => status.fmt(formatter),
            Self::Executed(report) => write!(formatter, "cuda {report}"),
        }
    }
}

#[must_use]
pub fn cuda_status() -> CudaStatus {
    status_impl()
}

#[cfg(not(feature = "cuda"))]
fn status_impl() -> CudaStatus {
    CudaStatus::FeatureDisabled
}

#[cfg(all(feature = "cuda", not(sembla_cuda_toolkit)))]
fn status_impl() -> CudaStatus {
    CudaStatus::ToolkitAbsent
}

#[cfg(all(feature = "cuda", sembla_cuda_toolkit))]
fn status_impl() -> CudaStatus {
    ffi::probe()
}

/// Runs and scores a CUDA tick when both toolkit and device are present.
/// Feature/toolkit/device absence is an ordinary output row, never an error.
pub fn run_cuda_f64_accuracy_smoke() -> Result<CudaF64Outcome, CudaError> {
    let status = cuda_status();
    let CudaStatus::Available(profile) = status else {
        return Ok(CudaF64Outcome::Unavailable(status));
    };
    run_available(profile)
}

#[cfg(all(feature = "cuda", sembla_cuda_toolkit))]
fn run_available(profile: Fp64Throughput) -> Result<CudaF64Outcome, CudaError> {
    let workload = Workload::generate(accuracy_workload_config())
        .map_err(|error| CudaError::new(format!("CUDA accuracy workload failed: {error}")))?;
    let oracle = run_oracle(&workload, ACCURACY_TICK);
    let mirror = run_f64_mirror(&workload, ACCURACY_TICK);
    let output = ffi::run_tick(&workload, ACCURACY_TICK)?;
    let device = NativeF64Device {
        adapter_name: profile.device_name.clone(),
        backend: "CUDA".to_owned(),
        vendor_id: 0x10de,
        device_id: 0,
        throughput: profile,
    };
    let report = score_native(
        device,
        workload.config.rows,
        workload.config.groups,
        ACCURACY_TICK,
        &output,
        &oracle,
        &mirror,
    );
    Ok(CudaF64Outcome::Executed(report))
}

#[cfg(not(all(feature = "cuda", sembla_cuda_toolkit)))]
fn run_available(_profile: Fp64Throughput) -> Result<CudaF64Outcome, CudaError> {
    unreachable!("CUDA cannot be available without a compiled toolkit")
}

#[cfg(all(feature = "cuda", sembla_cuda_toolkit))]
mod ffi {
    use std::{ffi::CStr, os::raw::c_char};

    use super::*;

    unsafe extern "C" {
        fn sembla_cuda_f64_probe(
            name: *mut c_char,
            name_capacity: u32,
            fp32_to_fp64_ratio: *mut i32,
        ) -> i32;
        fn sembla_cuda_f64_error_string(code: i32) -> *const c_char;
        fn sembla_cuda_f64_run_tick(
            rows: u32,
            groups: u32,
            seed: u64,
            beta: f64,
            dt: f64,
            tick: u32,
            offsets: *const u32,
            employers: *const u32,
            health: *const u32,
            weights: *const f64,
            sums: *mut f64,
            winners: *mut u32,
            fired: *mut u32,
        ) -> i32;
    }

    pub(super) fn probe() -> CudaStatus {
        let mut name = [0_i8; 256];
        let mut ratio = 0_i32;
        // SAFETY: both output buffers are valid for the supplied capacities.
        let code =
            unsafe { sembla_cuda_f64_probe(name.as_mut_ptr(), name.len() as u32, &mut ratio) };
        if code != 0 {
            return CudaStatus::DeviceUnavailable(error_message(code));
        }
        // SAFETY: the C wrapper always NUL-terminates the fixed output buffer.
        let device_name = unsafe { CStr::from_ptr(name.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        CudaStatus::Available(Fp64Throughput::from_cuda_ratio(
            device_name,
            ratio.max(0) as u32,
        ))
    }

    pub(super) fn run_tick(
        workload: &Workload,
        tick: u32,
    ) -> Result<NativeF64TickResult, CudaError> {
        let mut segmented_sums = vec![0.0_f64; workload.config.groups as usize];
        let mut winner_entity_ids = vec![u32::MAX; workload.config.groups as usize];
        let mut fired_flags = vec![0_u32; workload.config.rows as usize];
        // SAFETY: every pointer references a contiguous slice of the exact
        // rows/groups length passed to the C wrapper and remains live for the call.
        let code = unsafe {
            sembla_cuda_f64_run_tick(
                workload.config.rows,
                workload.config.groups,
                workload.config.seed,
                workload.config.beta,
                workload.config.dt,
                tick,
                workload.group_offsets.as_ptr(),
                workload.employer.as_ptr(),
                workload.health.as_ptr(),
                workload.weight.as_ptr(),
                segmented_sums.as_mut_ptr(),
                winner_entity_ids.as_mut_ptr(),
                fired_flags.as_mut_ptr(),
            )
        };
        if code != 0 {
            return Err(CudaError::new(format!(
                "CUDA native f64 tick failed: {}",
                error_message(code)
            )));
        }
        Ok(NativeF64TickResult {
            segmented_sums,
            winner_entity_ids,
            fired_flags,
        })
    }

    fn error_message(code: i32) -> String {
        // SAFETY: CUDA returns a process-lifetime string for every error code.
        let pointer = unsafe { sembla_cuda_f64_error_string(code) };
        if pointer.is_null() {
            return format!("CUDA error {code}");
        }
        // SAFETY: non-null CUDA error strings are NUL-terminated.
        let text = unsafe { CStr::from_ptr(pointer) }.to_string_lossy();
        format!("{text} (code {code})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuda_absence_is_a_reported_no_op() {
        let status = cuda_status();
        println!("{status}");
        let outcome = run_cuda_f64_accuracy_smoke().unwrap();
        println!("{outcome}");
        match outcome {
            CudaF64Outcome::Unavailable(unavailable) => assert!(!unavailable.is_available()),
            CudaF64Outcome::Executed(report) => report.assert_expected().unwrap(),
        }
    }
}
