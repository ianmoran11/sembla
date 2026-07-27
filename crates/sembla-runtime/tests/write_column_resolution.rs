const EXECUTOR_SOURCE: &str = include_str!("../src/executor.rs");

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let (_, tail) = source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing section start: {start}"));
    let (body, _) = tail
        .split_once(end)
        .unwrap_or_else(|| panic!("missing section end: {end}"));
    body
}

#[test]
fn pending_writes_keep_compact_deferred_identity_without_owned_strings() {
    let pending_destination = section(
        EXECUTOR_SOURCE,
        "struct PendingDestination {",
        "\n}\n\n#[derive(Clone, Debug)]\nstruct PendingWrite",
    );
    assert!(pending_destination.contains("resolution: Result<ResolvedWriteColumn, StateError>"));

    let pending_write = section(
        EXECUTOR_SOURCE,
        "struct PendingWrite {",
        "\n}\n\nstruct TickOutcome",
    );
    assert!(pending_write.contains("destination_index: usize"));
    assert!(!pending_write.contains("String"));
    assert!(!pending_write.contains("transition_name"));
}

#[test]
fn write_application_does_not_regress_to_per_write_name_resolution() {
    assert_eq!(
        EXECUTOR_SOURCE
            .matches("snapshot.resolve_write_column(")
            .count(),
        1,
        "the destination should be resolved in the per-effect staging loop"
    );
    let stage = section(
        EXECUTOR_SOURCE,
        "let mut effect_columns = Vec::with_capacity(transition.effects.len());",
        "\n        for (winner_offset, candidate_index) in winner_indices.into_iter().enumerate()",
    );
    let resolution = section(
        stage,
        "resolution: snapshot.resolve_write_column(",
        "\n            });",
    );
    assert!(
        !resolution.contains('?'),
        "destination errors must be captured during staging, not published early"
    );

    let application = section(
        EXECUTOR_SOURCE,
        "let apply_result = {",
        "\n    if let Err(error) = apply_result",
    );
    assert!(application.contains("let mut writes = state.write_buffer()?;"));
    assert!(application.contains("destinations[write.destination_index]"));
    assert!(application.contains(".map_err(Clone::clone)?;"));

    for setter in ["real", "int", "enum", "ref"] {
        assert!(
            EXECUTOR_SOURCE.contains(&format!("writes.set_resolved_{setter}(")),
            "write application must use the resolved {setter} destination"
        );
        assert!(
            !EXECUTOR_SOURCE.contains(&format!("writes.set_{setter}(")),
            "executor write application must not scan destination names for {setter} writes"
        );
    }
}

#[test]
fn double_write_detection_uses_a_reused_bitmap_without_hashing_or_sorting() {
    let scratch = section(
        EXECUTOR_SOURCE,
        "struct DoubleWriteScratch {",
        "\nstruct TickOutcome",
    );
    assert!(scratch.contains("words: Vec<u64>"));
    assert!(scratch.contains("touched_words: Vec<usize>"));
    assert!(scratch.contains("destination_slots: Vec<usize>"));
    assert!(scratch.contains("columns: Vec<BitmapColumn>"));
    assert!(scratch.contains("thread_local!"));
    assert!(scratch.contains("self.touched_words.drain(..)"));

    let detector = section(
        EXECUTOR_SOURCE,
        "fn detect_double_writes(",
        "\nfn transition_name(",
    );
    assert!(detector.contains("scratch.prepare(pending, destinations)"));
    assert!(detector.contains("scratch.mark(write)"));
    assert!(detector.contains("write_cell(write, destinations)"));
    for forbidden in ["HashMap", "Hasher", ".sort"] {
        assert!(
            !detector.contains(forbidden),
            "double-write detection must not use {forbidden}"
        );
    }
}
