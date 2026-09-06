//! Single-run execution, result serialization, and timing artifacts.

mod lifecycle;
mod timing;
pub(crate) use timing::*;

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct RunOptions {
    pub(crate) seed: u64,
    pub(crate) ticks: u32,
    pub(crate) population: String,
    pub(crate) out: Option<String>,
    pub(crate) export_state: Option<String>,
    pub(crate) dt: Option<f64>,
    pub(crate) params: Option<String>,
    pub(crate) timing_json: Option<String>,
    pub(crate) lifecycle_timing_json: Option<String>,
    pub(crate) backend: BackendSelection,
    pub(crate) enabled_features: FeatureSet,
}

pub(crate) fn parse_run_options(flags: &[String]) -> Result<RunOptions, String> {
    let mut seed = None;
    let mut ticks = None;
    let mut population = None;
    let mut out = None;
    let mut export_state = None;
    let mut dt = None;
    let mut params = None;
    let mut timing_json = None;
    let mut lifecycle_timing_json = None;
    let mut backend = None;
    let mut enabled_features = FeatureSet::new();
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index].as_str();
        let value = flags
            .get(index + 1)
            .ok_or_else(|| format!("missing value for '{flag}'"))?;
        match flag {
            "--seed" => set_once(&mut seed, parse_number(value, flag)?, flag)?,
            "--ticks" => set_once(&mut ticks, parse_number(value, flag)?, flag)?,
            "--population" => {
                if value.parse::<usize>().is_err() && !Path::new(value).is_file() {
                    return Err(format!(
                        "invalid numeric value or population file '{value}' for '{flag}'"
                    ));
                }
                set_once(&mut population, value.clone(), flag)?;
            }
            "--out" => set_once(&mut out, value.clone(), flag)?,
            "--export-state" => set_once(&mut export_state, value.clone(), flag)?,
            "--dt" => {
                let value: f64 = parse_number(value, flag)?;
                if !value.is_finite() || value <= 0.0 {
                    return Err("'--dt' must be finite and greater than zero".to_owned());
                }
                set_once(&mut dt, value, flag)?;
            }
            "--params" => set_once(&mut params, value.clone(), flag)?,
            "--timing-json" => set_once(&mut timing_json, value.clone(), flag)?,
            "--lifecycle-timing-json" => set_once(&mut lifecycle_timing_json, value.clone(), flag)?,
            "--backend" => set_once(&mut backend, parse_backend(value)?, flag)?,
            "--enable" => {
                enabled_features.insert(parse_feature(value)?);
            }
            _ => return Err(format!("unknown run flag '{flag}'")),
        }
        index += 2;
    }
    Ok(RunOptions {
        seed: seed.ok_or_else(|| "missing required flag '--seed'".to_owned())?,
        ticks: ticks.ok_or_else(|| "missing required flag '--ticks'".to_owned())?,
        population: population.ok_or_else(|| "missing required flag '--population'".to_owned())?,
        out,
        export_state,
        dt,
        params,
        timing_json,
        lifecycle_timing_json,
        backend: backend.unwrap_or_default(),
        enabled_features,
    })
}

pub(crate) fn run_file(path: &str, options: RunOptions) -> i32 {
    match run_file_result(path, options) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

pub(crate) fn run_file_result(path: &str, options: RunOptions) -> Result<(), String> {
    let lifecycle_started = Instant::now();
    reject_cuda_sweep_final_state_selector_for_run()?;
    if let (Some(export_path), Some(out)) =
        (options.export_state.as_deref(), options.out.as_deref())
    {
        let export_path = Path::new(export_path);
        for output_path in [
            PathBuf::from(out),
            summaries_path(out),
            manifest::sidecar_path(out),
        ] {
            if paths_resolve_to_same_file(export_path, &output_path) {
                return Err(format!(
                    "--export-state path '{}' conflicts with run output path '{}'",
                    export_path.display(),
                    output_path.display()
                ));
            }
        }
    }
    if let Some(timing_path) = options.timing_json.as_deref() {
        let timing_path = Path::new(timing_path);
        let mut output_paths = Vec::new();
        if let Some(out) = options.out.as_deref() {
            output_paths.extend([
                PathBuf::from(out),
                summaries_path(out),
                manifest::sidecar_path(out),
            ]);
        }
        if let Some(export_path) = options.export_state.as_deref() {
            output_paths.push(PathBuf::from(export_path));
        }
        for output_path in output_paths {
            if paths_resolve_to_same_file(timing_path, &output_path) {
                return Err(format!(
                    "--timing-json path '{}' conflicts with run output path '{}'",
                    timing_path.display(),
                    output_path.display()
                ));
            }
        }
    }
    if let Some(export_path) = options.export_state.as_deref() {
        let export_path = Path::new(export_path);
        if export_path
            .try_exists()
            .map_err(|error| format!("{}: {error}", export_path.display()))?
        {
            return Err(format!(
                "refusing to overwrite existing state artifact '{}'",
                export_path.display()
            ));
        }
    }

    let RunInput { model, plan } = read_run_input(path, &options)?;
    if let (Some(timing_path), Some(out)) = (options.timing_json.as_deref(), options.out.as_deref())
    {
        let timing_path = Path::new(timing_path);
        for view in model
            .model()
            .boxes
            .iter()
            .flat_map(|model_box| model_box.grouped_views.iter())
        {
            let grouped_path = grouped_output_path(Path::new(out), &view.name);
            if paths_resolve_to_same_file(timing_path, &grouped_path) {
                return Err(format!(
                    "--timing-json path '{}' conflicts with run output path '{}'",
                    timing_path.display(),
                    grouped_path.display()
                ));
            }
        }
    }
    lifecycle::check_path(path, &options, &model)?;
    let (population_source, population_sha256) =
        manifest::population_identity(&options.population)?;
    let initialized = initialized_tables(&model, &options.population)?;
    let initial_state = initialized.state_hash.map(state_artifact_tuple);
    let initial = initialized.tables;
    let params = resolve_params(&model, options.params.as_deref())?;
    if options.out.is_none()
        && options.export_state.is_none()
        && options.timing_json.is_none()
        && options.lifecycle_timing_json.is_none()
        && options.backend == BackendSelection::Cpu
    {
        let mut state =
            StateStore::new(&model, initial).map_err(|error| format!("{path}: {error}"))?;
        let report = executor::run_with_features(
            &model,
            &mut state,
            &params,
            options.seed,
            options.ticks,
            &options.enabled_features,
        )
        .map_err(|error| format!("{path}: {error}"))?;
        for warning in &report.warnings {
            eprintln!(
                "warning: tick {} resource table '{}': {} deferred exceeds 10% of {} fired",
                warning.tick, warning.table, warning.deferred_count, warning.fired_count
            );
        }
        for tick in report.ticks {
            for (box_name, rules) in tick.fired_per_box {
                for (rule_id, fired) in rules {
                    println!(
                        "tick={} box={} rule_id={} fired={}",
                        tick.tick, box_name, rule_id, fired
                    );
                }
            }
        }
        return Ok(());
    }
    let prepared_at = Instant::now();
    let (execution, timing) = if options.timing_json.is_some() {
        let scale = initial
            .iter()
            .map(|table| table.row_count)
            .max()
            .unwrap_or(0);
        let (execution, timing) = execute_backend_output_timed_with_features(
            &model,
            initial,
            &params,
            options.seed,
            options.ticks,
            BackendRunMode::final_only(options.backend),
            &options.enabled_features,
            scale,
        )?;
        (execution, Some(timing))
    } else {
        (
            execute_backend_output_with_features(
                &model,
                initial,
                &params,
                options.seed,
                options.ticks,
                BackendRunMode::final_only(options.backend),
                &options.enabled_features,
            )?,
            None,
        )
    };
    let executed_at = Instant::now();
    let exported_state = options
        .export_state
        .as_deref()
        .map(|export_path| {
            let tables = committed_table_inits(&model, &execution.state)
                .map_err(|error| format!("{export_path}: {error}"))?;
            write_new_state_artifact(export_path, &model, &tables)
                .map_err(|error| error.to_string())?;
            let hash = state_artifact_hash(export_path).map_err(|error| error.to_string())?;
            Ok::<_, String>(state_artifact_tuple(hash))
        })
        .transpose()?;

    if let Some(out) = options.out.as_deref() {
        write_atomic(out, execution.output.csv.as_bytes())?;
        let summaries = summaries_path(out);
        write_atomic(summaries, execution.output.summaries_csv.as_bytes())?;
        for grouped in &execution.output.grouped {
            let path = grouped_output_path(Path::new(out), &grouped.view);
            write_atomic(&path, grouped.csv.as_bytes())?;
        }
        let hashes = execution_hashes(&execution.output, &execution.state);
        println!(
            "results_sha256={} final_state_sha256={} observation_sha256={}",
            hashes.results_sha256, hashes.final_state_sha256, hashes.observation_sha256
        );
        let mut run_manifest = manifest::RunManifest::new(
            manifest::ManifestKind::Run,
            options.seed,
            options.ticks,
            population_source,
            population_sha256,
        );
        run_manifest.model = Some(model.model().name.clone());
        run_manifest.dt = Some(model.model().dt);
        run_manifest.enabled_features = options.enabled_features.iter().cloned().collect();
        run_manifest.grouped_outputs = grouped_output_records(&execution.output.grouped);
        run_manifest.ir_hash = Some(manifest::canonical_ir_hash(&model)?);
        run_manifest.backend_identity = Some(execution.identity);
        run_manifest.resolved_theta = manifest::resolved_theta(&params);
        run_manifest.results_sha256 = Some(hashes.results_sha256);
        run_manifest.final_state_sha256 = Some(hashes.final_state_sha256);
        run_manifest.observation_sha256 = Some(hashes.observation_sha256);
        run_manifest.initial_state = initial_state;
        run_manifest.exported_state = exported_state;
        if let Some(plan) = &plan {
            let (plan_identity, linked_source) = manifest::plan_identity_tuples(plan)?;
            run_manifest.plan = Some(plan_identity);
            run_manifest.linked_source = linked_source;
        }
        manifest::write(&manifest::sidecar_path(out), &run_manifest)?;
    } else {
        for (tick, row) in execution.output.series.rows.iter().enumerate() {
            for transition in model.transitions() {
                let model_box = &model.model().boxes[transition.box_index];
                let declaration = &model_box.transitions[transition.transition_index];
                let plain = format!("fired_{}", declaration.name);
                let qualified = format!("fired:{}.{}", model_box.name, declaration.name);
                let column = execution
                    .output
                    .series
                    .columns
                    .iter()
                    .position(|name| name == &plain || name == &qualified)
                    .ok_or_else(|| {
                        format!("missing firing output for rule {}", transition.rule_id)
                    })?;
                println!(
                    "tick={} box={} rule_id={} fired={}",
                    tick,
                    model_box.name,
                    transition.rule_id,
                    row[column].as_usize("firing output")?
                );
            }
        }
    }
    if let (Some(timing_path), Some(timing)) = (options.timing_json.as_deref(), timing.as_ref()) {
        write_timing_document(timing_path, timing)?;
    }
    if let Some(report) = options.lifecycle_timing_json.as_deref() {
        lifecycle::write(
            report,
            options.backend,
            [
                prepared_at - lifecycle_started,
                executed_at - prepared_at,
                executed_at.elapsed(),
            ],
            execution.elapsed,
            #[cfg(feature = "cuda")]
            execution.construction_timing,
        )?;
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExecutionHashes {
    pub(crate) results_sha256: String,
    pub(crate) final_state_sha256: String,
    pub(crate) observation_sha256: String,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ReportedValue {
    Unsigned(usize),
    Int(i64),
    Real(f64),
}

impl ReportedValue {
    pub(crate) fn csv(self) -> String {
        match self {
            Self::Unsigned(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::Real(value) => value.to_string(),
        }
    }

    pub(crate) fn cmp(self, other: Self) -> Result<std::cmp::Ordering, String> {
        match (self, other) {
            (Self::Unsigned(left), Self::Unsigned(right)) => Ok(left.cmp(&right)),
            (Self::Int(left), Self::Int(right)) => Ok(left.cmp(&right)),
            (Self::Real(left), Self::Real(right)) => Ok(left.total_cmp(&right)),
            _ => Err("reported column changed numeric type across draws".to_owned()),
        }
    }

    pub(crate) fn as_usize(self, context: &str) -> Result<usize, String> {
        match self {
            Self::Unsigned(value) => Ok(value),
            Self::Int(value) => usize::try_from(value)
                .map_err(|_| format!("{context} is negative or exceeds usize")),
            Self::Real(value) => Err(format!("{context} is real-valued ({value})")),
        }
    }
}

impl From<ObservationValue> for ReportedValue {
    fn from(value: ObservationValue) -> Self {
        match value {
            ObservationValue::Real(value) => Self::Real(value),
            ObservationValue::Int(value) => Self::Int(value),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ReportedSeries {
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<Vec<ReportedValue>>,
}

#[derive(Clone, Debug)]
pub(crate) struct GroupedCsvOutput {
    pub(crate) view: String,
    pub(crate) csv: String,
}

#[derive(Clone, Debug)]
pub(crate) struct RunOutput {
    pub(crate) csv: String,
    pub(crate) grouped: Vec<GroupedCsvOutput>,
    pub(crate) series: ReportedSeries,
    pub(crate) summaries: Vec<SummaryValue>,
    pub(crate) summaries_csv: String,
    pub(crate) per_tick_hashes: PerTickHashes,
}

pub(crate) fn summaries_path(output: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{output}.summaries.csv"))
}

pub(crate) fn grouped_output_path(output: &Path, view: &str) -> PathBuf {
    let stem = output
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("results");
    let name = format!("{stem}.grouped.{view}.csv");
    output.parent().unwrap_or_else(|| Path::new("")).join(name)
}

pub(crate) fn grouped_output_records(
    outputs: &[GroupedCsvOutput],
) -> Vec<manifest::GroupedOutputRecord> {
    outputs
        .iter()
        .map(|output| manifest::GroupedOutputRecord {
            view: output.view.clone(),
            algorithm: manifest::HASH_ALGORITHM.to_owned(),
            sha256: hex(&Sha256::digest(output.csv.as_bytes())),
        })
        .collect()
}

pub(crate) fn paths_resolve_to_same_file(left: &Path, right: &Path) -> bool {
    if left == right || same_file_identity(left, right) {
        return true;
    }
    matches!(
        (
            resolve_collision_path(left),
            resolve_collision_path(right)
        ),
        (Some(left), Some(right)) if left == right
    )
}

pub(crate) fn resolve_collision_path(path: &Path) -> Option<PathBuf> {
    if let Ok(path) = path.canonicalize() {
        return Some(path);
    }
    let mut candidate = canonical_parent_with_final_component(path)?;
    // Follow a final-component symlink even when its target does not yet
    // exist. That prevents a dangling timing alias from becoming destructive
    // after the ordinary output is created.
    for _ in 0..16 {
        match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target = std::fs::read_link(&candidate).ok()?;
                candidate = if target.is_absolute() {
                    target
                } else {
                    candidate.parent()?.join(target)
                };
                candidate = canonical_parent_with_final_component(&candidate)?;
                if let Ok(path) = candidate.canonicalize() {
                    return Some(path);
                }
            }
            _ => return Some(candidate),
        }
    }
    None
}

pub(crate) fn canonical_parent_with_final_component(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name()?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent
        .canonicalize()
        .ok()
        .map(|parent| parent.join(file_name))
}

#[cfg(unix)]
pub(crate) fn same_file_identity(left: &Path, right: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    matches!(
        (std::fs::metadata(left), std::fs::metadata(right)),
        (Ok(left), Ok(right)) if left.dev() == right.dev() && left.ino() == right.ino()
    )
}

#[cfg(not(unix))]
pub(crate) fn same_file_identity(_left: &Path, _right: &Path) -> bool {
    false
}

pub(crate) struct BackendRunOutput {
    #[cfg(feature = "cuda")]
    pub(crate) construction_timing: Option<CudaConstructionTiming>,
    pub(crate) output: RunOutput,
    pub(crate) state: StateStore,
    pub(crate) identity: manifest::BackendIdentity,
    pub(crate) per_tick_hashes: PerTickHashes,
    pub(crate) elapsed: std::time::Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BackendRunMode {
    backend: BackendSelection,
    hash_mode: HashMode,
}

impl BackendRunMode {
    pub(crate) fn final_only(backend: BackendSelection) -> Self {
        Self {
            backend,
            hash_mode: HashMode::FinalOnly,
        }
    }

    pub(crate) fn every_tick(backend: BackendSelection) -> Self {
        Self {
            backend,
            hash_mode: HashMode::EveryTick,
        }
    }
}

pub(crate) fn execute_backend_output_with_features(
    model: &sembla_ir::ValidatedModel,
    initial: Vec<TableInit>,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
    run_mode: BackendRunMode,
    enabled_features: &FeatureSet,
) -> Result<BackendRunOutput, String> {
    match run_mode.backend {
        BackendSelection::Cpu => {
            let mut state = StateStore::new(model, initial).map_err(|error| error.to_string())?;
            let started = std::time::Instant::now();
            let output = run_results_output_with_features(
                model,
                &mut state,
                params,
                seed,
                ticks,
                run_mode.hash_mode,
                enabled_features,
            )?;
            let elapsed = started.elapsed();
            let per_tick_hashes = output.per_tick_hashes.clone();
            Ok(BackendRunOutput {
                #[cfg(feature = "cuda")]
                construction_timing: None,
                output,
                state,
                identity: manifest::BackendIdentity::cpu_oracle(),
                per_tick_hashes,
                elapsed,
            })
        }
        BackendSelection::Cuda => {
            run_results_output_cuda(model, initial, params, seed, ticks, run_mode.hash_mode)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_backend_output_timed_with_features(
    model: &sembla_ir::ValidatedModel,
    initial: Vec<TableInit>,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
    run_mode: BackendRunMode,
    enabled_features: &FeatureSet,
    scale: usize,
) -> Result<(BackendRunOutput, TimingDocument), String> {
    match run_mode.backend {
        BackendSelection::Cpu => {
            let mut state = StateStore::new(model, initial).map_err(|error| error.to_string())?;
            let started = Instant::now();
            let (output, tick_timings) = run_results_output_timed_with_features(
                model,
                &mut state,
                params,
                seed,
                ticks,
                run_mode.hash_mode,
                enabled_features,
            )?;
            let elapsed = started.elapsed();
            let per_tick_hashes = output.per_tick_hashes.clone();
            let timing =
                TimingDocument::new(run_mode.backend, scale, ticks, seed, false, tick_timings)?;
            Ok((
                BackendRunOutput {
                    #[cfg(feature = "cuda")]
                    construction_timing: None,
                    output,
                    state,
                    identity: manifest::BackendIdentity::cpu_oracle(),
                    per_tick_hashes,
                    elapsed,
                },
                timing,
            ))
        }
        BackendSelection::Cuda => {
            #[cfg(feature = "cuda")]
            {
                let (output, tick_timings) = run_results_output_cuda_timed(
                    model,
                    initial,
                    params,
                    seed,
                    ticks,
                    run_mode.hash_mode,
                )?;
                let timing =
                    TimingDocument::new(run_mode.backend, scale, ticks, seed, false, tick_timings)?;
                Ok((output, timing))
            }
            #[cfg(not(feature = "cuda"))]
            {
                let _ = scale;
                match run_results_output_cuda(
                    model,
                    initial,
                    params,
                    seed,
                    ticks,
                    run_mode.hash_mode,
                ) {
                    Ok(_) => Err(
                        "CUDA timing unexpectedly succeeded without the cuda feature".to_owned(),
                    ),
                    Err(error) => Err(error),
                }
            }
        }
    }
}

pub(crate) fn execution_hashes(output: &RunOutput, state: &StateStore) -> ExecutionHashes {
    execution_hashes_with_state_hash(output, state.state_hash())
}

pub(crate) fn execution_hashes_with_state_hash(
    output: &RunOutput,
    final_state_hash: [u8; 32],
) -> ExecutionHashes {
    ExecutionHashes {
        results_sha256: hex(&Sha256::digest(output.csv.as_bytes())),
        final_state_sha256: hex(&final_state_hash),
        observation_sha256: hex(&Sha256::digest(output.summaries_csv.as_bytes())),
    }
}

#[derive(Debug)]
pub(crate) struct EnumCountDescriptor {
    box_name: String,
    table_name: String,
    attr_name: String,
    variants: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct FiringDescriptor {
    box_name: String,
    transition_name: String,
    rule_id: u32,
}

pub(crate) fn csv_field(value: &str) -> String {
    if value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\n' | '\r'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

pub(crate) fn generic_enum_descriptors(
    model: &sembla_ir::ValidatedModel,
) -> Vec<EnumCountDescriptor> {
    let mut descriptors = Vec::new();
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            for attr in &table.attrs {
                if let AttrType::Enum { variants } = &attr.ty {
                    descriptors.push(EnumCountDescriptor {
                        box_name: model_box.name.clone(),
                        table_name: table.name.clone(),
                        attr_name: attr.name.clone(),
                        variants: variants.clone(),
                    });
                }
            }
        }
    }
    descriptors
}

pub(crate) fn grouped_csv_outputs(
    model: &sembla_ir::ValidatedModel,
    ticks: &[executor::TickReport],
) -> Result<Vec<GroupedCsvOutput>, String> {
    let mut outputs = Vec::new();
    for model_box in &model.model().boxes {
        for view in &model_box.grouped_views {
            let table = model_box
                .tables
                .iter()
                .find(|table| table.name == view.table)
                .expect("validated grouped table disappeared");
            let mut csv = String::from("tick");
            for key in &view.keys {
                csv.push(',');
                csv.push_str(&csv_field(&key.attr));
            }
            csv.push_str(",count\n");
            for tick in ticks {
                let mut rows = tick
                    .grouped_views
                    .iter()
                    .filter(|row| row.box_name == model_box.name && row.name == view.name)
                    .collect::<Vec<_>>();
                rows.sort_by(|left, right| left.keys.cmp(&right.keys));
                for row in rows {
                    csv.push_str(&tick.tick.to_string());
                    for (key, value) in view.keys.iter().zip(&row.keys) {
                        let attr = table
                            .attrs
                            .iter()
                            .find(|attr| attr.name == key.attr)
                            .expect("validated grouped key disappeared");
                        let rendered = match (&attr.ty, key.band_width) {
                            (AttrType::Enum { variants }, None) => {
                                let index = usize::try_from(*value).map_err(|_| {
                                    format!("grouped enum key '{}' is negative", key.attr)
                                })?;
                                variants.get(index).cloned().ok_or_else(|| {
                                    format!(
                                        "grouped enum key '{}' has invalid variant index {index}",
                                        key.attr
                                    )
                                })?
                            }
                            (AttrType::Ref { .. }, None) | (AttrType::Int, Some(_)) => {
                                value.to_string()
                            }
                            _ => unreachable!("validated grouped key type disappeared"),
                        };
                        csv.push(',');
                        csv.push_str(&csv_field(&rendered));
                    }
                    csv.push(',');
                    csv.push_str(&row.count.to_string());
                    csv.push('\n');
                }
            }
            outputs.push(GroupedCsvOutput {
                view: view.name.clone(),
                csv,
            });
        }
    }
    Ok(outputs)
}

pub(crate) fn generic_firing_descriptors(
    model: &sembla_ir::ValidatedModel,
) -> Vec<FiringDescriptor> {
    model
        .transitions()
        .iter()
        .map(|rule| {
            let model_box = &model.model().boxes[rule.box_index];
            let transition = &model_box.transitions[rule.transition_index];
            FiringDescriptor {
                box_name: model_box.name.clone(),
                transition_name: transition.name.clone(),
                rule_id: rule.rule_id,
            }
        })
        .collect()
}

pub(crate) struct RunOutputAccumulator {
    has_views: bool,
    enums: Option<Vec<EnumCountDescriptor>>,
    firings: Vec<FiringDescriptor>,
    headers: Vec<String>,
    pub(crate) csv: String,
    rows: Vec<Vec<ReportedValue>>,
    tick_reports: Vec<executor::TickReport>,
}

impl RunOutputAccumulator {
    pub(crate) fn new(
        model: &sembla_ir::ValidatedModel,
        params: &ParamEnv,
        ticks: u32,
    ) -> Result<Self, String> {
        let has_views = model
            .model()
            .boxes
            .iter()
            .any(|model_box| !model_box.views.is_empty());
        let enums = (!has_views).then(|| generic_enum_descriptors(model));
        let firings = generic_firing_descriptors(model);
        let mut csv = String::new();
        csv.push_str("# params=");
        csv.push_str(&canonical_params(params)?);
        csv.push('\n');
        csv.push_str(&format!("# dt={}\n", model.model().dt));

        let mut headers = vec!["tick".to_owned()];
        if has_views {
            headers.extend(
                model
                    .model()
                    .boxes
                    .iter()
                    .flat_map(|model_box| model_box.views.iter().map(|view| view.name.clone())),
            );
        } else {
            for descriptor in enums.as_deref().unwrap_or_default() {
                for variant in &descriptor.variants {
                    headers.push(format!(
                        "count:{}.{}.{}={variant}",
                        descriptor.box_name, descriptor.table_name, descriptor.attr_name
                    ));
                }
            }
        }
        for descriptor in &firings {
            if has_views {
                headers.push(format!("fired_{}", descriptor.transition_name));
            } else {
                headers.push(format!(
                    "fired:{}.{}",
                    descriptor.box_name, descriptor.transition_name
                ));
            }
        }
        headers.push("deferred_total".to_owned());
        csv.push_str(
            &headers
                .iter()
                .map(|header| csv_field(header))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push('\n');

        Ok(Self {
            has_views,
            enums,
            firings,
            headers,
            csv,
            rows: Vec::with_capacity(ticks as usize),
            tick_reports: Vec::with_capacity(ticks as usize),
        })
    }

    pub(crate) fn push_tick(
        &mut self,
        state: &StateStore,
        tick: u32,
        report: executor::TickReport,
    ) -> Result<(), String> {
        self.push_tick_with_enum_counts(state, tick, report, None)
    }

    pub(crate) fn push_tick_with_enum_counts(
        &mut self,
        state: &StateStore,
        tick: u32,
        report: executor::TickReport,
        generic_enum_counts: Option<&[usize]>,
    ) -> Result<(), String> {
        let mut row = Vec::with_capacity(self.headers.len() - 1);
        if self.has_views {
            row.extend(
                report
                    .views
                    .iter()
                    .map(|view| ReportedValue::from(view.value)),
            );
        } else if let Some(counts) = generic_enum_counts {
            let expected = self
                .enums
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|descriptor| descriptor.variants.len())
                .sum::<usize>();
            if counts.len() != expected {
                return Err(format!(
                    "tick {tick}: device generic enum report has {} counts, expected {expected}",
                    counts.len()
                ));
            }
            row.extend(counts.iter().copied().map(ReportedValue::Unsigned));
        } else {
            let snapshot = state.snapshot();
            for descriptor in self.enums.as_deref().unwrap_or_default() {
                let values = snapshot
                    .enum_values(
                        &descriptor.box_name,
                        &descriptor.table_name,
                        &descriptor.attr_name,
                    )
                    .map_err(|error| error.to_string())?;
                let mut counts = vec![0_usize; descriptor.variants.len()];
                for value in values {
                    let slot = counts.get_mut(usize::from(*value)).ok_or_else(|| {
                        format!(
                            "invalid enum index {value} for {}.{}.{} with {} variants",
                            descriptor.box_name,
                            descriptor.table_name,
                            descriptor.attr_name,
                            descriptor.variants.len()
                        )
                    })?;
                    *slot += 1;
                }
                row.extend(counts.into_iter().map(ReportedValue::Unsigned));
            }
        }
        for descriptor in &self.firings {
            let (reported_rule_id, fired) = report
                .fired
                .get(descriptor.rule_id as usize)
                .ok_or_else(|| {
                    format!(
                        "tick {tick}: internal firing report has no rule {}",
                        descriptor.rule_id
                    )
                })?;
            if *reported_rule_id != descriptor.rule_id {
                return Err(format!(
                    "tick {tick}: internal firing report rule mismatch: expected {}, found {}",
                    descriptor.rule_id, reported_rule_id
                ));
            }
            row.push(ReportedValue::Unsigned(*fired));
        }
        row.push(ReportedValue::Unsigned(
            report
                .deferred_per_resource_table
                .iter()
                .map(|(_, count)| count)
                .sum(),
        ));
        self.csv.push_str(&tick.to_string());
        for value in &row {
            self.csv.push(',');
            self.csv.push_str(&value.csv());
        }
        self.csv.push('\n');
        self.rows.push(row);
        self.tick_reports.push(report);
        Ok(())
    }

    pub(crate) fn finish(
        self,
        model: &sembla_ir::ValidatedModel,
        per_tick_hashes: PerTickHashes,
    ) -> Result<RunOutput, String> {
        let grouped = grouped_csv_outputs(model, &self.tick_reports)?;
        let summaries =
            executor::summarize(model, &self.tick_reports).map_err(|error| error.to_string())?;
        let summaries_csv = summaries_csv(&summaries);
        Ok(RunOutput {
            csv: self.csv,
            grouped,
            series: ReportedSeries {
                columns: self.headers.into_iter().skip(1).collect(),
                rows: self.rows,
            },
            summaries,
            summaries_csv,
            per_tick_hashes,
        })
    }
}

#[cfg(test)]
pub(crate) fn run_results_output(
    model: &sembla_ir::ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
) -> Result<RunOutput, String> {
    run_results_output_with_features(
        model,
        state,
        params,
        seed,
        ticks,
        HashMode::FinalOnly,
        &FeatureSet::new(),
    )
}

pub(crate) fn run_results_output_with_features(
    model: &sembla_ir::ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
    hash_mode: HashMode,
    enabled_features: &FeatureSet,
) -> Result<RunOutput, String> {
    let mut output = RunOutputAccumulator::new(model, params, ticks)?;
    let mut per_tick_hashes =
        (hash_mode == HashMode::EveryTick).then(|| Vec::with_capacity(ticks as usize));
    for tick in 0..ticks {
        let report =
            executor::run_tick_with_features(model, state, params, seed, tick, enabled_features)
                .map_err(|error| format!("tick {tick}: {error}"))?;
        output.push_tick(state, tick, report)?;
        if let Some(per_tick_hashes) = per_tick_hashes.as_mut() {
            per_tick_hashes.push(state.state_hash());
        }
    }
    output.finish(model, per_tick_hashes)
}

pub(crate) fn run_results_output_timed_with_features(
    model: &sembla_ir::ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
    hash_mode: HashMode,
    enabled_features: &FeatureSet,
) -> Result<(RunOutput, Vec<TickTiming>), String> {
    let mut output = RunOutputAccumulator::new(model, params, ticks)?;
    let mut per_tick_hashes =
        (hash_mode == HashMode::EveryTick).then(|| Vec::with_capacity(ticks as usize));
    let mut tick_timings = Vec::with_capacity(ticks as usize);
    for tick in 0..ticks {
        let tick_started = Instant::now();
        let timed = executor::run_tick_with_features_timed(
            model,
            state,
            params,
            seed,
            tick,
            enabled_features,
        )
        .map_err(|error| format!("tick {tick}: {error}"))?;

        let report_started = Instant::now();
        output.push_tick(state, tick, timed.report)?;
        let report = timed.phases.report + report_started.elapsed();

        let state_hash = if let Some(per_tick_hashes) = per_tick_hashes.as_mut() {
            let phase_started = Instant::now();
            per_tick_hashes.push(state.state_hash());
            phase_started.elapsed()
        } else {
            Duration::ZERO
        };

        let wall_time = tick_started.elapsed();
        tick_timings.push(finish_tick_timing(
            tick,
            wall_time,
            PhaseDurations {
                execute_tick: Some(timed.phases.execute_tick),
                state_hash: Some(state_hash),
                observe_views: Some(timed.phases.observe_views),
                report: Some(report),
                ..PhaseDurations::default()
            },
        )?);
    }
    Ok((output.finish(model, per_tick_hashes)?, tick_timings))
}

#[cfg(feature = "cuda")]
pub(crate) fn report_cuda_observation_eligibility(
    eligibility: &sembla_runtime::core::DeviceObservationEligibility,
) {
    eprintln!(
        "cuda_device_observation eligible={} reason={}",
        eligibility.eligible, eligibility.reason
    );
    for view in &eligibility.views {
        eprintln!(
            "cuda_device_observation_view box={:?} view={:?} eligible={} reason={}",
            view.box_name, view.name, view.eligible, view.reason
        );
    }
}

#[cfg(feature = "cuda")]
fn cuda_observations(
    model: &sembla_ir::ValidatedModel,
    state: &StateStore,
    params: &ParamEnv,
    device: Option<CudaDeviceObservations>,
) -> Result<CudaDeviceObservations, String> {
    match device {
        Some(observation) => Ok(observation),
        None => Ok(CudaDeviceObservations {
            views: executor::observe_views(model, state, params)
                .map_err(|error| error.to_string())?,
            grouped_views: executor::observe_grouped_views(model, state, params)
                .map_err(|error| error.to_string())?,
            generic_enum_counts: None,
        }),
    }
}

pub(crate) fn cuda_tick_report(
    model: &sembla_ir::ValidatedModel,
    tick: u32,
    fired_per_box: Vec<(String, Vec<(u32, usize)>)>,
    deferred_per_resource_table: Vec<(String, usize)>,
    views: Vec<sembla_runtime::core::ViewValue>,
    grouped_views: Vec<sembla_runtime::core::GroupedViewValue>,
) -> executor::TickReport {
    let fired = model
        .transitions()
        .iter()
        .map(|transition| {
            let count = fired_per_box
                .iter()
                .flat_map(|(_, rules)| rules)
                .find(|(rule_id, _)| *rule_id == transition.rule_id)
                .map_or(0, |(_, count)| *count);
            (transition.rule_id, count)
        })
        .collect();
    executor::TickReport {
        tick,
        views,
        grouped_views,
        fired,
        fired_per_box,
        deferred_per_resource_table,
        aggregate_builds: 0,
    }
}

#[cfg(not(feature = "cuda"))]
pub(crate) fn run_results_output_cuda(
    model: &sembla_ir::ValidatedModel,
    initial: Vec<TableInit>,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
    hash_mode: HashMode,
) -> Result<BackendRunOutput, String> {
    let mut state = StateStore::new(model, initial.clone()).map_err(|error| error.to_string())?;
    let mut backend = CudaBackend::new(model, initial, params, seed, hash_mode)
        .map_err(|error| error.to_string())?;
    let device = backend.device_identity().clone();
    let identity =
        manifest::BackendIdentity::cuda_native_f64(device.gpu_model, device.driver_version);
    let mut hashes = (hash_mode == HashMode::EveryTick).then(|| Vec::with_capacity(ticks as usize));
    let mut output = RunOutputAccumulator::new(model, params, ticks)?;

    let started = Instant::now();
    for tick in 0..ticks {
        let observation = backend
            .run_tick_observed()
            .map_err(|error| format!("tick {tick}: {error}"))?;
        debug_assert_eq!(observation.tick, tick);
        state = observation.state;
        if let Some(hashes) = hashes.as_mut() {
            hashes.push(state.state_hash());
        }
        let views = executor::observe_views(model, &state, params)
            .map_err(|error| format!("tick {tick}: {error}"))?;
        let grouped_views = executor::observe_grouped_views(model, &state, params)
            .map_err(|error| format!("tick {tick}: {error}"))?;
        let report = cuda_tick_report(
            model,
            tick,
            observation.fired_per_box,
            observation.deferred_per_resource_table,
            views,
            grouped_views,
        );
        output.push_tick(&state, tick, report)?;
    }
    let elapsed = started.elapsed();
    let output = output.finish(model, hashes.clone())?;
    Ok(BackendRunOutput {
        #[cfg(feature = "cuda")]
        construction_timing: None,
        output,
        state,
        identity,
        per_tick_hashes: hashes,
        elapsed,
    })
}

#[cfg(feature = "cuda")]
pub(crate) fn run_results_output_cuda(
    model: &sembla_ir::ValidatedModel,
    initial: Vec<TableInit>,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
    hash_mode: HashMode,
) -> Result<BackendRunOutput, String> {
    let mut backend = CudaBackend::new(model, initial, params, seed, hash_mode)
        .map_err(|error| error.to_string())?;
    let construction_timing = Some(backend.construction_timing());
    let device = backend.device_identity().clone();
    report_cuda_observation_eligibility(backend.observation_eligibility());
    let identity =
        manifest::BackendIdentity::cuda_native_f64(device.gpu_model, device.driver_version);
    let mut hashes = (hash_mode == HashMode::EveryTick).then(|| Vec::with_capacity(ticks as usize));
    let mut output = RunOutputAccumulator::new(model, params, ticks)?;

    let started = Instant::now();
    for tick in 0..ticks {
        let (observed_tick, fired_per_box, deferred_per_resource_table, device_views) = backend
            .run_tick_observed_reused()
            .map_err(|error| format!("tick {tick}: {error}"))?;
        debug_assert_eq!(observed_tick, tick);
        if let Some(hashes) = hashes.as_mut() {
            hashes.push(
                backend
                    .observed_hash()
                    .map_err(|error| format!("tick {tick}: {error}"))?,
            );
        }
        let observations = cuda_observations(model, backend.observed_state(), params, device_views)
            .map_err(|error| format!("tick {tick}: {error}"))?;
        let report = cuda_tick_report(
            model,
            tick,
            fired_per_box,
            deferred_per_resource_table,
            observations.views,
            observations.grouped_views,
        );
        output.push_tick_with_enum_counts(
            backend.observed_state(),
            tick,
            report,
            observations.generic_enum_counts.as_deref(),
        )?;
    }
    let output = output.finish(model, hashes.clone())?;
    let state = backend
        .into_observed_state()
        .map_err(|error| error.to_string())?;
    let elapsed = started.elapsed();
    Ok(BackendRunOutput {
        #[cfg(feature = "cuda")]
        construction_timing,
        output,
        state,
        identity,
        per_tick_hashes: hashes,
        elapsed,
    })
}

#[cfg(feature = "cuda")]
pub(crate) fn run_results_output_cuda_timed(
    model: &sembla_ir::ValidatedModel,
    initial: Vec<TableInit>,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
    hash_mode: HashMode,
) -> Result<(BackendRunOutput, Vec<TickTiming>), String> {
    let mut backend = CudaBackend::new(model, initial, params, seed, hash_mode)
        .map_err(|error| error.to_string())?;
    let construction_timing = Some(backend.construction_timing());
    let device = backend.device_identity().clone();
    report_cuda_observation_eligibility(backend.observation_eligibility());
    let identity =
        manifest::BackendIdentity::cuda_native_f64(device.gpu_model, device.driver_version);
    let mut hashes = (hash_mode == HashMode::EveryTick).then(|| Vec::with_capacity(ticks as usize));
    let mut output = RunOutputAccumulator::new(model, params, ticks)?;
    let mut tick_timings = Vec::with_capacity(ticks as usize);

    let started = Instant::now();
    for tick in 0..ticks {
        let tick_started = Instant::now();
        let (
            observed_tick,
            fired_per_box,
            deferred_per_resource_table,
            device_views,
            backend_phases,
        ) = backend
            .run_tick_observed_reused_timed()
            .map_err(|error| format!("tick {tick}: {error}"))?;
        debug_assert_eq!(observed_tick, tick);

        let state_hash = if let Some(hashes) = hashes.as_mut() {
            let phase_started = Instant::now();
            hashes.push(
                backend
                    .observed_hash()
                    .map_err(|error| format!("tick {tick}: {error}"))?,
            );
            phase_started.elapsed()
        } else {
            Duration::ZERO
        };

        let phase_started = Instant::now();
        let observations = cuda_observations(model, backend.observed_state(), params, device_views)
            .map_err(|error| format!("tick {tick}: {error}"))?;
        let observe_views = phase_started.elapsed();

        let phase_started = Instant::now();
        let report = cuda_tick_report(
            model,
            tick,
            fired_per_box,
            deferred_per_resource_table,
            observations.views,
            observations.grouped_views,
        );
        output.push_tick_with_enum_counts(
            backend.observed_state(),
            tick,
            report,
            observations.generic_enum_counts.as_deref(),
        )?;
        let report = backend_phases[4] + phase_started.elapsed();

        let wall_time = tick_started.elapsed();
        tick_timings.push(finish_tick_timing(
            tick,
            wall_time,
            PhaseDurations {
                kernels: Some(backend_phases[0]),
                readback_control: Some(backend_phases[1]),
                state_transfer: Some(backend_phases[2]),
                state_reconstruct: Some(backend_phases[3]),
                state_hash: Some(state_hash),
                observe_views: Some(observe_views),
                report: Some(report),
                ..PhaseDurations::default()
            },
        )?);
    }
    let output = output.finish(model, hashes.clone())?;
    let state = backend
        .into_observed_state()
        .map_err(|error| error.to_string())?;
    let elapsed = started.elapsed();
    Ok((
        BackendRunOutput {
            #[cfg(feature = "cuda")]
            construction_timing,
            output,
            state,
            identity,
            per_tick_hashes: hashes,
            elapsed,
        },
        tick_timings,
    ))
}

pub(crate) fn summaries_csv(summaries: &[SummaryValue]) -> String {
    let mut csv = String::from("name,value\n");
    for summary in summaries {
        csv.push_str(&csv_field(&summary.name));
        csv.push(',');
        csv.push_str(&ReportedValue::from(summary.value).csv());
        csv.push('\n');
    }
    csv
}

pub(crate) fn parent_occurrence_path(box_name: &str) -> &str {
    box_name.rsplit_once('/').map_or("", |(parent, _)| parent)
}
