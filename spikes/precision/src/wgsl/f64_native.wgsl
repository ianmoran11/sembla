// Native binary64 hot path. This module is created only after a Vulkan adapter
// advertises SHADER_F64 and the requested device enables that feature.
//
// Reduction order matches the Rust mirror and CUDA reference: two ascending
// half-group partials followed by partial 0 + partial 1. Race times are
// non-negative, so numeric f64 ordering is exactly equivalent to ordering their
// IEEE-754 t_bits; rule_id is then compared before entity_id.

struct NativeConfig {
  rows: u32,
  groups: u32,
  tick: u32,
  map_workgroups_x: u32,
  seed_lo: u32,
  seed_hi: u32,
  partials_per_group: u32,
  _pad0: u32,
  beta: f64,
  dt: f64,
}

@group(0) @binding(0) var<uniform> config: NativeConfig;
@group(0) @binding(1) var<storage, read> group_offsets: array<u32>;
@group(0) @binding(2) var<storage, read> employers: array<u32>;
@group(0) @binding(3) var<storage, read> health: array<u32>;
@group(0) @binding(4) var<storage, read> weights: array<f64>;
@group(0) @binding(5) var<storage, read_write> partial_sums: array<f64>;
@group(0) @binding(6) var<storage, read_write> segmented_sums: array<f64>;
@group(0) @binding(7) var<storage, read_write> race_times: array<f64>;
@group(0) @binding(8) var<storage, read_write> winner_entities: array<u32>;
@group(0) @binding(9) var<storage, read_write> fired_flags: array<u32>;

const INFECTIOUS: u32 = 1u;
const SUSCEPTIBLE: u32 = 0u;
const INFECTION_RULE_ID: u32 = 0u;
const NO_WINNER: u32 = 0xffffffffu;
const MAX_FINITE_F64: f64 = 1.7976931348623157e308lf;

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

fn uniform_open_f64(words: vec4<u32>) -> f64 {
  let high = f64(words.x) * 2097152.0lf;
  let low = f64(words.y >> 11u);
  let sample = (high + low + 0.5lf) * 1.1102230246251565e-16lf;
  return min(sample, 0.9999999999999999lf);
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
fn reduce_partial_f64(@builtin(global_invocation_id) gid: vec3<u32>) {
  let partial_index = gid.x;
  if (partial_index >= config.groups * config.partials_per_group) {
    return;
  }
  let bounds = partial_bounds(partial_index);
  var sum = 0.0lf;
  for (var row = bounds.x; row < bounds.y; row = row + 1u) {
    if (health[row] == INFECTIOUS) {
      sum = sum + weights[row];
    }
  }
  partial_sums[partial_index] = sum;
}

@compute @workgroup_size(64)
fn reduce_finish_f64(@builtin(global_invocation_id) gid: vec3<u32>) {
  let group = gid.x;
  if (group >= config.groups) {
    return;
  }
  segmented_sums[group] =
    partial_sums[group * config.partials_per_group] +
    partial_sums[group * config.partials_per_group + 1u];
}

@compute @workgroup_size(256)
fn map_f64(@builtin(global_invocation_id) gid: vec3<u32>) {
  let row = map_row_index(gid);
  if (row >= config.rows) {
    return;
  }
  fired_flags[row] = 0u;
  race_times[row] = MAX_FINITE_F64;
  if (health[row] != SUSCEPTIBLE) {
    return;
  }

  let group = employers[row];
  let group_size = f64(group_offsets[group + 1u] - group_offsets[group]);
  let lambda = config.beta * segmented_sums[group] / group_size;
  if (lambda <= 0.0lf) {
    return;
  }
  let words = philox4x32_10(config.seed_lo, config.seed_hi, config.tick, INFECTION_RULE_ID, row, 0u);
  let uniform = uniform_open_f64(words);
  let time = -log(1.0lf - uniform) / lambda;
  race_times[row] = time;
  if (time < config.dt && !is_contested(row)) {
    fired_flags[row] = 1u;
  }
}

@compute @workgroup_size(64)
fn argmin_f64(@builtin(global_invocation_id) gid: vec3<u32>) {
  let group = gid.x;
  if (group >= config.groups) {
    return;
  }
  var best_time = MAX_FINITE_F64;
  var best_rule = 0xffffffffu;
  var best_entity = NO_WINNER;
  let start = group_offsets[group];
  let end = group_offsets[group + 1u];
  for (var row = start; row < end; row = row + 1u) {
    let time = race_times[row];
    if (!is_contested(row) || !(time < config.dt)) {
      continue;
    }
    let better = time < best_time ||
      (time == best_time &&
        (INFECTION_RULE_ID < best_rule ||
          (INFECTION_RULE_ID == best_rule && row < best_entity)));
    if (better) {
      best_time = time;
      best_rule = INFECTION_RULE_ID;
      best_entity = row;
    }
  }
  winner_entities[group] = best_entity;
  if (best_entity != NO_WINNER) {
    fired_flags[best_entity] = 1u;
  }
}
