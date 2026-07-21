# Research: Current Topos Institute contributions relevant to compositional modelling and attributed C-sets

## Summary

Topos and the closely associated AlgebraicJulia ecosystem provide mature, usable foundations for **ACSet data representation**, categorical (co)limits, structured cospans, and recursively nested wiring-diagram syntax; they also provide working, domain-specific composition in AlgebraicDynamics, StockFlow, and—most recently—CatColab. These sources strongly support `composition-options.md`’s separation of composition syntax from execution semantics and its hybrid “source graph → canonical executable” direction, but they do **not** supply an off-the-shelf semantics for Sembla’s delayed table wires, stochastic identity, atomic cross-component events, constrained products, or heterogeneous schedulers.

The most important current development is CatColab’s progression from experimental plans for interoperability (2024–2025) to actual model instantiation and variable-sharing composition across its supported logics (2026), while its Julia/AlgebraicJulia backend remains explicitly proof-of-concept. Confidence is high for the claims below where backed by papers, maintained docs, release posts, and repositories; confidence is lower when extrapolating those results to Sembla’s scheduler and reproducibility requirements.

## Findings

1. **ACSets are a mature practical data/IR substrate, not themselves a machine-composition semantics.** — The original paper (arXiv posted 8 June 2021; journal publication 28 December 2022) defines attributed C-sets as functors/data structures extending C-sets with fixed-type attributes and presents an efficient in-memory implementation for categorical databases, including constructions needed by structured/decorated cospans. The maintained `ACSets.jl` docs describe ACSets as generalizing graphs and data frames, with schema/data structures and serialization; Catlab adds homomorphisms, limits, colimits, and related categorical algorithms. The repository release listing shows `ACSets.jl` v0.2.27 published 10 February 2026, evidence of continued maintenance. **Implication for the design note:** ACSets are a credible representation for typed model graphs, ports, wires, provenance maps, and source schemas, but adopting them would not determine one-tick causality, scheduler boundaries, RNG identity, or execution. **Maturity: mature/practical. Confidence: high.** [Paper](https://doi.org/10.32408/compositionality-4-5) [arXiv](https://arxiv.org/abs/2106.04703) [ACSets.jl docs](https://algebraicjulia.github.io/ACSets.jl/stable/) [releases](https://github.com/AlgebraicJulia/ACSets.jl/releases)

2. **Topos explicitly warns that categorical “product” of attributed data is not a neutral machine product.** — In “Acsets with variables” (20 June 2023), Owen Lynch explains that products in the relevant coslice can pair/change attribute types, while products in the slice retain only items with identical attribute values; the implemented “faux product” abstracts constants to variables, takes a product, and then requires an application-specific evaluation. **Implication:** this strongly supports §§2 and 8 of `composition-options.md`: `A ⊗ B` for reusable machines must not be confused with a relational/ACSet product, and attribute reconciliation/adapters must be explicit. The same post says variable-equipped ACSets were implemented in AlgebraicRewriting for attributed rewriting, but describes broader data migration uses as future directions. **Maturity: core ACSets and categorical operations practical; varACSet product-like constructions specialized/less mature. Confidence: high.** [Topos post](https://topos.institute/blog/2023-06-20-acsets-with-variables/)

3. **Catlab’s wiring diagrams already realize a reusable, recursively nested composition-source representation.** — Current Catlab docs define directed wiring diagrams as boxes with typed input/output ports and wires; boxes may be atomic or themselves wiring diagrams. The high-level API supports categorical composition and monoidal product, while the wiring diagrams themselves can be represented as attributed C-sets and can carry arbitrary data on boxes, ports, and wires. Catlab also provides Julia DSLs (`@program` for directed and related macros for other diagram types). **Implication:** this is the closest maintained precedent for the note’s free/hybrid source IR, nesting, tensor, wiring, and flattening story. It supports using an ACSet-like graph as syntax and interpreting/lowering it separately. It does not establish Sembla’s proposed stable IDs, canonical serialization, exact one-tick mailbox semantics, or source-map preservation. **Maturity: mature/practical library representation and manipulation; Sembla-specific linker properties unproven. Confidence: high.** [Catlab wiring API](https://algebraicjulia.github.io/Catlab.jl/stable/apis/wiring_diagrams/) [Wiring diagrams as attributed C-sets](https://algebraicjulia.github.io/Catlab.jl/stable/generated/wiring_diagrams/wd_cset/) [Catlab](https://algebraicjulia.github.io/Catlab.jl/stable/)

4. **Operads separate composition syntax from system semantics and have a working dynamical-systems implementation.** — Libkind, Baas, Patterson, and Fairbanks, “Operadic Modeling of Dynamical Systems” (arXiv 26 May 2021), reformulates directed and undirected wiring diagrams as operads and gives algebras for deterministic discrete and continuous dynamical systems, enabling hierarchical composition. `AlgebraicDynamics.jl` implements this via `oapply`: a wiring diagram specifies the composition pattern and compatible primitive machines fill its boxes to produce a composite machine. **Implication:** this directly supports the design note’s “typed composition-source representation + interpreter/linker” and its insistence that composition syntax not dictate a single semantics. However, this work concerns deterministic machine/dynamical-system algebras, not Sembla’s tau-leaped stochastic transitions, atomic multi-component families, or Philox coordinate stability. **Maturity: practical for supported deterministic dynamics; extension to Sembla semantics is new work. Confidence: high.** [Paper](https://arxiv.org/abs/2105.12282) [AlgebraicDynamics docs](https://algebraicjulia.github.io/AlgebraicDynamics.jl/stable/) [API](https://algebraicjulia.github.io/AlgebraicDynamics.jl/stable/api/)

5. **Topos’s 2024 Mealy-machine work highlights that feedback safety depends on semantics, not wiring syntax alone.** — “Wiring diagrams for Mealy machines” (19 August 2024) extends directed wiring diagrams with input→output dependency information using a Grothendieck construction, then restricts to an acyclic wide subcategory because instantaneous Mealy dependencies can create unsolvable algebraic loops. The post calls this work a setup for an operad algebra rather than reporting a maintained production implementation. **Implication:** this supports the note’s distinction between zero-delay exposure/aliasing and delayed wires, and its requirement to detect combinational cycles if zero-delay adapters are ever introduced. Sembla’s existing one-tick delay is a valid alternative that prevents this particular loop, but the Topos work does not imply every categorical feedback operator is delayed. **Maturity: research/prototype; not established production software. Confidence: high.** [Topos post](https://topos.institute/blog/2024-08-19-wiring-diagrams-mealy-machines/)

6. **Structured cospans are the strongest established formalism here for open systems composed by boundary gluing.** — Patterson’s Topos post (15 March 2023; updated 3 April 2023) and accompanying ACT 2023 paper show that, under mild finite-(co)limit and preservation hypotheses, structured-cospan double categories are cocartesian equipments. Horizontal composition is by pushout, coproduct supplies parallel composition, and a generic implementation exists in Catlab. Decorated cospans similarly turn closed systems into open systems and, in their refined form, live naturally in double categories. **Implication:** these provide rigorous laws for interfaces, parallel composition, and gluing, and justify considering structured cospans where Sembla components expose boundary data. They do **not** directly model directional, one-tick table transport: pushout/variable sharing identifies boundary structure, whereas Sembla wires are stateful directed channels. Therefore structured cospans should inform the source algebra and laws, not be claimed as the runtime semantics without a dedicated interpretation. **Maturity: mathematical framework mature; Catlab primitives practical; mapping to Sembla delayed channels speculative. Confidence: high.** [Topos post](https://topos.institute/blog/2023-03-15-structured-cospans-cocartesian-equipment/) [paper](https://arxiv.org/abs/2304.00447) [Catlab categorical algebra API](https://algebraicjulia.github.io/Catlab.jl/stable/apis/categorical_algebra/)

7. **Double categories organize several dimensions of open-system composition, but do not force a recursive executable IR.** — Patterson’s structured/decorated-cospan work uses objects/arrows for boundary maps, proarrows for open systems, and cells for maps of open systems, with external composition by pushout. Topos’s 2022 Grothendieck/decorated-cospan posts and later double-category papers deepen this unification. **Implication:** the theory supports the note’s need to distinguish components, interfaces, open-system composition, and transformations between models. Nothing in these sources requires runtime execution to preserve recursive hierarchy; a double-categorical source model interpreted into a flat plan is compatible with the theory. Treating double categories as a ready-made scheduler or provenance implementation would overclaim. **Maturity: mature theory; software coverage partial; runtime architecture application speculative. Confidence: high.** [Decorated cospans post](https://topos.institute/blog/2022-05-30-decorated-cospans-via-grothendieck/) [double Grothendieck post](https://topos.institute/blog/2022-05-23-double-grothendieck/) [2023 paper](https://arxiv.org/abs/2304.00447)

8. **AlgebraicJulia demonstrates domain-specific open-model composition, especially variable sharing, but not one universal notion of wiring.** — `StockFlow.jl` and related papers use decorated/structured-cospan ideas to compose stock-flow models; “Compositional Modeling with Stock and Flow Diagrams” (arXiv 17 May 2022) explicitly composes diagrams into larger ones along interfaces. AlgebraicDynamics supports both directed machine composition and undirected “resource sharing.” These are materially different semantics. **Implication:** `composition-options.md` is right to define product, wiring, exposure, and constraints separately rather than selecting “categorical composition” generically. Variable-sharing/gluing is a useful precedent for constrained/shared-resource models but is not equivalent to delayed messages or atomic synchronized transition families. **Maturity: practical within domain packages; general cross-domain semantics limited. Confidence: high.** [Stock-flow paper](https://arxiv.org/abs/2205.08373) [AlgebraicDynamics docs](https://algebraicjulia.github.io/AlgebraicDynamics.jl/stable/)

9. **CatColab has moved model instantiation and composition from roadmap to usable alpha features through June 2026.** — CatColab v0.4 “Robin” (8 January 2026) introduced `Instantiate` cells for reusing a model and binding exposed variables, initially only for discrete theories; the post says this is based on Owen Lynch’s `DoubleTT`. v0.5 (23 March 2026) added Petri-net composition by sharing places. v0.6 “Starling” (1 June 2026) added stock-flow composition by sharing stocks, so all then-supported CatColab logics supported the variable-sharing paradigm, plus an undirected-wiring-diagram composition-pattern visualization and first-class composable polynomial ODE models. **Implication:** reusable definitions, instances, qualified members, variable binding, and composition visualization are current practical precedents for Sembla’s source graph and tooling. But CatColab remains described as alpha/experimental, and its principal composition paradigm is sharing/identification, not delayed typed channels. **Maturity: usable alpha; semantics practical for supported logics, not production-hardened. Confidence: high.** [v0.4](https://topos.institute/blog/2026-01-08-catcolab-0-4-robin/) [v0.5](https://topos.institute/blog/2026-03-23-catcolab-0-5-sandpiper/) [v0.6](https://topos.institute/blog/2026-06-01-catcolab-0-6-starling/)

10. **Current interoperability is promising but incomplete.** — The CatColab launch post (2 October 2024) framed migration among logics as a future “universal translator.” InterTypes (14 November 2023) pursued language-neutral type descriptions because scientific models should be inspectable data, transferable between languages, and storable in databases, with native ACSet support as a goal. By CatColab v0.6, a Julia compute service existed, but Topos explicitly called its only use a **proof of concept** converting a schema diagram to a tabular instance through Catlab; Petrinaut JSON import was marked experimental. **Implication:** embedding source identity/provenance in a frontend-neutral plan is aligned with Topos’s direction, but claiming general model interoperability today would be premature. Explicit schema versions and adapters remain necessary. **Maturity: InterTypes and cross-tool migration experimental; Julia service proof-of-concept. Confidence: high.** [InterTypes](https://topos.institute/blog/2023-11-14-introducing-intertypes/) [CatColab launch](https://topos.institute/blog/2024-10-02-introducing-catcolab/) [CatColab v0.6](https://topos.institute/blog/2026-06-01-catcolab-0-6-starling/)

11. **Topos’s “compositional world-modeling” is explicitly a research program, not a settled implementation blueprint.** — The 15 June 2023 post asks for a formal framework covering openness, multiple disciplines, continuous and stochastic time, nondeterminism, partiality, and hybrid systems. It states that different classes will likely need different definitions connected by formal translations, and labels a single “big tent” doctrine enabling nontrivial cross-type composition as speculative. **Implication:** this supports Sembla’s restricted, demand-driven constructors and explicit scheduler domains. It does not validate generalized stochastic/hybrid composition yet; the design note should cite this as motivation and a warning against overgeneralization, not as evidence that heterogeneous scheduler composition has been solved. **Maturity: speculative research program. Confidence: high.** [Topos post](https://topos.institute/blog/2023-06-15-compositional-world-modeling/)

12. **No reviewed Topos source found solves Sembla’s hardest semantic obligations.** — Across the sources reviewed, there is no direct treatment of composition-stable counter-based RNG identities, declaration-order-independent stochastic traces, atomic cross-component tau-leap families, prospective-state invariant closure under concurrent commits, or CPU/CUDA canonical-plan equivalence. These remain original design/engineering obligations. The closest related work supplies algebraic interfaces and composition laws, not those operational guarantees. **Implication:** §§7–11 and Phase 0 of the note should remain explicit prerequisites; citing category theory cannot discharge them. **Maturity: speculative/unimplemented for Sembla. Confidence: medium-high (absence claim limited to reviewed sources).**

## Direct evaluation of `composition-options.md`

### Strongly supported

- **Option D / hybrid source IR plus flat executable plan.** Catlab wiring diagrams-as-ACSets and operad algebras are direct precedents for representing composition syntax independently and interpreting it into semantics.
- **Typed reusable components, instances, tensor, nesting, and source maps.** Catlab and CatColab demonstrate these authoring concepts in maintained software; CatColab’s qualified instantiated names are especially relevant.
- **Separate product from data/relational product.** The varACSet product discussion makes this separation essential rather than terminological nicety.
- **Differentiate directed wiring from sharing/gluing.** AlgebraicDynamics and stock-flow/CatColab show that directed flow and undirected resource/variable sharing are genuinely different composition theories.
- **Delay-free exposure is structural, while feedback semantics require an interpretation.** Mealy-machine dependency loops show why an outer-box alias cannot casually be treated as a delayed wire, and why any zero-delay path needs cycle analysis.
- **Keep constructors demand-driven.** Topos’s own world-modeling program says no settled universal framework presently covers all desired system classes.

### Needs qualification or additional design work

- **“Operad commitment.”** Operads provide excellent syntax and algebraic laws, but an operad algebra must still define Sembla’s mailbox, transition, conflict, and RNG semantics. “Operadic” alone is not an execution contract.
- **Structured/decorated cospans.** Best suited to open systems composed by gluing/shared boundaries. Sembla’s directed, delayed finite-table channels require a distinct interpretation; do not identify pushout composition with message delivery.
- **Flattening and canonical byte identity.** Catlab supports nesting and substitution, but reviewed docs do not guarantee stable-ID-sorted canonical serialization or byte-identical plans under reassociation/symmetry.
- **Constrained products/invariants.** ACSet schemas and categorical limits can express structural constraints, but concurrent stochastic invariant preservation and no-rollback runtime behavior are not supplied.
- **Heterogeneous schedulers and cross-component atomic families.** Current Topos work motivates these areas but does not make them mature. Keep deferred phases and explicit scheduler-domain restrictions.
- **Model interoperability.** CatColab’s current composition is real, but cross-logic/tool execution remains partial; the Julia backend is explicitly proof-of-concept.

## Maturity matrix

| Area | Current evidence | Assessment for Sembla |
|---|---|---|
| ACSets as typed graph/database IR | Paper, maintained ACSets.jl/Catlab, active releases | **Mature/practical** representation substrate |
| Catlab nested directed wiring diagrams | Maintained docs and DSL/API | **Mature/practical** source syntax and manipulation |
| Operadic deterministic system composition | Reviewed paper + AlgebraicDynamics implementation | **Practical in supported domains**; new stochastic algebra needed |
| Structured/decorated cospans | Established literature + Catlab generic API | **Mature theory; practical primitives**; delayed-wire mapping not direct |
| Stock-flow/Petri-net variable-sharing composition | Packages/papers + CatColab releases | **Practical domain-specific composition** |
| CatColab reusable model instantiation | v0.4–v0.6 alpha releases | **Usable alpha**, evolving |
| Cross-language/cross-logic interoperability | InterTypes, CatColab roadmap, Julia POC | **Experimental** |
| Dependent wiring diagrams for Mealy machines | 2024 research post | **Research/prototype** |
| Universal stochastic/hybrid world-modeling | Explicit conjectures/research program | **Speculative** |
| Stable stochastic identity, atomic families, concurrent invariants | No direct reviewed solution found | **Unsolved/new engineering and semantics** |

## Sources

### Kept

- [Patterson, Lynch, Fairbanks, “Categorical Data Structures for Technical Computing”](https://doi.org/10.32408/compositionality-4-5) — primary ACSet definition and implementation claims; published 28 December 2022.
- [ACSets.jl stable documentation](https://algebraicjulia.github.io/ACSets.jl/stable/) and [releases](https://github.com/AlgebraicJulia/ACSets.jl/releases) — maintained software scope and recency.
- [Catlab stable documentation](https://algebraicjulia.github.io/Catlab.jl/stable/) — maintained categorical and wiring-diagram APIs.
- [Wiring Diagrams as Attributed C-Sets](https://algebraicjulia.github.io/Catlab.jl/stable/generated/wiring_diagrams/wd_cset/) — direct evidence that composition syntax can itself be an ACSet.
- [Libkind et al., “Operadic Modeling of Dynamical Systems”](https://arxiv.org/abs/2105.12282) and [AlgebraicDynamics.jl](https://algebraicjulia.github.io/AlgebraicDynamics.jl/stable/) — primary theory/software pairing for hierarchical composition.
- [Patterson, “Structured and Decorated Cospans…”](https://arxiv.org/abs/2304.00447) and [Topos explainer](https://topos.institute/blog/2023-03-15-structured-cospans-cocartesian-equipment/) — primary double-categorical open-systems result and Catlab implementation link.
- [Topos, “Wiring diagrams for Mealy machines”](https://topos.institute/blog/2024-08-19-wiring-diagrams-mealy-machines/) — precise treatment of instantaneous dependencies and algebraic loops.
- [Topos, “Acsets with variables”](https://topos.institute/blog/2023-06-20-acsets-with-variables/) — precise warning about products and attributed data.
- [CatColab v0.4](https://topos.institute/blog/2026-01-08-catcolab-0-4-robin/) and [v0.6](https://topos.institute/blog/2026-06-01-catcolab-0-6-starling/) — current primary release evidence for instantiation, composition, analyses, and Julia POC.
- [Topos, “Introducing InterTypes”](https://topos.institute/blog/2023-11-14-introducing-intertypes/) — primary statement of cross-language model-data aims and ACSet support.
- [Topos, “Towards a Research Program on Compositional World-Modeling”](https://topos.institute/blog/2023-06-15-compositional-world-modeling/) — authoritative scope and explicit speculation boundaries.
- [“Compositional Modeling with Stock and Flow Diagrams”](https://arxiv.org/abs/2205.08373) — domain-specific cospan-based composition evidence.

### Dropped

- Generic category-theory explainers and nLab pages — useful background but not Topos primary evidence.
- Search-result-only summaries of future/current releases — excluded where the underlying Topos post or GitHub release page was available.
- SEO aggregators, ResearchGate, and citation-index copies of ACSet papers — redundant to DOI/arXiv and official docs.
- Unrelated neural wiring-diagram and Bayesian-update posts — mathematically adjacent but insufficiently direct for Sembla’s composition decision.
- 2025 “Compositional System Dynamics” search result — not needed once primary StockFlow and CatColab sources established the relevant claims.

## Gaps

- I did not find a Topos benchmark comparing Catlab/ACSet composition/linking performance or demonstrating scale comparable to Sembla’s anticipated workloads.
- Catlab’s docs establish recursive nesting and composition, but not the exact normalization/canonicalization guarantees proposed in the design note.
- The reviewed sources do not establish a production-ready cross-language interchange standard for ACSets plus executable semantics; InterTypes/CatColab remain evolving.
- No source directly addresses stochastic trace identity under hierarchy refactors, synchronized tau-leap events, heterogeneous solver co-simulation error, or invariant preservation under concurrent commits.
- A later acceptance review should verify the exact latest Catlab release after v0.17.5 and inspect CatColab’s current RFCs/DoubleTT implementation if implementation-level reuse is being considered; these details do not change the architectural conclusions above.

## Acceptance evidence

- Requested scope was research-only. No project source or documentation file was modified.
- The only written artifact is this mandated research output outside the project’s tracked design files.
- Validation consisted of reading `docs/design/composition-options.md`, running four-angle web searches, fetching the most relevant Topos/AlgebraicJulia primary sources, and cross-checking mature software claims against maintained docs/release listings.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Produced only the requested Topos/AlgebraicJulia research evaluation; no project files were modified."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Findings include dates, precise claims, direct URLs, confidence, maturity assessments, design-note implications, source triage, and explicit gaps."
    }
  ],
  "changedFiles": [
    "/Users/ian/projects/sembla/.pi-subagents/artifacts/outputs/f720232d-7324-42ea-a47d-c65c21ec2f34/research/topos-current.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "read docs/design/composition-options.md",
      "result": "passed",
      "summary": "Reviewed the full design note to target research findings and identify unsupported semantic obligations."
    },
    {
      "command": "web searches across Topos, AlgebraicJulia docs, arXiv, and GitHub release sources",
      "result": "passed",
      "summary": "Covered ACSets/Catlab, cospans/double categories, operads/wiring diagrams/open systems, interoperability, and recent releases."
    },
    {
      "command": "fetch and inspect selected primary Topos and maintained AlgebraicJulia sources",
      "result": "passed",
      "summary": "Cross-checked exact claims and maturity labels against full source content rather than search snippets alone."
    }
  ],
  "validationOutput": [
    "Artifact contains 12 sourced findings, a direct design evaluation, maturity matrix, kept/dropped source audit, and gaps.",
    "No tests were applicable because the task was research-only and explicitly prohibited project-file modifications."
  ],
  "residualRisks": [
    "Absence claims are limited to the reviewed primary-source corpus.",
    "No shell tool was available to independently run git status; no staging command was issued and only the mandated artifact was written.",
    "CatColab and AlgebraicJulia are actively evolving, so exact latest patch versions should be rechecked when implementation begins."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added the mandated research artifact only; project files remain unchanged.",
  "reviewFindings": [
    "no blockers",
    "review required: independently confirm that structured-cospan variable sharing is not presented as equivalent to Sembla delayed-wire semantics",
    "review required: verify recent CatColab release dates and alpha/POC qualifiers before quoting externally"
  ],
  "manualNotes": "The strongest actionable conclusion is to retain the hybrid source-IR/linker direction while treating RNG identity, delayed transport, atomic families, invariants, and heterogeneous schedulers as Sembla-specific obligations rather than solved consequences of Catlab or category theory."
}
```
