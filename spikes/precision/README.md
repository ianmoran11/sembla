# Precision strategy spike

Standalone throwaway crate for choosing the v0.2 GPU precision strategy. PRD
0001 contains the stable real-valued workload, adapter sizing probe, and scalar
CPU `f64` oracle only; GPU compute begins in PRD 0002.

## Workspace isolation

The empty `[workspace]` table in this crate's `Cargo.toml` makes
`spikes/precision/` a nested workspace root. The repository root also lists only
its three production crates as workspace members. Consequently, root
`cargo build --workspace` does not discover or compile this spike; run Cargo
commands from this directory to build it.

## Workload contract

Defaults are 26,000,000 person rows and 1,300,000 contiguous employer groups
(20 rows/group). Each person has `employer: u32`, SIR `health: u32`, and a
Philox-generated binary64 susceptibility `weight` in `(0, 1)`. A tick performs:

1. fixed-order segmented sums of infectious weights;
2. susceptible hazard/race mapping with `lambda = beta * sum / group_size` and
   `t = -ln(1-u) / lambda`;
3. segmented argmin for rows selected by `entity_id % 10 == 5`, keyed by
   employer and ordered exactly by `(t_bits, rule_id, entity_id)`.

An eligible unselected row fires directly. For selected rows, only the per-key
winner fires. The oracle also repeats each group sum in descending entity order
and records bitwise differences from its canonical ascending order.

Static weights reserve Philox coordinates
`(tick=0, rule_id=0xffff_fe00, entity_id, draw_idx=0)`. Infection races use
`(tick, rule_id=0, entity_id, draw_idx=0)`. The key is `(seed_lo, seed_hi)` and
counter words are `(tick, rule_id, entity_id, draw_idx)`.

## Probe and sizing

```console
cargo run
```

The probe prints adapter name/backend/device type, `SHADER_F64`, safe `(N, G)`,
and any downscale reason. The sizing model checks the largest 8-byte storage
binding and a documented resident footprint of 32 bytes/person plus 12
bytes/employer. Because wgpu exposes no portable heap-budget query, aggregate
resident bytes are conservatively bounded by `min(max_buffer_size, 1 GiB)`.
Software adapters additionally use a 200,000-row functional safety cap.
