use std::path::Path;

use sembla_ir::{AttrType, ParamType, ParamValue};
use sembla_runtime::eval::{ParamEnv, ParamOverride};
use sembla_runtime::executor;
use sembla_runtime::population::SyntheticPopulation;
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};
use sha2::{Digest, Sha256};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = "usage: sembla --version | sembla validate <path> | sembla synth-pop --persons N --employers E --initial-infected I --seed S --out pop.bin | sembla run <model.json> --seed N --ticks K --population N|pop.bin [--out results.csv] [--dt D] [--params file.json]";

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
        Err(_) => SyntheticPopulation::read(&options.population)
            .map_err(|error| error.to_string())?
            .sir_table_initializers(),
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

fn resolve_params(
    model: &sembla_ir::ValidatedModel,
    path: Option<&str>,
) -> Result<ParamEnv, String> {
    let Some(path) = path else {
        return Ok(ParamEnv::defaults(model));
    };
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
    ParamEnv::resolve(model, &overrides).map_err(|error| format!("{path}: {error}"))
}

fn run_results(
    model: &sembla_ir::ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
    out: &str,
) -> Result<(), String> {
    validate_sir_shape(model)?;
    let mut csv = String::new();
    csv.push_str("# params=");
    csv.push_str(&canonical_params(params)?);
    csv.push('\n');
    csv.push_str(&format!("# dt={}\n", model.model().dt));
    csv.push_str("tick,S,I,R,fired_infect,fired_recover,deferred_total\n");
    for tick in 0..ticks {
        let report = executor::run_tick(model, state, params, seed, tick)
            .map_err(|error| format!("tick {tick}: {error}"))?;
        let counts = sir_counts(state)?;
        let fired_infect = report.fired.first().map_or(0, |entry| entry.1);
        let fired_recover = report.fired.get(1).map_or(0, |entry| entry.1);
        let deferred_total: usize = report
            .deferred_per_resource_table
            .iter()
            .map(|(_, count)| count)
            .sum();
        csv.push_str(&format!(
            "{tick},{},{},{},{fired_infect},{fired_recover},{deferred_total}\n",
            counts[0], counts[1], counts[2]
        ));
    }
    std::fs::write(out, csv.as_bytes()).map_err(|error| format!("{out}: {error}"))?;
    let results_hash = Sha256::digest(csv.as_bytes());
    println!(
        "results_sha256={} final_state_sha256={}",
        hex(&results_hash),
        hex(&state.state_hash())
    );
    Ok(())
}

fn validate_sir_shape(model: &sembla_ir::ValidatedModel) -> Result<(), String> {
    let model_box = model
        .model()
        .boxes
        .iter()
        .find(|model_box| model_box.name == "sir")
        .ok_or_else(|| "--out CSV currently requires the SIR box named 'sir'".to_owned())?;
    let person = model_box
        .tables
        .iter()
        .find(|table| table.name == "person")
        .ok_or_else(|| "--out CSV requires table 'sir.person'".to_owned())?;
    let health = person
        .attrs
        .iter()
        .find(|attr| attr.name == "health")
        .ok_or_else(|| "--out CSV requires column 'sir.person.health'".to_owned())?;
    match &health.ty {
        AttrType::Enum { variants } if variants == &["S", "I", "R"] => Ok(()),
        _ => Err("--out CSV requires health Enum variants [S,I,R]".to_owned()),
    }
}

fn sir_counts(state: &StateStore) -> Result<[usize; 3], String> {
    let snapshot = state.snapshot();
    let health = snapshot
        .enum_values("sir", "person", "health")
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
    use super::{run, VERSION};

    #[test]
    fn version_matches_library_versions() {
        assert_eq!(VERSION, sembla_ir::VERSION);
        assert_eq!(VERSION, sembla_runtime::VERSION);
    }

    #[test]
    fn invalid_usage_is_nonzero() {
        assert_eq!(run(&[]), 2);
    }
}
