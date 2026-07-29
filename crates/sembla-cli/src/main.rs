use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sembla_cuda::{CudaBackend, HashMode};
use sembla_ir::{AttrType, FeatureSet, ParamType, ParamValue, GROUPED_OBSERVATIONS_FEATURE};
use sembla_runtime::eval::{ParamEnv, ParamOverride};
use sembla_runtime::executor::{self, ObservationValue, SummaryValue};
use sembla_runtime::population::SyntheticPopulation;
use sembla_runtime::prior::sample_parameters_for_draw;
use sembla_runtime::rng::derive_sweep_replica_seed;
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};
use sembla_runtime::state_artifact::{
    committed_table_inits, read as read_state_artifact, sniff_magic, state_artifact_hash,
    to_table_inits, write_new as write_new_state_artifact, StateKind, STATE_ARTIFACT_SCHEMA,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

mod manifest;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = "usage: sembla --version | sembla validate <model-or-plan.json> | sembla plan-hash <plan-envelope.json> | sembla state-hash <file.state> | sembla bundle-verify <bundle-dir> | sembla diff-ir <a.json> <b.json> | sembla synth-pop --persons N --employers E --initial-infected I --seed S --out pop.bin | sembla synth-state --model model-or-plan.json --slots N --areas K --present-fraction F --streams birth:B,overseas:O,internal:I --seed S --out state.artifact (benchmark/test tooling for the documented demographic column roles; emits state.artifact.model.json) | sembla run <model-or-plan.json> --seed N --ticks K --population N|pop.bin|file.state [--backend cpu|cuda] [--out results.csv] [--export-state final.state] [--dt D] [--params file.json] [--timing-json timing.json] [--enable grouped-observations] | sembla sweep <model-or-plan.json> --population N|pop.bin|file.state --seed S (--draws K | --theta-file file.json) --ticks T --out dir [--backend cpu|cuda] [--noise crn|independent] [--params file.json] [--export-pairs pairs.csv] [--timing-json timing.json] [--enable grouped-observations] | sembla compare <model-or-plan.json> <model-or-plan.json> --population pop.bin|file.state --seed N --ticks K --out compare.csv [--backend cpu|cuda] | sembla compare <model-or-plan.json> --population pop.bin|file.state --seed N --ticks K --params-a a.json --params-b b.json --out compare.csv [--backend cpu|cuda] [--enable grouped-observations] | sembla verify-run <manifest.json> <model-or-plan.json> --population N|pop.bin|file.state [--params file.json] [--draw K] | sembla diff-backends <model-or-plan.json> --population N|pop.bin|file.state --seed N --ticks K [--dt D] [--params file.json] [--enable grouped-observations] | sembla diff-backends --all-examples [--population N] [--seed N] [--ticks K] [--dt D] | sembla diff-backends --all-plan-fixtures [--population N] [--seed N] [--ticks K]";
const PLAN_NOT_RUNNABLE: &str = "plan envelopes are not yet runnable; see PRD 0004";
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
        [command, path] if command == "plan-hash" => plan_hash_file(path),
        [command, path] if command == "state-hash" => state_hash_file(path),
        [command, path] if command == "bundle-verify" => bundle_verify(path),
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
        [command, flags @ ..] if command == "synth-state" => {
            let options = match parse_synth_state_options(flags) {
                Ok(options) => options,
                Err(message) => {
                    eprintln!("{message}\n{USAGE}");
                    return 2;
                }
            };
            synth_state(options)
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
        [command, manifest_path, model_path, flags @ ..] if command == "verify-run" => {
            let options = match parse_verify_options(flags) {
                Ok(options) => options,
                Err(message) => {
                    eprintln!("{message}\n{USAGE}");
                    return 2;
                }
            };
            verify_run(manifest_path, model_path, options)
        }
        [command, arguments @ ..] if command == "compare" => compare_command(arguments),
        [command, arguments @ ..] if command == "diff-backends" => diff_backends_command(arguments),
        _ => {
            eprintln!("{USAGE}");
            2
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BackendSelection {
    #[default]
    Cpu,
    Cuda,
}

fn parse_backend(value: &str) -> Result<BackendSelection, String> {
    match value {
        "cpu" => Ok(BackendSelection::Cpu),
        "cuda" => Ok(BackendSelection::Cuda),
        _ => Err(format!(
            "invalid backend '{value}' (expected 'cpu' or 'cuda')"
        )),
    }
}

#[derive(Clone, Debug)]
struct RunOptions {
    seed: u64,
    ticks: u32,
    population: String,
    out: Option<String>,
    export_state: Option<String>,
    dt: Option<f64>,
    params: Option<String>,
    timing_json: Option<String>,
    backend: BackendSelection,
    enabled_features: FeatureSet,
}

fn parse_feature(value: &str) -> Result<String, String> {
    match value {
        GROUPED_OBSERVATIONS_FEATURE => Ok(value.to_owned()),
        _ => Err(format!(
            "unknown feature '{value}' (known features: {GROUPED_OBSERVATIONS_FEATURE})"
        )),
    }
}

fn parse_run_options(flags: &[String]) -> Result<RunOptions, String> {
    let mut seed = None;
    let mut ticks = None;
    let mut population = None;
    let mut out = None;
    let mut export_state = None;
    let mut dt = None;
    let mut params = None;
    let mut timing_json = None;
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
        backend: backend.unwrap_or_default(),
        enabled_features,
    })
}

#[derive(Clone, Debug)]
struct SweepOptions {
    seed: u64,
    draws: Option<u32>,
    theta_file: Option<String>,
    noise_mode: manifest::NoiseMode,
    ticks: u32,
    population: String,
    out: String,
    params: Option<String>,
    export_pairs: Option<String>,
    timing_json: Option<String>,
    backend: BackendSelection,
    enabled_features: FeatureSet,
}

fn parse_sweep_options(flags: &[String]) -> Result<SweepOptions, String> {
    let mut seed = None;
    let mut draws = None;
    let mut ticks = None;
    let mut population = None;
    let mut out = None;
    let mut params = None;
    let mut theta_file = None;
    let mut noise_mode = None;
    let mut export_pairs = None;
    let mut timing_json = None;
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
            "--draws" => set_once(&mut draws, parse_number(value, flag)?, flag)?,
            "--theta-file" => set_once(&mut theta_file, value.clone(), flag)?,
            "--noise" => {
                let value = match value.as_str() {
                    "crn" => manifest::NoiseMode::Crn,
                    "independent" => manifest::NoiseMode::Independent,
                    _ => {
                        return Err(format!(
                            "invalid value '{value}' for '--noise' (expected 'crn' or 'independent')"
                        ));
                    }
                };
                set_once(&mut noise_mode, value, flag)?;
            }
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
            "--export-pairs" => set_once(&mut export_pairs, value.clone(), flag)?,
            "--timing-json" => set_once(&mut timing_json, value.clone(), flag)?,
            "--backend" => set_once(&mut backend, parse_backend(value)?, flag)?,
            "--enable" => {
                enabled_features.insert(parse_feature(value)?);
            }
            _ => return Err(format!("unknown sweep flag '{flag}'")),
        }
        index += 2;
    }
    if draws.is_some() && theta_file.is_some() {
        return Err("'--theta-file' cannot be combined with '--draws'".to_owned());
    }
    if draws.is_none() && theta_file.is_none() {
        return Err("missing required flag '--draws' or '--theta-file'".to_owned());
    }
    if draws == Some(0) {
        return Err("'--draws' must be greater than zero".to_owned());
    }
    Ok(SweepOptions {
        seed: seed.ok_or_else(|| "missing required flag '--seed'".to_owned())?,
        draws,
        theta_file,
        noise_mode: noise_mode.unwrap_or(manifest::NoiseMode::Crn),
        ticks: ticks.ok_or_else(|| "missing required flag '--ticks'".to_owned())?,
        population: population.ok_or_else(|| "missing required flag '--population'".to_owned())?,
        out: out.ok_or_else(|| "missing required flag '--out'".to_owned())?,
        params,
        export_pairs,
        timing_json,
        backend: backend.unwrap_or_default(),
        enabled_features,
    })
}

#[derive(Clone, Debug)]
struct VerifyOptions {
    population: String,
    params: Option<String>,
    draw: Option<u32>,
}

fn parse_verify_options(flags: &[String]) -> Result<VerifyOptions, String> {
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

#[derive(Clone, Debug)]
struct CompareOptions {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SynthStreams {
    birth: u64,
    overseas: u64,
    internal: u64,
}

#[derive(Clone, Debug)]
struct SynthStateOptions {
    model: String,
    slots: usize,
    areas: usize,
    present_fraction: f64,
    streams: SynthStreams,
    seed: u64,
    out: String,
}

fn parse_synth_state_options(flags: &[String]) -> Result<SynthStateOptions, String> {
    let mut model = None;
    let mut slots = None;
    let mut areas = None;
    let mut present_fraction = None;
    let mut streams = None;
    let mut seed = None;
    let mut out = None;
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index].as_str();
        let value = flags
            .get(index + 1)
            .ok_or_else(|| format!("missing value for '{flag}'"))?;
        match flag {
            "--model" => set_once(&mut model, value.clone(), flag)?,
            "--slots" => set_once(&mut slots, parse_number(value, flag)?, flag)?,
            "--areas" => set_once(&mut areas, parse_number(value, flag)?, flag)?,
            "--present-fraction" => {
                let value: f64 = parse_number(value, flag)?;
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(
                        "'--present-fraction' must be finite and between 0 and 1".to_owned()
                    );
                }
                set_once(&mut present_fraction, value, flag)?;
            }
            "--streams" => set_once(&mut streams, parse_synth_streams(value)?, flag)?,
            "--seed" => set_once(&mut seed, parse_number(value, flag)?, flag)?,
            "--out" => set_once(&mut out, value.clone(), flag)?,
            _ => return Err(format!("unknown synth-state flag '{flag}'")),
        }
        index += 2;
    }
    let slots = slots.ok_or_else(|| "missing required flag '--slots'".to_owned())?;
    let areas = areas.ok_or_else(|| "missing required flag '--areas'".to_owned())?;
    if slots == 0 {
        return Err("'--slots' must be greater than zero".to_owned());
    }
    if areas == 0 {
        return Err("'--areas' must be greater than zero".to_owned());
    }
    if slots > u32::MAX as usize || areas > u32::MAX as usize {
        return Err(
            "'--slots' and '--areas' must fit the state artifact Ref encoding (u32)".to_owned(),
        );
    }
    Ok(SynthStateOptions {
        model: model.ok_or_else(|| "missing required flag '--model'".to_owned())?,
        slots,
        areas,
        present_fraction: present_fraction
            .ok_or_else(|| "missing required flag '--present-fraction'".to_owned())?,
        streams: streams.ok_or_else(|| "missing required flag '--streams'".to_owned())?,
        seed: seed.ok_or_else(|| "missing required flag '--seed'".to_owned())?,
        out: out.ok_or_else(|| "missing required flag '--out'".to_owned())?,
    })
}

fn parse_synth_streams(value: &str) -> Result<SynthStreams, String> {
    let mut parsed = std::collections::BTreeMap::new();
    for item in value.split(',') {
        let (name, weight) = item.split_once(':').ok_or_else(|| {
            format!("invalid '--streams' value '{value}' (expected birth:B,overseas:O,internal:I)")
        })?;
        let weight: u64 = weight.parse().map_err(|_| {
            format!("invalid stream weight '{weight}' in '--streams' value '{value}'")
        })?;
        if !matches!(name, "birth" | "overseas" | "internal") {
            return Err(format!(
                "unknown stream '{name}' in '--streams' value '{value}'"
            ));
        }
        if parsed.insert(name, weight).is_some() {
            return Err(format!(
                "duplicate stream '{name}' in '--streams' value '{value}'"
            ));
        }
    }
    let streams = SynthStreams {
        birth: *parsed
            .get("birth")
            .ok_or_else(|| "'--streams' is missing 'birth'".to_owned())?,
        overseas: *parsed
            .get("overseas")
            .ok_or_else(|| "'--streams' is missing 'overseas'".to_owned())?,
        internal: *parsed
            .get("internal")
            .ok_or_else(|| "'--streams' is missing 'internal'".to_owned())?,
    };
    if streams
        .birth
        .checked_add(streams.overseas)
        .and_then(|sum| sum.checked_add(streams.internal))
        .filter(|sum| *sum > 0)
        .is_none()
    {
        return Err("'--streams' weights must have a nonzero, non-overflowing sum".to_owned());
    }
    if parsed.len() != 3 {
        return Err(format!(
            "invalid '--streams' value '{value}' (expected exactly birth:B,overseas:O,internal:I)"
        ));
    }
    Ok(streams)
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

fn read_input(path: &str) -> Result<(String, sembla_ir::ParsedInput), String> {
    let source = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let input = sembla_ir::parse_input(&source).map_err(|error| format!("{path}: {error}"))?;
    Ok((source, input))
}

fn read_model(path: &str) -> Result<sembla_ir::Model, String> {
    match read_input(path)?.1 {
        sembla_ir::ParsedInput::LegacyModel(model) => Ok(model),
        sembla_ir::ParsedInput::Plan(_) => Err(format!("{path}: {PLAN_NOT_RUNNABLE}")),
    }
}

fn read_validated(path: &str) -> Result<sembla_ir::ValidatedModel, String> {
    sembla_ir::validate(read_model(path)?).map_err(|error| format!("{path}: {error}"))
}

struct RunInput {
    model: sembla_ir::ValidatedModel,
    plan: Option<sembla_ir::ExecutablePlanV1>,
}

fn read_run_input(path: &str, options: &RunOptions) -> Result<RunInput, String> {
    read_executable_input(path, options.dt, &options.enabled_features)
}

fn read_executable_input(
    path: &str,
    dt: Option<f64>,
    enabled_features: &FeatureSet,
) -> Result<RunInput, String> {
    let (source, input) = read_input(path)?;
    match input {
        sembla_ir::ParsedInput::LegacyModel(mut model) => {
            if let Some(dt) = dt {
                model.dt = dt;
            }
            let model = sembla_ir::validate_with_features(model, enabled_features)
                .map_err(|error| format!("{path}: {error}"))?;
            Ok(RunInput { model, plan: None })
        }
        sembla_ir::ParsedInput::Plan(plan) => {
            let validated =
                sembla_ir::validate_plan(&plan).map_err(|error| format!("{path}: {error}"))?;
            // Plan validation accepts an artifact that accurately describes its
            // features. Executing it separately requires the runtime flag.
            sembla_ir::validate_with_features(plan.model.clone(), enabled_features)
                .map_err(|error| format!("{path}: {error}"))?;
            require_canonical_plan(path, &source)?;
            if dt.is_some() {
                return Err(format!(
                    "{path}: plan envelopes do not support --dt overrides; edit and re-canonicalize the plan instead"
                ));
            }
            Ok(RunInput {
                model: validated.model_with_rule_words(),
                plan: Some(plan),
            })
        }
    }
}

fn require_canonical_plan(path: &str, source: &str) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(source)
        .map_err(|error| format!("{path}: invalid JSON after plan parsing: {error}"))?;
    let canonical = sembla_ir::to_canonical_string(&value)
        .map_err(|error| format!("{path}: canonical serialization failed: {error}"))?;
    if canonical == source {
        Ok(())
    } else {
        Err(format!("{path}: plan file is not canonical"))
    }
}

fn read_validated_plan(path: &str) -> Result<sembla_ir::ExecutablePlanV1, String> {
    let (source, input) = read_input(path)?;
    let sembla_ir::ParsedInput::Plan(plan) = input else {
        return Err(format!("{path}: expected an executable plan envelope"));
    };
    sembla_ir::validate_plan(&plan).map_err(|error| format!("{path}: {error}"))?;
    require_canonical_plan(path, &source)?;
    Ok(plan)
}

fn validate_file(path: &str) -> i32 {
    let result = (|| -> Result<(), String> {
        let (source, input) = read_input(path)?;
        match input {
            sembla_ir::ParsedInput::LegacyModel(model) => {
                sembla_ir::validate(model).map_err(|error| format!("{path}: {error}"))?;
            }
            sembla_ir::ParsedInput::Plan(plan) => {
                sembla_ir::validate_plan(&plan).map_err(|error| format!("{path}: {error}"))?;
                require_canonical_plan(path, &source)?;
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn plan_hash_file(path: &str) -> i32 {
    let result = (|| -> Result<(), String> {
        let plan = read_validated_plan(path)?;
        let semantic = sembla_ir::plan_semantic_hash(&plan)
            .map_err(|error| format!("{path}: semantic hash failed: {error}"))?;
        let envelope = sembla_ir::plan_envelope_hash(&plan)
            .map_err(|error| format!("{path}: envelope hash failed: {error}"))?;
        println!(
            "semantic {} {} {}",
            semantic.algorithm, semantic.domain, semantic.digest
        );
        println!(
            "envelope {} {} {}",
            envelope.algorithm, envelope.domain, envelope.digest
        );
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn state_hash_file(path: &str) -> i32 {
    match state_artifact_hash(path) {
        Ok(hash) => {
            println!("state {} {} {}", hash.algorithm, hash.domain, hash.digest);
            0
        }
        Err(error) => {
            eprintln!("{path}: {error}");
            1
        }
    }
}

fn bundle_verify(directory: &str) -> i32 {
    match bundle_verify_result(Path::new(directory)) {
        Ok(checks) => {
            for check in checks {
                println!("ok {check}");
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn require_bundle_hash(
    record_name: &str,
    recorded: &sembla_ir::HashRecordV1,
    actual: &sembla_ir::HashRecordV1,
) -> Result<(), String> {
    if recorded == actual {
        Ok(())
    } else {
        Err(format!(
            "{record_name} mismatch: recorded={} actual={}",
            recorded.digest, actual.digest
        ))
    }
}

fn bundle_verify_result(directory: &Path) -> Result<Vec<&'static str>, String> {
    let (bundle, _) = manifest::read_bundle_manifest(directory)?;
    let mut checks = vec!["bundle-manifest schema, encoding, domains, and canonicality"];

    let source_path = directory.join(&bundle.source.path);
    let source_bytes = std::fs::read(&source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let plan_path = directory.join(&bundle.plan.path);
    let plan_bytes =
        std::fs::read(&plan_path).map_err(|error| format!("{}: {error}", plan_path.display()))?;
    let report_path = directory.join(manifest::BUNDLE_REPORT_PATH);
    let report_bytes = std::fs::read(&report_path)
        .map_err(|error| format!("{}: {error}", report_path.display()))?;

    let source_hash = manifest::source_artifact_hash(&source_bytes);
    require_bundle_hash("source.hash", &bundle.source.hash, &source_hash)?;
    checks.push("composition-source.json source.hash");

    let envelope_hash = manifest::plan_envelope_artifact_hash(&plan_bytes);
    require_bundle_hash(
        "plan.envelope_hash",
        &bundle.plan.envelope_hash,
        &envelope_hash,
    )?;
    checks.push("executable-plan.json plan.envelope_hash");

    let plan_source = std::str::from_utf8(&plan_bytes)
        .map_err(|error| format!("{}: invalid UTF-8: {error}", plan_path.display()))?;
    let parsed = sembla_ir::parse_input(plan_source)
        .map_err(|error| format!("{}: {error}", plan_path.display()))?;
    let sembla_ir::ParsedInput::Plan(plan) = parsed else {
        return Err(format!(
            "{}: expected an executable plan envelope",
            plan_path.display()
        ));
    };
    let semantic_hash = sembla_ir::plan_semantic_hash(&plan)
        .map_err(|error| format!("plan.semantic_hash recomputation failed: {error}"))?;
    require_bundle_hash(
        "plan.semantic_hash",
        &bundle.plan.semantic_hash,
        &semantic_hash,
    )?;
    checks.push("executable-plan.json plan.semantic_hash");

    let integrity = manifest::bundle_integrity_hash(
        &bundle,
        &[
            (manifest::BUNDLE_SOURCE_PATH, source_bytes.as_slice()),
            (manifest::BUNDLE_PLAN_PATH, plan_bytes.as_slice()),
            (manifest::BUNDLE_REPORT_PATH, report_bytes.as_slice()),
        ],
    )?;
    require_bundle_hash(
        "bundle_integrity",
        bundle
            .bundle_integrity
            .as_ref()
            .expect("bundle-manifest validation requires bundle_integrity"),
        &integrity,
    )?;
    checks.push(
        "composition-source.json, executable-plan.json, and link-report.json bundle_integrity",
    );

    sembla_ir::validate_plan(&plan).map_err(|error| format!("{}: {error}", plan_path.display()))?;
    require_canonical_plan(&plan_path.display().to_string(), plan_source)?;
    checks.push("executable-plan.json validation and canonicality");

    let provenance = plan.linked_provenance.as_ref().ok_or_else(|| {
        "manifest/plan agreement: linked plan has no linked_provenance".to_owned()
    })?;
    let origin = match plan.origin {
        sembla_ir::PlanOrigin::Linked => "linked",
        sembla_ir::PlanOrigin::DirectStable => "direct_stable",
    };
    let agreement = [
        ("plan.origin", bundle.plan.origin.as_str(), origin),
        (
            "plan.schema",
            bundle.plan.schema.as_str(),
            plan.schema_version.as_str(),
        ),
        (
            "plan.identity_scheme",
            bundle.plan.identity_scheme.as_str(),
            plan.identity_scheme.as_str(),
        ),
        (
            "source.schema",
            bundle.source.schema.as_str(),
            provenance.linker.source_schema.as_str(),
        ),
        (
            "linker.semantics",
            bundle.linker.semantics.as_str(),
            provenance.linker.semantics.as_str(),
        ),
        (
            "source_map_schema",
            bundle.source_map_schema.as_str(),
            provenance.linker.source_map_schema.as_str(),
        ),
        (
            "canonical_encoding",
            bundle.canonical_encoding.as_str(),
            provenance.linker.canonical_encoding.as_str(),
        ),
    ];
    for (field, manifest_value, plan_value) in agreement {
        if manifest_value != plan_value {
            return Err(format!(
                "manifest/plan agreement failed for {field}: manifest='{manifest_value}' plan='{plan_value}'"
            ));
        }
    }
    if bundle.plan.enabled_features != plan.identity.enabled_features {
        return Err(format!(
            "manifest/plan agreement failed for plan.enabled_features: manifest={:?} plan={:?}",
            bundle.plan.enabled_features, plan.identity.enabled_features
        ));
    }
    if bundle.source.hash != provenance.source_hash {
        return Err(format!(
            "manifest/plan agreement failed for linked_provenance.source_hash: manifest={} plan={}",
            bundle.source.hash.digest, provenance.source_hash.digest
        ));
    }
    checks
        .push("manifest/plan origin, schemas, identity, features, and source provenance agreement");
    Ok(checks)
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

fn synth_state(options: SynthStateOptions) -> i32 {
    match synth_state_result(&options) {
        Ok(companion) => {
            println!(
                "slots={} areas={} state={} companion_model={}",
                options.slots,
                options.areas,
                options.out,
                companion.display()
            );
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn synth_state_result(options: &SynthStateOptions) -> Result<PathBuf, String> {
    let (_, input) = read_input(&options.model)?;
    let mut model = match input {
        sembla_ir::ParsedInput::LegacyModel(model) => model,
        sembla_ir::ParsedInput::Plan(plan) => {
            sembla_ir::validate_plan(&plan)
                .map_err(|error| format!("{}: {error}", options.model))?;
            plan.model
        }
    };
    require_demographic_synth_schema(&model)?;
    for table in &mut model.boxes[0].tables {
        match table.name.as_str() {
            "area" => table.size_hint = options.areas as u64,
            "person_slot" | "slot_resource" => table.size_hint = options.slots as u64,
            _ => unreachable!("the demographic synthesis schema was checked"),
        }
    }
    let mut features = FeatureSet::new();
    if model
        .boxes
        .iter()
        .any(|model_box| !model_box.grouped_views.is_empty())
    {
        features.insert(GROUPED_OBSERVATIONS_FEATURE.to_owned());
    }
    let validated = sembla_ir::validate_with_features(model.clone(), &features)
        .map_err(|error| format!("{}: resized companion model: {error}", options.model))?;
    let tables = synthetic_demographic_tables(&validated, options)?;
    let companion = synth_state_companion_path(&options.out);
    for path in [Path::new(&options.out), companion.as_path()] {
        if path
            .try_exists()
            .map_err(|error| format!("{}: {error}", path.display()))?
        {
            return Err(format!(
                "refusing to overwrite synthesis output '{}'",
                path.display()
            ));
        }
    }
    let companion_json = sembla_ir::to_canonical_json(&model)
        .map_err(|error| format!("companion model serialization failed: {error}"))?;
    let mut companion_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&companion)
        .map_err(|error| format!("{}: {error}", companion.display()))?;
    if let Err(error) = std::io::Write::write_all(&mut companion_file, companion_json.as_bytes()) {
        drop(companion_file);
        let _ = std::fs::remove_file(&companion);
        return Err(format!("{}: {error}", companion.display()));
    }
    drop(companion_file);
    if let Err(error) = write_new_state_artifact(&options.out, &validated, &tables) {
        let _ = std::fs::remove_file(&companion);
        return Err(error.to_string());
    }
    Ok(companion)
}

fn synth_state_companion_path(out: &str) -> PathBuf {
    let mut path = std::ffi::OsString::from(out);
    path.push(".model.json");
    PathBuf::from(path)
}

fn require_demographic_synth_schema(model: &sembla_ir::Model) -> Result<(), String> {
    if model.boxes.len() != 1 || model.boxes[0].name != "demographic" {
        return Err(
            "synth-state benchmark tooling requires exactly one box named 'demographic'".to_owned(),
        );
    }
    let model_box = &model.boxes[0];
    if model_box.tables.len() != 3 {
        return Err(
            "synth-state benchmark tooling requires exactly area, person_slot, and slot_resource tables"
                .to_owned(),
        );
    }
    let area = synth_table(model_box, "area")?;
    let person = synth_table(model_box, "person_slot")?;
    let resource = synth_table(model_box, "slot_resource")?;
    require_synth_attrs(area, &[("area_key", AttrType::Int)])?;
    require_synth_attrs(resource, &[])?;
    require_synth_attrs(
        person,
        &[
            (
                "occupancy",
                AttrType::Enum {
                    variants: vec!["vacant".to_owned(), "present".to_owned()],
                },
            ),
            (
                "event",
                AttrType::Enum {
                    variants: [
                        "none_",
                        "birth",
                        "death",
                        "overseas_arrival",
                        "overseas_departure",
                        "internal_arrival",
                        "internal_departure",
                    ]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                },
            ),
            (
                "sex",
                AttrType::Enum {
                    variants: vec!["male".to_owned(), "female".to_owned()],
                },
            ),
            ("age_months", AttrType::Int),
            ("event_age_months", AttrType::Int),
            ("generation", AttrType::Int),
            (
                "entry_stream",
                AttrType::Enum {
                    variants: vec![
                        "birth_slot".to_owned(),
                        "overseas_slot".to_owned(),
                        "internal_slot".to_owned(),
                    ],
                },
            ),
            ("entry_age_months", AttrType::Int),
            (
                "area",
                AttrType::Ref {
                    table: "area".to_owned(),
                },
            ),
            (
                "slot_resource",
                AttrType::Ref {
                    table: "slot_resource".to_owned(),
                },
            ),
        ],
    )
}

fn synth_table<'a>(
    model_box: &'a sembla_ir::Box,
    name: &str,
) -> Result<&'a sembla_ir::Table, String> {
    model_box
        .tables
        .iter()
        .find(|table| table.name == name)
        .ok_or_else(|| format!("synth-state benchmark tooling requires table '{name}'"))
}

fn require_synth_attrs(
    table: &sembla_ir::Table,
    expected: &[(&str, AttrType)],
) -> Result<(), String> {
    if table.attrs.len() != expected.len() {
        return Err(format!(
            "synth-state table '{}' has {} attributes; expected the documented {} demographic column roles",
            table.name,
            table.attrs.len(),
            expected.len()
        ));
    }
    for (name, ty) in expected {
        let attr = table
            .attrs
            .iter()
            .find(|attr| attr.name == *name)
            .ok_or_else(|| {
                format!(
                    "synth-state table '{}' is missing documented demographic column role '{}'",
                    table.name, name
                )
            })?;
        if &attr.ty != ty {
            return Err(format!(
                "synth-state demographic column role '{}.{}' has type {:?}; expected {:?}",
                table.name, name, attr.ty, ty
            ));
        }
    }
    Ok(())
}

fn synthetic_demographic_tables(
    model: &sembla_ir::ValidatedModel,
    options: &SynthStateOptions,
) -> Result<Vec<TableInit>, String> {
    let present_count = ((options.slots as f64) * options.present_fraction).floor() as usize;
    let vacant_count = options.slots - present_count;
    let weight_sum = options.streams.birth as u128
        + options.streams.overseas as u128
        + options.streams.internal as u128;
    let birth_count =
        ((vacant_count as u128 * options.streams.birth as u128) / weight_sum) as usize;
    let overseas_count =
        ((vacant_count as u128 * options.streams.overseas as u128) / weight_sum) as usize;
    let model_box = &model.model().boxes[0];
    let mut tables = Vec::with_capacity(model_box.tables.len());
    for table in &model_box.tables {
        let row_count = usize::try_from(table.size_hint)
            .map_err(|_| format!("table '{}' row count is not representable", table.name))?;
        let mut columns = Vec::with_capacity(table.attrs.len());
        for attr in &table.attrs {
            let data = match (table.name.as_str(), attr.name.as_str()) {
                ("area", "area_key") => {
                    ColumnData::Int((0..row_count).map(|row| row as i64).collect())
                }
                ("person_slot", name) => synthetic_person_column(
                    name,
                    options,
                    present_count,
                    birth_count,
                    overseas_count,
                )?,
                _ => {
                    return Err(format!(
                        "synth-state has no documented mapping for '{}.{}'",
                        table.name, attr.name
                    ))
                }
            };
            columns.push(ColumnInit::new(&attr.name, data));
        }
        tables.push(TableInit::new(
            &model_box.name,
            &table.name,
            row_count,
            columns,
        ));
    }
    Ok(tables)
}

fn synthetic_person_column(
    name: &str,
    options: &SynthStateOptions,
    present_count: usize,
    birth_count: usize,
    overseas_count: usize,
) -> Result<ColumnData, String> {
    let rows = 0..options.slots;
    let data = match name {
        "occupancy" => ColumnData::Enum(
            rows.map(|row| u16::from(synth_slot_role(row, options, present_count).0))
                .collect(),
        ),
        "event" => ColumnData::Enum(vec![0; options.slots]),
        "sex" => ColumnData::Enum(
            rows.map(|row| (synth_word(options.seed, row, 1) & 1) as u16)
                .collect(),
        ),
        "age_months" => ColumnData::Int(
            rows.map(|row| {
                if synth_slot_role(row, options, present_count).0 {
                    (synth_word(options.seed, row, 2) % (90 * 12)) as i64
                } else {
                    0
                }
            })
            .collect(),
        ),
        "event_age_months" => ColumnData::Int(vec![-1; options.slots]),
        "generation" => ColumnData::Int(
            rows.map(|row| i64::from(synth_slot_role(row, options, present_count).0))
                .collect(),
        ),
        "entry_stream" => ColumnData::Enum(
            rows.map(|row| {
                let (_, vacant_rank) = synth_slot_role(row, options, present_count);
                if vacant_rank < birth_count {
                    0
                } else if vacant_rank < birth_count + overseas_count {
                    1
                } else {
                    2
                }
            })
            .collect(),
        ),
        "entry_age_months" => ColumnData::Int(
            rows.map(|row| {
                let (present, vacant_rank) = synth_slot_role(row, options, present_count);
                if present || vacant_rank < birth_count {
                    0
                } else if vacant_rank < birth_count + overseas_count {
                    (18 * 12 + synth_word(options.seed, row, 3) % (52 * 12)) as i64
                } else {
                    (synth_word(options.seed, row, 4) % (80 * 12)) as i64
                }
            })
            .collect(),
        ),
        "area" => ColumnData::Ref(
            rows.map(|row| (synth_word(options.seed, row, 5) % options.areas as u64) as u32)
                .collect(),
        ),
        "slot_resource" => ColumnData::Ref(rows.map(|row| row as u32).collect()),
        _ => {
            return Err(format!(
                "unknown synthesized demographic column role '{name}'"
            ))
        }
    };
    Ok(data)
}

fn synth_slot_role(row: usize, options: &SynthStateOptions, present_count: usize) -> (bool, usize) {
    let shift = options.seed % options.slots as u64;
    let coordinate = ((row as u64 + shift) % options.slots as u64) as usize;
    if coordinate < present_count {
        (true, 0)
    } else {
        (false, coordinate - present_count)
    }
}

fn synth_word(seed: u64, row: usize, lane: u64) -> u64 {
    let mut value = seed
        ^ (row as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ lane.wrapping_mul(0xd1b5_4a32_d192_ed03);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
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
    let (population_source, population_sha256) =
        manifest::population_identity(&options.population)?;
    let initialized = initialized_tables(&model, &options.population)?;
    let initial_state = initialized.state_hash.map(state_artifact_tuple);
    let initial = initialized.tables;
    let params = resolve_params(&model, options.params.as_deref())?;
    if options.out.is_none()
        && options.export_state.is_none()
        && options.timing_json.is_none()
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
        std::fs::write(out, execution.output.csv.as_bytes())
            .map_err(|error| format!("{out}: {error}"))?;
        let summaries = summaries_path(out);
        std::fs::write(&summaries, execution.output.summaries_csv.as_bytes())
            .map_err(|error| format!("{}: {error}", summaries.display()))?;
        for grouped in &execution.output.grouped {
            let path = grouped_output_path(Path::new(out), &grouped.view);
            std::fs::write(&path, grouped.csv.as_bytes())
                .map_err(|error| format!("{}: {error}", path.display()))?;
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
    Ok(())
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

#[derive(Debug)]
struct ThetaFile {
    assignments: Vec<Vec<ParamOverride>>,
    sha256: String,
}

fn read_theta_file(model: &sembla_ir::ValidatedModel, path: &str) -> Result<ThetaFile, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("{path}: {error}"))?;
    let entries = value
        .as_array()
        .ok_or_else(|| format!("{path}: theta file must be a JSON array"))?;
    if entries.is_empty() {
        return Err(format!(
            "{path}: theta file must contain at least one theta assignment"
        ));
    }
    u32::try_from(entries.len())
        .map_err(|_| format!("{path}: theta file contains more than u32::MAX assignments"))?;

    let mut assignments = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let object = entry
            .as_object()
            .ok_or_else(|| format!("{path}: theta assignment {index} must be a JSON object"))?;
        for declaration in model
            .model()
            .params
            .iter()
            .filter(|declaration| declaration.prior.is_some())
        {
            if !object.contains_key(&declaration.name) {
                return Err(format!(
                    "{path}: theta assignment {index} is missing prior-bearing parameter '{}'",
                    declaration.name
                ));
            }
        }

        let mut overrides = Vec::with_capacity(object.len());
        for (name, value) in object {
            let declaration = model
                .model()
                .params
                .iter()
                .find(|parameter| parameter.name == *name)
                .ok_or_else(|| {
                    format!("{path}: theta assignment {index} has unknown parameter '{name}'")
                })?;
            let value = param_value_from_json(
                declaration,
                value,
                &format!("{path}: theta assignment {index}"),
            )?;
            overrides.push(ParamOverride::new(name, value));
        }
        ParamEnv::resolve(model, &overrides)
            .map_err(|error| format!("{path}: theta assignment {index}: {error}"))?;
        assignments.push(overrides);
    }

    Ok(ThetaFile {
        assignments,
        sha256: hex(&Sha256::digest(&bytes)),
    })
}

fn params_from_theta_assignment(
    model: &sembla_ir::ValidatedModel,
    path: &str,
    draw: u32,
    assignment: &[ParamOverride],
    pinned: &[ParamOverride],
) -> Result<ParamEnv, String> {
    for supplied in assignment {
        if pinned.iter().any(|pin| pin.name == supplied.name) {
            return Err(format!(
                "{path}: theta assignment {draw} parameter '{}' is also supplied by --params",
                supplied.name
            ));
        }
    }
    let mut overrides = Vec::with_capacity(pinned.len() + assignment.len());
    overrides.extend_from_slice(pinned);
    overrides.extend_from_slice(assignment);
    ParamEnv::resolve(model, &overrides)
        .map_err(|error| format!("{path}: theta assignment {draw}: {error}"))
}

const SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV: &str = "SEMBLA_SWEEP_SPIKE_DRAW_WORKERS";
const SWEEP_CONCURRENCY_SPIKE_DELAY_DRAW_ZERO_ENV: &str = "SEMBLA_SWEEP_SPIKE_DELAY_DRAW_ZERO_MS";
const SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV: &str = "SEMBLA_SWEEP_SPIKE_CUDA_LOCKSTEP_STREAMS";

#[derive(Clone)]
struct SweepPreparedDraw {
    k: u32,
    params: ParamEnv,
    execution_seed: u64,
}

struct SweepConcurrentCompletedDraw {
    index: usize,
    lane: usize,
    start_offset: Duration,
    finish_offset: Duration,
    elapsed: Duration,
    execution: Result<SweepDrawOutput, String>,
}

struct SweepConcurrentExecution {
    setup_elapsed: Duration,
    execution_window_elapsed: Duration,
    identity: manifest::BackendIdentity,
    draws: Vec<SweepConcurrentCompletedDraw>,
}

fn sweep_concurrency_spike_workers(
    draw_count: u32,
    backend: BackendSelection,
) -> Result<usize, String> {
    let Some(raw) = std::env::var_os(SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV) else {
        return Ok(1);
    };
    let raw = raw.to_string_lossy();
    let workers = raw.parse::<usize>().map_err(|_| {
        format!("{SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV} must be a positive integer, found '{raw}'")
    })?;
    if workers == 0 {
        return Err(format!(
            "{SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV} must be greater than zero"
        ));
    }
    let draw_count = usize::try_from(draw_count).expect("draw count is u32");
    if workers > draw_count {
        return Err(format!(
            "{SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV}={workers} exceeds draw count {draw_count}"
        ));
    }
    if workers > 1
        && backend == BackendSelection::Cpu
        && std::env::var_os("SEMBLA_EVAL_THREADS").is_none()
    {
        return Err(format!(
            "{SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV}>1 with --backend cpu requires an explicit SEMBLA_EVAL_THREADS budget per draw"
        ));
    }
    Ok(workers)
}

fn sweep_concurrency_spike_cuda_lockstep(
    draw_count: u32,
    workers: usize,
    backend: BackendSelection,
) -> Result<bool, String> {
    let Some(raw) = std::env::var_os(SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV) else {
        return Ok(false);
    };
    let raw = raw.to_string_lossy();
    if raw != "1" {
        return Err(format!(
            "{SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV} must be 1 when set, found '{raw}'"
        ));
    }
    if backend != BackendSelection::Cuda {
        return Err(format!(
            "{SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV}=1 requires --backend cuda"
        ));
    }
    if workers <= 1 {
        return Err(format!(
            "{SWEEP_CONCURRENCY_SPIKE_CUDA_LOCKSTEP_ENV}=1 requires {SWEEP_CONCURRENCY_SPIKE_WORKERS_ENV}>1"
        ));
    }
    let draw_count = usize::try_from(draw_count).expect("draw count is u32");
    if draw_count % workers != 0 {
        return Err(format!(
            "lockstep CUDA spike requires draw count {draw_count} to be divisible by worker count {workers}"
        ));
    }
    Ok(true)
}

fn sweep_concurrency_spike_draw_zero_delay() -> Result<Duration, String> {
    let Some(raw) = std::env::var_os(SWEEP_CONCURRENCY_SPIKE_DELAY_DRAW_ZERO_ENV) else {
        return Ok(Duration::ZERO);
    };
    let raw = raw.to_string_lossy();
    let milliseconds = raw.parse::<u64>().map_err(|_| {
        format!(
            "{SWEEP_CONCURRENCY_SPIKE_DELAY_DRAW_ZERO_ENV} must be an unsigned integer, found '{raw}'"
        )
    })?;
    Ok(Duration::from_millis(milliseconds))
}

fn run_concurrent_sweep_spike(
    model: &sembla_ir::ValidatedModel,
    initial_tables: &[TableInit],
    construction_params: &ParamEnv,
    options: &SweepOptions,
    workers: usize,
    prepared: &[SweepPreparedDraw],
    cuda_lockstep: bool,
) -> Result<SweepConcurrentExecution, String> {
    let setup_started = Instant::now();
    let draw_zero_delay = sweep_concurrency_spike_draw_zero_delay()?;
    let next_draw = std::sync::atomic::AtomicUsize::new(0);
    let lane_construction_failed = std::sync::atomic::AtomicBool::new(false);
    let ready = std::sync::Barrier::new(workers + 1);
    let start = std::sync::Barrier::new(workers + 1);
    let lockstep_tick = std::sync::Barrier::new(workers);
    let execution_started = std::sync::OnceLock::<Instant>::new();
    let (lanes, setup_elapsed, execution_window_elapsed) = std::thread::scope(
        |scope| -> Result<(Vec<_>, Duration, Duration), String> {
            let handles = (0..workers)
                .map(|lane| {
                    let next_draw = &next_draw;
                    let lane_construction_failed = &lane_construction_failed;
                    let ready = &ready;
                    let start = &start;
                    let lockstep_tick = &lockstep_tick;
                    let execution_started = &execution_started;
                    scope.spawn(move || -> Result<_, String> {
                        // CUDA contexts are thread-current. Constructing a backend on
                        // the coordinator and moving it here produces
                        // CUDA_ERROR_INVALID_CONTEXT on the first driver operation.
                        // Each isolated lane therefore owns and uses its backend on
                        // one worker thread for its complete lifetime.
                        let backend = std::panic::catch_unwind(
                            std::panic::AssertUnwindSafe(|| -> Result<_, String> {
                                let backend = SweepBackend::new_concurrency_lane(
                                    model,
                                    initial_tables.to_vec(),
                                    construction_params,
                                    options.seed,
                                    options.backend,
                                    cuda_lockstep,
                                )?;
                                let identity = backend.identity();
                                Ok((identity, backend))
                            }),
                        )
                        .unwrap_or_else(|_| {
                            Err("sweep concurrency spike worker panicked during backend construction"
                                .to_owned())
                        });
                        if backend.is_err() {
                            lane_construction_failed
                                .store(true, std::sync::atomic::Ordering::Release);
                        }
                        // Both barriers must be reached even if construction failed,
                        // otherwise one failed lane would deadlock every healthy lane.
                        ready.wait();
                        start.wait();
                        if lane_construction_failed.load(std::sync::atomic::Ordering::Acquire) {
                            return match backend {
                                Err(error) => Err(error),
                                Ok(_) => {
                                    Err("sweep concurrency spike peer backend construction failed"
                                        .to_owned())
                                }
                            };
                        }
                        let (identity, mut backend) = backend?;
                        let execution_started = *execution_started
                            .get()
                            .expect("coordinator sets execution start before release");
                        let mut completed = Vec::new();
                        if cuda_lockstep {
                            for index in (lane..prepared.len()).step_by(workers) {
                                let draw = &prepared[index];
                                let start_offset = execution_started.elapsed();
                                let started = Instant::now();
                                if draw.k == 0 && !draw_zero_delay.is_zero() {
                                    std::thread::sleep(draw_zero_delay);
                                }
                                let execution = std::panic::catch_unwind(
                                    std::panic::AssertUnwindSafe(|| {
                                        backend.run_draw_lockstep(
                                            model,
                                            &draw.params,
                                            draw.execution_seed,
                                            options.ticks,
                                            &options.enabled_features,
                                            lockstep_tick,
                                        )
                                    }),
                                )
                                .unwrap_or_else(|_| {
                                    Err(format!(
                                        "draw {}: lockstep worker panicked after entering the barrier protocol",
                                        draw.k
                                    ))
                                });
                                let elapsed = started.elapsed();
                                completed.push(SweepConcurrentCompletedDraw {
                                    index,
                                    lane,
                                    start_offset,
                                    finish_offset: execution_started.elapsed(),
                                    elapsed,
                                    execution,
                                });
                            }
                        } else {
                            loop {
                                let index =
                                    next_draw.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let Some(draw) = prepared.get(index) else {
                                    break;
                                };
                                let start_offset = execution_started.elapsed();
                                let started = Instant::now();
                                if draw.k == 0 && !draw_zero_delay.is_zero() {
                                    std::thread::sleep(draw_zero_delay);
                                }
                                let execution = backend.run_draw(
                                    model,
                                    &draw.params,
                                    draw.execution_seed,
                                    options.ticks,
                                    &options.enabled_features,
                                );
                                let elapsed = started.elapsed();
                                completed.push(SweepConcurrentCompletedDraw {
                                    index,
                                    lane,
                                    start_offset,
                                    finish_offset: execution_started.elapsed(),
                                    elapsed,
                                    execution,
                                });
                            }
                        }
                        Ok((identity, completed))
                    })
                })
                .collect::<Vec<_>>();
            ready.wait();
            let setup_elapsed = setup_started.elapsed();
            let execution_start = Instant::now();
            execution_started
                .set(execution_start)
                .expect("execution start is set exactly once");
            start.wait();

            let mut lanes = Vec::with_capacity(workers);
            let mut errors = Vec::new();
            for (lane, handle) in handles.into_iter().enumerate() {
                match handle.join() {
                    Ok(Ok(result)) => lanes.push(result),
                    Ok(Err(error)) => errors.push((lane, error)),
                    Err(_) => errors.push((
                        lane,
                        "sweep concurrency spike worker panicked before returning its draws"
                            .to_owned(),
                    )),
                }
            }
            if !errors.is_empty() {
                let selected = errors
                    .iter()
                    .find(|(_, error)| !error.contains("peer backend construction failed"))
                    .unwrap_or(&errors[0]);
                return Err(format!("concurrency lane {}: {}", selected.0, selected.1));
            }
            Ok((lanes, setup_elapsed, execution_start.elapsed()))
        },
    )?;
    let identity = lanes
        .first()
        .expect("concurrency spike has at least one backend")
        .0
        .clone();
    if lanes
        .iter()
        .skip(1)
        .any(|(lane_identity, _)| lane_identity != &identity)
    {
        return Err("sweep concurrency spike backend identity changed across lanes".to_owned());
    }
    let mut draws = lanes
        .into_iter()
        .flat_map(|(_, completed)| completed)
        .collect::<Vec<_>>();
    draws.sort_by_key(|draw| draw.index);
    if draws.len() != prepared.len()
        || draws
            .iter()
            .enumerate()
            .any(|(index, draw)| draw.index != index)
    {
        return Err("sweep concurrency spike did not return every draw exactly once".to_owned());
    }
    Ok(SweepConcurrentExecution {
        setup_elapsed,
        execution_window_elapsed,
        identity,
        draws,
    })
}

#[allow(clippy::too_many_arguments)]
fn publish_sweep_draw(
    draw: u32,
    params: &ParamEnv,
    execution_seed: u64,
    execution: SweepDrawOutput,
    reported_columns: &mut Option<Vec<String>>,
    pairs_csv: &mut Option<String>,
    parameter_columns: &[String],
    summary_columns: &[String],
    run_manifest: &mut manifest::RunManifest,
    out: &Path,
    export_pairs: bool,
) -> Result<Vec<Vec<ReportedValue>>, String> {
    let output = execution.output;
    if let Some(columns) = reported_columns.as_ref() {
        if columns != &output.series.columns {
            return Err(format!(
                "draw {draw}: reported column schema changed across draws"
            ));
        }
    } else {
        *reported_columns = Some(output.series.columns.clone());
    }
    if let Some(csv) = pairs_csv {
        append_pairs_row(
            csv,
            draw,
            params,
            parameter_columns,
            &output.summaries,
            summary_columns,
        )?;
    }
    let hashes = execution_hashes_with_state_hash(&output, execution.final_state_hash);
    let grouped_outputs = grouped_output_records(&output.grouped);
    run_manifest.executions.push(manifest::ManifestExecution {
        k: draw,
        seed: Some(execution_seed),
        scenario: None,
        model: None,
        ir_hash: None,
        dt: None,
        resolved_theta: manifest::resolved_theta(params),
        results_sha256: hashes.results_sha256,
        final_state_sha256: hashes.final_state_sha256,
        observation_sha256: Some(hashes.observation_sha256),
        grouped_outputs,
    });
    let draw_path = out.join(format!("draw_{draw}.csv"));
    std::fs::write(&draw_path, output.csv.as_bytes())
        .map_err(|error| format!("{}: {error}", draw_path.display()))?;
    for grouped in &output.grouped {
        let path = grouped_output_path(&draw_path, &grouped.view);
        std::fs::write(&path, grouped.csv.as_bytes())
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    if export_pairs {
        let draw_summaries = PathBuf::from(format!("{}.summaries.csv", draw_path.display()));
        std::fs::write(&draw_summaries, output.summaries_csv.as_bytes())
            .map_err(|error| format!("{}: {error}", draw_summaries.display()))?;
    }
    Ok(output.series.rows)
}

fn sweep_file_result(path: &str, options: SweepOptions) -> Result<(), String> {
    let sweep_started = Instant::now();
    let RunInput { model, plan } = read_executable_input(path, None, &options.enabled_features)?;
    if options.export_pairs.is_some() && model.model().summaries.is_empty() {
        return Err(format!(
            "model '{}' declares no summaries; --export-pairs requires declared summaries (DESIGN.md §4.6)",
            model.model().name
        ));
    }
    if options.export_pairs.is_some() && options.noise_mode == manifest::NoiseMode::Crn {
        eprintln!(
            "warning: --export-pairs with --noise crn is unsuitable for NPE training (DECISIONS.md §G5); use --noise independent"
        );
    }
    let effective_ir_hash = manifest::canonical_ir_hash(&model)?;
    let theta_file = options
        .theta_file
        .as_deref()
        .map(|theta_path| read_theta_file(&model, theta_path))
        .transpose()?;
    let draw_count = match (&theta_file, options.draws) {
        (Some(theta), None) => u32::try_from(theta.assignments.len())
            .expect("theta-file length was checked while reading"),
        (None, Some(draws)) => draws,
        _ => unreachable!("sweep option exclusivity was checked while parsing"),
    };

    let (population_source, population_sha256) =
        manifest::population_identity(&options.population)?;
    let mut run_manifest = manifest::RunManifest::new(
        manifest::ManifestKind::Sweep,
        options.seed,
        options.ticks,
        population_source,
        population_sha256,
    );
    run_manifest.model = Some(model.model().name.clone());
    run_manifest.dt = Some(model.model().dt);
    run_manifest.enabled_features = options.enabled_features.iter().cloned().collect();
    if let Some(plan) = &plan {
        let (plan_identity, linked_source) = manifest::plan_identity_tuples(plan)?;
        run_manifest.plan = Some(plan_identity);
        run_manifest.linked_source = linked_source;
    } else {
        run_manifest.ir_hash = Some(effective_ir_hash.clone());
    }
    run_manifest.noise_mode = Some(options.noise_mode);
    run_manifest.theta_source = Some(match &theta_file {
        Some(theta) => manifest::ThetaSource {
            kind: manifest::ThetaSourceKind::File,
            sha256: theta.sha256.clone(),
            algorithm: manifest::HASH_ALGORITHM.to_owned(),
        },
        None => manifest::ThetaSource {
            kind: manifest::ThetaSourceKind::Prior,
            // Prior-mode theta comes from declarations in the effective,
            // canonical IR. Plan manifests use their plan tuple as run
            // identity, while this digest continues to identify the priors.
            sha256: effective_ir_hash.clone(),
            algorithm: manifest::HASH_ALGORITHM.to_owned(),
        },
    });
    let pinned = match options.params.as_deref() {
        Some(params_path) => read_param_overrides(&model, params_path)?,
        None => Vec::new(),
    };
    let initialized = initialized_tables(&model, &options.population)?;
    run_manifest.initial_state = initialized.state_hash.map(state_artifact_tuple);
    let initial_tables = initialized.tables;
    let out = Path::new(&options.out);
    std::fs::create_dir_all(out).map_err(|error| format!("{}: {error}", out.display()))?;
    if let Some(timing_path) = options.timing_json.as_deref().map(Path::new) {
        let output_directory = out
            .canonicalize()
            .map_err(|error| format!("{}: {error}", out.display()))?;
        if canonical_parent_with_final_component(timing_path)
            .is_some_and(|path| path.starts_with(&output_directory))
        {
            return Err(format!(
                "--timing-json path '{}' must be outside the sweep output directory '{}'",
                timing_path.display(),
                out.display()
            ));
        }
        if options
            .export_pairs
            .as_deref()
            .is_some_and(|path| paths_resolve_to_same_file(timing_path, Path::new(path)))
        {
            return Err("--timing-json path conflicts with --export-pairs output".to_owned());
        }
    }
    remove_previous_sweep_outputs(out)?;
    if let Some(export_path) = options.export_pairs.as_deref().map(Path::new) {
        if let Some(parent) = export_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
    }

    let mut parameter_columns = model
        .model()
        .params
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect::<Vec<_>>();
    parameter_columns.sort();
    let summary_columns = model
        .model()
        .summaries
        .iter()
        .map(|summary| summary.name.clone())
        .collect::<Vec<_>>();
    let mut pairs_csv = options.export_pairs.as_ref().map(|_| {
        let mut columns = vec!["k".to_owned()];
        columns.extend(parameter_columns.iter().cloned());
        columns.extend(summary_columns.iter().cloned());
        let mut csv = columns
            .iter()
            .map(|column| csv_field(column))
            .collect::<Vec<_>>()
            .join(",");
        csv.push('\n');
        csv
    });

    let mut csv_manifest = if theta_file.is_some() {
        String::from("# theta_source=file\n# parameter_status")
    } else {
        String::from("# parameter_status")
    };
    for declaration in &model.model().params {
        let supplied_by_file = theta_file.as_ref().is_some_and(|theta| {
            theta.assignments.iter().any(|assignment| {
                assignment
                    .iter()
                    .any(|value| value.name == declaration.name)
            })
        });
        let status = if supplied_by_file {
            "file"
        } else if pinned.iter().any(|pin| pin.name == declaration.name) {
            "pinned"
        } else if declaration.prior.is_some() {
            "sampled"
        } else {
            "default"
        };
        csv_manifest.push_str(&format!(",{}={status}", declaration.name));
    }
    csv_manifest.push_str("\nk");
    for declaration in &model.model().params {
        csv_manifest.push(',');
        csv_manifest.push_str(&declaration.name);
    }
    csv_manifest.push('\n');

    let mut all_series = Vec::with_capacity(draw_count as usize);
    let mut reported_columns: Option<Vec<String>> = None;
    // Construction values are placeholders only: draw zero also follows the
    // same explicit reset and reseed path as every later draw.
    let construction_params = ParamEnv::defaults(&model);
    let draw_workers = sweep_concurrency_spike_workers(draw_count, options.backend)?;
    let cuda_lockstep =
        sweep_concurrency_spike_cuda_lockstep(draw_count, draw_workers, options.backend)?;
    let mut draw_durations = Vec::with_capacity(draw_count as usize);
    let mut concurrency_spike_timing = None;
    let setup_elapsed;

    if draw_workers == 1 {
        let setup_started = Instant::now();
        let mut backend = SweepBackend::new(
            &model,
            initial_tables,
            &construction_params,
            options.seed,
            options.backend,
        )?;
        setup_elapsed = setup_started.elapsed();
        // This sole retained object cannot span devices. Capture identity once
        // at construction in the unchanged manifest field/schema.
        run_manifest.backend_identity = Some(backend.identity());
        // Deliberately sequential: declaration order within each k, then k order.
        for draw in 0..draw_count {
            let params = match &theta_file {
                Some(theta) => params_from_theta_assignment(
                    &model,
                    options.theta_file.as_deref().expect("theta path exists"),
                    draw,
                    &theta.assignments[draw as usize],
                    &pinned,
                )?,
                None => sample_parameters_for_draw(&model, options.seed, draw, &pinned)
                    .map_err(|error| format!("draw {draw}: {error}"))?,
            };
            csv_manifest.push_str(&draw.to_string());
            for (_, value) in params.values() {
                csv_manifest.push(',');
                csv_manifest.push_str(&param_value_csv(value));
            }
            csv_manifest.push('\n');

            let execution_seed = match options.noise_mode {
                manifest::NoiseMode::Crn => options.seed,
                manifest::NoiseMode::Independent => derive_sweep_replica_seed(options.seed, draw),
            };
            let draw_started = Instant::now();
            let execution = backend.run_draw(
                &model,
                &params,
                execution_seed,
                options.ticks,
                &options.enabled_features,
            )?;
            draw_durations.push(draw_started.elapsed());
            all_series.push(publish_sweep_draw(
                draw,
                &params,
                execution_seed,
                execution,
                &mut reported_columns,
                &mut pairs_csv,
                &parameter_columns,
                &summary_columns,
                &mut run_manifest,
                out,
                options.export_pairs.is_some(),
            )?);
        }
    } else {
        if cuda_lockstep {
            eprintln!(
                "EXPERIMENTAL CUDA lockstep-stream spike: {draw_workers} draw lanes on non-blocking streams; default sweep behavior remains sequential"
            );
        } else {
            eprintln!(
                "EXPERIMENTAL sweep concurrency spike: {draw_workers} isolated {:?} backends; default sweep behavior remains sequential",
                options.backend
            );
        }
        let mut prepared = Vec::with_capacity(draw_count as usize);
        for draw in 0..draw_count {
            let params = match &theta_file {
                Some(theta) => params_from_theta_assignment(
                    &model,
                    options.theta_file.as_deref().expect("theta path exists"),
                    draw,
                    &theta.assignments[draw as usize],
                    &pinned,
                )?,
                None => sample_parameters_for_draw(&model, options.seed, draw, &pinned)
                    .map_err(|error| format!("draw {draw}: {error}"))?,
            };
            csv_manifest.push_str(&draw.to_string());
            for (_, value) in params.values() {
                csv_manifest.push(',');
                csv_manifest.push_str(&param_value_csv(value));
            }
            csv_manifest.push('\n');
            let execution_seed = match options.noise_mode {
                manifest::NoiseMode::Crn => options.seed,
                manifest::NoiseMode::Independent => derive_sweep_replica_seed(options.seed, draw),
            };
            prepared.push(SweepPreparedDraw {
                k: draw,
                params,
                execution_seed,
            });
        }

        let concurrent = run_concurrent_sweep_spike(
            &model,
            &initial_tables,
            &construction_params,
            &options,
            draw_workers,
            &prepared,
            cuda_lockstep,
        )?;
        setup_elapsed = concurrent.setup_elapsed;
        run_manifest.backend_identity = Some(concurrent.identity);
        let publication_started = Instant::now();
        let mut timing_draws = Vec::with_capacity(concurrent.draws.len());
        for completed in concurrent.draws {
            let prepared_draw = &prepared[completed.index];
            timing_draws.push(SweepConcurrencySpikeTimingDraw {
                k: prepared_draw.k,
                lane: completed.lane,
                start_offset_ms: duration_ms(completed.start_offset),
                finish_offset_ms: duration_ms(completed.finish_offset),
                wall_time_ms: duration_ms(completed.elapsed),
            });
            draw_durations.push(completed.elapsed);
            let execution = completed
                .execution
                .map_err(|error| format!("draw {}: {error}", prepared_draw.k))?;
            all_series.push(publish_sweep_draw(
                prepared_draw.k,
                &prepared_draw.params,
                prepared_draw.execution_seed,
                execution,
                &mut reported_columns,
                &mut pairs_csv,
                &parameter_columns,
                &summary_columns,
                &mut run_manifest,
                out,
                options.export_pairs.is_some(),
            )?);
        }
        concurrency_spike_timing = Some((
            concurrent.execution_window_elapsed,
            publication_started.elapsed(),
            timing_draws,
        ));
    }

    let summary = summary_csv(
        reported_columns.as_deref().unwrap_or_default(),
        &all_series,
        options.ticks,
    )?;
    let manifest_path = out.join("manifest.csv");
    let summary_path = out.join("summary.csv");
    std::fs::write(&manifest_path, csv_manifest.as_bytes())
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    std::fs::write(&summary_path, summary.as_bytes())
        .map_err(|error| format!("{}: {error}", summary_path.display()))?;
    manifest::write(&out.join("run-manifest.json"), &run_manifest)?;
    if let (Some(export_path), Some(pairs_csv)) =
        (options.export_pairs.as_deref().map(Path::new), pairs_csv)
    {
        std::fs::write(export_path, pairs_csv.as_bytes())
            .map_err(|error| format!("{}: {error}", export_path.display()))?;
        let pairs_sha256 = hex(&Sha256::digest(pairs_csv.as_bytes()));
        let metadata = manifest::PairsMetadata::for_sweep(
            &run_manifest,
            effective_ir_hash,
            draw_count,
            parameter_columns,
            summary_columns,
            pairs_sha256,
        )?;
        manifest::write_pairs_metadata(&manifest::pairs_sidecar_path(export_path), &metadata)?;
    }
    if let Some(path) = &options.timing_json {
        let backend = match options.backend {
            BackendSelection::Cpu => "cpu",
            BackendSelection::Cuda => "cuda",
        };
        let repository_commit = repository_commit()?;
        let binary_sha256 = current_binary_sha256()?;
        let mut json =
            if let Some((execution_window, publication, timing_draws)) = concurrency_spike_timing {
                serde_json::to_string_pretty(&SweepConcurrencySpikeTimingDocument {
                    schema: "sembla-sweep-concurrency-spike-timing-v1",
                    backend,
                    draws: draw_count,
                    ticks_per_draw: options.ticks,
                    requested_draw_workers: draw_workers,
                    effective_draw_workers: draw_workers,
                    execution_mode: if cuda_lockstep {
                        "cuda-lockstep-nonblocking-streams"
                    } else {
                        "independent-backends"
                    },
                    setup_wall_time_ms: duration_ms(setup_elapsed),
                    execution_window_wall_time_ms: duration_ms(execution_window),
                    publication_wall_time_ms: duration_ms(publication),
                    draw_timings: timing_draws,
                    whole_sweep_wall_time_ms: duration_ms(sweep_started.elapsed()),
                    repository_commit,
                    binary_sha256,
                })
            } else {
                serde_json::to_string_pretty(&SweepTimingDocument {
                    schema: "sembla-sweep-timing-v1",
                    backend,
                    draws: draw_count,
                    ticks_per_draw: options.ticks,
                    setup_wall_time_ms: duration_ms(setup_elapsed),
                    draw_zero_including_setup_wall_time_ms: duration_ms(
                        setup_elapsed + draw_durations[0],
                    ),
                    draw_timings: draw_durations
                        .into_iter()
                        .enumerate()
                        .map(|(k, elapsed)| SweepTimingDraw {
                            k: u32::try_from(k).expect("draw count is u32"),
                            wall_time_ms: duration_ms(elapsed),
                        })
                        .collect(),
                    whole_sweep_wall_time_ms: duration_ms(sweep_started.elapsed()),
                    repository_commit,
                    binary_sha256,
                })
            }
            .map_err(|error| format!("could not serialize sweep timing JSON: {error}"))?;
        json.push('\n');
        std::fs::write(path, json).map_err(|error| format!("{path}: {error}"))?;
    }
    let manifest_hash = hex(&Sha256::digest(csv_manifest.as_bytes()));
    let summary_hash = hex(&Sha256::digest(summary.as_bytes()));
    if let Some(theta) = &theta_file {
        println!(
            "manifest_sha256={manifest_hash} summary_sha256={summary_hash} theta_file_sha256={}",
            theta.sha256
        );
    } else {
        println!("manifest_sha256={manifest_hash} summary_sha256={summary_hash}");
    }
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
            || name == "run-manifest.json"
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

fn append_pairs_row(
    csv: &mut String,
    draw: u32,
    params: &ParamEnv,
    parameter_columns: &[String],
    summaries: &[SummaryValue],
    summary_columns: &[String],
) -> Result<(), String> {
    let mut values = params.values().collect::<Vec<_>>();
    values.sort_by(|left, right| left.0.cmp(right.0));
    if values
        .iter()
        .map(|(name, _)| *name)
        .ne(parameter_columns.iter().map(String::as_str))
    {
        return Err(format!(
            "draw {draw}: resolved parameter columns do not match the export schema"
        ));
    }
    if summaries
        .iter()
        .map(|summary| summary.name.as_str())
        .ne(summary_columns.iter().map(String::as_str))
    {
        return Err(format!(
            "draw {draw}: summary columns do not match model declaration order"
        ));
    }

    csv.push_str(&draw.to_string());
    for (_, value) in values {
        csv.push(',');
        csv.push_str(&param_value_csv(value));
    }
    for summary in summaries {
        csv.push(',');
        csv.push_str(&ReportedValue::from(summary.value).csv());
    }
    csv.push('\n');
    Ok(())
}

fn summary_csv(
    columns: &[String],
    all_series: &[Vec<Vec<ReportedValue>>],
    ticks: u32,
) -> Result<String, String> {
    const PERCENTILES: [usize; 5] = [5, 25, 50, 75, 95];
    let mut csv = String::from("tick");
    for name in columns {
        for percentile in PERCENTILES {
            csv.push(',');
            csv.push_str(&csv_field(&format!("{name}_p{percentile:02}")));
        }
    }
    csv.push('\n');
    for tick in 0..ticks as usize {
        csv.push_str(&tick.to_string());
        for (column, column_name) in columns.iter().enumerate() {
            let mut values = all_series
                .iter()
                .map(|series| {
                    series
                        .get(tick)
                        .and_then(|row| row.get(column))
                        .copied()
                        .ok_or_else(|| {
                            format!("reported series is missing tick {tick} column '{column_name}'")
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(first) = values.first().copied() {
                for value in values.iter().skip(1).copied() {
                    value.cmp(first)?;
                }
            }
            values.sort_by(|left, right| {
                left.cmp(*right)
                    .expect("reported column type was checked before sorting")
            });
            for percentile in PERCENTILES {
                // Deterministic nearest index to p * (n - 1).
                let index = ((values.len() - 1) * percentile + 50) / 100;
                csv.push(',');
                csv.push_str(&values[index].csv());
            }
        }
        csv.push('\n');
    }
    Ok(csv)
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

fn param_value_from_json(
    declaration: &sembla_ir::ParamDecl,
    value: &serde_json::Value,
    context: &str,
) -> Result<ParamValue, String> {
    match declaration.ty {
        ParamType::Real => Ok(ParamValue::Real {
            value: value.as_f64().ok_or_else(|| {
                format!(
                    "{context}: parameter '{}' must have type real",
                    declaration.name
                )
            })?,
        }),
        ParamType::Int => Ok(ParamValue::Int {
            value: value.as_i64().ok_or_else(|| {
                format!(
                    "{context}: parameter '{}' must have type int",
                    declaration.name
                )
            })?,
        }),
    }
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
        let value = param_value_from_json(declaration, value, path)?;
        overrides.push(ParamOverride::new(name, value));
    }
    ParamEnv::resolve(model, &overrides).map_err(|error| format!("{path}: {error}"))?;
    Ok(overrides)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExecutionHashes {
    results_sha256: String,
    final_state_sha256: String,
    observation_sha256: String,
}

#[derive(Clone, Copy, Debug)]
enum ReportedValue {
    Unsigned(usize),
    Int(i64),
    Real(f64),
}

impl ReportedValue {
    fn csv(self) -> String {
        match self {
            Self::Unsigned(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::Real(value) => value.to_string(),
        }
    }

    fn cmp(self, other: Self) -> Result<std::cmp::Ordering, String> {
        match (self, other) {
            (Self::Unsigned(left), Self::Unsigned(right)) => Ok(left.cmp(&right)),
            (Self::Int(left), Self::Int(right)) => Ok(left.cmp(&right)),
            (Self::Real(left), Self::Real(right)) => Ok(left.total_cmp(&right)),
            _ => Err("reported column changed numeric type across draws".to_owned()),
        }
    }

    fn as_usize(self, context: &str) -> Result<usize, String> {
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
struct ReportedSeries {
    columns: Vec<String>,
    rows: Vec<Vec<ReportedValue>>,
}

#[derive(Clone, Debug)]
struct GroupedCsvOutput {
    view: String,
    csv: String,
}

type StateHash = [u8; 32];
type PerTickHashes = Option<Vec<StateHash>>;
type ComparedPerTickHashes<'a> = (&'a [StateHash], &'a [StateHash]);

#[derive(Clone, Debug)]
struct RunOutput {
    csv: String,
    grouped: Vec<GroupedCsvOutput>,
    series: ReportedSeries,
    summaries: Vec<SummaryValue>,
    summaries_csv: String,
    per_tick_hashes: PerTickHashes,
}

fn summaries_path(output: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{output}.summaries.csv"))
}

fn grouped_output_path(output: &Path, view: &str) -> PathBuf {
    let stem = output
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("results");
    let name = format!("{stem}.grouped.{view}.csv");
    output.parent().unwrap_or_else(|| Path::new("")).join(name)
}

fn grouped_output_records(outputs: &[GroupedCsvOutput]) -> Vec<manifest::GroupedOutputRecord> {
    outputs
        .iter()
        .map(|output| manifest::GroupedOutputRecord {
            view: output.view.clone(),
            algorithm: manifest::HASH_ALGORITHM.to_owned(),
            sha256: hex(&Sha256::digest(output.csv.as_bytes())),
        })
        .collect()
}

fn paths_resolve_to_same_file(left: &Path, right: &Path) -> bool {
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

fn resolve_collision_path(path: &Path) -> Option<PathBuf> {
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

fn canonical_parent_with_final_component(path: &Path) -> Option<PathBuf> {
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
fn same_file_identity(left: &Path, right: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    matches!(
        (std::fs::metadata(left), std::fs::metadata(right)),
        (Ok(left), Ok(right)) if left.dev() == right.dev() && left.ino() == right.ino()
    )
}

#[cfg(not(unix))]
fn same_file_identity(_left: &Path, _right: &Path) -> bool {
    false
}

#[cfg(test)]
static SWEEP_BACKEND_CONSTRUCTIONS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

enum SweepBackend {
    Cpu {
        state: StateStore,
        initial: Vec<TableInit>,
    },
    #[cfg(feature = "cuda")]
    Cuda(CudaBackend),
}

struct SweepDrawOutput {
    output: RunOutput,
    final_state_hash: [u8; 32],
}

impl SweepBackend {
    fn new(
        model: &sembla_ir::ValidatedModel,
        initial: Vec<TableInit>,
        initial_params: &ParamEnv,
        seed: u64,
        backend: BackendSelection,
    ) -> Result<Self, String> {
        #[cfg(test)]
        SWEEP_BACKEND_CONSTRUCTIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match backend {
            BackendSelection::Cpu => {
                let state =
                    StateStore::new(model, initial.clone()).map_err(|error| error.to_string())?;
                Ok(Self::Cpu { state, initial })
            }
            BackendSelection::Cuda => {
                #[cfg(feature = "cuda")]
                {
                    let backend =
                        CudaBackend::new(model, initial, initial_params, seed, HashMode::FinalOnly)
                            .map_err(|error| error.to_string())?;
                    report_cuda_observation_eligibility(backend.observation_eligibility());
                    Ok(Self::Cuda(backend))
                }
                #[cfg(not(feature = "cuda"))]
                {
                    let _ = (model, initial, initial_params, seed);
                    Err(
                        "cuda backend unavailable: crate built without the 'cuda' feature"
                            .to_owned(),
                    )
                }
            }
        }
    }

    fn new_concurrency_lane(
        model: &sembla_ir::ValidatedModel,
        initial: Vec<TableInit>,
        initial_params: &ParamEnv,
        seed: u64,
        backend: BackendSelection,
        cuda_lockstep: bool,
    ) -> Result<Self, String> {
        if !cuda_lockstep {
            return Self::new(model, initial, initial_params, seed, backend);
        }
        #[cfg(test)]
        SWEEP_BACKEND_CONSTRUCTIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match backend {
            BackendSelection::Cuda => {
                #[cfg(feature = "cuda")]
                {
                    let backend = CudaBackend::new_nonblocking_stream(
                        model,
                        initial,
                        initial_params,
                        seed,
                        HashMode::FinalOnly,
                    )
                    .map_err(|error| error.to_string())?;
                    report_cuda_observation_eligibility(backend.observation_eligibility());
                    Ok(Self::Cuda(backend))
                }
                #[cfg(not(feature = "cuda"))]
                {
                    let _ = (model, initial, initial_params, seed);
                    Err(
                        "cuda backend unavailable: crate built without the 'cuda' feature"
                            .to_owned(),
                    )
                }
            }
            BackendSelection::Cpu => {
                Err("CUDA lockstep-stream spike requires --backend cuda".to_owned())
            }
        }
    }

    fn identity(&self) -> manifest::BackendIdentity {
        match self {
            Self::Cpu { .. } => manifest::BackendIdentity::cpu_oracle(),
            #[cfg(feature = "cuda")]
            Self::Cuda(backend) => {
                let device = backend.device_identity();
                manifest::BackendIdentity::cuda_native_f64(
                    device.gpu_model.clone(),
                    device.driver_version.clone(),
                )
            }
        }
    }

    fn run_draw(
        &mut self,
        model: &sembla_ir::ValidatedModel,
        params: &ParamEnv,
        seed: u64,
        ticks: u32,
        enabled_features: &FeatureSet,
    ) -> Result<SweepDrawOutput, String> {
        match self {
            Self::Cpu { state, initial } => {
                state
                    .reset_backend_draw(model, initial)
                    .map_err(|error| error.to_string())?;
                let output = run_results_output_with_features(
                    model,
                    state,
                    params,
                    seed,
                    ticks,
                    HashMode::FinalOnly,
                    enabled_features,
                )?;
                Ok(SweepDrawOutput {
                    output,
                    final_state_hash: state.state_hash(),
                })
            }
            #[cfg(feature = "cuda")]
            Self::Cuda(backend) => {
                backend
                    .reset_draw(params, seed)
                    .map_err(|error| error.to_string())?;
                let mut output = RunOutputAccumulator::new(model, params, ticks)?;
                for tick in 0..ticks {
                    let (observed_tick, fired_per_box, deferred_per_resource_table, device_views) =
                        backend
                            .run_tick_observed_reused()
                            .map_err(|error| format!("tick {tick}: {error}"))?;
                    debug_assert_eq!(observed_tick, tick);
                    let (views, grouped_views, generic_enum_counts) = match device_views {
                        Some(observation) => (
                            observation.views,
                            observation.grouped_views,
                            observation.generic_enum_counts,
                        ),
                        None => {
                            let views =
                                executor::observe_views(model, backend.observed_state(), params)
                                    .map_err(|error| format!("tick {tick}: {error}"))?;
                            let grouped_views = executor::observe_grouped_views(
                                model,
                                backend.observed_state(),
                                params,
                            )
                            .map_err(|error| format!("tick {tick}: {error}"))?;
                            (views, grouped_views, None)
                        }
                    };
                    let report = cuda_tick_report(
                        model,
                        tick,
                        fired_per_box,
                        deferred_per_resource_table,
                        views,
                        grouped_views,
                    );
                    output.push_tick_with_enum_counts(
                        backend.observed_state(),
                        tick,
                        report,
                        generic_enum_counts.as_deref(),
                    )?;
                }
                let output = output.finish(model, None)?;
                let final_state_hash = backend
                    .ensure_observed_state()
                    .map_err(|error| error.to_string())?
                    .state_hash();
                Ok(SweepDrawOutput {
                    output,
                    final_state_hash,
                })
            }
        }
    }

    fn run_draw_lockstep(
        &mut self,
        model: &sembla_ir::ValidatedModel,
        params: &ParamEnv,
        seed: u64,
        ticks: u32,
        _enabled_features: &FeatureSet,
        tick_barrier: &std::sync::Barrier,
    ) -> Result<SweepDrawOutput, String> {
        #[cfg(not(feature = "cuda"))]
        let _ = (model, params, seed);
        match self {
            Self::Cpu { .. } => {
                // Preserve the barrier protocol even on an impossible route so
                // a validation error cannot strand CUDA peers.
                tick_barrier.wait();
                for _ in 0..ticks {
                    tick_barrier.wait();
                }
                Err("CUDA lockstep-stream spike requires --backend cuda".to_owned())
            }
            #[cfg(feature = "cuda")]
            Self::Cuda(backend) => {
                let reset = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    backend
                        .reset_draw(params, seed)
                        .map_err(|error| error.to_string())
                }))
                .unwrap_or_else(|_| {
                    Err("lockstep worker panicked while resetting its draw".to_owned())
                });
                let mut failure = reset.err();
                let mut output = if failure.is_none() {
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        RunOutputAccumulator::new(model, params, ticks)
                    }))
                    .unwrap_or_else(|_| {
                        Err(
                            "lockstep worker panicked while creating its output accumulator"
                                .to_owned(),
                        )
                    }) {
                        Ok(output) => Some(output),
                        Err(error) => {
                            failure = Some(error);
                            None
                        }
                    }
                } else {
                    None
                };

                // All lanes finish reset before tick zero. Every lane reaches
                // every later barrier even after a local error, so one failing
                // draw cannot deadlock its peers.
                tick_barrier.wait();
                for tick in 0..ticks {
                    if failure.is_none() {
                        let tick_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            || -> Result<(), String> {
                                let (
                                    observed_tick,
                                    fired_per_box,
                                    deferred_per_resource_table,
                                    device_views,
                                ) = backend
                                    .run_tick_observed_reused()
                                    .map_err(|error| format!("tick {tick}: {error}"))?;
                                debug_assert_eq!(observed_tick, tick);
                                let (views, grouped_views, generic_enum_counts) = match device_views
                                {
                                    Some(observation) => (
                                        observation.views,
                                        observation.grouped_views,
                                        observation.generic_enum_counts,
                                    ),
                                    None => {
                                        let views = executor::observe_views(
                                            model,
                                            backend.observed_state(),
                                            params,
                                        )
                                        .map_err(|error| format!("tick {tick}: {error}"))?;
                                        let grouped_views = executor::observe_grouped_views(
                                            model,
                                            backend.observed_state(),
                                            params,
                                        )
                                        .map_err(|error| format!("tick {tick}: {error}"))?;
                                        (views, grouped_views, None)
                                    }
                                };
                                let report = cuda_tick_report(
                                    model,
                                    tick,
                                    fired_per_box,
                                    deferred_per_resource_table,
                                    views,
                                    grouped_views,
                                );
                                output
                                    .as_mut()
                                    .expect("output exists while lockstep draw is healthy")
                                    .push_tick_with_enum_counts(
                                        backend.observed_state(),
                                        tick,
                                        report,
                                        generic_enum_counts.as_deref(),
                                    )
                            },
                        ))
                        .unwrap_or_else(|_| Err(format!("tick {tick}: lockstep worker panicked")));
                        if let Err(error) = tick_result {
                            failure = Some(error);
                        }
                    }
                    tick_barrier.wait();
                }
                if let Some(error) = failure {
                    return Err(error);
                }
                let output = output
                    .expect("healthy lockstep draw has an accumulator")
                    .finish(model, None)?;
                let final_state_hash = backend
                    .ensure_observed_state()
                    .map_err(|error| error.to_string())?
                    .state_hash();
                Ok(SweepDrawOutput {
                    output,
                    final_state_hash,
                })
            }
        }
    }
}

struct BackendRunOutput {
    output: RunOutput,
    state: StateStore,
    identity: manifest::BackendIdentity,
    per_tick_hashes: PerTickHashes,
    elapsed: std::time::Duration,
}

#[derive(Clone, Copy, Debug, Default)]
struct PhaseDurations {
    execute_tick: Option<Duration>,
    kernels: Option<Duration>,
    readback_control: Option<Duration>,
    state_transfer: Option<Duration>,
    state_reconstruct: Option<Duration>,
    state_hash: Option<Duration>,
    observe_views: Option<Duration>,
    report: Option<Duration>,
    other: Option<Duration>,
}

impl PhaseDurations {
    fn zero_for_backend(backend: BackendSelection) -> Self {
        match backend {
            BackendSelection::Cpu => Self {
                execute_tick: Some(Duration::ZERO),
                state_hash: Some(Duration::ZERO),
                observe_views: Some(Duration::ZERO),
                report: Some(Duration::ZERO),
                other: Some(Duration::ZERO),
                ..Self::default()
            },
            BackendSelection::Cuda => Self {
                kernels: Some(Duration::ZERO),
                readback_control: Some(Duration::ZERO),
                state_transfer: Some(Duration::ZERO),
                state_reconstruct: Some(Duration::ZERO),
                state_hash: Some(Duration::ZERO),
                observe_views: Some(Duration::ZERO),
                report: Some(Duration::ZERO),
                other: Some(Duration::ZERO),
                ..Self::default()
            },
        }
    }

    fn attributed(&self) -> Duration {
        [
            self.execute_tick,
            self.kernels,
            self.readback_control,
            self.state_transfer,
            self.state_reconstruct,
            self.state_hash,
            self.observe_views,
            self.report,
        ]
        .into_iter()
        .flatten()
        .sum()
    }

    fn total(&self) -> Duration {
        self.attributed() + self.other.unwrap_or_default()
    }

    fn add_assign(&mut self, other: Self) {
        fn add(slot: &mut Option<Duration>, value: Option<Duration>) {
            if let Some(value) = value {
                *slot = Some(slot.unwrap_or_default() + value);
            }
        }
        add(&mut self.execute_tick, other.execute_tick);
        add(&mut self.kernels, other.kernels);
        add(&mut self.readback_control, other.readback_control);
        add(&mut self.state_transfer, other.state_transfer);
        add(&mut self.state_reconstruct, other.state_reconstruct);
        add(&mut self.state_hash, other.state_hash);
        add(&mut self.observe_views, other.observe_views);
        add(&mut self.report, other.report);
        add(&mut self.other, other.other);
    }

    fn milliseconds(self) -> TimingPhases {
        TimingPhases {
            execute_tick: self.execute_tick.map(duration_ms),
            kernels: self.kernels.map(duration_ms),
            readback_control: self.readback_control.map(duration_ms),
            state_transfer: self.state_transfer.map(duration_ms),
            state_reconstruct: self.state_reconstruct.map(duration_ms),
            state_hash: self.state_hash.map(duration_ms),
            observe_views: self.observe_views.map(duration_ms),
            report: self.report.map(duration_ms),
            other: self.other.map(duration_ms),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct TickTiming {
    tick: u32,
    wall_time: Duration,
    phases: PhaseDurations,
}

fn finish_tick_timing(
    tick: u32,
    wall_time: Duration,
    mut phases: PhaseDurations,
) -> Result<TickTiming, String> {
    phases.other = Some(wall_time.checked_sub(phases.attributed()).ok_or_else(|| {
        format!("tick {tick}: attributed timing exceeds measured tick wall time")
    })?);
    Ok(TickTiming {
        tick,
        wall_time,
        phases,
    })
}

#[derive(Serialize)]
struct SweepTimingDraw {
    k: u32,
    wall_time_ms: f64,
}

#[derive(Serialize)]
struct SweepTimingDocument {
    schema: &'static str,
    backend: &'static str,
    draws: u32,
    ticks_per_draw: u32,
    setup_wall_time_ms: f64,
    draw_zero_including_setup_wall_time_ms: f64,
    draw_timings: Vec<SweepTimingDraw>,
    whole_sweep_wall_time_ms: f64,
    repository_commit: String,
    binary_sha256: String,
}

#[derive(Serialize)]
struct SweepConcurrencySpikeTimingDraw {
    k: u32,
    lane: usize,
    start_offset_ms: f64,
    finish_offset_ms: f64,
    wall_time_ms: f64,
}

#[derive(Serialize)]
struct SweepConcurrencySpikeTimingDocument {
    schema: &'static str,
    backend: &'static str,
    draws: u32,
    ticks_per_draw: u32,
    requested_draw_workers: usize,
    effective_draw_workers: usize,
    execution_mode: &'static str,
    setup_wall_time_ms: f64,
    execution_window_wall_time_ms: f64,
    publication_wall_time_ms: f64,
    draw_timings: Vec<SweepConcurrencySpikeTimingDraw>,
    whole_sweep_wall_time_ms: f64,
    repository_commit: String,
    binary_sha256: String,
}

#[derive(Serialize)]
struct TimingSession {
    backend: &'static str,
    scale: usize,
    ticks: u32,
    seed: u64,
    repository_commit: String,
    binary_sha256: String,
}

#[derive(Serialize)]
struct TimerMetadata {
    clock: &'static str,
    resolution: &'static str,
    reported_unit: &'static str,
}

#[derive(Clone, Copy, Serialize)]
struct TimingPhases {
    #[serde(skip_serializing_if = "Option::is_none")]
    execute_tick: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kernels: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readback_control: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_transfer: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_reconstruct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_hash: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observe_views: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    other: Option<f64>,
}

#[derive(Serialize)]
struct TimingTick {
    tick: u32,
    wall_time_ms: f64,
    phases_ms: TimingPhases,
    phase_sum_ms: f64,
    within_tolerance: bool,
}

#[derive(Serialize)]
struct TimingTotals {
    wall_time_ms: f64,
    phases_ms: TimingPhases,
    phase_sum_ms: f64,
}

#[derive(Serialize)]
struct TimingSelfCheck {
    tolerance_ms: f64,
    all_ticks_reconciled: bool,
    other_non_negative: bool,
}

#[derive(Serialize)]
struct TimingDocument {
    schema: &'static str,
    session: TimingSession,
    kernel_sync_inserted: bool,
    timer: TimerMetadata,
    ticks: Vec<TimingTick>,
    totals: TimingTotals,
    self_check: TimingSelfCheck,
}

impl TimingDocument {
    fn new(
        backend: BackendSelection,
        scale: usize,
        ticks: u32,
        seed: u64,
        kernel_sync_inserted: bool,
        tick_timings: Vec<TickTiming>,
    ) -> Result<Self, String> {
        const TOLERANCE_MS: f64 = 0.001;
        let mut total_wall = Duration::ZERO;
        let mut total_phases = PhaseDurations::zero_for_backend(backend);
        let mut rows = Vec::with_capacity(tick_timings.len());
        for timing in tick_timings {
            let phase_sum = timing.phases.total();
            let within_tolerance =
                (duration_ms(phase_sum) - duration_ms(timing.wall_time)).abs() <= TOLERANCE_MS;
            total_wall += timing.wall_time;
            total_phases.add_assign(timing.phases);
            rows.push(TimingTick {
                tick: timing.tick,
                wall_time_ms: duration_ms(timing.wall_time),
                phases_ms: timing.phases.milliseconds(),
                phase_sum_ms: duration_ms(phase_sum),
                within_tolerance,
            });
        }
        let total_phase_time = total_phases.total();
        let all_ticks_reconciled = rows.iter().all(|row| row.within_tolerance)
            && (duration_ms(total_phase_time) - duration_ms(total_wall)).abs() <= TOLERANCE_MS;
        if !all_ticks_reconciled {
            return Err("timing phases did not reconcile with measured tick wall time".to_owned());
        }
        Ok(Self {
            schema: "sembla-execution-timing-v1",
            session: TimingSession {
                backend: match backend {
                    BackendSelection::Cpu => "cpu",
                    BackendSelection::Cuda => "cuda",
                },
                scale,
                ticks,
                seed,
                repository_commit: repository_commit()?,
                binary_sha256: current_binary_sha256()?,
            },
            kernel_sync_inserted,
            timer: TimerMetadata {
                clock: "std::time::Instant",
                resolution: "nanoseconds",
                reported_unit: "milliseconds",
            },
            ticks: rows,
            totals: TimingTotals {
                wall_time_ms: duration_ms(total_wall),
                phases_ms: total_phases.milliseconds(),
                phase_sum_ms: duration_ms(total_phase_time),
            },
            self_check: TimingSelfCheck {
                tolerance_ms: TOLERANCE_MS,
                all_ticks_reconciled,
                other_non_negative: true,
            },
        })
    }
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn repository_commit() -> Result<String, String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|error| format!("could not resolve repository commit for timing JSON: {error}"))?;
    if !output.status.success() {
        return Err("could not resolve repository commit for timing JSON".to_owned());
    }
    let commit = String::from_utf8(output.stdout)
        .map_err(|_| "repository commit is not UTF-8".to_owned())?
        .trim()
        .to_owned();
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("repository commit is not a full SHA-1".to_owned());
    }
    Ok(commit)
}

fn current_binary_sha256() -> Result<String, String> {
    let binary = std::env::current_exe()
        .map_err(|error| format!("could not locate current binary for timing JSON: {error}"))?;
    let bytes = std::fs::read(&binary).map_err(|error| format!("{}: {error}", binary.display()))?;
    Ok(hex(&Sha256::digest(bytes)))
}

fn write_timing_document(path: &str, timing: &TimingDocument) -> Result<(), String> {
    let mut json = serde_json::to_string_pretty(timing)
        .map_err(|error| format!("could not serialize timing JSON: {error}"))?;
    json.push('\n');
    std::fs::write(path, json).map_err(|error| format!("{path}: {error}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BackendRunMode {
    backend: BackendSelection,
    hash_mode: HashMode,
}

impl BackendRunMode {
    fn final_only(backend: BackendSelection) -> Self {
        Self {
            backend,
            hash_mode: HashMode::FinalOnly,
        }
    }

    fn every_tick(backend: BackendSelection) -> Self {
        Self {
            backend,
            hash_mode: HashMode::EveryTick,
        }
    }
}

fn execute_backend_output_with_features(
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
fn execute_backend_output_timed_with_features(
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

fn execution_hashes(output: &RunOutput, state: &StateStore) -> ExecutionHashes {
    execution_hashes_with_state_hash(output, state.state_hash())
}

fn execution_hashes_with_state_hash(
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

fn grouped_csv_outputs(
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

struct RunOutputAccumulator {
    has_views: bool,
    enums: Option<Vec<EnumCountDescriptor>>,
    firings: Vec<FiringDescriptor>,
    headers: Vec<String>,
    csv: String,
    rows: Vec<Vec<ReportedValue>>,
    tick_reports: Vec<executor::TickReport>,
}

impl RunOutputAccumulator {
    fn new(
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

    fn push_tick(
        &mut self,
        state: &StateStore,
        tick: u32,
        report: executor::TickReport,
    ) -> Result<(), String> {
        self.push_tick_with_enum_counts(state, tick, report, None)
    }

    fn push_tick_with_enum_counts(
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

    fn finish(
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
fn run_results_output(
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

fn run_results_output_with_features(
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

fn run_results_output_timed_with_features(
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
fn report_cuda_observation_eligibility(
    eligibility: &sembla_runtime::executor::DeviceObservationEligibility,
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

fn cuda_tick_report(
    model: &sembla_ir::ValidatedModel,
    tick: u32,
    fired_per_box: Vec<(String, Vec<(u32, usize)>)>,
    deferred_per_resource_table: Vec<(String, usize)>,
    views: Vec<sembla_runtime::executor::ViewValue>,
    grouped_views: Vec<sembla_runtime::executor::GroupedViewValue>,
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
fn run_results_output_cuda(
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
        output,
        state,
        identity,
        per_tick_hashes: hashes,
        elapsed,
    })
}

#[cfg(feature = "cuda")]
fn run_results_output_cuda(
    model: &sembla_ir::ValidatedModel,
    initial: Vec<TableInit>,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
    hash_mode: HashMode,
) -> Result<BackendRunOutput, String> {
    let mut backend = CudaBackend::new(model, initial, params, seed, hash_mode)
        .map_err(|error| error.to_string())?;
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
        let (views, grouped_views, generic_enum_counts) = match device_views {
            Some(observation) => (
                observation.views,
                observation.grouped_views,
                observation.generic_enum_counts,
            ),
            None => {
                let views = executor::observe_views(model, backend.observed_state(), params)
                    .map_err(|error| format!("tick {tick}: {error}"))?;
                let grouped_views =
                    executor::observe_grouped_views(model, backend.observed_state(), params)
                        .map_err(|error| format!("tick {tick}: {error}"))?;
                (views, grouped_views, None)
            }
        };
        let report = cuda_tick_report(
            model,
            tick,
            fired_per_box,
            deferred_per_resource_table,
            views,
            grouped_views,
        );
        output.push_tick_with_enum_counts(
            backend.observed_state(),
            tick,
            report,
            generic_enum_counts.as_deref(),
        )?;
    }
    let output = output.finish(model, hashes.clone())?;
    let state = backend
        .into_observed_state()
        .map_err(|error| error.to_string())?;
    let elapsed = started.elapsed();
    Ok(BackendRunOutput {
        output,
        state,
        identity,
        per_tick_hashes: hashes,
        elapsed,
    })
}

#[cfg(feature = "cuda")]
fn run_results_output_cuda_timed(
    model: &sembla_ir::ValidatedModel,
    initial: Vec<TableInit>,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
    hash_mode: HashMode,
) -> Result<(BackendRunOutput, Vec<TickTiming>), String> {
    let mut backend = CudaBackend::new(model, initial, params, seed, hash_mode)
        .map_err(|error| error.to_string())?;
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
        let (views, grouped_views, generic_enum_counts) = match device_views {
            Some(observation) => (
                observation.views,
                observation.grouped_views,
                observation.generic_enum_counts,
            ),
            None => {
                let views = executor::observe_views(model, backend.observed_state(), params)
                    .map_err(|error| format!("tick {tick}: {error}"))?;
                let grouped_views =
                    executor::observe_grouped_views(model, backend.observed_state(), params)
                        .map_err(|error| format!("tick {tick}: {error}"))?;
                (views, grouped_views, None)
            }
        };
        let observe_views = phase_started.elapsed();

        let phase_started = Instant::now();
        let report = cuda_tick_report(
            model,
            tick,
            fired_per_box,
            deferred_per_resource_table,
            views,
            grouped_views,
        );
        output.push_tick_with_enum_counts(
            backend.observed_state(),
            tick,
            report,
            generic_enum_counts.as_deref(),
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
            output,
            state,
            identity,
            per_tick_hashes: hashes,
            elapsed,
        },
        tick_timings,
    ))
}

fn summaries_csv(summaries: &[SummaryValue]) -> String {
    let mut csv = String::from("name,value\n");
    for summary in summaries {
        csv.push_str(&csv_field(&summary.name));
        csv.push(',');
        csv.push_str(&ReportedValue::from(summary.value).csv());
        csv.push('\n');
    }
    csv
}

fn parent_occurrence_path(box_name: &str) -> &str {
    box_name.rsplit_once('/').map_or("", |(parent, _)| parent)
}

fn initializers_from_population(
    model: &sembla_ir::ValidatedModel,
    population: &SyntheticPopulation,
) -> Result<Vec<TableInit>, String> {
    let population_boxes = model
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
    if population_boxes.is_empty() {
        return Err(
            "population file requires exactly one compatible person/employer schema, found 0"
                .to_owned(),
        );
    }
    let controller_boxes = model
        .model()
        .boxes
        .iter()
        .filter(|model_box| {
            model_box.tables.iter().any(|table| {
                table.name == "controller"
                    && table.size_hint == 1
                    && table.attrs.iter().any(|attr| attr.name == "mode")
                    && table.attrs.iter().any(|attr| attr.name == "modifier")
            })
        })
        .collect::<Vec<_>>();

    let mut initial = Vec::new();
    let mut initialized_population_boxes = Vec::new();
    let mut initialized_controller_boxes = Vec::new();
    if let [population_box] = population_boxes.as_slice() {
        // Preserve the pre-PRD single-population behavior exactly.
        let controller_box = match controller_boxes.as_slice() {
            [] => None,
            [model_box] => Some(*model_box),
            _ => {
                return Err(format!(
                    "population file found {} compatible controller schemas",
                    controller_boxes.len()
                ));
            }
        };
        initial.extend(match controller_box {
            Some(controller) => population
                .sir_policy_table_initializers_for_boxes(&population_box.name, &controller.name),
            None => population.sir_table_initializers_for_box(&population_box.name),
        });
        initialized_population_boxes.push(population_box.name.as_str());
        if let Some(controller) = controller_box {
            initialized_controller_boxes.push(controller.name.as_str());
        }
    } else {
        for (index, population_box) in population_boxes.iter().enumerate() {
            let scope = parent_occurrence_path(&population_box.name);
            if population_boxes[..index]
                .iter()
                .any(|other| parent_occurrence_path(&other.name) == scope)
            {
                return Err(format!(
                    "population file found multiple compatible population schemas in occurrence scope '{scope}'"
                ));
            }
            let matching_controllers = controller_boxes
                .iter()
                .filter(|controller| parent_occurrence_path(&controller.name) == scope)
                .copied()
                .collect::<Vec<_>>();
            let controller = match matching_controllers.as_slice() {
                [] if controller_boxes.is_empty() => None,
                [controller] => Some(*controller),
                _ => {
                    return Err(format!(
                        "population file found {} compatible controller schemas in occurrence scope '{scope}'",
                        matching_controllers.len()
                    ));
                }
            };
            initial.extend(match controller {
                Some(controller) => population.sir_policy_table_initializers_for_boxes(
                    &population_box.name,
                    &controller.name,
                ),
                None => population.sir_table_initializers_for_box(&population_box.name),
            });
            initialized_population_boxes.push(population_box.name.as_str());
            if let Some(controller) = controller {
                initialized_controller_boxes.push(controller.name.as_str());
            }
        }
        if let Some(unmatched) = controller_boxes
            .iter()
            .find(|controller| !initialized_controller_boxes.contains(&controller.name.as_str()))
        {
            return Err(format!(
                "population file found unmatched compatible controller schema '{}'",
                unmatched.name
            ));
        }
    }

    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            if (initialized_population_boxes.contains(&model_box.name.as_str())
                && (table.name == "person" || table.name == "employer"))
                || (initialized_controller_boxes.contains(&model_box.name.as_str())
                    && table.name == "controller")
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

fn verify_run(manifest_path: &str, model_path: &str, options: VerifyOptions) -> i32 {
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

fn verify_run_result(
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

struct InitializedTables {
    tables: Vec<TableInit>,
    state_hash: Option<sembla_ir::HashRecordV1>,
}

fn state_artifact_tuple(hash: sembla_ir::HashRecordV1) -> manifest::StateArtifactTuple {
    manifest::StateArtifactTuple {
        format: STATE_ARTIFACT_SCHEMA.to_owned(),
        hash,
    }
}

fn initialized_tables(
    model: &sembla_ir::ValidatedModel,
    population_spec: &str,
) -> Result<InitializedTables, String> {
    if let Ok(population) = population_spec.parse::<usize>() {
        return Ok(InitializedTables {
            tables: initialize_population(model, population),
            state_hash: None,
        });
    }
    match sniff_magic(population_spec).map_err(|error| error.to_string())? {
        StateKind::SemblaPop => Ok(InitializedTables {
            tables: initializers_from_population(
                model,
                &SyntheticPopulation::read(population_spec).map_err(|error| error.to_string())?,
            )?,
            state_hash: None,
        }),
        StateKind::SemblaState => {
            let artifact = read_state_artifact(population_spec).map_err(|error| error.to_string())?;
            let tables = to_table_inits(&artifact, model).map_err(|error| error.to_string())?;
            let state_hash =
                state_artifact_hash(population_spec).map_err(|error| error.to_string())?;
            Ok(InitializedTables {
                tables,
                state_hash: Some(state_hash),
            })
        }
        StateKind::Unknown => Err(format!(
            "unrecognized population artifact magic in '{population_spec}'; supported formats: SEMBLA_POP, SEMBLA_STATE"
        )),
    }
}

fn params_from_manifest(
    model: &sembla_ir::ValidatedModel,
    values: &std::collections::BTreeMap<String, manifest::ResolvedValue>,
) -> Result<ParamEnv, String> {
    let expected_names = model
        .model()
        .params
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let actual_names = values
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if actual_names != expected_names {
        return Err(format!(
            "manifest resolved_theta parameter names mismatch: recorded={actual_names:?} expected={expected_names:?}"
        ));
    }
    let overrides = values
        .iter()
        .map(|(name, value)| {
            let value = match value {
                manifest::ResolvedValue::Real(value) => ParamValue::Real { value: *value },
                manifest::ResolvedValue::Int(value) => ParamValue::Int { value: *value },
            };
            ParamOverride::new(name, value)
        })
        .collect::<Vec<_>>();
    ParamEnv::resolve(model, &overrides)
        .map_err(|error| format!("manifest resolved_theta: {error}"))
}

fn compare_field<T: std::fmt::Debug + PartialEq>(
    field: &str,
    recorded: &T,
    actual: &T,
    differences: &mut Vec<String>,
) {
    if recorded != actual {
        differences.push(format!("{field}: recorded={recorded:?} actual={actual:?}"));
    }
}

fn finish_verification(differences: Vec<String>, count: usize) -> Result<usize, String> {
    if differences.is_empty() {
        Ok(count)
    } else {
        Err(format!(
            "verification mismatch:\n  {}",
            differences.join("\n  ")
        ))
    }
}

#[derive(Clone, Debug)]
struct CompareTick {
    counts: [usize; 3],
    fired_infect: usize,
    fired_recover: usize,
    deferred_total: usize,
}

#[derive(Clone, Debug)]
struct CompareArmOutcome {
    series: ReportedSeries,
    hashes: ExecutionHashes,
    identity: manifest::BackendIdentity,
}

fn legacy_compare_ticks(series: &ReportedSeries) -> Result<Vec<CompareTick>, String> {
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

fn reported_difference(left: ReportedValue, right: ReportedValue) -> Result<String, String> {
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

fn compare_result(options: CompareOptions) -> Result<(), String> {
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

    std::fs::write(&options.out, csv.as_bytes())
        .map_err(|error| format!("{}: {error}", options.out))?;
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

fn compare_arm(
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiffInputMode {
    Single,
    AllExamples,
    AllPlanFixtures,
}

#[derive(Clone, Debug)]
struct DiffOptions {
    models: Vec<String>,
    population: String,
    seed: u64,
    ticks: u32,
    dt: Option<f64>,
    params: Option<String>,
    enabled_features: FeatureSet,
}

fn diff_backends_command(arguments: &[String]) -> i32 {
    match parse_diff_options(arguments).and_then(diff_backends) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn parse_diff_options(arguments: &[String]) -> Result<DiffOptions, String> {
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

fn collect_diff_corpus_paths(directory: &str, suffix: &str) -> Result<Vec<String>, String> {
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

fn compare_per_tick_hashes<'a>(
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

fn diff_backends(options: DiffOptions) -> Result<(), String> {
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
        collect_diff_corpus_paths, compare_per_tick_hashes, csv_field, initialize_population,
        parse_backend, parse_diff_options, run, run_file_result, run_results_output,
        run_results_output_with_features, sweep_file_result, BackendSelection, HashMode,
        RunOptions, SweepOptions, SWEEP_BACKEND_CONSTRUCTIONS, VERSION,
    };
    use sembla_runtime::{eval::ParamEnv, state::StateStore};

    fn load(source: &str) -> sembla_ir::ValidatedModel {
        sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
    }

    fn initialized(model: &sembla_ir::ValidatedModel, rows: usize) -> StateStore {
        StateStore::new(model, initialize_population(model, rows)).unwrap()
    }

    #[test]
    fn sweep_constructs_one_backend_for_multiple_draws() {
        use std::sync::atomic::Ordering;

        let out = std::env::temp_dir().join(format!(
            "sembla-sweep-construction-count-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let _ = std::fs::remove_dir_all(&out);
        let model = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/reversible_ctmc.json");
        let options = SweepOptions {
            seed: 71,
            draws: Some(4),
            theta_file: None,
            noise_mode: super::manifest::NoiseMode::Independent,
            ticks: 3,
            population: "32".to_owned(),
            out: out.display().to_string(),
            params: None,
            export_pairs: None,
            timing_json: None,
            backend: BackendSelection::Cpu,
            enabled_features: sembla_ir::FeatureSet::new(),
        };

        SWEEP_BACKEND_CONSTRUCTIONS.store(0, Ordering::SeqCst);
        sweep_file_result(model.to_str().unwrap(), options).unwrap();
        assert_eq!(SWEEP_BACKEND_CONSTRUCTIONS.load(Ordering::SeqCst), 1);
        assert_eq!(
            std::fs::read_dir(&out)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().starts_with("draw_"))
                .count(),
            4
        );
        std::fs::remove_dir_all(out).unwrap();
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
    fn backend_and_differential_run_options_are_strict() {
        assert_eq!(parse_backend("cpu").unwrap(), BackendSelection::Cpu);
        assert_eq!(parse_backend("cuda").unwrap(), BackendSelection::Cuda);
        assert!(parse_backend("auto").is_err());

        let options = parse_diff_options(&[
            "model.json".to_owned(),
            "--population".to_owned(),
            "10".to_owned(),
            "--seed".to_owned(),
            "2".to_owned(),
            "--ticks".to_owned(),
            "3".to_owned(),
            "--dt".to_owned(),
            "0.5".to_owned(),
            "--params".to_owned(),
            "params.json".to_owned(),
        ])
        .unwrap();
        assert_eq!(options.dt, Some(0.5));
        assert_eq!(options.params.as_deref(), Some("params.json"));
        let grouped = parse_diff_options(&[
            "model.json".to_owned(),
            "--enable".to_owned(),
            sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned(),
        ])
        .unwrap();
        assert!(grouped
            .enabled_features
            .contains(sembla_ir::GROUPED_OBSERVATIONS_FEATURE));
        assert!(parse_diff_options(&[
            "model.json".to_owned(),
            "--enable".to_owned(),
            "future-feature".to_owned(),
        ])
        .unwrap_err()
        .contains("unknown feature 'future-feature'"));
        assert!(parse_diff_options(&[
            "--all-examples".to_owned(),
            "--params".to_owned(),
            "params.json".to_owned(),
        ])
        .unwrap_err()
        .contains("single diff-backends input"));
        assert!(parse_diff_options(&[
            "--all-plan-fixtures".to_owned(),
            "--all-examples".to_owned(),
        ])
        .unwrap_err()
        .contains("cannot be combined"));
        assert!(
            parse_diff_options(&["model.json".to_owned(), "--all-plan-fixtures".to_owned(),])
                .unwrap_err()
                .contains("cannot be combined")
        );
        assert!(
            parse_diff_options(&["--all-plan-fixtures".to_owned(), "model.json".to_owned(),])
                .unwrap_err()
                .contains("positional path")
        );
        assert!(parse_diff_options(&[
            "--all-plan-fixtures".to_owned(),
            "--dt".to_owned(),
            "0.5".to_owned(),
        ])
        .unwrap_err()
        .contains("plan envelopes do not support --dt overrides"));
    }

    #[test]
    fn plan_fixture_corpus_is_the_exact_sorted_top_level_and_linked_set() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let plans = root.join("fixtures/plans");
        let linked = plans.join("linked");
        let mut paths = collect_diff_corpus_paths(plans.to_str().unwrap(), ".plan.json").unwrap();
        paths.extend(collect_diff_corpus_paths(linked.to_str().unwrap(), ".plan.json").unwrap());
        paths.sort();
        let relative = paths
            .iter()
            .map(|path| {
                std::path::Path::new(path)
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            relative,
            [
                // DECISIONS §K6 sanctions this first feature-bearing plan fixture.
                "fixtures/plans/grouped_observation.plan.json",
                "fixtures/plans/linked/epidemic_policy.plan.json",
                "fixtures/plans/linked/independent_epidemic_policy.plan.json",
                "fixtures/plans/linked/ping_pong.plan.json",
                "fixtures/plans/linked/regional_response.plan.json",
                "fixtures/plans/linked/solo_population.plan.json",
                "fixtures/plans/linked/two_independent_regions.plan.json",
                "fixtures/plans/linked/two_regions.plan.json",
                "fixtures/plans/linked/wrapped_ping_pong.plan.json",
                "fixtures/plans/observations.plan.json",
                "fixtures/plans/sir.plan.json",
                "fixtures/plans/sir_policy.plan.json",
                "fixtures/plans/two_box.plan.json",
                "fixtures/plans/two_box_plus_sibling.plan.json",
            ]
        );
    }

    #[test]
    fn generic_csv_is_ordered_deterministic_and_conservative() {
        let model = load(include_str!("../../../examples/reversible_ctmc.json"));
        let params = ParamEnv::defaults(&model);
        let mut first_state = initialized(&model, 1000);
        let mut second_state = initialized(&model, 1000);
        let first = run_results_output(&model, &mut first_state, &params, 55, 20).unwrap();
        let second = run_results_output(&model, &mut second_state, &params, 55, 20).unwrap();
        assert_eq!(first.csv, second.csv);
        assert!(first.per_tick_hashes.is_none());
        assert!(second.per_tick_hashes.is_none());
        assert_eq!(
            first.csv.lines().nth(2).unwrap(),
            "tick,count:chain.particle.phase=A,count:chain.particle.phase=B,fired:chain.move_ab,fired:chain.move_ba,deferred_total"
        );
        assert_eq!(first.series.rows.len(), 20);
        for row in &first.series.rows {
            assert_eq!(
                row[0].as_usize("A").unwrap() + row[1].as_usize("B").unwrap(),
                1000
            );
            assert_eq!(row.len(), 5);
        }
        assert_eq!(
            first.series.rows[0][3].as_usize("B to A").unwrap(),
            0,
            "B to A must still have a zero-valued column"
        );
        assert!(first.series.rows.last().unwrap()[1].as_usize("B").unwrap() > 0);
    }

    #[test]
    fn device_generic_enum_counts_preserve_legacy_csv_bytes() {
        let model = load(include_str!("../../../examples/reversible_ctmc.json"));
        let params = ParamEnv::defaults(&model);
        let mut state = initialized(&model, 100);
        let report =
            sembla_runtime::executor::run_tick(&model, &mut state, &params, 55, 0).unwrap();
        let snapshot = state.snapshot();
        let values = snapshot.enum_values("chain", "particle", "phase").unwrap();
        let counts = [
            values.iter().filter(|value| **value == 0).count(),
            values.iter().filter(|value| **value == 1).count(),
        ];

        let mut host = super::RunOutputAccumulator::new(&model, &params, 1).unwrap();
        host.push_tick(&state, 0, report.clone()).unwrap();
        let mut device = super::RunOutputAccumulator::new(&model, &params, 1).unwrap();
        device
            .push_tick_with_enum_counts(&state, 0, report, Some(&counts))
            .unwrap();
        assert_eq!(host.csv, device.csv);
    }

    #[test]
    fn plan_run_is_bitwise_deterministic_twice_in_process() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp = std::env::temp_dir().join(format!(
            "sembla-plan-in-process-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        let plan = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/plans/two_box.plan.json");
        let outputs = [temp.join("first.csv"), temp.join("second.csv")];
        for output in &outputs {
            run_file_result(
                plan.to_str().unwrap(),
                RunOptions {
                    seed: 55,
                    ticks: 40,
                    population: "16".to_owned(),
                    out: Some(output.to_str().unwrap().to_owned()),
                    export_state: None,
                    dt: None,
                    params: None,
                    timing_json: None,
                    backend: BackendSelection::Cpu,
                    enabled_features: sembla_ir::FeatureSet::new(),
                },
            )
            .unwrap();
        }
        for suffix in ["", ".summaries.csv", ".manifest.json"] {
            assert_eq!(
                std::fs::read(format!("{}{suffix}", outputs[0].display())).unwrap(),
                std::fs::read(format!("{}{suffix}", outputs[1].display())).unwrap(),
                "plan run artifact '{suffix}' changed between in-process executions"
            );
        }
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn per_tick_hash_mode_type_enforces_absence() {
        let model = load(include_str!("../../../examples/reversible_ctmc.json"));
        let params = ParamEnv::defaults(&model);
        let mut state = initialized(&model, 10);
        let output = run_results_output_with_features(
            &model,
            &mut state,
            &params,
            55,
            3,
            HashMode::EveryTick,
            &sembla_ir::FeatureSet::new(),
        )
        .unwrap();
        assert_eq!(output.per_tick_hashes.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn per_tick_hash_comparison_detects_the_first_divergent_tick() {
        let cpu = Some(vec![[0; 32], [1; 32], [2; 32]]);
        let cuda = Some(vec![[0; 32], [9; 32], [8; 32]]);
        let error = compare_per_tick_hashes("model.json", &cpu, &cuda).unwrap_err();
        assert!(error.contains("first divergence at tick 1"));
        assert!(error.contains(&format!("cpu={}", super::hex(&[1; 32]))));
        assert!(error.contains(&format!("cuda={}", super::hex(&[9; 32]))));
    }

    #[test]
    fn per_tick_hash_comparison_checks_lengths_before_elements() {
        let cpu = Some(vec![[1; 32], [2; 32]]);
        let cuda = Some(vec![[9; 32]]);
        let error = compare_per_tick_hashes("model.json", &cpu, &cuda).unwrap_err();
        assert_eq!(
            error,
            "model.json: per-tick hash sequence lengths differ: cpu=2 cuda=1"
        );
    }

    #[test]
    fn per_tick_hash_comparison_rejects_absent_sequences() {
        let hashes = Some(vec![[0; 32]]);
        let cpu_error = compare_per_tick_hashes("model.json", &None, &hashes).unwrap_err();
        assert!(cpu_error.contains("internal invariant violation"));
        assert!(cpu_error.contains("cpu per-tick hashes are absent"));

        let cuda_error = compare_per_tick_hashes("model.json", &hashes, &None).unwrap_err();
        assert!(cuda_error.contains("internal invariant violation"));
        assert!(cuda_error.contains("cuda per-tick hashes are absent"));
    }

    #[test]
    fn generated_csv_headers_are_escaped() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("has,comma"), "\"has,comma\"");
        assert_eq!(csv_field("has\"quote"), "\"has\"\"quote\"");
        assert_eq!(csv_field("has\nnewline"), "\"has\nnewline\"");
    }
}
