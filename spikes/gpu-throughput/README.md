# GPU throughput spike

This is the throwaway PRD-0012 benchmark. It is intentionally a standalone
Cargo workspace and is not linked to any Sembla production crate.

```sh
cargo build
cargo test
cargo run --release
```

The release run auto-selects a wgpu adapter, chooses the requested 26M/1.3M
scale when per-buffer limits and a conservative aggregate resident-memory
budget permit, performs 10 warmup and 100 measured ticks, and overwrites
`RESULTS.md` with the machine's measurements. Software adapters are reduced
automatically and explicitly leave the throughput question unanswered.

The columns remain device-resident across ticks. Each tick restores the fixed
initial enum column to keep the measured workload steady, clears scratch
storage, atomically aggregates infectious counts, maps Philox draws to
exponential-race candidates, sends about 10% of all rows (two eligible rows
per employer at the canonical scale) through two-pass atomic segmented argmin,
and writes enum state plus a fired counter. Every compute pipeline uses one
bind group. GPU tests contain no dependency on `sembla-runtime`; their scalar
Philox and tick implementations are local, and the smoke test checks exact
candidate flags, lexicographic winner IDs, and loser suppression.

### Precision scope

The portable WGSL path uses `f32` for hazard and exponential-race arithmetic.
Portable WGSL on the measured Apple Metal adapter does not expose shader
`f64`, although Sembla's production numeric convention requires `f64`. The
10k smoke test therefore establishes exact parity with a scalar implementation
of this spike's `f32` arithmetic, not differential parity with the production
CPU runtime. `RESULTS.md` treats the measurement as directional kernel-shape
evidence and leaves production-`f64` throughput unanswered; native-only shader
paths or software-double emulation would answer a different question and are
outside this throwaway WGSL spike.
