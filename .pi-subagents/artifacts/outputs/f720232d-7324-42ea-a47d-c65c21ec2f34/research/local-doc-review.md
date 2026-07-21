# Code Context

## Files Retrieved

1. `docs/design/composition-options.md` (lines 1-1242) — document under review; all options, semantics, laws, sequencing, and recommendation.
2. `DESIGN.md` (lines 130-151, 216-246, 643-653) — establishes ACSet state, operadic/table-wire commitments, claimed Lean semantics, and the repository’s brief Topos/AlgebraicJulia references.
3. `DECISIONS.md` (lines 150-224, 304-333) — records ACSets, restricted ACSet rewriting, operadic macro-composition, and boundary invariance as prior decisions.
4. `docs/ROADMAP.md` (lines 129-176) — confirms flat n-box composition is the currently planned nearer-term scope and nesting is demand-driven.
5. `docs/sembla-vs-pfclbs.md` (lines 102-128) — useful internal caveat that general ACSet schema morphisms/data migrations are not implemented and composition remains immature.

## Key Code

This is an architecture-document review; no source implementation was changed.

### Option map and recommendation

- **Option A, surface-only lowering** (`composition-options.md:765-785`): smallest experiment, but loses hierarchy and makes composition frontend-specific.
- **Option B, recursive executable IR** (`787-807`): runtime-visible hierarchy at the cost of pervasive recursive semantics and migration risk; assessed highest risk.
- **Option C, free composition AST** (`809-844`): algebraic constructors and interpreters; strongest formal surface, but normalization and excessive generality are risks.
- **Option D, source IR + canonical flat plan** (`846-881`): typed source structure, single deterministic linker, flat execution; selected long-term architecture.
- **Option E, textual cloning/table merging** (`883-894`): rejected because it destroys interface and identity semantics.
- Pipeline and linker rejection criteria are explicit at `896-923`; incremental recommendation is reiterated at `1223-1242`.

### Decision criteria actually used

1. Preserve one executable semantics and the existing CPU/CUDA-shaped flat core (`19-51`, `127-180`).
2. Preserve hierarchy/provenance without requiring recursive runtime execution (`604-652`, `846-881`).
3. Make stable identities and stochastic noninterference survive refactors (`182-200`, `325-345`, `654-720`).
4. Keep wiring causal: real wires delay one tick, while exposure/rename are aliases (`347-402`).
5. Respect scheduler domains and numerical contracts (`722-761`).
6. Support formal laws, deterministic normalization, and differential/property testing (`996-1114`).
7. Minimize migration and backend disruption (`763-894`, `1116-1180`).

### Core concepts

The vocabulary at `202-277` distinguishes definition, instance, primitive/composite component, tensor, wire, exposure, hiding, renaming, scheduler domain, and constrained product. Product semantics (`279-345`) uses product state and disjoint tagged interfaces. Transition composition (`404-526`) separates lifted local events, delayed influence, optional atomic families, and claims. Constraints (`528-602`) distinguish observational assertions from semantic invariants. Flattening (`604-652`) must preserve state, identities, mailboxes, scheduler domains, and provenance.

## Architecture

The note proposes a typed composition graph that is linked once into a canonical flat runtime plan. Leaf components own state and scheduler domains; products juxtapose leaves; wires add delayed mailboxes; exposure/hiding/renaming alter boundaries without execution; optional synchronized families coordinate atomic cross-leaf changes; constraints restrict joint states. Source and identity maps reconnect runtime diagnostics and stochastic coordinates to declarations.

## Topos Institute / ACSet overlap

### Strong overlaps already present

- **Wiring-diagram/operadic composition and open systems:** `composition-options.md:238-277, 279-345, 604-652, 809-881, 996-1053`. This overlaps Topos work on wiring diagrams, polynomial functors, operads, and compositional systems. The parent design explicitly cites Spivak/Niu at `DESIGN.md:648-649`.
- **ACSets and relational state:** surprisingly, the reviewed note barely says “ACSet”; the actual commitment lives at `DESIGN.md:130-147` and `DECISIONS.md:150-172`. Topos/AlgebraicJulia work treats schemas, instances, morphisms, queries, and rewriting categorically.
- **Graph rewriting / multi-entity transitions:** synchronized families at `composition-options.md:433-495` overlap ACSet rewriting, DPO-style rule application, match enumeration, and typed graph transformations. The repository already identifies Brown et al./AlgebraicABMs as prior art (`DESIGN.md:650-652`; `DECISIONS.md:209-224`).
- **Open-system boundaries and composition:** ports, exposure, hiding, and flattening (`248-267, 347-402, 604-652`) overlap structured/decorated cospans and open-system semantics, where boundary maps are explicit and composition is pushout/gluing.
- **Algebraic laws and normalization:** `996-1114` overlaps Catlab-style executable categorical syntax, normal forms, functorial interpretations, and property tests for categorical equations.
- **Constraints/schema views:** `528-602` overlaps functorial data migration, conjunctive queries, sketches, and schema-level constraints, although the note frames constraints operationally rather than categorically.

### Material omissions

1. **No explicit choice of categorical composition formalism.** Option C says “free composition AST” (`809-844`), while the rest alternates among operad, tensor, feedback/trace, aliases, and restriction. These are not automatically one algebraic theory. The note should compare wiring-diagram operads, polynomial interfaces/lenses, and structured cospans, then state which construction gives the source IR and which gives semantics.
2. **ACSet state is disconnected from composition.** Machine state is written abstractly as `State(A) × State(B)` (`283-290`), but no account is given of schemas, ACSet instances, or morphisms. A categorical product of ACSets can pair object populations and is generally not the desired “two independently owned databases”; disjoint juxtaposition is more naturally schema/instance coproduct (or a product in an explicitly chosen category of machines). This distinction is essential given the warning against relational Cartesian product at `64-105`.
3. **No schema morphisms or functorial data migration.** Exact positional schema equality and explicit adapters (`383-390`) are a reasonable first runtime restriction, but the document omits the established ACSet account of schema maps and their pullback/left/right data migrations (Δ/Σ/Π). Those provide semantics for rename, projection, extension, and aggregation adapters rather than treating each as an ad hoc leaf box.
4. **No structured-cospan account of boundaries.** Exposure/hiding are described informally as aliases (`248-267, 609-613`). Structured cospans could make interfaces and gluing precise, prove associativity up to canonical isomorphism, and clarify when pushout composition identifies versus merely connects ports.
5. **No ACSet-native representation of the source graph.** The proposed frontend-agnostic graph (`896-923`) could itself be a typed ACSet: definitions, instances, ports, wires, exposure maps, scheduler domains, and provenance as objects/arrows/attributes. That would reuse Catlab-style validation, visualization, migration, and canonical serialization concepts.
6. **Rewriting is mentioned only indirectly.** Atomic families (`433-495`) recreate matching, ownership, gluing, and atomic application concerns familiar from DPO rewriting. The note should explicitly say which DPO/SqPO conditions are adopted or rejected, and how its bounded keyed-join fragment relates to AlgebraicRewriting/AlgebraicABMs.
7. **No model transformation/comparison layer.** Topos compositional-modeling work emphasizes maps between models, translation across formalisms, and functorial semantics—not only composing components. Source-to-plan linking is itself such an interpretation, but `1182-1221` leaves its semantic status open rather than defining a functor/meaning-preservation obligation.
8. **Weak bibliography.** The reviewed document contains no references section and relies on inline “kimi” review callouts. It should directly cite Topos/AlgebraicJulia work on ACSets, Catlab, AlgebraicRewriting/AlgebraicABMs, polynomial functors, wiring diagrams, and structured cospans/open systems.

### Misleading, overstrong, or potentially dated claims

- **“Product state” ambiguity (`283-290`).** Correct for machine semantics, but hazardous in an ACSet project: readers may infer categorical ACSet product or relational product. Specify independent machine product as a pair of owned ACSet instances, likely formed via tagged/disjoint schema-instance juxtaposition, not pointwise ACSet product.
- **“Exact ordered-schema equality” (`383-390`).** Accurate as an initial implementation policy, but misleading if presented as the natural typed semantics. ACSet schema morphisms offer principled compatibility and migration; label this a backend/linker restriction, not the categorical endpoint.
- **“Free composition AST” constructors (`809-821`).** Listing `Feedback/Trace` beside ordinary wiring suggests an unrestricted traced monoidal structure. A one-tick delayed feedback operator is guarded/stateful, not ordinary instantaneous trace. Its equations must be stated separately.
- **“Operad commitment” is underspecified (`54-61` callout; inherited from `DESIGN.md:216-243`).** Operads describe substitution shapes but do not alone supply delayed mailbox state, stochastic identity, scheduler coupling, or invariants. The document generally handles these correctly operationally, but should stop implying that “the operad” settles them.
- **Boundary invariance is narrower than prior repository wording.** The note responsibly scopes it by scheduler and identity (`47-51`, `722-761`), whereas `DECISIONS.md:323-332` claims moving boundaries “never” changes observable semantics. The new qualification should be promoted into the normative decision record.
- **SoA “is the ACSet” in supporting docs (`DECISIONS.md:169-172`, `DESIGN.md:137-139`) is overstrong.** SoA is one faithful representation of finite ACSet data, not the categorical object itself; layout/order and categorical semantics should not be equated. This matters directly to canonicalization and alpha/order invariance (`688-703`).
- **General graph rewriting performance claim is unsupported and likely dated (`DECISIONS.md:211-216`, relevant to families).** “Nothing in that line runs at 26M agents” is an absolute empirical claim without benchmark, hardware, version, or date. Retain the restricted-kernel design rationale but replace the absolute with measured scope/cost evidence.
- **Document dating/status risk (`composition-options.md:3-4`).** It is dated 2026-07-21 and embeds named reviewer callouts throughout. If those callouts are not normative, separate them into review history; otherwise future readers cannot distinguish selected architecture from commentary.

### Concrete improvements using Topos work

1. Add a section after the vocabulary (`after 202-277`) defining three categorical levels: ACSet schema/instance for leaf state; a chosen open-system construction (preferably structured cospans or a precisely named wiring-diagram operad) for interfaces; and a functor/interpreter to delayed stochastic machines.
2. Rewrite `State(A ⊗ B)` (`283-290`) to say “pair/tagged disjoint union of independently owned ACSet instances,” and explicitly contrast machine product, ACSet pointwise product, ACSet coproduct, and relational Cartesian product.
3. Refine schema compatibility (`383-390`) with two layers: v1 equality restriction; future adapters denoted by ACSet schema morphisms and functorial data migration. State which migrations preserve keys, attributes, and finite-table execution.
4. Model the composition-source artifact (`896-923`) as, or against, a versioned ACSet schema. This gives explicit foreign keys from wires to ports, instances to definitions, exposure maps to boundaries, and provenance to source declarations.
5. Recast synchronized families (`433-495`) as a deliberately restricted ACSet rewrite language compiled to declared-key joins. Document match, dangling/identification, application, and concurrency conditions; cite DPO/SqPO prior art while retaining the GPU-safe bounded fragment.
6. Give exposure/wiring a structured-cospan semantics (`248-267, 347-402, 604-652`). Distinguish structural gluing from delayed channel insertion; only the latter allocates mailbox state. This makes the zero-delay/one-tick distinction mathematical rather than an implementation convention.
7. Define the linker (`896-923`) as a semantics-preserving interpretation/functor and make preservation laws explicit: identities/composition, boundary maps, ACSet ownership, rewrite semantics, mailbox delay, and stochastic coordinates.
8. Add executable examples in Catlab/AlgebraicJulia notation alongside Lean sketches (`925-994`) to test whether the proposed source schema actually captures nested components and migrations without inventing parallel concepts.
9. Add direct references and versioned benchmark evidence. Topos work should be used as formal prior art, not merely name-checked through the parent design.

## Start Here

Open `docs/design/composition-options.md` lines **279-345** first. The unresolved meaning of product state in an ACSet-based system is the most consequential conceptual gap; resolving it determines the source schema, linker, laws, and whether Topos categorical machinery genuinely strengthens Option D.

## Review Gate

**Finding: changes requested (architecture-document quality), no implementation blocker.** Option D remains the strongest choice, but the document should not be treated as architecture-complete until it specifies how ACSet schemas/instances and morphisms relate to machine product, boundaries, adapters, and the linker. The guarded-delay semantics must also be distinguished from ordinary categorical trace.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Performed only the requested read-only architecture review and wrote the required review artifact; no project/source file was modified."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Report maps all five options, decision criteria, recommendation, exact line ranges, Topos/ACSet overlaps, omissions, misleading claims, and concrete improvements."
    }
  ],
  "changedFiles": [
    ".pi-subagents/artifacts/outputs/f720232d-7324-42ea-a47d-c65c21ec2f34/research/local-doc-review.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "read docs/design/composition-options.md",
      "result": "passed",
      "summary": "Reviewed all 1,242 lines."
    },
    {
      "command": "grep repository for ACSet, Topos, composition, open-system, and structured-cospan references",
      "result": "passed",
      "summary": "Located supporting decisions, design commitments, roadmap, and comparison documentation."
    },
    {
      "command": "nl -ba docs/design/composition-options.md; selected ranges from DESIGN.md and DECISIONS.md",
      "result": "passed",
      "summary": "Verified exact section and supporting-document line ranges."
    },
    {
      "command": "git status --short && wc -l docs/design/composition-options.md",
      "result": "passed",
      "summary": "Confirmed 1,242-line target and observed only pre-existing/unrelated working-tree changes before writing the artifact."
    }
  ],
  "validationOutput": [
    "Review artifact written to the authoritative output path.",
    "No project/source files modified by this review."
  ],
  "residualRisks": [
    "Topos research is assessed from established conceptual overlap and repository references; no live external literature/version verification was available.",
    "Working tree already contained modified and untracked orchestration files, so repository cleanliness cannot be claimed globally."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added one review artifact only; source architecture document remains unchanged.",
  "reviewFindings": [
    "major: docs/design/composition-options.md:283-290 - machine product is not related precisely to ACSet product/coproduct, risking the wrong state semantics.",
    "major: docs/design/composition-options.md:809-881 - categorical source formalism is unspecified despite mixing operads, tensor, trace, hiding, and restriction.",
    "major: docs/design/composition-options.md:383-390 - exact positional equality omits schema morphisms and functorial data migration as the principled adapter semantics.",
    "major: docs/design/composition-options.md:433-495 - synchronized families duplicate ACSet rewriting concerns without connecting to DPO/SqPO prior art or conditions.",
    "minor: docs/design/composition-options.md:3-4 - named inline review callouts blur normative design and review history."
  ],
  "manualNotes": "Review gate result: changes requested to strengthen the architecture document; Option D recommendation itself remains sound."
}
```
