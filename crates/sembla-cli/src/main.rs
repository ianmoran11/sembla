use std::path::Path;

use sembla_ir::{AttrType, ParamType, ParamValue};
use sembla_runtime::eval::{ParamEnv, ParamOverride};
use sembla_runtime::executor;
use sembla_runtime::population::SyntheticPopulation;
use sembla_runtime::prior::sample_parameters_for_draw;
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};
use sha2::{Digest, Sha256};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = "usage: sembla --version | sembla validate <path> | sembla diff-ir <a.json> <b.json> | sembla synth-pop --persons N --employers E --initial-infected I --seed S --out pop.bin | sembla run <model.json> --seed N --ticks K --population N|pop.bin [--out results.csv] [--dt D] [--params file.json] | sembla sweep <model.json> --population N|pop.bin --seed S --draws K --ticks T --out dir [--params file.json] | sembla compare <modelA.json> <modelB.json> --population pop.bin --seed N --ticks K --out compare.csv | sembla compare <model.json> --population pop.bin --seed N --ticks K --params-a a.json --params-b b.json --out compare.csv";

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let exit_code = run(&arguments);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn run(arguments: &[String]) -> i32 {
    match arguments {
        [flag] if flag == "--version" => {
            println!("sembla {VERSION}");
            0
        }
        [command, path] if command == "validate" => validate_file(path),
        [command, left, right] if command == "diff-ir" => diff_ir(left, right),
        [command, flags @ ..] if command == "synth-pop" => {
            let options = match parse_synth_options(flags) {
                Ok(options) => options,
                Err(message) => {
                    eprintln!("{message}\n{USAGE}");
                    return 2;
                }
            };
            synth_population(options)
        }
        [command, path, flags @ ..] if command == "run" => {
            let options = match parse_run_options(flags) {
                Ok(options) => options,
                Err(message) => {
                    eprintln!("{message}\n{USAGE}");
                    return 2;
                }
            };
            run_file(path, options)
        }
        [command, path, flags @ ..] if command == "sweep" => {
            let options = match parse_sweep_options(flags) {
                Ok(options) => options,
                Err(message) => {
                    eprintln!("{message}\n{USAGE}");
                    return 2;
                }
            };
            sweep_file(path, options)
        }
        [command, arguments @ ..] if command == "compare" => compare_command(arguments),
        _ => {
            eprintln!("{USAGE}");
            2
        }
    }
}

#[derive(Clone, Debug)]
struct RunOptions {
    seed: u64,
    ticks: u32,
    population: String,
    out: Option<String>,
    dt: Option<f64>,
    params: Option<String>,
}

fn parse_run_options(flags: &[String]) -> Result<RunOptions, String> {
    let mut seed = None;
    let mut ticks = None;
    let mut population = None;
    let mut out = None;
    let mut dt = None;
    let mut params = None;
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
            "--dt" => {
                let value: f64 = parse_number(value, flag)?;
                if !value.is_finite() || value <= 0.0 {
                    return Err("'--dt' must be finite and greater than zero".to_owned());
                }
                set_once(&mut dt, value, flag)?;
            }
            "--params" => set_once(&mut params, value.clone(), flag)?,
            _ => return Err(format!("unknown run flag '{flag}'")),
        }
        index += 2;
    }
    Ok(RunOptions {
        seed: seed.ok_or_else(|| "missing required flag '--seed'".to_owned())?,
        ticks: ticks.ok_or_else(|| "missing required flag '--ticks'".to_owned())?,
        population: population.ok_or_else(|| "missing required flag '--population'".to_owned())?,
        out,
        dt,
        params,
    })
}

#[derive(Clone, Debug)]
struct SweepOptions {
    seed: u64,
    draws: u32,
    ticks: u32,
    population: String,
    out: String,
    params: Option<String>,
}

fn parse_sweep_options(flags: &[String]) -> Result<SweepOptions, String> {
    let mut seed = None;
    let mut draws = None;
    let mut ticks = None;
    let mut population = None;
    let mut out = None;
    let mut params = None;
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index].as_str();
        let value = flags
            .get(index + 1)
            .ok_or_else(|| format!("missing value for '{flag}'"))?;
        match flag {
            "--seed" => set_once(&mut seed, parse_number(value, flag)?, flag)?,
            "--draws" => set_once(&mut draws, parse_number(value, flag)?, flag)?,
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
            "--params" => set_once(&mut params, value.clone(), flag)?,
            _ => return Err(format!("unknown sweep flag '{flag}'")),
        }
        index += 2;
    }
    let draws = draws.ok_or_else(|| "missing required flag '--draws'".to_owned())?;
    if draws == 0 {
        return Err("'--draws' must be greater than zero".to_owned());
    }
    Ok(SweepOptions {
        seed: seed.ok_or_else(|| "missing required flag '--seed'".to_owned())?,
        draws,
        ticks: ticks.ok_or_else(|| "missing required flag '--ticks'".to_owned())?,
        population: population.ok_or_else(|| "missing required flag '--population'".to_owned())?,
        out: out.ok_or_else(|| "missing required flag '--out'".to_owned())?,
        params,
    })
}

#[derive(Clone, Debug)]
struct CompareOptions {
    models: Vec<String>,
    population: String,
    seed: u64,
    ticks: u32,
    out: String,
    params_a: Option<String>,
    params_b: Option<String>,
}

fn compare_command(arguments: &[String]) -> i32 {
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

fn parse_compare_options(arguments: &[String]) -> Result<CompareOptions, String> {
    let positional_count = arguments
        .iter()
        .position(|argument| argument.starts_with("--"))
        .unwrap_or(arguments.len());
    if !(1..=2).contains(&positional_count) {
        return Err(
            "compare requires one model (parameter contrast) or two models (model contrast)"
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
    })
}

#[derive(Clone, Debug)]
struct SynthOptions {
    persons: usize,
    employers: usize,
    initial_infected: usize,
    seed: u64,
    out: String,
}

fn parse_synth_options(flags: &[String]) -> Result<SynthOptions, String> {
    let mut persons = None;
    let mut employers = None;
    let mut initial_infected = None;
    let mut seed = None;
    let mut out = None;
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index].as_str();
        let value = flags
            .get(index + 1)
            .ok_or_else(|| format!("missing value for '{flag}'"))?;
        match flag {
            "--persons" => set_once(&mut persons, parse_number(value, flag)?, flag)?,
            "--employers" => set_once(&mut employers, parse_number(value, flag)?, flag)?,
            "--initial-infected" => {
                set_once(&mut initial_infected, parse_number(value, flag)?, flag)?
            }
            "--seed" => set_once(&mut seed, parse_number(value, flag)?, flag)?,
            "--out" => set_once(&mut out, value.clone(), flag)?,
            _ => return Err(format!("unknown synth-pop flag '{flag}'")),
        }
        index += 2;
    }
    Ok(SynthOptions {
        persons: persons.ok_or_else(|| "missing required flag '--persons'".to_owned())?,
        employers: employers.ok_or_else(|| "missing required flag '--employers'".to_owned())?,
        initial_infected: initial_infected
            .ok_or_else(|| "missing required flag '--initial-infected'".to_owned())?,
        seed: seed.ok_or_else(|| "missing required flag '--seed'".to_owned())?,
        out: out.ok_or_else(|| "missing required flag '--out'".to_owned())?,
    })
}

fn parse_number<T: std::str::FromStr>(value: &str, flag: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid numeric value '{value}' for '{flag}'"))
}

fn set_once<T>(slot: &mut Option<T>, value: T, flag: &str) -> Result<(), String> {
    if slot.is_some() {
        Err(format!("duplicate flag '{flag}'"))
    } else {
        *slot = Some(value);
        Ok(())
    }
}

fn read_model(path: &str) -> Result<sembla_ir::Model, String> {
    let source = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    sembla_ir::parse_json(&source).map_err(|error| format!("{path}: {error}"))
}

fn read_validated(path: &str) -> Result<sembla_ir::ValidatedModel, String> {
    sembla_ir::validate(read_model(path)?).map_err(|error| format!("{path}: {error}"))
}

fn validate_file(path: &str) -> i32 {
    match read_validated(path) {
        Ok(_) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn diff_ir(left: &str, right: &str) -> i32 {
    let compare = || -> Result<bool, String> {
        let left_model = read_validated(left)?;
        let right_model = read_validated(right)?;
        let left_json = sembla_ir::to_canonical_json(left_model.model())
            .map_err(|error| format!("{left}: canonical serialization failed: {error}"))?;
        let right_json = sembla_ir::to_canonical_json(right_model.model())
            .map_err(|error| format!("{right}: canonical serialization failed: {error}"))?;
        Ok(left_json == right_json)
    };
    match compare() {
        Ok(true) => {
            println!("IR models are semantically identical");
            0
        }
        Ok(false) => {
            eprintln!("IR models differ after canonical normalization: '{left}' != '{right}'");
            1
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn synth_population(options: SynthOptions) -> i32 {
    let result = SyntheticPopulation::generate(
        options.persons,
        options.employers,
        options.initial_infected,
        options.seed,
    )
    .and_then(|population| population.write(&options.out));
    match result {
        Ok(()) => {
            println!(
                "persons={} employers={} initial_infected={} population={}",
                options.persons, options.employers, options.initial_infected, options.out
            );
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_file(path: &str, options: RunOptions) -> i32 {
    match run_file_result(path, options) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run_file_result(path: &str, options: RunOptions) -> Result<(), String> {
    let mut raw_model = read_model(path)?;
    if let Some(dt) = options.dt {
        raw_model.dt = dt;
    }
    let model = sembla_ir::validate(raw_model).map_err(|error| format!("{path}: {error}"))?;
    let initial = match options.population.parse::<usize>() {
        Ok(population) => initialize_population(&model, population),
        Err(_) => initializers_from_population(
            &model,
            &SyntheticPopulation::read(&options.population).map_err(|error| error.to_string())?,
        )?,
    };
    let mut state = StateStore::new(&model, initial).map_err(|error| format!("{path}: {error}"))?;
    let params = resolve_params(&model, options.params.as_deref())?;

    if let Some(out) = options.out.as_deref() {
        run_results(
            &model,
            &mut state,
            &params,
            options.seed,
            options.ticks,
            out,
        )
    } else {
        let report = executor::run(&model, &mut state, &params, options.seed, options.ticks)
            .map_err(|error| format!("{path}: {error}"))?;
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
        Ok(())
    }
}

fn sweep_file(path: &str, options: SweepOptions) -> i32 {
    match sweep_file_result(path, options) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn sweep_file_result(path: &str, options: SweepOptions) -> Result<(), String> {
    let model = read_validated(path)?;
    if optional_sir_box_name(&model)?.is_none() {
        return Err(
            "sweep summary currently requires exactly one SIR person/employer box; use sembla run for generic models"
                .to_owned(),
        );
    }
    let pinned = match options.params.as_deref() {
        Some(params_path) => read_param_overrides(&model, params_path)?,
        None => Vec::new(),
    };
    let population = options.population.parse::<usize>().ok();
    let population_file = if population.is_none() {
        Some(SyntheticPopulation::read(&options.population).map_err(|error| error.to_string())?)
    } else {
        None
    };
    let out = Path::new(&options.out);
    std::fs::create_dir_all(out).map_err(|error| format!("{}: {error}", out.display()))?;
    remove_previous_sweep_outputs(out)?;

    let mut manifest = String::from("# parameter_status");
    for declaration in &model.model().params {
        let status = if pinned.iter().any(|pin| pin.name == declaration.name) {
            "pinned"
        } else if declaration.prior.is_some() {
            "sampled"
        } else {
            "default"
        };
        manifest.push_str(&format!(",{}={status}", declaration.name));
    }
    manifest.push_str("\nk");
    for declaration in &model.model().params {
        manifest.push(',');
        manifest.push_str(&declaration.name);
    }
    manifest.push('\n');

    let mut all_series = Vec::with_capacity(options.draws as usize);
    // Deliberately sequential: declaration order within each k, then k order.
    for draw in 0..options.draws {
        let params = sample_parameters_for_draw(&model, options.seed, draw, &pinned)
            .map_err(|error| format!("draw {draw}: {error}"))?;
        manifest.push_str(&draw.to_string());
        for (_, value) in params.values() {
            manifest.push(',');
            manifest.push_str(&param_value_csv(value));
        }
        manifest.push('\n');

        let initial = match (&population, &population_file) {
            (Some(row_count), None) => initialize_population(&model, *row_count),
            (None, Some(population)) => initializers_from_population(&model, population)?,
            _ => return Err("invalid sweep population source".to_owned()),
        };
        let mut state =
            StateStore::new(&model, initial).map_err(|error| format!("{path}: {error}"))?;
        // The simulation seed is intentionally identical across k: this is
        // common random numbers, so only theta varies between unpinned draws.
        let (csv, series) =
            run_sir_results_csv(&model, &mut state, &params, options.seed, options.ticks)?;
        let draw_path = out.join(format!("draw_{draw}.csv"));
        std::fs::write(&draw_path, csv.as_bytes())
            .map_err(|error| format!("{}: {error}", draw_path.display()))?;
        all_series.push(series);
    }

    let summary = summary_csv(&all_series, options.ticks);
    let manifest_path = out.join("manifest.csv");
    let summary_path = out.join("summary.csv");
    std::fs::write(&manifest_path, manifest.as_bytes())
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    std::fs::write(&summary_path, summary.as_bytes())
        .map_err(|error| format!("{}: {error}", summary_path.display()))?;
    println!(
        "manifest_sha256={} summary_sha256={}",
        hex(&Sha256::digest(manifest.as_bytes())),
        hex(&Sha256::digest(summary.as_bytes()))
    );
    Ok(())
}

fn remove_previous_sweep_outputs(directory: &Path) -> Result<(), String> {
    for entry in
        std::fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?
    {
        let path = entry
            .map_err(|error| format!("{}: {error}", directory.display()))?
            .path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name == "manifest.csv"
            || name == "summary.csv"
            || (name.starts_with("draw_") && name.ends_with(".csv"))
        {
            std::fs::remove_file(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    Ok(())
}

fn param_value_csv(value: &ParamValue) -> String {
    match value {
        ParamValue::Real { value } => value.to_string(),
        ParamValue::Int { value } => value.to_string(),
    }
}

fn summary_csv(all_series: &[Vec<[usize; 6]>], ticks: u32) -> String {
    const NAMES: [&str; 6] = [
        "S",
        "I",
        "R",
        "fired_infect",
        "fired_recover",
        "deferred_total",
    ];
    const PERCENTILES: [usize; 5] = [5, 25, 50, 75, 95];
    let mut csv = String::from("tick");
    for name in NAMES {
        for percentile in PERCENTILES {
            csv.push_str(&format!(",{name}_p{percentile:02}"));
        }
    }
    csv.push('\n');
    for tick in 0..ticks as usize {
        csv.push_str(&tick.to_string());
        for column in 0..NAMES.len() {
            let mut values = all_series
                .iter()
                .map(|series| series[tick][column])
                .collect::<Vec<_>>();
            values.sort_unstable();
            for percentile in PERCENTILES {
                // Deterministic nearest index to p * (n - 1).
                let index = ((values.len() - 1) * percentile + 50) / 100;
                csv.push(',');
                csv.push_str(&values[index].to_string());
            }
        }
        csv.push('\n');
    }
    csv
}

fn resolve_params(
    model: &sembla_ir::ValidatedModel,
    path: Option<&str>,
) -> Result<ParamEnv, String> {
    let Some(path) = path else {
        return Ok(ParamEnv::defaults(model));
    };
    let overrides = read_param_overrides(model, path)?;
    ParamEnv::resolve(model, &overrides).map_err(|error| format!("{path}: {error}"))
}

fn read_param_overrides(
    model: &sembla_ir::ValidatedModel,
    path: &str,
) -> Result<Vec<ParamOverride>, String> {
    let source = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&source).map_err(|error| format!("{path}: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("{path}: parameter overrides must be a JSON object"))?;
    let mut overrides = Vec::with_capacity(object.len());
    for (name, value) in object {
        let declaration = model
            .model()
            .params
            .iter()
            .find(|parameter| parameter.name == *name)
            .ok_or_else(|| format!("{path}: unknown parameter '{name}'"))?;
        let value = match declaration.ty {
            ParamType::Real => ParamValue::Real {
                value: value
                    .as_f64()
                    .ok_or_else(|| format!("{path}: parameter '{name}' must have type real"))?,
            },
            ParamType::Int => ParamValue::Int {
                value: value
                    .as_i64()
                    .ok_or_else(|| format!("{path}: parameter '{name}' must have type int"))?,
            },
        };
        overrides.push(ParamOverride::new(name, value));
    }
    ParamEnv::resolve(model, &overrides).map_err(|error| format!("{path}: {error}"))?;
    Ok(overrides)
}

fn run_results(
    model: &sembla_ir::ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
    out: &str,
) -> Result<(), String> {
    let csv = match optional_sir_box_name(model)? {
        Some(_) => run_sir_results_csv(model, state, params, seed, ticks)?.0,
        None => run_generic_results_csv(model, state, params, seed, ticks)?.0,
    };
    std::fs::write(out, csv.as_bytes()).map_err(|error| format!("{out}: {error}"))?;
    let results_hash = Sha256::digest(csv.as_bytes());
    println!(
        "results_sha256={} final_state_sha256={}",
        hex(&results_hash),
        hex(&state.state_hash())
    );
    Ok(())
}

/// Preserve the original SIR CSV and fixed six-column sweep series byte-for-byte.
fn run_sir_results_csv(
    model: &sembla_ir::ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
) -> Result<(String, Vec<[usize; 6]>), String> {
    let sir_box = sir_box_name(model)?.to_owned();
    let mut csv = String::new();
    let mut series = Vec::with_capacity(ticks as usize);
    csv.push_str("# params=");
    csv.push_str(&canonical_params(params)?);
    csv.push('\n');
    csv.push_str(&format!("# dt={}\n", model.model().dt));
    csv.push_str("tick,S,I,R,fired_infect,fired_recover,deferred_total\n");
    for tick in 0..ticks {
        let report = executor::run_tick(model, state, params, seed, tick)
            .map_err(|error| format!("tick {tick}: {error}"))?;
        let counts = sir_counts(state, &sir_box)?;
        let fired_infect = report.fired.first().map_or(0, |entry| entry.1);
        let fired_recover = report.fired.get(1).map_or(0, |entry| entry.1);
        let deferred_total: usize = report
            .deferred_per_resource_table
            .iter()
            .map(|(_, count)| count)
            .sum();
        let row = [
            counts[0],
            counts[1],
            counts[2],
            fired_infect,
            fired_recover,
            deferred_total,
        ];
        series.push(row);
        csv.push_str(&format!(
            "{tick},{},{},{},{},{},{}\n",
            row[0], row[1], row[2], row[3], row[4], row[5]
        ));
    }
    Ok((csv, series))
}

#[derive(Debug)]
struct EnumCountDescriptor {
    box_name: String,
    table_name: String,
    attr_name: String,
    variants: Vec<String>,
}

#[derive(Debug)]
struct FiringDescriptor {
    box_name: String,
    transition_name: String,
    rule_id: u32,
}

fn csv_field(value: &str) -> String {
    if value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\n' | '\r'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn generic_enum_descriptors(model: &sembla_ir::ValidatedModel) -> Vec<EnumCountDescriptor> {
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

fn generic_firing_descriptors(model: &sembla_ir::ValidatedModel) -> Vec<FiringDescriptor> {
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

fn run_generic_results_csv(
    model: &sembla_ir::ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
) -> Result<(String, Vec<Vec<usize>>), String> {
    let enums = generic_enum_descriptors(model);
    let firings = generic_firing_descriptors(model);
    let mut csv = String::new();
    let mut series = Vec::with_capacity(ticks as usize);
    csv.push_str("# params=");
    csv.push_str(&canonical_params(params)?);
    csv.push('\n');
    csv.push_str(&format!("# dt={}\n", model.model().dt));

    let mut headers = vec!["tick".to_owned()];
    for descriptor in &enums {
        for variant in &descriptor.variants {
            headers.push(format!(
                "count:{}.{}.{}={variant}",
                descriptor.box_name, descriptor.table_name, descriptor.attr_name
            ));
        }
    }
    for descriptor in &firings {
        headers.push(format!(
            "fired:{}.{}",
            descriptor.box_name, descriptor.transition_name
        ));
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

    for tick in 0..ticks {
        let report = executor::run_tick(model, state, params, seed, tick)
            .map_err(|error| format!("tick {tick}: {error}"))?;
        let snapshot = state.snapshot();
        let mut row = vec![tick as usize];
        for descriptor in &enums {
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
            row.extend(counts);
        }
        if report.fired.len() != firings.len() {
            return Err(format!(
                "tick {tick}: internal firing report length mismatch: expected {}, found {}",
                firings.len(),
                report.fired.len()
            ));
        }
        for (descriptor, (reported_rule_id, fired)) in firings.iter().zip(&report.fired) {
            if *reported_rule_id != descriptor.rule_id {
                return Err(format!(
                    "tick {tick}: internal firing report rule mismatch: expected {}, found {}",
                    descriptor.rule_id, reported_rule_id
                ));
            }
            row.push(*fired);
        }
        row.push(
            report
                .deferred_per_resource_table
                .iter()
                .map(|(_, count)| count)
                .sum(),
        );
        csv.push_str(
            &row.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push('\n');
        series.push(row);
    }
    Ok((csv, series))
}

fn optional_sir_box_name(model: &sembla_ir::ValidatedModel) -> Result<Option<&str>, String> {
    let matches = model
        .model()
        .boxes
        .iter()
        .filter(|model_box| {
            model_box.tables.iter().any(|table| {
                table.name == "person"
                    && table.attrs.iter().any(|attr| {
                        attr.name == "health"
                            && matches!(&attr.ty, AttrType::Enum { variants } if variants == &["S", "I", "R"])
                    })
                    && table.attrs.iter().any(|attr| {
                        attr.name == "employer"
                            && matches!(&attr.ty, AttrType::Ref { table } if table == "employer")
                    })
            }) && model_box.tables.iter().any(|table| table.name == "employer")
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [model_box] => Ok(Some(&model_box.name)),
        [] => Ok(None),
        _ => Err("CSV output found more than one SIR person/employer box".to_owned()),
    }
}

fn sir_box_name(model: &sembla_ir::ValidatedModel) -> Result<&str, String> {
    optional_sir_box_name(model)?
        .ok_or_else(|| "CSV output requires exactly one SIR person/employer box".to_owned())
}

fn sir_counts(state: &StateStore, box_name: &str) -> Result<[usize; 3], String> {
    let snapshot = state.snapshot();
    let health = snapshot
        .enum_values(box_name, "person", "health")
        .map_err(|error| error.to_string())?;
    let mut counts = [0_usize; 3];
    for value in health {
        let slot = counts
            .get_mut(*value as usize)
            .ok_or_else(|| format!("invalid health enum index {value}"))?;
        *slot += 1;
    }
    Ok(counts)
}

fn initializers_from_population(
    model: &sembla_ir::ValidatedModel,
    population: &SyntheticPopulation,
) -> Result<Vec<TableInit>, String> {
    let sir_box = sir_box_name(model)?.to_owned();
    let is_sir_policy = sir_box == "population"
        && model.model().boxes.iter().any(|model_box| {
            model_box.name == "policy"
                && model_box.tables.iter().any(|table| {
                    table.name == "controller"
                        && table.size_hint == 1
                        && table.attrs.iter().any(|attr| attr.name == "mode")
                        && table.attrs.iter().any(|attr| attr.name == "modifier")
                })
        });
    let mut initial = if is_sir_policy {
        population.sir_policy_table_initializers()
    } else {
        population.sir_table_initializers_for_box(&sir_box)
    };
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            if (model_box.name == sir_box && (table.name == "person" || table.name == "employer"))
                || (is_sir_policy && model_box.name == "policy" && table.name == "controller")
            {
                continue;
            }
            let row_count = usize::try_from(table.size_hint).map_err(|_| {
                format!("{}.{} size_hint exceeds usize", model_box.name, table.name)
            })?;
            let columns = table
                .attrs
                .iter()
                .map(|attr| {
                    let data = match &attr.ty {
                        AttrType::Real => ColumnData::Real(vec![0.0; row_count]),
                        AttrType::Int => ColumnData::Int(vec![0; row_count]),
                        AttrType::Enum { .. } => ColumnData::Enum(vec![0; row_count]),
                        AttrType::Ref { .. } => ColumnData::Ref(vec![0; row_count]),
                    };
                    ColumnInit::new(&attr.name, data)
                })
                .collect();
            initial.push(TableInit::new(
                &model_box.name,
                &table.name,
                row_count,
                columns,
            ));
        }
    }
    Ok(initial)
}

#[derive(Clone, Debug)]
struct CompareTick {
    counts: [usize; 3],
    fired_infect: usize,
    fired_recover: usize,
    deferred_total: usize,
}

fn compare_result(options: CompareOptions) -> Result<(), String> {
    let path_a = &options.models[0];
    let path_b = options.models.get(1).unwrap_or(path_a);
    let model_a = read_validated(path_a)?;
    let model_b = read_validated(path_b)?;
    let params_a = resolve_params(&model_a, options.params_a.as_deref())?;
    let params_b = resolve_params(&model_b, options.params_b.as_deref())?;
    let population =
        SyntheticPopulation::read(&options.population).map_err(|error| error.to_string())?;
    let ticks_a = compare_arm(
        &model_a,
        &params_a,
        &population,
        options.seed,
        options.ticks,
    )?;
    let ticks_b = compare_arm(
        &model_b,
        &params_b,
        &population,
        options.seed,
        options.ticks,
    )?;

    let mut csv = String::new();
    csv.push_str("# arm_a_model=");
    csv.push_str(path_a);
    csv.push('\n');
    csv.push_str("# arm_b_model=");
    csv.push_str(path_b);
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
    csv.push_str("tick,S_a,I_a,R_a,S_b,I_b,R_b,dS,dI,dR,fired_infect_a,fired_recover_a,deferred_a,fired_infect_b,fired_recover_b,deferred_b\n");
    for (tick, (arm_a, arm_b)) in ticks_a.iter().zip(&ticks_b).enumerate() {
        let difference = |a: usize, b: usize| b as i128 - a as i128;
        csv.push_str(&format!(
            "{tick},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            arm_a.counts[0],
            arm_a.counts[1],
            arm_a.counts[2],
            arm_b.counts[0],
            arm_b.counts[1],
            arm_b.counts[2],
            difference(arm_a.counts[0], arm_b.counts[0]),
            difference(arm_a.counts[1], arm_b.counts[1]),
            difference(arm_a.counts[2], arm_b.counts[2]),
            arm_a.fired_infect,
            arm_a.fired_recover,
            arm_a.deferred_total,
            arm_b.fired_infect,
            arm_b.fired_recover,
            arm_b.deferred_total,
        ));
    }
    std::fs::write(&options.out, csv.as_bytes())
        .map_err(|error| format!("{}: {error}", options.out))?;
    println!("compare_sha256={}", hex(&Sha256::digest(csv.as_bytes())));
    Ok(())
}

fn compare_arm(
    model: &sembla_ir::ValidatedModel,
    params: &ParamEnv,
    population: &SyntheticPopulation,
    seed: u64,
    ticks: u32,
) -> Result<Vec<CompareTick>, String> {
    let sir_box = sir_box_name(model)?.to_owned();
    let initial = initializers_from_population(model, population)?;
    let mut state = StateStore::new(model, initial).map_err(|error| error.to_string())?;
    let mut rows = Vec::with_capacity(ticks as usize);
    for tick in 0..ticks {
        let report = executor::run_tick(model, &mut state, params, seed, tick)
            .map_err(|error| format!("tick {tick}: {error}"))?;
        rows.push(CompareTick {
            counts: sir_counts(&state, &sir_box)?,
            fired_infect: report.fired.first().map_or(0, |entry| entry.1),
            fired_recover: report.fired.get(1).map_or(0, |entry| entry.1),
            deferred_total: report
                .deferred_per_resource_table
                .iter()
                .map(|(_, count)| count)
                .sum(),
        });
    }
    Ok(rows)
}

fn canonical_params(params: &ParamEnv) -> Result<String, String> {
    let mut object = String::from("{");
    for (index, (name, value)) in params.values().enumerate() {
        if index != 0 {
            object.push(',');
        }
        object.push_str(&serde_json::to_string(name).map_err(|error| error.to_string())?);
        object.push(':');
        match value {
            ParamValue::Real { value } => {
                object.push_str(&serde_json::to_string(value).map_err(|error| error.to_string())?)
            }
            ParamValue::Int { value } => object.push_str(&value.to_string()),
        }
    }
    object.push('}');
    Ok(object)
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn initialize_population(model: &sembla_ir::ValidatedModel, population: usize) -> Vec<TableInit> {
    let mut initial = Vec::new();
    let composed = model.model().boxes.len() > 1 || !model.model().wires.is_empty();
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            let row_count = if composed && table.size_hint != 0 {
                usize::try_from(table.size_hint).expect("table size_hint exceeds usize")
            } else {
                population
            };
            let columns = table
                .attrs
                .iter()
                .map(|attr| {
                    let data = match attr.ty {
                        AttrType::Real => ColumnData::Real(vec![0.0; row_count]),
                        AttrType::Int => ColumnData::Int(vec![0; row_count]),
                        AttrType::Enum { .. } => ColumnData::Enum(vec![0; row_count]),
                        AttrType::Ref { .. } => ColumnData::Ref(vec![0; row_count]),
                    };
                    ColumnInit::new(&attr.name, data)
                })
                .collect();
            initial.push(TableInit::new(
                &model_box.name,
                &table.name,
                row_count,
                columns,
            ));
        }
    }
    initial
}

#[cfg(test)]
mod tests {
    use super::{
        csv_field, initialize_population, optional_sir_box_name, run, run_generic_results_csv,
        run_sir_results_csv, VERSION,
    };
    use sembla_runtime::{eval::ParamEnv, state::StateStore};

    fn load(source: &str) -> sembla_ir::ValidatedModel {
        sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
    }

    fn initialized(model: &sembla_ir::ValidatedModel, rows: usize) -> StateStore {
        StateStore::new(model, initialize_population(model, rows)).unwrap()
    }

    #[test]
    fn version_matches_library_versions() {
        assert_eq!(VERSION, sembla_ir::VERSION);
        assert_eq!(VERSION, sembla_runtime::VERSION);
    }

    #[test]
    fn invalid_usage_is_nonzero() {
        assert_eq!(run(&[]), 2);
    }

    #[test]
    fn legacy_sir_csv_bytes_are_frozen() {
        let model = load(include_str!("../../../examples/sir.json"));
        let params = ParamEnv::defaults(&model);
        let mut state = initialized(&model, 4);
        let (csv, series) = run_sir_results_csv(&model, &mut state, &params, 1, 2).unwrap();
        assert_eq!(
            csv,
            "# params={\"beta\":0.8,\"gamma\":0.1}\n\
# dt=0.25\n\
tick,S,I,R,fired_infect,fired_recover,deferred_total\n\
0,4,0,0,0,0,0\n\
1,4,0,0,0,0,0\n"
        );
        assert_eq!(series, vec![[4, 0, 0, 0, 0, 0]; 2]);
        assert_eq!(optional_sir_box_name(&model).unwrap(), Some("sir"));
    }

    #[test]
    fn multiple_sir_boxes_are_rejected_as_ambiguous() {
        let mut raw = sembla_ir::parse_json(include_str!("../../../examples/sir.json")).unwrap();
        let mut duplicate = raw.boxes[0].clone();
        duplicate.name = "sir_duplicate".to_owned();
        raw.boxes.push(duplicate);
        let model = sembla_ir::validate(raw).unwrap();
        assert_eq!(
            optional_sir_box_name(&model).unwrap_err(),
            "CSV output found more than one SIR person/employer box"
        );
    }

    #[test]
    fn generic_csv_is_ordered_deterministic_and_conservative() {
        let model = load(include_str!("../../../examples/reversible_ctmc.json"));
        let params = ParamEnv::defaults(&model);
        let mut first_state = initialized(&model, 1000);
        let mut second_state = initialized(&model, 1000);
        let first = run_generic_results_csv(&model, &mut first_state, &params, 55, 20).unwrap();
        let second = run_generic_results_csv(&model, &mut second_state, &params, 55, 20).unwrap();
        assert_eq!(first, second);
        assert_eq!(optional_sir_box_name(&model).unwrap(), None);
        assert_eq!(
            first.0.lines().nth(2).unwrap(),
            "tick,count:chain.particle.phase=A,count:chain.particle.phase=B,fired:chain.move_ab,fired:chain.move_ba,deferred_total"
        );
        assert_eq!(first.1.len(), 20);
        for row in &first.1 {
            assert_eq!(row[1] + row[2], 1000);
            assert_eq!(row.len(), 6);
        }
        assert_eq!(
            first.1[0][4], 0,
            "B to A must still have a zero-valued column"
        );
        assert!(first.1.last().unwrap()[2] > 0);
    }

    #[test]
    fn generated_csv_headers_are_escaped() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("has,comma"), "\"has,comma\"");
        assert_eq!(csv_field("has\"quote"), "\"has\"\"quote\"");
        assert_eq!(csv_field("has\nnewline"), "\"has\nnewline\"");
    }
}
