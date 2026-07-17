//! Unified PRD-0005 throughput and accuracy orchestration.

use std::{collections::BTreeMap, time::SystemTime};

use crate::{
    cuda::{
        run_cuda_f64_accuracy_smoke, run_cuda_f64_benchmark, CudaBenchmarkOutcome, CudaF64Outcome,
    },
    f64_mirror::{run_f64_mirror, F64MirrorResult},
    fp64::Fp64Throughput,
    gpu::{
        accuracy_workload_config, score_strategy, FastMathStatus, PortableRunner, PortableStrategy,
        StrategyAccuracy, ACCURACY_TICK, DF64_MAX_REDUCTION_RELATIVE_ERROR,
        DF64_REDUCTION_ERROR_FACTOR,
    },
    native_f64::{
        run_native_f64_accuracy_smoke, NativeF64Outcome, NativeF64Runner, NativeF64RunnerInit,
        NativeF64TickResult,
    },
    oracle::{run_oracle, OracleResult},
    probe_default_adapter,
    results::{
        AccuracyMetrics, Fp64Metadata, GuardStatus, HardwareMetadata, MachineRun, StrategyRow,
        StrategyStatus, StrictMathMetadata, WorkloadMetadata,
    },
    timing::{BENCHMARK_TICK, MEASURED_TICKS, WARMUP_TICKS},
    workload::{Workload, WorkloadConfig},
    DEFAULT_GROUPS, DEFAULT_ROWS,
};

const REDUCTION_CHOICE: &str = "deterministic fixed-order two-pass reduction, no atomics (Level A)";
const CONTESTED_SELECTOR: &str = "entity_id % 10 == 5, keyed by employer";

/// Runs the complete benchmark workload once and returns a four-row machine
/// result ready for durable assembly.
pub async fn run_benchmark(guards: BTreeMap<String, GuardStatus>) -> Result<MachineRun, String> {
    let probe = probe_default_adapter(DEFAULT_ROWS, DEFAULT_GROUPS)
        .await
        .map_err(|error| format!("adapter probe failed: {error}"))?;
    let config = WorkloadConfig::with_size(probe.sizing.rows, probe.sizing.groups);
    let workload = Workload::generate(config)
        .map_err(|error| format!("benchmark workload generation failed: {error}"))?;

    // This is the only oracle evaluation for the measured workload. Every
    // answered strategy below is scored against this exact value.
    let oracle = run_oracle(&workload, BENCHMARK_TICK);
    // The fixed-tree mirror is supplemental native-path evidence, not a second
    // oracle. Compute it lazily at most once and reuse it for wgpu and CUDA.
    let mut native_mirror = None;

    let unavailable_strict_math = || FastMathStatus {
        adapter_name: probe.name.clone(),
        backend: probe.backend.clone(),
        strict_math_requested: false,
        strict_math_backend_supported: false,
        fma_contraction_observed: false,
        reassociation_observed: false,
        df64_residuals_preserved: false,
        trustworthy_on_adapter: false,
    };
    let (strict_math, strict_math_error, f32_measurement, df64_measurement) =
        match PortableRunner::new(&workload).await {
            Err(error) => {
                let reason = format!("portable runner initialization failed: {error}");
                (
                    unavailable_strict_math(),
                    Some(reason.clone()),
                    Err(reason.clone()),
                    Err(reason),
                )
            }
            Ok(portable) => {
                let (strict_math, strict_math_error) = match portable.fast_math_status() {
                    Ok(status) => (status, None),
                    Err(error) => (
                        unavailable_strict_math(),
                        Some(format!("portable arithmetic probe failed: {error}")),
                    ),
                };
                (
                    strict_math,
                    strict_math_error,
                    measure_portable_strategy(&portable, PortableStrategy::F32, &oracle, "f32"),
                    measure_portable_strategy(
                        &portable,
                        PortableStrategy::Df64,
                        &oracle,
                        "double-single",
                    ),
                )
            }
        };

    let f32_row = match &f32_measurement {
        Ok((timing, accuracy)) => answered_portable_row(
            "f32",
            *timing,
            accuracy,
            &oracle,
            "Portable baseline. It is the cheapest arithmetic path; its reported reduction, winner, and fired errors are the reference for candidate qualification.",
        ),
        Err(error) => unanswered_row("f32", error.clone()),
    };
    let df64_row = match &df64_measurement {
        Err(error) => unanswered_row("double-single", error.clone()),
        Ok((timing, accuracy)) => {
            let df64_verdict = match &f32_measurement {
                Err(error) => format!(
                    "Portable double-single produced usable measurements but is unqualified because the f32 performance/accuracy baseline is unavailable: {error}."
                ),
                Ok((_, f32_accuracy)) => {
                    match validate_portable_thresholds(f32_accuracy, accuracy, false) {
                        Err(error) => format!(
                            "Portable double-single produced usable measurements but is unqualified: {error}."
                        ),
                        Ok(()) if strict_math_error.is_some() => format!(
                            "Portable double-single produced usable measurements but is unqualified: {}.",
                            strict_math_error.as_deref().unwrap_or("strict arithmetic probe failed")
                        ),
                        Ok(()) if strict_math.trustworthy_on_adapter => {
                            "Portable double-single passed the strict-math behavior probe and the PRD-0002 reduction/winner/fired thresholds on this adapter.".to_owned()
                        }
                        Ok(()) => {
                            "Portable double-single produced usable measurements but is unqualified: this backend cannot guarantee the strict no-contraction/no-reassociation mode.".to_owned()
                        }
                    }
                }
            };
            answered_portable_row("double-single", *timing, accuracy, &oracle, &df64_verdict)
        }
    };
    let mut strategies = vec![f32_row, df64_row];

    let mut best_fp64 = Fp64Throughput::from_model_name(probe.name.clone());

    match NativeF64Runner::initialize(&workload).await {
        Err(error) => strategies.push(unanswered_row(
            "native f64 (wgpu)",
            format!("native f64 wgpu initialization failed: {error}"),
        )),
        Ok(NativeF64RunnerInit::Unsupported(status)) => strategies.push(unanswered_row(
            "native f64 (wgpu)",
            format!("unanswered on this adapter: {status}"),
        )),
        Ok(NativeF64RunnerInit::Ready(runner)) => {
            let profile = runner.profile().throughput.clone();
            best_fp64 = profile.clone();
            let measured = runner
                .benchmark()
                .map_err(|error| format!("native f64 wgpu benchmark failed: {error}"))
                .and_then(|timing| {
                    runner
                        .dispatch_tick(BENCHMARK_TICK)
                        .map(|output| (timing, output))
                        .map_err(|error| format!("native f64 wgpu accuracy tick failed: {error}"))
                });
            match measured {
                Err(error) => strategies.push(unanswered_row("native f64 (wgpu)", error)),
                Ok((timing, output)) => {
                    let mirror = native_mirror
                        .get_or_insert_with(|| run_f64_mirror(&workload, BENCHMARK_TICK));
                    match score_native_output(&output, &oracle, mirror) {
                        Err(error) => strategies.push(unanswered_row(
                            "native f64 (wgpu)",
                            format!("native f64 wgpu scoring failed: {error}"),
                        )),
                        Ok(accuracy) => strategies.push(answered_native_row(
                            "native f64 (wgpu)",
                            timing,
                            accuracy,
                            &profile,
                        )),
                    }
                }
            }
        }
    }

    match run_cuda_f64_benchmark(&workload) {
        Err(error) => strategies.push(unanswered_row(
            "native f64 (CUDA)",
            format!("native f64 CUDA benchmark failed: {error}"),
        )),
        Ok(CudaBenchmarkOutcome::Unavailable(status)) => strategies.push(unanswered_row(
            "native f64 (CUDA)",
            format!("unanswered on this adapter: {status}"),
        )),
        Ok(CudaBenchmarkOutcome::Executed(result)) => {
            best_fp64 = result.profile.clone();
            let mirror =
                native_mirror.get_or_insert_with(|| run_f64_mirror(&workload, BENCHMARK_TICK));
            match score_native_output(&result.output, &oracle, mirror) {
                Err(error) => strategies.push(unanswered_row(
                    "native f64 (CUDA)",
                    format!("native f64 CUDA scoring failed: {error}"),
                )),
                Ok(accuracy) => strategies.push(answered_native_row(
                    "native f64 (CUDA)",
                    result.timing,
                    accuracy,
                    &result.profile,
                )),
            }
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
        guards,
        infrastructure,
    })
}

/// One correctness tick at the frozen PRD-0002 scale for every strategy that is
/// available on the current machine. Unsupported native paths are honest no-ops.
pub async fn run_regression_guard() -> BTreeMap<String, GuardStatus> {
    let mut guards = BTreeMap::new();
    match Workload::generate(accuracy_workload_config()) {
        Err(error) => record_shared_portable_guard_failure(
            &mut guards,
            format!("portable accuracy workload generation failed: {error}"),
        ),
        Ok(workload) => {
            let oracle = run_oracle(&workload, ACCURACY_TICK);
            match PortableRunner::new(&workload).await {
                Err(error) => record_shared_portable_guard_failure(
                    &mut guards,
                    format!("portable accuracy runner initialization failed: {error}"),
                ),
                Ok(runner) => match runner.assert_philox_known_answers() {
                    Err(error) => record_shared_portable_guard_failure(
                        &mut guards,
                        format!("portable shared Philox guard failed: {error}"),
                    ),
                    Ok(()) => {
                        let strict_math = runner
                            .fast_math_status()
                            .map_err(|error| format!("strict arithmetic probe failed: {error}"));
                        let f32_accuracy = match runner
                            .dispatch_tick(PortableStrategy::F32, ACCURACY_TICK)
                        {
                            Err(error) => {
                                guards.insert(
                                    "f32".to_owned(),
                                    GuardStatus::Failed {
                                        reason: format!("f32 guard dispatch failed: {error}"),
                                    },
                                );
                                None
                            }
                            Ok(output) => {
                                let accuracy = score_strategy(&output, &oracle);
                                let validation = validate_finite_accuracy(&accuracy, "f32 guard");
                                guards.insert("f32".to_owned(), guard_status(validation.clone()));
                                validation.ok().map(|()| accuracy)
                            }
                        };

                        let df64 = match runner.dispatch_tick(PortableStrategy::Df64, ACCURACY_TICK)
                        {
                            Err(error) => GuardStatus::Failed {
                                reason: format!("double-single guard dispatch failed: {error}"),
                            },
                            Ok(output) => {
                                let accuracy = score_strategy(&output, &oracle);
                                guard_status(
                                    validate_finite_accuracy(&accuracy, "double-single guard")
                                        .and_then(|()| {
                                            let baseline =
                                                f32_accuracy.as_ref().ok_or_else(|| {
                                                    "double-single guard has no valid f32 baseline"
                                                        .to_owned()
                                                })?;
                                            validate_portable_thresholds(baseline, &accuracy, true)
                                        })
                                        .and_then(|()| match &strict_math {
                                            Err(error) => Err(error.clone()),
                                            Ok(status) if status.trustworthy_on_adapter => Ok(()),
                                            Ok(status) => Err(format!(
                                                "strict arithmetic probe is untrustworthy: {status}"
                                            )),
                                        }),
                                )
                            }
                        };
                        guards.insert("double-single".to_owned(), df64);
                    }
                },
            }
        }
    }

    let wgpu = match run_native_f64_accuracy_smoke().await {
        Err(error) => GuardStatus::Failed {
            reason: format!("native f64 wgpu guard execution failed: {error}"),
        },
        Ok(NativeF64Outcome::Unsupported(status)) => GuardStatus::Unavailable {
            reason: status.to_string(),
        },
        Ok(NativeF64Outcome::Executed(report)) => guard_status(report.assert_expected()),
    };
    guards.insert("native f64 (wgpu)".to_owned(), wgpu);

    let cuda = match run_cuda_f64_accuracy_smoke() {
        Err(error) => GuardStatus::Failed {
            reason: format!("native f64 CUDA guard execution failed: {error}"),
        },
        Ok(CudaF64Outcome::Unavailable(status)) => GuardStatus::Unavailable {
            reason: status.to_string(),
        },
        Ok(CudaF64Outcome::Executed(report)) => guard_status(report.assert_expected()),
    };
    guards.insert("native f64 (CUDA)".to_owned(), cuda);
    guards
}

fn record_shared_portable_guard_failure(
    guards: &mut BTreeMap<String, GuardStatus>,
    reason: String,
) {
    guards.insert(
        "f32".to_owned(),
        GuardStatus::Failed {
            reason: reason.clone(),
        },
    );
    guards.insert("double-single".to_owned(), GuardStatus::Failed { reason });
}

fn guard_status(result: Result<(), String>) -> GuardStatus {
    match result {
        Ok(()) => GuardStatus::Passed,
        Err(reason) => GuardStatus::Failed { reason },
    }
}

fn measure_portable_strategy(
    runner: &PortableRunner,
    strategy: PortableStrategy,
    oracle: &OracleResult,
    label: &str,
) -> Result<(crate::timing::StageTiming, StrategyAccuracy), String> {
    let timing = runner
        .benchmark(strategy)
        .map_err(|error| format!("{label} benchmark failed: {error}"))?;
    let output = runner
        .dispatch_tick(strategy, BENCHMARK_TICK)
        .map_err(|error| format!("{label} accuracy tick failed: {error}"))?;
    let accuracy = score_strategy(&output, oracle);
    validate_finite_accuracy(&accuracy, label)?;
    Ok((timing, accuracy))
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
                fired_mismatch_count: Some(accuracy.fired_mismatch_count),
                unexplained_arithmetic_mirror_difference_count: None,
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
    let diagnostic = match validate_native_diagnostics(&accuracy, strategy) {
        Ok(()) => "Native binary64 matched every oracle argmin winner and fired flag, with zero unexplained fixed-tree arithmetic-mirror differences.".to_owned(),
        Err(error) => format!(
            "Native binary64 produced usable measurements but is unqualified: {error}."
        ),
    };
    StrategyRow {
        strategy: strategy.to_owned(),
        reduction_choice: REDUCTION_CHOICE.to_owned(),
        verdict: format!(
            "{diagnostic} Device `{}` is classified `{}` from {}; {extrapolation}",
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
    mirror: &F64MirrorResult,
) -> Result<AccuracyMetrics, String> {
    if output.segmented_sums.len() != oracle.segmented_sums.len()
        || output.segmented_sums.len() != mirror.segmented_sums.len()
        || output.winner_entity_ids.len() != oracle.winner_entity_ids.len()
        || output.fired_flags.len() != oracle.fired_flags.len()
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
    let fired_mismatches = output
        .fired_flags
        .iter()
        .zip(&oracle.fired_flags)
        .filter(|(actual, expected)| actual != expected)
        .count();
    let unexplained_arithmetic_mirror_differences = output
        .segmented_sums
        .iter()
        .zip(&mirror.segmented_sums)
        .filter(|(actual, expected)| actual.to_bits() != expected.to_bits())
        .count();
    Ok(AccuracyMetrics {
        reduction_max_relative_error: maximum,
        reduction_mean_relative_error: sum / oracle.segmented_sums.len() as f64,
        winner_mismatch_fraction: mismatches as f64 / oracle.winner_entity_ids.len() as f64,
        order_sensitive_groups: oracle.order_sensitive_group_count,
        fired_mismatch_count: Some(fired_mismatches),
        unexplained_arithmetic_mirror_difference_count: Some(
            unexplained_arithmetic_mirror_differences,
        ),
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
    if df64.fired_mismatch_count != 0 {
        return Err(format!(
            "double-single fired-flag mismatches must be zero; f32 baseline={}, double-single={}",
            f32.fired_mismatch_count, df64.fired_mismatch_count
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

fn validate_native_diagnostics(accuracy: &AccuracyMetrics, strategy: &str) -> Result<(), String> {
    if accuracy.winner_mismatch_fraction != 0.0 {
        return Err(format!(
            "{strategy} has non-zero winner mismatch fraction {}",
            accuracy.winner_mismatch_fraction
        ));
    }
    if accuracy.fired_mismatch_count != Some(0) {
        return Err(format!(
            "{strategy} has non-zero or missing fired mismatch count {:?}",
            accuracy.fired_mismatch_count
        ));
    }
    if accuracy.unexplained_arithmetic_mirror_difference_count != Some(0) {
        return Err(format!(
            "{strategy} has non-zero or missing unexplained arithmetic-mirror difference count {:?}",
            accuracy.unexplained_arithmetic_mirror_difference_count
        ));
    }
    Ok(())
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
    use super::*;

    #[test]
    fn native_scoring_preserves_fired_and_arithmetic_mirror_differences() {
        let output = NativeF64TickResult {
            segmented_sums: vec![1.0, 2.0],
            winner_entity_ids: vec![7],
            fired_flags: vec![0, 1],
        };
        let oracle = OracleResult {
            segmented_sums: vec![1.0, 2.0],
            reversed_segmented_sums: vec![1.0, 2.0],
            order_sensitive_flags: vec![0, 0],
            order_sensitive_group_count: 0,
            winner_entity_ids: vec![7],
            fired_flags: vec![0, 0],
        };
        let mirror = F64MirrorResult {
            segmented_sums: vec![1.0, 2.5],
            winner_entity_ids: vec![7],
            fired_flags: vec![0, 1],
        };
        let accuracy = score_native_output(&output, &oracle, &mirror).unwrap();
        assert_eq!(accuracy.fired_mismatch_count, Some(1));
        assert_eq!(
            accuracy.unexplained_arithmetic_mirror_difference_count,
            Some(1)
        );
        assert!(validate_native_diagnostics(&accuracy, "fixture").is_err());

        let row = answered_native_row(
            "native f64 (CUDA)",
            crate::timing::StageTiming {
                total_ms: 1.0,
                reduce_ms: 0.25,
                argmin_ms: 0.25,
                rows_per_second: 1_000.0,
                warmup_ticks: WARMUP_TICKS,
                measured_ticks: MEASURED_TICKS,
                method: crate::timing::TimingMethod::GpuTimestampQueries,
            },
            accuracy,
            &Fp64Throughput::from_cuda_ratio("NVIDIA A100", 2),
        );
        assert!(matches!(row.status, StrategyStatus::Answered { .. }));
        assert!(row
            .verdict
            .contains("usable measurements but is unqualified"));
    }

    #[test]
    fn unified_accuracy_guard_records_all_available_strategies() {
        let guards = pollster::block_on(run_regression_guard());
        assert_eq!(guards.len(), 4);
        assert!(matches!(guards.get("f32"), Some(GuardStatus::Passed)));
        assert!(crate::results::STRATEGIES
            .iter()
            .all(|strategy| guards.contains_key(*strategy)));
    }
}
