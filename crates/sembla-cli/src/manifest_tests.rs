use super::{
    read, to_canonical_json, BackendIdentity, ManifestKind, PopulationSource, RunManifest,
};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_file(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sembla-manifest-{label}-{nonce}.json"))
}

#[test]
fn canonical_json_is_compact_sorted_and_has_one_newline() {
    let manifest = RunManifest::new(
        ManifestKind::Run,
        1,
        2,
        PopulationSource::Numeric(10),
        "abc".to_owned(),
    );
    let json = to_canonical_json(&manifest).unwrap();
    assert!(json.ends_with('\n'));
    assert!(!json.ends_with("\n\n"));
    assert!(!json.contains("\n  "));
    assert!(json.find("\"backend_identity\"").unwrap() < json.find("\"seed\"").unwrap());
}

#[test]
fn numeric_population_hash_uses_canonical_decimal_and_newline() {
    let (source, hash) = super::population_identity("20").unwrap();
    assert_eq!(source, PopulationSource::Numeric(20));
    assert_eq!(
        hash,
        "5378796307535df3ec8d8b15a2e2dc5641419c3d3060cfe32238c0fa973f7aa3"
    );
}

#[test]
fn linked_plan_identity_tuple_copies_validated_provenance() {
    let sembla_ir::ParsedInput::Plan(mut plan) =
        sembla_ir::parse_input(include_str!("../../../fixtures/plans/two_box.plan.json")).unwrap()
    else {
        panic!("fixture did not parse as a plan")
    };
    plan.origin = sembla_ir::PlanOrigin::Linked;
    plan.linked_provenance = Some(sembla_ir::LinkedProvenanceV1 {
        source_hash: sembla_ir::HashRecordV1 {
            algorithm: "sha256".to_owned(),
            domain: "sembla.source-artifact/v1".to_owned(),
            digest: "0".repeat(64),
        },
        linker: sembla_ir::LinkerDescriptorV1 {
            semantics: "sembla.linker/v1".to_owned(),
            source_schema: "sembla.composition-source/v1".to_owned(),
            plan_schema: "sembla.executable-plan/v1".to_owned(),
            identity_scheme: "sembla.identity/stable-v1".to_owned(),
            canonical_encoding: "sembla.canonical-json/v1".to_owned(),
            source_map_schema: "sembla.source-map/v1".to_owned(),
        },
        source_map: serde_json::json!({}),
    });
    for (index, mailbox) in plan.identity.mailboxes.iter_mut().enumerate() {
        mailbox.identity = sembla_ir::mailbox_identity(
            &format!("occ:#wire:declared_{index}"),
            &mailbox.source_box,
            &mailbox.source_port,
            &mailbox.target_box,
            &mailbox.target_port,
        );
    }
    plan.identity
        .mailboxes
        .sort_by(|left, right| left.identity.cmp(&right.identity));
    let mailboxes = plan.identity.mailboxes.clone();
    plan.model.wires.sort_by_key(|wire| {
        mailboxes
            .iter()
            .find(|mailbox| {
                mailbox.source_box == wire.from.r#box
                    && mailbox.source_port == wire.from.port
                    && mailbox.target_box == wire.to.r#box
                    && mailbox.target_port == wire.to.port
            })
            .unwrap()
            .identity
            .clone()
    });
    sembla_ir::validate_plan(&plan).unwrap();
    let (identity, linked) = super::plan_identity_tuples(&plan).unwrap();
    assert_eq!(identity.origin, "linked");
    let linked = linked.unwrap();
    assert_eq!(linked.linker_semantics, "sembla.linker/v1");
    assert_eq!(
        linked.source_hash,
        plan.linked_provenance.unwrap().source_hash
    );
}

#[test]
fn reader_rejects_partial_observation_hash_tuples() {
    let path = temp_file("partial-observation");
    let mut missing_algorithm = RunManifest::new(
        ManifestKind::Run,
        1,
        2,
        PopulationSource::Numeric(10),
        "abc".to_owned(),
    );
    missing_algorithm.observation_hash_algorithm = None;
    missing_algorithm.observation_sha256 = Some("hash".to_owned());
    std::fs::write(&path, to_canonical_json(&missing_algorithm).unwrap()).unwrap();
    let error = read(&path).unwrap_err();
    assert!(error.contains("observation hash tuple"), "{error}");

    let missing_hash = RunManifest::new(
        ManifestKind::Run,
        1,
        2,
        PopulationSource::Numeric(10),
        "abc".to_owned(),
    );
    std::fs::write(&path, to_canonical_json(&missing_hash).unwrap()).unwrap();
    let error = read(&path).unwrap_err();
    assert!(error.contains("observation hash tuple"), "{error}");
    let _ = std::fs::remove_file(path);
}

#[test]
fn integer_parameter_type_survives_manifest_round_trip() {
    let path = temp_file("integer-round-trip");
    let mut manifest = RunManifest::new(
        ManifestKind::Run,
        1,
        2,
        PopulationSource::Numeric(10),
        "abc".to_owned(),
    );
    manifest.observation_sha256 = Some("observation".to_owned());
    manifest
        .resolved_theta
        .insert("count".to_owned(), super::ResolvedValue::Int(3));
    std::fs::write(&path, to_canonical_json(&manifest).unwrap()).unwrap();
    let parsed = read(&path).unwrap();
    assert_eq!(parsed.resolved_theta["count"], super::ResolvedValue::Int(3));
    let _ = std::fs::remove_file(path);
}

#[test]
fn real_parameter_bits_survive_manifest_round_trip() {
    let path = temp_file("real-round-trip");
    let mut manifest = RunManifest::new(
        ManifestKind::Run,
        1,
        2,
        PopulationSource::Numeric(10),
        "abc".to_owned(),
    );
    let value = 0.10963506619780773_f64;
    manifest.observation_sha256 = Some("observation".to_owned());
    manifest
        .resolved_theta
        .insert("gamma".to_owned(), super::ResolvedValue::Real(value));
    std::fs::write(&path, to_canonical_json(&manifest).unwrap()).unwrap();
    let parsed = read(&path).unwrap();
    let super::ResolvedValue::Real(actual) = parsed.resolved_theta["gamma"] else {
        panic!("gamma was not real")
    };
    assert_eq!(actual.to_bits(), value.to_bits());
    let _ = std::fs::remove_file(path);
}

#[test]
fn reader_rejects_partial_backend_identity_tuple_by_name() {
    let path = temp_file("partial-backend");
    std::fs::write(
        &path,
        r#"{"backend_identity":{"backend":"cpu-oracle","precision":"f64"},"schema_versions":{"backend_identity":1,"manifest":1}}"#,
    )
    .unwrap();
    let error = read(&path).unwrap_err();
    assert!(error.contains("backend_identity tuple"), "{error}");
    assert!(error.contains("fell_back"), "{error}");
    let _ = std::fs::remove_file(path);
}

#[test]
fn reader_rejects_partial_cuda_backend_identity() {
    let path = temp_file("partial-cuda-backend");
    let mut manifest = RunManifest::new(
        ManifestKind::Run,
        1,
        2,
        PopulationSource::Numeric(10),
        "abc".to_owned(),
    );
    manifest.backend_identity = Some(BackendIdentity {
        backend: "cuda-native-f64".to_owned(),
        precision: "f64".to_owned(),
        fell_back: false,
        gpu_model: Some("GPU".to_owned()),
        driver_version: None,
    });
    manifest.observation_sha256 = Some("observation".to_owned());
    std::fs::write(&path, to_canonical_json(&manifest).unwrap()).unwrap();
    let error = read(&path).unwrap_err();
    assert!(error.contains("gpu_model and driver_version"), "{error}");
    let _ = std::fs::remove_file(path);
}

#[test]
fn reader_rejects_partial_theta_source_tuple_by_name() {
    let path = temp_file("partial-theta-source");
    std::fs::write(
        &path,
        r#"{"backend_identity":null,"schema_versions":{"backend_identity":1,"manifest":1},"theta_source":{"kind":"file","sha256":"abc"}}"#,
    )
    .unwrap();
    let error = read(&path).unwrap_err();
    assert!(error.contains("theta_source tuple"), "{error}");
    assert!(error.contains("algorithm"), "{error}");
    let _ = std::fs::remove_file(path);
}

#[test]
fn reader_rejects_invalid_state_artifact_tuple_values() {
    let path = temp_file("invalid-state-artifact-values");
    let mut manifest = RunManifest::new(
        ManifestKind::Run,
        1,
        2,
        PopulationSource::Numeric(10),
        "abc".to_owned(),
    );
    manifest.observation_sha256 = Some("observation".to_owned());
    manifest.initial_state = Some(super::StateArtifactTuple {
        format: "sembla.state/v2".to_owned(),
        hash: sembla_ir::HashRecordV1 {
            algorithm: "sha256".to_owned(),
            domain: "sembla.state-artifact/v1".to_owned(),
            digest: "0".repeat(64),
        },
    });
    std::fs::write(&path, to_canonical_json(&manifest).unwrap()).unwrap();
    let error = read(&path).unwrap_err();
    assert!(error.contains("initial_state.format"), "{error}");

    "sembla.state/v1".clone_into(&mut manifest.initial_state.as_mut().unwrap().format);
    "wrong-domain".clone_into(&mut manifest.initial_state.as_mut().unwrap().hash.domain);
    std::fs::write(&path, to_canonical_json(&manifest).unwrap()).unwrap();
    let error = read(&path).unwrap_err();
    assert!(error.contains("initial_state.hash.domain"), "{error}");
    let _ = std::fs::remove_file(path);
}

#[test]
fn reader_rejects_partial_state_artifact_tuples() {
    let path = temp_file("partial-state-artifact");
    for tuple_name in ["initial_state", "exported_state"] {
        for missing in ["format", "hash"] {
            let mut tuple = serde_json::json!({
                "format": "sembla.state/v1",
                "hash": {
                    "algorithm": "sha256",
                    "domain": "sembla.state-artifact/v1",
                    "digest": "0".repeat(64)
                }
            });
            tuple.as_object_mut().unwrap().remove(missing);
            let mut value = serde_json::json!({
                "backend_identity": null,
                "schema_versions": {"backend_identity": 1, "manifest": 1}
            });
            value[tuple_name] = tuple;
            std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            let error = read(&path).unwrap_err();
            assert!(error.contains(&format!("{tuple_name} tuple")), "{error}");
            assert!(error.contains(missing), "{error}");
        }

        let mut value = serde_json::json!({
            "backend_identity": null,
            "schema_versions": {"backend_identity": 1, "manifest": 1}
        });
        value[tuple_name] = serde_json::json!({
            "format": "sembla.state/v1",
            "hash": {
                "algorithm": "sha256",
                "domain": "sembla.state-artifact/v1"
            }
        });
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let error = read(&path).unwrap_err();
        assert!(
            error.contains(&format!("{tuple_name}.hash tuple")),
            "{error}"
        );
        assert!(error.contains("digest"), "{error}");
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn reader_rejects_partial_plan_and_linked_source_tuples() {
    let path = temp_file("partial-plan-identity");
    let complete_plan = serde_json::json!({
        "plan_schema": "sembla.executable-plan/v1",
        "identity_scheme": "sembla.identity/stable-v1",
        "origin": "direct_stable",
        "plan_semantic_hash": {
            "algorithm": "sha256",
            "domain": "sembla.plan-core/v1",
            "digest": "0".repeat(64)
        },
        "enabled_features": []
    });
    for field in [
        "plan_schema",
        "identity_scheme",
        "origin",
        "plan_semantic_hash",
        "enabled_features",
    ] {
        let mut plan = complete_plan.clone();
        plan.as_object_mut().unwrap().remove(field);
        let value = serde_json::json!({
            "backend_identity": null,
            "plan": plan,
            "schema_versions": {"backend_identity": 1, "manifest": 1}
        });
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let error = read(&path).unwrap_err();
        assert!(error.contains("plan tuple"), "{field}: {error}");
        assert!(error.contains(field), "{field}: {error}");
    }

    let mut partial_hash = complete_plan.clone();
    partial_hash["plan_semantic_hash"]
        .as_object_mut()
        .unwrap()
        .remove("digest");
    let value = serde_json::json!({
        "backend_identity": null,
        "plan": partial_hash,
        "schema_versions": {"backend_identity": 1, "manifest": 1}
    });
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let error = read(&path).unwrap_err();
    assert!(error.contains("plan.plan_semantic_hash tuple"), "{error}");
    assert!(error.contains("digest"), "{error}");

    let mut linked_plan = complete_plan;
    linked_plan["origin"] = serde_json::json!("linked");
    let value = serde_json::json!({
        "backend_identity": null,
        "plan": linked_plan,
        "linked_source": {"linker_semantics": "sembla.linker/v1"},
        "schema_versions": {"backend_identity": 1, "manifest": 1}
    });
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let error = read(&path).unwrap_err();
    assert!(error.contains("linked_source tuple"), "{error}");
    assert!(error.contains("source_hash"), "{error}");
    let _ = std::fs::remove_file(path);
}

#[test]
fn reader_rejects_plan_linkage_shape_mismatches() {
    let path = temp_file("plan-linkage-shape");
    let value = serde_json::json!({
        "backend_identity": null,
        "linked_source": {
            "source_hash": {
                "algorithm": "sha256",
                "domain": "sembla.source-artifact/v1",
                "digest": "0".repeat(64)
            },
            "linker_semantics": "sembla.linker/v1"
        },
        "schema_versions": {"backend_identity": 1, "manifest": 1}
    });
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let error = read(&path).unwrap_err();
    assert!(error.contains("linked_source tuple requires"), "{error}");
    let _ = std::fs::remove_file(path);
}

#[test]
fn reader_rejects_unknown_schema_major() {
    let path = temp_file("unknown-schema");
    std::fs::write(
        &path,
        r#"{"backend_identity":null,"schema_versions":{"backend_identity":1,"manifest":2}}"#,
    )
    .unwrap();
    let error = read(&path).unwrap_err();
    assert!(error.contains("schema_versions"), "{error}");
    assert!(error.contains("manifest"), "{error}");
    let _ = std::fs::remove_file(path);
}
