struct Config {
    rows: u32,
    groups: u32,
    workgroups_x: u32,
    tick: u32,
    seed_lo: u32,
    seed_hi: u32,
    group_size: u32,
    _pad0: u32,
    beta: f32,
    dt: f32,
    _pad1: u32,
    _pad2: u32,
}

struct KatInput {
    seed_lo: u32,
    seed_hi: u32,
    tick: u32,
    rule_id: u32,
    entity_id: u32,
    draw_idx: u32,
}

struct KatOutput { words: array<u32, 4>, }

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

fn philox(seed_lo: u32, seed_hi: u32, tick: u32, rule_id: u32,
          entity_id: u32, draw_idx: u32) -> vec4<u32> {
    var c = vec4<u32>(tick, rule_id, entity_id, draw_idx);
    var key = vec2<u32>(seed_lo, seed_hi);
    for (var round = 0u; round < 10u; round = round + 1u) {
        let lo0 = 0xd2511f53u * c.x;
        let hi0 = mul_hi(0xd2511f53u, c.x);
        let lo1 = 0xcd9e8d57u * c.z;
        let hi1 = mul_hi(0xcd9e8d57u, c.z);
        c = vec4<u32>(hi1 ^ c.y ^ key.x, lo1, hi0 ^ c.w ^ key.y, lo0);
        if (round != 9u) {
            key = key + vec2<u32>(0x9e3779b9u, 0xbb67ae85u);
        }
    }
    return c;
}

fn row_index(gid: vec3<u32>, workgroups_x: u32) -> u32 {
    return gid.x + gid.y * workgroups_x * 256u;
}

@group(0) @binding(0) var<storage, read> kat_inputs: array<KatInput>;
@group(0) @binding(1) var<storage, read_write> kat_outputs: array<KatOutput>;

@compute @workgroup_size(64)
fn philox_kat(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= arrayLength(&kat_inputs)) { return; }
    let k = kat_inputs[gid.x];
    let value = philox(k.seed_lo, k.seed_hi, k.tick, k.rule_id, k.entity_id, k.draw_idx);
    kat_outputs[gid.x].words[0] = value.x;
    kat_outputs[gid.x].words[1] = value.y;
    kat_outputs[gid.x].words[2] = value.z;
    kat_outputs[gid.x].words[3] = value.w;
}

@group(0) @binding(10) var<uniform> clear_config: Config;
@group(0) @binding(11) var<storage, read_write> clear_counts: array<atomic<u32>>;
@group(0) @binding(12) var<storage, read_write> clear_best_race: array<atomic<u32>>;
@group(0) @binding(13) var<storage, read_write> clear_winner: array<atomic<u32>>;
@group(0) @binding(14) var<storage, read_write> clear_fired: atomic<u32>;
@group(0) @binding(15) var<storage, read_write> clear_health: array<u32>;

@compute @workgroup_size(256)
fn clear(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = row_index(gid, clear_config.workgroups_x);
    if (i < clear_config.groups) {
        atomicStore(&clear_counts[i], 0u);
        atomicStore(&clear_best_race[i], 0xffffffffu);
        atomicStore(&clear_winner[i], 0xffffffffu);
    }
    if (i < clear_config.rows) {
        clear_health[i] = select(0u, 1u, i % clear_config.group_size < clear_config.group_size / 5u);
    }
    if (i == 0u) { atomicStore(&clear_fired, 0u); }
}

@group(0) @binding(20) var<uniform> aggregate_config: Config;
@group(0) @binding(21) var<storage, read> aggregate_health: array<u32>;
@group(0) @binding(22) var<storage, read> aggregate_employer: array<u32>;
@group(0) @binding(23) var<storage, read_write> aggregate_counts: array<atomic<u32>>;

@compute @workgroup_size(256)
fn aggregate(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = row_index(gid, aggregate_config.workgroups_x);
    if (i >= aggregate_config.rows) { return; }
    if (aggregate_health[i] == 1u) {
        atomicAdd(&aggregate_counts[aggregate_employer[i]], 1u);
    }
}

@group(0) @binding(30) var<uniform> map_config: Config;
@group(0) @binding(31) var<storage, read> map_health: array<u32>;
@group(0) @binding(32) var<storage, read> map_employer: array<u32>;
@group(0) @binding(33) var<storage, read> map_counts: array<u32>;
@group(0) @binding(34) var<storage, read_write> map_candidate: array<u32>;
@group(0) @binding(35) var<storage, read_write> map_race: array<u32>;

@compute @workgroup_size(256)
fn hazard_map(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = row_index(gid, map_config.workgroups_x);
    if (i >= map_config.rows) { return; }
    let lanes = philox(map_config.seed_lo, map_config.seed_hi, map_config.tick, 0u, i, 0u);
    let u = (f32(lanes.x) + 0.5) * (1.0 / 4294967296.0);
    let infectious = f32(map_counts[map_employer[i]]);
    let lambda = map_config.beta * infectious / f32(map_config.group_size);
    let race = select(3.402823466e+38, -log(u) / lambda, lambda > 0.0);
    map_race[i] = bitcast<u32>(race);
    map_candidate[i] = select(0u, 1u, map_health[i] == 0u && race < map_config.dt);
}

@group(0) @binding(40) var<uniform> argmin_config: Config;
@group(0) @binding(41) var<storage, read> argmin_employer: array<u32>;
@group(0) @binding(42) var<storage, read> argmin_candidate: array<u32>;
@group(0) @binding(43) var<storage, read> argmin_race_values: array<u32>;
@group(0) @binding(44) var<storage, read_write> argmin_best_race: array<atomic<u32>>;

// About 10% of rows claim their employer as a contested resource. Positive
// IEEE-754 race bits preserve race ordering for atomicMin.
@compute @workgroup_size(256)
fn argmin_race(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = row_index(gid, argmin_config.workgroups_x);
    if (i >= argmin_config.rows) { return; }
    if (argmin_candidate[i] != 0u && i % 10u == 5u) {
        atomicMin(&argmin_best_race[argmin_employer[i]], argmin_race_values[i]);
    }
}

@group(0) @binding(50) var<uniform> resolve_config: Config;
@group(0) @binding(51) var<storage, read> resolve_employer: array<u32>;
@group(0) @binding(52) var<storage, read> resolve_candidate: array<u32>;
@group(0) @binding(53) var<storage, read> resolve_race: array<u32>;
@group(0) @binding(54) var<storage, read> resolve_best_race: array<u32>;
@group(0) @binding(55) var<storage, read_write> resolve_winner: array<atomic<u32>>;

// Second pass completes lexicographic (race, entity_id) ordering. rule_id is
// fixed at zero for this SIR transition, so entity_id is the remaining key.
@compute @workgroup_size(256)
fn argmin_tie(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = row_index(gid, resolve_config.workgroups_x);
    if (i >= resolve_config.rows) { return; }
    let employer = resolve_employer[i];
    if (resolve_candidate[i] != 0u && i % 10u == 5u &&
        resolve_race[i] == resolve_best_race[employer]) {
        atomicMin(&resolve_winner[employer], i);
    }
}

@group(0) @binding(60) var<uniform> write_config: Config;
@group(0) @binding(61) var<storage, read_write> write_health: array<u32>;
@group(0) @binding(62) var<storage, read> write_employer: array<u32>;
@group(0) @binding(63) var<storage, read> write_candidate: array<u32>;
@group(0) @binding(64) var<storage, read> write_winner: array<u32>;
@group(0) @binding(65) var<storage, read_write> write_fired: atomic<u32>;

@compute @workgroup_size(256)
fn state_write(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = row_index(gid, write_config.workgroups_x);
    if (i >= write_config.rows || write_candidate[i] == 0u) { return; }
    let wins = i % 10u != 5u || write_winner[write_employer[i]] == i;
    if (wins) {
        write_health[i] = 1u;
        atomicAdd(&write_fired, 1u);
    }
}
