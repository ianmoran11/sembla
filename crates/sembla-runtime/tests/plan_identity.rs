use sembla_ir::{
    parse_input, validate, validate_plan, AttrType, Expr, ParsedInput, ValidatedModel,
};
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::executor::{run_tick, ObservationValue};
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

fn load_plan(source: &str) -> sembla_ir::ExecutablePlanV1 {
    let ParsedInput::Plan(mut plan) = parse_input(source).unwrap() else {
        panic!("fixture did not parse as a plan")
    };
    // The checked-in two-box fixture is deliberately nearly deterministic.
    // Give each fixture's transitions stochastic hazards in memory so this
    // test proves that shifted dense ordinals would perturb shared traces.
    for model_box in &mut plan.model.boxes {
        for transition in &mut model_box.transitions {
            transition.guard = match model_box.name.as_str() {
                "population" => transition.guard.clone(),
                _ => Expr::Bool { value: true },
            };
            transition.hazard = Expr::Real { value: 0.35 };
        }
    }
    plan
}

fn executable(plan: &sembla_ir::ExecutablePlanV1) -> ValidatedModel {
    validate_plan(plan).unwrap().model_with_rule_words()
}

fn zero_state(model: &ValidatedModel) -> StateStore {
    let tables = model
        .model()
        .boxes
        .iter()
        .flat_map(|model_box| {
            model_box.tables.iter().map(|table| {
                let rows = usize::try_from(table.size_hint).unwrap();
                let columns = table
                    .attrs
                    .iter()
                    .map(|attr| {
                        let data = match &attr.ty {
                            AttrType::Real => ColumnData::Real(vec![0.0; rows]),
                            AttrType::Int => ColumnData::Int(vec![0; rows]),
                            AttrType::Enum { .. } => ColumnData::Enum(vec![0; rows]),
                            AttrType::Ref { .. } => ColumnData::Ref(vec![0; rows]),
                        };
                        ColumnInit::new(&attr.name, data)
                    })
                    .collect();
                TableInit::new(&model_box.name, &table.name, rows, columns)
            })
        })
        .collect();
    StateStore::new(model, tables).unwrap()
}

type SharedTick = (
    Vec<(String, Vec<usize>)>,
    Vec<(String, String, ObservationValue)>,
    Vec<(String, usize)>,
);

fn shared_trace(model: &ValidatedModel) -> Vec<SharedTick> {
    let params = ParamEnv::defaults(model);
    let mut state = zero_state(model);
    (0..40)
        .map(|tick| {
            let report = run_tick(model, &mut state, &params, 55, tick).unwrap();
            let fired = report
                .fired_per_box
                .into_iter()
                .filter(|(name, _)| name == "controller" || name == "population")
                .map(|(name, rules)| (name, rules.into_iter().map(|(_, count)| count).collect()))
                .collect();
            let views = report
                .views
                .into_iter()
                .filter(|view| view.box_name == "controller" || view.box_name == "population")
                .map(|view| (view.box_name, view.name, view.value))
                .collect();
            let deferred = report
                .deferred_per_resource_table
                .into_iter()
                .filter(|(table, _)| {
                    table.starts_with("controller.") || table.starts_with("population.")
                })
                .collect();
            (fired, views, deferred)
        })
        .collect()
}

#[test]
fn sibling_insertion_preserves_shared_stochastic_trace_via_plan_words() {
    let base_plan = load_plan(include_str!("../../../fixtures/plans/two_box.plan.json"));
    let sibling_plan = load_plan(include_str!(
        "../../../fixtures/plans/two_box_plus_sibling.plan.json"
    ));
    let base = executable(&base_plan);
    let sibling = executable(&sibling_plan);

    assert_eq!(shared_trace(&base), shared_trace(&sibling));

    // This counterexample proves the test is not vacuous: running the sibling
    // model with legacy dense words changes at least one shared stochastic
    // firing trace because its insertion shifts both shared ordinals.
    let dense_sibling = validate(sibling_plan.model).unwrap();
    assert_ne!(shared_trace(&sibling), shared_trace(&dense_sibling));
}
