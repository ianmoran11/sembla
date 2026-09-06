//! CPU/CUDA differential workflows.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiffInputMode {
    Single,
    AllExamples,
    AllPlanFixtures,
}

#[derive(Clone, Debug)]
pub(crate) struct DiffOptions {
    pub(crate) models: Vec<String>,
    pub(crate) population: String,
    pub(crate) seed: u64,
    pub(crate) ticks: u32,
    pub(crate) dt: Option<f64>,
    pub(crate) params: Option<String>,
    pub(crate) enabled_features: FeatureSet,
}

pub(crate) fn diff_backends_command(arguments: &[String]) -> i32 {
    match parse_diff_options(arguments).and_then(diff_backends) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

pub(crate) fn parse_diff_options(arguments: &[String]) -> Result<DiffOptions, String> {
    let selector = arguments.first().ok_or_else(|| {
        "diff-backends requires a model or plan path, --all-examples, or --all-plan-fixtures"
            .to_owned()
    })?;
    let (mode, model) = match selector.as_str() {
        "--all-examples" => (DiffInputMode::AllExamples, None),
        "--all-plan-fixtures" => (DiffInputMode::AllPlanFixtures, None),
        value if value.starts_with("--") => {
            return Err(format!("unknown diff-backends input selector '{value}'"));
        }
        value => (DiffInputMode::Single, Some(value.to_owned())),
    };

    let mut population = None;
    let mut seed = None;
    let mut ticks = None;
    let mut dt = None;
    let mut params = None;
    let mut enabled_features = FeatureSet::new();
    let mut index = 1;
    while index < arguments.len() {
        let flag = &arguments[index];
        if matches!(flag.as_str(), "--all-examples" | "--all-plan-fixtures") {
            return Err(format!(
                "diff-backends input selector '{selector}' cannot be combined with '{flag}'"
            ));
        }
        if !flag.starts_with("--") {
            return Err(format!(
                "diff-backends input selector '{selector}' cannot be combined with positional path '{flag}'"
            ));
        }
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| "diff-backends flags require values".to_owned())?;
        match flag.as_str() {
            "--population" => set_once(&mut population, value.clone(), "--population")?,
            "--seed" => set_once(&mut seed, parse_number(value, "--seed")?, "--seed")?,
            "--ticks" => set_once(&mut ticks, parse_number(value, "--ticks")?, "--ticks")?,
            "--dt" => {
                let value: f64 = parse_number(value, "--dt")?;
                if !value.is_finite() || value <= 0.0 {
                    return Err("'--dt' must be finite and greater than zero".to_owned());
                }
                set_once(&mut dt, value, "--dt")?;
            }
            "--params" => set_once(&mut params, value.clone(), "--params")?,
            "--enable" => {
                enabled_features.insert(parse_feature(value)?);
            }
            flag => return Err(format!("unknown diff-backends flag '{flag}'")),
        }
        index += 2;
    }

    if mode != DiffInputMode::Single && params.is_some() {
        return Err("'--params' is only valid for a single diff-backends input".to_owned());
    }
    if mode == DiffInputMode::AllPlanFixtures && dt.is_some() {
        return Err(
            "plan envelopes do not support --dt overrides; edit and re-canonicalize the plan instead"
                .to_owned(),
        );
    }

    let models = match mode {
        DiffInputMode::Single => vec![model.expect("single input has a model or plan path")],
        DiffInputMode::AllExamples => collect_diff_corpus_paths("examples", ".json")?,
        DiffInputMode::AllPlanFixtures => {
            let mut paths = collect_diff_corpus_paths("fixtures/plans", ".plan.json")?;
            paths.extend(collect_diff_corpus_paths(
                "fixtures/plans/linked",
                ".plan.json",
            )?);
            paths.sort();
            paths
        }
    };
    let population = population.unwrap_or_else(|| "100".to_owned());
    if mode != DiffInputMode::Single && population.parse::<usize>().is_err() {
        let selector = match mode {
            DiffInputMode::AllExamples => "--all-examples",
            DiffInputMode::AllPlanFixtures => "--all-plan-fixtures",
            DiffInputMode::Single => unreachable!(),
        };
        return Err(format!("'{selector}' requires a numeric '--population'"));
    }
    Ok(DiffOptions {
        models,
        population,
        seed: seed.unwrap_or(1),
        ticks: ticks.unwrap_or(10),
        dt,
        params,
        enabled_features,
    })
}

pub(crate) fn collect_diff_corpus_paths(
    directory: &str,
    suffix: &str,
) -> Result<Vec<String>, String> {
    let mut paths = std::fs::read_dir(directory)
        .map_err(|error| format!("{directory}: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.ends_with(suffix))
        })
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

pub(crate) fn compare_per_tick_hashes<'a>(
    path: &str,
    cpu: &'a PerTickHashes,
    cuda: &'a PerTickHashes,
) -> Result<ComparedPerTickHashes<'a>, String> {
    let cpu = cpu.as_deref().ok_or_else(|| {
        format!("{path}: internal invariant violation: cpu per-tick hashes are absent")
    })?;
    let cuda = cuda.as_deref().ok_or_else(|| {
        format!("{path}: internal invariant violation: cuda per-tick hashes are absent")
    })?;
    if cpu.len() != cuda.len() {
        return Err(format!(
            "{path}: per-tick hash sequence lengths differ: cpu={} cuda={}",
            cpu.len(),
            cuda.len()
        ));
    }
    if let Some((tick, (cpu_hash, cuda_hash))) = cpu
        .iter()
        .zip(cuda)
        .enumerate()
        .find(|(_, (cpu, cuda))| cpu != cuda)
    {
        return Err(format!(
            "{}: first divergence at tick {tick}: cpu={} cuda={}",
            path,
            hex(cpu_hash),
            hex(cuda_hash)
        ));
    }
    Ok((cpu, cuda))
}

pub(crate) fn diff_backends(options: DiffOptions) -> Result<(), String> {
    for path in &options.models {
        let run_options = RunOptions {
            seed: options.seed,
            ticks: options.ticks,
            population: options.population.clone(),
            out: None,
            export_state: None,
            dt: options.dt,
            params: options.params.clone(),
            timing_json: None,
            lifecycle_timing_json: None,
            backend: BackendSelection::Cpu,
            enabled_features: options.enabled_features.clone(),
        };
        let model = read_run_input(path, &run_options)?.model;
        let params = resolve_params(&model, options.params.as_deref())?;
        let initial = initialized_tables(&model, &options.population)?.tables;
        let cpu = execute_backend_output_with_features(
            &model,
            initial.clone(),
            &params,
            options.seed,
            options.ticks,
            BackendRunMode::every_tick(BackendSelection::Cpu),
            &options.enabled_features,
        )?;
        let cuda = execute_backend_output_with_features(
            &model,
            initial,
            &params,
            options.seed,
            options.ticks,
            BackendRunMode::every_tick(BackendSelection::Cuda),
            &options.enabled_features,
        )?;
        let (cpu_per_tick_hashes, cuda_per_tick_hashes) =
            compare_per_tick_hashes(path, &cpu.per_tick_hashes, &cuda.per_tick_hashes)?;
        let cpu_final = cpu.state.state_hash();
        let cuda_final = cuda.state.state_hash();
        if cpu_final != cuda_final {
            return Err(format!(
                "{path}: final state differs: cpu={} cuda={}",
                hex(&cpu_final),
                hex(&cuda_final)
            ));
        }
        if cpu.output.csv.as_bytes() != cuda.output.csv.as_bytes() {
            if let Some(tick) = cpu
                .output
                .series
                .rows
                .iter()
                .zip(&cuda.output.series.rows)
                .position(|(cpu_row, cuda_row)| {
                    cpu_row.len() != cuda_row.len()
                        || cpu_row
                            .iter()
                            .zip(cuda_row)
                            .any(|(cpu_value, cuda_value)| cpu_value.csv() != cuda_value.csv())
                })
            {
                return Err(format!(
                    "{}: first divergence at tick {tick}: cpu={} cuda={}; results bytes differ",
                    path,
                    hex(&cpu_per_tick_hashes[tick]),
                    hex(&cuda_per_tick_hashes[tick])
                ));
            }
            return Err(format!(
                "{path}: results metadata bytes differ after matching state hashes"
            ));
        }
        if cpu.output.summaries_csv.as_bytes() != cuda.output.summaries_csv.as_bytes() {
            return Err(format!(
                "{path}: summaries bytes differ after matching state hashes"
            ));
        }
        if cpu.output.grouped.len() != cuda.output.grouped.len()
            || cpu.output.grouped.iter().zip(&cuda.output.grouped).any(
                |(cpu_grouped, cuda_grouped)| {
                    cpu_grouped.view != cuda_grouped.view
                        || cpu_grouped.csv.as_bytes() != cuda_grouped.csv.as_bytes()
                },
            )
        {
            return Err(format!(
                "{path}: grouped observation bytes differ after matching state hashes"
            ));
        }
        let rate = |elapsed: std::time::Duration| {
            if elapsed.as_secs_f64() == 0.0 {
                0.0
            } else {
                f64::from(options.ticks) / elapsed.as_secs_f64()
            }
        };
        println!(
            "model={} verdict=equal cpu_ticks_per_sec={:.3} cuda_ticks_per_sec={:.3}",
            path,
            rate(cpu.elapsed),
            rate(cuda.elapsed)
        );
    }
    if options.models.len() > 1 {
        println!("corpus_verdict=equal models={}", options.models.len());
    }
    Ok(())
}
