# PRD 0003 Review — Attempt 1

## Assessment

**REVISE**

## Blocking issue

- `scripts/check.sh:11-13` does not fully enforce acceptance criterion 5. Its regex detects ordinary dependency keys and double-quoted inline aliases, but misses valid Cargo forms including `[dependencies.rand]`, `package = 'rand'`, and workspace-inherited aliases such as `my_rng.workspace = true`. This contradicts the script comment that aliases cannot bypass the guard. Replace the text grep with manifest-aware dependency inspection (including resolved workspace inheritance and renamed package names), or otherwise cover all valid Cargo declaration forms.

## Acceptance criteria

1. **Known answers: PASS.** Random123 all-zero and all-ones vectors pass; an asymmetric reference vector also freezes coordinate packing.
2. **Purity and collisions: PASS.** Every public function is called twice identically; 1,000 coordinate sets independently perturb every coordinate and reject collisions across full 128-bit outputs.
3. **Uniform statistics: PASS.** One million samples are strictly open, meet the mean tolerance, and satisfy the 100-bucket 5% bound. Extreme mantissa tests cover both endpoints.
4. **Exponential statistics: PASS.** One million λ=2 samples meet the mean tolerance; zero and negative rates return infinity.
5. **No external RNG dependency: REVISE.** The current manifest and lockfile are clean, but the required enforcement is bypassable as described above.
6. **Rustdoc: PASS.** Exact key/counter packing and CRN behavior across scenarios are documented.

## Checks

- `./scripts/check.sh`: passed
- Workspace tests: 21 passed, including all Philox/statistical tests
- Rustdoc with warnings denied: passed
- `git diff --check`: passed
- Guard probes: direct key detected; double-quoted inline alias detected; dependency table, literal-string alias, and workspace alias missed

The implementation otherwise remains within PRD 0003 scope. Unrelated `.DS_Store` and managed `.piprd*` state are not implementation blockers.
