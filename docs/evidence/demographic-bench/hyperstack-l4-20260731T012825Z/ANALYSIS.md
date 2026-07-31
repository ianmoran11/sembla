# Incomplete H100 final-state A/B/C attempt

This attempt at repository commit
`632ed94e0a4f9040f7d5e0995abba86324a020d2` is **not decision evidence**.

The correctness preflight passed, the independent timed matrix completed, and
the CRN A/B/C set completed with byte-identical outputs. Profile arm 25 then
ran successfully, but Nsight wrote its report under `/tmp` because the declared
`-o` parent directory did not exist. The collector failed closed when the
expected report was absent, so arms 26–27 and the analyzer did not run.

The defect was fixed by creating the report parent before launch and covered by
a regression test in commit `7b0e83e0b4284654c3fe1c73382fd939b0317616`.
Do not calculate a promotion verdict from this directory; use the subsequent
`hyperstack-l4-20260731T015324Z` run.

Teardown completed in one attempt: status 0, empty Terraform state, zero
Hyperstack VMs/orphans, and GPU performance-counter access remained admin-only.
The original `SHA256SUMS` covers the collected attempt; the sidecar
`SHA256SUMS.local-analysis` covers this explanatory file.
