# Full and one-in-ten initial-state verification

Date: 2026-08-06  
Host: Apple M2 Pro, arm64, Python 3.14.0, rustc 1.79.0

This is the explicit heavy verification for the generated-on-demand `full` and
`tenth` initial states. The binary artifacts are ignored and were removed after
verification; their reproducible hashes remain binding in
[`initial-state-2010.md`](../../../../data/abs/extracts/initial-state-2010.md).

## Commands

```bash
python3 data/abs/build_state.py \
  --scale tenth \
  --model fixtures/australian-population/australian_population.hundredth.json
python3 data/abs/build_state.py \
  --scale full \
  --model fixtures/australian-population/australian_population.hundredth.json
sembla validate \
  data/abs/generated/australian_population_2010_tenth.state.model.json
sembla validate \
  data/abs/generated/australian_population_2010_full.state.model.json
cargo test -p sembla-cli --test australian_population \
  generated_full_and_tenth_artifacts_match_hashes_and_enforce_rows \
  -- --ignored --nocapture
```

## Builder output

```text
scale=tenth present=2202870 birth=500805 overseas=820917 total=3524592
state sha256 sembla.state-artifact/v1 926fc80a0330c764cac8c4a69c09dee6b4d088653dc9bf830c9e068a2b87c02a
file sha256 b6f7e827df3da3d247f4194194c0ed3770e16677bc71b739cd617c19cbd96ee2
elapsed_seconds=18.40

scale=full present=22028695 birth=5008051 overseas=8209168 total=35245914
state sha256 sembla.state-artifact/v1 ef3c93722199cf96b4d7839d79561de1f4c0e944924c4a9d490e64ba6a92c083
file sha256 55429a20f03d2a0f63d1490ca13e0191579451043c99b573bed037fb398461ae
elapsed_seconds=179.43
```

These displayed results were the second complete materialisations on the same
host. The first pass took 19.62 seconds (`tenth`) and 196.04 seconds (`full`);
both ordinary file hashes and both domain-separated state hashes were identical.
After strengthening the negative gate to mutate each table independently, a
third pass (20.66 and 194.46 seconds) again reproduced all four hashes and fed
the Rust result below. This is a byte comparison through independent digest
computations, not merely a repeated row-count report.

## Companion and Rust checks

| scale | state bytes | paired-model bytes | paired-model SHA-256 | declared rows per table |
|---|---:|---:|---|---:|
| `tenth` | 169,181,189 | 529,151 | `f4354847fca022182e34288b2056ef066513cb05dc4be1cf0c4aa81a6b85e562` | 3,524,592 |
| `full` | 1,691,804,647 | 529,153 | `29af3565f938d4613ab13352f024e8ae08b0c7bff2746d2f9c5d0a4a53311f7e` | 35,245,914 |

Both generated companions passed the unchanged public `sembla validate`
command. They omit feature-gated grouped views; a routine structural test proves
their tables, parameters, transitions and scalar views match the canonical
execution model after restoring those views.

The ignored Rust test independently:

1. validates each scale-specialised companion without hidden feature state;
2. loads the Python artifact through `sembla.state/v1` and checks both table row
   counts;
3. computes the Rust domain-separated state hash and compares it with the
   builder record; and
4. increments each declared row count independently and proves the same
   artifact is rejected in both cases.

```text
running 1 test
test generated_full_and_tenth_artifacts_match_hashes_and_enforce_rows ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out
finished in 13.08s
```

The test is ignored in routine CI because it materialises approximately 1.86 GB
of generated state and deliberately performs full Rust decoding. The committed
1:100 artifact exercises the same byte and loader path in every ABS check.
