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
const CALIBRATION: &str = "fixtures/demographic/calibration";
const CALIBRATION_GOLDENS: &str = "fixtures/demographic/calibration/goldens";
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
    let entry_stream = (0..slots)
        .map(|row| {
            if row < present || row < present + 600 {
                0_u16
            } else if row < present + 850 {
                1_u16
            } else {
                2_u16
            }
        })
        .collect();
    let entry_age_months = (0..slots)
        .map(|row| {
            if row < present + 600 {
                0_i64
            } else if row < present + 850 {
                let offset = row - (present + 600);
                if offset < 12 {
                    1_140_i64 + offset as i64
                } else {
                    (216 + (offset * 29) % 840) as i64
                }
            } else {
                let offset = row - (present + 850);
                (60 + (offset * 17) % 901) as i64
            }
        })
        .collect();
    let area = (0..slots).map(|row| (row % 4) as u32).collect();
    let slot_resource = (0..slots).map(|row| row as u32).collect();

    vec![
        TableInit::new(
            "demographic",
            "area",
            4,
            vec![ColumnInit::new(
                "area_key",
                ColumnData::Int(vec![0, 1, 2, 3]),
            )],
        ),
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

fn run_calibration_sweep(directory: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("sweep")
        .arg(repository_path(PLAN))
        .arg("--population")
        .arg(repository_path(STATE))
        .args(["--seed", "7505", "--ticks", "12", "--theta-file"])
        .arg(repository_path(CALIBRATION).join("theta-grid.json"))
        .arg("--noise")
        .arg("independent")
        .arg("--out")
        .arg(directory.join("sweep"))
        .arg("--export-pairs")
        .arg(directory.join("pairs.csv"))
        .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
        .output()
        .unwrap()
}

fn run_calibration_compare(out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sembla"))
        .arg("compare")
        .arg(repository_path(PLAN))
        .arg("--population")
        .arg(repository_path(STATE))
        .args(["--seed", "7404", "--ticks", "12", "--params-a"])
        .arg(repository_path(CALIBRATION).join("low-migration.json"))
        .arg("--params-b")
        .arg(repository_path(CALIBRATION).join("high-migration.json"))
        .arg("--out")
        .arg(out)
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
        (
            grouped_path(directory, "vacancy_cells"),
            "run.grouped.vacancy_cells.csv",
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

fn write_calibration_goldens(directory: &Path) {
    let goldens = repository_path(CALIBRATION_GOLDENS);
    std::fs::create_dir_all(&goldens).unwrap();
    let sweep = directory.join("sweep");
    for (actual, name) in [
        (sweep.join("manifest.csv"), "sweep.manifest.csv"),
        (sweep.join("summary.csv"), "sweep.summary.csv"),
        (sweep.join("draw_0.csv"), "sweep.draw_0.csv"),
        (
            sweep.join("draw_0.csv.summaries.csv"),
            "sweep.draw_0.csv.summaries.csv",
        ),
        (
            sweep.join("draw_0.grouped.population_cells.csv"),
            "sweep.draw_0.grouped.population_cells.csv",
        ),
        (
            sweep.join("draw_0.grouped.deaths_cells.csv"),
            "sweep.draw_0.grouped.deaths_cells.csv",
        ),
        (
            sweep.join("draw_0.grouped.vacancy_cells.csv"),
            "sweep.draw_0.grouped.vacancy_cells.csv",
        ),
        (directory.join("pairs.csv"), "pairs.csv"),
        (directory.join("compare.csv"), "compare.csv"),
    ] {
        std::fs::copy(actual, goldens.join(name)).unwrap();
    }
    std::fs::write(
        goldens.join("sweep.run-manifest.normalized.json"),
        normalized_manifest(&sweep.join("run-manifest.json")),
    )
    .unwrap();
    std::fs::write(
        goldens.join("pairs.meta.normalized.json"),
        normalized_manifest(&directory.join("pairs.csv.meta.json")),
    )
    .unwrap();
    std::fs::write(
        goldens.join("compare.manifest.normalized.json"),
        normalized_manifest(&manifest_path(&directory.join("compare.csv"))),
    )
    .unwrap();
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

#[allow(clippy::too_many_arguments)]
fn write_params(
    path: &Path,
    mortality_young: f64,
    mortality_adult: f64,
    mortality_old: f64,
    overseas_arrival_rate: f64,
    emigration_rate: f64,
    internal_departure_rate: f64,
    internal_arrival_rate: f64,
) {
    std::fs::write(
        path,
        serde_json::to_vec(&serde_json::json!({
            "birth_rate": 0.025,
            "mortality_young": mortality_young,
            "mortality_adult": mortality_adult,
            "mortality_old": mortality_old,
            "overseas_arrival_rate": overseas_arrival_rate,
            "emigration_rate": emigration_rate,
            "internal_departure_rate": internal_departure_rate,
            "internal_arrival_rate": internal_arrival_rate
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
        .arg(repository_path(PLAN))
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

    let tables = demographic_tables();
    let person = tables
        .iter()
        .find(|table| table.table_name == "person_slot")
        .unwrap();
    let ColumnData::Enum(entry_stream) = &person
        .columns
        .iter()
        .find(|column| column.name == "entry_stream")
        .unwrap()
        .data
    else {
        panic!("entry_stream must be enum")
    };
    assert_eq!(entry_stream[..4_000], [0_u16; 4_000]);
    assert_eq!(
        entry_stream[4_000..]
            .iter()
            .fold([0_usize; 3], |mut counts, stream| {
                counts[usize::from(*stream)] += 1;
                counts
            }),
        [600, 250, 150]
    );
    let ColumnData::Int(initial_ages) = &person
        .columns
        .iter()
        .find(|column| column.name == "age_months")
        .unwrap()
        .data
    else {
        panic!("age_months must be int")
    };
    assert!(initial_ages[..4_000].iter().all(|age| *age < 1_140));
    let ColumnData::Int(entry_ages) = &person
        .columns
        .iter()
        .find(|column| column.name == "entry_age_months")
        .unwrap()
        .data
    else {
        panic!("entry_age_months must be int")
    };
    assert!(entry_ages[4_600..4_850].iter().any(|age| *age > 0));
    assert!(entry_ages[4_850..].iter().any(|age| *age > 0));

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
#[ignore = "calibration regeneration is explicit and must not run during normal checks"]
fn regenerate_demographic_calibration_goldens() {
    let temp = temp_dir("calibration-regeneration");
    let sweep = run_calibration_sweep(&temp);
    assert_success(&sweep);
    let compare = run_calibration_compare(&temp.join("compare.csv"));
    assert_success(&compare);
    write_calibration_goldens(&temp);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
#[ignore = "chain golden regeneration is explicit and must not run during normal checks"]
fn regenerate_demographic_chain_goldens() {
    let temp = temp_dir("chain-regeneration");
    let params_a = temp.join("window-a.json");
    let params_b = temp.join("window-b.json");
    write_params(&params_a, 0.001, 0.003, 0.012, 0.02, 0.002, 0.0025, 0.018);
    write_params(&params_b, 0.002, 0.006, 0.024, 0.04, 0.004, 0.005, 0.036);
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
    assert_success(&run_window(&a_state, &params_b, 7202, &b_out, &b_state));
    let goldens = repository_path(CALIBRATION_GOLDENS).join("chain");
    std::fs::create_dir_all(&goldens).unwrap();
    for (actual, name) in [
        (a_out.clone(), "window_a.csv"),
        (summaries_path(&a_out), "window_a.csv.summaries.csv"),
        (
            grouped_for_run(&a_out, "population_cells"),
            "window_a.grouped.population_cells.csv",
        ),
        (
            grouped_for_run(&a_out, "deaths_cells"),
            "window_a.grouped.deaths_cells.csv",
        ),
        (
            grouped_for_run(&a_out, "vacancy_cells"),
            "window_a.grouped.vacancy_cells.csv",
        ),
        (b_out.clone(), "window_b.csv"),
        (summaries_path(&b_out), "window_b.csv.summaries.csv"),
        (
            grouped_for_run(&b_out, "population_cells"),
            "window_b.grouped.population_cells.csv",
        ),
        (
            grouped_for_run(&b_out, "deaths_cells"),
            "window_b.grouped.deaths_cells.csv",
        ),
        (
            grouped_for_run(&b_out, "vacancy_cells"),
            "window_b.grouped.vacancy_cells.csv",
        ),
    ] {
        std::fs::copy(actual, goldens.join(name)).unwrap();
    }
    std::fs::write(
        goldens.join("window_a.manifest.normalized.json"),
        normalized_manifest(&manifest_path(&a_out)),
    )
    .unwrap();
    std::fs::write(
        goldens.join("window_b.manifest.normalized.json"),
        normalized_manifest(&manifest_path(&b_out)),
    )
    .unwrap();
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn full_six_flow_stock_identity_holds_at_every_tick() {
    let rows = golden_rows();
    assert_eq!(rows.len(), 24);
    let mut previous = INITIAL_POPULATION;
    for row in rows {
        assert_eq!(
            row["population"],
            previous + row["births_this_tick"] - row["deaths_this_tick"]
                + row["overseas_arrivals_this_tick"]
                - row["overseas_departures_this_tick"]
                + row["internal_arrivals_this_tick"]
                - row["internal_departures_this_tick"],
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
    assert!(summaries["births_total"] > 0);
    assert_eq!(
        summaries["minimum_vacant_birth_slots"],
        rows.iter()
            .map(|row| row["vacant_birth_slots"])
            .min()
            .unwrap()
    );
    for (summary, column) in [
        ("deaths_total", "deaths_this_tick"),
        ("overseas_arrivals_total", "overseas_arrivals_this_tick"),
        ("overseas_departures_total", "overseas_departures_this_tick"),
        ("internal_arrivals_total", "internal_arrivals_this_tick"),
        ("internal_departures_total", "internal_departures_this_tick"),
    ] {
        let total = rows.iter().map(|row| row[column]).sum::<i64>();
        assert_eq!(summaries[summary], total, "{summary}");
        assert!(total > 0, "{column} must be exercised by the golden run");
    }
}

#[test]
fn no_present_slot_has_an_invalid_age() {
    let rows = golden_rows();
    let summaries = summary_values();
    assert!(rows.iter().all(|row| row["invalid_age"] == 0));
    assert_eq!(summaries["maximum_invalid_age_count"], 0);
}

#[test]
fn internal_balance_residual_is_visible_and_documented() {
    let rows = golden_rows();
    assert!(rows
        .iter()
        .any(|row| { row["internal_arrivals_this_tick"] != row["internal_departures_this_tick"] }));
    let documentation =
        std::fs::read_to_string(repository_path("docs/demographic-model.md")).unwrap();
    assert!(documentation.contains(
        "National internal-migration balance holds only in expectation; the residual is always reported and never silently reconciled."
    ));
}

#[test]
fn arrival_age_stratum_appears_in_population_cells() {
    let source =
        std::fs::read_to_string(repository_path(GOLDENS).join("run.grouped.population_cells.csv"))
            .unwrap();
    let mut lines = source.lines();
    let headers = lines.next().unwrap().split(',').collect::<Vec<_>>();
    let index = |name: &str| headers.iter().position(|value| *value == name).unwrap();
    let tick = index("tick");
    let sex = index("sex");
    let area = index("area");
    let age = index("age_months");
    let count = index("count");
    let matching = lines
        .map(|line| line.split(',').collect::<Vec<_>>())
        .find(|row| row[tick] == "0" && row[sex] == "male" && row[area] == "2" && row[age] == "19")
        .expect("the preclassified high-age overseas stratum must gain mass at tick 0");
    assert_eq!(matching[count], "1");
    assert!(golden_rows()[0]["overseas_arrivals_this_tick"] > 0);
}

#[test]
fn competing_exits_and_slot_accounting_are_closed() {
    let rows = golden_rows();
    let mut previous = INITIAL_POPULATION;
    let mut cumulative_entries = 0_i64;
    let mut cumulative_exits = 0_i64;
    for row in rows {
        let entries = row["births_this_tick"]
            + row["overseas_arrivals_this_tick"]
            + row["internal_arrivals_this_tick"];
        let exits = row["deaths_this_tick"]
            + row["overseas_departures_this_tick"]
            + row["internal_departures_this_tick"];
        assert!(exits <= previous, "tick {}", row["tick"]);
        cumulative_entries += entries;
        cumulative_exits += exits;
        assert_eq!(
            INITIAL_POPULATION + cumulative_entries,
            row["population"] + cumulative_exits,
            "logical people at tick {}",
            row["tick"]
        );
        assert_eq!(
            row["population"]
                + row["vacant_birth_slots"]
                + row["vacant_overseas_slots"]
                + row["vacant_internal_slots"]
                + exits,
            5_000,
            "physical slots at tick {}",
            row["tick"]
        );
        previous = row["population"];
    }
    assert!(golden_rows().iter().any(|row| row["deferred_total"] > 0));
}

#[test]
fn grouped_population_death_and_vacancy_cells_match_scalar_views() {
    let population = grouped_totals("population_cells");
    let deaths = grouped_totals("deaths_cells");
    let vacancies = grouped_totals("vacancy_cells");
    for row in golden_rows() {
        assert_eq!(population[&row["tick"]], row["population"]);
        assert_eq!(
            deaths.get(&row["tick"]).copied().unwrap_or_default(),
            row["deaths_this_tick"]
        );
        assert_eq!(
            vacancies.get(&row["tick"]).copied().unwrap_or_default(),
            row["vacant_birth_slots"] + row["vacant_overseas_slots"] + row["vacant_internal_slots"]
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
    assert!(rows.iter().all(|row| {
        row["locked_out"]
            == row["births_this_tick"]
                + row["overseas_arrivals_this_tick"]
                + row["internal_arrivals_this_tick"]
    }));
    assert_eq!(
        summaries["locked_out_total"],
        rows.iter().map(|row| row["locked_out"]).sum::<i64>()
    );
    assert!(summaries["locked_out_total"] > 0);
}

#[test]
fn overseas_capacity_exhaustion_is_visible_and_deterministic() {
    let temp = temp_dir("saturation");
    let params = temp.join("params.json");
    write_params(&params, 0.0, 0.0, 0.0, 1e300, 0.0, 0.0, 0.018);
    let outputs = [temp.join("first.csv"), temp.join("second.csv")];
    for out in &outputs {
        let output = Command::new(env!("CARGO_BIN_EXE_sembla"))
            .arg("run")
            .arg(repository_path(MODEL))
            .arg("--population")
            .arg(repository_path(STATE))
            .arg("--params")
            .arg(&params)
            .args(["--seed", "7303", "--ticks", "8", "--out"])
            .arg(out)
            .args(["--enable", GROUPED_OBSERVATIONS_FEATURE])
            .output()
            .unwrap();
        assert_success(&output);
    }
    for (left, right) in [
        (outputs[0].clone(), outputs[1].clone()),
        (summaries_path(&outputs[0]), summaries_path(&outputs[1])),
        (
            grouped_for_run(&outputs[0], "population_cells"),
            grouped_for_run(&outputs[1], "population_cells"),
        ),
        (
            grouped_for_run(&outputs[0], "deaths_cells"),
            grouped_for_run(&outputs[1], "deaths_cells"),
        ),
        (
            grouped_for_run(&outputs[0], "vacancy_cells"),
            grouped_for_run(&outputs[1], "vacancy_cells"),
        ),
    ] {
        assert_eq!(std::fs::read(left).unwrap(), std::fs::read(right).unwrap());
    }
    assert_eq!(
        normalized_manifest(&manifest_path(&outputs[0])),
        normalized_manifest(&manifest_path(&outputs[1]))
    );
    let rows = csv_rows(&outputs[0]);
    let exhausted_at = rows
        .iter()
        .position(|row| row["vacant_overseas_slots"] == 0)
        .expect("overseas capacity must exhaust");
    assert!(rows[exhausted_at..]
        .iter()
        .all(|row| row["vacant_overseas_slots"] == 0));
    assert!(rows[exhausted_at..]
        .iter()
        .any(|row| { row["births_this_tick"] > 0 && row["internal_arrivals_this_tick"] > 0 }));
    assert!(rows.iter().all(|row| row["deferred_total"] == 0));
    let documentation =
        std::fs::read_to_string(repository_path("docs/demographic-model.md")).unwrap();
    assert!(
        documentation.contains("a run that saturates a slot stratum is not calibrated evidence")
    );
    std::fs::remove_dir_all(temp).unwrap();
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
        (
            "run.grouped.vacancy_cells.csv",
            grouped_path(&first, "vacancy_cells"),
            grouped_path(&second, "vacancy_cells"),
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
fn calibration_sweep_pairs_and_crn_compare_match_goldens() {
    let temp = temp_dir("calibration");
    let sweep = run_calibration_sweep(&temp);
    assert_success(&sweep);
    let compare_path = temp.join("compare.csv");
    let compare = run_calibration_compare(&compare_path);
    assert_success(&compare);

    let goldens = repository_path(CALIBRATION_GOLDENS);
    let sweep_dir = temp.join("sweep");
    for (actual, name) in [
        (sweep_dir.join("manifest.csv"), "sweep.manifest.csv"),
        (sweep_dir.join("summary.csv"), "sweep.summary.csv"),
        (sweep_dir.join("draw_0.csv"), "sweep.draw_0.csv"),
        (
            sweep_dir.join("draw_0.csv.summaries.csv"),
            "sweep.draw_0.csv.summaries.csv",
        ),
        (
            sweep_dir.join("draw_0.grouped.population_cells.csv"),
            "sweep.draw_0.grouped.population_cells.csv",
        ),
        (
            sweep_dir.join("draw_0.grouped.deaths_cells.csv"),
            "sweep.draw_0.grouped.deaths_cells.csv",
        ),
        (
            sweep_dir.join("draw_0.grouped.vacancy_cells.csv"),
            "sweep.draw_0.grouped.vacancy_cells.csv",
        ),
        (temp.join("pairs.csv"), "pairs.csv"),
        (compare_path.clone(), "compare.csv"),
    ] {
        assert_eq!(
            std::fs::read(actual).unwrap(),
            std::fs::read(goldens.join(name)).unwrap(),
            "{name} changed"
        );
    }
    assert_eq!(
        normalized_manifest(&sweep_dir.join("run-manifest.json")),
        std::fs::read(goldens.join("sweep.run-manifest.normalized.json")).unwrap()
    );
    assert_eq!(
        normalized_manifest(&temp.join("pairs.csv.meta.json")),
        std::fs::read(goldens.join("pairs.meta.normalized.json")).unwrap()
    );
    assert_eq!(
        normalized_manifest(&manifest_path(&compare_path)),
        std::fs::read(goldens.join("compare.manifest.normalized.json")).unwrap()
    );

    let sweep_manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(sweep_dir.join("run-manifest.json")).unwrap())
            .unwrap();
    assert_eq!(sweep_manifest["noise_mode"], "independent");
    assert_eq!(
        sweep_manifest["enabled_features"],
        serde_json::json!([GROUPED_OBSERVATIONS_FEATURE])
    );
    assert_eq!(sweep_manifest["plan"]["origin"], "direct_stable");
    assert_eq!(
        sweep_manifest["plan"]["enabled_features"],
        serde_json::json!([GROUPED_OBSERVATIONS_FEATURE])
    );
    assert_eq!(sweep_manifest["executions"].as_array().unwrap().len(), 6);
    assert!(sweep_manifest["executions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|execution| execution["grouped_outputs"].as_array().unwrap().len() == 3));

    let pairs = std::fs::read_to_string(temp.join("pairs.csv")).unwrap();
    let pairs_header = pairs.lines().next().unwrap();
    for summary in [
        "overseas_arrivals_total",
        "overseas_departures_total",
        "internal_arrivals_total",
        "internal_departures_total",
    ] {
        assert!(pairs_header.split(',').any(|column| column == summary));
    }

    let compare_manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(manifest_path(&compare_path)).unwrap()).unwrap();
    assert_eq!(
        compare_manifest["enabled_features"],
        serde_json::json!([GROUPED_OBSERVATIONS_FEATURE])
    );
    assert_eq!(compare_manifest["plan"]["origin"], "direct_stable");
    assert_eq!(
        compare_manifest["plan"]["enabled_features"],
        serde_json::json!([GROUPED_OBSERVATIONS_FEATURE])
    );

    let rows = csv_rows(&compare_path);
    let first_diverging_tick = rows
        .iter()
        .position(|row| {
            [
                "dbirths_this_tick",
                "ddeaths_this_tick",
                "doverseas_arrivals_this_tick",
                "doverseas_departures_this_tick",
                "dinternal_arrivals_this_tick",
                "dinternal_departures_this_tick",
            ]
            .iter()
            .any(|column| row[*column] != 0)
        })
        .unwrap();
    assert_eq!(first_diverging_tick, 0);
    let first = &rows[first_diverging_tick];
    // §J4 content-addressed coordinates keep unchanged birth/death/departure
    // draws identical for the same untouched slots. Only the changed arrival
    // rates diverge before their tick-0 commits perturb shared state.
    assert_eq!(first["dbirths_this_tick"], 0);
    assert_eq!(first["ddeaths_this_tick"], 0);
    assert_eq!(first["doverseas_departures_this_tick"], 0);
    assert_eq!(first["dinternal_departures_this_tick"], 0);
    assert_eq!(first["doverseas_arrivals_this_tick"], 7);
    assert_eq!(first["dinternal_arrivals_this_tick"], 2);
    let compare_source = std::fs::read_to_string(&compare_path).unwrap();
    let header = compare_source
        .lines()
        .find(|line| !line.starts_with('#'))
        .unwrap()
        .split(',')
        .collect::<Vec<_>>();
    let arm_width = (header.len() - 1) / 3;
    let first_delta_columns = header[(1 + 2 * arm_width)..]
        .iter()
        .copied()
        .filter(|column| first[*column] != 0)
        .collect::<Vec<_>>();
    assert_eq!(
        first_delta_columns,
        [
            "dage_65_69",
            "dage_85_plus",
            "dfemales",
            "dinternal_arrivals_this_tick",
            "dlocked_out",
            "dmales",
            "doverseas_arrivals_this_tick",
            "dpopulation",
            "dvacant_internal_slots",
            "dvacant_overseas_slots",
            "dfired_internal_arrive",
            "dfired_overseas_arrive",
        ]
    );

    let repeated_compare = temp.join("compare-repeat.csv");
    let repeat = run_calibration_compare(&repeated_compare);
    assert_success(&repeat);
    assert_eq!(
        std::fs::read(&compare_path).unwrap(),
        std::fs::read(&repeated_compare).unwrap()
    );
    assert_eq!(
        normalized_manifest(&manifest_path(&compare_path)),
        normalized_manifest(&manifest_path(&repeated_compare))
    );
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn demographic_model_chains_two_deterministic_migration_windows() {
    let temp = temp_dir("chain");
    let params_a = temp.join("window-a.json");
    let params_b = temp.join("window-b.json");
    write_params(&params_a, 0.001, 0.003, 0.012, 0.02, 0.002, 0.0025, 0.018);
    write_params(&params_b, 0.002, 0.006, 0.024, 0.04, 0.004, 0.005, 0.036);

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
        (
            grouped_for_run(&b_out, "vacancy_cells"),
            grouped_for_run(&b_repeat_out, "vacancy_cells"),
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
    assert_eq!(manifest_a["plan"]["origin"], "direct_stable");
    assert_eq!(
        manifest_a["enabled_features"],
        serde_json::json!([GROUPED_OBSERVATIONS_FEATURE])
    );
    assert_ne!(
        manifest_a["resolved_theta"]["mortality_old"],
        manifest_b["resolved_theta"]["mortality_old"]
    );
    assert_ne!(
        manifest_a["resolved_theta"]["overseas_arrival_rate"],
        manifest_b["resolved_theta"]["overseas_arrival_rate"]
    );

    let chain_goldens = repository_path(CALIBRATION_GOLDENS).join("chain");
    for (actual, name) in [
        (a_out.clone(), "window_a.csv"),
        (summaries_path(&a_out), "window_a.csv.summaries.csv"),
        (
            grouped_for_run(&a_out, "population_cells"),
            "window_a.grouped.population_cells.csv",
        ),
        (
            grouped_for_run(&a_out, "deaths_cells"),
            "window_a.grouped.deaths_cells.csv",
        ),
        (
            grouped_for_run(&a_out, "vacancy_cells"),
            "window_a.grouped.vacancy_cells.csv",
        ),
        (b_out.clone(), "window_b.csv"),
        (summaries_path(&b_out), "window_b.csv.summaries.csv"),
        (
            grouped_for_run(&b_out, "population_cells"),
            "window_b.grouped.population_cells.csv",
        ),
        (
            grouped_for_run(&b_out, "deaths_cells"),
            "window_b.grouped.deaths_cells.csv",
        ),
        (
            grouped_for_run(&b_out, "vacancy_cells"),
            "window_b.grouped.vacancy_cells.csv",
        ),
    ] {
        assert_eq!(
            std::fs::read(actual).unwrap(),
            std::fs::read(chain_goldens.join(name)).unwrap(),
            "{name} changed"
        );
    }
    assert_eq!(
        normalized_manifest(&manifest_path(&a_out)),
        std::fs::read(chain_goldens.join("window_a.manifest.normalized.json")).unwrap()
    );
    assert_eq!(
        normalized_manifest(&manifest_path(&b_out)),
        std::fs::read(chain_goldens.join("window_b.manifest.normalized.json")).unwrap()
    );

    std::fs::remove_dir_all(temp).unwrap();
}
