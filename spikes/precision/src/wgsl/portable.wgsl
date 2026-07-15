// Portable f32 and double-single hot paths for the precision spike.
//
// Reduction is deterministic and atomics-free in both variants. Pass 1 emits
// two fixed, ascending-row partials per employer; pass 2 merges partial 0 then
// partial 1. The map is row-parallel. Argmin is one invocation per employer and
// scans selected rows in ascending entity order, applying the full precision-
// specific time key followed by rule_id and entity_id.

struct Config {
  rows: u32,
  groups: u32,
  tick: u32,
  map_workgroups_x: u32,
  seed_lo: u32,
  seed_hi: u32,
  partials_per_group: u32,
  _pad0: u32,
  beta: vec2<f32>,
  dt: vec2<f32>,
}

@group(0) @binding(0) var<uniform> config: Config;
@group(0) @binding(1) var<storage, read> group_offsets: array<u32>;
@group(0) @binding(2) var<storage, read> employers: array<u32>;
@group(0) @binding(3) var<storage, read> health: array<u32>;
@group(0) @binding(4) var<storage, read> weights: array<vec2<f32>>;
@group(0) @binding(5) var<storage, read_write> partial_sums: array<vec2<f32>>;
@group(0) @binding(6) var<storage, read_write> segmented_sums: array<vec2<f32>>;
@group(0) @binding(7) var<storage, read_write> race_times: array<vec2<f32>>;
@group(0) @binding(8) var<storage, read_write> winner_entities: array<u32>;
@group(0) @binding(9) var<storage, read_write> fired_flags: array<u32>;

const INFECTIOUS: u32 = 1u;
const SUSCEPTIBLE: u32 = 0u;
const INFECTION_RULE_ID: u32 = 0u;
const NO_WINNER: u32 = 0xffffffffu;
fn positive_infinity() -> f32 {
  return bitcast<f32>(0x7f800000u);
}

fn mul_hi(a: u32, b: u32) -> u32 {
  let a0 = a & 0xffffu;
  let a1 = a >> 16u;
  let b0 = b & 0xffffu;
  let b1 = b >> 16u;
  let w0 = a0 * b0;
  let t = a1 * b0 + (w0 >> 16u);
  let w1 = a0 * b1 + (t & 0xffffu);
  return a1 * b1 + (t >> 16u) + (w1 >> 16u);
}

fn philox4x32_10(
  seed_lo: u32,
  seed_hi: u32,
  tick: u32,
  rule_id: u32,
  entity_id: u32,
  draw_idx: u32,
) -> vec4<u32> {
  var counter = vec4<u32>(tick, rule_id, entity_id, draw_idx);
  var key = vec2<u32>(seed_lo, seed_hi);
  for (var round = 0u; round < 10u; round = round + 1u) {
    let low0 = 0xd2511f53u * counter.x;
    let high0 = mul_hi(0xd2511f53u, counter.x);
    let low1 = 0xcd9e8d57u * counter.z;
    let high1 = mul_hi(0xcd9e8d57u, counter.z);
    counter = vec4<u32>(
      high1 ^ counter.y ^ key.x,
      low1,
      high0 ^ counter.w ^ key.y,
      low0,
    );
    if (round != 9u) {
      key = key + vec2<u32>(0x9e3779b9u, 0xbb67ae85u);
    }
  }
  return counter;
}

// The f32 baseline follows the v0.1 spike: high 24 bits plus a half-bin offset.
fn uniform_open_f32(words: vec4<u32>) -> f32 {
  return (f32(words.x >> 8u) + 0.5) * 0x1p-24;
}

// The df64 path consumes the same 53 Philox bits as the CPU f64 oracle. Splitting
// word 0 into two 16-bit pieces avoids losing low bits in a u32->f32 conversion.
fn uniform_open_df64(words: vec4<u32>) -> df64 {
  let word0_high = df_from_f32(f32(words.x >> 16u) * 0x1p-16);
  let word0_low = df_from_f32(f32(words.x & 0xffffu) * 0x1p-32);
  let word1_high = df_from_f32(f32(words.y >> 11u) * 0x1p-53);
  let half_bin = df_from_f32(0x1p-54);
  return df_add(df_add(word0_high, word0_low), df_add(word1_high, half_bin));
}

fn is_contested(entity_id: u32) -> bool {
  return entity_id % 10u == 5u;
}

fn map_row_index(gid: vec3<u32>) -> u32 {
  return gid.x + gid.y * config.map_workgroups_x * 256u;
}

fn partial_bounds(partial_index: u32) -> vec2<u32> {
  let group = partial_index / config.partials_per_group;
  let part = partial_index % config.partials_per_group;
  let start = group_offsets[group];
  let end = group_offsets[group + 1u];
  let midpoint = start + (end - start) / 2u;
  if (part == 0u) {
    return vec2<u32>(start, midpoint);
  }
  return vec2<u32>(midpoint, end);
}

@compute @workgroup_size(64)
fn reduce_partial_f32(@builtin(global_invocation_id) gid: vec3<u32>) {
  let partial_index = gid.x;
  if (partial_index >= config.groups * config.partials_per_group) {
    return;
  }
  let bounds = partial_bounds(partial_index);
  var sum = 0.0;
  for (var row = bounds.x; row < bounds.y; row = row + 1u) {
    if (health[row] == INFECTIOUS) {
      sum = sum + weights[row].x;
    }
  }
  partial_sums[partial_index] = vec2<f32>(sum, 0.0);
}

@compute @workgroup_size(64)
fn reduce_finish_f32(@builtin(global_invocation_id) gid: vec3<u32>) {
  let group = gid.x;
  if (group >= config.groups) {
    return;
  }
  let first = partial_sums[group * config.partials_per_group].x;
  let second = partial_sums[group * config.partials_per_group + 1u].x;
  segmented_sums[group] = vec2<f32>(first + second, 0.0);
}

@compute @workgroup_size(64)
fn reduce_partial_df64(@builtin(global_invocation_id) gid: vec3<u32>) {
  let partial_index = gid.x;
  if (partial_index >= config.groups * config.partials_per_group) {
    return;
  }
  let bounds = partial_bounds(partial_index);
  var sum = df_from_f32(0.0);
  for (var row = bounds.x; row < bounds.y; row = row + 1u) {
    if (health[row] == INFECTIOUS) {
      sum = df_add(sum, weights[row]);
    }
  }
  partial_sums[partial_index] = sum;
}

@compute @workgroup_size(64)
fn reduce_finish_df64(@builtin(global_invocation_id) gid: vec3<u32>) {
  let group = gid.x;
  if (group >= config.groups) {
    return;
  }
  let first = partial_sums[group * config.partials_per_group];
  let second = partial_sums[group * config.partials_per_group + 1u];
  segmented_sums[group] = df_add(first, second);
}

@compute @workgroup_size(256)
fn map_f32(@builtin(global_invocation_id) gid: vec3<u32>) {
  let row = map_row_index(gid);
  if (row >= config.rows) {
    return;
  }

  fired_flags[row] = 0u;
  race_times[row] = vec2<f32>(positive_infinity(), 0.0);
  if (health[row] != SUSCEPTIBLE) {
    return;
  }

  let group = employers[row];
  let group_size = f32(group_offsets[group + 1u] - group_offsets[group]);
  let lambda = config.beta.x * segmented_sums[group].x / group_size;
  if (lambda <= 0.0) {
    return;
  }
  let words = philox4x32_10(config.seed_lo, config.seed_hi, config.tick, INFECTION_RULE_ID, row, 0u);
  let uniform = uniform_open_f32(words);
  let time = -log(1.0 - uniform) / lambda;
  race_times[row] = vec2<f32>(time, 0.0);
  if (time < config.dt.x && !is_contested(row)) {
    fired_flags[row] = 1u;
  }
}

@compute @workgroup_size(256)
fn map_df64(@builtin(global_invocation_id) gid: vec3<u32>) {
  let row = map_row_index(gid);
  if (row >= config.rows) {
    return;
  }

  fired_flags[row] = 0u;
  race_times[row] = vec2<f32>(positive_infinity(), 0.0);
  if (health[row] != SUSCEPTIBLE) {
    return;
  }

  let group = employers[row];
  let group_size = df_from_f32(f32(group_offsets[group + 1u] - group_offsets[group]));
  let lambda = df_div(df_mul(config.beta, segmented_sums[group]), group_size);
  if (!df_less(df_from_f32(0.0), lambda)) {
    return;
  }
  let words = philox4x32_10(config.seed_lo, config.seed_hi, config.tick, INFECTION_RULE_ID, row, 0u);
  let uniform = uniform_open_df64(words);
  let one_minus_uniform = df_add(df_from_f32(1.0), df_neg(uniform));
  let negative_log = df_neg(df_log(one_minus_uniform));
  let time = df_div(negative_log, lambda);
  race_times[row] = time;
  if (df_less(time, config.dt) && !is_contested(row)) {
    fired_flags[row] = 1u;
  }
}

fn f32_key_less(
  time: f32,
  rule_id: u32,
  entity: u32,
  best_time: f32,
  best_rule_id: u32,
  best_entity: u32,
) -> bool {
  let time_bits = bitcast<u32>(time);
  let best_bits = bitcast<u32>(best_time);
  return time_bits < best_bits ||
    (time_bits == best_bits &&
      (rule_id < best_rule_id ||
        (rule_id == best_rule_id && entity < best_entity)));
}

fn df64_key_less(
  time: df64,
  rule_id: u32,
  entity: u32,
  best_time: df64,
  best_rule_id: u32,
  best_entity: u32,
) -> bool {
  return df_less(time, best_time) ||
    (df_equal(time, best_time) &&
      (rule_id < best_rule_id ||
        (rule_id == best_rule_id && entity < best_entity)));
}

@compute @workgroup_size(64)
fn argmin_f32(@builtin(global_invocation_id) gid: vec3<u32>) {
  let group = gid.x;
  if (group >= config.groups) {
    return;
  }
  var best_entity = NO_WINNER;
  var best_rule_id = NO_WINNER;
  var best_time = positive_infinity();
  for (var row = group_offsets[group]; row < group_offsets[group + 1u]; row = row + 1u) {
    let candidate_time = race_times[row].x;
    if (is_contested(row) && candidate_time < config.dt.x &&
        f32_key_less(
          candidate_time,
          INFECTION_RULE_ID,
          row,
          best_time,
          best_rule_id,
          best_entity,
        )) {
      best_time = candidate_time;
      best_rule_id = INFECTION_RULE_ID;
      best_entity = row;
    }
  }
  winner_entities[group] = best_entity;
  if (best_entity != NO_WINNER) {
    fired_flags[best_entity] = 1u;
  }
}

@compute @workgroup_size(64)
fn argmin_df64(@builtin(global_invocation_id) gid: vec3<u32>) {
  let group = gid.x;
  if (group >= config.groups) {
    return;
  }
  var best_entity = NO_WINNER;
  var best_rule_id = NO_WINNER;
  var best_time = vec2<f32>(positive_infinity(), 0.0);
  for (var row = group_offsets[group]; row < group_offsets[group + 1u]; row = row + 1u) {
    let candidate_time = race_times[row];
    if (is_contested(row) && df_less(candidate_time, config.dt) &&
        df64_key_less(
          candidate_time,
          INFECTION_RULE_ID,
          row,
          best_time,
          best_rule_id,
          best_entity,
        )) {
      best_time = candidate_time;
      best_rule_id = INFECTION_RULE_ID;
      best_entity = row;
    }
  }
  winner_entities[group] = best_entity;
  if (best_entity != NO_WINNER) {
    fired_flags[best_entity] = 1u;
  }
}

struct KatInput {
  seed_lo: u32,
  seed_hi: u32,
  tick: u32,
  rule_id: u32,
  entity_id: u32,
  draw_idx: u32,
}

@group(0) @binding(10) var<storage, read> kat_inputs: array<KatInput>;
@group(0) @binding(11) var<storage, read_write> kat_outputs: array<vec4<u32>>;

@compute @workgroup_size(64)
fn philox_known_answers(@builtin(global_invocation_id) gid: vec3<u32>) {
  let index = gid.x;
  if (index >= arrayLength(&kat_inputs)) {
    return;
  }
  let input = kat_inputs[index];
  kat_outputs[index] = philox4x32_10(
    input.seed_lo,
    input.seed_hi,
    input.tick,
    input.rule_id,
    input.entity_id,
    input.draw_idx,
  );
}

@group(0) @binding(12) var<storage, read> arithmetic_probe_input: array<f32>;
@group(0) @binding(13) var<storage, read_write> arithmetic_probe_output: array<vec2<f32>>;

// Empirical Level-B probe. A contracted multiply-add yields 1 instead of 0;
// reassociation also yields 1 instead of 0. The other outputs ensure two-sum
// and two-prod residuals survive compilation.
@compute @workgroup_size(1)
fn arithmetic_behavior_probe(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x != 0u) {
    return;
  }
  let product = arithmetic_probe_input[0] * arithmetic_probe_input[1];
  let contraction = product + arithmetic_probe_input[2];
  let sum = arithmetic_probe_input[3] + arithmetic_probe_input[4];
  let reassociation = sum + arithmetic_probe_input[5];
  arithmetic_probe_output[0] = vec2<f32>(contraction, reassociation);
  arithmetic_probe_output[1] = two_sum(arithmetic_probe_input[6], arithmetic_probe_input[7]);
  arithmetic_probe_output[2] = two_prod(arithmetic_probe_input[0], arithmetic_probe_input[1]);
}
