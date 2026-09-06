# CUDA follow-up: initial compilation failure

Initial approved candidate: `7c4e03886b6232c8d99c47bef80b08420f3a0df3`.
The first NVRTC compilation failed because the new fired-count reduction used
`partials` for unsigned dynamic shared memory, colliding with an existing signed
shared declaration in another kernel. No performance result was accepted.

`failure-profile-cuda.stderr` retains the compiler diagnostic; `remote-run.log`
retains the collector progress. The local regression check now verifies shared
names have consistent types across each generated translation unit.

Fixed candidate: `e05261ee1428dec77d9e53da004df2614ad186f6`. The retry uses the
same approved H100 VM and destruction watchdog. Infrastructure variables and
the applied plan were not changed. The collector now runs the differential
corpus before profiles, matching the runbook's intended ordering.
