# Two state artifacts are intentionally absent

`sweep/1000000/initial.state` (46 MB) and `sweep/10000000/initial.state`
(458 MB) were removed before committing. GitHub rejects any file over 100 MB,
and the pair added 504 MB to a bundle that is otherwise 26 MB.

**No evidence is lost.** Both are deterministic outputs of `synth-state`, their
SHA-256 digests are recorded in `state.sha256` beside each and in `SHA256SUMS`
and `SHA256SUMS.remote`, and the 10M digest
`02934c1f4161ced37395e82dacf64039cdb99f1d12434e83c5a87f0b07c9b57c` is the same
artifact the frozen §L4 gate used in this session.

`sha256sum -c SHA256SUMS` therefore reports exactly two missing files and
verifies everything else. That is deliberate: the manifests are left exactly as
the remote host produced them, because editing a manifest to make verification
pass would destroy the property it exists to provide.

Regenerate either with:

```sh
sembla synth-state \
  --model fixtures/demographic/benchmark/demographic_slots.full.json \
  --slots <1000000|10000000> --areas 4 --present-fraction 0.8 \
  --streams birth:600,overseas:250,internal:150 --seed 9009 \
  --out initial.state
```

The collector now deletes these before packaging, so later bundles will not
contain them at all.
