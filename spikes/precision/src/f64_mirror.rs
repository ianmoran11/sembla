//! Scalar Rust mirror of the native-f64 WGSL and CUDA kernel arithmetic.
//!
//! Unlike the oracle's single ascending accumulation, the native kernels form
//! two ascending half-group partials and merge partial 0 before partial 1. This
//! mirror makes that reduction tree and any resulting oracle residual explicit.

use crate::{
    oracle::NO_WINNER,
    workload::{
        is_contested, uniform_f64, Workload, INFECTION_RULE_ID, INFECTIOUS, RACE_DRAW_INDEX,
        SUSCEPTIBLE,
    },
};

#[derive(Clone, Debug, PartialEq)]
pub struct F64MirrorResult {
    pub segmented_sums: Vec<f64>,
    pub winner_entity_ids: Vec<u32>,
    pub fired_flags: Vec<u32>,
}

/// Native kernel reduction order: two ascending halves, then partial 0 + 1.
#[must_use]
pub fn reduce_two_pass(values: &[f64]) -> f64 {
    let midpoint = values.len() / 2;
    let first = values[..midpoint].iter().copied().sum::<f64>();
    let second = values[midpoint..].iter().copied().sum::<f64>();
    first + second
}

/// Native map arithmetic for one susceptible entity.
#[must_use]
pub fn race_time(beta: f64, segmented_sum: f64, group_size: u32, uniform: f64) -> f64 {
    let lambda = beta * segmented_sum / f64::from(group_size);
    if lambda > 0.0 {
        -(1.0 - uniform).ln() / lambda
    } else {
        f64::INFINITY
    }
}

/// Runs the exact scalar arithmetic and fixed reduction tree used by both
/// native backends. Positive finite race times are ordered lexicographically by
/// `(t_bits, rule_id, entity_id)`.
#[must_use]
pub fn run_f64_mirror(workload: &Workload, tick: u32) -> F64MirrorResult {
    let mut segmented_sums = vec![0.0; workload.config.groups as usize];
    for group in 0..workload.config.groups {
        let range = workload.group_range(group);
        let midpoint = range.start + range.len() / 2;
        let mut first = 0.0;
        for entity in range.start..midpoint {
            if workload.health[entity] == INFECTIOUS {
                first += workload.weight[entity];
            }
        }
        let mut second = 0.0;
        for entity in midpoint..range.end {
            if workload.health[entity] == INFECTIOUS {
                second += workload.weight[entity];
            }
        }
        segmented_sums[group as usize] = first + second;
    }

    let mut race_times = vec![f64::INFINITY; workload.config.rows as usize];
    let mut fired_flags = vec![0_u32; workload.config.rows as usize];
    for entity in 0..workload.config.rows {
        let index = entity as usize;
        if workload.health[index] != SUSCEPTIBLE {
            continue;
        }
        let group = workload.employer[index];
        let uniform = uniform_f64(
            workload.config.seed,
            tick,
            INFECTION_RULE_ID,
            entity,
            RACE_DRAW_INDEX,
        );
        let time = race_time(
            workload.config.beta,
            segmented_sums[group as usize],
            workload.group_size(group),
            uniform,
        );
        race_times[index] = time;
        if time < workload.config.dt && !is_contested(entity) {
            fired_flags[index] = 1;
        }
    }

    let mut winner_entity_ids = vec![NO_WINNER; workload.config.groups as usize];
    let mut winner_bits = vec![u64::MAX; workload.config.groups as usize];
    for entity in 0..workload.config.rows {
        if !is_contested(entity) {
            continue;
        }
        let time = race_times[entity as usize];
        if time >= workload.config.dt {
            continue;
        }
        let group = workload.employer[entity as usize] as usize;
        let bits = time.to_bits();
        let current = (
            winner_bits[group],
            INFECTION_RULE_ID,
            winner_entity_ids[group],
        );
        let candidate = (bits, INFECTION_RULE_ID, entity);
        if candidate < current {
            winner_bits[group] = bits;
            winner_entity_ids[group] = entity;
        }
    }
    for winner in winner_entity_ids.iter().copied() {
        if winner != NO_WINNER {
            fired_flags[winner as usize] = 1;
        }
    }

    F64MirrorResult {
        segmented_sums,
        winner_entity_ids,
        fired_flags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{oracle::run_oracle, workload::WorkloadConfig};

    #[test]
    fn fixed_two_pass_reduction_is_explicit() {
        let values = [1.0e16, 1.0, -1.0e16, 1.0];
        assert_eq!(reduce_two_pass(&values), 0.0);
        assert_eq!(values.into_iter().sum::<f64>(), 1.0);
    }

    #[test]
    fn map_arithmetic_handles_zero_hazard_and_open_uniforms() {
        assert!(race_time(0.35, 0.0, 20, 0.5).is_infinite());
        let time = race_time(0.35, 2.0, 20, 0.5);
        assert!(time.is_finite() && time > 0.0);
        assert_eq!(time, -(1.0_f64 - 0.5).ln() / (0.35 * 2.0 / 20.0));
    }

    #[test]
    fn full_tick_mirror_matches_oracle_on_local_workload() {
        let workload = Workload::generate(WorkloadConfig::with_size(10_000, 500)).unwrap();
        let mirror = run_f64_mirror(&workload, 7);
        let oracle = run_oracle(&workload, 7);
        assert_eq!(mirror.segmented_sums, oracle.segmented_sums);
        assert_eq!(mirror.winner_entity_ids, oracle.winner_entity_ids);
        assert_eq!(mirror.fired_flags, oracle.fired_flags);
    }
}
