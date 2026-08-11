//! Run-manifest replay and verification.

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct VerifyOptions {
    population: String,
    params: Option<String>,
    draw: Option<u32>,
}

pub(crate) fn parse_verify_options(flags: &[String]) -> Result<VerifyOptions, String> {
    let mut population = None;
    let mut params = None;
    let mut draw = None;
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index].as_str();
        let value = flags
            .get(index + 1)
            .ok_or_else(|| format!("missing value for '{flag}'"))?;
        match flag {
            "--population" => {
                if value.parse::<usize>().is_err() && !Path::new(value).is_file() {
                    return Err(format!(
                        "invalid numeric value or population file '{value}' for '{flag}'"
                    ));
                }
                set_once(&mut population, value.clone(), flag)?;
            }
            "--params" => set_once(&mut params, value.clone(), flag)?,
            "--draw" => set_once(&mut draw, parse_number(value, flag)?, flag)?,
            _ => return Err(format!("unknown verify-run flag '{flag}'")),
        }
        index += 2;
    }
    Ok(VerifyOptions {
        population: population.ok_or_else(|| "missing required flag '--population'".to_owned())?,
        params,
        draw,
    })
}

pub(crate) fn verify_run(manifest_path: &str, model_path: &str, options: VerifyOptions) -> i32 {
    match verify_run_result(manifest_path, model_path, options) {
        Ok(count) => {
            println!("verified {count} execution(s)");
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

pub(crate) fn verify_run_result(
    manifest_path: &str,
    model_path: &str,
    options: VerifyOptions,
) -> Result<usize, String> {
    let recorded = manifest::read(Path::new(manifest_path))?;
    // Replay derives runtime feature enablement from the recorded run contract.
    // Plan identity features describe the artifact and must not implicitly enable
    // execution; verify-run has no user override for this manifest-owned value.
    let enabled_features = recorded
        .enabled_features
        .iter()
        .cloned()
        .collect::<FeatureSet>();
    if recorded.manifest_kind == manifest::ManifestKind::Compare {
        return Err(
            "verify-run does not accept compare manifests because both original model inputs are required"
                .to_owned(),
        );
    }

    let dt = recorded
        .dt
        .ok_or_else(|| "manifest is missing required field 'dt'".to_owned())?;
    let (input_source, input) = read_input(model_path)?;
    let (model, plan) = match input {
        sembla_ir::ParsedInput::LegacyModel(mut raw_model) => {
            raw_model.dt = dt;
            let model = sembla_ir::validate_with_features(raw_model, &enabled_features)
                .map_err(|error| format!("{model_path}: {error}"))?;
            (model, None)
        }
        sembla_ir::ParsedInput::Plan(plan) => {
            let validated = sembla_ir::validate_plan(&plan)
                .map_err(|error| format!("{model_path}: {error}"))?;
            sembla_ir::validate_with_features(plan.model.clone(), &enabled_features)
                .map_err(|error| format!("{model_path}: {error}"))?;
            require_canonical_plan(model_path, &input_source)?;
            if plan.model.dt != dt {
                return Err(format!(
                    "verification mismatch:\n  dt: recorded={dt:?} plan={:?}",
                    plan.model.dt
                ));
            }
            (validated.model_with_rule_words(), Some(plan))
        }
    };
    let (population_source, population_sha256) =
        manifest::population_identity(&options.population)?;
    let backend = match recorded
        .backend_identity
        .as_ref()
        .map(|identity| identity.backend.as_str())
    {
        Some("cpu-oracle") => BackendSelection::Cpu,
        Some("cuda-native-f64") => BackendSelection::Cuda,
        other => return Err(format!("manifest has unsupported backend {other:?}")),
    };
    let mut expected_base = manifest::RunManifest::new(
        recorded.manifest_kind,
        recorded.seed,
        recorded.ticks,
        population_source.clone(),
        population_sha256.clone(),
    );
    expected_base
        .backend_identity
        .clone_from(&recorded.backend_identity);
    let mut differences = Vec::new();

    compare_field(
        "backend_identity",
        &recorded.backend_identity,
        &expected_base.backend_identity,
        &mut differences,
    );
    compare_field(
        "component_versions",
        &recorded.component_versions,
        &expected_base.component_versions,
        &mut differences,
    );
    compare_field(
        "determinism_level",
        &recorded.determinism_level,
        &expected_base.determinism_level,
        &mut differences,
    );
    compare_field(
        "enabled_flags",
        &recorded.enabled_flags,
        &expected_base.enabled_flags,
        &mut differences,
    );
    compare_field(
        "population_source",
        &recorded.population_source,
        &population_source,
        &mut differences,
    );
    compare_field(
        "population_sha256",
        &recorded.population_sha256,
        &population_sha256,
        &mut differences,
    );
    compare_field(
        "model",
        &recorded.model,
        &Some(model.model().name.clone()),
        &mut differences,
    );
    let expected_ir_hash =
        if plan.is_some() && recorded.manifest_kind == manifest::ManifestKind::Sweep {
            None
        } else {
            Some(manifest::canonical_ir_hash(&model)?)
        };
    compare_field(
        "ir_hash",
        &recorded.ir_hash,
        &expected_ir_hash,
        &mut differences,
    );
    let (expected_plan, expected_linked_source) = match plan.as_ref() {
        Some(plan) => {
            let (identity, linked_source) = manifest::plan_identity_tuples(plan)?;
            (Some(identity), linked_source)
        }
        None => (None, None),
    };
    compare_field("plan", &recorded.plan, &expected_plan, &mut differences);
    compare_field(
        "linked_source",
        &recorded.linked_source,
        &expected_linked_source,
        &mut differences,
    );

    match recorded.manifest_kind {
        manifest::ManifestKind::Run => {
            if let Some(params_path) = options.params.as_deref() {
                let supplied = resolve_params(&model, Some(params_path))?;
                compare_field(
                    "resolved_theta",
                    &recorded.resolved_theta,
                    &manifest::resolved_theta(&supplied),
                    &mut differences,
                );
            }
            let params = params_from_manifest(&model, &recorded.resolved_theta)?;
            let execution = execute_backend_output_with_features(
                &model,
                initialized_tables(&model, &options.population)?.tables,
                &params,
                recorded.seed,
                recorded.ticks,
                BackendRunMode::final_only(backend),
                &enabled_features,
            )?;
            compare_field(
                "backend_identity",
                &recorded.backend_identity,
                &Some(execution.identity.clone()),
                &mut differences,
            );
            let actual = execution_hashes(&execution.output, &execution.state);
            compare_field(
                "results_sha256",
                &recorded.results_sha256,
                &Some(actual.results_sha256),
                &mut differences,
            );
            compare_field(
                "final_state_sha256",
                &recorded.final_state_sha256,
                &Some(actual.final_state_sha256),
                &mut differences,
            );
            if recorded.observation_sha256.is_some() {
                compare_field(
                    "observation_sha256",
                    &recorded.observation_sha256,
                    &Some(actual.observation_sha256),
                    &mut differences,
                );
            }
            compare_field(
                "grouped_outputs",
                &recorded.grouped_outputs,
                &grouped_output_records(&execution.output.grouped),
                &mut differences,
            );
            finish_verification(differences, 1)
        }
        manifest::ManifestKind::Sweep => {
            let executions = match options.draw {
                Some(draw) => vec![recorded
                    .executions
                    .iter()
                    .find(|execution| execution.k == draw)
                    .ok_or_else(|| format!("manifest has no sweep execution with k={draw}"))?],
                None => recorded.executions.iter().collect::<Vec<_>>(),
            };
            if executions.is_empty() {
                return Err("sweep manifest contains no executions".to_owned());
            }
            let supplied_pins = match options.params.as_deref() {
                Some(path) => read_param_overrides(&model, path)?,
                None => Vec::new(),
            };
            for execution in &executions {
                for pin in &supplied_pins {
                    let expected = manifest::ResolvedValue::from(&pin.value);
                    compare_field(
                        &format!("executions[{}].resolved_theta.{}", execution.k, pin.name),
                        &execution.resolved_theta.get(&pin.name),
                        &Some(&expected),
                        &mut differences,
                    );
                }
                let expected_seed = match recorded.noise_mode {
                    Some(manifest::NoiseMode::Independent) => {
                        derive_sweep_replica_seed(recorded.seed, execution.k)
                    }
                    Some(manifest::NoiseMode::Crn) | None => recorded.seed,
                };
                if recorded.noise_mode.is_some() || execution.seed.is_some() {
                    compare_field(
                        &format!("executions[{}].seed", execution.k),
                        &execution.seed,
                        &Some(expected_seed),
                        &mut differences,
                    );
                }
                let params = params_from_manifest(&model, &execution.resolved_theta)?;
                let replay = execute_backend_output_with_features(
                    &model,
                    initialized_tables(&model, &options.population)?.tables,
                    &params,
                    execution.seed.unwrap_or(recorded.seed),
                    recorded.ticks,
                    BackendRunMode::final_only(backend),
                    &enabled_features,
                )?;
                compare_field(
                    "backend_identity",
                    &recorded.backend_identity,
                    &Some(replay.identity.clone()),
                    &mut differences,
                );
                let actual = execution_hashes(&replay.output, &replay.state);
                compare_field(
                    &format!("executions[{}].results_sha256", execution.k),
                    &execution.results_sha256,
                    &actual.results_sha256,
                    &mut differences,
                );
                compare_field(
                    &format!("executions[{}].final_state_sha256", execution.k),
                    &execution.final_state_sha256,
                    &actual.final_state_sha256,
                    &mut differences,
                );
                if execution.observation_sha256.is_some() {
                    compare_field(
                        &format!("executions[{}].observation_sha256", execution.k),
                        &execution.observation_sha256,
                        &Some(actual.observation_sha256),
                        &mut differences,
                    );
                }
                compare_field(
                    &format!("executions[{}].grouped_outputs", execution.k),
                    &execution.grouped_outputs,
                    &grouped_output_records(&replay.output.grouped),
                    &mut differences,
                );
            }
            finish_verification(differences, executions.len())
        }
        manifest::ManifestKind::Compare => unreachable!("handled above"),
    }
}

pub(crate) fn compare_field<T: std::fmt::Debug + PartialEq>(
    field: &str,
    recorded: &T,
    actual: &T,
    differences: &mut Vec<String>,
) {
    if recorded != actual {
        differences.push(format!("{field}: recorded={recorded:?} actual={actual:?}"));
    }
}

pub(crate) fn finish_verification(differences: Vec<String>, count: usize) -> Result<usize, String> {
    if differences.is_empty() {
        Ok(count)
    } else {
        Err(format!(
            "verification mismatch:\n  {}",
            differences.join("\n  ")
        ))
    }
}
