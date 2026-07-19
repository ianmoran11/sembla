use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const HASH_ALGORITHM: &str = "sha256";
pub const DETERMINISM_LEVEL: &str = "A";
const MANIFEST_SCHEMA_VERSION: u32 = 1;
const BACKEND_IDENTITY_SCHEMA_VERSION: u32 = 1;
const PAIRS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestKind {
    Run,
    Sweep,
    Compare,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoiseMode {
    Crn,
    Independent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThetaSourceKind {
    Prior,
    File,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThetaSource {
    pub kind: ThetaSourceKind,
    pub sha256: String,
    pub algorithm: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PairsMetadata {
    pub component_versions: BTreeMap<String, String>,
    pub determinism_level: String,
    pub draws: u32,
    pub dt: f64,
    pub ir_hash: String,
    pub ir_hash_algorithm: String,
    pub model: String,
    pub noise_mode: NoiseMode,
    pub parameter_columns: Vec<String>,
    pub pairs_hash_algorithm: String,
    pub pairs_sha256: String,
    pub schema_versions: BTreeMap<String, u32>,
    pub seed: u64,
    pub summary_columns: Vec<String>,
    pub theta_source: ThetaSource,
    pub ticks: u32,
}

impl PairsMetadata {
    pub fn for_sweep(
        run_manifest: &RunManifest,
        draws: u32,
        parameter_columns: Vec<String>,
        summary_columns: Vec<String>,
        pairs_sha256: String,
    ) -> Result<Self, String> {
        if run_manifest.manifest_kind != ManifestKind::Sweep {
            return Err("pairs metadata requires a sweep run manifest".to_owned());
        }
        Ok(Self {
            component_versions: run_manifest.component_versions.clone(),
            determinism_level: run_manifest.determinism_level.clone(),
            draws,
            dt: run_manifest
                .dt
                .ok_or_else(|| "sweep run manifest has no dt".to_owned())?,
            ir_hash: run_manifest
                .ir_hash
                .clone()
                .ok_or_else(|| "sweep run manifest has no IR hash".to_owned())?,
            ir_hash_algorithm: run_manifest.ir_hash_algorithm.clone(),
            model: run_manifest
                .model
                .clone()
                .ok_or_else(|| "sweep run manifest has no model name".to_owned())?,
            noise_mode: run_manifest
                .noise_mode
                .ok_or_else(|| "sweep run manifest has no noise mode".to_owned())?,
            parameter_columns,
            pairs_hash_algorithm: HASH_ALGORITHM.to_owned(),
            pairs_sha256,
            schema_versions: BTreeMap::from([("pairs".to_owned(), PAIRS_SCHEMA_VERSION)]),
            seed: run_manifest.seed,
            summary_columns,
            theta_source: run_manifest
                .theta_source
                .clone()
                .ok_or_else(|| "sweep run manifest has no theta source".to_owned())?,
            ticks: run_manifest.ticks,
        })
    }
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gpu_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub driver_version: Option<String>,
}

impl BackendIdentity {
    pub fn cpu_oracle() -> Self {
        Self {
            backend: "cpu-oracle".to_owned(),
            precision: "f64".to_owned(),
            fell_back: false,
            gpu_model: None,
            driver_version: None,
        }
    }

    pub fn cuda_native_f64(gpu_model: String, driver_version: String) -> Self {
        Self {
            backend: "cuda-native-f64".to_owned(),
            precision: "f64".to_owned(),
            fell_back: false,
            gpu_model: Some(gpu_model),
            driver_version: Some(driver_version),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManifestExecution {
    pub k: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub seed: Option<u64>,
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub observation_sha256: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub noise_mode: Option<NoiseMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub observation_hash_algorithm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub observation_sha256: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub theta_source: Option<ThetaSource>,
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
        Self {
            backend_identity: Some(BackendIdentity::cpu_oracle()),
            component_versions: component_versions(),
            determinism_level: DETERMINISM_LEVEL.to_owned(),
            enabled_flags: Vec::new(),
            executions: Vec::new(),
            final_state_hash_algorithm: HASH_ALGORITHM.to_owned(),
            final_state_sha256: None,
            ir_hash: None,
            ir_hash_algorithm: HASH_ALGORITHM.to_owned(),
            manifest_kind: kind,
            noise_mode: None,
            model: None,
            observation_hash_algorithm: Some(HASH_ALGORITHM.to_owned()),
            observation_sha256: None,
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
            theta_source: None,
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

pub fn pairs_sidecar_path(output: &Path) -> PathBuf {
    PathBuf::from(format!("{}.meta.json", output.display()))
}

pub fn write_pairs_metadata(path: &Path, metadata: &PairsMetadata) -> Result<(), String> {
    if metadata.ir_hash_algorithm != HASH_ALGORITHM {
        return Err(format!(
            "unsupported ir_hash_algorithm '{}' (supported: '{HASH_ALGORITHM}')",
            metadata.ir_hash_algorithm
        ));
    }
    if metadata.pairs_hash_algorithm != HASH_ALGORITHM {
        return Err(format!(
            "unsupported pairs_hash_algorithm '{}' (supported: '{HASH_ALGORITHM}')",
            metadata.pairs_hash_algorithm
        ));
    }
    if metadata.theta_source.algorithm != HASH_ALGORITHM {
        return Err(format!(
            "unsupported theta_source.algorithm '{}' (supported: '{HASH_ALGORITHM}')",
            metadata.theta_source.algorithm
        ));
    }
    let bytes = serialize_canonical(metadata)?;
    std::fs::write(path, bytes.as_bytes()).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn write(path: &Path, manifest: &RunManifest) -> Result<(), String> {
    let value = serde_json::to_value(manifest).map_err(|error| error.to_string())?;
    validate_backend_identity_tuple(&value)?;
    validate_observation_tuple(manifest)?;
    validate_algorithms(manifest)?;
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
    validate_theta_source_tuple(&value)?;
    // Deserialize from the original bytes rather than through `Value`: the
    // latter can round an already-parsed f64 during `from_value`, which would
    // break exact replay of sampled sweep parameters.
    let manifest: RunManifest =
        serde_json::from_str(&source).map_err(|error| format!("{}: {error}", path.display()))?;
    validate_observation_tuple(&manifest)?;
    validate_algorithms(&manifest)?;
    Ok(manifest)
}

pub fn to_canonical_json(manifest: &RunManifest) -> Result<String, String> {
    let mut normalized = manifest.clone();
    normalized.enabled_flags.sort();
    normalized.enabled_flags.dedup();
    serialize_canonical(&normalized)
}

fn component_versions() -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "sembla-cli".to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
        ),
        ("sembla-ir".to_owned(), sembla_ir::VERSION.to_owned()),
        (
            "sembla-runtime".to_owned(),
            sembla_runtime::VERSION.to_owned(),
        ),
    ])
}

fn serialize_canonical(value: &impl Serialize) -> Result<String, String> {
    let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
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
    let backend = object.get("backend").and_then(serde_json::Value::as_str);
    let precision = object.get("precision").and_then(serde_json::Value::as_str);
    let fell_back = object.get("fell_back").and_then(serde_json::Value::as_bool);
    let gpu_model = object.get("gpu_model");
    let driver_version = object.get("driver_version");
    match backend {
        Some("cpu-oracle")
            if precision == Some("f64")
                && fell_back == Some(false)
                && gpu_model.is_none()
                && driver_version.is_none() =>
        {
            Ok(())
        }
        Some("cuda-native-f64")
            if precision == Some("f64")
                && fell_back == Some(false)
                && gpu_model
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| !value.is_empty())
                && driver_version
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| !value.is_empty()) =>
        {
            Ok(())
        }
        Some("cuda-native-f64") => Err(
            "cuda backend identity requires non-empty gpu_model and driver_version fields"
                .to_owned(),
        ),
        Some("cpu-oracle") => Err(
            "cpu backend identity must be f64, must not fall back, and must not contain GPU fields"
                .to_owned(),
        ),
        other => Err(format!("unsupported manifest backend identity {other:?}")),
    }
}

fn validate_theta_source_tuple(value: &serde_json::Value) -> Result<(), String> {
    let Some(source) = value.get("theta_source") else {
        return Ok(());
    };
    if source.is_null() {
        return Ok(());
    }
    let object = source.as_object().ok_or_else(|| {
        "theta_source tuple must be all-present or all-absent and must be an object".to_owned()
    })?;
    let missing = ["kind", "sha256", "algorithm"]
        .into_iter()
        .filter(|field| !object.contains_key(*field))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "theta_source tuple must be all-present or all-absent; missing {}",
            missing.join(", ")
        ));
    }
    Ok(())
}

fn validate_observation_tuple(manifest: &RunManifest) -> Result<(), String> {
    let algorithm_present = manifest.observation_hash_algorithm.is_some();
    let top_level_present = manifest.observation_sha256.is_some();
    let execution_count = manifest
        .executions
        .iter()
        .filter(|execution| execution.observation_sha256.is_some())
        .count();
    let all_execution_hashes = execution_count == manifest.executions.len();
    let shape_is_complete = match manifest.manifest_kind {
        ManifestKind::Run => top_level_present && execution_count == 0,
        ManifestKind::Sweep | ManifestKind::Compare => {
            !top_level_present && !manifest.executions.is_empty() && all_execution_hashes
        }
    };
    let shape_is_absent = !top_level_present && execution_count == 0;
    if (algorithm_present && !shape_is_complete) || (!algorithm_present && !shape_is_absent) {
        return Err(
            "observation hash tuple must be all-present or all-absent for the manifest kind"
                .to_owned(),
        );
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
    if let Some(value) = manifest.observation_hash_algorithm.as_deref() {
        if value != HASH_ALGORITHM {
            return Err(format!(
                "unsupported observation_hash_algorithm '{value}' (supported: '{HASH_ALGORITHM}')"
            ));
        }
    }
    if let Some(source) = &manifest.theta_source {
        if source.algorithm != HASH_ALGORITHM {
            return Err(format!(
                "unsupported theta_source.algorithm '{}' (supported: '{HASH_ALGORITHM}')",
                source.algorithm
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
