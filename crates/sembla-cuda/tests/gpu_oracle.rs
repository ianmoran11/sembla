#![cfg(feature = "cuda")]

use sembla_cuda::{CudaBackend, HashMode};
use sembla_ir::ValidatedModel;
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::executor::run_tick;
use sembla_runtime::population::SyntheticPopulation;
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

fn load(example: &str) -> ValidatedModel {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../examples/{example}"));
    let source = std::fs::read_to_string(path).unwrap();
    sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
}

fn cpu_hashes(
    model: &ValidatedModel,
    initial: Vec<TableInit>,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
) -> Vec<[u8; 32]> {
    let mut state = StateStore::new(model, initial).unwrap();
    let mut hashes = Vec::with_capacity(ticks as usize);
    for tick in 0..ticks {
        run_tick(model, &mut state, params, seed, tick).unwrap();
        hashes.push(state.state_hash());
    }
    hashes
}

fn gpu_hashes(
    model: &ValidatedModel,
    initial: Vec<TableInit>,
    params: &ParamEnv,
    seed: u64,
    ticks: u32,
) -> Vec<[u8; 32]> {
    CudaBackend::new(model, initial, params, seed, HashMode::EveryTick)
        .expect("CUDA device, driver, and NVRTC are required")
        .run(ticks)
        .unwrap()
        .per_tick_state_hashes
}

fn sir_case() -> (ValidatedModel, Vec<TableInit>, ParamEnv, u64, u32) {
    let model = load("sir.json");
    let population = SyntheticPopulation::generate(100_000, 500, 100, 0x5eed).unwrap();
    let initial = population.sir_table_initializers();
    let params = ParamEnv::defaults(&model);
    (model, initial, params, 77, 200)
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn sir_100k_200_ticks_matches_cpu_per_tick() {
    let (model, initial, params, seed, ticks) = sir_case();
    let expected = cpu_hashes(&model, initial.clone(), &params, seed, ticks);
    let actual = gpu_hashes(&model, initial, &params, seed, ticks);
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn two_box_sir_policy_matches_cpu_per_tick() {
    let model = load("sir_policy.json");
    let population = SyntheticPopulation::generate(100_000, 500, 100, 12).unwrap();
    let initial = population.sir_policy_table_initializers();
    let params = ParamEnv::defaults(&model);
    let expected = cpu_hashes(&model, initial.clone(), &params, 55, 200);
    let actual = gpu_hashes(&model, initial, &params, 55, 200);
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn canonical_two_state_model_matches_cpu_per_tick() {
    let model = load("two_state.json");
    let initial = vec![TableInit::new(
        "population",
        "Person",
        1_000,
        vec![ColumnInit::new("mood", ColumnData::Enum(vec![0; 1_000]))],
    )];
    let params = ParamEnv::defaults(&model);
    let expected = cpu_hashes(&model, initial.clone(), &params, 42, 200);
    let actual = gpu_hashes(&model, initial, &params, 42, 200);
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn level_a_same_gpu_run_twice_has_byte_identical_hashes() {
    let (model, initial, params, seed, ticks) = sir_case();
    let first = gpu_hashes(&model, initial.clone(), &params, seed, ticks);
    let repeat = gpu_hashes(&model, initial, &params, seed, ticks);
    assert_eq!(first, repeat);
}
