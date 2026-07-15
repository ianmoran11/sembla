//! Shared steady-state timing types and validation for the precision benchmark.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Required warmup ticks before collecting steady-state samples.
pub const WARMUP_TICKS: usize = 10;
/// Required measured ticks for every reported strategy.
pub const MEASURED_TICKS: usize = 100;
/// Fixed workload tick used for benchmark timing and accuracy readback.
pub const BENCHMARK_TICK: u32 = 7;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimingMethod {
    GpuTimestampQueries,
    SynchronizedWallClockFallback,
}

impl fmt::Display for TimingMethod {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GpuTimestampQueries => formatter.write_str("GPU timestamp queries"),
            Self::SynchronizedWallClockFallback => {
                formatter.write_str("synchronized wall-clock fallback")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct StageTiming {
    pub total_ms: f64,
    pub reduce_ms: f64,
    pub argmin_ms: f64,
    pub rows_per_second: f64,
    pub warmup_ticks: usize,
    pub measured_ticks: usize,
    pub method: TimingMethod,
}

/// Builds a validated report from independently collected timing samples.
///
/// The total median is measured directly; it is never reconstructed by adding
/// stage medians. The map stage remains represented in `total_ms` even though
/// PRD 0005 calls out only reduction and argmin as separate columns.
pub fn summarize(
    rows: u32,
    warmup_ticks: usize,
    measured_ticks: usize,
    method: TimingMethod,
    total_samples: &mut [f64],
    reduce_samples: &mut [f64],
    argmin_samples: &mut [f64],
) -> Result<StageTiming, String> {
    validate_config(warmup_ticks, measured_ticks)?;
    for (name, samples) in [
        ("total", &*total_samples),
        ("reduce", &*reduce_samples),
        ("argmin", &*argmin_samples),
    ] {
        if samples.len() != measured_ticks {
            return Err(format!(
                "{name} timing collected {} samples, expected {measured_ticks}",
                samples.len()
            ));
        }
    }

    let total_ms = median(total_samples)?;
    let reduce_ms = median(reduce_samples)?;
    let argmin_ms = median(argmin_samples)?;
    let rows_per_second = f64::from(rows) / (total_ms / 1000.0);
    if !rows_per_second.is_finite() || rows_per_second <= 0.0 {
        return Err(format!(
            "rows/sec must be finite and positive, got {rows_per_second}"
        ));
    }

    Ok(StageTiming {
        total_ms,
        reduce_ms,
        argmin_ms,
        rows_per_second,
        warmup_ticks,
        measured_ticks,
        method,
    })
}

pub fn validate_config(warmup_ticks: usize, measured_ticks: usize) -> Result<(), String> {
    if warmup_ticks < WARMUP_TICKS {
        return Err(format!(
            "benchmark requires at least {WARMUP_TICKS} warmup ticks, got {warmup_ticks}"
        ));
    }
    if measured_ticks < MEASURED_TICKS {
        return Err(format!(
            "benchmark requires at least {MEASURED_TICKS} measured ticks, got {measured_ticks}"
        ));
    }
    Ok(())
}

/// Sorts the input and returns its median after rejecting empty, non-finite, or
/// non-positive timing samples.
pub fn median(values: &mut [f64]) -> Result<f64, String> {
    if values.is_empty() {
        return Err("cannot compute the median of an empty sample".to_owned());
    }
    if let Some(value) = values
        .iter()
        .find(|value| !value.is_finite() || **value <= 0.0)
    {
        return Err(format!(
            "timing samples must be finite and positive, got {value}"
        ));
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    let result = if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    };
    if result.is_finite() && result > 0.0 {
        Ok(result)
    } else {
        Err(format!(
            "timing median must be finite and positive, got {result}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_handles_odd_and_even_sample_counts() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]).unwrap(), 2.0);
        assert_eq!(median(&mut [4.0, 1.0, 3.0, 2.0]).unwrap(), 2.5);
    }

    #[test]
    fn timing_validation_rejects_short_or_invalid_samples() {
        assert!(validate_config(WARMUP_TICKS, MEASURED_TICKS).is_ok());
        assert!(validate_config(WARMUP_TICKS - 1, MEASURED_TICKS).is_err());
        assert!(validate_config(WARMUP_TICKS, MEASURED_TICKS - 1).is_err());
        assert!(median(&mut []).is_err());
        assert!(median(&mut [0.0]).is_err());
        assert!(median(&mut [f64::NAN]).is_err());
    }
}
