//! Validation, hashing, bundle verification, and structural IR inspection commands.

use super::*;

pub(crate) fn validate_file(path: &str) -> i32 {
    let result = (|| -> Result<(), String> {
        let (source, input) = read_input(path)?;
        match input {
            sembla_ir::ParsedInput::LegacyModel(model) => {
                sembla_ir::validate(model).map_err(|error| format!("{path}: {error}"))?;
            }
            sembla_ir::ParsedInput::Plan(plan) => {
                sembla_ir::validate_plan(&plan).map_err(|error| format!("{path}: {error}"))?;
                require_canonical_plan(path, &source)?;
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

pub(crate) fn plan_hash_file(path: &str) -> i32 {
    let result = (|| -> Result<(), String> {
        let plan = read_validated_plan(path)?;
        let semantic = sembla_ir::plan_semantic_hash(&plan)
            .map_err(|error| format!("{path}: semantic hash failed: {error}"))?;
        let envelope = sembla_ir::plan_envelope_hash(&plan)
            .map_err(|error| format!("{path}: envelope hash failed: {error}"))?;
        println!(
            "semantic {} {} {}",
            semantic.algorithm, semantic.domain, semantic.digest
        );
        println!(
            "envelope {} {} {}",
            envelope.algorithm, envelope.domain, envelope.digest
        );
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

pub(crate) fn state_hash_file(path: &str) -> i32 {
    match state_artifact_hash(path) {
        Ok(hash) => {
            println!("state {} {} {}", hash.algorithm, hash.domain, hash.digest);
            0
        }
        Err(error) => {
            eprintln!("{path}: {error}");
            1
        }
    }
}

pub(crate) fn bundle_verify(directory: &str) -> i32 {
    match bundle_verify_result(Path::new(directory)) {
        Ok(checks) => {
            for check in checks {
                println!("ok {check}");
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

pub(crate) fn require_bundle_hash(
    record_name: &str,
    recorded: &sembla_ir::HashRecordV1,
    actual: &sembla_ir::HashRecordV1,
) -> Result<(), String> {
    if recorded == actual {
        Ok(())
    } else {
        Err(format!(
            "{record_name} mismatch: recorded={} actual={}",
            recorded.digest, actual.digest
        ))
    }
}

pub(crate) fn bundle_verify_result(directory: &Path) -> Result<Vec<&'static str>, String> {
    let (bundle, _) = manifest::read_bundle_manifest(directory)?;
    let mut checks = vec!["bundle-manifest schema, encoding, domains, and canonicality"];

    let source_path = directory.join(&bundle.source.path);
    let source_bytes = std::fs::read(&source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let plan_path = directory.join(&bundle.plan.path);
    let plan_bytes =
        std::fs::read(&plan_path).map_err(|error| format!("{}: {error}", plan_path.display()))?;
    let report_path = directory.join(manifest::BUNDLE_REPORT_PATH);
    let report_bytes = std::fs::read(&report_path)
        .map_err(|error| format!("{}: {error}", report_path.display()))?;

    let source_hash = manifest::source_artifact_hash(&source_bytes);
    require_bundle_hash("source.hash", &bundle.source.hash, &source_hash)?;
    checks.push("composition-source.json source.hash");

    let envelope_hash = manifest::plan_envelope_artifact_hash(&plan_bytes);
    require_bundle_hash(
        "plan.envelope_hash",
        &bundle.plan.envelope_hash,
        &envelope_hash,
    )?;
    checks.push("executable-plan.json plan.envelope_hash");

    let plan_source = std::str::from_utf8(&plan_bytes)
        .map_err(|error| format!("{}: invalid UTF-8: {error}", plan_path.display()))?;
    let parsed = sembla_ir::parse_input(plan_source)
        .map_err(|error| format!("{}: {error}", plan_path.display()))?;
    let sembla_ir::ParsedInput::Plan(plan) = parsed else {
        return Err(format!(
            "{}: expected an executable plan envelope",
            plan_path.display()
        ));
    };
    let semantic_hash = sembla_ir::plan_semantic_hash(&plan)
        .map_err(|error| format!("plan.semantic_hash recomputation failed: {error}"))?;
    require_bundle_hash(
        "plan.semantic_hash",
        &bundle.plan.semantic_hash,
        &semantic_hash,
    )?;
    checks.push("executable-plan.json plan.semantic_hash");

    let integrity = manifest::bundle_integrity_hash(
        &bundle,
        &[
            (manifest::BUNDLE_SOURCE_PATH, source_bytes.as_slice()),
            (manifest::BUNDLE_PLAN_PATH, plan_bytes.as_slice()),
            (manifest::BUNDLE_REPORT_PATH, report_bytes.as_slice()),
        ],
    )?;
    require_bundle_hash(
        "bundle_integrity",
        bundle
            .bundle_integrity
            .as_ref()
            .expect("bundle-manifest validation requires bundle_integrity"),
        &integrity,
    )?;
    checks.push(
        "composition-source.json, executable-plan.json, and link-report.json bundle_integrity",
    );

    sembla_ir::validate_plan(&plan).map_err(|error| format!("{}: {error}", plan_path.display()))?;
    require_canonical_plan(&plan_path.display().to_string(), plan_source)?;
    checks.push("executable-plan.json validation and canonicality");

    let provenance = plan.linked_provenance.as_ref().ok_or_else(|| {
        "manifest/plan agreement: linked plan has no linked_provenance".to_owned()
    })?;
    let origin = match plan.origin {
        sembla_ir::PlanOrigin::Linked => "linked",
        sembla_ir::PlanOrigin::DirectStable => "direct_stable",
    };
    let agreement = [
        ("plan.origin", bundle.plan.origin.as_str(), origin),
        (
            "plan.schema",
            bundle.plan.schema.as_str(),
            plan.schema_version.as_str(),
        ),
        (
            "plan.identity_scheme",
            bundle.plan.identity_scheme.as_str(),
            plan.identity_scheme.as_str(),
        ),
        (
            "source.schema",
            bundle.source.schema.as_str(),
            provenance.linker.source_schema.as_str(),
        ),
        (
            "linker.semantics",
            bundle.linker.semantics.as_str(),
            provenance.linker.semantics.as_str(),
        ),
        (
            "source_map_schema",
            bundle.source_map_schema.as_str(),
            provenance.linker.source_map_schema.as_str(),
        ),
        (
            "canonical_encoding",
            bundle.canonical_encoding.as_str(),
            provenance.linker.canonical_encoding.as_str(),
        ),
    ];
    for (field, manifest_value, plan_value) in agreement {
        if manifest_value != plan_value {
            return Err(format!(
                "manifest/plan agreement failed for {field}: manifest='{manifest_value}' plan='{plan_value}'"
            ));
        }
    }
    if bundle.plan.enabled_features != plan.identity.enabled_features {
        return Err(format!(
            "manifest/plan agreement failed for plan.enabled_features: manifest={:?} plan={:?}",
            bundle.plan.enabled_features, plan.identity.enabled_features
        ));
    }
    if bundle.source.hash != provenance.source_hash {
        return Err(format!(
            "manifest/plan agreement failed for linked_provenance.source_hash: manifest={} plan={}",
            bundle.source.hash.digest, provenance.source_hash.digest
        ));
    }
    checks
        .push("manifest/plan origin, schemas, identity, features, and source provenance agreement");
    Ok(checks)
}

pub(crate) fn diff_ir(left: &str, right: &str) -> i32 {
    let compare = || -> Result<bool, String> {
        let left_model = read_validated(left)?;
        let right_model = read_validated(right)?;
        let left_json = sembla_ir::to_canonical_json(left_model.model())
            .map_err(|error| format!("{left}: canonical serialization failed: {error}"))?;
        let right_json = sembla_ir::to_canonical_json(right_model.model())
            .map_err(|error| format!("{right}: canonical serialization failed: {error}"))?;
        Ok(left_json == right_json)
    };
    match compare() {
        Ok(true) => {
            println!("IR models are semantically identical");
            0
        }
        Ok(false) => {
            eprintln!("IR models differ after canonical normalization: '{left}' != '{right}'");
            1
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}
