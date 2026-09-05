use super::{ConflictLaunchGeometry, CudaBackend, HashMode};
use sembla_cpu::run_tick;
use sembla_runtime::core::{ColumnData, ColumnInit, ParamEnv, StateStore, TableInit};

fn contested_model() -> sembla_ir::ValidatedModel {
    // Rules are deliberately ordered B, C, A. Their only enabled rows have
    // keys 1, 2, 0 and entity IDs 0, 1, 2 respectively, so key, rule, and
    // entity component minima come from different instances. The CPU
    // compare_instances lexicographic minimum is A (key 0), not a
    // component-wise synthetic tuple.
    let source = r#"{"name":"segmented_argmin_geometry","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Worker","size_hint":1,"attrs":[]},{"name":"Applicant","size_hint":3,"attrs":[{"name":"worker","ty":{"kind":"ref","table":"Worker"}},{"name":"priority","ty":{"kind":"int"}},{"name":"role","ty":{"kind":"enum","variants":["B","C","A"]}},{"name":"outcome","ty":{"kind":"enum","variants":["Waiting","Won"]}}]}],"transitions":[{"name":"rule_b","table":"Applicant","guard":{"kind":"enum_is","attr":"role","variant":"B"},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"outcome","value":{"kind":"enum","variant":"Won"}}],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"self_attr","name":"priority"}}}]},{"name":"rule_c","table":"Applicant","guard":{"kind":"enum_is","attr":"role","variant":"C"},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"outcome","value":{"kind":"enum","variant":"Won"}}],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"self_attr","name":"priority"}}}]},{"name":"rule_a","table":"Applicant","guard":{"kind":"enum_is","attr":"role","variant":"A"},"hazard":{"kind":"real","value":1e300},"effects":[{"kind":"set_attr","attr":"outcome","value":{"kind":"enum","variant":"Won"}}],"contests":[{"resource":{"kind":"self_attr","name":"worker"},"ordering":{"kind":"key","expr":{"kind":"self_attr","name":"priority"}}}]}],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap()
}

fn contested_state() -> Vec<TableInit> {
    vec![
        TableInit::new("world", "Worker", 1, Vec::new()),
        TableInit::new(
            "world",
            "Applicant",
            3,
            vec![
                ColumnInit::new("worker", ColumnData::Ref(vec![0, 0, 0])),
                ColumnInit::new("priority", ColumnData::Int(vec![1, 2, 0])),
                ColumnInit::new("role", ColumnData::Enum(vec![0, 1, 2])),
                ColumnInit::new("outcome", ColumnData::Enum(vec![0, 0, 0])),
            ],
        ),
    ]
}

#[test]
#[ignore = "requires a CUDA GPU; exercises explicit conflict launch geometries"]
fn segmented_argmin_winner_matches_cpu_under_three_geometries() {
    let model = contested_model();
    let initial = contested_state();
    let params = ParamEnv::defaults(&model);
    let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
    run_tick(&model, &mut cpu, &params, 9009, 0).unwrap();
    let expected = cpu.state_hash();

    for (grid, block) in [(1, 1), (1, 32), (3, 4)] {
        let mut backend =
            CudaBackend::new(&model, initial.clone(), &params, 9009, HashMode::EveryTick)
                .expect("CUDA device, driver, and NVRTC are required");
        backend.conflict_launch_override = Some(ConflictLaunchGeometry { grid, block });
        let result = backend.run(1).unwrap();
        assert_eq!(result.final_state_hash, expected, "geometry {grid}x{block}");
        assert_eq!(result.per_tick_state_hashes, [expected]);
    }
}
