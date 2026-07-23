use sembla_ir::{
    validate_with_features, Attr, AttrType, Box as ModelBox, Expr, FeatureSet, GroupKey,
    GroupedViewDecl, Model, Table, GROUPED_OBSERVATIONS_FEATURE,
};

fn model(view: GroupedViewDecl) -> Model {
    Model {
        name: "grouped_validation".to_owned(),
        dt: 1.0,
        params: vec![],
        boxes: vec![ModelBox {
            name: "world".to_owned(),
            tables: vec![Table {
                name: "person".to_owned(),
                size_hint: 1,
                attrs: vec![
                    Attr {
                        name: "sex".to_owned(),
                        ty: AttrType::Enum {
                            variants: vec!["a".to_owned(), "b".to_owned()],
                        },
                    },
                    Attr {
                        name: "age".to_owned(),
                        ty: AttrType::Int,
                    },
                    Attr {
                        name: "rate".to_owned(),
                        ty: AttrType::Real,
                    },
                ],
            }],
            transitions: vec![],
            inputs: vec![],
            outputs: vec![],
            views: vec![],
            grouped_views: vec![view],
        }],
        wires: vec![],
        summaries: vec![],
    }
}

fn grouped(keys: Vec<GroupKey>) -> GroupedViewDecl {
    GroupedViewDecl {
        name: "cells".to_owned(),
        table: "person".to_owned(),
        filter: Some(Box::new(Expr::Bool { value: true })),
        keys,
    }
}

fn enabled() -> FeatureSet {
    FeatureSet::from([GROUPED_OBSERVATIONS_FEATURE.to_owned()])
}

#[test]
fn grouped_model_requires_explicit_feature() {
    let error = sembla_ir::validate(model(grouped(vec![GroupKey {
        attr: "sex".to_owned(),
        band_width: None,
    }])))
    .unwrap_err();
    assert_eq!(error.path, "$.boxes[0].grouped_views[0]");
    assert!(error.message.contains("--enable grouped-observations"));
}

#[test]
fn grouped_keys_and_filters_are_strictly_validated() {
    for (key, path, message) in [
        (
            GroupKey {
                attr: "missing".to_owned(),
                band_width: None,
            },
            "$.boxes[0].grouped_views[0].keys[0].attr",
            "unknown attribute",
        ),
        (
            GroupKey {
                attr: "rate".to_owned(),
                band_width: None,
            },
            "$.boxes[0].grouped_views[0].keys[0].attr",
            "Enum, Ref, or Int",
        ),
        (
            GroupKey {
                attr: "sex".to_owned(),
                band_width: Some(2),
            },
            "$.boxes[0].grouped_views[0].keys[0].band_width",
            "must not declare",
        ),
        (
            GroupKey {
                attr: "age".to_owned(),
                band_width: None,
            },
            "$.boxes[0].grouped_views[0].keys[0].band_width",
            "requires band_width",
        ),
        (
            GroupKey {
                attr: "age".to_owned(),
                band_width: Some(0),
            },
            "$.boxes[0].grouped_views[0].keys[0].band_width",
            "at least 1",
        ),
    ] {
        let error = validate_with_features(model(grouped(vec![key])), &enabled()).unwrap_err();
        assert_eq!(error.path, path);
        assert!(error.message.contains(message), "{}", error.message);
    }

    let mut aggregate = grouped(vec![GroupKey {
        attr: "sex".to_owned(),
        band_width: None,
    }]);
    aggregate.filter = Some(Box::new(Expr::Agg {
        op: sembla_ir::AggOp::Count,
        table: "person".to_owned(),
        on: sembla_ir::AggJoin {
            fk_attr: "sex".to_owned(),
            self_fk_attr: "sex".to_owned(),
        },
        filter: Box::new(Expr::Bool { value: true }),
    }));
    let error = validate_with_features(model(aggregate), &enabled()).unwrap_err();
    assert_eq!(error.path, "$.boxes[0].grouped_views[0].filter");
    assert!(error.message.contains("aggregates are not supported"));
}

#[test]
fn grouped_view_requires_one_to_four_keys() {
    for count in [0_usize, 5] {
        let keys = (0..count)
            .map(|_| GroupKey {
                attr: "sex".to_owned(),
                band_width: None,
            })
            .collect();
        let error = validate_with_features(model(grouped(keys)), &enabled()).unwrap_err();
        assert_eq!(error.path, "$.boxes[0].grouped_views[0].keys");
    }
}
