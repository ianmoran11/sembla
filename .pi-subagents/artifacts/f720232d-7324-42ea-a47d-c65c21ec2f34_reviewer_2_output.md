## Review

### Overall assessment

- **Blocker:** before `docs/design/composition-options.md` is used as a framework-selection or architecture-lineage note, it needs a compact related-work/evaluation section for the Topos Institute–associated AlgebraicJulia ecosystem. The file contains no occurrence of `Topos`, `Catlab`, `AlgebraicJulia`, `ACSet`, `C-set`, `structured cospan`, `open system`, or `wiring diagram`. Its sole mention of operads is a reviewer callout saying the recommendation agrees with another local document (`docs/design/composition-options.md:53-62`), not an explanation or comparison. This is a material omission because the recommended source-graph-plus-interpreter design closely overlaps established work in that ecosystem.
- **Correct:** the omission does not make the selected hybrid architecture wrong. The split between a typed composition source, deterministic linking, and a flat runtime (`docs/design/composition-options.md:34-45`, `846-867`) is broadly compatible with the AlgebraicJulia/Catlab separation between categorical syntax/data and an interpretation. The local document also correctly makes Sembla-specific execution details—delay, stochastic identity, scheduler domains, and deterministic flattening—first-class rather than assuming generic category-theory machinery supplies them.

### Evidence-backed missing contributions

1. **The design independently arrives at compositional-modelling ideas without acknowledging the closest working ecosystem.**
   - Local evidence: the component/instance/composite/tensor/interface vocabulary at `docs/design/composition-options.md:202-277`, and the free composition AST and hybrid source graph at `docs/design/composition-options.md:809-867`, are presented without related work.
   - External evidence: AlgebraicJulia describes its goal as “bringing compositionality to technical computing” and says it creates scientific-computing approaches based on applied category theory; its catalog includes Catlab, AlgebraicDynamics, and compositional scientific-model case studies. Topos Institute's own output catalog lists Catlab as software.
   - Sources: https://www.algebraicjulia.org/ and https://topos.institute/work/output/software/catlab/
   - Finding: add a short “Prior art and points of departure” subsection. It should characterize this as a **Topos-associated/currently maintained ecosystem**, not claim every underlying result originated at Topos.

2. **ACSets/C-sets are missing as a concrete candidate for the composition-source representation.**
   - Local evidence: Option C proposes an abstract constructor list (`Primitive`, `Tensor`, `Wire`, etc.) at `docs/design/composition-options.md:809-844`; Option D chooses a “typed, frontend-agnostic composition-source graph/AST” at `846-850`; and the pipeline speaks only of a generic graph at `896-923`. None evaluates how that graph is represented.
   - External evidence: Catlab documents wiring diagrams as attributed C-sets and provides data structures, algorithms, and JSON/GraphML serialization for categorical structures. The C-set literature treats C-sets as schema-indexed data encompassing graphs and relational data; ACSets add attributes suitable for labels, types, parameters, and provenance.
   - Sources: https://algebraicjulia.github.io/Catlab.jl/stable/ ; https://algebraicjulia.github.io/ACSets.jl/stable/ ; https://doi.org/10.1007/s10485-022-09683-x
   - Finding: evaluate an ACSet-like schema as (a) the source-graph data model, (b) an interchange/reference format, or (c) explicitly rejected prior art. This is especially relevant to typed ports, instances, wires, source maps, and stable IDs. It need not imply a Julia runtime dependency or replacement of Sembla's existing table IR.

3. **Wiring diagrams and operad algebras are named only indirectly, despite matching the proposed syntax/semantics split.**
   - Local evidence: product and interface operations are specified directly at `docs/design/composition-options.md:243-262`; wiring is then assigned bespoke delayed semantics at `347-387`; Option C proposes interpretation of composition constructors at `809-832`. The only word “operad” is in `53-62`.
   - External evidence: AlgebraicDynamics states that it supplies compositional and hierarchical dynamical systems and that composition follows operads and operad algebras. It supports undirected wiring diagrams, directed wiring diagrams, and open circular port graphs as composition syntaxes, with an operad algebra interpreting a composition pattern together with primitive systems into a composite.
   - Sources: https://algebraicjulia.github.io/AlgebraicDynamics.jl/stable/ and https://arxiv.org/abs/2105.12282
   - Finding: this is the most direct conceptual precedent for Options C/D. The note should compare Sembla's composition-source graph to a wiring-diagram operad and its linker/runtime lowering to an algebra/interpreter. This would sharpen, rather than broaden, the chosen architecture.

4. **Open systems and structured cospans are absent from the treatment of boundaries and composition.**
   - Local evidence: composite boundaries, exposure, hiding, and wires are defined operationally at `docs/design/composition-options.md:238-262`; directed, delayed wire behavior and single-driver fan-in constraints are fixed at `347-387`. There is no comparison with open-system formalisms.
   - External evidence: structured cospans provide a formalism for open systems whose feet represent interfaces and whose apex represents the system, with composition by gluing compatible interfaces. The Topos/AlgebraicJulia research lineage includes applying structured cospans to compositional epidemiological models.
   - Sources: https://arxiv.org/abs/1911.04630 and https://arxiv.org/abs/2203.16345 ; see also the AlgebraicJulia publication listing at https://www.algebraicjulia.org/
   - Finding: the note should evaluate structured cospans as a model for source-level boundaries/exposure/gluing, especially because its running `Population`/`Policy` examples (`docs/design/composition-options.md:925-969`) sit near existing compositional-epidemiology work. It should not silently equate the two: Sembla's ports are directed, a wire allocates a one-tick mailbox, and fan-in is restricted, whereas generic cospan gluing does not by itself impose those runtime semantics.

5. **The missing comparison hides an important design distinction: connectivity syntax is not execution semantics.**
   - Local evidence: Sembla deliberately makes every real wire delayed (`docs/design/composition-options.md:347-371`) and lowers only to one canonical executable plan (`846-867`).
   - External evidence: Catlab wiring diagrams encode compositional shape; AlgebraicDynamics requires a chosen algebra/semantics to turn the pattern and primitive systems into a composite. Structured cospans likewise formalize interface gluing but do not prescribe Philox identities, tick barriers, delayed mailboxes, CUDA behavior, or heterogeneous scheduler contracts.
   - Sources: https://algebraicjulia.github.io/Catlab.jl/stable/ and https://algebraicjulia.github.io/AlgebraicDynamics.jl/stable/
   - Finding: a good revision should say what is borrowed—typed compositional syntax, open boundaries, algebraic laws, ACSet-like data—and what remains Sembla-specific—one-tick channel state, conflict/claim rules, stochastic identity, scheduler domains, and canonical CPU/CUDA execution. This caveat prevents a superficial “adopt Catlab” recommendation.

### Suggested bounded revision

- Add one related-work subsection near Options C/D, not a redesign of the whole document.
- Include a small comparison table with rows: Catlab/ACSets, AlgebraicDynamics wiring-diagram operads, structured cospans/open systems, and Sembla hybrid; columns: source representation, boundary model, directionality, state/delay semantics, interpreter/linker, and runtime fit.
- Add the cited sources to a references list.
- Record one explicit open decision: whether Sembla merely borrows these formalisms, emits/accepts an ACSet-like interchange artifact, or uses an AlgebraicJulia tool as a development-time oracle. Given the Lean/Rust/CUDA stack, direct runtime adoption should not be presumed.

### Notes and residual risks

- `plan.md` and `progress.md`, requested as inputs, were absent at the supplied paths (`ENOENT`), so no plan-specific assumptions could be checked.
- Web evidence was checked against the current official Topos, AlgebraicJulia, Catlab, and AlgebraicDynamics pages. Papers establish the mathematical lineage; the report intentionally avoids claiming that Topos Institute originated structured cospans or all AlgebraicJulia work.
- No project/source file was modified. Only this required review artifact was written.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Performed only the requested independent review; left docs/design/composition-options.md and all project/source files unchanged and wrote the required review artifact."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Findings cite exact local line ranges and official Topos/AlgebraicJulia/Catlab/AlgebraicDynamics pages plus primary papers on C-sets, operadic dynamical systems, and structured cospans."
    }
  ],
  "changedFiles": [
    ".pi-subagents/artifacts/outputs/f720232d-7324-42ea-a47d-c65c21ec2f34/research/independent-review.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "read plan.md, progress.md, and docs/design/composition-options.md",
      "result": "passed with noted missing inputs",
      "summary": "Composition note was read; plan.md and progress.md returned ENOENT."
    },
    {
      "command": "grep -Ein 'Topos|Catlab|AlgebraicJulia|ACSet|C-set|cospan|operad|wiring diagram|compositional model|open system|category|categor' docs/design/composition-options.md",
      "result": "passed",
      "summary": "Found only an operad mention and generic categorical wording; none of the named frameworks or formalisms appears."
    },
    {
      "command": "nl -ba docs/design/composition-options.md | sed ...",
      "result": "passed",
      "summary": "Verified the exact line ranges cited in this report."
    },
    {
      "command": "fetch and inspect current official Topos, AlgebraicJulia, Catlab, and AlgebraicDynamics web pages",
      "result": "passed",
      "summary": "Confirmed Catlab's categorical data/wiring-diagram support, AlgebraicDynamics' operad-algebra composition model, and the ecosystem's compositional-modelling scope."
    },
    {
      "command": "git diff --cached --name-only",
      "result": "passed",
      "summary": "Produced no output; there are no staged files."
    }
  ],
  "validationOutput": [
    "The local term search contains no Topos, Catlab, AlgebraicJulia, ACSet/C-set, structured-cospan, open-system, or wiring-diagram treatment.",
    "Official Catlab documentation states that wiring diagrams have specialized data structures and serialization and identifies wiring diagrams as attributed C-sets.",
    "Official AlgebraicDynamics documentation states that it implements compositional/hierarchical dynamical systems using operads and operad algebras and supports directed and undirected wiring syntaxes.",
    "No staged files were detected."
  ],
  "residualRisks": [
    "plan.md and progress.md were unavailable at the requested paths.",
    "The cited formalisms are relevant prior art, not proof that Sembla should take a Julia dependency; runtime fit requires a separate prototype or ADR."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added only the required independent-review artifact; no project/source changes.",
  "reviewFindings": [
    "blocker: docs/design/composition-options.md:53-62,809-867 - no substantive evaluation of Topos/AlgebraicJulia compositional-framework prior art before framework selection.",
    "note: docs/design/composition-options.md:809-923 - ACSet/C-set representation is an unevaluated concrete option for the proposed typed source graph.",
    "note: docs/design/composition-options.md:238-262,347-387 - open systems and structured cospans are relevant to boundaries/gluing but must be distinguished from Sembla's directed one-tick wires.",
    "note: docs/design/composition-options.md:809-867 - wiring-diagram operads and operad algebras closely match the source-syntax/interpreter split."
  ],
  "manualNotes": "Review-only task completed. Attribution is deliberately phrased as Topos-associated rather than assigning invention of every cited formalism to Topos Institute."
}
```
