# CUDA follow-up: test-oracle setup failure

Candidate `e05261ee1428dec77d9e53da004df2614ad186f6` compiled on H100 and
passed the existing resource test. The new grouped/fused test stopped before
comparing results because its CPU oracle did not enable `grouped-observations`.
`optimization-corpus.log` retains that diagnostic. The collector correctly
stopped before profiles and the frozen gate. No performance result was accepted.

Candidate `7a62c65be7bbabcc38a43826d8d6d619aee33011` fixes only that test setup.
The retry uses the same approved VM, price, infrastructure, and watchdog.
