//! Scalar CPU `f64` ground truth for the precision spike.
//!
//! The reference reduction visits each employer independently and adds
//! infectious weights in ascending entity-id order. That order is part of the
//! oracle contract: it defines one result rather than relying on the accidental
//! iteration order of a parallel reduction. The diagnostic repeats each group
//! in descending entity-id order and compares the resulting IEEE-754 bits.

use crate::workload::{
    is_contested, uniform_f64, Workload, INFECTION_RULE_ID, RACE_DRAW_INDEX, SUSCEPTIBLE,
};

/// Sentinel used when a contested employer key has no eligible candidate.
pub const NO_WINNER: u32 = u32::MAX;

#[derive(Clone, Debug, PartialEq)]
pub struct OracleResult {
    /// Infectious susceptibility sums in ascending-entity reduction order.
    pub segmented_sums: Vec<f64>,
    /// The same sums accumulated in descending entity-id order.
    pub reversed_segmented_sums: Vec<f64>,
    /// One byte per group: 1 when forward/reverse sum bits differ, else 0.
    pub order_sensitive_flags: Vec<u8>,
    pub order_sensitive_group_count: usize,
    /// Winner entity per employer key, or [`NO_WINNER`].
    pub winner_entity_ids: Vec<u32>,
    /// One `u32` flag per person; contested losers remain zero.
    pub fired_flags: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ArgminKey {
    time_bits: u64,
    rule_id: u32,
    entity_id: u32,
}

/// Runs one tick of segmented reduce, hazard/race map, and segmented argmin.
///
/// All generated race times are non-negative finite values or infinity, so raw
/// IEEE-754 `time_bits` have the same ordering as the numeric values. The exact
/// conflict key is therefore `(t_bits, rule_id, entity_id)`, matching
/// `DESIGN.md` §5.1 while making bit-level ties explicit.
#[must_use]
pub fn run_oracle(workload: &Workload, tick: u32) -> OracleResult {
    let groups = workload.config.groups as usize;
    let rows = workload.config.rows as usize;

    // Stage 1: fixed-order segmented reduction and its reverse-order diagnostic.
    let mut segmented_sums = vec![0.0_f64; groups];
    let mut reversed_segmented_sums = vec![0.0_f64; groups];
    for group in 0..workload.config.groups {
        let range = workload.group_range(group);
        let mut forward = 0.0;
        for entity in range.clone() {
            if workload.health[entity] == crate::workload::INFECTIOUS {
                forward += workload.weight[entity];
            }
        }
        let mut reversed = 0.0;
        for entity in range.rev() {
            if workload.health[entity] == crate::workload::INFECTIOUS {
                reversed += workload.weight[entity];
            }
        }
        segmented_sums[group as usize] = forward;
        reversed_segmented_sums[group as usize] = reversed;
    }
    let order_sensitive_flags: Vec<u8> = segmented_sums
        .iter()
        .zip(&reversed_segmented_sums)
        .map(|(forward, reversed)| u8::from(forward.to_bits() != reversed.to_bits()))
        .collect();
    let order_sensitive_group_count = order_sensitive_flags
        .iter()
        .filter(|flag| **flag != 0)
        .count();

    // Stage 2: scalar map from each susceptible row to its exponential race.
    // Infinity denotes a row that cannot fire (including non-susceptible rows).
    let mut race_times = vec![f64::INFINITY; rows];
    let mut fired_flags = vec![0_u32; rows];
    for entity in 0..workload.config.rows {
        let index = entity as usize;
        if workload.health[index] != SUSCEPTIBLE {
            continue;
        }
        let group = workload.employer[index];
        let group_size = f64::from(workload.group_size(group));
        let lambda = workload.config.beta * segmented_sums[group as usize] / group_size;
        let uniform = uniform_f64(
            workload.config.seed,
            tick,
            INFECTION_RULE_ID,
            entity,
            RACE_DRAW_INDEX,
        );
        let time = if lambda > 0.0 {
            -(1.0 - uniform).ln() / lambda
        } else {
            f64::INFINITY
        };
        race_times[index] = time;
        if time < workload.config.dt && !is_contested(entity) {
            fired_flags[index] = 1;
        }
    }

    // Stage 3: one lexicographic argmin per contested employer key.
    let mut best_keys: Vec<Option<ArgminKey>> = vec![None; groups];
    let mut winner_entity_ids = vec![NO_WINNER; groups];
    for entity in 0..workload.config.rows {
        let index = entity as usize;
        let time = race_times[index];
        if !is_contested(entity) || time >= workload.config.dt {
            continue;
        }
        let group = workload.employer[index] as usize;
        let key = ArgminKey {
            time_bits: time.to_bits(),
            rule_id: INFECTION_RULE_ID,
            entity_id: entity,
        };
        if best_keys[group].map_or(true, |best| key < best) {
            best_keys[group] = Some(key);
            winner_entity_ids[group] = entity;
        }
    }
    for winner in winner_entity_ids.iter().copied() {
        if winner != NO_WINNER {
            fired_flags[winner as usize] = 1;
        }
    }

    OracleResult {
        segmented_sums,
        reversed_segmented_sums,
        order_sensitive_flags,
        order_sensitive_group_count,
        winner_entity_ids,
        fired_flags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workload::WorkloadConfig;

    fn f64_bytes(values: &[f64]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_bits().to_le_bytes())
            .collect()
    }

    fn u32_bytes(values: &[u32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    #[test]
    fn oracle_is_byte_deterministic_at_fixed_seed() {
        let workload = Workload::generate(WorkloadConfig::with_size(10_000, 500)).unwrap();
        let first = run_oracle(&workload, 7);
        let second = run_oracle(&workload, 7);

        assert_eq!(
            f64_bytes(&first.segmented_sums),
            f64_bytes(&second.segmented_sums)
        );
        assert_eq!(
            u32_bytes(&first.winner_entity_ids),
            u32_bytes(&second.winner_entity_ids)
        );
        assert_eq!(
            u32_bytes(&first.fired_flags),
            u32_bytes(&second.fired_flags)
        );
    }

    #[test]
    fn reversed_order_diagnostic_is_well_defined() {
        let workload = Workload::generate(WorkloadConfig::with_size(10_000, 500)).unwrap();
        let result = run_oracle(&workload, 0);
        let counted = result
            .order_sensitive_flags
            .iter()
            .filter(|flag| **flag != 0)
            .count();

        assert_eq!(result.reversed_segmented_sums.len(), 500);
        assert_eq!(result.order_sensitive_flags.len(), 500);
        assert_eq!(result.order_sensitive_group_count, counted);
        println!(
            "order-sensitive groups at 10k/500 scale: {}",
            result.order_sensitive_group_count
        );
        assert!(result.order_sensitive_group_count <= 500);
        assert!(result
            .reversed_segmented_sums
            .iter()
            .all(|sum| sum.is_finite()));
    }

    #[test]
    fn exact_lexicographic_key_breaks_bitwise_ties() {
        let early_rule = ArgminKey {
            time_bits: 1.25_f64.to_bits(),
            rule_id: 2,
            entity_id: 99,
        };
        let late_rule = ArgminKey {
            rule_id: 3,
            entity_id: 1,
            ..early_rule
        };
        let early_entity = ArgminKey {
            entity_id: 98,
            ..early_rule
        };
        assert!(early_rule < late_rule);
        assert!(early_entity < early_rule);
    }
}
