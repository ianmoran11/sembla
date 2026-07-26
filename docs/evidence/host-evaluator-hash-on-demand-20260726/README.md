# Host evaluator on-demand per-tick hash evidence — 2026-07-26

## Fixed case

Both builds ran the binding host-evaluator case on the same Apple M2 Pro in one
session: the no-grouped demographic model, 1,000,000 slots, 24 ticks, seed
9009, four areas, present fraction 0.8, streams
`birth:600,overseas:250,internal:150`, and the CPU backend. The resized model
and initial state were shared byte-for-byte across every run.

The before build was commit
`65fd6c2a2ec1c73de0fad3a9548c7ade960e2c9a`. The implementation build was made
from that worktree with only the PRD 0003 `main.rs` changes. Each measurement
used:

```sh
/usr/bin/time -lp <binary> run <resized-no-grouped-model> \
  --seed 9009 --population <shared-1m-state> --backend cpu --ticks 24 \
  --out <run-output.csv>
```

Input SHA-256 values:

- resized model: `601766d8c11443cb05da2500b00bb78fade375b8df2d0323bae35b7d8a17a130`
- initial state: `896e0062228b74ba24df95e53e28ca368df510f957ed03ef2f49160590a6922b`

## Results

| Build | Run 1 wall/user/sys | Run 2 wall/user/sys | Run 3 wall/user/sys | Median wall | Median user |
|---|---:|---:|---:|---:|---:|
| Before | 11.57 / 9.34 / 1.57 s | 10.88 / 9.24 / 1.54 s | 11.09 / 9.30 / 1.61 s | 11.09 s | 9.30 s |
| After | 12.96 / 6.88 / 2.46 s | 12.15 / 6.89 / 2.53 s | 8.17 / 6.19 / 1.70 s | 12.15 s | 6.88 s |

After runs 1 and 2 were contended: wall time exceeded user plus system time by
3.62 s and 2.73 s respectively, versus at most 0.66 s before and 0.28 s in
after run 3. The wall median therefore moved from 11.09 s to 12.15 s and is not
the load-bearing comparison. **User time is load-bearing:** its median improved
from 9.30 s to 6.88 s, a **1.35× speedup** and **26.02% reduction**. All three
runs remain in the reported dataset; none was discarded or replaced.

Every before, after, and profiled primary CSV is byte-identical at SHA-256
`eb6d095740127bbf41576d6b05f1470656dbb5f85372ef2ff5f1751576303e37`.
Every summaries CSV is byte-identical at
`329bc9e17af3032a81d4dd60263cd70c26fd734e33fce7cfcb8da66558bca6d3`,
and every run manifest is byte-identical at
`dbeaa57719ef88945ac46336ad8033c5cdac91d8cd5b8bb21679437e0122aa1f`.
The emitted `final_state_sha256` remains
`2d509ead9aa506e71be155faaa5608542f7ca32cee203ee42b0d3179d670020c`.

## Full-duration post-change profile

`post-change-full-duration.sample.txt` covers the profiled process lifetime,
not a fixed prefix. `/usr/bin/sample` was started before the benchmark with
`-wait -mayDie`; 600 seconds was only an upper bound, and sampling ended when
the benchmark process exited:

```sh
sample sembla-prd0003-final-profile 600 1 -wait -mayDie \
  -file post-change-full-duration.sample.txt &
sembla-prd0003-final-profile run <resized-no-grouped-model> \
  --seed 9009 --population <shared-1m-state> --backend cpu --ticks 24 \
  --out <profile-output.csv>
wait
```

The capture contains 6,579 one-millisecond main-thread samples, with 6,471 in
main execution. There is no `StateStore::state_hash` branch beneath
`execute_backend_output_with_features` or the tick loop. The only
`StateStore::state_hash` branch is the required final call beneath
`execution_hashes` (135 samples), proving that plain output-producing `run`
no longer hashes state per tick while retaining the recorded final digest.
The profile SHA-256 is
`55b130429e8cae62a96a1610b31a705cd1f7c51bbfa50fdac14486b3b1c902fa`.
