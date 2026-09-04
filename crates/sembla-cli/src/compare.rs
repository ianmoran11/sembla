//! Model and parameter-arm comparison workflows.

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct CompareOptions {
    models: Vec<String>,
    population: String,
    seed: u64,
    ticks: u32,
    out: String,
    params_a: Option<String>,
    params_b: Option<String>,
    backend: BackendSelection,
    enabled_features: FeatureSet,
}

pub(crate) fn compare_command(arguments: &[String]) -> i32 {
    let options = match parse_compare_options(arguments) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return 2;
        }
    };
    match compare_result(options) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

pub(crate) fn parse_compare_options(arguments: &[String]) -> Result<CompareOptions, String> {
    let positional_count = arguments
        .iter()
        .position(|argument| argument.starts_with("--"))
        .unwrap_or(arguments.len());
    if !(1..=2).contains(&positional_count) {
        return Err(
            "compare requires one model or plan (parameter contrast) or two models or plans (model contrast)"
                .to_owned(),
        );
    }
    let models = arguments[..positional_count].to_vec();
    let flags = &arguments[positional_count..];
    if flags.len() % 2 != 0 {
        return Err(format!(
            "missing value for '{}'",
            flags.last().expect("odd flag list is nonempty")
        ));
    }
    let mut population = None;
    let mut seed = None;
    let mut ticks = None;
    let mut out = None;
    let mut params_a = None;
    let mut params_b = None;
    let mut backend = None;
    let mut enabled_features = FeatureSet::new();
    for pair in flags.chunks_exact(2) {
        let flag = pair[0].as_str();
        let value = pair[1].clone();
        match flag {
            "--population" => {
                if !Path::new(&value).is_file() {
                    return Err(format!("population file '{value}' does not exist"));
                }
                set_once(&mut population, value, flag)?;
            }
            "--seed" => set_once(&mut seed, parse_number(&value, flag)?, flag)?,
            "--ticks" => set_once(&mut ticks, parse_number(&value, flag)?, flag)?,
            "--out" => set_once(&mut out, value, flag)?,
            "--params-a" => set_once(&mut params_a, value, flag)?,
            "--params-b" => set_once(&mut params_b, value, flag)?,
            "--backend" => set_once(&mut backend, parse_backend(&value)?, flag)?,
            "--enable" => {
                enabled_features.insert(parse_feature(&value)?);
            }
            _ => return Err(format!("unknown compare flag '{flag}'")),
        }
    }
    match models.len() {
        1 if params_a.is_none() || params_b.is_none() => {
            return Err("parameter contrast requires both '--params-a' and '--params-b'".to_owned())
        }
        2 if params_a.is_some() || params_b.is_some() => {
            return Err("model contrast does not accept '--params-a' or '--params-b'".to_owned())
        }
        2 if !enabled_features.is_empty() => {
            return Err("model contrast does not accept '--enable'; feature-aware compare is limited to same-model parameter contrasts".to_owned())
        }
        _ => {}
    }
    Ok(CompareOptions {
        models,
        population: population.ok_or_else(|| "missing required flag '--population'".to_owned())?,
        seed: seed.ok_or_else(|| "missing required flag '--seed'".to_owned())?,
        ticks: ticks.ok_or_else(|| "missing required flag '--ticks'".to_owned())?,
        out: out.ok_or_else(|| "missing required flag '--out'".to_owned())?,
        params_a,
        params_b,
        backend: backend.unwrap_or_default(),
        enabled_features,
    })
}

#[derive(Clone, Debug)]
pub(crate) struct CompareTick {
    counts: [usize; 3],
    fired_infect: usize,
    fired_recover: usize,
    deferred_total: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct CompareArmOutcome {
    series: ReportedSeries,
    hashes: ExecutionHashes,
    identity: manifest::BackendIdentity,
}

pub(crate) fn legacy_compare_ticks(series: &ReportedSeries) -> Result<Vec<CompareTick>, String> {
    let column = |name: &str| {
        series
            .columns
            .iter()
            .position(|column| column == name)
            .ok_or_else(|| format!("compare arm is missing reported column '{name}'"))
    };
    let indices = [
        column("S")?,
        column("I")?,
        column("R")?,
        column("fired_infect")?,
        column("fired_recover")?,
        column("deferred_total")?,
    ];
    series
        .rows
        .iter()
        .map(|row| {
            Ok(CompareTick {
                counts: [
                    row[indices[0]].as_usize("compare S value")?,
                    row[indices[1]].as_usize("compare I value")?,
                    row[indices[2]].as_usize("compare R value")?,
                ],
                fired_infect: row[indices[3]].as_usize("compare infect firing")?,
                fired_recover: row[indices[4]].as_usize("compare recover firing")?,
                deferred_total: row[indices[5]].as_usize("compare deferred total")?,
            })
        })
        .collect()
}

pub(crate) fn reported_difference(
    left: ReportedValue,
    right: ReportedValue,
) -> Result<String, String> {
    match (left, right) {
        (ReportedValue::Unsigned(left), ReportedValue::Unsigned(right)) => {
            Ok((right as i128 - left as i128).to_string())
        }
        (ReportedValue::Int(left), ReportedValue::Int(right)) => {
            Ok((right as i128 - left as i128).to_string())
        }
        (ReportedValue::Real(left), ReportedValue::Real(right)) => Ok((right - left).to_string()),
        _ => Err("compare reported column changed numeric type between arms".to_owned()),
    }
}

pub(crate) fn compare_result(options: CompareOptions) -> Result<(), String> {
    let path_a = &options.models[0];
    let path_b = options.models.get(1).unwrap_or(path_a);
    let input_a = read_executable_input(path_a, None, &options.enabled_features)?;
    let input_b = read_executable_input(path_b, None, &options.enabled_features)?;
    if input_a.plan.is_some() != input_b.plan.is_some() {
        let (legacy_path, plan_path) = if input_a.plan.is_some() {
            (path_b, path_a)
        } else {
            (path_a, path_b)
        };
        return Err(format!(
            "compare requires both inputs to use the same identity scheme; got legacy model '{legacy_path}' and plan envelope '{plan_path}'"
        ));
    }
    let plan_identity = if options.models.len() == 1 {
        input_a
            .plan
            .as_ref()
            .map(manifest::plan_identity_tuples)
            .transpose()?
    } else {
        None
    };
    let model_a = input_a.model;
    let model_b = input_b.model;
    let params_a = resolve_params(&model_a, options.params_a.as_deref())?;
    let params_b = resolve_params(&model_b, options.params_b.as_deref())?;
    let initialized_a = initialized_tables(&model_a, &options.population)?;
    let initialized_b = initialized_tables(&model_b, &options.population)?;
    if initialized_a.state_hash != initialized_b.state_hash {
        return Err("compare arms resolved different initial state artifact hashes".to_owned());
    }
    let initial_state_hash = initialized_a.state_hash;
    let initial_a = initialized_a.tables;
    let initial_b = initialized_b.tables;
    let arm_a = compare_arm(
        &model_a,
        &params_a,
        initial_a,
        options.seed,
        options.ticks,
        options.backend,
        &options.enabled_features,
    )?;
    let arm_b = compare_arm(
        &model_b,
        &params_b,
        initial_b,
        options.seed,
        options.ticks,
        options.backend,
        &options.enabled_features,
    )?;

    let mut csv = String::new();
    csv.push_str("# arm_a_model=");
    csv.push_str(&model_a.model().name);
    csv.push('\n');
    csv.push_str("# arm_b_model=");
    csv.push_str(&model_b.model().name);
    csv.push('\n');
    csv.push_str("# arm_a_params=");
    csv.push_str(&canonical_params(&params_a)?);
    csv.push('\n');
    csv.push_str("# arm_b_params=");
    csv.push_str(&canonical_params(&params_b)?);
    csv.push('\n');
    csv.push_str(&format!("# seed={}\n", options.seed));
    csv.push_str(&format!("# dt_a={}\n", model_a.model().dt));
    csv.push_str(&format!("# dt_b={}\n", model_b.model().dt));

    let legacy_a = legacy_compare_ticks(&arm_a.series);
    let legacy_b = legacy_compare_ticks(&arm_b.series);
    match (legacy_a, legacy_b) {
        (Ok(ticks_a), Ok(ticks_b)) => {
            csv.push_str("tick,S_a,I_a,R_a,S_b,I_b,R_b,dS,dI,dR,fired_infect_a,fired_recover_a,deferred_a,fired_infect_b,fired_recover_b,deferred_b\n");
            for (tick, (tick_a, tick_b)) in ticks_a.iter().zip(&ticks_b).enumerate() {
                let difference = |a: usize, b: usize| b as i128 - a as i128;
                csv.push_str(&format!(
                    "{tick},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    tick_a.counts[0],
                    tick_a.counts[1],
                    tick_a.counts[2],
                    tick_b.counts[0],
                    tick_b.counts[1],
                    tick_b.counts[2],
                    difference(tick_a.counts[0], tick_b.counts[0]),
                    difference(tick_a.counts[1], tick_b.counts[1]),
                    difference(tick_a.counts[2], tick_b.counts[2]),
                    tick_a.fired_infect,
                    tick_a.fired_recover,
                    tick_a.deferred_total,
                    tick_b.fired_infect,
                    tick_b.fired_recover,
                    tick_b.deferred_total,
                ));
            }
        }
        (Err(error), _) | (_, Err(error)) if options.models.len() != 1 => return Err(error),
        _ => {
            if arm_a.series.columns != arm_b.series.columns {
                return Err(format!(
                    "generic parameter compare requires identical reported columns; arm_a={:?} arm_b={:?}",
                    arm_a.series.columns, arm_b.series.columns
                ));
            }
            let mut headers = vec!["tick".to_owned()];
            headers.extend(
                arm_a
                    .series
                    .columns
                    .iter()
                    .map(|column| format!("{column}_a")),
            );
            headers.extend(
                arm_b
                    .series
                    .columns
                    .iter()
                    .map(|column| format!("{column}_b")),
            );
            headers.extend(
                arm_a
                    .series
                    .columns
                    .iter()
                    .map(|column| format!("d{column}")),
            );
            csv.push_str(
                &headers
                    .iter()
                    .map(|header| csv_field(header))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            csv.push('\n');
            for (tick, (row_a, row_b)) in
                arm_a.series.rows.iter().zip(&arm_b.series.rows).enumerate()
            {
                csv.push_str(&tick.to_string());
                for value in row_a {
                    csv.push(',');
                    csv.push_str(&value.csv());
                }
                for value in row_b {
                    csv.push(',');
                    csv.push_str(&value.csv());
                }
                for (left, right) in row_a.iter().zip(row_b) {
                    csv.push(',');
                    csv.push_str(&reported_difference(*left, *right)?);
                }
                csv.push('\n');
            }
        }
    }

    write_atomic(&options.out, csv.as_bytes())?;
    let compare_sha256 = hex(&Sha256::digest(csv.as_bytes()));
    let (population_source, population_sha256) =
        manifest::population_identity(&options.population)?;
    let mut run_manifest = manifest::RunManifest::new(
        manifest::ManifestKind::Compare,
        options.seed,
        options.ticks,
        population_source,
        population_sha256,
    );
    run_manifest.backend_identity = Some(arm_a.identity.clone());
    run_manifest.initial_state = initial_state_hash.map(state_artifact_tuple);
    run_manifest.enabled_features = options.enabled_features.iter().cloned().collect();
    if let Some((identity, linked_source)) = plan_identity {
        run_manifest.plan = Some(identity);
        run_manifest.linked_source = linked_source;
    }
    if arm_a.identity != arm_b.identity {
        return Err("backend device identity changed between compare arms".to_owned());
    }
    run_manifest.results_sha256 = Some(compare_sha256.clone());
    for (k, scenario, model, params, arm) in [
        (0, "arm_a", &model_a, &params_a, &arm_a),
        (1, "arm_b", &model_b, &params_b, &arm_b),
    ] {
        run_manifest.executions.push(manifest::ManifestExecution {
            k,
            seed: None,
            scenario: Some(scenario.to_owned()),
            model: Some(model.model().name.clone()),
            ir_hash: Some(manifest::canonical_ir_hash(model)?),
            dt: Some(model.model().dt),
            resolved_theta: manifest::resolved_theta(params),
            results_sha256: arm.hashes.results_sha256.clone(),
            final_state_sha256: arm.hashes.final_state_sha256.clone(),
            observation_sha256: Some(arm.hashes.observation_sha256.clone()),
            grouped_outputs: Vec::new(),
        });
    }
    manifest::write(&manifest::sidecar_path(&options.out), &run_manifest)?;
    println!("compare_sha256={compare_sha256}");
    Ok(())
}

pub(crate) fn compare_arm(
    model: &sembla_ir::ValidatedModel,
    params: &ParamEnv,
    initial: Vec<TableInit>,
    seed: u64,
    ticks: u32,
    backend: BackendSelection,
    enabled_features: &FeatureSet,
) -> Result<CompareArmOutcome, String> {
    let execution = execute_backend_output_with_features(
        model,
        initial,
        params,
        seed,
        ticks,
        BackendRunMode::final_only(backend),
        enabled_features,
    )?;
    let hashes = execution_hashes(&execution.output, &execution.state);
    Ok(CompareArmOutcome {
        series: execution.output.series,
        hashes,
        identity: execution.identity,
    })
}
