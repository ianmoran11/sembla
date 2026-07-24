# Demographic benchmark model variants

These are benchmark fixtures, not canonical scientific models. They are derived
from `../demographic_slots.json` and are guarded by
`crates/sembla-cli/tests/synth_state.rs`:

- `demographic_slots.full.json` is byte-equivalent IR to the canonical model;
- `demographic_slots.no-ageing.json` removes only `age_monthly`;
- `demographic_slots.no-grouped.json` removes only the grouped views.

`scripts/bench-demographic.sh` rewrites only table `size_hint` values in working
copies for each requested scale. Any other drift from the canonical model makes
the test fail.
