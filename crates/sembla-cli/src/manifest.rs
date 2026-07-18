use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const HASH_ALGORITHM: &str = "sha256";
pub const DETERMINISM_LEVEL: &str = "A";
const MANIFEST_SCHEMA_VERSION: u32 = 1;
const BACKEND_IDENTITY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestKind {
    Run,
    Sweep,
    Compare,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResolvedValue {
    // Integer must be attempted first during untagged deserialization; JSON
    // integer tokens are also accepted by f64 visitors.
    Int(i64),
    Real(f64),
}

impl From<&sembla_ir::ParamValue> for ResolvedValue {
    fn from(value: &sembla_ir::ParamValue) -> Self {
        match value {
            sembla_ir::ParamValue::Real { value } => Self::Real(*value),
            sembla_ir::ParamValue::Int { value } => Self::Int(*value),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PopulationSource {
    Numeric(u64),
    File(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendIdentity {
    pub backend: String,
    pub fell_back: bool,
    pub precision: String,
}

impl BackendIdentity {
    pub fn cpu_oracle() -> Self {
        Self {
            backend: "cpu-oracle".to_owned(),
            precision: "f64".to_owned(),
            fell_back: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManifestExecution {
    pub k: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ir_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dt: Option<f64>,
    pub resolved_theta: BTreeMap<String, ResolvedValue>,
    pub results_sha256: String,
    pub final_state_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunManifest {
    pub backend_identity: Option<BackendIdentity>,
    pub component_versions: BTreeMap<String, String>,
    pub determinism_level: String,
    pub enabled_flags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub executions: Vec<ManifestExecution>,
    pub final_state_hash_algorithm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_state_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ir_hash: Option<String>,
    pub ir_hash_algorithm: String,
    pub manifest_kind: ManifestKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub population_hash_algorithm: String,
    pub population_sha256: String,
    pub population_source: PopulationSource,
    #[serde(default)]
    pub resolved_theta: BTreeMap<String, ResolvedValue>,
    pub results_hash_algorithm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results_sha256: Option<String>,
    pub schema_versions: BTreeMap<String, u32>,
    pub seed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dt: Option<f64>,
    pub ticks: u32,
}

impl RunManifest {
    pub fn new(
        kind: ManifestKind,
        seed: u64,
        ticks: u32,
        population_source: PopulationSource,
        population_sha256: String,
    ) -> Self {
        let mut schema_versions = BTreeMap::new();
        schema_versions.insert(
            "backend_identity".to_owned(),
            BACKEND_IDENTITY_SCHEMA_VERSION,
        );
        schema_versions.insert("manifest".to_owned(), MANIFEST_SCHEMA_VERSION);
        let mut component_versions = BTreeMap::new();
        component_versions.insert(
            "sembla-cli".to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
        );
        component_versions.insert("sembla-ir".to_owned(), sembla_ir::VERSION.to_owned());
        component_versions.insert(
            "sembla-runtime".to_owned(),
            sembla_runtime::VERSION.to_owned(),
        );
        Self {
            backend_identity: Some(BackendIdentity::cpu_oracle()),
            component_versions,
            determinism_level: DETERMINISM_LEVEL.to_owned(),
            enabled_flags: Vec::new(),
            executions: Vec::new(),
            final_state_hash_algorithm: HASH_ALGORITHM.to_owned(),
            final_state_sha256: None,
            ir_hash: None,
            ir_hash_algorithm: HASH_ALGORITHM.to_owned(),
            manifest_kind: kind,
            model: None,
            population_hash_algorithm: HASH_ALGORITHM.to_owned(),
            population_sha256,
            population_source,
            resolved_theta: BTreeMap::new(),
            results_hash_algorithm: HASH_ALGORITHM.to_owned(),
            results_sha256: None,
            schema_versions,
            seed,
            dt: None,
            ticks,
        }
    }
}

pub fn resolved_theta(params: &sembla_runtime::eval::ParamEnv) -> BTreeMap<String, ResolvedValue> {
    params
        .values()
        .map(|(name, value)| (name.to_owned(), ResolvedValue::from(value)))
        .collect()
}

pub fn canonical_ir_hash(model: &sembla_ir::ValidatedModel) -> Result<String, String> {
    let canonical = sembla_ir::to_canonical_json(model.model())
        .map_err(|error| format!("canonical IR serialization failed: {error}"))?;
    Ok(hex(&Sha256::digest(canonical.as_bytes())))
}

pub fn population_identity(spec: &str) -> Result<(PopulationSource, String), String> {
    if let Ok(value) = spec.parse::<u64>() {
        let canonical = format!("{value}\n");
        return Ok((
            PopulationSource::Numeric(value),
            hex(&Sha256::digest(canonical.as_bytes())),
        ));
    }
    let path = Path::new(spec);
    let basename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("population path '{spec}' has no UTF-8 basename"))?;
    let bytes = std::fs::read(path).map_err(|error| format!("{spec}: {error}"))?;
    Ok((
        PopulationSource::File(basename.to_owned()),
        hex(&Sha256::digest(bytes)),
    ))
}

pub fn sidecar_path(output: &str) -> PathBuf {
    PathBuf::from(format!("{output}.manifest.json"))
}

pub fn write(path: &Path, manifest: &RunManifest) -> Result<(), String> {
    let bytes = to_canonical_json(manifest)?;
    std::fs::write(path, bytes.as_bytes()).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn read(path: &Path) -> Result<RunManifest, String> {
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&source).map_err(|error| format!("{}: {error}", path.display()))?;
    validate_schema_versions(&value)?;
    validate_backend_identity_tuple(&value)?;
    // Deserialize from the original bytes rather than through `Value`: the
    // latter can round an already-parsed f64 during `from_value`, which would
    // break exact replay of sampled sweep parameters.
    let manifest: RunManifest =
        serde_json::from_str(&source).map_err(|error| format!("{}: {error}", path.display()))?;
    validate_algorithms(&manifest)?;
    Ok(manifest)
}

pub fn to_canonical_json(manifest: &RunManifest) -> Result<String, String> {
    let mut normalized = manifest.clone();
    normalized.enabled_flags.sort();
    normalized.enabled_flags.dedup();
    let value = serde_json::to_value(normalized).map_err(|error| error.to_string())?;
    let mut json = serde_json::to_string(&sort_json(value)).map_err(|error| error.to_string())?;
    json.push('\n');
    Ok(json)
}

fn sort_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let mut entries = object.into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let object = entries
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect();
            serde_json::Value::Object(object)
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(sort_json).collect())
        }
        other => other,
    }
}

fn validate_schema_versions(value: &serde_json::Value) -> Result<(), String> {
    let versions = value
        .get("schema_versions")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "manifest schema_versions must be an object".to_owned())?;
    for (concern, expected) in [
        ("manifest", MANIFEST_SCHEMA_VERSION),
        ("backend_identity", BACKEND_IDENTITY_SCHEMA_VERSION),
    ] {
        let actual = versions
            .get(concern)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("manifest schema_versions.{concern} is missing or invalid"))?;
        if actual != u64::from(expected) {
            return Err(format!(
                "unknown schema_versions major for {concern}: {actual} (supported: {expected})"
            ));
        }
    }
    for concern in versions.keys() {
        if concern != "manifest" && concern != "backend_identity" {
            return Err(format!(
                "unknown schema_versions concern '{concern}' and major {}",
                versions[concern]
            ));
        }
    }
    Ok(())
}

fn validate_backend_identity_tuple(value: &serde_json::Value) -> Result<(), String> {
    let Some(identity) = value.get("backend_identity") else {
        return Ok(());
    };
    if identity.is_null() {
        return Ok(());
    }
    let object = identity.as_object().ok_or_else(|| {
        "backend_identity tuple must be all-present or all-absent and must be an object".to_owned()
    })?;
    let missing = ["backend", "precision", "fell_back"]
        .into_iter()
        .filter(|field| !object.contains_key(*field))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "backend_identity tuple must be all-present or all-absent; missing {}",
            missing.join(", ")
        ));
    }
    Ok(())
}

fn validate_algorithms(manifest: &RunManifest) -> Result<(), String> {
    for (field, value) in [
        ("ir_hash_algorithm", manifest.ir_hash_algorithm.as_str()),
        (
            "population_hash_algorithm",
            manifest.population_hash_algorithm.as_str(),
        ),
        (
            "results_hash_algorithm",
            manifest.results_hash_algorithm.as_str(),
        ),
        (
            "final_state_hash_algorithm",
            manifest.final_state_hash_algorithm.as_str(),
        ),
    ] {
        if value != HASH_ALGORITHM {
            return Err(format!(
                "unsupported {field} '{value}' (supported: '{HASH_ALGORITHM}')"
            ));
        }
    }
    Ok(())
}

pub fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{read, to_canonical_json, ManifestKind, PopulationSource, RunManifest};
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
    fn integer_parameter_type_survives_manifest_round_trip() {
        let path = temp_file("integer-round-trip");
        let mut manifest = RunManifest::new(
            ManifestKind::Run,
            1,
            2,
            PopulationSource::Numeric(10),
            "abc".to_owned(),
        );
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
}
