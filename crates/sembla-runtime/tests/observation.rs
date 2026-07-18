use sembla_ir::{SummaryDecl, SummaryReduce, ViewDecl, ViewReduce};
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::executor::{self, ObservationValue};
use sembla_runtime::population::SyntheticPopulation;
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

#[derive(Debug, PartialEq, Eq)]
struct Trace {
    state_hashes: Vec<[u8; 32]>,
    fired: Vec<Vec<(u32, usize)>>,
    deferred: Vec<Vec<(String, usize)>>,
    view_counts: Vec<usize>,
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
