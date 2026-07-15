//! Stable, real-valued workload used by every precision strategy in the spike.
//!
//! Rows are sorted by employer. Employer `g` owns the half-open interval in
//! [`Workload::group_offsets`] at `g..=g + 1`, so segmented kernels never need
//! to sort or follow an indirection before reducing.
//!
//! Random values use Philox4x32-10 with the same frozen packing used by Sembla,
//! implemented locally so this standalone crate has no `sembla-*` dependency:
//! the key is `(seed_lo, seed_hi)` and the counter is
//! `(tick, rule_id, entity_id, draw_idx)`. Static susceptibility weights reserve
//! `(tick=0, rule_id=0xffff_fe00, draw_idx=0)`. Infection racing clocks reserve
//! `rule_id=0` and `draw_idx=0` at the simulated tick. These namespaces must not
//! be repurposed by later precision-spike PRDs.

use std::{error::Error, fmt, ops::Range};

pub const DEFAULT_ROWS: u32 = 26_000_000;
pub const DEFAULT_GROUPS: u32 = 1_300_000;
pub const DEFAULT_SEED: u64 = 0x0123_4567_89ab_cdef;
pub const DEFAULT_BETA: f64 = 0.35;
pub const DEFAULT_DT: f64 = 0.25;

pub const SUSCEPTIBLE: u32 = 0;
pub const INFECTIOUS: u32 = 1;
pub const RECOVERED: u32 = 2;

/// Reserved Philox rule namespace for immutable susceptibility weights.
pub const WEIGHT_RULE_ID: u32 = 0xffff_fe00;
/// The infection rule whose racing time is used by the contested argmin.
pub const INFECTION_RULE_ID: u32 = 0;
pub const WEIGHT_DRAW_INDEX: u32 = 0;
pub const RACE_DRAW_INDEX: u32 = 0;

const PHILOX_M0: u32 = 0xd251_1f53;
const PHILOX_M1: u32 = 0xcd9e_8d57;
const PHILOX_W0: u32 = 0x9e37_79b9;
const PHILOX_W1: u32 = 0xbb67_ae85;
const PHILOX_ROUNDS: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkloadConfig {
    pub rows: u32,
    pub groups: u32,
    pub seed: u64,
    pub beta: f64,
    pub dt: f64,
}

impl Default for WorkloadConfig {
    fn default() -> Self {
        Self {
            rows: DEFAULT_ROWS,
            groups: DEFAULT_GROUPS,
            seed: DEFAULT_SEED,
            beta: DEFAULT_BETA,
            dt: DEFAULT_DT,
        }
    }
}

impl WorkloadConfig {
    #[must_use]
    pub fn with_size(rows: u32, groups: u32) -> Self {
        Self {
            rows,
            groups,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkloadError(String);

impl WorkloadError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for WorkloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for WorkloadError {}

/// Struct-of-arrays input for one SIR-shaped precision tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Workload {
    pub config: WorkloadConfig,
    /// Contiguous foreign key from person row to employer group.
    pub employer: Vec<u32>,
    /// SIR enum encoded as `S=0`, `I=1`, `R=2`.
    pub health: Vec<u32>,
    /// Real susceptibility weights in the open interval `(0, 1)`.
    pub weight: Vec<f64>,
    /// `groups + 1` boundaries into the person columns.
    pub group_offsets: Vec<u32>,
}

impl Workload {
    /// Builds a deterministic population with balanced, contiguous employers.
    ///
    /// Within each employer, approximately the first 20% of rows are
    /// infectious, approximately the final 10% are recovered, and the rest are
    /// susceptible. The default exact 20-row groups therefore contain 4 I,
    /// 14 S, and 2 R rows.
    pub fn generate(config: WorkloadConfig) -> Result<Self, WorkloadError> {
        validate_config(config)?;

        let rows = config.rows as usize;
        let groups = config.groups as usize;
        let mut employer = Vec::with_capacity(rows);
        let mut health = Vec::with_capacity(rows);
        let mut weight = Vec::with_capacity(rows);
        let mut group_offsets = Vec::with_capacity(groups + 1);

        for group in 0..=config.groups {
            group_offsets.push(ceil_ratio(group, config.rows, config.groups));
        }

        for group in 0..config.groups {
            let start = group_offsets[group as usize];
            let end = group_offsets[group as usize + 1];
            let group_size = end - start;
            for entity_id in start..end {
                let local = entity_id - start;
                let state = if u64::from(local) * 5 < u64::from(group_size) {
                    INFECTIOUS
                } else if u64::from(local) * 10 >= u64::from(group_size) * 9 {
                    RECOVERED
                } else {
                    SUSCEPTIBLE
                };
                employer.push(group);
                health.push(state);
                weight.push(uniform_f64(
                    config.seed,
                    0,
                    WEIGHT_RULE_ID,
                    entity_id,
                    WEIGHT_DRAW_INDEX,
                ));
            }
        }

        Ok(Self {
            config,
            employer,
            health,
            weight,
            group_offsets,
        })
    }

    #[must_use]
    pub fn group_range(&self, group: u32) -> Range<usize> {
        assert!(group < self.config.groups, "group index out of range");
        let start = self.group_offsets[group as usize] as usize;
        let end = self.group_offsets[group as usize + 1] as usize;
        start..end
    }

    #[must_use]
    pub fn group_size(&self, group: u32) -> u32 {
        self.group_offsets[group as usize + 1] - self.group_offsets[group as usize]
    }
}

fn validate_config(config: WorkloadConfig) -> Result<(), WorkloadError> {
    if config.rows == 0 {
        return Err(WorkloadError::new("workload must contain at least one row"));
    }
    if config.groups == 0 {
        return Err(WorkloadError::new(
            "workload must contain at least one employer group",
        ));
    }
    if config.groups > config.rows {
        return Err(WorkloadError::new(format!(
            "employer groups ({}) cannot exceed person rows ({})",
            config.groups, config.rows
        )));
    }
    if !config.beta.is_finite() || config.beta <= 0.0 {
        return Err(WorkloadError::new("beta must be finite and positive"));
    }
    if !config.dt.is_finite() || config.dt <= 0.0 {
        return Err(WorkloadError::new("dt must be finite and positive"));
    }
    Ok(())
}

/// Exactly 10% of consecutive entity IDs enter segmented conflict resolution.
///
/// Selected rows contend on their employer key; eligible rows outside this
/// selector are uncontested and can fire directly. This is the selector used by
/// the v0.1 throughput spike.
#[inline]
#[must_use]
pub const fn is_contested(entity_id: u32) -> bool {
    entity_id % 10 == 5
}

#[inline]
fn ceil_ratio(value: u32, numerator: u32, denominator: u32) -> u32 {
    let product = u64::from(value) * u64::from(numerator);
    product.div_ceil(u64::from(denominator)) as u32
}

#[inline]
fn philox_round(counter: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    let product_0 = u64::from(PHILOX_M0) * u64::from(counter[0]);
    let product_1 = u64::from(PHILOX_M1) * u64::from(counter[2]);
    [
        (product_1 >> 32) as u32 ^ counter[1] ^ key[0],
        product_1 as u32,
        (product_0 >> 32) as u32 ^ counter[3] ^ key[1],
        product_0 as u32,
    ]
}

/// Returns a Philox4x32-10 block for the frozen coordinate packing documented
/// at module level.
#[must_use]
pub fn draw_u32x4(seed: u64, tick: u32, rule_id: u32, entity_id: u32, draw_idx: u32) -> [u32; 4] {
    let mut counter = [tick, rule_id, entity_id, draw_idx];
    let mut key = [seed as u32, (seed >> 32) as u32];
    for round in 0..PHILOX_ROUNDS {
        counter = philox_round(counter, key);
        if round + 1 != PHILOX_ROUNDS {
            key[0] = key[0].wrapping_add(PHILOX_W0);
            key[1] = key[1].wrapping_add(PHILOX_W1);
        }
    }
    counter
}

/// Converts two Philox lanes to a deterministic binary64 sample in `(0, 1)`.
///
/// All 32 bits of lane 0 followed by the high 21 bits of lane 1 form a 53-bit
/// mantissa. The half-bin offset excludes zero; the sole rounding case at one
/// is clamped to the greatest representable value below one.
#[must_use]
pub fn uniform_f64(seed: u64, tick: u32, rule_id: u32, entity_id: u32, draw_idx: u32) -> f64 {
    let lanes = draw_u32x4(seed, tick, rule_id, entity_id, draw_idx);
    let mantissa = (u64::from(lanes[0]) << 21) | (u64::from(lanes[1]) >> 11);
    let sample = (mantissa as f64 + 0.5) * (1.0 / ((1_u64 << 53) as f64));
    if sample == 1.0 {
        f64::from_bits(1.0_f64.to_bits() - 1)
    } else {
        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn philox_matches_random123_known_answers() {
        assert_eq!(
            draw_u32x4(0, 0, 0, 0, 0),
            [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8]
        );
        assert_eq!(
            draw_u32x4(u64::MAX, u32::MAX, u32::MAX, u32::MAX, u32::MAX),
            [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd]
        );
    }

    #[test]
    fn ten_thousand_rows_have_balanced_contiguous_groups() {
        let workload = Workload::generate(WorkloadConfig::with_size(10_000, 500)).unwrap();
        assert_eq!(workload.employer.len(), 10_000);
        assert_eq!(workload.health.len(), 10_000);
        assert_eq!(workload.weight.len(), 10_000);
        assert_eq!(workload.group_offsets.len(), 501);

        for group in 0..500 {
            let range = workload.group_range(group);
            assert_eq!(range.len(), 20);
            assert!(workload.employer[range.clone()]
                .iter()
                .all(|employer| *employer == group));
        }
        assert!(workload.employer.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(workload
            .weight
            .iter()
            .all(|weight| *weight > 0.0 && *weight < 1.0));
        assert!(workload.health.iter().all(|health| *health <= RECOVERED));
    }
}
