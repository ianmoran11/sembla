//! Spike: how much is there to win from scalar broadcast and from tiling?
//!
//! NOT a benchmark of the product and NOT a proposed implementation. It exists
//! to size two separable prizes before either is scoped as a PRD:
//!
//!   H1 (scalar broadcast) — `Expr::Int { value }` materialises
//!       `vec![value; row_count]`. In the demographic model the expression trees
//!       average 3.3 nodes, so roughly a third of all materialised vectors are
//!       constant broadcasts that need no memory traffic at all.
//!
//!   H2 (tiling / fusion) — every node returns a full-length `Vec`, so
//!       intermediates round-trip through RAM instead of staying in cache.
//!
//! Method: run the real `eval_column` against a real `StateStore`, then run a
//! hand-written fused loop computing the identical result, and compare. The
//! hand-written version is the ceiling: it is what a perfect fusing evaluator
//! would achieve, with no interpretive overhead at all. Real fusion lands
//! somewhere below it.
//!
//! The expression shapes mirror `demographic_slots.no-grouped.json`: a 3-node
//! comparison, a 5-node banded filter (the most common view shape), and an
//! 11-node guard (the deepest expression in the model).
//!
//!   cargo run --release -p sembla-runtime --example fusion_spike -- [rows] [reps]

use std::time::Instant;

use sembla_ir::{Attr, AttrType, Box as IrBox, Expr, Model, Table};
use sembla_runtime::eval::{eval_column, AggCache, EvalTable, ParamEnv, ValueColumn};
use sembla_runtime::state::{ColumnData, ColumnInit, StateStore, TableInit};

fn int(value: i64) -> std::boxed::Box<Expr> {
    std::boxed::Box::new(Expr::Int { value })
}
fn attr(name: &str) -> std::boxed::Box<Expr> {
    std::boxed::Box::new(Expr::SelfAttr { name: name.into() })
}

/// `age > 20` — 3 nodes. The commonest shape in the model.
fn expr_compare() -> Expr {
    Expr::Gt {
        lhs: attr("age"),
        rhs: int(20),
    }
}

/// `age >= 20 && age < 25` — 5 nodes. The banded view-filter shape.
fn expr_band() -> Expr {
    Expr::And {
        lhs: std::boxed::Box::new(Expr::Ge {
            lhs: attr("age"),
            rhs: int(20),
        }),
        rhs: std::boxed::Box::new(Expr::Lt {
            lhs: attr("age"),
            rhs: int(25),
        }),
    }
}

/// 11 nodes, matching `die_adult.guard`, the deepest expression in the model.
fn expr_guard() -> Expr {
    Expr::And {
        lhs: std::boxed::Box::new(Expr::And {
            lhs: std::boxed::Box::new(Expr::Ge {
                lhs: attr("age"),
                rhs: int(20),
            }),
            rhs: std::boxed::Box::new(Expr::Lt {
                lhs: attr("age"),
                rhs: int(65),
            }),
        }),
        rhs: std::boxed::Box::new(Expr::And {
            lhs: std::boxed::Box::new(Expr::Gt {
                lhs: attr("tenure"),
                rhs: int(0),
            }),
            rhs: std::boxed::Box::new(Expr::Ne {
                lhs: attr("status"),
                rhs: int(3),
            }),
        }),
    }
}

// Hand-fused equivalents: one pass, no intermediates, no dispatch.
fn fused_compare(age: &[i64], out: &mut Vec<bool>) {
    out.clear();
    out.extend(age.iter().map(|a| *a > 20));
}

fn fused_band(age: &[i64], out: &mut Vec<bool>) {
    out.clear();
    out.extend(age.iter().map(|a| *a >= 20 && *a < 25));
}

// Staged variants, to attribute the win rather than just bound it.
//
// h1  — constants become scalars, leaf columns still copied (as `eval_self_attr`
//       does today), intermediates still materialised. Isolates scalar broadcast.
// h13 — also borrows the leaf column instead of copying it.
// fused — additionally keeps intermediates out of memory. The full ceiling.

fn h1_band(age: &[i64]) -> Vec<bool> {
    let a = age.to_vec();
    let b = age.to_vec();
    let l: Vec<bool> = a.iter().map(|v| *v >= 20).collect();
    let r: Vec<bool> = b.iter().map(|v| *v < 25).collect();
    l.iter().zip(&r).map(|(x, y)| *x && *y).collect()
}

fn h13_band(age: &[i64]) -> Vec<bool> {
    let l: Vec<bool> = age.iter().map(|v| *v >= 20).collect();
    let r: Vec<bool> = age.iter().map(|v| *v < 25).collect();
    l.iter().zip(&r).map(|(x, y)| *x && *y).collect()
}

fn h1_guard(age: &[i64], tenure: &[i64], status: &[i64]) -> Vec<bool> {
    let (a, b, t, s) = (age.to_vec(), age.to_vec(), tenure.to_vec(), status.to_vec());
    let ge: Vec<bool> = a.iter().map(|v| *v >= 20).collect();
    let lt: Vec<bool> = b.iter().map(|v| *v < 65).collect();
    let gt: Vec<bool> = t.iter().map(|v| *v > 0).collect();
    let ne: Vec<bool> = s.iter().map(|v| *v != 3).collect();
    let left: Vec<bool> = ge.iter().zip(&lt).map(|(x, y)| *x && *y).collect();
    let right: Vec<bool> = gt.iter().zip(&ne).map(|(x, y)| *x && *y).collect();
    left.iter().zip(&right).map(|(x, y)| *x && *y).collect()
}

fn h13_guard(age: &[i64], tenure: &[i64], status: &[i64]) -> Vec<bool> {
    let ge: Vec<bool> = age.iter().map(|v| *v >= 20).collect();
    let lt: Vec<bool> = age.iter().map(|v| *v < 65).collect();
    let gt: Vec<bool> = tenure.iter().map(|v| *v > 0).collect();
    let ne: Vec<bool> = status.iter().map(|v| *v != 3).collect();
    let left: Vec<bool> = ge.iter().zip(&lt).map(|(x, y)| *x && *y).collect();
    let right: Vec<bool> = gt.iter().zip(&ne).map(|(x, y)| *x && *y).collect();
    left.iter().zip(&right).map(|(x, y)| *x && *y).collect()
}

fn fused_guard(age: &[i64], tenure: &[i64], status: &[i64], out: &mut Vec<bool>) {
    out.clear();
    out.extend(
        age.iter()
            .zip(tenure)
            .zip(status)
            .map(|((a, t), s)| *a >= 20 && *a < 65 && *t > 0 && *s != 3),
    );
}

fn build_model() -> sembla_ir::ValidatedModel {
    sembla_ir::validate(Model {
        name: "fusion_spike".into(),
        dt: 1.0,
        params: vec![],
        boxes: vec![IrBox {
            name: "world".into(),
            tables: vec![Table {
                name: "slot".into(),
                size_hint: 1,
                attrs: vec![
                    Attr {
                        name: "age".into(),
                        ty: AttrType::Int,
                    },
                    Attr {
                        name: "tenure".into(),
                        ty: AttrType::Int,
                    },
                    Attr {
                        name: "status".into(),
                        ty: AttrType::Int,
                    },
                ],
            }],
            transitions: vec![],
            inputs: vec![],
            outputs: vec![],
            views: vec![],
            grouped_views: vec![],
        }],
        wires: vec![],
        summaries: vec![],
    })
    .expect("spike model must validate")
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    xs[xs.len() / 2]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let rows: usize = args
        .next()
        .and_then(|a| a.parse().ok())
        .unwrap_or(5_000_000);
    let reps: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(7);

    // Deterministic, cheap, and spread across the branch outcomes so neither
    // side wins on branch prediction.
    let age: Vec<i64> = (0..rows).map(|i| (i % 90) as i64).collect();
    let tenure: Vec<i64> = (0..rows).map(|i| (i % 7) as i64).collect();
    let status: Vec<i64> = (0..rows).map(|i| (i % 5) as i64).collect();

    let model = build_model();
    let store = StateStore::new(
        &model,
        vec![TableInit::new(
            "world",
            "slot",
            rows,
            vec![
                ColumnInit::new("age", ColumnData::Int(age.clone())),
                ColumnInit::new("tenure", ColumnData::Int(tenure.clone())),
                ColumnInit::new("status", ColumnData::Int(status.clone())),
            ],
        )],
    )
    .expect("spike state must initialize");
    let params = ParamEnv::defaults(&model);
    let snapshot = store.snapshot();
    let table = EvalTable::new(&model, "world", "slot").expect("table");

    println!("rows={rows} reps={reps}\n");
    println!(
        "{:<10} {:>5} {:>10} {:>10} {:>10} {:>10} {:>8}",
        "expr", "nodes", "eval_ms", "h1_ms", "h13_ms", "fused_ms", "ceiling"
    );

    let cases: Vec<(&str, usize, Expr)> = vec![
        ("compare", 3, expr_compare()),
        ("band", 5, expr_band()),
        ("guard", 11, expr_guard()),
    ];

    let mut out: Vec<bool> = Vec::with_capacity(rows);

    for (name, nodes, expr) in cases {
        let mut eval_times = Vec::new();
        let mut fused_times = Vec::new();
        let mut eval_last: Option<ValueColumn> = None;

        for _ in 0..reps {
            let mut cache = AggCache::new(&model, &snapshot, &params);
            let t = Instant::now();
            let got = eval_column(&expr, table, &snapshot, &params, &mut cache).expect("eval");
            eval_times.push(t.elapsed().as_secs_f64() * 1000.0);
            eval_last = Some(got);

            let t = Instant::now();
            match name {
                "compare" => fused_compare(&age, &mut out),
                "band" => fused_band(&age, &mut out),
                _ => fused_guard(&age, &tenure, &status, &mut out),
            }
            fused_times.push(t.elapsed().as_secs_f64() * 1000.0);
        }

        // The comparison is meaningless unless both produce the same answer.
        match eval_last.expect("a result") {
            ValueColumn::Bool(values) => {
                assert_eq!(values.len(), out.len(), "{name}: length differs");
                assert!(values == out, "{name}: fused result differs from eval_column");
            }
            other => panic!("{name}: expected Bool, got {other:?}"),
        }

        let mut h1_times = Vec::new();
        let mut h13_times = Vec::new();
        for _ in 0..reps {
            let t = Instant::now();
            let a = match name {
                "band" => h1_band(&age),
                "guard" => h1_guard(&age, &tenure, &status),
                _ => age.iter().map(|v| *v > 20).collect(),
            };
            h1_times.push(t.elapsed().as_secs_f64() * 1000.0);

            let t = Instant::now();
            let b = match name {
                "band" => h13_band(&age),
                "guard" => h13_guard(&age, &tenure, &status),
                _ => age.iter().map(|v| *v > 20).collect(),
            };
            h13_times.push(t.elapsed().as_secs_f64() * 1000.0);
            assert!(a == b && b == out, "{name}: staged variants disagree");
        }

        let e = median(eval_times);
        let f = median(fused_times);
        let h1 = median(h1_times);
        let h13 = median(h13_times);
        println!(
            "{name:<10} {nodes:>5} {e:>10.2} {h1:>10.2} {h13:>10.2} {f:>10.2} {:>7.1}x",
            e / f
        );
    }

    println!(
        "\nfused = hand-written single pass: the ceiling a perfect fusing evaluator\n\
         could reach. All variants asserted to produce identical results.\n\
         \n\
         h1  = constants as scalars, leaf columns still copied, intermediates kept\n\
         h13 = also borrows leaf columns instead of copying them\n\
         \n\
         `compare` is degenerate: a 3-node expression has no intermediate, so its\n\
         h1/h13/fused columns are the same code and only the total is meaningful.\n\
         Read the attribution off `band` and `guard`."
    );
}
