use std::time::{SystemTime, UNIX_EPOCH};

use super::SyntheticPopulation;

#[test]
fn generation_is_exact_and_seeded() {
    let first = SyntheticPopulation::generate(10_000, 200, 37, 9).unwrap();
    let second = SyntheticPopulation::generate(10_000, 200, 37, 9).unwrap();
    let different = SyntheticPopulation::generate(10_000, 200, 37, 10).unwrap();
    assert_eq!(first, second);
    assert_ne!(first, different);
    assert_eq!(first.health.iter().filter(|value| **value == 1).count(), 37);
    assert!(first.employer.iter().all(|value| (*value as usize) < 200));
}

#[test]
fn binary_round_trip_and_corruption_errors() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sembla-population-{}-{nonce}.bin",
        std::process::id()
    ));
    let population = SyntheticPopulation::generate(101, 7, 9, 88).unwrap();
    population.write(&path).unwrap();
    assert_eq!(SyntheticPopulation::read(&path).unwrap(), population);

    let mut bytes = std::fs::read(&path).unwrap();
    bytes.push(0);
    std::fs::write(&path, bytes).unwrap();
    assert!(SyntheticPopulation::read(&path)
        .unwrap_err()
        .to_string()
        .contains("payload length"));
    std::fs::remove_file(path).unwrap();
}
