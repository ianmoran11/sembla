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
    let tables = vec![
        TableInit::new("world", "Target", 2, Vec::new()),
        TableInit::new(
            "world",
            "Row",
            2,
            vec![
                ColumnInit::new("real", ColumnData::Real(vec![1.25, -3.5])),
                ColumnInit::new("int", ColumnData::Int(vec![4, -9])),
                ColumnInit::new("kind", ColumnData::Enum(vec![0, 1])),
                ColumnInit::new("target", ColumnData::Ref(vec![1, 0])),
            ],
        ),
    ];
    let generated = generate(&model).unwrap();
    let layout = build_layout(&model, &tables, &generated).unwrap();
    let packed_state = pack_initial_state(&model, &tables, &layout).unwrap();
    let materialized = StateStore::new(&model, tables).unwrap();
    assert_eq!(
        hash_state(&model, &layout, &packed_state, &[0], &[0]),
        materialized.state_hash()
    );
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
