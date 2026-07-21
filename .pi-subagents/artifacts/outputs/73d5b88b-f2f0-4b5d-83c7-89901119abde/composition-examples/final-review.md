## Review

- **High — `docs/design/composition-options.md:248-270`:** The note says `examples/sir_policy.json` “serializes the running example directly,” but it is not a serialization of the linked `Step05_PolicyFeedback.lean` model. The Lean model is named `tutorial_05_policy_feedback_sir`, uses schema field/state attribute `restriction`, thresholds `100`/`25`, and values `0.6`/`0.0` (`frontend/Sembla/Tutorial/Step05_PolicyFeedback.lean:25-26,37-42,54-74`). The JSON is named `sir_workplace_policy_feedback`, uses `modifier_offset`/`modifier`, thresholds `500`/`150`, and values `0.4`/`1.0` (`examples/sir_policy.json`). The two artifacts share the two-box/two-wire topology, but not the claimed concrete running-system serialization.

- **High — `docs/design/composition-options.md:100-107,385-396,861-885,1417-1425,1640-1642`:** The nesting/hiding examples cross an encapsulation boundary inconsistently. `EpidemicPolicy` exposes only `population.infection_count` as its boundary `infection_count`, and §4 says exposure is what lets a parent connect to a child port. Nevertheless, both `RegionalResponse` and `PublicPolicyModel` address and hide the unexposed grandchild path `epidemic/internal.policy.restriction_modifier`. Either all qualified descendant ports remain public—in which case the stated exposure boundary is not meaningful—or that deep `hide` is invalid/redundant. The document needs one default-visibility rule and examples that obey it.

- **Medium — `docs/design/composition-options.md:419-436,1125-1133`:** The proposed source-record example reverses the exposure shown everywhere else. Surface syntax `expose population.infection_count as infection_count` and the concrete ACSet-like record put the child port first and the composite boundary second, but line 435 draws `EpidemicPolicy.infection_count -> population.infection_count`. Because the same block uses directional arrows for wires and annotates delay, this gives contradictory lowering direction for the running exposure.

- **Medium — `docs/design/composition-options.md:1103-1135`:** The “minimal ACSet-like representation” cannot encode its own concrete exposure record. Its schema gives `Exposure` only `Exposure.inner -> Port`, with no foreign key to an outer/boundary port or owning composite; it also says every `Port.owner` is an `Instance`, while the record references the component boundary `EpidemicPolicy.infection_count` without an `EpidemicPolicy` instance record. Consequently `Exposure(population.infection_count, EpidemicPolicy.infection_count)` has an unmodeled second endpoint and cannot support the claimed ownership/connectivity validation as written.

- **Medium — `docs/design/composition-options.md:665-716,1442-1455`:** The repeated `admit_patient` family does not declare a correlation that satisfies its own no-Cartesian-search rule. It matches the person “by `person_id`” and the bed “by `hospital_id`”; no equality/binding relates those different keys and no selector supplies their values. The text then requires participant tuples to come from declared join keys. The example must use a shared key (for example `hospital_id` on both sides) or spell out an explicit selector/binding before it can serve as a concrete `CareNetwork` family.

- **Medium — `docs/design/composition-options.md:569-575,915-925`:** The proposed mailbox identity `(source instance-id, output port-id)` is not unique under the fan-out the note says may be allowed. The note defines each wire as a stateful mailbox, so two wires from one output to two target inputs would receive the same proposed identity. Mailbox/wire identity must include the target endpoint or a stable wire identity.

- **Low — `docs/design/composition-options.md:161-171`:** The `lean` fence containing `health` and `custody` declarations has no adjacent current/proposed/conceptual label, despite the explicit line 55-67 promise that status is repeated beside every block. It is the one syntax-looking fragment in the product distinctions whose parser status is not unmistakable.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "not_satisfied",
      "evidence": "The documentation-only scope is appropriate, but the reviewed edit retains semantic and provenance contradictions in the running examples, including the false direct-serialization claim and invalid boundary-hiding examples."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Each finding cites exact edited-file ranges and, where applicable, the authoritative Lean/JSON evidence needed to reproduce it."
    }
  ],
  "changedFiles": [
    "docs/design/composition-options.md",
    ".pi-subagents/artifacts/outputs/73d5b88b-f2f0-4b5d-83c7-89901119abde/composition-examples/final-review.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "git status --short && git diff -- docs/design/composition-options.md && git diff --cached -- docs/design/composition-options.md",
      "result": "passed",
      "summary": "Confirmed the design note is an untracked edited file and that no staged version exists."
    },
    {
      "command": "read docs/design/composition-options.md, frontend/Sembla/Tutorial/Step05_PolicyFeedback.lean, and examples/sir_policy.json",
      "result": "passed",
      "summary": "Reviewed the complete note and compared its current-syntax and running-model claims with both requested authoritative artifacts."
    },
    {
      "command": "numbered excerpt and Python validation command for fences, internal links, staging, and JSON model fields",
      "result": "passed",
      "summary": "Found 90 balanced fence markers; verified DESIGN.md, DECISIONS.md, Step05, and sir_policy.json paths exist; found no staged files; extracted the Lean/JSON schema, threshold, and effect differences."
    }
  ],
  "validationOutput": [
    "Markdown fence markers: 90; balanced: true.",
    "Checked repository-relative links to DESIGN.md, DECISIONS.md, Step05_PolicyFeedback.lean, and examples/sir_policy.json: all targets exist.",
    "One-wire and two-wire timing at lines 529-567 is internally consistent: delivery is next-tick and the feedback round trip is two ticks.",
    "The ACSet-product distinction and structured-cospan/variable-sharing caveats at lines 190-208 and 1074-1101 are appropriately distinguished from machine product and delayed wire semantics.",
    "git diff --cached --name-only produced no output."
  ],
  "residualRisks": [
    "External URLs were assessed from the supplied prior research context; this final pass did not rerun live HTTP checks.",
    "The file is untracked, so Git cannot show a baseline diff identifying which individual lines were newly edited."
  ],
  "noStagedFiles": true,
  "diffSummary": "Reviewed the untracked 1,768-line composition design note; no project/source file was modified by this review.",
  "reviewFindings": [
    "high: docs/design/composition-options.md:248-270 - sir_policy.json is not a direct serialization of the linked Step05 running model.",
    "high: docs/design/composition-options.md:100-107,385-396,861-885,1417-1425,1640-1642 - hiding examples address an unexposed grandchild port and contradict boundary encapsulation.",
    "medium: docs/design/composition-options.md:419-436,1125-1133 - exposure direction is inconsistent.",
    "medium: docs/design/composition-options.md:1103-1135 - ACSet-like schema cannot encode its concrete exposure record.",
    "medium: docs/design/composition-options.md:665-716,1442-1455 - family match keys do not declare a join/correlation.",
    "medium: docs/design/composition-options.md:569-575,915-925 - source-only mailbox identity collides under fan-out.",
    "low: docs/design/composition-options.md:161-171 - Lean-looking factored-attribute fence lacks a local status label."
  ],
  "manualNotes": "Review gate fails pending resolution of the high-severity running-example/provenance and encapsulation issues. No tests were added because this was a review-only documentation task."
}
```
