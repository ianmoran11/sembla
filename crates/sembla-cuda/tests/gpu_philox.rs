#![cfg(feature = "cuda")]

use sembla_cuda::{CudaBackend, HashMode, PhiloxCoordinate};
use sembla_runtime::eval::ParamEnv;
use sembla_runtime::rng::draw_u32x4;
use sembla_runtime::state::{ColumnData, ColumnInit, TableInit};

fn diagnostic_backend() -> CudaBackend {
    let source = include_str!("../../../examples/two_state.json");
    let model = sembla_ir::validate(sembla_ir::parse_json(source).unwrap()).unwrap();
    let params = ParamEnv::defaults(&model);
    CudaBackend::new(
        &model,
        vec![TableInit::new(
            "population",
            "Person",
            1,
            vec![ColumnInit::new("mood", ColumnData::Enum(vec![0]))],
        )],
        &params,
        0,
        HashMode::FinalOnly,
    )
    .expect("CUDA device, driver, and NVRTC are required")
}

#[test]
#[ignore = "requires a CUDA GPU; run crates/sembla-cuda/scripts/run-gpu-tests.sh"]
fn device_philox_is_bit_identical_to_checked_cpu_vectors() {
    // Random123 zero/all-ones/asymmetric vectors plus an ordinary coordinate.
    // These are shared with crates/sembla-runtime/tests/rng.rs.
    let coordinates = [
        PhiloxCoordinate::new(0, 0, 0, 0, 0),
        PhiloxCoordinate::new(u64::MAX, u32::MAX, u32::MAX, u32::MAX, u32::MAX),
        PhiloxCoordinate::new(
            0x299f_31d0_a409_3822,
            0x243f_6a88,
            0x85a3_08d3,
            0x1319_8a2e,
            0x0370_7344,
        ),
        PhiloxCoordinate::new(0x0123_4567_89ab_cdef, 17, 23, 42, 5),
    ];
    let expected = coordinates
        .iter()
        .map(|coordinate| {
            draw_u32x4(
                coordinate.seed,
                coordinate.tick,
                coordinate.rule_id,
                coordinate.entity_id,
                coordinate.draw_index,
            )
        })
        .collect::<Vec<_>>();
    let actual = diagnostic_backend().philox_vectors(&coordinates).unwrap();
    assert_eq!(actual, expected);
}
