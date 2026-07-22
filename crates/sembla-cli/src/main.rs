use std::path::{Path, PathBuf};

use sembla_cuda::{CudaBackend, HashMode};
use sembla_ir::{AttrType, ParamType, ParamValue};
use sembla_runtime::eval::{ParamEnv, ParamOverride};
use sembla_runtime::executor::{self, ObservationValue, SummaryValue};
use sembla_runtime::population::SyntheticPopulation;
use sembla_runtime::prior::sample_parameters_for_draw;
use sembla_runtime::rng::derive_sweep_replica_seed;
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};
use sha2::{Digest, Sha256};

mod manifest;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = "usage: sembla --version | sembla validate <model-or-plan.json> | sembla plan-hash <plan-envelope.json> | sembla bundle-verify <bundle-dir> | sembla diff-ir <a.json> <b.json> | sembla synth-pop --persons N --employers E --initial-infected I --seed S --out pop.bin | sembla run <model-or-plan.json> --seed N --ticks K --population N|pop.bin [--backend cpu|cuda] [--out results.csv] [--dt D] [--params file.json] | sembla sweep <model-or-plan.json> --population N|pop.bin --seed S (--draws K | --theta-file file.json) --ticks T --out dir [--backend cpu|cuda] [--noise crn|independent] [--params file.json] [--export-pairs pairs.csv] | sembla compare <model-or-plan.json> <model-or-plan.json> --population pop.bin --seed N --ticks K --out compare.csv [--backend cpu|cuda] | sembla compare <model-or-plan.json> --population pop.bin --seed N --ticks K --params-a a.json --params-b b.json --out compare.csv [--backend cpu|cuda] | sembla verify-run <manifest.json> <model-or-plan.json> --population N|pop.bin [--params file.json] [--draw K] | sembla diff-backends <model-or-plan.json> --population N|pop.bin --seed N --ticks K [--dt D] [--params file.json] | sembla diff-backends --all-examples [--population N] [--seed N] [--ticks K] [--dt D] | sembla diff-backends --all-plan-fixtures [--population N] [--seed N] [--ticks K]";
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
    dt: Option<f64>,
    params: Option<String>,
    backend: BackendSelection,
}

fn parse_run_options(flags: &[String]) -> Result<RunOptions, String> {
    let mut seed = None;
    let mut ticks = None;
    let mut population = None;
    let mut out = None;
    let mut dt = None;
    let mut params = None;
    let mut backend = None;
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
            "--backend" => set_once(&mut backend, parse_backend(value)?, flag)?,
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
        backend: backend.unwrap_or_default(),
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
    backend: BackendSelection,
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
    let mut backend = None;
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
            "--backend" => set_once(&mut backend, parse_backend(value)?, flag)?,
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
        backend: backend.unwrap_or_default(),
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
        backend: backend.unwrap_or_default(),
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
    read_executable_input(path, options.dt)
}

fn read_executable_input(path: &str, dt: Option<f64>) -> Result<RunInput, String> {
    let (source, input) = read_input(path)?;
    match input {
        sembla_ir::ParsedInput::LegacyModel(mut model) => {
            if let Some(dt) = dt {
                model.dt = dt;
            }
            let model = sembla_ir::validate(model).map_err(|error| format!("{path}: {error}"))?;
            Ok(RunInput { model, plan: None })
        }
        sembla_ir::ParsedInput::Plan(plan) => {
            let validated =
                sembla_ir::validate_plan(&plan).map_err(|error| format!("{path}: {error}"))?;
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
    let RunInput { model, plan } = read_run_input(path, &options)?;
    let (population_source, population_sha256) =
        manifest::population_identity(&options.population)?;
    let initial = match options.population.parse::<usize>() {
        Ok(population) => initialize_population(&model, population),
        Err(_) => initializers_from_population(
            &model,
            &SyntheticPopulation::read(&options.population).map_err(|error| error.to_string())?,
        )?,
    };
    let params = resolve_params(&model, options.params.as_deref())?;
    if options.out.is_none() && options.backend == BackendSelection::Cpu {
        let mut state =
            StateStore::new(&model, initial).map_err(|error| format!("{path}: {error}"))?;
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
        return Ok(());
    }
    let execution = execute_backend_output(
        &model,
        initial,
        &params,
        options.seed,
        options.ticks,
        options.backend,
    )?;

    if let Some(out) = options.out.as_deref() {
        std::fs::write(out, execution.output.csv.as_bytes())
            .map_err(|error| format!("{out}: {error}"))?;
        let summaries = summaries_path(out);
        std::fs::write(&summaries, execution.output.summaries_csv.as_bytes())
            .map_err(|error| format!("{}: {error}", summaries.display()))?;
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
        run_manifest.ir_hash = Some(manifest::canonical_ir_hash(&model)?);
        run_manifest.backend_identity = Some(execution.identity);
        run_manifest.resolved_theta = manifest::resolved_theta(&params);
        run_manifest.results_sha256 = Some(hashes.results_sha256);
        run_manifest.final_state_sha256 = Some(hashes.final_state_sha256);
        run_manifest.observation_sha256 = Some(hashes.observation_sha256);
        if let Some(plan) = &plan {
            let (plan_identity, linked_source) = manifest::plan_identity_tuples(plan)?;
            run_manifest.plan = Some(plan_identity);
            run_manifest.linked_source = linked_source;
        }
        manifest::write(&manifest::sidecar_path(out), &run_manifest)
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

fn sweep_file_result(path: &str, options: SweepOptions) -> Result<(), String> {
    let RunInput { model, plan } = read_executable_input(path, None)?;
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
    let population = options.population.parse::<usize>().ok();
    let population_file = if population.is_none() {
        Some(SyntheticPopulation::read(&options.population).map_err(|error| error.to_string())?)
    } else {
        None
    };
    let out = Path::new(&options.out);
    std::fs::create_dir_all(out).map_err(|error| format!("{}: {error}", out.display()))?;
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

        let initial = match (&population, &population_file) {
            (Some(row_count), None) => initialize_population(&model, *row_count),
            (None, Some(population)) => initializers_from_population(&model, population)?,
            _ => return Err("invalid sweep population source".to_owned()),
        };
        let execution_seed = match options.noise_mode {
            manifest::NoiseMode::Crn => options.seed,
            manifest::NoiseMode::Independent => derive_sweep_replica_seed(options.seed, draw),
        };
        let execution = execute_backend_output(
            &model,
            initial,
            &params,
            execution_seed,
            options.ticks,
            options.backend,
        )?;
        if draw == 0 {
            run_manifest.backend_identity = Some(execution.identity.clone());
        } else if run_manifest.backend_identity.as_ref() != Some(&execution.identity) {
            return Err("backend device identity changed during sweep".to_owned());
        }
        let output = execution.output;
        if let Some(columns) = &reported_columns {
            if columns != &output.series.columns {
                return Err(format!(
                    "draw {draw}: reported column schema changed across draws"
                ));
            }
        } else {
            reported_columns = Some(output.series.columns.clone());
        }
        if let Some(csv) = &mut pairs_csv {
            append_pairs_row(
                csv,
                draw,
                &params,
                &parameter_columns,
                &output.summaries,
                &summary_columns,
            )?;
        }
        let hashes = execution_hashes(&output, &execution.state);
        run_manifest.executions.push(manifest::ManifestExecution {
            k: draw,
            seed: Some(execution_seed),
            scenario: None,
            model: None,
            ir_hash: None,
            dt: None,
            resolved_theta: manifest::resolved_theta(&params),
            results_sha256: hashes.results_sha256,
            final_state_sha256: hashes.final_state_sha256,
            observation_sha256: Some(hashes.observation_sha256),
        });
        let draw_path = out.join(format!("draw_{draw}.csv"));
        std::fs::write(&draw_path, output.csv.as_bytes())
            .map_err(|error| format!("{}: {error}", draw_path.display()))?;
        if options.export_pairs.is_some() {
            let draw_summaries = PathBuf::from(format!("{}.summaries.csv", draw_path.display()));
            std::fs::write(&draw_summaries, output.summaries_csv.as_bytes())
                .map_err(|error| format!("{}: {error}", draw_summaries.display()))?;
        }
        all_series.push(output.series.rows);
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
struct RunOutput {
    csv: String,
    series: ReportedSeries,
    summaries: Vec<SummaryValue>,
    summaries_csv: String,
    per_tick_hashes: Vec<[u8; 32]>,
}

fn summaries_path(output: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{output}.summaries.csv"))
}

struct BackendRunOutput {
    output: RunOutput,
    state: StateStore,
    identity: manifest::BackendIdentity,
    per_tick_hashes: Vec<[u8; 32]>,
    elapsed: std::time::Duration,
}

fn execute_backend_output(
    model: &sembla_ir::ValidatedModel,
    initial: Vec<TableInit>,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
    backend: BackendSelection,
) -> Result<BackendRunOutput, String> {
    match backend {
        BackendSelection::Cpu => {
            let mut state = StateStore::new(model, initial).map_err(|error| error.to_string())?;
            let started = std::time::Instant::now();
            let output = run_results_output(model, &mut state, params, seed, ticks)?;
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
        BackendSelection::Cuda => run_results_output_cuda(model, initial, params, seed, ticks),
    }
}

fn execution_hashes(output: &RunOutput, state: &StateStore) -> ExecutionHashes {
    ExecutionHashes {
        results_sha256: hex(&Sha256::digest(output.csv.as_bytes())),
        final_state_sha256: hex(&state.state_hash()),
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

fn run_results_output(
    model: &sembla_ir::ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
) -> Result<RunOutput, String> {
    let has_views = model
        .model()
        .boxes
        .iter()
        .any(|model_box| !model_box.views.is_empty());
    let enums = (!has_views).then(|| generic_enum_descriptors(model));
    let firings = generic_firing_descriptors(model);
    let mut csv = String::new();
    let mut rows = Vec::with_capacity(ticks as usize);
    let mut tick_reports = Vec::with_capacity(ticks as usize);
    let mut per_tick_hashes = Vec::with_capacity(ticks as usize);
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

    for tick in 0..ticks {
        let report = executor::run_tick(model, state, params, seed, tick)
            .map_err(|error| format!("tick {tick}: {error}"))?;
        let mut row = Vec::with_capacity(headers.len() - 1);
        if has_views {
            row.extend(
                report
                    .views
                    .iter()
                    .map(|view| ReportedValue::from(view.value)),
            );
        } else {
            let snapshot = state.snapshot();
            for descriptor in enums.as_deref().unwrap_or_default() {
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
        for descriptor in &firings {
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
        csv.push_str(&tick.to_string());
        for value in &row {
            csv.push(',');
            csv.push_str(&value.csv());
        }
        csv.push('\n');
        rows.push(row);
        tick_reports.push(report);
        per_tick_hashes.push(state.state_hash());
    }
    let summaries = executor::summarize(model, &tick_reports).map_err(|error| error.to_string())?;
    let summaries_csv = summaries_csv(&summaries);
    Ok(RunOutput {
        csv,
        series: ReportedSeries {
            columns: headers.into_iter().skip(1).collect(),
            rows,
        },
        summaries,
        summaries_csv,
        per_tick_hashes,
    })
}

fn run_results_output_cuda(
    model: &sembla_ir::ValidatedModel,
    initial: Vec<TableInit>,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
) -> Result<BackendRunOutput, String> {
    let mut state = StateStore::new(model, initial.clone()).map_err(|error| error.to_string())?;
    let mut backend = CudaBackend::new(model, initial, params, seed, HashMode::EveryTick)
        .map_err(|error| error.to_string())?;
    let device = backend.device_identity().clone();
    let identity =
        manifest::BackendIdentity::cuda_native_f64(device.gpu_model, device.driver_version);
    let mut hashes = Vec::with_capacity(ticks as usize);
    let has_views = model
        .model()
        .boxes
        .iter()
        .any(|model_box| !model_box.views.is_empty());
    let enums = (!has_views).then(|| generic_enum_descriptors(model));
    let firings = generic_firing_descriptors(model);
    let mut csv = String::new();
    let mut rows = Vec::with_capacity(ticks as usize);
    let mut tick_reports = Vec::with_capacity(ticks as usize);
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

    let started = std::time::Instant::now();
    for tick in 0..ticks {
        let observation = backend
            .run_tick_observed()
            .map_err(|error| format!("tick {tick}: {error}"))?;
        state = observation.state;
        hashes.push(state.state_hash());
        let views = executor::observe_views(model, &state, params)
            .map_err(|error| format!("tick {tick}: {error}"))?;
        let fired = model
            .transitions()
            .iter()
            .map(|transition| {
                let count = observation
                    .fired_per_box
                    .iter()
                    .flat_map(|(_, rules)| rules)
                    .find(|(rule_id, _)| *rule_id == transition.rule_id)
                    .map_or(0, |(_, count)| *count);
                (transition.rule_id, count)
            })
            .collect();
        let report = executor::TickReport {
            tick,
            views,
            fired,
            fired_per_box: observation.fired_per_box,
            deferred_per_resource_table: observation.deferred_per_resource_table,
            aggregate_builds: 0,
        };
        let mut row = Vec::with_capacity(headers.len() - 1);
        if has_views {
            row.extend(
                report
                    .views
                    .iter()
                    .map(|view| ReportedValue::from(view.value)),
            );
        } else {
            let snapshot = state.snapshot();
            for descriptor in enums.as_deref().unwrap_or_default() {
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
        for descriptor in &firings {
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
        csv.push_str(&tick.to_string());
        for value in &row {
            csv.push(',');
            csv.push_str(&value.csv());
        }
        csv.push('\n');
        rows.push(row);
        tick_reports.push(report);
    }
    let elapsed = started.elapsed();
    let summaries = executor::summarize(model, &tick_reports).map_err(|error| error.to_string())?;
    let summaries_csv = summaries_csv(&summaries);
    Ok(BackendRunOutput {
        output: RunOutput {
            csv,
            series: ReportedSeries {
                columns: headers.into_iter().skip(1).collect(),
                rows,
            },
            summaries,
            summaries_csv,
            per_tick_hashes: hashes.clone(),
        },
        state,
        identity,
        per_tick_hashes: hashes,
        elapsed,
    })
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
            let model =
                sembla_ir::validate(raw_model).map_err(|error| format!("{model_path}: {error}"))?;
            (model, None)
        }
        sembla_ir::ParsedInput::Plan(plan) => {
            let validated = sembla_ir::validate_plan(&plan)
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
            let execution = execute_backend_output(
                &model,
                initialized_tables(&model, &options.population)?,
                &params,
                recorded.seed,
                recorded.ticks,
                backend,
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
                let replay = execute_backend_output(
                    &model,
                    initialized_tables(&model, &options.population)?,
                    &params,
                    execution.seed.unwrap_or(recorded.seed),
                    recorded.ticks,
                    backend,
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
            }
            finish_verification(differences, executions.len())
        }
        manifest::ManifestKind::Compare => unreachable!("handled above"),
    }
}

fn initialized_tables(
    model: &sembla_ir::ValidatedModel,
    population_spec: &str,
) -> Result<Vec<TableInit>, String> {
    let initial = match population_spec.parse::<usize>() {
        Ok(population) => initialize_population(model, population),
        Err(_) => initializers_from_population(
            model,
            &SyntheticPopulation::read(population_spec).map_err(|error| error.to_string())?,
        )?,
    };
    Ok(initial)
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
    ticks: Vec<CompareTick>,
    hashes: ExecutionHashes,
    identity: manifest::BackendIdentity,
}

fn compare_result(options: CompareOptions) -> Result<(), String> {
    let path_a = &options.models[0];
    let path_b = options.models.get(1).unwrap_or(path_a);
    let input_a = read_executable_input(path_a, None)?;
    let input_b = read_executable_input(path_b, None)?;
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
    let model_a = input_a.model;
    let model_b = input_b.model;
    let params_a = resolve_params(&model_a, options.params_a.as_deref())?;
    let params_b = resolve_params(&model_b, options.params_b.as_deref())?;
    let population =
        SyntheticPopulation::read(&options.population).map_err(|error| error.to_string())?;
    let arm_a = compare_arm(
        &model_a,
        &params_a,
        &population,
        options.seed,
        options.ticks,
        options.backend,
    )?;
    let arm_b = compare_arm(
        &model_b,
        &params_b,
        &population,
        options.seed,
        options.ticks,
        options.backend,
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
    csv.push_str("tick,S_a,I_a,R_a,S_b,I_b,R_b,dS,dI,dR,fired_infect_a,fired_recover_a,deferred_a,fired_infect_b,fired_recover_b,deferred_b\n");
    for (tick, (tick_a, tick_b)) in arm_a.ticks.iter().zip(&arm_b.ticks).enumerate() {
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
        });
    }
    manifest::write(&manifest::sidecar_path(&options.out), &run_manifest)?;
    println!("compare_sha256={compare_sha256}");
    Ok(())
}

fn compare_arm(
    model: &sembla_ir::ValidatedModel,
    params: &ParamEnv,
    population: &SyntheticPopulation,
    seed: u64,
    ticks: u32,
    backend: BackendSelection,
) -> Result<CompareArmOutcome, String> {
    let initial = initializers_from_population(model, population)?;
    let execution = execute_backend_output(model, initial, params, seed, ticks, backend)?;
    let output = execution.output;
    let column = |name: &str| {
        output
            .series
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
    let ticks = output
        .series
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
        .collect::<Result<Vec<_>, String>>()?;
    Ok(CompareArmOutcome {
        ticks,
        hashes: execution_hashes(&output, &execution.state),
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

fn diff_backends(options: DiffOptions) -> Result<(), String> {
    for path in &options.models {
        let run_options = RunOptions {
            seed: options.seed,
            ticks: options.ticks,
            population: options.population.clone(),
            out: None,
            dt: options.dt,
            params: options.params.clone(),
            backend: BackendSelection::Cpu,
        };
        let model = read_run_input(path, &run_options)?.model;
        let params = resolve_params(&model, options.params.as_deref())?;
        let initial = match options.population.parse::<usize>() {
            Ok(population) => initialize_population(&model, population),
            Err(_) => initializers_from_population(
                &model,
                &SyntheticPopulation::read(&options.population)
                    .map_err(|error| error.to_string())?,
            )?,
        };
        let cpu = execute_backend_output(
            &model,
            initial.clone(),
            &params,
            options.seed,
            options.ticks,
            BackendSelection::Cpu,
        )?;
        let cuda = execute_backend_output(
            &model,
            initial,
            &params,
            options.seed,
            options.ticks,
            BackendSelection::Cuda,
        )?;
        if let Some((tick, (cpu_hash, cuda_hash))) = cpu
            .per_tick_hashes
            .iter()
            .zip(&cuda.per_tick_hashes)
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
        if cpu.per_tick_hashes.len() != cuda.per_tick_hashes.len() {
            return Err(format!("{path}: per-tick hash sequence lengths differ"));
        }
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
                    hex(&cpu.per_tick_hashes[tick]),
                    hex(&cuda.per_tick_hashes[tick])
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
        collect_diff_corpus_paths, csv_field, initialize_population, parse_backend,
        parse_diff_options, run, run_file_result, run_results_output, BackendSelection, RunOptions,
        VERSION,
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
                    dt: None,
                    params: None,
                    backend: BackendSelection::Cpu,
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
    fn generated_csv_headers_are_escaped() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("has,comma"), "\"has,comma\"");
        assert_eq!(csv_field("has\"quote"), "\"has\"\"quote\"");
        assert_eq!(csv_field("has\nnewline"), "\"has\nnewline\"");
    }
}
