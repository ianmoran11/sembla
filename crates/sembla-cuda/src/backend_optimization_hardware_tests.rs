use super::{CudaBackend, HashMode, ParamEnv};
use sembla_runtime::core::{ColumnData, ColumnInit, StateStore, TableInit};

#[test]
#[ignore = "requires CUDA hardware"]
fn optimized_reductions_match_cpu_for_group_sizes_and_fused_resets() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sembla-cli/tests/fixtures/grouped_observation.json");
    let mut model = sembla_ir::parse_json(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut no_effects = model.boxes[0].transitions[0].clone();
    "no_effects".clone_into(&mut no_effects.name);
    no_effects.effects.clear();
    no_effects.guard = sembla_ir::Expr::Bool { value: true };
    no_effects.hazard = sembla_ir::Expr::Real { value: 1000.0 };
    model.boxes[0].transitions.push(no_effects);
    let features =
        sembla_ir::FeatureSet::from([sembla_ir::GROUPED_OBSERVATIONS_FEATURE.to_owned()]);
    let model = sembla_ir::validate_with_features(model, &features).unwrap();
    let params = ParamEnv::defaults(&model);
    for (rows, bands) in [(0, 17), (1, 17), (1027, 17), (1027, 173)] {
        let initial = vec![
            TableInit::new("world", "area", 12, vec![]),
            TableInit::new(
                "world",
                "person_slot",
                rows,
                vec![
                    ColumnInit::new(
                        "sex",
                        ColumnData::Enum((0..rows).map(|i| (i % 2) as u16).collect()),
                    ),
                    ColumnInit::new(
                        "area",
                        ColumnData::Ref((0..rows).map(|i| (i % 12) as u32).collect()),
                    ),
                    ColumnInit::new(
                        "age_months",
                        ColumnData::Int(
                            (0..rows)
                                .map(|i| (i as i64 % bands - bands / 2) * 60 + i as i64 % 60)
                                .collect(),
                        ),
                    ),
                    ColumnInit::new(
                        "occupancy",
                        ColumnData::Enum((0..rows).map(|i| u16::from(i % 3 == 0)).collect()),
                    ),
                ],
            ),
        ];
        let seeds = [19, 9009];
        let mut backend = CudaBackend::new(
            &model,
            initial.clone(),
            &params,
            seeds[0],
            HashMode::FinalOnly,
        )
        .unwrap();
        let mut fused = CudaBackend::new_fused_batch(
            &model,
            initial.clone(),
            &params,
            seeds[0],
            2,
            HashMode::FinalOnly,
        )
        .unwrap();
        for width in [2, 1, 2] {
            backend.reset_draw(&params, seeds[0]).unwrap();
            fused
                .reset_fused_batch(&vec![params.clone(); width], &seeds[..width])
                .unwrap();
            let mut cpu: Vec<_> = (0..width)
                .map(|_| StateStore::new(&model, initial.clone()).unwrap())
                .collect();
            for tick in 0..3 {
                let actual = backend.run_tick_observed_reused().unwrap();
                let batch = fused.run_tick_observed_reused_fused().unwrap();
                for (slot, result) in batch.into_iter().enumerate() {
                    let expected = sembla_cpu::run_tick_with_features(
                        &model,
                        &mut cpu[slot],
                        &params,
                        seeds[slot],
                        tick,
                        &features,
                    )
                    .unwrap();
                    let (_, fired, deferred, views) = result.unwrap();
                    assert_eq!(fired, expected.fired_per_box);
                    assert_eq!(deferred, expected.deferred_per_resource_table);
                    let views = views.unwrap();
                    assert_eq!(views.views, expected.views);
                    assert_eq!(views.grouped_views, expected.grouped_views);
                    assert_eq!(
                        fused.fused_observed_state(slot).unwrap().state_hash(),
                        cpu[slot].state_hash()
                    );
                    if slot == 0 {
                        assert_eq!(actual.1, fired);
                        assert_eq!(actual.2, deferred);
                        assert_eq!(
                            actual.3.as_ref().unwrap().grouped_views,
                            views.grouped_views
                        );
                        assert_eq!(
                            backend.ensure_observed_state().unwrap().state_hash(),
                            cpu[slot].state_hash()
                        );
                    }
                }
            }
        }
    }
}
