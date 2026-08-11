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

mod compare;
mod diff_backends;
mod inspect;
mod manifest;
mod run;
mod shared;
mod sweep;
mod synth;
mod verify;

use compare::*;
use diff_backends::*;
use inspect::*;
use run::*;
use shared::*;
use sweep::*;
use synth::*;
use verify::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = "usage: sembla --version | sembla validate <model-or-plan.json> | sembla plan-hash <plan-envelope.json> | sembla state-hash <file.state> | sembla bundle-verify <bundle-dir> | sembla diff-ir <a.json> <b.json> | sembla synth-pop --persons N --employers E --initial-infected I --seed S --out pop.bin | sembla synth-state --model model-or-plan.json --slots N --areas K --present-fraction F --streams birth:B,overseas:O,internal:I --seed S --out state.artifact (benchmark/test tooling for the documented demographic column roles; emits state.artifact.model.json) | sembla run <model-or-plan.json> --seed N --ticks K --population N|pop.bin|file.state [--backend cpu|cuda] [--out results.csv] [--export-state final.state] [--dt D] [--params file.json] [--timing-json timing.json] [--enable grouped-observations] | sembla sweep <model-or-plan.json> --population N|pop.bin|file.state --seed S (--draws K | --theta-file file.json) --ticks T --out dir [--backend cpu|cuda] [--draw-workers N] [--noise crn|independent] [--params file.json] [--export-pairs pairs.csv] [--timing-json timing.json] [--enable grouped-observations] | sembla compare <model-or-plan.json> <model-or-plan.json> --population pop.bin|file.state --seed N --ticks K --out compare.csv [--backend cpu|cuda] | sembla compare <model-or-plan.json> --population pop.bin|file.state --seed N --ticks K --params-a a.json --params-b b.json --out compare.csv [--backend cpu|cuda] [--enable grouped-observations] | sembla verify-run <manifest.json> <model-or-plan.json> --population N|pop.bin|file.state [--params file.json] [--draw K] | sembla diff-backends <model-or-plan.json> --population N|pop.bin|file.state --seed N --ticks K [--dt D] [--params file.json] [--enable grouped-observations] | sembla diff-backends --all-examples [--population N] [--seed N] [--ticks K] [--dt D] | sembla diff-backends --all-plan-fixtures [--population N] [--seed N] [--ticks K]";
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

#[cfg(test)]
mod tests;
