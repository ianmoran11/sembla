use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use sembla_ir::{FeatureSet, ParsedInput, GROUPED_OBSERVATIONS_FEATURE};
use sembla_runtime::state::{ColumnData, ColumnInit, TableInit};
use sembla_runtime::state_artifact::write;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

const MODEL: &str = "fixtures/demographic/demographic_slots.json";
const PLAN: &str = "fixtures/demographic/demographic_slots.plan.json";
const STATE: &str = "fixtures/state/demographic_slots.state";
const GOLDENS: &str = "fixtures/demographic/goldens";
const SEED: &str = "7007";
const TICKS: &str = "24";
const INITIAL_POPULATION: i64 = 4_000;

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn temp_dir(label: &str) -> PathBuf {
    let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "sembla-demographic-{label}-{}-{sequence}",
        std::process::id()
    ));
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

fn enabled_features() -> FeatureSet {
    FeatureSet::from([GROUPED_OBSERVATIONS_FEATURE.to_owned()])
}

fn validated_model() -> sembla_ir::ValidatedModel {
    let source = std::fs::read_to_string(repository_path(MODEL)).unwrap();
    let model = sembla_ir::parse_json(&source).unwrap();
    sembla_ir::validate_with_features(model, &enabled_features()).unwrap()
}

/// Deliberately non-scientific, deterministic arithmetic synthesis.
fn demographic_tables() -> Vec<TableInit> {
    let slots = 5_000_usize;
    let present = 4_000_usize;
    let occupancy = (0..slots)
        .map(|row| if row < present { 1_u16 } else { 0_u16 })
        .collect();
    let event = vec![0_u16; slots];
    let sex = (0..slots).map(|row| (row % 2) as u16).collect();
    let age_months = (0..slots)
        .map(|row| {
            if row < present {
                ((row * 37) % 1_081) as i64
            } else {
                0
            }
        })
        .collect();
    let event_age_months = vec![0_i64; slots];
    let generation = (0..slots)
        .map(|row| if row < present { 1_i64 } else { 0_i64 })
        .collect();
    let entry_stream = vec![0_u16; slots];
    let entry_age_months = vec![0_i64; slots];
    let area = (0..slots).map(|row| (row % 4) as u32).collect();
    let slot_resource = (0..slots).map(|row| row as u32).collect();

    vec![
        TableInit::new(
            "demographic",
            "person_slot",
            slots,
            vec![
                ColumnInit::new("occupancy", ColumnData::Enum(occupancy)),
                ColumnInit::new("event", ColumnData::Enum(event)),
                ColumnInit::new("sex", ColumnData::Enum(sex)),
                ColumnInit::new("age_months", ColumnData::Int(age_months)),
                ColumnInit::new("event_age_months", ColumnData::Int(event_age_months)),
                ColumnInit::new("generation", ColumnData::Int(generation)),
                ColumnInit::new("entry_stream", ColumnData::Enum(entry_stream)),
                ColumnInit::new("entry_age_months", ColumnData::Int(entry_age_months)),
                ColumnInit::new("area", ColumnData::Ref(area)),
                ColumnInit::new("slot_resource", ColumnData::Ref(slot_resource)),
            ],
        ),
        TableInit::new(
            "demographic",
            "area",
            4,
            vec![ColumnInit::new(
                "area_key",
                ColumnData::Int(vec![0, 1, 2, 3]),
            )],
        ),
        TableInit::new("demographic", "slot_resource", slots, Vec::new()),
    ]
}

fn write_synthesized_state(path: &Path) {
    write(path, &validated_model(), &demographic_tables()).unwrap();
}

fn run_path(directory: &Path) -> PathBuf {
    directory.join("run.csv")
}

fn summaries_path(run: &Path) -> PathBuf {
    PathBuf::from(format!("{}.summaries.csv", run.display()))
}

fn manifest_path(run: &Path) -> PathBuf {
    PathBuf::from(format!("{}.manifest.json", run.display()))
}

fn grouped_for_run(run: &Path, view: &str) -> PathBuf {
    let stem = run.file_stem().and_then(|value| value.to_str()).unwrap();
    run.parent()
        .unwrap()
        .join(format!("{stem}.grouped.{view}.csv"))
}

fn grouped_path(directory: &Path, view: &str) -> PathBuf {
    grouped_for_run(&run_path(directory), view)
}

fn run_demographic(directory: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path(MODEL))
        .arg("--population")
        .arg(repository_path(STATE))
        .args(["--seed", SEED, "--ticks", TICKS, "--out"])
        .arg(run_path(directory))
        .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
        .output()
        .unwrap()
}

fn normalized_manifest(path: &Path) -> Vec<u8> {
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    serde_json::to_vec(&value).unwrap()
}

fn write_golden_files(directory: &Path, output: &Output) {
    let goldens = repository_path(GOLDENS);
    std::fs::create_dir_all(&goldens).unwrap();
    let run = run_path(directory);
    for (actual, name) in [
        (run.clone(), "run.csv"),
        (summaries_path(&run), "run.csv.summaries.csv"),
        (
            grouped_path(directory, "population_cells"),
            "run.grouped.population_cells.csv",
        ),
        (
            grouped_path(directory, "deaths_cells"),
            "run.grouped.deaths_cells.csv",
        ),
    ] {
        std::fs::copy(actual, goldens.join(name)).unwrap();
    }
    std::fs::write(
        goldens.join("run.manifest.normalized.json"),
        normalized_manifest(&manifest_path(&run)),
    )
    .unwrap();
    std::fs::write(goldens.join("run.hashes.txt"), &output.stdout).unwrap();
}

fn csv_rows(path: &Path) -> Vec<BTreeMap<String, i64>> {
    let source = std::fs::read_to_string(path).unwrap();
    let mut lines = source.lines().filter(|line| !line.starts_with('#'));
    let headers = lines
        .next()
        .unwrap()
        .split(',')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    lines
        .map(|line| {
            headers
                .iter()
                .cloned()
                .zip(line.split(',').map(|value| value.parse::<i64>().unwrap()))
                .collect()
        })
        .collect()
}

fn summary_values() -> BTreeMap<String, i64> {
    let source =
        std::fs::read_to_string(repository_path(GOLDENS).join("run.csv.summaries.csv")).unwrap();
    source
        .lines()
        .skip(1)
        .map(|line| {
            let mut fields = line.split(',');
            (
                fields.next().unwrap().to_owned(),
                fields.next().unwrap().parse().unwrap(),
            )
        })
        .collect()
}

fn golden_rows() -> Vec<BTreeMap<String, i64>> {
    csv_rows(&repository_path(GOLDENS).join("run.csv"))
}

fn grouped_totals(view: &str) -> BTreeMap<i64, i64> {
    let source =
        std::fs::read_to_string(repository_path(GOLDENS).join(format!("run.grouped.{view}.csv")))
            .unwrap();
    let mut totals = BTreeMap::new();
    for line in source.lines().skip(1) {
        let fields = line.split(',').collect::<Vec<_>>();
        *totals.entry(fields[0].parse().unwrap()).or_default() +=
            fields.last().unwrap().parse::<i64>().unwrap();
    }
    totals
}

fn write_params(path: &Path, mortality_young: f64, mortality_adult: f64, mortality_old: f64) {
    std::fs::write(
        path,
        serde_json::to_vec(&serde_json::json!({
            "birth_rate": 0.025,
            "mortality_young": mortality_young,
            "mortality_adult": mortality_adult,
            "mortality_old": mortality_old
        }))
        .unwrap(),
    )
    .unwrap();
}

fn run_window(
    population: &Path,
    params: &Path,
    seed: u64,
    out: &Path,
    exported_state: &Path,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("run")
        .arg(repository_path(MODEL))
        .arg("--population")
        .arg(population)
        .arg("--params")
        .arg(params)
        .arg("--seed")
        .arg(seed.to_string())
        .args(["--ticks", "12", "--out"])
        .arg(out)
        .arg("--export-state")
        .arg(exported_state)
        .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
        .output()
        .unwrap()
}

#[test]
fn canonical_model_plan_and_state_fixtures_are_exact() {
    let model_source = std::fs::read_to_string(repository_path(MODEL)).unwrap();
    let model = sembla_ir::parse_json(&model_source).unwrap();
    assert_eq!(
        model_source,
        sembla_ir::to_canonical_json(&model).unwrap(),
        "Lean model export must be Rust-canonical"
    );
    sembla_ir::validate_with_features(model, &enabled_features()).unwrap();

    let plan_source = std::fs::read_to_string(repository_path(PLAN)).unwrap();
    let ParsedInput::Plan(plan) = sembla_ir::parse_input(&plan_source).unwrap() else {
        panic!("demographic plan fixture must be a plan")
    };
    sembla_ir::validate_plan(&plan).unwrap();
    assert_eq!(
        plan_source,
        sembla_ir::to_canonical_string(&plan).unwrap(),
        "Lean plan export must be Rust-canonical"
    );
    assert_eq!(
        plan.identity.enabled_features,
        [GROUPED_OBSERVATIONS_FEATURE]
    );

    let temp = temp_dir("state-parity");
    let regenerated = temp.join("demographic_slots.state");
    write_synthesized_state(&regenerated);
    assert_eq!(
        std::fs::read(regenerated).unwrap(),
        std::fs::read(repository_path(STATE)).unwrap(),
        "state fixture must match deterministic non-scientific synthesis"
    );
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
#[ignore = "fixture regeneration is explicit and must not run during normal checks"]
fn regenerate_demographic_state_fixture() {
    write_synthesized_state(&repository_path(STATE));
}

#[test]
#[ignore = "golden regeneration is explicit and must not run during normal checks"]
fn regenerate_demographic_goldens() {
    let temp = temp_dir("golden-regeneration");
    let output = run_demographic(&temp);
    assert_success(&output);
    write_golden_files(&temp, &output);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn stock_flow_identity_holds_at_every_tick() {
    let rows = golden_rows();
    assert_eq!(rows.len(), 24);
    let mut previous = INITIAL_POPULATION;
    for row in rows {
        assert_eq!(
            row["population"],
            previous + row["births_this_tick"] - row["deaths_this_tick"],
            "tick {}",
            row["tick"]
        );
        previous = row["population"];
    }
}

#[test]
fn marker_totals_cross_check_summaries() {
    let rows = golden_rows();
    let summaries = summary_values();
    assert_eq!(
        summaries["births_total"],
        rows.iter().map(|row| row["births_this_tick"]).sum::<i64>()
    );
    assert_eq!(
        summaries["deaths_total"],
        rows.iter().map(|row| row["deaths_this_tick"]).sum::<i64>()
    );
}

#[test]
fn no_present_slot_has_an_invalid_age() {
    let rows = golden_rows();
    let summaries = summary_values();
    assert!(rows.iter().all(|row| row["invalid_age"] == 0));
    assert_eq!(summaries["maximum_invalid_age_count"], 0);
}

#[test]
fn death_is_the_single_exit_and_never_exceeds_prior_stock() {
    let rows = golden_rows();
    let mut previous = INITIAL_POPULATION;
    for row in rows {
        assert!(row["deaths_this_tick"] <= previous, "tick {}", row["tick"]);
        previous = row["population"];
    }
}

#[test]
fn grouped_population_and_death_cells_match_scalar_views() {
    let population = grouped_totals("population_cells");
    let deaths = grouped_totals("deaths_cells");
    for row in golden_rows() {
        assert_eq!(population[&row["tick"]], row["population"]);
        assert_eq!(
            deaths.get(&row["tick"]).copied().unwrap_or_default(),
            row["deaths_this_tick"]
        );
    }
}

#[test]
fn slot_reuse_strictly_increases_generation() {
    let rows = golden_rows();
    let summaries = summary_values();
    assert_eq!(rows[0]["max_generation"], 1);
    assert!(summaries["final_max_generation"] >= 2);
    assert_eq!(
        summaries["final_max_generation"],
        rows.last().unwrap()["max_generation"]
    );
}

#[test]
fn one_tick_entry_lockout_is_measured() {
    let rows = golden_rows();
    let summaries = summary_values();
    assert!(rows
        .iter()
        .all(|row| { row["locked_out"] == row["births_this_tick"] }));
    assert_eq!(
        summaries["locked_out_total"],
        rows.iter().map(|row| row["locked_out"]).sum::<i64>()
    );
    assert!(summaries["locked_out_total"] > 0);
}

#[test]
fn golden_run_reproduces_bitwise_twice() {
    let first = temp_dir("golden-first");
    let second = temp_dir("golden-second");
    let first_output = run_demographic(&first);
    let second_output = run_demographic(&second);
    assert_success(&first_output);
    assert_success(&second_output);
    assert_eq!(first_output.stdout, second_output.stdout);

    for (name, first_actual, second_actual) in [
        ("run.csv", run_path(&first), run_path(&second)),
        (
            "run.csv.summaries.csv",
            summaries_path(&run_path(&first)),
            summaries_path(&run_path(&second)),
        ),
        (
            "run.grouped.population_cells.csv",
            grouped_path(&first, "population_cells"),
            grouped_path(&second, "population_cells"),
        ),
        (
            "run.grouped.deaths_cells.csv",
            grouped_path(&first, "deaths_cells"),
            grouped_path(&second, "deaths_cells"),
        ),
    ] {
        let expected = std::fs::read(repository_path(GOLDENS).join(name)).unwrap();
        let first_bytes = std::fs::read(first_actual).unwrap();
        let second_bytes = std::fs::read(second_actual).unwrap();
        assert_eq!(first_bytes, expected, "{name} differs from golden");
        assert_eq!(second_bytes, expected, "{name} repeated run differs");
    }
    assert_eq!(
        normalized_manifest(&manifest_path(&run_path(&first))),
        std::fs::read(repository_path(GOLDENS).join("run.manifest.normalized.json")).unwrap()
    );
    assert_eq!(
        normalized_manifest(&manifest_path(&run_path(&second))),
        std::fs::read(repository_path(GOLDENS).join("run.manifest.normalized.json")).unwrap()
    );
    assert_eq!(
        first_output.stdout,
        std::fs::read(repository_path(GOLDENS).join("run.hashes.txt")).unwrap()
    );

    std::fs::remove_dir_all(first).unwrap();
    std::fs::remove_dir_all(second).unwrap();
}

#[test]
fn demographic_model_chains_two_deterministic_mortality_windows() {
    let temp = temp_dir("chain");
    let params_a = temp.join("window-a.json");
    let params_b = temp.join("window-b.json");
    write_params(&params_a, 0.001, 0.003, 0.012);
    write_params(&params_b, 0.002, 0.006, 0.024);

    let a_out = temp.join("a.csv");
    let a_state = temp.join("a.state");
    assert_success(&run_window(
        &repository_path(STATE),
        &params_a,
        7101,
        &a_out,
        &a_state,
    ));

    let b_out = temp.join("b.csv");
    let b_state = temp.join("b.state");
    let b_repeat_out = temp.join("b-repeat.csv");
    let b_repeat_state = temp.join("b-repeat.state");
    assert_success(&run_window(&a_state, &params_b, 7202, &b_out, &b_state));
    assert_success(&run_window(
        &a_state,
        &params_b,
        7202,
        &b_repeat_out,
        &b_repeat_state,
    ));

    for (left, right) in [
        (b_out.clone(), b_repeat_out.clone()),
        (summaries_path(&b_out), summaries_path(&b_repeat_out)),
        (
            grouped_for_run(&b_out, "population_cells"),
            grouped_for_run(&b_repeat_out, "population_cells"),
        ),
        (
            grouped_for_run(&b_out, "deaths_cells"),
            grouped_for_run(&b_repeat_out, "deaths_cells"),
        ),
        (b_state.clone(), b_repeat_state.clone()),
    ] {
        assert_eq!(std::fs::read(left).unwrap(), std::fs::read(right).unwrap());
    }

    let manifest_a: serde_json::Value =
        serde_json::from_slice(&std::fs::read(manifest_path(&a_out)).unwrap()).unwrap();
    let manifest_b: serde_json::Value =
        serde_json::from_slice(&std::fs::read(manifest_path(&b_out)).unwrap()).unwrap();
    let manifest_b_repeat: serde_json::Value =
        serde_json::from_slice(&std::fs::read(manifest_path(&b_repeat_out)).unwrap()).unwrap();
    assert_eq!(
        manifest_a["exported_state"]["hash"],
        manifest_b["initial_state"]["hash"]
    );
    assert_eq!(manifest_b, manifest_b_repeat);
    assert_ne!(
        manifest_a["resolved_theta"]["mortality_old"],
        manifest_b["resolved_theta"]["mortality_old"]
    );

    std::fs::remove_dir_all(temp).unwrap();
}
