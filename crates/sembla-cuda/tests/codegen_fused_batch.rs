use sembla_cuda::{generate, generate_fused_batch};

fn sir_model() -> sembla_ir::ValidatedModel {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sir.json");
    let source = std::fs::read_to_string(path).unwrap();
    sembla_ir::validate(sembla_ir::parse_json(&source).unwrap()).unwrap()
}

fn kernels(source: &str) -> Vec<(&str, &str)> {
    let marker = "extern \"C\" __global__ void ";
    let mut output = Vec::new();
    let mut remaining = source;
    while let Some(start) = remaining.find(marker) {
        remaining = &remaining[start + marker.len()..];
        let open = remaining.find('(').unwrap();
        let name = remaining[..open].trim();
        let end = remaining
            .find("\n}\n")
            .map(|offset| offset + 2)
            .unwrap_or(remaining.len());
        output.push((name, &remaining[..end]));
        remaining = &remaining[end..];
    }
    output
}

#[test]
fn fused_source_adds_one_grid_y_slot_contract_to_every_simulation_kernel() {
    let generated = generate_fused_batch(&sir_model()).unwrap();
    let kernels = kernels(&generated.source);
    assert!(kernels.len() > 30);
    for (name, body) in kernels {
        if name == "sembla_philox_vectors" {
            assert!(!body.contains("sembla_batch_strides"));
            assert!(!body.contains("blockIdx.y"));
            continue;
        }
        assert!(
            body.contains("const unsigned long long* sembla_batch_strides"),
            "{name} is missing the stride table"
        );
        assert!(
            body.contains("const unsigned char* sembla_batch_active"),
            "{name} is missing the active-slot mask"
        );
        assert!(
            body.contains("const unsigned int sembla_slot = blockIdx.y;"),
            "{name} is missing blockIdx.y slot selection"
        );
        assert!(
            body.contains("sembla_batch_active[sembla_slot] == 0U"),
            "{name} is missing inactive-tail suppression"
        );
    }
}

#[test]
fn fused_source_rebases_representative_mutable_families_but_not_layout_metadata() {
    let source = generate_fused_batch(&sir_model()).unwrap().source;
    for pointer in [
        "state",
        "next_state",
        "inputs",
        "next_inputs",
        "input_counts",
        "next_input_counts",
        "params",
        "aggs",
        "aggregate_facts",
        "enabled",
        "times",
        "wins",
        "deferred",
        "instance_resources",
        "winner_keys",
        "owners",
        "owner_values",
        "output_partials",
        "status",
        "effect_active",
    ] {
        assert!(
            source.contains(&format!(
                "{pointer} += (unsigned long long)sembla_slot * sembla_batch_strides["
            )),
            "missing draw-major binding for {pointer}"
        );
    }
    for pointer in [
        "column_offsets",
        "row_counts",
        "input_offsets",
        "agg_offsets",
        "candidate_offsets",
        "claim_instance_offsets",
        "resource_offsets",
        "write_offsets",
    ] {
        assert!(
            !source.contains(&format!("{pointer} +=")),
            "immutable metadata {pointer} must remain shared"
        );
    }
}

#[test]
fn every_non_metadata_kernel_pointer_is_explicitly_shared_or_draw_major() {
    let source = generate_fused_batch(&sir_model()).unwrap().source;
    let shared = [
        "column_offsets",
        "row_counts",
        "resource_offsets",
        "input_offsets",
        "next_input_offsets",
        "agg_offsets",
        "candidate_offsets",
        "claim_instance_offsets",
        "write_offsets",
        "sembla_batch_strides",
        "sembla_batch_active",
        "sembla_batch_seeds",
    ];
    for (kernel, body) in kernels(&source) {
        if kernel == "sembla_philox_vectors" {
            continue;
        }
        let signature = body.split_once('{').unwrap().0;
        let arguments = signature
            .split_once('(')
            .unwrap()
            .1
            .rsplit_once(')')
            .unwrap()
            .0;
        for argument in arguments
            .split(',')
            .filter(|argument| argument.contains('*'))
        {
            let name = argument
                .split_whitespace()
                .last()
                .unwrap()
                .trim_start_matches('*');
            if shared.contains(&name) {
                continue;
            }
            assert!(
                body.contains(&format!(
                    "{name} += (unsigned long long)sembla_slot * sembla_batch_strides["
                )),
                "{kernel} pointer {name} is neither shared nor draw-major"
            );
        }
    }
}

#[test]
fn fused_transition_uses_per_slot_seed_without_changing_philox_draw_coordinate() {
    let source = generate_fused_batch(&sir_model()).unwrap().source;
    assert!(source.contains("seed = sembla_batch_seeds[sembla_slot];"));
    assert!(source.contains("sembla_exp(seed, tick"));
    assert!(source.contains(", 0U, lambda);"));
    assert!(!source.contains("sembla_slot, lambda"));
}

#[test]
fn ordinary_generation_remains_free_of_the_fused_abi() {
    let ordinary = generate(&sir_model()).unwrap();
    assert!(!ordinary.source.contains("sembla_batch_strides"));
    assert!(!ordinary
        .source
        .contains("const unsigned int sembla_slot = blockIdx.y"));
    assert_eq!(ordinary, generate(&sir_model()).unwrap());
}
