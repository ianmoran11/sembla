use std::path::Path;
use std::time::Instant;

use sembla_ir::{ParamValue, ValidatedModel};
use sembla_runtime::eval::{ParamEnv, ParamOverride};
use sembla_runtime::executor::run_tick;
use sembla_runtime::population::SyntheticPopulation;
use sembla_runtime::state::StateStore;
use sha2::{Digest, Sha256};

fn sir_model() -> ValidatedModel {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sir.json");
    let source = std::fs::read_to_string(path).unwrap();
    sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
}

fn params(model: &ValidatedModel, beta: f64, gamma: f64) -> ParamEnv {
    ParamEnv::resolve(
        model,
        &[
            ParamOverride::new("beta", ParamValue::Real { value: beta }),
            ParamOverride::new("gamma", ParamValue::Real { value: gamma }),
        ],
    )
    .unwrap()
}

fn counts(state: &StateStore) -> [usize; 3] {
    let mut counts = [0; 3];
    for value in state
        .snapshot()
        .enum_values("sir", "person", "health")
        .unwrap()
    {
        counts[*value as usize] += 1;
    }
    counts
}

fn simulate(
    population: &SyntheticPopulation,
    run_seed: u64,
    ticks: u32,
    beta: f64,
    gamma: f64,
) -> (Vec<[usize; 3]>, Vec<u8>, [u8; 32]) {
    let model = sir_model();
    let theta = params(&model, beta, gamma);
    let mut state = StateStore::new(&model, population.sir_table_initializers()).unwrap();
    let mut rows = Vec::with_capacity(ticks as usize);
    let mut results = format!(
        "# params={{\"beta\":{beta},\"gamma\":{gamma}}}\n# dt=0.25\ntick,S,I,R,fired_infect,fired_recover,deferred_total\n"
    )
    .into_bytes();
    for tick in 0..ticks {
        let report = run_tick(&model, &mut state, &theta, run_seed, tick).unwrap();
        let row = counts(&state);
        let infect = report.fired[0].1;
        let recover = report.fired[1].1;
        results.extend_from_slice(
            format!(
                "{tick},{},{},{},{infect},{recover},0\n",
                row[0], row[1], row[2]
            )
            .as_bytes(),
        );
        rows.push(row);
    }
    (rows, results, state.state_hash())
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[test]
fn hundred_thousand_end_to_end_run_contract_is_deterministic() {
    let population = SyntheticPopulation::generate(100_000, 500, 100, 0x5eed).unwrap();
    let first = simulate(&population, 77, 100, 0.8, 0.1);
    let repeat = simulate(&population, 77, 100, 0.8, 0.1);
    let temp = std::env::temp_dir().join(format!("sembla-sir-{}", std::process::id()));
    std::fs::create_dir_all(&temp).unwrap();
    let first_file = temp.join("first.csv");
    let repeat_file = temp.join("repeat.csv");
    std::fs::write(&first_file, &first.1).unwrap();
    std::fs::write(&repeat_file, &repeat.1).unwrap();
    assert_eq!(
        digest(&std::fs::read(&first_file).unwrap()),
        digest(&std::fs::read(&repeat_file).unwrap()),
        "results-file digest must repeat"
    );
    assert_eq!(first.2, repeat.2, "final state digest must repeat");

    let different_seed = simulate(&population, 78, 100, 0.8, 0.1);
    assert_ne!(digest(&first.1), digest(&different_seed.1));
    assert_ne!(first.2, different_seed.2);

    let different_theta = simulate(&population, 77, 100, 0.65, 0.1);
    assert_ne!(digest(&first.1), digest(&different_theta.1));
    assert_ne!(first.2, different_theta.2);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn epidemic_sanity_and_zero_beta() {
    // Frequency-dependent SIR has R0 = beta/gamma. Here R0 = 0.8/0.1 = 8,
    // clearly above one, and dt=0.25 resolves both hazards conservatively.
    let population = SyntheticPopulation::generate(100_000, 100, 100, 91).unwrap();
    let (series, _, _) = simulate(&population, 44, 300, 0.8, 0.1);
    assert!(series.windows(2).all(|pair| pair[1][0] <= pair[0][0]));
    assert!(series.windows(2).all(|pair| pair[1][2] >= pair[0][2]));
    let initial_i = 100;
    let peak = series.iter().map(|row| row[1]).max().unwrap();
    assert!(peak > initial_i, "I must rise above I0");
    assert!(
        series.last().unwrap()[1] < peak,
        "I must fall after its peak"
    );
    let final_attack_rate = 1.0 - series.last().unwrap()[0] as f64 / 100_000.0;
    assert!(
        final_attack_rate > 0.5,
        "final attack rate was {final_attack_rate}"
    );

    let (zero_beta, _, _) = simulate(&population, 44, 100, 0.0, 0.1);
    assert!(zero_beta.iter().all(|row| row[1] <= initial_i));
    assert!(zero_beta.windows(2).all(|pair| pair[1][0] == pair[0][0]));
}

#[test]
fn million_person_lumping_builds_each_group_accumulator_once() {
    let model = sir_model();
    let theta = params(&model, 0.8, 0.1);
    let population = SyntheticPopulation::generate(1_000_000, 50_000, 100, 123).unwrap();
    let mut state = StateStore::new(&model, population.sir_table_initializers()).unwrap();
    let report = run_tick(&model, &mut state, &theta, 456, 0).unwrap();
    // The infection hazard has two structurally distinct group-by accumulators:
    // infected count and total workplace size. Each is built once, rather than
    // once for each of the one million querying rows.
    assert_eq!(report.aggregate_builds, 2);
}

#[test]
#[ignore = "release-only 1M-agent regression tripwire; run with --release --ignored --nocapture"]
fn million_agent_ten_tick_performance_floor() {
    let model = sir_model();
    let theta = params(&model, 0.8, 0.1);
    let population = SyntheticPopulation::generate(1_000_000, 50_000, 100, 321).unwrap();
    let mut state = StateStore::new(&model, population.sir_table_initializers()).unwrap();
    let started = Instant::now();
    for tick in 0..10 {
        let report = run_tick(&model, &mut state, &theta, 654, tick).unwrap();
        assert_eq!(report.aggregate_builds, 2);
    }
    let elapsed = started.elapsed();
    let seconds_per_tick = elapsed.as_secs_f64() / 10.0;
    eprintln!(
        "PRD0008 performance: 1,000,000 persons x 10 ticks in {:.3}s ({seconds_per_tick:.3}s/tick)",
        elapsed.as_secs_f64()
    );
    assert!(
        seconds_per_tick <= 2.0,
        "performance floor exceeded: {seconds_per_tick:.3}s/tick"
    );
}

mod policy_feedback {
    use std::path::Path;

    use sembla_ir::ValidatedModel;
    use sembla_runtime::eval::ParamEnv;
    use sembla_runtime::executor;
    use sembla_runtime::population::SyntheticPopulation;
    use sembla_runtime::state::{ColumnData, StateStore};

    fn load(example: &str) -> ValidatedModel {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../examples/{example}"));
        let source = std::fs::read_to_string(path).unwrap();
        sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
    }

    fn baseline_state(model: &ValidatedModel, population: &SyntheticPopulation) -> StateStore {
        StateStore::new(model, population.sir_table_initializers()).unwrap()
    }

    fn policy_state(model: &ValidatedModel, population: &SyntheticPopulation) -> StateStore {
        StateStore::new(model, population.sir_policy_table_initializers()).unwrap()
    }

    fn infected(state: &StateStore, box_name: &str) -> usize {
        state
            .snapshot()
            .enum_values(box_name, "person", "health")
            .unwrap()
            .iter()
            .filter(|value| **value == 1)
            .count()
    }

    fn policy_mode(state: &StateStore) -> u16 {
        state
            .snapshot()
            .enum_values("policy", "controller", "mode")
            .unwrap()[0]
    }

    fn effective_modifier_at_tick_start(state: &StateStore) -> f64 {
        let snapshot = state.snapshot();
        let input = snapshot
            .input_table("population", "restriction_modifier")
            .unwrap();
        let offset = match &input.columns[0] {
            ColumnData::Real(values) => values.iter().copied().sum::<f64>(),
            other => panic!("unexpected modifier input column: {other:?}"),
        };
        1.0 + offset
    }

    #[test]
    fn policy_restricts_lowers_peak_has_hysteresis_and_exact_one_tick_feedback_delay() {
        // Fixed paired CRN scenario: 100k people, 500 workplaces, I0=100,
        // population seed 12, run seed 55, beta=0.8, gamma=0.1, dt=0.25.
        // The on threshold is 500 and off threshold is 150. In this frozen
        // scenario restriction must engage during ticks 10..=20. Across 200
        // ticks hysteresis permits at most two changes (restrict, maybe reopen).
        let baseline_model = load("sir.json");
        let policy_model = load("sir_policy.json");
        let population = SyntheticPopulation::generate(100_000, 500, 100, 12).unwrap();
        let mut baseline = baseline_state(&baseline_model, &population);
        let mut policy = policy_state(&policy_model, &population);
        let baseline_params = ParamEnv::defaults(&baseline_model);
        let policy_params = ParamEnv::defaults(&policy_model);

        let mut baseline_peak = infected(&baseline, "sir");
        let mut policy_peak = infected(&policy, "population");
        let mut previous_mode = policy_mode(&policy);
        let mut mode_changes = 0;
        let mut restrict_tick = None;
        let mut first_restricted_hazard_tick = None;

        for tick in 0..200 {
            let effective_modifier = effective_modifier_at_tick_start(&policy);
            if effective_modifier == 0.4 && first_restricted_hazard_tick.is_none() {
                first_restricted_hazard_tick = Some(tick);
            }
            executor::run_tick(&baseline_model, &mut baseline, &baseline_params, 55, tick).unwrap();
            let report =
                executor::run_tick(&policy_model, &mut policy, &policy_params, 55, tick).unwrap();
            baseline_peak = baseline_peak.max(infected(&baseline, "sir"));
            policy_peak = policy_peak.max(infected(&policy, "population"));

            let mode = policy_mode(&policy);
            if mode != previous_mode {
                mode_changes += 1;
                previous_mode = mode;
            }
            if report.fired[2].1 == 1 {
                assert_eq!(mode, 1, "restrict transition must install Restricted mode");
                restrict_tick = Some(tick);
            }
        }

        let restrict_tick = restrict_tick.expect("policy never switched to Restricted");
        assert!(
            (10..=20).contains(&restrict_tick),
            "restriction fired outside documented range: {restrict_tick}"
        );
        assert!(
            policy_peak < baseline_peak,
            "paired policy peak {policy_peak} must be below baseline peak {baseline_peak}"
        );
        assert!(
            mode_changes <= 2,
            "hysteresis changed mode {mode_changes} times (documented maximum is 2)"
        );
        assert_eq!(
            first_restricted_hazard_tick,
            Some(restrict_tick + 1),
            "the modifier output produced by a policy firing must first affect the next tick"
        );
    }
}
