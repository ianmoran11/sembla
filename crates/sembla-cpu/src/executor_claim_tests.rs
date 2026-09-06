use super::*;

#[test]
fn resource_counts_deduplicate_each_candidate_and_require_all_claims() {
    let model = sembla_ir::validate(
        sembla_ir::parse_json(include_str!("../../../examples/sir.json")).unwrap(),
    )
    .unwrap();
    let candidate = |row: usize, key, resources: &[(usize, u32)]| Candidate {
        rule_word: 0,
        entity_id: row as u32,
        claims: resources
            .iter()
            .map(|&(table_index, resource_row)| CandidateClaim {
                table_index,
                resource_row,
                ordering: OrderingValue::Int(key),
            })
            .collect(),
    };
    let candidates = vec![
        candidate(0, 0, &[(1, 0), (0, 1), (0, 0)]),
        candidate(1, 1, &[(0, 0), (1, 0), (0, 1), (1, 1)]),
        candidate(2, 0, &[(1, 1)]),
        candidate(3, 0, &[]),
        // Wins its claim on table 0, but loses on table 1: it must not
        // contribute a fired count to either resource table.
        candidate(4, 1, &[(0, 2), (1, 1)]),
    ];
    for reverse in [false, true] {
        let mut candidates = candidates.clone();
        if reverse {
            candidates.reverse();
        }
        let resolution = resolve_claims(&candidates, 3, &model.model().boxes[0]).unwrap();
        let fired_rows = candidates
            .iter()
            .zip(resolution.fires)
            .filter_map(|(candidate, fire)| fire.then_some(candidate.entity_id as usize))
            .collect::<Vec<_>>();
        assert_eq!(
            fired_rows,
            if reverse {
                vec![3, 2, 0]
            } else {
                vec![0, 2, 3]
            }
        );
        assert_eq!(resolution.deferred, vec![1, 2, 0]);
        assert_eq!(resolution.fired_per_resource_table, vec![1, 2, 0]);
    }
}

#[test]
fn uncontested_and_empty_candidates_have_no_resource_counts() {
    let model = sembla_ir::validate(
        sembla_ir::parse_json(include_str!("../../../examples/sir.json")).unwrap(),
    )
    .unwrap();
    for count in [0, 3] {
        let candidates = (0..count)
            .map(|row| Candidate {
                rule_word: 0,
                entity_id: row as u32,
                claims: vec![],
            })
            .collect::<Vec<_>>();
        let resolution = resolve_claims(&candidates, 3, &model.model().boxes[0]).unwrap();
        assert_eq!(resolution.fires, vec![true; count]);
        assert_eq!(resolution.deferred, vec![0; 3]);
        assert_eq!(resolution.fired_per_resource_table, vec![0; 3]);
    }
}
