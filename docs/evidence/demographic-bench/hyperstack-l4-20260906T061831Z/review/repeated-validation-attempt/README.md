# Incomplete repeated-validation attempt

No repeated performance result is accepted from this attempt. The first 1M
baseline warmup returned exit 1 before a paired timed sweep was recorded.
The coordinator resumed the original collector, which had retrieved and verified
the primary archive and then destroyed the VM. Per-command stderr, final-build
logs and final-corpus logs on the VM were not transferred before teardown.

The protocol retained the baseline binary but the primary collector had removed
its original build checkout. The unchanged CLI repository_commit() resolves
CARGO_MANIFEST_DIR/../.. at runtime for native timing output, so that deleted
checkout makes --timing-json fail. This cause follows from the retained protocol
and source; the baseline's stderr was not recovered. A future repeat must retain
or recreate each binary's build checkout and package diagnostic artifacts on
failure before resuming teardown.

The script reached the first baseline warmup only after its checked build and
hardware-corpus subprocesses returned zero for d584369. Their raw logs were not
retrieved, so the independently retained hardware corpus and timing evidence in
this directory's parent belong to the original f84bb05 candidate. The final
copy-order correction has passing retained local checks; neither its peak RSS
nor repeated whole-command performance was confirmed by this attempt.
