use super::control_reports_from_counts;

fn model(source: &str) -> sembla_ir::ValidatedModel {
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

#[test]
fn synthetic_counts_preserve_report_shape_order_and_qualification() {
    let model = model(
        r#"{"name":"control_reports","dt":1.0,"params":[],"boxes":[{"name":"left","tables":[{"name":"zero","size_hint":0,"attrs":[]},{"name":"kept","size_hint":0,"attrs":[]}],"transitions":[{"name":"first","table":"zero","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":0.0},"effects":[],"contests":[]},{"name":"second","table":"zero","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":0.0},"effects":[],"contests":[]}],"inputs":[],"outputs":[],"views":[]},{"name":"right","tables":[{"name":"also_kept","size_hint":0,"attrs":[]}],"transitions":[{"name":"third","table":"also_kept","guard":{"kind":"bool","value":true},"hazard":{"kind":"real","value":0.0},"effects":[],"contests":[]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#,
    );
    let (fired, deferred) = control_reports_from_counts(&model, &[0, 7, 2], &[0, 5, 3]).unwrap();
    assert_eq!(
        fired,
        vec![
            ("left".to_owned(), vec![(0, 0), (1, 7)]),
            ("right".to_owned(), vec![(2, 2)]),
        ]
    );
    assert_eq!(
        deferred,
        vec![
            ("left.kept".to_owned(), 5),
            ("right.also_kept".to_owned(), 3)
        ]
    );
}

#[test]
fn synthetic_counts_preserve_single_box_names_and_empty_domains() {
    let single = model(
        r#"{"name":"single","dt":1.0,"params":[],"boxes":[{"name":"only","tables":[{"name":"zero","size_hint":0,"attrs":[]},{"name":"kept","size_hint":0,"attrs":[]}],"transitions":[],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#,
    );
    let (fired, deferred) = control_reports_from_counts(&single, &[], &[0, 4]).unwrap();
    assert_eq!(fired, vec![("only".to_owned(), Vec::new())]);
    assert_eq!(deferred, vec![("kept".to_owned(), 4)]);

    let empty = model(
        r#"{"name":"empty","dt":1.0,"params":[],"boxes":[{"name":"empty","tables":[],"transitions":[],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#,
    );
    let (fired, deferred) = control_reports_from_counts(&empty, &[], &[]).unwrap();
    assert_eq!(fired, vec![("empty".to_owned(), Vec::new())]);
    assert!(deferred.is_empty());
}
