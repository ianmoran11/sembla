use super::final_state::{
    allocate_cacheable_staging, checked_final_state_component_bytes,
    estimate_isolated_sweep_capacity, final_state_component_bytes, FinalStateAllocationInjection,
    SWEEP_CAPACITY_MIB,
};
use super::layout::{build_layout, hash_state, pack_initial_state, write_column};
use super::{generate, CudaFinalStateReadbackMode};
use sembla_runtime::core::{ColumnData, ColumnInit, InputTable, StateStore, TableInit};

fn demographic_shape(scale: usize) -> (sembla_ir::ValidatedModel, Vec<TableInit>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/demographic/benchmark/demographic_slots.full.json");
    let source = std::fs::read_to_string(path).unwrap();
    let features =
        sembla_ir::FeatureSet::from([sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
    let model =
        sembla_ir::validate_with_features(sembla_ir::parse_json(&source).unwrap(), &features)
            .unwrap();
    let composed = model.model().boxes.len() > 1 || !model.model().wires.is_empty();
    // The estimator consumes only names and row counts. Avoid allocating a
    // 10M-row test state; constructor validation remains unchanged and the
    // production caller passes the already validated real tables.
    let tables = model
        .model()
        .boxes
        .iter()
        .flat_map(|model_box| {
            model_box.tables.iter().map(|table| {
                let rows = if composed && table.size_hint != 0 {
                    usize::try_from(table.size_hint).unwrap()
                } else {
                    scale
                };
                TableInit::new(&model_box.name, &table.name, rows, Vec::new())
            })
        })
        .collect();
    (model, tables)
}

fn sparse_resource_model() -> sembla_ir::ValidatedModel {
    let model_box = |name| {
        serde_json::json!({
            "name": name,
            "tables": [
                {"name": "unused", "size_hint": 9, "attrs": []},
                {"name": "first", "size_hint": 2, "attrs": []},
                {"name": "second", "size_hint": 3, "attrs": []},
                {"name": "agents", "size_hint": 4, "attrs": [
                    {"name": "a", "ty": {"kind": "ref", "table": "first"}},
                    {"name": "b", "ty": {"kind": "ref", "table": "second"}},
                    {"name": "c", "ty": {"kind": "ref", "table": "first"}}
                ]}
            ],
            "transitions": [{
                "name": "claim", "table": "agents",
                "guard": {"kind": "bool", "value": true},
                "hazard": {"kind": "real", "value": 1e300},
                "effects": [],
                "contests": (["a", "b", "c"].map(|attr| serde_json::json!({
                    "resource": {"kind": "self_attr", "name": attr},
                    "ordering": {"kind": "race_time"}
                })))
            }],
            "inputs": [], "outputs": [], "views": []
        })
    };
    let source = serde_json::json!({
        "name": "sparse_resources", "dt": 1.0, "params": [],
        "boxes": [model_box("left"), model_box("right")],
        "wires": [], "summaries": []
    });
    sembla_ir::validate(sembla_ir::parse_json(&source.to_string()).unwrap()).unwrap()
}

fn sparse_resource_state() -> Vec<TableInit> {
    ["left", "right"]
        .into_iter()
        .flat_map(|name| {
            [
                TableInit::new(name, "unused", 9, vec![]),
                TableInit::new(name, "first", 2, vec![]),
                TableInit::new(name, "second", 3, vec![]),
                TableInit::new(
                    name,
                    "agents",
                    4,
                    vec![
                        ColumnInit::new("a", ColumnData::Ref(vec![0, 0, 0, 0])),
                        ColumnInit::new("b", ColumnData::Ref(vec![0, 1, 2, 0])),
                        ColumnInit::new("c", ColumnData::Ref(vec![0, 0, 0, 0])),
                    ],
                ),
            ]
        })
        .collect()
}

#[test]
fn resource_layout_packs_distinct_contest_targets_across_boxes() {
    let model = sparse_resource_model();
    let generated = generate(&model).unwrap();
    assert_eq!(generated.resource_tables, [1, 2, 5, 6]);
    let mut initial = sparse_resource_state();
    let layout = build_layout(&model, &initial, &generated).unwrap();
    assert_eq!(layout.resource_offsets, [0, 0, 2, 5, 5, 5, 7, 10]);
    assert_eq!(layout.resource_count, 10);

    // Empty targets still occupy their global table position without reserving
    // winner slots; all later target offsets remain valid.
    initial[1].row_count = 0;
    let layout = build_layout(&model, &initial, &generated).unwrap();
    assert_eq!(layout.resource_offsets, [0, 0, 0, 3, 3, 3, 5, 8]);
    assert_eq!(layout.resource_count, 8);

    let mut no_claims = model.model().clone();
    for model_box in &mut no_claims.boxes {
        model_box.transitions[0].contests.clear();
    }
    let no_claims = sembla_ir::validate(no_claims).unwrap();
    let generated = generate(&no_claims).unwrap();
    assert!(generated.resource_tables.is_empty());
    let layout = build_layout(&no_claims, &initial, &generated).unwrap();
    assert_eq!(layout.resource_count, 0);
    assert_eq!(layout.resource_offsets, [0; 8]);
}

#[test]
#[ignore = "requires a CUDA GPU; compares packed contest targets and deferred reports"]
fn sparse_resource_reports_match_cpu_across_ticks_and_reset() {
    use super::{CudaBackend, HashMode};
    use sembla_runtime::core::ParamEnv;

    let model = sparse_resource_model();
    let initial = sparse_resource_state();
    let params = ParamEnv::defaults(&model);
    let mut backend =
        CudaBackend::new(&model, initial.clone(), &params, 9009, HashMode::FinalOnly).unwrap();
    assert_eq!(backend.deferred.len(), 8 * 4);
    for seed in [9009, 19] {
        backend.reset_draw(&params, seed).unwrap();
        let mut cpu = StateStore::new(&model, initial.clone()).unwrap();
        for tick in 0..3 {
            let expected = sembla_cpu::run_tick(&model, &mut cpu, &params, seed, tick).unwrap();
            let (_, fired, deferred, _) = backend.run_tick_observed_reused().unwrap();
            assert_eq!(fired, expected.fired_per_box);
            assert_eq!(deferred, expected.deferred_per_resource_table);
            assert_eq!(
                backend.ensure_observed_state().unwrap().state_hash(),
                cpu.state_hash()
            );
        }
    }

    let seeds = [9009, 19];
    let mut fused = CudaBackend::new_fused_batch(
        &model,
        initial.clone(),
        &params,
        seeds[0],
        2,
        HashMode::FinalOnly,
    )
    .unwrap();
    assert_eq!(fused.deferred.len(), 2 * 8 * 4);
    for width in [2, 1, 2] {
        fused
            .reset_fused_batch(&vec![params.clone(); width], &seeds[..width])
            .unwrap();
        let mut cpu_states = seeds[..width]
            .iter()
            .map(|_| StateStore::new(&model, initial.clone()).unwrap())
            .collect::<Vec<_>>();
        for tick in 0..3 {
            let observed = fused.run_tick_observed_reused_fused().unwrap();
            for (slot, observation) in observed.into_iter().enumerate() {
                let expected =
                    sembla_cpu::run_tick(&model, &mut cpu_states[slot], &params, seeds[slot], tick)
                        .unwrap();
                let (_, fired, deferred, _) = observation.unwrap();
                assert_eq!(fired, expected.fired_per_box);
                assert_eq!(deferred, expected.deferred_per_resource_table);
                assert_eq!(
                    fused.fused_observed_state(slot).unwrap().state_hash(),
                    cpu_states[slot].state_hash()
                );
            }
        }
    }
}

#[test]
fn isolated_lane_estimate_is_conservative_against_measured_h100_arms() {
    for (scale, observed_mib) in [
        (1_000_000, [1_033_usize, 1_613, 2_733]),
        (10_000_000, [6_025_usize, 11_597, 22_701]),
    ] {
        let (model, tables) = demographic_shape(scale);
        let generated = generate(&model).unwrap();
        let layout = build_layout(&model, &tables, &generated).unwrap();
        let parameter_bytes = model.model().params.len().max(1) * 8;
        for (workers, observed) in [1_usize, 2, 4].into_iter().zip(observed_mib) {
            let estimate = estimate_isolated_sweep_capacity(
                &layout,
                &generated,
                parameter_bytes,
                workers,
                CudaFinalStateReadbackMode::Materialized,
            )
            .unwrap();
            assert!(
                estimate.device_bytes >= observed * SWEEP_CAPACITY_MIB,
                "{scale} rows/{workers} lanes: estimate {} MiB under measured {observed} MiB",
                estimate.device_bytes / SWEEP_CAPACITY_MIB
            );
        }
    }
}

#[test]
fn capacity_estimator_rejects_zero_workers() {
    let (model, tables) = demographic_shape(1);
    let generated = generate(&model).unwrap();
    let layout = build_layout(&model, &tables, &generated).unwrap();
    assert!(estimate_isolated_sweep_capacity(
        &layout,
        &generated,
        8,
        0,
        CudaFinalStateReadbackMode::PackedPinned,
    )
    .unwrap_err()
    .to_string()
    .contains("greater than zero"));
}

#[test]
fn packed_pinned_accounting_is_exact_per_lane_and_zero_aware() {
    let empty_source = r#"{"name":"empty","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[],"transitions":[],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    let empty = sembla_ir::validate(sembla_ir::parse_json(empty_source).unwrap()).unwrap();
    let generated = generate(&empty).unwrap();
    let layout = build_layout(&empty, &[], &generated).unwrap();
    let zero = checked_final_state_component_bytes(0, 0, 0).unwrap();
    assert_eq!(zero.total, 0);
    assert_eq!(zero.state, 0);
    assert_eq!(zero.inputs, 0);
    assert_eq!(zero.input_counts, 0);
    assert!(checked_final_state_component_bytes(usize::MAX, 1, 0)
        .unwrap_err()
        .to_string()
        .contains("final-state byte total overflow"));
    assert!(checked_final_state_component_bytes(0, 0, usize::MAX)
        .unwrap_err()
        .to_string()
        .contains("input-count byte total overflow"));
    let empty_bytes = final_state_component_bytes(&layout).unwrap();
    assert_eq!(empty_bytes, zero);
    let empty_materialized = StateStore::new(&empty, Vec::new()).unwrap();
    assert_eq!(
        hash_state(&empty, &layout, &[], &[], &[]),
        empty_materialized.state_hash()
    );
    let estimate = estimate_isolated_sweep_capacity(
        &layout,
        &generated,
        8,
        4,
        CudaFinalStateReadbackMode::PackedPinned,
    )
    .unwrap();
    assert_eq!(estimate.requested_buffer_set_count, 4);
    let empty_allocations_per_lane = usize::from(empty_bytes.state != 0);
    assert_eq!(
        estimate.requested_underlying_pinned_allocation_count,
        empty_allocations_per_lane * 4
    );
    assert_eq!(estimate.requested_pinned_bytes, empty_bytes.total * 4);
    assert_eq!(
        estimate.requested_cacheable_staging_bytes,
        empty_bytes.total * 4
    );

    let (model, tables) = demographic_shape(3);
    let generated = generate(&model).unwrap();
    let layout = build_layout(&model, &tables, &generated).unwrap();
    let bytes = final_state_component_bytes(&layout).unwrap();
    let workers = 2;
    let pinned = estimate_isolated_sweep_capacity(
        &layout,
        &generated,
        8,
        workers,
        CudaFinalStateReadbackMode::PackedPinned,
    )
    .unwrap();
    assert_eq!(pinned.final_state_bytes_per_lane, bytes);
    assert_eq!(pinned.requested_pinned_bytes_per_lane, bytes.total);
    assert_eq!(
        pinned.requested_cacheable_staging_bytes_per_lane,
        bytes.total
    );
    assert_eq!(pinned.requested_pinned_bytes, bytes.total * workers);
    assert_eq!(
        pinned.requested_cacheable_staging_bytes,
        bytes.total * workers
    );
    assert_eq!(pinned.requested_buffer_set_count, workers);
    let allocations_per_lane = usize::from(bytes.state != 0)
        + usize::from(bytes.inputs != 0)
        + usize::from(bytes.input_counts != 0);
    assert_eq!(
        pinned.requested_underlying_pinned_allocation_count,
        allocations_per_lane * workers
    );
    assert!(pinned.requested_underlying_pinned_allocation_count <= 3 * workers);

    assert!(estimate_isolated_sweep_capacity(
        &layout,
        &generated,
        8,
        usize::MAX,
        CudaFinalStateReadbackMode::PackedPinned,
    )
    .unwrap_err()
    .to_string()
    .contains("overflow"));

    for mode in [
        CudaFinalStateReadbackMode::Materialized,
        CudaFinalStateReadbackMode::PackedPageable,
    ] {
        let control =
            estimate_isolated_sweep_capacity(&layout, &generated, 8, workers, mode).unwrap();
        assert_eq!(control.requested_pinned_bytes, 0);
        assert_eq!(control.requested_cacheable_staging_bytes, 0);
        assert_eq!(control.requested_buffer_set_count, 0);
        assert_eq!(control.requested_underlying_pinned_allocation_count, 0);
    }
}

#[test]
fn cacheable_staging_allocation_is_fallible_and_zero_safe() {
    let empty =
        allocate_cacheable_staging::<u64>(0, "input counts", FinalStateAllocationInjection::None)
            .unwrap();
    assert!(empty.is_empty());
    assert_eq!(empty.capacity(), 0);

    let error = allocate_cacheable_staging::<u8>(
        17,
        "state",
        FinalStateAllocationInjection::Staging("state"),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("injected packed-pinned cacheable staging allocation failure"));
    assert!(error.contains("requested 17 bytes"));
    assert!(error.contains("one lane"));
    assert!(!error.contains("fallback"));
    assert!(FinalStateAllocationInjection::Pinned("state").rejects_pinned("state"));
    assert!(!FinalStateAllocationInjection::Pinned("inputs").rejects_pinned("state"));
}

#[test]
fn canonical_packed_hash_matches_materialized_v1_mixed_layout() {
    let source = r#"{"name":"hash_mixed","dt":1.0,"params":[],"boxes":[{"name":"world","tables":[{"name":"Target","size_hint":2,"attrs":[]},{"name":"Row","size_hint":2,"attrs":[{"name":"real","ty":{"kind":"real"}},{"name":"int","ty":{"kind":"int"}},{"name":"kind","ty":{"kind":"enum","variants":["a","b"]}},{"name":"target","ty":{"kind":"ref","table":"Target"}}]}],"transitions":[],"inputs":[],"outputs":[],"views":[]}],"wires":[],"summaries":[]}"#;
    let model = sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap();
    for rows in [0, 1, 2] {
        let tables = vec![
            TableInit::new("world", "Target", 2, Vec::new()),
            TableInit::new(
                "world",
                "Row",
                rows,
                vec![
                    ColumnInit::new("real", ColumnData::Real([1.25, -3.5][..rows].to_vec())),
                    ColumnInit::new("int", ColumnData::Int([4, -9][..rows].to_vec())),
                    ColumnInit::new("kind", ColumnData::Enum([0, 1][..rows].to_vec())),
                    ColumnInit::new("target", ColumnData::Ref([1, 0][..rows].to_vec())),
                ],
            ),
        ];
        let generated = generate(&model).unwrap();
        let layout = build_layout(&model, &tables, &generated).unwrap();
        let packed_state = pack_initial_state(&model, &tables, &layout).unwrap();
        assert_eq!(layout.state_len % 8, 0);
        assert_eq!(layout.input_len % 8, 0);
        assert_eq!(layout.aggregate_len % 8, 0);
        assert!(layout.state_logical_len <= layout.state_len);
        for slot in 0..3 {
            for offset in &layout.column_offsets {
                assert_eq!((slot * layout.state_len + *offset as usize) % 8, 0);
            }
        }
        let materialized = StateStore::new(&model, tables).unwrap();
        assert_eq!(
            hash_state(&model, &layout, &packed_state, &[0], &[0]),
            materialized.state_hash()
        );
    }
}

#[test]
fn canonical_packed_hash_matches_materialized_v2_inputs_and_negative_control() {
    let source = r#"{"name":"hash_inputs","dt":1.0,"params":[],"boxes":[{"name":"source","tables":[{"name":"Event","size_hint":2,"attrs":[{"name":"amount","ty":{"kind":"int"}}]}],"transitions":[],"inputs":[],"outputs":[{"name":"events","schema":[{"name":"amount","ty":{"kind":"int"}}],"builder":{"kind":"per_table","table":"Event","fields":[{"name":"amount","op":{"kind":"sum","value":{"kind":"self_attr","name":"amount"}},"filter":null}]}}],"views":[]},{"name":"sink","tables":[],"transitions":[],"inputs":[{"name":"events","schema":[{"name":"amount","ty":{"kind":"int"}}]}],"outputs":[],"views":[]}],"wires":[{"from":{"box":"source","port":"events"},"to":{"box":"sink","port":"events"}}],"summaries":[]}"#;
    let model = sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap();
    let tables = vec![TableInit::new(
        "source",
        "Event",
        2,
        vec![ColumnInit::new("amount", ColumnData::Int(vec![4, 9]))],
    )];
    let generated = generate(&model).unwrap();
    let layout = build_layout(&model, &tables, &generated).unwrap();
    assert_eq!(layout.ports.len(), 1);
    let packed_state = pack_initial_state(&model, &tables, &layout).unwrap();
    let values = ColumnData::Int(vec![13]);
    let mut packed_inputs = vec![0_u8; layout.input_len.max(1)];
    write_column(
        &mut packed_inputs,
        layout.input_offsets[0] as usize,
        &values,
    );
    let input_counts = vec![1_u64];
    let input_table = InputTable {
        box_name: "sink".to_owned(),
        port_name: "events".to_owned(),
        schema: model.model().boxes[1].inputs[0].schema.clone(),
        row_count: 1,
        columns: vec![values],
    };
    let mut materialized = StateStore::new(&model, tables.clone()).unwrap();
    materialized
        .refresh_backend_snapshot(&model, &tables, vec![input_table])
        .unwrap();
    let packed = hash_state(
        &model,
        &layout,
        &packed_state,
        &packed_inputs,
        &input_counts,
    );
    assert_eq!(packed, materialized.state_hash());

    packed_inputs[layout.input_offsets[0] as usize] ^= 1;
    assert_ne!(
        hash_state(
            &model,
            &layout,
            &packed_state,
            &packed_inputs,
            &input_counts,
        ),
        packed
    );
}
