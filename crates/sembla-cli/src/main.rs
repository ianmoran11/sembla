use sembla_ir::AttrType;
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::executor;
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = "usage: sembla --version | sembla validate <path> | sembla run <model.json> --seed N --ticks K --population N";

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

#[derive(Clone, Copy)]
struct RunOptions {
    seed: u64,
    ticks: u32,
    population: usize,
}

fn parse_run_options(flags: &[String]) -> Result<RunOptions, String> {
    let mut seed = None;
    let mut ticks = None;
    let mut population = None;
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index].as_str();
        let value = flags
            .get(index + 1)
            .ok_or_else(|| format!("missing value for '{flag}'"))?;
        match flag {
            "--seed" => set_once(&mut seed, parse_number(value, flag)?, flag)?,
            "--ticks" => set_once(&mut ticks, parse_number(value, flag)?, flag)?,
            "--population" => set_once(&mut population, parse_number(value, flag)?, flag)?,
            _ => return Err(format!("unknown run flag '{flag}'")),
        }
        index += 2;
    }
    Ok(RunOptions {
        seed: seed.ok_or_else(|| "missing required flag '--seed'".to_owned())?,
        ticks: ticks.ok_or_else(|| "missing required flag '--ticks'".to_owned())?,
        population: population.ok_or_else(|| "missing required flag '--population'".to_owned())?,
    })
}

fn parse_number<T: std::str::FromStr>(value: &str, flag: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid numeric value '{value}' for '{flag}'"))
}

fn set_once<T>(slot: &mut Option<T>, value: T, flag: &str) -> Result<(), String> {
    if slot.is_some() {
        Err(format!("duplicate run flag '{flag}'"))
    } else {
        *slot = Some(value);
        Ok(())
    }
}

fn read_validated(path: &str) -> Result<sembla_ir::ValidatedModel, String> {
    let source = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let model = sembla_ir::parse_json(&source).map_err(|error| format!("{path}: {error}"))?;
    sembla_ir::validate(model).map_err(|error| format!("{path}: {error}"))
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

fn run_file(path: &str, options: RunOptions) -> i32 {
    let model = match read_validated(path) {
        Ok(model) => model,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let initial = initialize_population(&model, options.population);
    let mut state = match StateStore::new(&model, initial) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("{path}: {error}");
            return 1;
        }
    };
    let params = ParamEnv::defaults(&model);
    let report = match executor::run(&model, &mut state, &params, options.seed, options.ticks) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{path}: {error}");
            return 1;
        }
    };
    for tick in report.ticks {
        for (rule_id, fired) in tick.fired {
            println!("tick={} rule_id={} fired={}", tick.tick, rule_id, fired);
        }
    }
    0
}

fn initialize_population(model: &sembla_ir::ValidatedModel, population: usize) -> Vec<TableInit> {
    let mut initial = Vec::new();
    for model_box in &model.model().boxes {
        for table in &model_box.tables {
            let columns = table
                .attrs
                .iter()
                .map(|attr| {
                    let data = match attr.ty {
                        AttrType::Real => ColumnData::Real(vec![0.0; population]),
                        AttrType::Int => ColumnData::Int(vec![0; population]),
                        AttrType::Enum { .. } => ColumnData::Enum(vec![0; population]),
                        AttrType::Ref { .. } => ColumnData::Ref(vec![0; population]),
                    };
                    ColumnInit::new(&attr.name, data)
                })
                .collect();
            initial.push(TableInit::new(
                &model_box.name,
                &table.name,
                population,
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
