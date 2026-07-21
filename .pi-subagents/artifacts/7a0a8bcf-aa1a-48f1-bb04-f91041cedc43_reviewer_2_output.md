## Review

- **Correct:** The note consistently states that composition syntax is not implemented merely because it appears in the document (`docs/design/composition-options.md:3-4`), accurately describes the flat current IR (`:127-151`), and already gives a useful tick-level account of delayed wiring (`:347-365`). The constrained-product and synchronized-family snippets are also locally qualified as non-frozen or future syntax (`:433-478`, `:528-544`).
- **Correct:** Repository evidence supports the current/proposed boundary. Current Lean syntax includes `wire <box> <port> -> <box> <port>` at `frontend/Sembla/DSL.lean:250`, with a real two-wire feedback model at `frontend/Sembla/Tutorial/Step05_PolicyFeedback.lean:76-77`. The current IR has flat `Box` and `Model` records and no component/composition constructor (`frontend/Sembla/IR.lean`, `Box` through `Model`, approximately lines 108-151). A repository-wide Lean search found no `sembla_component`, component-level `restrict`, `hide`, or product grammar. Consequently, every such snippet must be marked **Conceptual/proposed; not accepted by the current parser** rather than relying only on the document-wide disclaimer.
- **Blocker:** Before this is treated as a pedagogically complete design note, the abstract claims at `docs/design/composition-options.md:69-113`, `:206-277`, `:279-332`, `:404-478`, `:528-584`, `:604-639`, `:654-704`, and `:722-745` need a small number of shared concrete examples. Currently a reader must mentally synthesize equations, vocabulary, and disconnected snippets; the likely failures are confusing product with table state or row cross-product, treating a wire as synchronization, assuming exposure adds a mailbox, and treating an assertion as an invariant.
- **Note:** `plan.md` and `progress.md`, named as required inputs, were absent at the requested repository paths. This review therefore uses the document and current repository as its evidence base.

## Pedagogy gaps and likely misunderstandings

| Location | Abstract claim needing an example | Likely reader misunderstanding | Smallest remedy |
|---|---|---|---|
| §2.1, lines 69-86; §5, lines 279-323 | Product state, tagged interfaces, and lifted transitions | `A ⊗ B` pairs every transition of `A` with every transition of `B`, or same-named ports merge | Example 1: one product state and one lifted transition from each child, with two identically named but qualified ports |
| §2.2-2.4, lines 88-113 | Machine product versus factored attributes, relational cross product, and complete-model product | Two columns on a row are reusable components; or product multiplies row count; or two top-level models can already be combined | Put a four-row contrast card immediately after §2.4; use prose for unsupported relational/model operations rather than invented syntax |
| §4, lines 206-277 | Definition, instance, leaf/composite, wire, exposure, hiding, rename, scheduler domain | A definition owns live state; two instances share tables/RNG; hiding deletes state; rename changes identity | Examples 1 and 2 should show one definition instantiated twice and a before/after identity/path table |
| §5 noninterference, lines 325-345 | Projection must match standalone execution, including draw coordinates after identity work | Equality of final state is sufficient, or the current runtime already satisfies the strong law | Example 1 should show a short projection table and explicitly label the draw-coordinate row “required future behavior; currently fails with dense rule IDs” |
| §6, lines 347-387 | One-tick wire, feedback, fan-in, exact schemas | Wire means same-tick call; a two-wire loop has one total tick; two producers can target one input | Example 2 should extend the existing tick trace with the concrete Population/Policy ports and show that two wires make a two-tick round trip |
| §7.1-7.3, lines 408-478 | Local lift versus wired influence versus atomic family | Two local transitions plus a wire are equivalent to a synchronized family | Example 3 should show the same ledger transfer encoded both ways and the observable intermediate state produced only by the wire version |
| §8, lines 528-584 | Assertion versus semantic invariant; concurrent preservation | An assertion blocks a bad commit; per-transition checks imply concurrent safety | Example 3 should include one state trace and the classic capacity=1/two-winners counterexample |
| §9, lines 604-639 | Exposure is a zero-delay alias; hiding retains state; flattening preserves execution | Every nesting boundary adds a tick; hidden data disappears from semantic state; flattening may regenerate identities | Example 2 should show hierarchical and flat endpoint maps with the same mailbox ID and trace |
| §10, lines 654-704 | Semantic identity survives permitted refactors | Display paths are semantic IDs; hashes/draws necessarily change on rename or regrouping | Example 2 should show old display path, new display path, unchanged semantic tuple and draw coordinate; distinguish this future contract from current behavior |
| §11, lines 722-745 | Frozen inputs and outer synchronization for heterogeneous domains | An “exact” child exposes mid-interval events to an ODE child, or nesting can merge domains harmlessly | Example 4: a two-boundary timeline with actual times and held inputs |

## Smallest coherent example set

Use **one recurring Population/Policy system**, plus **one Ledger exception case**. Four compact example cards are enough; adding separate examples for every noun would bloat the note.

### Syntax legend (required before the first surface snippet)

Add one two-line legend near `docs/design/composition-options.md:127-151` and repeat a one-line label above each conceptual fence:

> **Current executable Lean** is limited to flat `sembla_model`, `box`, ports, and model-level `wire` declarations.  
> **Conceptual composition syntax** below is proposed design notation and is not accepted by the current parser.

Do not mark conceptual fences merely as `lean`, which suggests they compile. Prefer a caption immediately above each fence. The current flat snippet may link to `frontend/Sembla/Tutorial/Step05_PolicyFeedback.lean` as the executable source of truth.

### Example 1 — Product is parallel composition, not a row product

**Place:** after §2.4 (`docs/design/composition-options.md:107-113`), then cross-reference from §5 (`:279-332`).

A compact contrast card should contain:

1. **Factored row state (current table idea):** one `Person` row has `(health = I, custody = prison)`; it is still one row in one owned table.
2. **Relational Cartesian product (not this design):** 1,000 Person rows crossed with 50 Facility rows would produce 50,000 pairs. State explicitly that no current Sembla composition syntax performs this operation.
3. **Machine product (conceptual):** instantiate `population : Population` and `policy : Policy`; state is `(population-state, policy-state)`. If both define an output named `status`, the interface contains `population.status` and `policy.status`, not one merged `status`.
4. **Complete-model product (deferred):** two objects each owning `dt`, priors, and summaries are not operands of the initial operator.

Then show only two transition rows, avoiding a large syntax block:

```text
(pop = S, policy = Open) --population.infect--> (pop = I, policy = Open)
(pop = S, policy = Open) --policy.restrict-->  (pop = S, policy = Restricted)
```

Caption this as **Conceptual transition projection**. It makes visible that transitions lift independently rather than forming an `infect × restrict` rule pair.

Finish with a three-column projection check:

```text
observation                 Population alone     project(Population ⊗ Policy)
committed Population state  same trace            same trace
delivered outputs           same trace            same trace
draw coordinates            same after Phase 0    same after Phase 0
```

Add: “The draw-coordinate row is a target law, not current behavior; dense positional rule IDs currently violate it” (consistent with lines 334-345).

### Example 2 — One feedback system shows wire, exposure, hiding, flattening, and identity

**Place:** after the concrete tick trace in §6 (`docs/design/composition-options.md:353-365`), with cross-references from §9 (`:604-639`) and §10 (`:654-704`).

Start with a **current executable Lean excerpt**, copied exactly from the tutorial rather than inventing syntax:

```lean
wire population infection_count -> policy infection_count
wire policy restriction_modifier -> population restriction_modifier
```

Cite `frontend/Sembla/Tutorial/Step05_PolicyFeedback.lean:76-77`. Then add this value-level trace:

```text
tick n start: policy reads infection_count delivered at n-1
commit n:     population builds infection_count = 120
boundary:     120 enters policy.infection_count mailbox
tick n+1:     policy may react to 120 and builds restriction_modifier
tick n+2:     population can first react to that returned restriction
```

This makes the two-wire feedback round trip visibly two ticks, while each individual wire remains one tick.

Follow with a very small **Conceptual/proposed; not current parser syntax** wrapper:

```text
component Coupled {
  instance population = Population
  instance policy = Policy
  wire population.infection_count -> policy.infection_count
  expose population.population_state as population_state
  hide policy.policy_debug
}
```

Immediately annotate:

- `wire` owns one delayed mailbox;
- `expose` is another name for the same child endpoint and owns no mailbox;
- `hide` removes boundary addressability but retains child state and internal mailboxes;
- flattening removes `Coupled` as an execution node but retains leaf and mailbox identities.

One before/after table can also teach identity:

```text
                         before refactor              after rename/nesting
visible path             Coupled.population.infect    Region.epidemic.infect
semantic identity        <PopulationDef,pop-17,infect> <PopulationDef,pop-17,infect>
mailbox identity         <pop-17,infection_count>      <pop-17,infection_count>
draw coordinate          unchanged (future contract)  unchanged (future contract)
```

This is smaller and clearer than separate exposure, flattening, and RNG examples.

### Example 3 — Delayed messaging is not atomic transfer; assertion is not invariant

**Place:** after the synchronized-family introduction (`docs/design/composition-options.md:433-452`), with the result table repeated or cross-referenced in §8 (`:528-584`). Label all family/restrict syntax **Conceptual/proposed**.

Use one ledger state:

```text
initial: DebitLedger.exposed_total = 5
         CreditLedger.exposed_total = 5
required semantic invariant: totals are equal after every commit
```

Show the two alternatives:

```text
two local events + wire: tick n  -> (6,5), tick n+1 -> (6,6)
one synchronized family: tick n -> (6,6) atomically
```

Then state the consequences:

- An **observational assertion** reports `(6,5)` but does not prevent it.
- A **semantic invariant** rejects the wired design if equality is required at every committed boundary.
- The synchronized family has one clock, wins all required claims or none, and reports one event.
- Replacing the wire representation with a family is a model-identity change, not a stochastic refactor.

Add one two-line concurrency counterexample rather than another component:

```text
available slots at tick start = 1; release A sees 1; release B sees 1
both commits would produce -1, so a shared exclusive claim or prospective-state check is required
```

This concretely justifies lines 569-580 and prevents readers from assuming that individual preservation checks compose.

### Example 4 — Heterogeneous domains exchange only at outer boundaries

**Place:** after §11’s numbered protocol (`docs/design/composition-options.md:727-745`). No new syntax is necessary.

Reuse Population/Policy:

```text
outer interval Δ = 1.0
[0.0,1.0): exact Population may fire internally at 0.2 and 0.7;
           ODE Policy advances using the input fixed at 0.0
at 1.0:    both domains commit/exchange outputs
[1.0,2.0): Policy first uses Population's output from boundary 1.0
```

Add one sentence: wrapping either unchanged domain in another composite is structural; splitting one domain or merging the two changes numerical semantics. This exposes the key distinction without speculative scheduler declaration syntax.

## What not to add

- Do not invent current syntax for relational cross-products, merge adapters, scheduler annotations, stable IDs, or invariants; none is evidenced in the repository.
- Do not add a fifth standalone example for renaming or stochastic identity; the before/after table in Example 2 covers both.
- Do not expand the synchronized-family grammar beyond the existing candidate. The state trace teaches the semantic distinction while the design mechanics remain open.
- Do not duplicate full component bodies. Link the current tutorial and show only the two existing wire lines; use value/state tables for the rest.

## Verification checks for the final edited document

1. **Syntax provenance:** Every code fence containing `component`, `instance`, `⊗`, component-level `expose`/`hide`, `restrict`, `invariant`, or `family` has an adjacent “Conceptual/proposed; not accepted by the current parser” label.
2. **Current syntax accuracy:** Any fence labeled current is copied from or checked against `frontend/Sembla/DSL.lean`; the feedback wires exactly match `frontend/Sembla/Tutorial/Step05_PolicyFeedback.lean:76-77`.
3. **Product distinction:** A reader can answer all four cases from one card: machine product preserves row cardinalities, factored attributes stay within one row/table, relational cross product may multiply rows, and top-level model product is deferred.
4. **Transition lifting:** The example contains exactly the singleton lifts `(a,b)→(a′,b)` and `(a,b)→(a,b′)` and never implies a Cartesian enumeration of rule pairs.
5. **Tagged ports:** Two same-visible-name ports remain qualified and are never implicitly merged.
6. **Causality:** The timeline shows one tick per actual wire, zero ticks for exposure/rename, and two ticks around the two-wire Population/Policy feedback loop.
7. **Mailbox semantics:** The hierarchy/flattening table contains one unchanged mailbox identity; no exposure mailbox appears.
8. **Hiding semantics:** The text explicitly says hidden state/mailboxes remain in semantic state.
9. **Wire versus family:** The ledger trace visibly contains `(6,5)` only for delayed messaging and an atomic `(6,6)` commit for the family.
10. **Assertion versus invariant:** The assertion reports without changing selection/commit; the semantic invariant constrains initialization and commits.
11. **Concurrent closure:** The capacity=1 example demonstrates why two individually legal old-snapshot decisions can be jointly illegal.
12. **Identity honesty:** Rename/nesting keeps the conceptual semantic tuple unchanged, while the document explicitly says this strong draw law is not satisfied by current dense positional IDs.
13. **Scheduler boundary:** Internal events at 0.2/0.7 are not visible cross-domain until 1.0; changing domain partition is labeled semantic/numerical.
14. **No accidental normative syntax:** Search the final document for all composition keywords and verify each occurrence is prose, mathematical notation, current evidenced syntax, or explicitly conceptual.
15. **Link/line validation:** Run a Markdown link checker and re-run line-numbered review after editing, because inserted examples will move the references recorded here.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Reviewed only composition-options.md pedagogy and wrote the requested review artifact; no project/source file was modified."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Findings include exact current-document line ranges, repository syntax evidence, four minimal example specifications, likely misunderstandings, and 15 final-document verification checks."
    }
  ],
  "changedFiles": [
    ".pi-subagents/artifacts/outputs/7a0a8bcf-aa1a-48f1-bb04-f91041cedc43/composition-examples/pedagogy-review.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "nl -ba docs/design/composition-options.md | sed -n '1,260p'",
      "result": "passed",
      "summary": "Captured exact line references for the introduction, product distinctions, baseline, and vocabulary."
    },
    {
      "command": "git status --short && git diff --cached --name-only && nl -ba docs/design/composition-options.md | sed -n '260,760p'",
      "result": "passed",
      "summary": "Confirmed no staged files and captured exact line references through heterogeneous schedulers."
    }
  ],
  "validationOutput": [
    "Current wire grammar verified at frontend/Sembla/DSL.lean:250.",
    "Current feedback syntax verified at frontend/Sembla/Tutorial/Step05_PolicyFeedback.lean:76-77.",
    "Lean source search found no implemented reusable-component/product/hide/restrict composition grammar.",
    "Requested plan.md and progress.md were absent (ENOENT); review proceeded from repository evidence.",
    "No staged files were present at inspection time."
  ],
  "residualRisks": [
    "Document line numbers will shift when examples are inserted; final review must refresh references.",
    "Conceptual syntax cannot be parser-tested until a composition surface is selected, so explicit labeling remains mandatory.",
    "The worktree already contained unrelated modified/untracked files; this review did not alter or assess them."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added one review artifact proposing a four-card, two-domain example set; project/source files remain unchanged.",
  "reviewFindings": [
    "blocker: docs/design/composition-options.md:69-113,206-332,404-478,528-745 - major semantic distinctions remain abstract and need the proposed shared concrete examples",
    "note: conceptual composition fences must be labeled locally because only flat box/wire syntax is currently evidenced",
    "no source-code blockers or edits"
  ],
  "manualNotes": "Review-only instruction was honored. The artifact itself is the sole file written."
}
```
