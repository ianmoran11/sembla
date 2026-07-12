//! Deterministic synthetic SIR populations and their portable binary format.
//!
//! Generation reserves rule IDs `0xffff_ff00` (employer assignment) and
//! `0xffff_ff01` (initial-infection shuffle) in the PRD 0003 Philox coordinate
//! space. These IDs are outside validated model rule IDs used by v0.1 models.

use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use crate::rng::uniform_f64;
use crate::state::{ColumnData, ColumnInit, TableInit};

const MAGIC: &[u8; 12] = b"SEMBLA_POP\0\0";
const VERSION: u32 = 1;
/// Reserved coordinate namespace for deterministic workplace assignment.
pub const SYNTH_EMPLOYER_RULE_ID: u32 = 0xffff_ff00;
/// Reserved coordinate namespace for deterministic initial-infection shuffling.
pub const SYNTH_INFECTION_RULE_ID: u32 = 0xffff_ff01;

/// A generated SIR population. Health indices are `S=0`, `I=1`, `R=2`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntheticPopulation {
    pub health: Vec<u16>,
    pub employer: Vec<u32>,
    pub employer_count: usize,
}

/// Population generation or binary-format failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationError(String);

impl PopulationError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for PopulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for PopulationError {}

impl SyntheticPopulation {
    /// Generates a population using only coordinate-keyed PRD 0003 draws.
    ///
    /// Employer ranks use `floor(E * U²)`. This deterministic power-law-ish
    /// bucketing produces many small workplaces and progressively fewer large
    /// ones without introducing mutable RNG state. Initial infections are a
    /// prefix of a deterministic Fisher-Yates permutation, so exactly `I0`
    /// distinct people are infected.
    pub fn generate(
        persons: usize,
        employers: usize,
        initial_infected: usize,
        seed: u64,
    ) -> Result<Self, PopulationError> {
        if employers == 0 {
            return Err(PopulationError::new(
                "employer count must be greater than zero",
            ));
        }
        if employers > u32::MAX as usize {
            return Err(PopulationError::new("employer count exceeds u32 capacity"));
        }
        if persons > u32::MAX as usize {
            return Err(PopulationError::new(
                "person count exceeds Philox entity-id capacity",
            ));
        }
        if initial_infected > persons {
            return Err(PopulationError::new(format!(
                "initial infected count {initial_infected} exceeds person count {persons}"
            )));
        }

        let mut employer = Vec::with_capacity(persons);
        for person in 0..persons {
            let entity_id = person as u32;
            let uniform = uniform_f64(seed, 0, SYNTH_EMPLOYER_RULE_ID, entity_id, 0);
            let rank = ((uniform * uniform) * employers as f64) as usize;
            employer.push(u32::try_from(rank.min(employers - 1)).expect("bounded employer rank"));
        }

        let mut permutation: Vec<u32> = (0..persons as u32).collect();
        for upper in (1..persons).rev() {
            let uniform = uniform_f64(seed, 0, SYNTH_INFECTION_RULE_ID, upper as u32, 0);
            let selected = (uniform * (upper + 1) as f64) as usize;
            permutation.swap(upper, selected.min(upper));
        }
        let mut health = vec![0_u16; persons];
        for person in permutation.into_iter().take(initial_infected) {
            health[person as usize] = 1;
        }

        Ok(Self {
            health,
            employer,
            employer_count: employers,
        })
    }

    /// Writes versioned little-endian `SEMBLA_POP` data atomically enough for
    /// normal CLI use (one complete in-memory byte vector, then one write).
    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), PopulationError> {
        if self.health.len() != self.employer.len() {
            return Err(PopulationError::new(
                "population health and employer columns have different lengths",
            ));
        }
        let mut bytes = Vec::with_capacity(32 + self.health.len() * 6);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&(self.health.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.employer_count as u64).to_le_bytes());
        for value in &self.health {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in &self.employer {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        fs::write(path.as_ref(), bytes)
            .map_err(|error| PopulationError::new(format!("{}: {error}", path.as_ref().display())))
    }

    /// Reads and validates version 1 `SEMBLA_POP` data.
    pub fn read(path: impl AsRef<Path>) -> Result<Self, PopulationError> {
        let bytes = fs::read(path.as_ref()).map_err(|error| {
            PopulationError::new(format!("{}: {error}", path.as_ref().display()))
        })?;
        Self::decode(&bytes)
            .map_err(|error| PopulationError::new(format!("{}: {error}", path.as_ref().display())))
    }

    fn decode(bytes: &[u8]) -> Result<Self, PopulationError> {
        if bytes.len() < 32 || &bytes[..12] != MAGIC {
            return Err(PopulationError::new(
                "invalid SEMBLA_POP magic or truncated header",
            ));
        }
        let version = u32::from_le_bytes(bytes[12..16].try_into().expect("fixed slice"));
        if version != VERSION {
            return Err(PopulationError::new(format!(
                "unsupported SEMBLA_POP version {version}"
            )));
        }
        let persons_u64 = u64::from_le_bytes(bytes[16..24].try_into().expect("fixed slice"));
        let employers_u64 = u64::from_le_bytes(bytes[24..32].try_into().expect("fixed slice"));
        let persons = usize::try_from(persons_u64)
            .map_err(|_| PopulationError::new("person count exceeds platform capacity"))?;
        let employer_count = usize::try_from(employers_u64)
            .map_err(|_| PopulationError::new("employer count exceeds platform capacity"))?;
        if employer_count == 0 || employer_count > u32::MAX as usize {
            return Err(PopulationError::new(
                "invalid employer count in population file",
            ));
        }
        let payload = persons
            .checked_mul(6)
            .and_then(|size| size.checked_add(32))
            .ok_or_else(|| PopulationError::new("population payload length overflow"))?;
        if bytes.len() != payload {
            return Err(PopulationError::new(format!(
                "population payload length is {}, expected {payload}",
                bytes.len()
            )));
        }
        let mut health = Vec::with_capacity(persons);
        let health_start = 32;
        for row in 0..persons {
            let offset = health_start + row * 2;
            let value =
                u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("fixed slice"));
            if value > 2 {
                return Err(PopulationError::new(format!(
                    "health[{row}] has invalid enum index {value}"
                )));
            }
            health.push(value);
        }
        let employer_start = health_start + persons * 2;
        let mut employer = Vec::with_capacity(persons);
        for row in 0..persons {
            let offset = employer_start + row * 4;
            let value =
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed slice"));
            if value as usize >= employer_count {
                return Err(PopulationError::new(format!(
                    "employer[{row}] index {value} is outside {employer_count} employers"
                )));
            }
            employer.push(value);
        }
        Ok(Self {
            health,
            employer,
            employer_count,
        })
    }

    /// Converts this population to SIR table initializers in `box_name`.
    ///
    /// Both the standalone PRD 0008 model (`sir`) and the composed PRD 0009
    /// model (`population`) use the same local `person`/`employer` schema.
    pub fn sir_table_initializers_for_box(&self, box_name: &str) -> Vec<TableInit> {
        vec![
            TableInit::new(
                box_name,
                "person",
                self.health.len(),
                vec![
                    ColumnInit::new("health", ColumnData::Enum(self.health.clone())),
                    ColumnInit::new("employer", ColumnData::Ref(self.employer.clone())),
                ],
            ),
            TableInit::new(box_name, "employer", self.employer_count, vec![]),
        ]
    }

    /// Converts this population to the standalone checked-in SIR model.
    pub fn sir_table_initializers(&self) -> Vec<TableInit> {
        self.sir_table_initializers_for_box("sir")
    }

    /// Converts this population to the checked-in SIR + policy model.
    ///
    /// The policy controller is the one non-population row in that model and
    /// starts in `Open` (enum index 0) with its contact modifier at `1.0`.
    pub fn sir_policy_table_initializers(&self) -> Vec<TableInit> {
        let mut initial = self.sir_table_initializers_for_box("population");
        initial.push(TableInit::new(
            "policy",
            "controller",
            1,
            vec![
                ColumnInit::new("mode", ColumnData::Enum(vec![0])),
                ColumnInit::new("modifier", ColumnData::Real(vec![1.0])),
            ],
        ));
        initial
    }
}

#[cfg(test)]
mod tests {
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
}
