//! Unified PRD-0005 throughput and accuracy orchestration.

use std::{collections::BTreeMap, time::SystemTime};

use crate::{
    cuda::{
        run_cuda_f64_accuracy_smoke, run_cuda_f64_benchmark, CudaBenchmarkOutcome, CudaF64Outcome,
    },
    fp64::Fp64Throughput,
    gpu::{
        run_accuracy_smoke, score_strategy, FastMathStatus, PortableRunner, PortableStrategy,
        StrategyAccuracy, DF64_MAX_REDUCTION_RELATIVE_ERROR, DF64_REDUCTION_ERROR_FACTOR,
    },
    native_f64::{
        run_native_f64_accuracy_smoke, NativeF64Outcome, NativeF64Runner, NativeF64RunnerInit,
        NativeF64TickResult,
    },
    oracle::{run_oracle, OracleResult},
    probe_default_adapter,
    results::{
        AccuracyMetrics, Fp64Metadata, HardwareMetadata, MachineRun, StrategyRow, StrategyStatus,
        StrictMathMetadata, WorkloadMetadata,
    },
    timing::{BENCHMARK_TICK, MEASURED_TICKS, WARMUP_TICKS},
    workload::{Workload, WorkloadConfig},
    DEFAULT_GROUPS, DEFAULT_ROWS,
};

const REDUCTION_CHOICE: &str = "deterministic fixed-order two-pass reduction, no atomics (Level A)";
const CONTESTED_SELECTOR: &str = "entity_id % 10 == 5, keyed by employer";

/// Runs the complete benchmark workload once and returns a four-row machine
/// result ready for durable assembly.
pub async fn run_benchmark() -> Result<MachineRun, String> {
    let probe = probe_default_adapter(DEFAULT_ROWS, DEFAULT_GROUPS)
        .await
        .map_err(|error| format!("adapter probe failed: {error}"))?;
    let config = WorkloadConfig::with_size(probe.sizing.rows, probe.sizing.groups);
    let workload = Workload::generate(config)
        .map_err(|error| format!("benchmark workload generation failed: {error}"))?;

    // This is the only oracle evaluation for the measured workload. Every
    // answered strategy below is scored against this exact value.
    let oracle = run_oracle(&workload, BENCHMARK_TICK);

    let portable = PortableRunner::new(&workload)
        .await
        .map_err(|error| format!("portable runner initialization failed: {error}"))?;
    let strict_math = portable
        .fast_math_status()
        .map_err(|error| format!("portable arithmetic probe failed: {error}"))?;

    let f32_timing = portable
        .benchmark(PortableStrategy::F32)
        .map_err(|error| format!("f32 benchmark failed: {error}"))?;
    let f32_output = portable
        .dispatch_tick(PortableStrategy::F32, BENCHMARK_TICK)
        .map_err(|error| format!("f32 accuracy tick failed: {error}"))?;
    let f32_accuracy = score_strategy(&f32_output, &oracle);
    validate_finite_accuracy(&f32_accuracy, "f32")?;

    let df64_timing = portable
        .benchmark(PortableStrategy::Df64)
        .map_err(|error| format!("double-single benchmark failed: {error}"))?;
    let df64_output = portable
        .dispatch_tick(PortableStrategy::Df64, BENCHMARK_TICK)
        .map_err(|error| format!("double-single accuracy tick failed: {error}"))?;
    let df64_accuracy = score_strategy(&df64_output, &oracle);
    validate_finite_accuracy(&df64_accuracy, "double-single")?;
    validate_portable_thresholds(&f32_accuracy, &df64_accuracy, false)?;

    let mut strategies = vec![
        answered_portable_row(
            "f32",
            f32_timing,
            &f32_accuracy,
            &oracle,
            "Portable baseline. It is the cheapest arithmetic path; its reported reduction and winner errors are the reference that double-single must improve.",
        ),
        answered_portable_row(
            "double-single",
            df64_timing,
            &df64_accuracy,
            &oracle,
            if strict_math.trustworthy_on_adapter {
                "Portable double-single passed the strict-math behavior probe and the PRD-0002 reduction/winner thresholds on this adapter."
            } else {
                "Portable double-single ran, but this backend cannot guarantee the strict no-contraction/no-reassociation mode; keep the numbers as an explicitly untrusted portability finding rather than silently treating them as Level B evidence."
            },
        ),
    ];

    let mut best_fp64 = Fp64Throughput::from_model_name(probe.name.clone());

    match NativeF64Runner::initialize(&workload)
        .await
        .map_err(|error| format!("native f64 wgpu initialization failed: {error}"))?
    {
        NativeF64RunnerInit::Unsupported(status) => strategies.push(unanswered_row(
            "native f64 (wgpu)",
            format!("unanswered on this adapter: {status}"),
        )),
        NativeF64RunnerInit::Ready(runner) => {
            let timing = runner
                .benchmark()
                .map_err(|error| format!("native f64 wgpu benchmark failed: {error}"))?;
            let output = runner
                .dispatch_tick(BENCHMARK_TICK)
                .map_err(|error| format!("native f64 wgpu accuracy tick failed: {error}"))?;
            let accuracy = score_native_output(&output, &oracle)?;
            require_zero_native_winner_mismatch(&accuracy, "native f64 (wgpu)")?;
            let profile = runner.profile().throughput.clone();
            best_fp64 = profile.clone();
            strategies.push(answered_native_row(
                "native f64 (wgpu)",
                timing,
                accuracy,
                &profile,
            ));
        }
    }

    match run_cuda_f64_benchmark(&workload)
        .map_err(|error| format!("native f64 CUDA benchmark failed: {error}"))?
    {
        CudaBenchmarkOutcome::Unavailable(status) => strategies.push(unanswered_row(
            "native f64 (CUDA)",
            format!("unanswered on this adapter: {status}"),
        )),
        CudaBenchmarkOutcome::Executed(result) => {
            let accuracy = score_native_output(&result.output, &oracle)?;
            require_zero_native_winner_mismatch(&accuracy, "native f64 (CUDA)")?;
            best_fp64 = result.profile.clone();
            strategies.push(answered_native_row(
                "native f64 (CUDA)",
                result.timing,
                accuracy,
                &result.profile,
            ));
        }
    }

    let machine_key = machine_key(&probe.name)?;
    let infrastructure = std::env::vars()
        .filter_map(|(name, value)| {
            name.strip_prefix("SEMBLA_INFRA_")
                .map(|short| (short.to_ascii_lowercase().replace('_', "-"), value))
        })
        .collect::<BTreeMap<_, _>>();

    Ok(MachineRun {
        machine_key,
        generated_at: generated_at(),
        hardware: HardwareMetadata {
            adapter_name: probe.name,
            backend: probe.backend,
            device_type: probe.device_type,
            driver: probe.driver,
            driver_info: probe.driver_info,
            shader_f64: probe.shader_f64,
            fp64: fp64_metadata(&best_fp64),
            strict_math: strict_math_metadata(&strict_math),
        },
        workload: WorkloadMetadata {
            requested_rows: probe.sizing.requested_rows,
            requested_groups: probe.sizing.requested_groups,
            actual_rows: workload.config.rows,
            actual_groups: workload.config.groups,
            downscale_reason: probe.sizing.downscale_reason.unwrap_or_else(|| {
                "none; the full requested workload fits the sizing limits".to_owned()
            }),
            contested_key_selector: CONTESTED_SELECTOR.to_owned(),
            benchmark_tick: BENCHMARK_TICK,
            warmup_ticks: WARMUP_TICKS,
            measured_ticks: MEASURED_TICKS,
            beta: workload.config.beta,
            dt: workload.config.dt,
        },
        strategies,
        infrastructure,
    })
}

/// One correctness tick at the frozen PRD-0002 scale for every strategy that is
/// available on the current machine. Unsupported native paths are honest no-ops.
pub async fn run_regression_guard() -> Result<(), String> {
    let portable = run_accuracy_smoke()
        .await
        .map_err(|error| format!("portable accuracy guard failed: {error}"))?;
    validate_finite_accuracy(&portable.f32, "f32 guard")?;
    validate_finite_accuracy(&portable.df64, "double-single guard")?;
    portable
        .assert_numerical_thresholds()
        .map_err(|error| format!("portable accuracy guard failed: {error}"))?;

    match run_native_f64_accuracy_smoke()
        .await
        .map_err(|error| format!("native f64 wgpu guard failed: {error}"))?
    {
        NativeF64Outcome::Unsupported(_) => {}
        NativeF64Outcome::Executed(report) => report
            .assert_expected()
            .map_err(|error| format!("native f64 wgpu guard failed: {error}"))?,
    }

    match run_cuda_f64_accuracy_smoke()
        .map_err(|error| format!("native f64 CUDA guard failed: {error}"))?
    {
        CudaF64Outcome::Unavailable(_) => {}
        CudaF64Outcome::Executed(report) => report
            .assert_expected()
            .map_err(|error| format!("native f64 CUDA guard failed: {error}"))?,
    }
    Ok(())
}

fn answered_portable_row(
    strategy: &str,
    timing: crate::timing::StageTiming,
    accuracy: &StrategyAccuracy,
    oracle: &OracleResult,
    verdict: &str,
) -> StrategyRow {
    StrategyRow {
        strategy: strategy.to_owned(),
        reduction_choice: REDUCTION_CHOICE.to_owned(),
        verdict: verdict.to_owned(),
        status: StrategyStatus::Answered {
            timing,
            accuracy: AccuracyMetrics {
                reduction_max_relative_error: accuracy.reduction_relative_error.max,
                reduction_mean_relative_error: accuracy.reduction_relative_error.mean,
                winner_mismatch_fraction: accuracy.winner_mismatch_rate,
                order_sensitive_groups: oracle.order_sensitive_group_count,
            },
        },
    }
}

fn answered_native_row(
    strategy: &str,
    timing: crate::timing::StageTiming,
    accuracy: AccuracyMetrics,
    profile: &Fp64Throughput,
) -> StrategyRow {
    let extrapolation = if profile.permits_full_rate_extrapolation() {
        "This is full-rate fp64 hardware, so full-rate extrapolation is allowed."
    } else {
        "This is rate-limited fp64 hardware; the result is pessimistic and full-rate extrapolation is refused."
    };
    StrategyRow {
        strategy: strategy.to_owned(),
        reduction_choice: REDUCTION_CHOICE.to_owned(),
        verdict: format!(
            "Native binary64 matched every oracle argmin winner. Device `{}` is classified `{}` from {}; {extrapolation}",
            profile.device_name, profile.class, profile.evidence
        ),
        status: StrategyStatus::Answered { timing, accuracy },
    }
}

fn unanswered_row(strategy: &str, reason: String) -> StrategyRow {
    StrategyRow {
        strategy: strategy.to_owned(),
        reduction_choice: REDUCTION_CHOICE.to_owned(),
        verdict: format!("{reason}. No throughput or accuracy number was fabricated."),
        status: StrategyStatus::Unanswered { reason },
    }
}

fn score_native_output(
    output: &NativeF64TickResult,
    oracle: &OracleResult,
) -> Result<AccuracyMetrics, String> {
    if output.segmented_sums.len() != oracle.segmented_sums.len()
        || output.winner_entity_ids.len() != oracle.winner_entity_ids.len()
    {
        return Err("native output dimensions do not match the shared oracle".to_owned());
    }
    let mut maximum = 0.0_f64;
    let mut sum = 0.0_f64;
    for (actual, expected) in output.segmented_sums.iter().zip(&oracle.segmented_sums) {
        let relative = if *expected == 0.0 {
            (actual - expected).abs()
        } else {
            (actual - expected).abs() / expected.abs()
        };
        if !relative.is_finite() {
            return Err("native reduction produced a non-finite relative error".to_owned());
        }
        maximum = maximum.max(relative);
        sum += relative;
    }
    let mismatches = output
        .winner_entity_ids
        .iter()
        .zip(&oracle.winner_entity_ids)
        .filter(|(actual, expected)| actual != expected)
        .count();
    Ok(AccuracyMetrics {
        reduction_max_relative_error: maximum,
        reduction_mean_relative_error: sum / oracle.segmented_sums.len() as f64,
        winner_mismatch_fraction: mismatches as f64 / oracle.winner_entity_ids.len() as f64,
        order_sensitive_groups: oracle.order_sensitive_group_count,
    })
}

fn validate_finite_accuracy(accuracy: &StrategyAccuracy, name: &str) -> Result<(), String> {
    for (metric, value) in [
        (
            "maximum reduction relative error",
            accuracy.reduction_relative_error.max,
        ),
        (
            "mean reduction relative error",
            accuracy.reduction_relative_error.mean,
        ),
        ("winner mismatch fraction", accuracy.winner_mismatch_rate),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(format!("{name} {metric} is invalid: {value}"));
        }
    }
    Ok(())
}

fn validate_portable_thresholds(
    f32: &StrategyAccuracy,
    df64: &StrategyAccuracy,
    require_strict_winner_improvement: bool,
) -> Result<(), String> {
    let winner_failed = if require_strict_winner_improvement {
        df64.winner_mismatch_rate >= f32.winner_mismatch_rate
    } else {
        df64.winner_mismatch_rate > f32.winner_mismatch_rate
    };
    if winner_failed {
        return Err(format!(
            "double-single winner mismatch rate {} does not satisfy the f32 baseline {}",
            df64.winner_mismatch_rate, f32.winner_mismatch_rate
        ));
    }
    // Numerical accuracy is never waived merely because a backend lacks an
    // exposed strict-math switch. The trust bit remains a reporting caveat,
    // while broken reduction arithmetic still blocks RESULTS.md generation.
    if df64.reduction_relative_error.max > DF64_MAX_REDUCTION_RELATIVE_ERROR {
        return Err(format!(
            "double-single max reduction error {} exceeds {}",
            df64.reduction_relative_error.max, DF64_MAX_REDUCTION_RELATIVE_ERROR
        ));
    }
    let max_limit = f32.reduction_relative_error.max * DF64_REDUCTION_ERROR_FACTOR;
    let mean_limit = f32.reduction_relative_error.mean * DF64_REDUCTION_ERROR_FACTOR;
    if df64.reduction_relative_error.max > max_limit
        || df64.reduction_relative_error.mean > mean_limit
    {
        return Err(format!(
            "double-single reduction errors ({}, {}) exceed {}x f32 ({}, {})",
            df64.reduction_relative_error.max,
            df64.reduction_relative_error.mean,
            DF64_REDUCTION_ERROR_FACTOR,
            f32.reduction_relative_error.max,
            f32.reduction_relative_error.mean
        ));
    }
    Ok(())
}

fn require_zero_native_winner_mismatch(
    accuracy: &AccuracyMetrics,
    strategy: &str,
) -> Result<(), String> {
    if accuracy.winner_mismatch_fraction == 0.0 {
        Ok(())
    } else {
        Err(format!(
            "{strategy} has non-zero winner mismatch fraction {}",
            accuracy.winner_mismatch_fraction
        ))
    }
}

fn fp64_metadata(profile: &Fp64Throughput) -> Fp64Metadata {
    Fp64Metadata {
        gpu_model: profile.device_name.clone(),
        class: profile.class.to_string(),
        fp32_to_fp64_ratio: profile.fp32_to_fp64_ratio,
        evidence: profile.evidence.clone(),
        full_rate_extrapolation: profile.permits_full_rate_extrapolation(),
    }
}

fn strict_math_metadata(status: &FastMathStatus) -> StrictMathMetadata {
    StrictMathMetadata {
        backend_supported: status.strict_math_backend_supported,
        requested: status.strict_math_requested,
        fma_contraction_observed: status.fma_contraction_observed,
        reassociation_observed: status.reassociation_observed,
        residuals_preserved: status.df64_residuals_preserved,
        trustworthy: status.trustworthy_on_adapter,
    }
}

fn machine_key(adapter_name: &str) -> Result<String, String> {
    if let Ok(value) = std::env::var("SEMBLA_MACHINE_KIND") {
        if matches!(value.as_str(), "development" | "nvidia") {
            return Ok(value);
        }
        return Err(format!(
            "SEMBLA_MACHINE_KIND must be development or nvidia, got {value}"
        ));
    }
    let normalized = adapter_name.to_ascii_lowercase();
    let model_token = normalized
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|token| {
            matches!(
                token,
                "a100" | "h100" | "h200" | "gh200" | "v100" | "t4" | "l4" | "a10" | "a10g"
            )
        });
    let nvidia_name = ["nvidia", "tesla", "geforce", "quadro"]
        .iter()
        .any(|brand| normalized.contains(brand))
        || model_token;
    if nvidia_name {
        Ok("nvidia".to_owned())
    } else {
        Ok("development".to_owned())
    }
}

fn generated_at() -> String {
    let seconds = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    format!("unix-seconds:{seconds}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn unified_accuracy_guard_covers_all_available_strategies() {
        pollster::block_on(super::run_regression_guard()).unwrap();
    }
}
