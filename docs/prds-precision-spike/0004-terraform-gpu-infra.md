---
max_review_cycles: 3
---

# PRD 0004: Terraform — CUDA-capable NVIDIA GPU instance

## Context

Native `f64` (PRD 0003) cannot be measured on the Apple M2 dev machine. This PRD
provisions, via Terraform, an NVIDIA GPU instance where the native-`f64` and CUDA
paths actually run, so the spike can fill the "native `f64`" cells of the
benchmark matrix (PRD 0005). Terraform `validate`/`plan` must succeed locally
without cloud credentials (the automated `/piprd` loop can go this far); `apply`,
the remote build, and the benchmark run are documented **manual** steps a human
performs with credentials, and whose numbers are pasted into `RESULTS.md`.

The fp64 throughput class of the chosen instance is a first-order concern
(`DESIGN.md` §5.2, §10.3): a commodity GPU (T4/L4/A10G) rate-limits `f64` to
~1:32–1:64 and gives a *pessimistic* native-`f64` number; a datacenter-compute
GPU (A100/V100/H100) at ~1:2 gives the number the v0.2 decision actually needs.
The module must make the instance's fp64 class explicit and easy to switch.

Implements: `docs/ROADMAP.md` v0.2 option A (measurement infrastructure);
`DESIGN.md` §5.2, §10.3.

## Goal

A self-contained Terraform module under `spikes/precision/infra/` that stands up a
single CUDA-capable NVIDIA GPU VM, with variables to select the GPU class
(commodity vs full-rate `f64`), a bootstrap that installs the toolchain, a
documented run + fetch-results + teardown workflow, and a prominent cost note.

## Specification

- **Module layout** `spikes/precision/infra/`: `main.tf`, `variables.tf`,
  `outputs.tf`, `versions.tf`, `README.md`, and a `cloud-init`/user-data
  bootstrap script. Default cloud provider: AWS (widely available GPU instances);
  keep provider-specific bits isolated so a GCP variant could be added later.
- **Instance selection via variable** `gpu_class`:
  - `commodity` (default, cheapest honest signal) → e.g. `g4dn.xlarge` (T4) or
    `g5.xlarge` (A10G) — records fp64 as **rate-limited**;
  - `full_rate` → e.g. `p4d`/`p3` (A100 / V100) — records fp64 as **full-rate**,
    the number the decision hinges on. Document on-demand vs spot and the price
    delta in the module README.
  The `RESULTS.md` fp64-class field (PRD 0003/0005) must reflect this choice.
- **Bootstrap (user-data):** install NVIDIA driver + CUDA toolkit + a recent Rust
  toolchain, clone/copy the spike, and leave a one-command run script
  (`run-spike.sh`) that builds `--release --features cuda`, executes the
  benchmark (PRD 0005), and writes `RESULTS.md` on the box for retrieval. Prefer a
  prebuilt Deep Learning AMI to shorten driver setup; pin the AMI via a variable
  with a documented default and region.
- **Security + lifecycle:** SSH ingress locked to a caller-supplied CIDR variable
  (no `0.0.0.0/0` default), key pair via variable, an explicit `terraform destroy`
  teardown step in the README, and — because GPU instances are expensive — a
  loud cost warning plus an optional auto-stop/self-terminate timer variable
  (default on) so a forgotten box does not bill indefinitely.
- **State + secrets:** local state only (this is a throwaway spike; document that
  and `.gitignore` `*.tfstate*` and `.terraform/` under the module). No secrets
  committed; credentials come from the operator's environment.
- **Docs (`infra/README.md`):** exact `init → validate → plan → apply → ssh →
  run-spike.sh → scp RESULTS.md back → destroy` sequence; the `gpu_class`
  trade-off and its effect on the fp64-class label; estimated $/hour for each
  class; and the reminder to run `full_rate` at least once if the `commodity`
  native-`f64` number is not clearly decisive.

## Non-goals

Running `terraform apply` inside the automated `/piprd` loop (manual, needs
credentials), multi-GPU / clusters, remote state backends, CI integration, a
production/reusable module (this is throwaway infra for one measurement), any
non-AWS provider in this PRD (leave the seam, don't build it).

## Acceptance criteria

1. `terraform init` and `terraform validate` succeed in `spikes/precision/infra/`
   with no cloud credentials; `terraform fmt -check` is clean.
2. `terraform plan` with a documented example `tfvars` (region, key, SSH CIDR,
   `gpu_class`) produces a plan for exactly one GPU instance and its minimal
   supporting resources, with no `0.0.0.0/0` ingress.
3. The `gpu_class` variable switches instance type and the recorded fp64-class
   label; both `commodity` and `full_rate` values validate.
4. The bootstrap script and `run-spike.sh` are present, install the CUDA + Rust
   toolchain, and build/run the spike with `--features cuda`.
5. `infra/README.md` documents the full apply→run→fetch→destroy workflow, the
   cost warning, the auto-stop timer, and the `gpu_class` fp64-class trade-off;
   `*.tfstate*` and `.terraform/` are gitignored.
