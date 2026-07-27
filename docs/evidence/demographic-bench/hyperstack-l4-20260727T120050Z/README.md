# CUDA validation §L4 frozen benchmark evidence

This directory records one frozen-case session on one host at repository commit
`00389a7876804c34ed2f57af68d39e13151d30ca`. The collector asserted that every arm used binary SHA-256
`8b206e6cc3f6fbd03839d0b92507bbfe6c50985fff5c93afd869dc90b7939be8` and initial-state SHA-256 `02934c1f4161ced37395e82dacf64039cdb99f1d12434e83c5a87f0b07c9b57c`; it aborted if the commit,
binary, or state changed.

## Gate result

- CUDA no-grouped replicates: 26.070, 26.370, 27.570 s; median **26.370 s**; spread 26.070–27.570 s.
- CPU no-grouped replicates: 50.700, 50.480, 50.040 s; median **50.480 s**; spread 50.040–50.700 s.
- Same-host CPU-median / CUDA-median ratio: **1.914×**.
- §L4 verdict: **NOT MET** (required: CUDA at least 3× faster).

## Ageing share

Paired full/no-ageing CPU replicates produce ageing shares
40.79%, 40.21%, 39.19%; median **40.21%**;
spread 39.19%–40.79%. This **strengthens**
the existing evidence for the §K2 10% trigger. It does **not** decide §K2.

`bench-results.json` is the machine-readable record. `bench-results.md` lists all
raw replicate timings. GPU, CPU, and RAM provenance are in the three named
`*-provenance.txt` files. Verify the directory with `sha256sum -c SHA256SUMS`
(or `shasum -a 256 -c SHA256SUMS` on macOS).

## Collection note (added 2026-07-27)

This session lost SSH mid-run: the operator's domestic egress IP changed from
`60.242.183.126` to `220.244.0.146`, and SSH is restricted by a `/32` twice over
— a Hyperstack security rule and an iptables rule cloud-init bakes into the
guest at boot. The guest rule uses `-j DROP`, so connections hung rather than
being refused, and the collector's polls discard SSH errors, so twenty minutes
of failure was indistinguishable from twenty minutes of progress.

The benchmark itself was unaffected: the remote job is detached, and it ran to
`SEMBLA_BENCH_COMPLETE` while unreachable. Recovery was an added API security
rule plus an added iptables rule via the VNC console, after which re-running the
collector rejoined the completed run and transferred the artifacts.

`SHA256SUMS.remote` was written on the VM and verified against the transferred
tarball before teardown. The local `SHA256SUMS` was regenerated afterwards
because the driver exited before its final manifest step: `terraform destroy`
received an HTTP 500 from the Hyperstack API, so the mandatory-teardown trap
fired. Terraform state was left with no VM resources.

No evidence was pushed to GitHub this session. `TF_VAR_evidence_deploy_key` was
unset at plan time, so cloud-init wrote `EVIDENCE_PUSH_ENABLED=0` and the payload
skipped the push. The path is untested in production as a result.
