//! Durable, mergeable `RESULTS.md` state and Markdown rendering.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::timing::{StageTiming, TimingMethod};

pub const STRATEGIES: [&str; 4] = [
    "f32",
    "double-single",
    "native f64 (wgpu)",
    "native f64 (CUDA)",
];

const STATE_VERSION: u32 = 1;
const STATE_BEGIN: &str = "<!-- sembla-precision-state-v1:begin -->";
const STATE_END: &str = "<!-- sembla-precision-state-v1:end -->";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AccuracyMetrics {
    pub reduction_max_relative_error: f64,
    pub reduction_mean_relative_error: f64,
    pub winner_mismatch_fraction: f64,
    pub order_sensitive_groups: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum StrategyStatus {
    Answered {
        timing: StageTiming,
        accuracy: AccuracyMetrics,
    },
    Unanswered {
        reason: String,
    },
}

impl StrategyStatus {
    #[must_use]
    pub fn is_answered(&self) -> bool {
        matches!(self, Self::Answered { .. })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StrategyRow {
    pub strategy: String,
    pub reduction_choice: String,
    pub verdict: String,
    pub status: StrategyStatus,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Fp64Metadata {
    pub gpu_model: String,
    pub class: String,
    pub fp32_to_fp64_ratio: Option<u32>,
    pub evidence: String,
    pub full_rate_extrapolation: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StrictMathMetadata {
    pub backend_supported: bool,
    pub requested: bool,
    pub fma_contraction_observed: bool,
    pub reassociation_observed: bool,
    pub residuals_preserved: bool,
    pub trustworthy: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HardwareMetadata {
    pub adapter_name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub driver_info: String,
    pub shader_f64: bool,
    pub fp64: Fp64Metadata,
    pub strict_math: StrictMathMetadata,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkloadMetadata {
    pub requested_rows: u32,
    pub requested_groups: u32,
    pub actual_rows: u32,
    pub actual_groups: u32,
    pub downscale_reason: String,
    pub contested_key_selector: String,
    pub benchmark_tick: u32,
    pub warmup_ticks: usize,
    pub measured_ticks: usize,
    pub beta: f64,
    pub dt: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MachineRun {
    pub machine_key: String,
    pub generated_at: String,
    pub hardware: HardwareMetadata,
    pub workload: WorkloadMetadata,
    pub strategies: Vec<StrategyRow>,
    pub infrastructure: BTreeMap<String, String>,
}

impl MachineRun {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.machine_key.as_str(), "development" | "nvidia") {
            return Err(format!(
                "machine key must be development or nvidia, got {}",
                self.machine_key
            ));
        }
        if self.strategies.len() != STRATEGIES.len() {
            return Err(format!(
                "machine run must contain exactly {} strategy rows",
                STRATEGIES.len()
            ));
        }
        for (row, expected) in self.strategies.iter().zip(STRATEGIES) {
            if row.strategy != expected {
                return Err(format!(
                    "strategy row order mismatch: expected {expected}, got {}",
                    row.strategy
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct ResultsState {
    version: u32,
    machines: BTreeMap<String, MachineRun>,
}

impl Default for ResultsState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            machines: BTreeMap::new(),
        }
    }
}

/// Reads any existing state, replaces only this machine's entry, renders the
/// merged report, and atomically renames a sibling temporary file into place.
pub fn update_results(path: &Path, run: MachineRun) -> Result<(), String> {
    run.validate()?;
    let existing = match fs::read_to_string(path) {
        Ok(contents) => Some(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };
    let mut state = existing
        .as_deref()
        .map(parse_state)
        .transpose()?
        .unwrap_or_default();
    state.machines.insert(run.machine_key.clone(), run);
    let rendered = render(&state)?;
    atomic_write(path, rendered.as_bytes())
}

fn parse_state(document: &str) -> Result<ResultsState, String> {
    let begin = document.find(STATE_BEGIN);
    let end = document.find(STATE_END);
    let (Some(begin), Some(end)) = (begin, end) else {
        return Err(
            "existing RESULTS.md has no complete embedded state markers; refusing to overwrite it"
                .to_owned(),
        );
    };
    if begin >= end
        || document[begin + STATE_BEGIN.len()..].contains(STATE_BEGIN)
        || document[end + STATE_END.len()..].contains(STATE_END)
    {
        return Err("existing RESULTS.md has malformed or duplicate state markers".to_owned());
    }
    let encoded = document[begin + STATE_BEGIN.len()..end].trim();
    let encoded = encoded
        .strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
        .ok_or_else(|| "embedded RESULTS.md state is not a json code fence".to_owned())?
        .trim();
    let state: ResultsState = serde_json::from_str(encoded)
        .map_err(|error| format!("embedded RESULTS.md state is invalid: {error}"))?;
    if state.version != STATE_VERSION {
        return Err(format!(
            "unsupported RESULTS.md state version {}; expected {STATE_VERSION}",
            state.version
        ));
    }
    for (key, run) in &state.machines {
        run.validate()?;
        if key != &run.machine_key {
            return Err(format!(
                "machine map key {key} disagrees with run key {}",
                run.machine_key
            ));
        }
    }
    Ok(state)
}

fn render(state: &ResultsState) -> Result<String, String> {
    let mut output = String::new();
    output.push_str("# Precision strategy benchmark results\n\n");
    output.push_str(
        "> This file is generated atomically by `cargo run --release`. Do not edit the\n> embedded state by hand. Unavailable cells remain explicitly unanswered.\n\n",
    );
    output.push_str("## Two-machine assembly\n\n");
    output.push_str(
        "1. Run `cargo run --release` on the development Mac and keep this generated file.\n2. Ensure that exact `RESULTS.md` is present in the NVIDIA checkout (commit it first, or copy it there) before running `cargo run --release --features cuda`.\n3. Copy the NVIDIA-generated file back. The embedded state preserves both machine blocks and the merged matrix chooses portable rows from development and native-f64 rows from NVIDIA.\n\nRows can come from different workloads when adapter sizing differs. Treat each row with its source machine and that machine's workload metadata; do not compare unlike `(N, G)` values as if they were one run.\n\n",
    );

    output.push_str("## Merged strategy × metric matrix\n\n");
    output.push_str("Accuracy cells compare the final benchmark tick against the scalar CPU `f64` oracle computed once for that machine's workload.\n\n");
    output.push_str("| Strategy | Source machine | Timing method | ms/tick total | ms/tick segmented reduce | ms/tick segmented argmin | rows/sec | Reduction rel-err (max / mean) | Winner mismatch % | Order-sensitive groups |\n");
    output.push_str("|---|---|---|---:|---:|---:|---:|---:|---:|---:|\n");
    let mut selected = Vec::new();
    for strategy in STRATEGIES {
        let (machine, row) = select_row(state, strategy)
            .ok_or_else(|| format!("no row available for required strategy {strategy}"))?;
        selected.push((machine, row));
        match &row.status {
            StrategyStatus::Answered { timing, accuracy } => {
                output.push_str(&format!(
                    "| {} | {} | {} | {:.6} | {:.6} | {:.6} | {:.3} | {:.6e} / {:.6e} | {:.6} | {} |\n",
                    row.strategy,
                    display_machine(machine),
                    timing_method_label(&row.strategy, timing.method),
                    timing.total_ms,
                    timing.reduce_ms,
                    timing.argmin_ms,
                    timing.rows_per_second,
                    accuracy.reduction_max_relative_error,
                    accuracy.reduction_mean_relative_error,
                    accuracy.winner_mismatch_fraction * 100.0,
                    accuracy.order_sensitive_groups,
                ));
            }
            StrategyStatus::Unanswered { reason } => {
                output.push_str(&format!(
                    "| {} | {} | {} | unanswered | unanswered | unanswered | unanswered | unanswered | unanswered | unanswered |\n",
                    row.strategy,
                    display_machine(machine),
                    escape_cell(reason),
                ));
            }
        }
    }

    output.push_str("\n## Reduction determinism choice\n\n");
    output.push_str("| Strategy | Source machine | Choice |\n|---|---|---|\n");
    for (machine, row) in &selected {
        output.push_str(&format!(
            "| {} | {} | {} |\n",
            row.strategy,
            display_machine(machine),
            escape_cell(&row.reduction_choice)
        ));
    }

    output.push_str("\n## Per-strategy verdicts\n\n");
    for (machine, row) in &selected {
        output.push_str(&format!(
            "### {}\n\n**Source:** {}. {}\n\n",
            row.strategy,
            display_machine(machine),
            row.verdict
        ));
    }

    render_machine_section(
        &mut output,
        "development",
        state.machines.get("development"),
    );
    render_machine_section(&mut output, "nvidia", state.machines.get("nvidia"));

    output.push_str("## Embedded merge state\n\n");
    output.push_str(STATE_BEGIN);
    output.push_str("\n```json\n");
    output.push_str(
        &serde_json::to_string_pretty(state)
            .map_err(|error| format!("failed to serialize RESULTS.md state: {error}"))?,
    );
    output.push_str("\n```\n");
    output.push_str(STATE_END);
    output.push('\n');
    Ok(output)
}

fn select_row<'a>(state: &'a ResultsState, strategy: &str) -> Option<(&'a str, &'a StrategyRow)> {
    let native = strategy.starts_with("native f64");
    let preferred = if native {
        ["nvidia", "development"]
    } else {
        ["development", "nvidia"]
    };
    for key in preferred {
        if let Some(row) = find_row(state.machines.get(key), strategy) {
            if row.status.is_answered() {
                return Some((key, row));
            }
        }
    }
    for (key, run) in &state.machines {
        if let Some(row) = find_row(Some(run), strategy) {
            if row.status.is_answered() {
                return Some((key.as_str(), row));
            }
        }
    }
    for key in preferred {
        if let Some(row) = find_row(state.machines.get(key), strategy) {
            return Some((key, row));
        }
    }
    state
        .machines
        .iter()
        .find_map(|(key, run)| find_row(Some(run), strategy).map(|row| (key.as_str(), row)))
}

fn find_row<'a>(run: Option<&'a MachineRun>, strategy: &str) -> Option<&'a StrategyRow> {
    run?.strategies.iter().find(|row| row.strategy == strategy)
}

fn render_machine_section(output: &mut String, key: &str, run: Option<&MachineRun>) {
    output.push_str(&format!("\n## {} machine\n\n", display_machine(key)));
    let Some(run) = run else {
        output.push_str("Not yet measured; its rows remain unanswered in the merged matrix.\n");
        return;
    };
    let hardware = &run.hardware;
    let workload = &run.workload;
    let ratio = hardware
        .fp64
        .fp32_to_fp64_ratio
        .map_or_else(|| "unknown".to_owned(), |ratio| format!("1:{ratio}"));
    output.push_str(&format!(
        "- generated: `{}`\n- adapter: `{}`\n- backend/device type: `{}` / `{}`\n- driver: `{}` (`{}`)\n- `SHADER_F64`: `{}`\n- exact GPU/fp64 model: `{}`\n- fp64 class and ratio: `{}` / `{}`\n- fp64 evidence: {}\n- full-rate extrapolation: `{}`\n- portable strict math trustworthy: `{}` (requested={}, backend-supported={}, FMA-contraction-observed={}, reassociation-observed={}, residuals-preserved={})\n\n",
        run.generated_at,
        hardware.adapter_name,
        hardware.backend,
        hardware.device_type,
        hardware.driver,
        hardware.driver_info,
        hardware.shader_f64,
        hardware.fp64.gpu_model,
        hardware.fp64.class,
        ratio,
        hardware.fp64.evidence,
        if hardware.fp64.full_rate_extrapolation { "allowed" } else { "refused" },
        hardware.strict_math.trustworthy,
        hardware.strict_math.requested,
        hardware.strict_math.backend_supported,
        hardware.strict_math.fma_contraction_observed,
        hardware.strict_math.reassociation_observed,
        hardware.strict_math.residuals_preserved,
    ));
    output.push_str("### Workload\n\n");
    output.push_str(&format!(
        "- requested `(N, G)`: `({}, {})`\n- actual `(N, G)`: `({}, {})`\n- downscale reason: {}\n- contested-key selector: `{}`\n- benchmark tick: `{}`\n- warmup/measured ticks: `{}` / `{}`\n- `beta` / `dt`: `{}` / `{}`\n\n",
        workload.requested_rows,
        workload.requested_groups,
        workload.actual_rows,
        workload.actual_groups,
        workload.downscale_reason,
        workload.contested_key_selector,
        workload.benchmark_tick,
        workload.warmup_ticks,
        workload.measured_ticks,
        workload.beta,
        workload.dt,
    ));
    if !run.infrastructure.is_empty() {
        output.push_str("### Infrastructure metadata\n\n");
        for (name, value) in &run.infrastructure {
            output.push_str(&format!("- `{}`: `{}`\n", name, value.replace('`', "'")));
        }
        output.push('\n');
    }
    output.push_str("### Local rows\n\n");
    for row in &run.strategies {
        let status = match &row.status {
            StrategyStatus::Answered { .. } => "answered".to_owned(),
            StrategyStatus::Unanswered { reason } => reason.clone(),
        };
        output.push_str(&format!("- **{}:** {}\n", row.strategy, status));
    }
}

fn display_machine(key: &str) -> &str {
    match key {
        "development" => "Development",
        "nvidia" => "NVIDIA",
        other => other,
    }
}

fn timing_method_label(strategy: &str, method: TimingMethod) -> String {
    if strategy == "native f64 (CUDA)" && method == TimingMethod::GpuTimestampQueries {
        "CUDA event timestamps".to_owned()
    } else {
        method.to_string()
    }
}

fn escape_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid results path {}", path.display()))?;
    let temporary: PathBuf = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    fs::write(&temporary, contents)
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "failed to atomically replace {}: {error}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timing::{MEASURED_TICKS, WARMUP_TICKS};

    fn run(key: &str, answer_native: bool) -> MachineRun {
        let rows = STRATEGIES
            .iter()
            .map(|strategy| {
                let answered = if strategy.starts_with("native") {
                    answer_native
                } else {
                    !answer_native
                };
                StrategyRow {
                    strategy: (*strategy).to_owned(),
                    reduction_choice: "deterministic two-pass, no atomics (Level A)".to_owned(),
                    verdict: format!("{strategy} fixture verdict"),
                    status: if answered {
                        StrategyStatus::Answered {
                            timing: StageTiming {
                                total_ms: 1.0,
                                reduce_ms: 0.25,
                                argmin_ms: 0.25,
                                rows_per_second: 1_000.0,
                                warmup_ticks: WARMUP_TICKS,
                                measured_ticks: MEASURED_TICKS,
                                method: TimingMethod::GpuTimestampQueries,
                            },
                            accuracy: AccuracyMetrics {
                                reduction_max_relative_error: 1.0e-9,
                                reduction_mean_relative_error: 1.0e-10,
                                winner_mismatch_fraction: 0.0,
                                order_sensitive_groups: 2,
                            },
                        }
                    } else {
                        StrategyStatus::Unanswered {
                            reason: "unanswered on this adapter: unavailable fixture".to_owned(),
                        }
                    },
                }
            })
            .collect();
        MachineRun {
            machine_key: key.to_owned(),
            generated_at: "unix-seconds:1".to_owned(),
            hardware: HardwareMetadata {
                adapter_name: key.to_owned(),
                backend: "fixture".to_owned(),
                device_type: "fixture".to_owned(),
                driver: "fixture".to_owned(),
                driver_info: "fixture".to_owned(),
                shader_f64: answer_native,
                fp64: Fp64Metadata {
                    gpu_model: key.to_owned(),
                    class: "rate-limited".to_owned(),
                    fp32_to_fp64_ratio: None,
                    evidence: "fixture".to_owned(),
                    full_rate_extrapolation: false,
                },
                strict_math: StrictMathMetadata {
                    backend_supported: true,
                    requested: true,
                    fma_contraction_observed: false,
                    reassociation_observed: false,
                    residuals_preserved: true,
                    trustworthy: true,
                },
            },
            workload: WorkloadMetadata {
                requested_rows: 1_000,
                requested_groups: 50,
                actual_rows: 1_000,
                actual_groups: 50,
                downscale_reason: "none".to_owned(),
                contested_key_selector: "entity_id % 10 == 5".to_owned(),
                benchmark_tick: 7,
                warmup_ticks: WARMUP_TICKS,
                measured_ticks: MEASURED_TICKS,
                beta: 0.35,
                dt: 0.25,
            },
            strategies: rows,
            infrastructure: BTreeMap::new(),
        }
    }

    #[test]
    fn rendering_always_has_four_rows_and_explicit_unanswered_cells() {
        let mut state = ResultsState::default();
        state
            .machines
            .insert("development".to_owned(), run("development", false));
        let document = render(&state).unwrap();
        for strategy in STRATEGIES {
            assert!(document.contains(&format!("| {strategy} |")));
        }
        assert!(document.contains("unanswered on this adapter"));
    }

    #[test]
    fn development_then_nvidia_merge_preserves_both_and_prefers_expected_rows() {
        let mut state = ResultsState::default();
        state
            .machines
            .insert("development".to_owned(), run("development", false));
        state
            .machines
            .insert("nvidia".to_owned(), run("nvidia", true));
        let document = render(&state).unwrap();
        let parsed = parse_state(&document).unwrap();
        assert_eq!(parsed.machines.len(), 2);
        assert!(document.contains("| f32 | Development |"));
        assert!(document.contains("| native f64 (CUDA) | NVIDIA |"));
        assert!(document.contains("## Development machine"));
        assert!(document.contains("## NVIDIA machine"));
    }

    #[test]
    fn sequential_development_then_nvidia_updates_preserve_both_machine_states() {
        let path = std::env::temp_dir().join(format!(
            "sembla-results-merge-{}-{}.md",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_file(&path);
        update_results(&path, run("development", false)).unwrap();
        update_results(&path, run("nvidia", true)).unwrap();
        let document = fs::read_to_string(&path).unwrap();
        let state = parse_state(&document).unwrap();
        assert_eq!(state.machines.len(), 2);
        assert!(state.machines.contains_key("development"));
        assert!(state.machines.contains_key("nvidia"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn malformed_state_is_refused_without_touching_the_file() {
        let path = std::env::temp_dir().join(format!(
            "sembla-results-malformed-{}-{}.md",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let malformed = "# existing without managed state\n";
        fs::write(&path, malformed).unwrap();
        let error = update_results(&path, run("development", false)).unwrap_err();
        assert!(error.contains("refusing to overwrite"));
        assert_eq!(fs::read_to_string(&path).unwrap(), malformed);
        let _ = fs::remove_file(path);
    }
}
