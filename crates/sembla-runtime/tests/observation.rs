use sembla_ir::{SummaryDecl, SummaryReduce, ViewDecl, ViewReduce};
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::executor::{self, device_observation_eligibility, ObservationValue};
use sembla_runtime::population::SyntheticPopulation;
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

#[derive(Debug, PartialEq, Eq)]
struct Trace {
    state_hashes: Vec<[u8; 32]>,
    fired: Vec<Vec<(u32, usize)>>,
    deferred: Vec<Vec<(String, usize)>>,
    view_counts: Vec<usize>,
}

fn eligibility_model(views: &str, grouped_views: &str) -> sembla_ir::ValidatedModel {
    let source = format!(
        r#"{{"name":"device_observation","dt":1.0,"params":[],"boxes":[{{"name":"world","tables":[{{"name":"Group","size_hint":1,"attrs":[]}},{{"name":"Person","size_hint":4,"attrs":[{{"name":"group","ty":{{"kind":"ref","table":"Group"}}}},{{"name":"n","ty":{{"kind":"int"}}}},{{"name":"x","ty":{{"kind":"real"}}}}]}}],"transitions":[],"inputs":[{{"name":"events","schema":[{{"name":"amount","ty":{{"kind":"int"}}}}]}}],"outputs":[],"views":[{views}],"grouped_views":[{grouped_views}]}}],"wires":[],"summaries":[]}}"#,
    );
    let raw = sembla_ir::parse_json(&source).unwrap();
    if grouped_views.is_empty() {
        sembla_ir::validate(raw).unwrap()
    } else {
        let features =
            sembla_ir::FeatureSet::from([sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
        sembla_ir::validate_with_features(raw, &features).unwrap()
    }
}

fn run_trace(model: sembla_ir::ValidatedModel) -> Trace {
    let population = SyntheticPopulation::generate(80, 8, 4, 123).unwrap();
    let box_name = model.model().boxes[0].name.clone();
    let initial = population.sir_table_initializers_for_box(&box_name);
    let params = ParamEnv::defaults(&model);
    let mut state = StateStore::new(&model, initial).unwrap();
    let mut trace = Trace {
        state_hashes: Vec::new(),
        fired: Vec::new(),
        deferred: Vec::new(),
        view_counts: Vec::new(),
    };
    for tick in 0..8 {
        let report = executor::run_tick(&model, &mut state, &params, 55, tick).unwrap();
        trace.state_hashes.push(state.state_hash());
        trace.fired.push(report.fired);
        trace.deferred.push(report.deferred_per_resource_table);
        trace.view_counts.push(report.views.len());
    }
    trace
}

#[test]
fn device_observation_eligibility_accepts_only_count_and_int_extrema() {
    let model = eligibility_model(
        r#"{"name":"selected","table":"Person","filter":{"kind":"gt","lhs":{"kind":"self_attr","name":"n"},"rhs":{"kind":"int","value":0}},"value":null,"reduce":"count"},{"name":"minimum","table":"Person","filter":null,"value":{"kind":"self_attr","name":"n"},"reduce":"min"},{"name":"maximum","table":"Person","filter":null,"value":{"kind":"self_attr","name":"n"},"reduce":"max"}"#,
        "",
    );
    let decision = device_observation_eligibility(&model);
    assert!(decision.eligible, "{decision:?}");
    assert_eq!(decision.views.len(), 3);
    assert!(decision.views.iter().all(|view| view.eligible));
}

#[test]
fn device_observation_eligibility_is_conservative_for_host_ordered_reductions() {
    let model = eligibility_model(
        r#"{"name":"sum_int","table":"Person","filter":null,"value":{"kind":"self_attr","name":"n"},"reduce":"sum"},{"name":"sum_real","table":"Person","filter":null,"value":{"kind":"self_attr","name":"x"},"reduce":"sum"},{"name":"min_real","table":"Person","filter":null,"value":{"kind":"self_attr","name":"x"},"reduce":"min"},{"name":"max_real","table":"Person","filter":null,"value":{"kind":"self_attr","name":"x"},"reduce":"max"}"#,
        "",
    );
    let decision = device_observation_eligibility(&model);
    assert!(!decision.eligible);
    assert!(decision.views.iter().all(|view| !view.eligible));
    assert!(decision.views[0].reason.contains("overflow association"));
    assert!(decision.views[1].reason.contains("host order"));
    assert!(decision.views[2].reason.contains("Int expression"));
    assert!(decision.views[3].reason.contains("Int expression"));
}

#[test]
fn device_observation_eligibility_rejects_aggregate_and_input_expressions() {
    let model = eligibility_model(
        r#"{"name":"aggregate_filter","table":"Person","filter":{"kind":"gt","lhs":{"kind":"agg","op":{"kind":"count"},"table":"Person","on":{"fk_attr":"group","self_fk_attr":"group"},"filter":{"kind":"bool","value":true}},"rhs":{"kind":"int","value":0}},"value":null,"reduce":"count"},{"name":"input_filter","table":"Person","filter":{"kind":"gt","lhs":{"kind":"input","port":"events","agg":{"op":{"kind":"count"},"filter":null}},"rhs":{"kind":"int","value":0}},"value":null,"reduce":"count"}"#,
        "",
    );
    let decision = device_observation_eligibility(&model);
    assert!(!decision.eligible);
    assert_eq!(decision.views.len(), 2);
    assert!(decision
        .views
        .iter()
        .all(|view| { !view.eligible && view.reason.contains("row-local infallible") }));
}

#[test]
fn grouped_and_zero_view_models_force_host_observation() {
    let grouped = eligibility_model(
        r#"{"name":"count","table":"Person","filter":null,"value":null,"reduce":"count"}"#,
        r#"{"name":"by_group","table":"Person","filter":null,"keys":[{"attr":"group"}]}"#,
    );
    let grouped = device_observation_eligibility(&grouped);
    assert!(!grouped.eligible);
    assert_eq!(grouped.views.len(), 2);
    assert!(grouped.views[1].reason.contains("grouped"));

    let empty = device_observation_eligibility(&eligibility_model("", ""));
    assert!(!empty.eligible);
    assert!(empty.reason.contains("legacy state reporting"));
}

#[test]
fn views_reduce_committed_rows_and_summaries_keep_earliest_argmax() {
    let mut raw =
        sembla_ir::parse_json(include_str!("../../../examples/observations.json")).unwrap();
    raw.summaries.extend([
        SummaryDecl {
            name: "minimum_total".to_owned(),
            r#box: "population".to_owned(),
            view: "total_value".to_owned(),
            reduce: SummaryReduce::Min,
        },
        SummaryDecl {
            name: "maximum_total".to_owned(),
            r#box: "population".to_owned(),
            view: "total_value".to_owned(),
            reduce: SummaryReduce::Max,
        },
        SummaryDecl {
            name: "last_total".to_owned(),
            r#box: "population".to_owned(),
            view: "total_value".to_owned(),
            reduce: SummaryReduce::Last,
        },
    ]);
    let model = sembla_ir::validate(raw).unwrap();
    let initial = vec![TableInit::new(
        "population",
        "Person",
        3,
        vec![
            ColumnInit::new("status", ColumnData::Enum(vec![0, 1, 0])),
            ColumnInit::new("value", ColumnData::Real(vec![2.0, 5.0, 1.0])),
            ColumnInit::new("visits", ColumnData::Int(vec![7, 3, 9])),
        ],
    )];
    let params = ParamEnv::defaults(&model);
    let mut state = StateStore::new(&model, initial).unwrap();
    let report = executor::run(&model, &mut state, &params, 1, 2).unwrap();

    assert_eq!(
        report.ticks[0]
            .views
            .iter()
            .map(|view| view.value)
            .collect::<Vec<_>>(),
        vec![
            ObservationValue::Real(8.0),
            ObservationValue::Int(2),
            ObservationValue::Int(3),
            ObservationValue::Real(5.0),
        ]
    );
    assert_eq!(
        report
            .summaries
            .iter()
            .map(|summary| (summary.name.as_str(), summary.value))
            .collect::<Vec<_>>(),
        vec![
            ("total_value_over_time", ObservationValue::Real(16.0)),
            ("peak_value_tick", ObservationValue::Int(0)),
            ("minimum_total", ObservationValue::Real(8.0)),
            ("maximum_total", ObservationValue::Real(8.0)),
            ("last_total", ObservationValue::Real(8.0)),
        ]
    );
}

#[test]
fn observation_is_a_bitwise_sink_for_state_and_scheduling() {
    let observed_raw = sembla_ir::parse_json(include_str!("../../../examples/sir.json")).unwrap();

    let mut disabled_raw = observed_raw.clone();
    disabled_raw.boxes[0].views.clear();
    disabled_raw.summaries.clear();

    let mut extended_raw = observed_raw.clone();
    extended_raw.boxes[0].views.push(ViewDecl {
        name: "all_rows".to_owned(),
        table: "person".to_owned(),
        filter: None,
        value: None,
        reduce: ViewReduce::Count,
    });

    let observed = run_trace(sembla_ir::validate(observed_raw).unwrap());
    let disabled = run_trace(sembla_ir::validate(disabled_raw).unwrap());
    let extended = run_trace(sembla_ir::validate(extended_raw).unwrap());

    assert_eq!(observed.state_hashes, disabled.state_hashes);
    assert_eq!(observed.state_hashes, extended.state_hashes);
    assert_eq!(observed.fired, disabled.fired);
    assert_eq!(observed.fired, extended.fired);
    assert_eq!(observed.deferred, disabled.deferred);
    assert_eq!(observed.deferred, extended.deferred);
    assert!(observed.view_counts.iter().all(|count| *count == 3));
    assert!(disabled.view_counts.iter().all(|count| *count == 0));
    assert!(extended.view_counts.iter().all(|count| *count == 4));
}
