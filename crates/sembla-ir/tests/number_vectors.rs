use std::path::{Path, PathBuf};

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
fn serde_f64_spelling_matches_the_shared_number_vectors() {
    let source =
        std::fs::read_to_string(repository_path("fixtures/hash/number-vectors.json")).unwrap();
    let vectors: serde_json::Value = serde_json::from_str(&source).unwrap();
    let vectors = vectors.as_array().unwrap();
    assert!(!vectors.is_empty());

    for vector in vectors {
        let label = vector["source"].as_str().unwrap();
        let value = vector["value"].as_f64().unwrap();
        let expected = vector["expected"].as_str().unwrap();
        assert!(value.is_finite(), "{label}");
        assert_eq!(serde_json::to_string(&value).unwrap(), expected, "{label}");
    }
}
