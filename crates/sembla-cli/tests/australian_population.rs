use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use sembla_ir::{
    Expr, FeatureSet, ParamValue, PriorFamily, ValidatedModel, GROUPED_OBSERVATIONS_FEATURE,
};
use sembla_runtime::eval::{ParamEnv, ParamOverride};
use sembla_runtime::executor::run_tick_with_features;
use sembla_runtime::state::{ColumnData, StateStore, TableInit};
use sembla_runtime::state_artifact::{read, state_artifact_hash, to_table_inits};
use sha2::{Digest, Sha256};

const MODEL: &str = "fixtures/australian-population/australian_population.hundredth.json";
const PLAN: &str = "fixtures/australian-population/australian_population.hundredth.plan.json";
const COMPANION: &str = "fixtures/state/australian_population_2010_hundredth.state.model.json";
const STATE: &str = "fixtures/state/australian_population_2010_hundredth.state";
const GOLDENS: &str = "fixtures/australian-population/goldens";
const AGGREGATE_GOLDEN: &str = "fixtures/demographic/goldens/run.csv";
const PARAMS_2010: &str = "data/abs/params/2010.json";
const FIDELITY_EVIDENCE: &str = "data/abs/params/fidelity-2010.json";
const TARGET_EXECUTION: &str = "data/abs/targets/execution.json";
const BASELINE_EVIDENCE: &str = "docs/evidence/australian-population/baseline-2026-08-06";
const ROWS: usize = 352_460;
const SCALE_FACTOR: i64 = 100;
const FIDELITY_STATES: [&str; 8] = ["nsw", "vic", "qld", "sa", "wa", "tas", "nt", "act"];
const FIDELITY_AGE_BANDS: [&str; 21] = [
    "0-4", "5-9", "10-14", "15-19", "20-24", "25-29", "30-34", "35-39", "40-44", "45-49", "50-54",
    "55-59", "60-64", "65-69", "70-74", "75-79", "80-84", "85-89", "90-94", "95-99", "100+",
];

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sembla-australian-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn raw_sha256(path: impl AsRef<Path>) -> String {
    let bytes = std::fs::read(path).unwrap();
    format!("{:x}", Sha256::digest(bytes))
}

fn csv_records(path: impl AsRef<Path>) -> Vec<BTreeMap<String, String>> {
    let source = std::fs::read_to_string(path).unwrap();
    let mut lines = source.lines().filter(|line| !line.starts_with('#'));
    let headers = lines.next().unwrap().split(',').collect::<Vec<_>>();
    lines
        .map(|line| {
            let values = line.split(',').collect::<Vec<_>>();
            assert_eq!(values.len(), headers.len());
            headers
                .iter()
                .zip(values)
                .map(|(name, value)| ((*name).to_owned(), value.to_owned()))
                .collect()
        })
        .collect()
}

fn integer_records(path: impl AsRef<Path>) -> Vec<BTreeMap<String, i64>> {
    csv_records(path)
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|(name, value)| (name, value.parse::<i64>().unwrap()))
                .collect()
        })
        .collect()
}

fn validated_model() -> sembla_ir::ValidatedModel {
    let source = std::fs::read_to_string(repository_path(MODEL)).unwrap();
    let raw = sembla_ir::parse_json(&source).unwrap();
    let features = FeatureSet::from([GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
    sembla_ir::validate_with_features(raw, &features).unwrap()
}

fn table<'a>(tables: &'a [TableInit], name: &str) -> &'a TableInit {
    tables
        .iter()
        .find(|table| table.box_name == "demographic" && table.table_name == name)
        .unwrap()
}

fn enum_column<'a>(table: &'a TableInit, name: &str) -> &'a [u16] {
    match &table
        .columns
        .iter()
        .find(|column| column.name == name)
        .unwrap()
        .data
    {
        ColumnData::Enum(values) => values,
        other => panic!("{name} is not enum data: {other:?}"),
    }
}

fn int_column<'a>(table: &'a TableInit, name: &str) -> &'a [i64] {
    match &table
        .columns
        .iter()
        .find(|column| column.name == name)
        .unwrap()
        .data
    {
        ColumnData::Int(values) => values,
        other => panic!("{name} is not int data: {other:?}"),
    }
}

fn ref_column<'a>(table: &'a TableInit, name: &str) -> &'a [u32] {
    match &table
        .columns
        .iter()
        .find(|column| column.name == name)
        .unwrap()
        .data
    {
        ColumnData::Ref(values) => values,
        other => panic!("{name} is not ref data: {other:?}"),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct FidelityOutcome {
    births_by_state: [i64; 8],
    deaths_by_state: [i64; 8],
    deaths_by_age_band: [i64; 21],
    overseas_arrivals_by_state: [i64; 8],
    overseas_departures_by_state: [i64; 8],
}

impl FidelityOutcome {
    fn births(&self) -> i64 {
        self.births_by_state.iter().sum()
    }

    fn deaths(&self) -> i64 {
        self.deaths_by_state.iter().sum()
    }

    fn scaled(mut self) -> Self {
        for values in [
            &mut self.births_by_state[..],
            &mut self.deaths_by_state[..],
            &mut self.deaths_by_age_band[..],
            &mut self.overseas_arrivals_by_state[..],
            &mut self.overseas_departures_by_state[..],
        ] {
            for value in values {
                *value *= SCALE_FACTOR;
            }
        }
        self
    }
}

fn parameter_env_2010(model: &ValidatedModel) -> ParamEnv {
    let values: BTreeMap<String, f64> =
        serde_json::from_str(&std::fs::read_to_string(repository_path(PARAMS_2010)).unwrap())
            .unwrap();
    let overrides = values
        .into_iter()
        .map(|(name, value)| ParamOverride::new(name, ParamValue::Real { value }))
        .collect::<Vec<_>>();
    ParamEnv::resolve(model, &overrides).unwrap()
}

fn run_fidelity_seed(
    model: &ValidatedModel,
    initial_tables: &[TableInit],
    params: &ParamEnv,
    features: &FeatureSet,
    seed: u64,
) -> FidelityOutcome {
    let mut state = StateStore::new(model, initial_tables.to_vec()).unwrap();
    let mut outcome = FidelityOutcome::default();
    for tick in 0..12_u32 {
        run_tick_with_features(model, &mut state, params, seed, tick, features).unwrap();
        let snapshot = state.snapshot();
        let events = snapshot
            .enum_values("demographic", "person_slot", "event")
            .unwrap();
        let areas = snapshot
            .enum_values("demographic", "person_slot", "area")
            .unwrap();
        for (row, event) in events.iter().copied().enumerate() {
            if event == 0 || event == 5 {
                continue;
            }
            let area = usize::from(areas[row]);
            assert!(area < FIDELITY_STATES.len());
            match event {
                1 => outcome.births_by_state[area] += 1,
                2 => {
                    outcome.deaths_by_state[area] += 1;
                    let event_age = snapshot
                        .int("demographic", "person_slot", "event_age_months", row)
                        .unwrap();
                    assert!(event_age >= 0);
                    let age_band = usize::try_from(event_age / 60).unwrap().min(20);
                    outcome.deaths_by_age_band[age_band] += 1;
                }
                3 => outcome.overseas_arrivals_by_state[area] += 1,
                4 => outcome.overseas_departures_by_state[area] += 1,
                other => panic!("unexpected event ordinal {other}"),
            }
        }
    }
    outcome.scaled()
}

fn state_index(state: &str) -> usize {
    FIDELITY_STATES
        .iter()
        .position(|candidate| *candidate == state)
        .unwrap()
}

fn age_band_index(age_band: &str) -> Option<usize> {
    FIDELITY_AGE_BANDS
        .iter()
        .position(|candidate| *candidate == age_band)
}

fn published_2010_outcomes() -> FidelityOutcome {
    let mut published = FidelityOutcome::default();
    for row in csv_records(repository_path("data/abs/extracts/births_state.csv")) {
        if row["year"] == "2010" {
            published.births_by_state[state_index(&row["state"])] = row["births"].parse().unwrap();
        }
    }
    for row in csv_records(repository_path(
        "data/abs/extracts/deaths_state_age_sex.csv",
    )) {
        if row["year"] != "2010" {
            continue;
        }
        let deaths = row["deaths"].parse::<i64>().unwrap();
        published.deaths_by_state[state_index(&row["state"])] += deaths;
        if let Some(age_band) = age_band_index(&row["age_band"]) {
            published.deaths_by_age_band[age_band] += deaths;
        }
    }
    for row in csv_records(repository_path("data/abs/extracts/overseas_margins.csv")) {
        if row["run_year"] == "2010" {
            let state = state_index(&row["state"]);
            published.overseas_arrivals_by_state[state] = row["arrivals"].parse().unwrap();
            published.overseas_departures_by_state[state] = row["departures"].parse().unwrap();
        }
    }
    published
}

fn sample_statistics(values: &[i64]) -> (f64, f64, i64) {
    assert!(values.len() > 1);
    let mean = values.iter().map(|value| *value as f64).sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| (*value as f64 - mean).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    let standard_deviation = variance.sqrt();
    (
        mean,
        standard_deviation,
        (3.0 * standard_deviation).ceil() as i64,
    )
}

fn comparisons(
    labels: &[&str],
    published: &[i64],
    simulated: &[i64],
) -> BTreeMap<String, serde_json::Value> {
    assert_eq!(labels.len(), published.len());
    assert_eq!(labels.len(), simulated.len());
    labels
        .iter()
        .zip(published)
        .zip(simulated)
        .map(|((label, published), simulated)| {
            (
                (*label).to_owned(),
                serde_json::json!({
                    "published": published,
                    "simulated": simulated,
                    "signed_error": simulated - published,
                }),
            )
        })
        .collect()
}

fn fidelity_evidence(
    pilots: &[(u64, FidelityOutcome)],
    held_out: &FidelityOutcome,
    published: &FidelityOutcome,
) -> serde_json::Value {
    let birth_values = pilots
        .iter()
        .map(|(_seed, outcome)| outcome.births())
        .collect::<Vec<_>>();
    let death_values = pilots
        .iter()
        .map(|(_seed, outcome)| outcome.deaths())
        .collect::<Vec<_>>();
    let (birth_mean, birth_spread, birth_tolerance) = sample_statistics(&birth_values);
    let (death_mean, death_spread, death_tolerance) = sample_statistics(&death_values);
    let birth_error = held_out.births() - published.births();
    let death_error = held_out.deaths() - published.deaths();
    let birth_pass = birth_error.abs() <= birth_tolerance;
    let death_pass = death_error.abs() <= death_tolerance;
    assert!(
        birth_pass,
        "held-out birth error {birth_error} exceeds spread-derived tolerance {birth_tolerance}"
    );
    assert!(
        death_pass,
        "held-out death error {death_error} exceeds spread-derived tolerance {death_tolerance}"
    );

    serde_json::json!({
        "format": "sembla.australian-population-fidelity/v1",
        "held_out": {
            "births": {
                "pass": birth_pass,
                "published": published.births(),
                "signed_error": birth_error,
                "simulated": held_out.births(),
            },
            "births_by_state": comparisons(
                &FIDELITY_STATES,
                &published.births_by_state,
                &held_out.births_by_state,
            ),
            "deaths": {
                "pass": death_pass,
                "published": published.deaths(),
                "signed_error": death_error,
                "simulated": held_out.deaths(),
            },
            "deaths_by_age_band": comparisons(
                &FIDELITY_AGE_BANDS,
                &published.deaths_by_age_band,
                &held_out.deaths_by_age_band,
            ),
            "deaths_by_state": comparisons(
                &FIDELITY_STATES,
                &published.deaths_by_state,
                &held_out.deaths_by_state,
            ),
            "overseas_arrivals_by_state": comparisons(
                &FIDELITY_STATES,
                &published.overseas_arrivals_by_state,
                &held_out.overseas_arrivals_by_state,
            ),
            "overseas_departures_by_state": comparisons(
                &FIDELITY_STATES,
                &published.overseas_departures_by_state,
                &held_out.overseas_departures_by_state,
            ),
        },
        "held_out_seed": 2001,
        "pilot_results": pilots.iter().map(|(seed, outcome)| serde_json::json!({
            "births": outcome.births(),
            "deaths": outcome.deaths(),
            "seed": seed,
        })).collect::<Vec<_>>(),
        "pilot_seeds": (1001_u64..=1010).collect::<Vec<_>>(),
        "predeclaration_path": "fidelity-2010-predeclaration.json",
        "predeclaration_sha256": "87f442ea1b97b90f718e0bf4205497aad03c2d24de0aa7470a09e160f391e18b",
        "run_year": 2010,
        "scale": "hundredth",
        "scale_factor": SCALE_FACTOR,
        "statistics": {
            "births": {
                "mean": birth_mean,
                "sample_standard_deviation": birth_spread,
                "tolerance": birth_tolerance,
            },
            "deaths": {
                "mean": death_mean,
                "sample_standard_deviation": death_spread,
                "tolerance": death_tolerance,
            },
        },
        "status": "measured",
        "targets": {
            "births": published.births(),
            "deaths": published.deaths(),
        },
        "ticks": 12,
        "tolerance_rule": "ceil(3 * sample_standard_deviation)",
    })
}

#[test]
fn generated_model_has_frozen_schema_parameter_and_transition_counts() {
    let model = validated_model();
    assert_eq!(model.model().name, "australian_population");
    assert_eq!(model.model().params.len(), 377);
    assert!(model
        .model()
        .params
        .iter()
        .all(|parameter| parameter.prior.is_some()));
    assert!(!model
        .model()
        .params
        .iter()
        .any(|parameter| { parameter.name == "push_nsw" || parameter.name == "pull_nsw" }));

    let model_box = &model.model().boxes[0];
    assert_eq!(model_box.name, "demographic");
    assert_eq!(
        model_box
            .tables
            .iter()
            .map(|table| (table.name.as_str(), table.size_hint, table.attrs.len()))
            .collect::<Vec<_>>(),
        vec![
            ("person_slot", ROWS as u64, 11),
            ("slot_resource", ROWS as u64, 0)
        ]
    );
    assert_eq!(model_box.transitions.len(), 418);
    assert_eq!(
        model_box
            .grouped_views
            .iter()
            .map(|view| view.name.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "births_cells",
            "deaths_cells",
            "deaths_state_age_cells",
            "interstate_age_sex_flows",
            "interstate_flows",
            "overseas_arrival_cells",
            "overseas_departure_cells",
            "population_cells",
            "population_single_year_cells",
            "vacancy_cells",
        ])
    );
    assert_eq!(
        model_box
            .transitions
            .iter()
            .filter(|transition| transition.name.starts_with("move_"))
            .count(),
        56
    );
    assert_eq!(
        model_box
            .transitions
            .iter()
            .filter(|transition| transition.name.starts_with("die_"))
            .count(),
        336
    );

    let regions = ["nsw", "vic", "qld", "sa", "wa", "tas", "nt", "act"];
    let expected_moves = regions
        .iter()
        .flat_map(|origin| {
            regions.iter().filter_map(move |destination| {
                (origin != destination).then(|| format!("move_{origin}_{destination}"))
            })
        })
        .collect::<BTreeSet<_>>();
    let actual_moves = model_box
        .transitions
        .iter()
        .filter(|transition| transition.name.starts_with("move_"))
        .map(|transition| transition.name.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_moves, expected_moves);

    for transition in &model_box.transitions {
        let contested = transition.name.starts_with("move_")
            || transition.name.starts_with("die_")
            || transition.name.starts_with("emigrate_");
        if contested {
            assert_eq!(transition.contests.len(), 1, "{}", transition.name);
            assert!(matches!(
                transition.contests[0].ordering,
                sembla_ir::ClaimOrdering::RaceTime
            ));
        }
        let written = transition
            .effects
            .iter()
            .map(|effect| match effect {
                sembla_ir::Effect::SetAttr { attr, .. } => attr.as_str(),
            })
            .collect::<BTreeSet<_>>();
        if transition.name.starts_with("move_") {
            assert!(!written.contains("generation"));
            assert!(!written.contains("age_months"));
            assert!(written.contains("prev_area"));
            assert!(written.contains("area"));
        }
        if transition.name.starts_with("die_") || transition.name.starts_with("emigrate_") {
            assert!(written.contains("entry_stream"));
        }
    }
}

#[test]
fn model_defaults_match_2010_params_and_direct_hazards_remain_symbolic() {
    let model = validated_model();
    let expected: BTreeMap<String, f64> =
        serde_json::from_str(&std::fs::read_to_string(repository_path(PARAMS_2010)).unwrap())
            .unwrap();
    let registry: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_path("data/abs/params/priors.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(expected.len(), 377);
    assert_eq!(model.model().params.len(), expected.len());

    for parameter in &model.model().params {
        let ParamValue::Real { value } = parameter.default else {
            panic!("{} is not a real parameter", parameter.name);
        };
        assert_eq!(value, expected[&parameter.name], "{}", parameter.name);
        let metadata = &registry["parameters"][&parameter.name]["lean_prior_2010"];
        let expected_family = match metadata["family"].as_str().unwrap() {
            "normal" => PriorFamily::Normal,
            "log_normal" => PriorFamily::LogNormal,
            other => panic!("unexpected registry prior family {other}"),
        };
        let prior = parameter.prior.as_ref().unwrap();
        assert_eq!(prior.family, expected_family, "{}", parameter.name);
        assert_eq!(
            prior.args,
            vec![
                metadata["location"].as_f64().unwrap(),
                metadata["spread"].as_f64().unwrap(),
            ],
            "{}",
            parameter.name
        );
    }

    for transition in &model.model().boxes[0].transitions {
        if transition.name.starts_with("die_")
            || transition.name.starts_with("birth_")
            || transition.name.starts_with("overseas_arrive_")
            || transition.name.starts_with("emigrate_")
        {
            let Expr::Param { name } = &transition.hazard else {
                panic!(
                    "{} no longer has a symbolic parameter hazard",
                    transition.name
                );
            };
            assert!(
                expected.contains_key(name),
                "unknown hazard parameter {name}"
            );
        }
    }
}

#[test]
fn uncalibrated_2010_fidelity_uses_predeclared_multiseed_spread() {
    let plan_source = std::fs::read_to_string(repository_path(PLAN)).unwrap();
    let plan = match sembla_ir::parse_input(&plan_source).unwrap() {
        sembla_ir::ParsedInput::Plan(plan) => plan,
        sembla_ir::ParsedInput::LegacyModel(_) => panic!("fidelity input is not a plan"),
    };
    let model = sembla_ir::validate_plan(&plan)
        .unwrap()
        .model_with_rule_words();
    let features = FeatureSet::from([GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
    let artifact = read(repository_path(STATE)).unwrap();
    let tables = to_table_inits(&artifact, &model).unwrap();
    let params = parameter_env_2010(&model);
    let pilots = (1001_u64..=1010)
        .map(|seed| {
            (
                seed,
                run_fidelity_seed(&model, &tables, &params, &features, seed),
            )
        })
        .collect::<Vec<_>>();
    let held_out = run_fidelity_seed(&model, &tables, &params, &features, 2001);
    let published = published_2010_outcomes();
    assert_eq!(published.births(), 303_299);
    assert_eq!(published.deaths(), 143_451);
    let actual = fidelity_evidence(&pilots, &held_out, &published);

    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(repository_path(FIDELITY_EVIDENCE)).unwrap())
            .unwrap();
    if expected["status"] == "predeclared" {
        let output = repository_path("data/abs/params/.fidelity-result.json");
        std::fs::write(&output, serde_json::to_vec_pretty(&actual).unwrap()).unwrap();
        println!("wrote predeclared fidelity result to {}", output.display());
    } else {
        assert_eq!(actual, expected);
    }
}

#[test]
fn companion_and_executable_plan_pass_the_public_validate_command() {
    for relative in [COMPANION, PLAN] {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("validate")
            .arg(repository_path(relative))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{relative}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn annual_parameter_files_are_accepted_by_public_run_and_sweep_paths() {
    let temp = temp_dir("annual-params");
    for year in 2010..=2024 {
        let output = temp.join(format!("run-{year}.csv"));
        let params = repository_path(format!("data/abs/params/{year}.json"));
        let process = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(repository_path(PLAN))
            .arg("--population")
            .arg(repository_path(STATE))
            .args(["--seed", "19", "--ticks", "1", "--out"])
            .arg(&output)
            .arg("--params")
            .arg(&params)
            .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
            .output()
            .unwrap();
        assert!(
            process.status.success(),
            "{year}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&process.stdout),
            String::from_utf8_lossy(&process.stderr)
        );
    }

    let process = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("sweep")
        .arg(repository_path(PLAN))
        .arg("--population")
        .arg(repository_path(STATE))
        .args(["--seed", "31", "--draws", "1", "--ticks", "1", "--out"])
        .arg(temp.join("sweep"))
        .arg("--params")
        .arg(repository_path(PARAMS_2010))
        .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
        .output()
        .unwrap();
    assert!(
        process.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&process.stdout),
        String::from_utf8_lossy(&process.stderr)
    );
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn annual_chain_driver_is_bitwise_reproducible_and_rejects_saturation() {
    let temp = temp_dir("annual-chain-driver");
    let first = temp.join("first");
    let second = temp.join("second");
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_sembla"));
    for output in [&first, &second] {
        let process = Command::new(repository_path("scripts/run-australian-population.sh"))
            .args([
                "--scale",
                "hundredth",
                "--start-year",
                "2010",
                "--end-year",
                "2010",
            ])
            .arg("--params-dir")
            .arg(repository_path("data/abs/params"))
            .arg("--targets-dir")
            .arg(repository_path("data/abs/targets"))
            .arg("--out")
            .arg(output)
            .args(["--backend", "cpu", "--enable", GROUPED_OBSERVATIONS_FEATURE])
            .arg("--sembla")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            process.status.success(),
            "stdout={}\nstderr={}",
            String::from_utf8_lossy(&process.stdout),
            String::from_utf8_lossy(&process.stderr)
        );
    }
    let proof = temp.join("reproduction.json");
    let comparison = Command::new("python3")
        .arg(repository_path("data/abs/chain.py"))
        .arg("compare")
        .arg("--left")
        .arg(&first)
        .arg("--right")
        .arg(&second)
        .arg("--out")
        .arg(&proof)
        .output()
        .unwrap();
    assert!(
        comparison.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&comparison.stdout),
        String::from_utf8_lossy(&comparison.stderr)
    );
    let reproduction: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&proof).unwrap()).unwrap();
    assert_eq!(reproduction["byte_identical"], true);
    let chain_report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(first.join("chain-report.json")).unwrap()).unwrap();
    assert_eq!(chain_report["link_count"], 1);
    assert_eq!(chain_report["links"][0]["run_year"], 2010);
    assert_eq!(
        chain_report["links"][0]["seed"]["seed"],
        2_594_735_361_883_248_024_u64
    );
    assert_eq!(
        chain_report["links"][0]["output_state"]["state_artifact"],
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(format!(
                "{}.manifest.json",
                first.join("2010.csv").display()
            ))
            .unwrap()
        )
        .unwrap()["exported_state"]
    );

    let saturated_params = temp.join("saturated.json");
    let mut params: serde_json::Value =
        serde_json::from_slice(&std::fs::read(repository_path(PARAMS_2010)).unwrap()).unwrap();
    params["interstate_base"] = serde_json::json!(1.0);
    for state in FIDELITY_STATES {
        params[format!("birth_rate_{state}")] = serde_json::json!(1_000_000.0);
    }
    std::fs::write(
        &saturated_params,
        format!("{}\n", serde_json::to_string_pretty(&params).unwrap()),
    )
    .unwrap();
    let saturated_run = temp.join("saturated.csv");
    let saturated = Command::new(&binary)
        .arg("run")
        .arg(repository_path(PLAN))
        .arg("--population")
        .arg(repository_path(STATE))
        .args(["--seed", "7", "--ticks", "12", "--params"])
        .arg(&saturated_params)
        .args(["--backend", "cpu", "--out"])
        .arg(&saturated_run)
        .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
        .output()
        .unwrap();
    assert!(
        saturated.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&saturated.stdout),
        String::from_utf8_lossy(&saturated.stderr)
    );
    let guard = Command::new("python3")
        .arg(repository_path("data/abs/chain.py"))
        .arg("check-capacity")
        .arg("--run")
        .arg(&saturated_run)
        .arg("--model")
        .arg(repository_path(MODEL))
        .output()
        .unwrap();
    assert!(!guard.status.success());
    assert!(
        String::from_utf8_lossy(&guard.stderr).contains("saturation"),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&guard.stdout),
        String::from_utf8_lossy(&guard.stderr)
    );
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn annual_chain_capacity_guard_uses_strict_threshold_and_zero_margins() {
    let temp = temp_dir("annual-chain-capacity");
    let model = temp.join("model.json");
    std::fs::write(
        &model,
        serde_json::to_vec_pretty(&serde_json::json!({
            "boxes": [{
                "transitions": [{
                    "name": "move",
                    "contests": [{
                        "resource": {"kind": "self_attr", "name": "slot_resource"},
                        "ordering": {"kind": "race_time"}
                    }]
                }]
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let run = temp.join("run.csv");
    let write_run = |deferred: u32, fired: u32, birth: u32, overseas: u32| {
        let mut source = String::from(
            "tick,vacant_birth_slots,vacant_overseas_slots,fired_move,deferred_total\n",
        );
        for tick in 0..12 {
            writeln!(source, "{tick},{birth},{overseas},{fired},{deferred}").unwrap();
        }
        std::fs::write(&run, source).unwrap();
    };
    let guard = || {
        Command::new("python3")
            .arg(repository_path("data/abs/chain.py"))
            .arg("check-capacity")
            .arg("--run")
            .arg(&run)
            .arg("--model")
            .arg(&model)
            .output()
            .unwrap()
    };

    write_run(1, 10, 1, 1);
    let exact = guard();
    assert!(
        exact.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&exact.stdout),
        String::from_utf8_lossy(&exact.stderr)
    );
    write_run(2, 10, 1, 1);
    let saturated = guard();
    assert!(!saturated.status.success());
    assert!(String::from_utf8_lossy(&saturated.stderr).contains("saturation"));
    write_run(0, 10, 0, 1);
    let zero_birth = guard();
    assert!(!zero_birth.status.success());
    assert!(String::from_utf8_lossy(&zero_birth.stderr).contains("birth slots reached zero"));
    write_run(0, 10, 1, 0);
    let zero_overseas = guard();
    assert!(!zero_overseas.status.success());
    assert!(String::from_utf8_lossy(&zero_overseas.stderr).contains("overseas slots reached zero"));
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn committed_baseline_chain_evidence_is_complete_and_hash_bound() {
    let evidence = repository_path(BASELINE_EVIDENCE);
    let summary: serde_json::Value =
        serde_json::from_slice(&std::fs::read(evidence.join("summary.json")).unwrap()).unwrap();
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(evidence.join("chain-report.json")).unwrap())
            .unwrap();
    let reproduction: serde_json::Value =
        serde_json::from_slice(&std::fs::read(evidence.join("reproduction.json")).unwrap())
            .unwrap();
    let middle: serde_json::Value = serde_json::from_slice(
        &std::fs::read(evidence.join("middle-year-reproduction.json")).unwrap(),
    )
    .unwrap();

    assert_eq!(
        summary["format"],
        "sembla.australian-population-baseline-evidence/v1"
    );
    assert_eq!(report["format"], "sembla.australian-population-chain/v1");
    assert_eq!(report["scale"], "hundredth");
    assert_eq!(report["start_run_year"], 2010);
    assert_eq!(report["end_run_year"], 2024);
    assert_eq!(report["link_count"], 15);
    let links = report["links"].as_array().unwrap();
    assert_eq!(links.len(), 15);
    for (index, link) in links.iter().enumerate() {
        assert_eq!(link["run_year"], 2010 + index as u64);
        assert_eq!(link["capacity"]["valid_calibration_evidence"], true);
        let residual = evidence
            .join("residuals")
            .join(format!("{}.json", 2010 + index));
        assert_eq!(raw_sha256(residual), link["score"]["raw_sha256"]);
    }
    for pair in links.windows(2) {
        assert_eq!(
            pair[0]["output_state"]["state_artifact"]["hash"],
            pair[1]["input_state"]["state_artifact"]["hash"]
        );
    }
    assert_eq!(
        raw_sha256(evidence.join("chain-report.json")),
        summary["chain_report_raw_sha256"]
    );
    assert_eq!(
        raw_sha256(evidence.join("reproduction.json")),
        summary["reproduction_raw_sha256"]
    );
    assert_eq!(
        raw_sha256(evidence.join("middle-year-reproduction.json")),
        summary["middle_year_reproduction_raw_sha256"]
    );
    assert_eq!(reproduction["byte_identical"], true);
    assert_eq!(reproduction["file_count"], 226);
    let report_inventory = reproduction["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"] == "chain-report.json")
        .unwrap();
    assert_eq!(
        report_inventory["raw_sha256"],
        summary["chain_report_raw_sha256"]
    );
    assert_eq!(middle["byte_identical"], true);
    assert_eq!(middle["run_year"], 2017);
    assert_eq!(middle["file_count"], 15);
    assert!(middle["run_year"].as_u64() > report["start_run_year"].as_u64());
    assert!(middle["run_year"].as_u64() < report["end_run_year"].as_u64());
    assert_eq!(summary["final_boundary_year"], 2025);
    assert_eq!(summary["final_eight_state_erp"]["signed_error"], 174_492.0);
    assert!(std::fs::read_dir(&evidence).unwrap().all(|entry| entry
        .unwrap()
        .path()
        .extension()
        .and_then(|value| value.to_str())
        != Some("state")));
}

#[test]
fn twenty_four_tick_golden_reproduces_all_committed_outputs() {
    let temp = temp_dir("golden");
    let output = temp.join("run.csv");
    let process = Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path(PLAN))
        .arg("--population")
        .arg(repository_path(STATE))
        .args(["--seed", "8305", "--ticks", "24", "--out"])
        .arg(&output)
        .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
        .output()
        .unwrap();
    assert!(
        process.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&process.stdout),
        String::from_utf8_lossy(&process.stderr)
    );
    assert_eq!(
        process.stdout,
        std::fs::read(repository_path(GOLDENS).join("run.hashes.txt")).unwrap()
    );
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(format!("{}.manifest.json", output.display())).unwrap(),
    )
    .unwrap();
    let execution: serde_json::Value =
        serde_json::from_slice(&std::fs::read(repository_path(TARGET_EXECUTION)).unwrap()).unwrap();
    assert_eq!(manifest["ir_hash"], execution["model"]["ir_hash"]["digest"]);
    assert_eq!(
        manifest["ir_hash_algorithm"],
        execution["model"]["ir_hash"]["algorithm"]
    );
    assert_eq!(
        manifest["plan"]["plan_semantic_hash"],
        execution["plan"]["semantic_hash"]
    );
    assert_eq!(manifest["plan"]["plan_schema"], execution["plan"]["schema"]);
    assert_eq!(
        manifest["plan"]["identity_scheme"],
        execution["plan"]["identity_scheme"]
    );
    assert_eq!(manifest["plan"]["origin"], execution["plan"]["origin"]);
    assert_eq!(
        manifest["plan"]["enabled_features"],
        execution["plan"]["enabled_features"]
    );
    for name in [
        "run.csv",
        "run.csv.summaries.csv",
        "run.grouped.births_cells.csv",
        "run.grouped.deaths_cells.csv",
        "run.grouped.deaths_state_age_cells.csv",
        "run.grouped.interstate_age_sex_flows.csv",
        "run.grouped.interstate_flows.csv",
        "run.grouped.overseas_arrival_cells.csv",
        "run.grouped.overseas_departure_cells.csv",
        "run.grouped.population_cells.csv",
        "run.grouped.population_single_year_cells.csv",
        "run.grouped.vacancy_cells.csv",
    ] {
        assert_eq!(
            std::fs::read(temp.join(name)).unwrap(),
            std::fs::read(repository_path(GOLDENS).join(name)).unwrap(),
            "{name}"
        );
    }
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn all_eight_invariant_groups_hold_over_every_golden_tick() {
    let scalar = integer_records(repository_path(GOLDENS).join("run.csv"));
    assert_eq!(scalar.len(), 24);
    let states = ["nsw", "vic", "qld", "sa", "wa", "tas", "nt", "act"];
    let state_index = states
        .iter()
        .enumerate()
        .map(|(index, state)| ((*state).to_owned(), index))
        .collect::<BTreeMap<_, _>>();

    let by_tick_area = |name: &str| {
        let mut totals = BTreeMap::new();
        for row in csv_records(repository_path(GOLDENS).join(name)) {
            let tick = row["tick"].parse::<i64>().unwrap();
            let area = row["area"].clone();
            let count = row["count"].parse::<i64>().unwrap();
            *totals.entry((tick, area)).or_insert(0_i64) += count;
        }
        totals
    };
    let populations = by_tick_area("run.grouped.population_cells.csv");
    let births = by_tick_area("run.grouped.births_cells.csv");
    let deaths = by_tick_area("run.grouped.deaths_cells.csv");
    let overseas_arrivals = by_tick_area("run.grouped.overseas_arrival_cells.csv");
    let overseas_departures = by_tick_area("run.grouped.overseas_departure_cells.csv");

    let mut interstate_in = BTreeMap::new();
    let mut interstate_out = BTreeMap::new();
    let mut interstate_total = BTreeMap::new();
    for row in csv_records(repository_path(GOLDENS).join("run.grouped.interstate_flows.csv")) {
        let tick = row["tick"].parse::<i64>().unwrap();
        let origin = row["prev_area"].clone();
        let destination = row["area"].clone();
        let count = row["count"].parse::<i64>().unwrap();
        assert_ne!(origin, destination, "self move at tick {tick}");
        *interstate_out.entry((tick, origin)).or_insert(0_i64) += count;
        *interstate_in.entry((tick, destination)).or_insert(0_i64) += count;
        *interstate_total.entry(tick).or_insert(0_i64) += count;
    }

    let mut vacancy_total = BTreeMap::new();
    for row in csv_records(repository_path(GOLDENS).join("run.grouped.vacancy_cells.csv")) {
        let tick = row["tick"].parse::<i64>().unwrap();
        let count = row["count"].parse::<i64>().unwrap();
        *vacancy_total.entry(tick).or_insert(0_i64) += count;
    }

    let model = validated_model();
    let artifact = read(repository_path(STATE)).unwrap();
    let tables = to_table_inits(&artifact, &model).unwrap();
    let people = table(&tables, "person_slot");
    let occupancy = enum_column(people, "occupancy");
    let area = enum_column(people, "area");
    let mut previous = [0_i64; 8];
    for row in 0..ROWS {
        if occupancy[row] == 1 {
            previous[area[row] as usize] += 1;
        }
    }

    let mut exercised = [false; 5];
    for row in scalar {
        let tick = row["tick"];
        let current = std::array::from_fn::<_, 8, _>(|index| {
            populations
                .get(&(tick, states[index].to_owned()))
                .copied()
                .unwrap_or_default()
        });
        let birth_total = row["births_this_tick"];
        let death_total = row["deaths_this_tick"];
        let arrival_total = row["overseas_arrivals_this_tick"];
        let departure_total = row["overseas_departures_this_tick"];
        let move_total = row["interstate_moves_this_tick"];
        for (index, value) in [
            birth_total,
            death_total,
            arrival_total,
            departure_total,
            move_total,
        ]
        .into_iter()
        .enumerate()
        {
            exercised[index] |= value > 0;
        }

        let grouped_sum = |values: &BTreeMap<(i64, String), i64>| {
            states
                .iter()
                .map(|state| {
                    values
                        .get(&(tick, (*state).to_owned()))
                        .copied()
                        .unwrap_or_default()
                })
                .sum::<i64>()
        };
        assert_eq!(grouped_sum(&births), birth_total);
        assert_eq!(grouped_sum(&deaths), death_total);
        assert_eq!(grouped_sum(&overseas_arrivals), arrival_total);
        assert_eq!(grouped_sum(&overseas_departures), departure_total);
        assert_eq!(
            interstate_total.get(&tick).copied().unwrap_or_default(),
            move_total
        );
        assert_eq!(
            interstate_in
                .iter()
                .filter(|((candidate, _), _)| *candidate == tick)
                .map(|(_, count)| count)
                .sum::<i64>(),
            move_total
        );
        assert_eq!(
            interstate_out
                .iter()
                .filter(|((candidate, _), _)| *candidate == tick)
                .map(|(_, count)| count)
                .sum::<i64>(),
            move_total
        );

        assert_eq!(
            current.iter().sum::<i64>(),
            previous.iter().sum::<i64>() + birth_total - death_total + arrival_total
                - departure_total,
            "national identity at tick {tick}"
        );
        for state in states {
            let index = state_index[state];
            let value = |values: &BTreeMap<(i64, String), i64>| {
                values
                    .get(&(tick, state.to_owned()))
                    .copied()
                    .unwrap_or_default()
            };
            assert_eq!(
                current[index],
                previous[index] + value(&births) - value(&deaths) + value(&overseas_arrivals)
                    - value(&overseas_departures)
                    + value(&interstate_in)
                    - value(&interstate_out),
                "state identity for {state} at tick {tick}"
            );
        }
        assert!(death_total + departure_total + move_total <= previous.iter().sum());
        assert_eq!(row["invalid_age"], 0);
        assert!(row["max_generation"] <= 1);
        assert_eq!(
            row["locked_out"],
            birth_total + arrival_total + move_total,
            "one-tick marker accounting at tick {tick}"
        );
        assert_eq!(
            row["population"]
                + vacancy_total.get(&tick).copied().unwrap_or_default()
                + death_total
                + departure_total,
            ROWS as i64,
            "closed physical slots at tick {tick}"
        );
        assert!(row["vacant_birth_slots"] > 0);
        assert!(row["vacant_overseas_slots"] > 0);
        previous = current;
    }
    assert!(exercised.into_iter().all(|value| value));

    // Explicit PRD contrast: individual interstate writes close exactly every
    // tick, while the accepted §K9 aggregate design has a visible nonzero
    // internal-arrival/departure residual in its own committed golden.
    let aggregate = integer_records(repository_path(AGGREGATE_GOLDEN));
    assert!(aggregate
        .iter()
        .any(|row| { row["internal_arrivals_this_tick"] != row["internal_departures_this_tick"] }));
}

#[test]
fn per_slot_golden_trajectory_preserves_identity_and_retirement_every_tick() {
    const SEED: u64 = 8305;
    let plan_source = std::fs::read_to_string(repository_path(PLAN)).unwrap();
    let plan = match sembla_ir::parse_input(&plan_source).unwrap() {
        sembla_ir::ParsedInput::Plan(plan) => plan,
        sembla_ir::ParsedInput::LegacyModel(_) => panic!("golden input is not a plan"),
    };
    let model = sembla_ir::validate_plan(&plan)
        .unwrap()
        .model_with_rule_words();
    let features = FeatureSet::from([GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
    let artifact = read(repository_path(STATE)).unwrap();
    let tables = to_table_inits(&artifact, &model).unwrap();
    let people = table(&tables, "person_slot");
    let mut expected_age = int_column(people, "age_months").to_vec();
    let entry_age = int_column(people, "entry_age_months").to_vec();
    let mut expected_generation = int_column(people, "generation").to_vec();
    let mut state = StateStore::new(&model, tables).unwrap();
    let params = ParamEnv::defaults(&model);
    let golden = integer_records(repository_path(GOLDENS).join("run.csv"));

    let mut activated = vec![false; ROWS];
    let mut activation_cleared = vec![false; ROWS];
    let mut permanently_retired = vec![false; ROWS];
    let mut entrant_retirement_observed_after_exit = false;
    let mut move_count = 0_i64;
    let mut activation_count = 0_i64;
    let mut activated_exit_count = 0_i64;

    for tick in 0..24_u32 {
        let before = state.snapshot();
        let before_occupancy = before
            .enum_values("demographic", "person_slot", "occupancy")
            .unwrap()
            .to_vec();
        let before_event = before
            .enum_values("demographic", "person_slot", "event")
            .unwrap()
            .to_vec();
        let before_area = before
            .enum_values("demographic", "person_slot", "area")
            .unwrap()
            .to_vec();
        let before_stream = before
            .enum_values("demographic", "person_slot", "entry_stream")
            .unwrap()
            .to_vec();
        for row in 0..ROWS {
            if permanently_retired[row] {
                assert_eq!(before_occupancy[row], 0, "retired row {row} at tick {tick}");
                assert_eq!(before_stream[row], 2, "retired stream {row} at tick {tick}");
                if activated[row] {
                    entrant_retirement_observed_after_exit = true;
                }
            }
            if before_occupancy[row] == 1 {
                expected_age[row] += 1;
            }
        }

        let report =
            run_tick_with_features(&model, &mut state, &params, SEED, tick, &features).unwrap();
        assert_eq!(report.tick, tick);

        let after = state.snapshot();
        let after_occupancy = after
            .enum_values("demographic", "person_slot", "occupancy")
            .unwrap();
        let after_event = after
            .enum_values("demographic", "person_slot", "event")
            .unwrap();
        let after_area = after
            .enum_values("demographic", "person_slot", "area")
            .unwrap();
        let after_previous_area = after
            .enum_values("demographic", "person_slot", "prev_area")
            .unwrap();
        let after_stream = after
            .enum_values("demographic", "person_slot", "entry_stream")
            .unwrap();
        let mut events = [0_i64; 6];
        let mut population = 0_i64;

        for row in 0..ROWS {
            let event = after_event[row] as usize;
            events[event] += 1;
            population += i64::from(after_occupancy[row] == 1);

            if before_event[row] != 0 {
                assert_eq!(event, 0, "event did not clear for row {row} at tick {tick}");
                if before_event[row] == 1 || before_event[row] == 3 {
                    activation_cleared[row] = true;
                }
            }
            if event != 5 {
                assert_eq!(
                    after_area[row], before_area[row],
                    "non-move changed area for row {row} at tick {tick}"
                );
                assert_eq!(
                    after_previous_area[row], 0,
                    "non-move retained an origin for row {row} at tick {tick}"
                );
            }

            match event {
                0 => {
                    assert_eq!(after_occupancy[row], before_occupancy[row]);
                    assert_eq!(after_stream[row], before_stream[row]);
                }
                1 | 3 => {
                    assert_eq!(before_occupancy[row], 0);
                    assert_eq!(before_event[row], 0);
                    assert_eq!(before_stream[row], if event == 1 { 0 } else { 1 });
                    assert_eq!(after_occupancy[row], 1);
                    assert_eq!(after_stream[row], before_stream[row]);
                    assert!(!activated[row], "row {row} activated twice");
                    activated[row] = true;
                    activation_count += 1;
                    expected_generation[row] += 1;
                    expected_age[row] = if event == 1 { 0 } else { entry_age[row] };
                    assert_eq!(
                        after
                            .int("demographic", "person_slot", "generation", row)
                            .unwrap(),
                        expected_generation[row]
                    );
                    assert_eq!(
                        after
                            .int("demographic", "person_slot", "age_months", row)
                            .unwrap(),
                        expected_age[row]
                    );
                }
                2 | 4 => {
                    assert_eq!(before_occupancy[row], 1);
                    assert_eq!(before_event[row], 0);
                    assert_eq!(after_occupancy[row], 0);
                    assert_eq!(after_stream[row], 2);
                    assert_eq!(
                        after
                            .int("demographic", "person_slot", "generation", row)
                            .unwrap(),
                        expected_generation[row]
                    );
                    assert_eq!(
                        after
                            .int("demographic", "person_slot", "age_months", row)
                            .unwrap(),
                        expected_age[row]
                    );
                    if activated[row] {
                        assert!(activation_cleared[row]);
                        activated_exit_count += 1;
                    }
                    permanently_retired[row] = true;
                }
                5 => {
                    move_count += 1;
                    assert_eq!(before_occupancy[row], 1);
                    assert_eq!(before_event[row], 0);
                    assert_eq!(after_occupancy[row], 1);
                    assert_eq!(after_stream[row], before_stream[row]);
                    assert_ne!(after_area[row], before_area[row]);
                    assert_eq!(after_previous_area[row], before_area[row] + 1);
                    assert_eq!(
                        after
                            .int("demographic", "person_slot", "generation", row)
                            .unwrap(),
                        expected_generation[row],
                        "move changed generation for row {row} at tick {tick}"
                    );
                    assert_eq!(
                        after
                            .int("demographic", "person_slot", "age_months", row)
                            .unwrap(),
                        expected_age[row],
                        "move broke monthly ageing for row {row} at tick {tick}"
                    );
                }
                other => panic!("unexpected event ordinal {other}"),
            }
            if permanently_retired[row] {
                assert_eq!(after_occupancy[row], 0);
                assert_eq!(after_stream[row], 2);
            }
            assert!(expected_generation[row] <= 1);
        }

        let expected = &golden[tick as usize];
        assert_eq!(expected["tick"], tick as i64);
        assert_eq!(population, expected["population"]);
        assert_eq!(events[1], expected["births_this_tick"]);
        assert_eq!(events[2], expected["deaths_this_tick"]);
        assert_eq!(events[3], expected["overseas_arrivals_this_tick"]);
        assert_eq!(events[4], expected["overseas_departures_this_tick"]);
        assert_eq!(events[5], expected["interstate_moves_this_tick"]);
        assert_eq!(events[1] + events[3] + events[5], expected["locked_out"]);
    }

    let final_snapshot = state.snapshot();
    for (row, expected) in expected_generation.iter().enumerate() {
        assert_eq!(
            final_snapshot
                .int("demographic", "person_slot", "generation", row)
                .unwrap(),
            *expected
        );
    }
    let mut final_hash = String::new();
    for byte in final_snapshot.state_hash() {
        write!(&mut final_hash, "{byte:02x}").unwrap();
    }
    assert_eq!(
        final_hash,
        "4dc8c3759afa8aaf529687fe9fe9023f69d60e6188356b98b2f88e5f5b98a7ff"
    );
    assert!(move_count > 0);
    assert!(activation_count > 0);
    assert!(activated_exit_count > 0);
    assert!(entrant_retirement_observed_after_exit);
}

#[test]
fn python_artifact_loads_exactly_through_the_rust_model() {
    let model = validated_model();
    let artifact = read(repository_path(STATE)).unwrap();
    let tables = to_table_inits(&artifact, &model).unwrap();
    assert_eq!(tables.len(), 2);

    let people = table(&tables, "person_slot");
    let resources = table(&tables, "slot_resource");
    assert_eq!(people.row_count, ROWS);
    assert_eq!(resources.row_count, ROWS);
    assert!(resources.columns.is_empty());

    let occupancy = enum_column(people, "occupancy");
    let event = enum_column(people, "event");
    let entry_stream = enum_column(people, "entry_stream");
    let area = enum_column(people, "area");
    let previous_area = enum_column(people, "prev_area");
    let age = int_column(people, "age_months");
    let event_age = int_column(people, "event_age_months");
    let generation = int_column(people, "generation");
    let entry_age = int_column(people, "entry_age_months");
    let slot_resource = ref_column(people, "slot_resource");

    assert_eq!(
        occupancy.iter().filter(|&&value| value == 1).count(),
        220_287
    );
    assert_eq!(
        occupancy.iter().filter(|&&value| value == 0).count(),
        132_173
    );
    assert!(event.iter().all(|&value| value == 0));
    assert_eq!(
        entry_stream.iter().filter(|&&value| value == 0).count(),
        50_081
    );
    assert_eq!(
        entry_stream.iter().filter(|&&value| value == 1).count(),
        82_092
    );
    assert_eq!(
        entry_stream.iter().filter(|&&value| value == 2).count(),
        220_287
    );
    assert!(area.iter().all(|&value| value < 8));
    assert!(previous_area.iter().all(|&value| value == 0));
    assert!(event_age.iter().all(|&value| value == 0));
    assert!(generation.iter().all(|&value| value == 0));
    assert!(age.iter().all(|&value| (0..=1211).contains(&value)));
    assert!(entry_age.iter().all(|&value| (0..=780).contains(&value)));
    assert!(slot_resource
        .iter()
        .enumerate()
        .all(|(row, &value)| value as usize == row));
}

#[test]
fn isolated_interstate_moves_conserve_state_counts_and_preserve_identity() {
    let temp = temp_dir("moves");
    let model_source = std::fs::read_to_string(repository_path(MODEL)).unwrap();
    let model_json: serde_json::Value = serde_json::from_str(&model_source).unwrap();
    let mut parameters = serde_json::Map::new();
    for parameter in model_json["params"].as_array().unwrap() {
        let name = parameter["name"].as_str().unwrap();
        let value = if name == "interstate_base" {
            serde_json::json!(0.02)
        } else if name.starts_with("birth_rate_")
            || name.starts_with("mortality_")
            || name.starts_with("overseas_arrival_")
            || name.starts_with("emigration_")
        {
            serde_json::json!(0.0)
        } else {
            parameter["default"]["value"].clone()
        };
        parameters.insert(name.to_owned(), value);
    }
    let parameter_path = temp.join("movement-only.json");
    std::fs::write(
        &parameter_path,
        serde_json::to_vec(&serde_json::Value::Object(parameters)).unwrap(),
    )
    .unwrap();

    let run = |label: &str| {
        let output = temp.join(format!("{label}.csv"));
        let final_state = temp.join(format!("{label}.state"));
        let process = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(repository_path(PLAN))
            .arg("--population")
            .arg(repository_path(STATE))
            .args(["--seed", "71", "--ticks", "1", "--out"])
            .arg(&output)
            .arg("--export-state")
            .arg(&final_state)
            .args(["--params"])
            .arg(&parameter_path)
            .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
            .output()
            .unwrap();
        assert!(
            process.status.success(),
            "stdout={}\nstderr={}",
            String::from_utf8_lossy(&process.stdout),
            String::from_utf8_lossy(&process.stderr)
        );
        (output, final_state, process.stdout)
    };

    let first = run("first");
    let second = run("second");
    assert_eq!(
        std::fs::read(&first.0).unwrap(),
        std::fs::read(&second.0).unwrap()
    );
    assert_eq!(
        std::fs::read(&first.1).unwrap(),
        std::fs::read(&second.1).unwrap()
    );
    assert_eq!(first.2, second.2);

    let model = validated_model();
    let initial_artifact = read(repository_path(STATE)).unwrap();
    let initial_tables = to_table_inits(&initial_artifact, &model).unwrap();
    let final_artifact = read(&first.1).unwrap();
    let final_tables = to_table_inits(&final_artifact, &model).unwrap();
    let initial = table(&initial_tables, "person_slot");
    let final_people = table(&final_tables, "person_slot");

    let initial_occupancy = enum_column(initial, "occupancy");
    let final_occupancy = enum_column(final_people, "occupancy");
    let initial_area = enum_column(initial, "area");
    let final_area = enum_column(final_people, "area");
    let final_previous = enum_column(final_people, "prev_area");
    let final_event = enum_column(final_people, "event");
    let initial_age = int_column(initial, "age_months");
    let final_age = int_column(final_people, "age_months");
    let initial_generation = int_column(initial, "generation");
    let final_generation = int_column(final_people, "generation");
    let initial_resource = ref_column(initial, "slot_resource");
    let final_resource = ref_column(final_people, "slot_resource");

    assert_eq!(initial_occupancy, final_occupancy);
    assert_eq!(initial_generation, final_generation);
    assert_eq!(initial_resource, final_resource);

    let mut initial_counts = [0_i64; 8];
    let mut final_counts = [0_i64; 8];
    let mut arrivals = [0_i64; 8];
    let mut departures = [0_i64; 8];
    let mut moves = 0_i64;
    for row in 0..ROWS {
        if initial_occupancy[row] == 1 {
            initial_counts[initial_area[row] as usize] += 1;
            final_counts[final_area[row] as usize] += 1;
            assert_eq!(final_age[row], initial_age[row] + 1);
            if final_area[row] != initial_area[row] {
                moves += 1;
                departures[initial_area[row] as usize] += 1;
                arrivals[final_area[row] as usize] += 1;
                assert_eq!(final_event[row], 5, "moving row lacks move event");
                assert_eq!(
                    final_previous[row],
                    initial_area[row] + 1,
                    "moving row lost its exact origin"
                );
            } else {
                assert_eq!(final_event[row], 0);
                assert_eq!(final_previous[row], 0);
            }
        } else {
            assert_eq!(final_age[row], initial_age[row]);
            assert_eq!(final_area[row], initial_area[row]);
            assert_eq!(final_event[row], 0);
            assert_eq!(final_previous[row], 0);
        }
    }
    assert!(moves > 0);
    assert_eq!(arrivals.iter().sum::<i64>(), moves);
    assert_eq!(departures.iter().sum::<i64>(), moves);
    assert_eq!(
        initial_counts.iter().sum::<i64>(),
        final_counts.iter().sum::<i64>()
    );
    for region in 0..8 {
        assert_eq!(
            final_counts[region],
            initial_counts[region] + arrivals[region] - departures[region]
        );
    }

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn one_tick_stock_flow_and_closed_slot_accounting_hold_exactly() {
    let temp = temp_dir("stock-flow");
    let run = |label: &str| {
        let output = temp.join(format!("{label}.csv"));
        let final_state = temp.join(format!("{label}.state"));
        let process = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(repository_path(PLAN))
            .arg("--population")
            .arg(repository_path(STATE))
            .args(["--seed", "83", "--ticks", "1", "--out"])
            .arg(&output)
            .arg("--export-state")
            .arg(&final_state)
            .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
            .output()
            .unwrap();
        assert!(
            process.status.success(),
            "stdout={}\nstderr={}",
            String::from_utf8_lossy(&process.stdout),
            String::from_utf8_lossy(&process.stderr)
        );
        (output, final_state, process.stdout)
    };
    let first = run("first");
    let second = run("second");
    assert_eq!(
        std::fs::read(&first.0).unwrap(),
        std::fs::read(&second.0).unwrap()
    );
    assert_eq!(
        std::fs::read(&first.1).unwrap(),
        std::fs::read(&second.1).unwrap()
    );
    assert_eq!(first.2, second.2);

    let model = validated_model();
    let initial_artifact = read(repository_path(STATE)).unwrap();
    let initial_tables = to_table_inits(&initial_artifact, &model).unwrap();
    let final_artifact = read(&first.1).unwrap();
    let final_tables = to_table_inits(&final_artifact, &model).unwrap();
    let initial = table(&initial_tables, "person_slot");
    let final_people = table(&final_tables, "person_slot");

    let initial_occupancy = enum_column(initial, "occupancy");
    let final_occupancy = enum_column(final_people, "occupancy");
    let initial_event = enum_column(initial, "event");
    let final_event = enum_column(final_people, "event");
    let initial_area = enum_column(initial, "area");
    let final_area = enum_column(final_people, "area");
    let final_previous = enum_column(final_people, "prev_area");
    let initial_stream = enum_column(initial, "entry_stream");
    let final_stream = enum_column(final_people, "entry_stream");
    let initial_age = int_column(initial, "age_months");
    let final_age = int_column(final_people, "age_months");
    let entry_age = int_column(final_people, "entry_age_months");
    let initial_generation = int_column(initial, "generation");
    let final_generation = int_column(final_people, "generation");

    assert!(initial_event.iter().all(|&event| event == 0));
    let mut initial_population = [0_i64; 8];
    let mut final_population = [0_i64; 8];
    let mut births = [0_i64; 8];
    let mut deaths = [0_i64; 8];
    let mut arrivals = [0_i64; 8];
    let mut departures = [0_i64; 8];
    let mut move_in = [0_i64; 8];
    let mut move_out = [0_i64; 8];
    let mut event_counts = [0_i64; 6];

    for row in 0..ROWS {
        let before_present = initial_occupancy[row] == 1;
        let after_present = final_occupancy[row] == 1;
        if before_present {
            initial_population[initial_area[row] as usize] += 1;
        }
        if after_present {
            final_population[final_area[row] as usize] += 1;
            assert!(final_age[row] >= 0);
        }
        let event = final_event[row] as usize;
        event_counts[event] += 1;
        match event {
            0 => {
                assert_eq!(before_present, after_present);
                assert_eq!(final_previous[row], 0);
                assert_eq!(initial_generation[row], final_generation[row]);
                if before_present {
                    assert_eq!(final_age[row], initial_age[row] + 1);
                } else {
                    assert_eq!(final_age[row], initial_age[row]);
                }
            }
            1 => {
                assert!(!before_present && after_present);
                births[final_area[row] as usize] += 1;
                assert_eq!(initial_stream[row], 0);
                assert_eq!(final_stream[row], 0);
                assert_eq!(final_age[row], 0);
                assert_eq!(final_generation[row], initial_generation[row] + 1);
            }
            2 => {
                assert!(before_present && !after_present);
                deaths[initial_area[row] as usize] += 1;
                assert_eq!(final_stream[row], 2);
                assert_eq!(initial_generation[row], final_generation[row]);
            }
            3 => {
                assert!(!before_present && after_present);
                arrivals[final_area[row] as usize] += 1;
                assert_eq!(initial_stream[row], 1);
                assert_eq!(final_stream[row], 1);
                assert_eq!(final_age[row], entry_age[row]);
                assert_eq!(final_generation[row], initial_generation[row] + 1);
            }
            4 => {
                assert!(before_present && !after_present);
                departures[initial_area[row] as usize] += 1;
                assert_eq!(final_stream[row], 2);
                assert_eq!(initial_generation[row], final_generation[row]);
            }
            5 => {
                assert!(before_present && after_present);
                assert_ne!(initial_area[row], final_area[row]);
                move_out[initial_area[row] as usize] += 1;
                move_in[final_area[row] as usize] += 1;
                assert_eq!(final_previous[row], initial_area[row] + 1);
                assert_eq!(final_generation[row], initial_generation[row]);
                assert_eq!(final_age[row], initial_age[row] + 1);
            }
            other => panic!("unknown event ordinal {other}"),
        }
        if event != 5 {
            assert_eq!(final_previous[row], 0);
        }
    }

    assert!(event_counts[1..].iter().all(|&count| count > 0));
    let total_births = births.iter().sum::<i64>();
    let total_deaths = deaths.iter().sum::<i64>();
    let total_arrivals = arrivals.iter().sum::<i64>();
    let total_departures = departures.iter().sum::<i64>();
    let total_move_in = move_in.iter().sum::<i64>();
    let total_move_out = move_out.iter().sum::<i64>();
    assert_eq!(total_move_in, total_move_out);
    assert_eq!(total_move_in, event_counts[5]);
    assert_eq!(
        final_population.iter().sum::<i64>(),
        initial_population.iter().sum::<i64>() + total_births - total_deaths + total_arrivals
            - total_departures
    );
    for region in 0..8 {
        assert_eq!(
            final_population[region],
            initial_population[region] + births[region] - deaths[region] + arrivals[region]
                - departures[region]
                + move_in[region]
                - move_out[region]
        );
    }

    let present = final_occupancy.iter().filter(|&&value| value == 1).count();
    let eligible_vacant = final_occupancy
        .iter()
        .zip(final_stream)
        .filter(|(occupancy, stream)| **occupancy == 0 && **stream < 2)
        .count();
    let retired_vacant = final_occupancy
        .iter()
        .zip(final_stream)
        .filter(|(occupancy, stream)| **occupancy == 0 && **stream == 2)
        .count();
    assert_eq!(present + eligible_vacant + retired_vacant, ROWS);

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn activated_entrants_clear_then_exit_to_retired_without_reuse() {
    let temp = temp_dir("retirement-lifecycle");
    let model_source = std::fs::read_to_string(repository_path(MODEL)).unwrap();
    let model_json: serde_json::Value = serde_json::from_str(&model_source).unwrap();
    let mut parameters = serde_json::Map::new();
    for parameter in model_json["params"].as_array().unwrap() {
        let name = parameter["name"].as_str().unwrap();
        let value = if name == "interstate_base" || name.starts_with("emigration_") {
            serde_json::json!(0.0)
        } else if name.starts_with("birth_rate_")
            || name.starts_with("mortality_")
            || name.starts_with("overseas_arrival_")
        {
            serde_json::json!(1e300)
        } else {
            parameter["default"]["value"].clone()
        };
        parameters.insert(name.to_owned(), value);
    }
    let parameter_path = temp.join("forced-lifecycle.json");
    std::fs::write(
        &parameter_path,
        serde_json::to_vec(&serde_json::Value::Object(parameters)).unwrap(),
    )
    .unwrap();

    let mut states = Vec::new();
    for ticks in 1..=3 {
        let output = temp.join(format!("tick-{ticks}.csv"));
        let final_state = temp.join(format!("tick-{ticks}.state"));
        let process = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(repository_path(PLAN))
            .arg("--population")
            .arg(repository_path(STATE))
            .args(["--seed", "97", "--ticks", &ticks.to_string(), "--out"])
            .arg(&output)
            .arg("--export-state")
            .arg(&final_state)
            .arg("--params")
            .arg(&parameter_path)
            .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
            .output()
            .unwrap();
        assert!(
            process.status.success(),
            "stdout={}\nstderr={}",
            String::from_utf8_lossy(&process.stdout),
            String::from_utf8_lossy(&process.stderr)
        );
        states.push(final_state);
    }

    let model = validated_model();
    let initial_artifact = read(repository_path(STATE)).unwrap();
    let initial_tables = to_table_inits(&initial_artifact, &model).unwrap();
    let tick_tables = states
        .iter()
        .map(|path| {
            let artifact = read(path).unwrap();
            to_table_inits(&artifact, &model).unwrap()
        })
        .collect::<Vec<_>>();
    let initial = table(&initial_tables, "person_slot");
    let tick_one = table(&tick_tables[0], "person_slot");
    let tick_two = table(&tick_tables[1], "person_slot");
    let tick_three = table(&tick_tables[2], "person_slot");
    let initial_occupancy = enum_column(initial, "occupancy");
    let initial_stream = enum_column(initial, "entry_stream");
    let initial_generation = int_column(initial, "generation");

    for row in 0..ROWS {
        let occupancies =
            [tick_one, tick_two, tick_three].map(|people| enum_column(people, "occupancy")[row]);
        let events =
            [tick_one, tick_two, tick_three].map(|people| enum_column(people, "event")[row]);
        let streams =
            [tick_one, tick_two, tick_three].map(|people| enum_column(people, "entry_stream")[row]);
        let generations =
            [tick_one, tick_two, tick_three].map(|people| int_column(people, "generation")[row]);
        if initial_occupancy[row] == 1 {
            assert_eq!(occupancies, [0, 0, 0]);
            assert_eq!(events, [2, 0, 0]);
            assert_eq!(streams, [2, 2, 2]);
            assert_eq!(generations, [0, 0, 0]);
        } else {
            assert!(initial_stream[row] < 2);
            assert_eq!(occupancies, [1, 1, 0]);
            assert_eq!(events[0], if initial_stream[row] == 0 { 1 } else { 3 });
            assert_eq!(events[1], 0);
            assert_eq!(events[2], 2);
            assert_eq!(streams[0], initial_stream[row]);
            assert_eq!(streams[1], initial_stream[row]);
            assert_eq!(streams[2], 2);
            assert_eq!(generations, [1, 1, 1]);
            assert_eq!(initial_generation[row], 0);
        }
    }
    assert!(enum_column(tick_three, "occupancy")
        .iter()
        .all(|&value| value == 0));
    assert!(enum_column(tick_three, "entry_stream")
        .iter()
        .all(|&value| value == 2));
    assert!(int_column(tick_three, "generation")
        .iter()
        .all(|&value| value <= 1));

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn companion_and_hash_are_frozen_cross_language_evidence() {
    let mut canonical: serde_json::Value =
        serde_json::from_slice(&std::fs::read(repository_path(MODEL)).unwrap()).unwrap();
    let companion: serde_json::Value =
        serde_json::from_slice(&std::fs::read(repository_path(COMPANION)).unwrap()).unwrap();
    assert!(!canonical["boxes"][0]["grouped_views"]
        .as_array()
        .unwrap()
        .is_empty());
    canonical["boxes"][0]["grouped_views"] = serde_json::json!([]);
    assert_eq!(canonical, companion);

    let hash = state_artifact_hash(repository_path(STATE)).unwrap();
    assert_eq!(hash.algorithm, "sha256");
    assert_eq!(hash.domain, "sembla.state-artifact/v1");
    assert_eq!(
        hash.digest,
        "c7db0d7324aecd9a50a3d297e604f71da8677058c20ae9b42f8fd7524a136df4"
    );
}

#[test]
#[ignore = "explicit full/tenth artifact regeneration and Rust row-enforcement evidence"]
fn generated_full_and_tenth_artifacts_match_hashes_and_enforce_rows() {
    for (scale, rows, expected_hash) in [
        (
            "full",
            35_245_914_usize,
            "ef3c93722199cf96b4d7839d79561de1f4c0e944924c4a9d490e64ba6a92c083",
        ),
        (
            "tenth",
            3_524_592_usize,
            "926fc80a0330c764cac8c4a69c09dee6b4d088653dc9bf830c9e068a2b87c02a",
        ),
    ] {
        let state_path = repository_path(format!(
            "data/abs/generated/australian_population_2010_{scale}.state"
        ));
        let model_path = PathBuf::from(format!("{}.model.json", state_path.display()));
        assert!(state_path.exists(), "regenerate {}", state_path.display());
        assert!(model_path.exists(), "regenerate {}", model_path.display());

        let source = std::fs::read_to_string(&model_path).unwrap();
        let raw = sembla_ir::parse_json(&source).unwrap();
        let model = sembla_ir::validate(raw.clone()).unwrap();
        let artifact = read(&state_path).unwrap();
        let inits = to_table_inits(&artifact, &model).unwrap();
        assert_eq!(table(&inits, "person_slot").row_count, rows);
        assert_eq!(table(&inits, "slot_resource").row_count, rows);
        drop(inits);

        let hash = state_artifact_hash(&state_path).unwrap();
        assert_eq!(hash.digest, expected_hash, "{scale}");

        for table_index in 0..raw.boxes[0].tables.len() {
            let mut wrong = raw.clone();
            wrong.boxes[0].tables[table_index].size_hint += 1;
            let table_name = wrong.boxes[0].tables[table_index].name.clone();
            let wrong = sembla_ir::validate(wrong).unwrap();
            assert!(
                to_table_inits(&artifact, &wrong).is_err(),
                "{scale} accepted a wrong {table_name} row declaration"
            );
        }
    }
}
