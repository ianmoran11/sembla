# Incomplete collection — diagnostic harness selected zero tests

This collection was stopped during the 10M baseline CPU sweep. Inspection found
that the diagnostic test parent launched an incomplete test path and its child
ran zero tests. Consequently, the reported corpus pass did not establish the
negative diagnostic/error-recovery coverage and is not accepted as the final
correctness gate. Compute Sanitizer ran the other three hardware test bodies.

The runtime implementation was not changed in response. Commit
107a686cebbcc4b002aa90b34cbdfb4448405860 fixes test selection, adds a test-discovery
regression, and makes the corpus reject logs without diagnostic case results.
Validation was restarted on the same H100 spot VM with the original destruction
deadline retained. See the later collection for final results.

These files were copied from the stopped remote directory. The unfinished 10M
outputs are partial; no 10M timing or frozen-gate result is claimed. The completed
1M runs are retained for audit, not used as the final performance sample.
Generated input states, the temporary build checkout, and work directory are
excluded. This local SHA256SUMS protects the retrieved snapshot; no complete
remote manifest was produced by this stopped collection.
