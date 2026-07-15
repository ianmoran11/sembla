# Sembla visual guide: boxes, tables, wires, and dynamics

This guide explains Sembla first as an abstract compositional language and then through three concrete models. Each diagram is an inline SVG designed to render directly in Obsidian Reading view and to inherit Obsidian's light or dark theme where possible.

> [!note] Two related names
> The Lean surface DSL says `system`; the exported IR and Rust runtime call the corresponding row collection a **table**. In this guide, a system/table means the same modeled population.

## 1. Abstract anatomy of a Sembla model

A model owns global parameters and a fixed step `dt`. It contains one or more **boxes**. Each box contains tables, row-local transitions, input ports, and output builders. A **wire** connects an output port to an input port and carries a stream of finite tables between boxes with a synchronous one-tick delay.

<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1120 650" width="100%" role="img" aria-labelledby="abstract-title abstract-desc" style="max-width:100%;height:auto;background:var(--background-secondary,#f5f7fa);border:1px solid var(--background-modifier-border,#cbd5e1);border-radius:14px">
  <title id="abstract-title">Abstract anatomy of a Sembla model</title>
  <desc id="abstract-desc">A model contains two boxes. Each box contains tables and transitions. Output tables travel over one-tick-delay wires to input ports on the other box.</desc>
  <defs>
    <marker id="abstract-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--interactive-accent,#4f7cac)"/>
    </marker>
    <marker id="abstract-ref-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--text-muted,#64748b)"/>
    </marker>
  </defs>

  <rect x="34" y="30" width="1052" height="574" rx="18" fill="var(--background-primary,#ffffff)" stroke="var(--background-modifier-border,#94a3b8)" stroke-width="2" stroke-dasharray="8 6"/>
  <text x="58" y="65" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="22" font-weight="700">MODEL</text>
  <text x="150" y="65" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="15">global parameters θ · fixed step dt · declaration order</text>

  <!-- Box A -->
  <rect x="72" y="105" width="410" height="390" rx="16" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-blue,#3b82f6)" stroke-width="2.5"/>
  <rect x="72" y="105" width="410" height="56" rx="16" fill="color-mix(in srgb, var(--color-blue,#3b82f6) 14%, var(--background-primary,#fff))" stroke="none"/>
  <text x="94" y="140" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="20" font-weight="700">BOX A</text>
  <text x="180" y="140" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="13">component / Moore machine</text>

  <rect x="102" y="188" width="155" height="142" rx="11" fill="var(--background-primary,#ffffff)" stroke="var(--color-green,#22c55e)" stroke-width="2"/>
  <text x="118" y="214" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="16" font-weight="700">TABLE α</text>
  <text x="118" y="239" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">rows: entities</text>
  <text x="118" y="260" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">state: enum</text>
  <text x="118" y="281" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">value: Real/Int</text>
  <text x="118" y="302" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">ref → table β</text>

  <rect x="290" y="188" width="155" height="142" rx="11" fill="var(--background-primary,#ffffff)" stroke="var(--color-green,#22c55e)" stroke-width="2"/>
  <text x="306" y="214" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="16" font-weight="700">TABLE β</text>
  <text x="306" y="239" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">group/resource rows</text>
  <text x="306" y="260" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">join target</text>
  <text x="306" y="281" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">aggregate key</text>
  <path d="M 256 304 C 278 304, 278 304, 298 304" fill="none" stroke="var(--text-muted,#64748b)" stroke-width="1.7" marker-end="url(#abstract-ref-arrow)"/>

  <rect x="102" y="360" width="343" height="98" rx="11" fill="var(--background-primary,#ffffff)" stroke="var(--color-orange,#f59e0b)" stroke-width="2"/>
  <text x="118" y="387" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="15" font-weight="700">TRANSITIONS + OUTPUT BUILDERS</text>
  <text x="118" y="412" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">guard(row, inputs) · hazard(row, θ, aggregates)</text>
  <text x="118" y="435" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">set row attributes · emit finite output tables</text>

  <!-- Box B -->
  <rect x="638" y="105" width="410" height="390" rx="16" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-purple,#8b5cf6)" stroke-width="2.5"/>
  <rect x="638" y="105" width="410" height="56" rx="16" fill="color-mix(in srgb, var(--color-purple,#8b5cf6) 14%, var(--background-primary,#fff))" stroke="none"/>
  <text x="660" y="140" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="20" font-weight="700">BOX B</text>
  <text x="746" y="140" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="13">another component</text>

  <rect x="682" y="188" width="320" height="142" rx="11" fill="var(--background-primary,#ffffff)" stroke="var(--color-green,#22c55e)" stroke-width="2"/>
  <text x="700" y="214" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="16" font-weight="700">TABLE γ</text>
  <text x="700" y="239" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">fixed rows · typed attributes</text>
  <text x="700" y="264" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">state transition graph</text>
  <text x="700" y="289" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">reads input tables delivered by wires</text>

  <rect x="682" y="360" width="320" height="98" rx="11" fill="var(--background-primary,#ffffff)" stroke="var(--color-orange,#f59e0b)" stroke-width="2"/>
  <text x="700" y="387" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="15" font-weight="700">TRANSITIONS + OUTPUT BUILDERS</text>
  <text x="700" y="412" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">same semantics, independently scoped state</text>
  <text x="700" y="435" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">outputs become next-tick input elsewhere</text>

  <!-- Ports and wires -->
  <rect x="451" y="215" width="62" height="34" rx="17" fill="var(--interactive-accent,#4f7cac)"/>
  <text x="482" y="237" text-anchor="middle" fill="white" font-family="Inter, system-ui, sans-serif" font-size="11" font-weight="700">OUTPUT</text>
  <rect x="607" y="215" width="62" height="34" rx="17" fill="var(--interactive-accent,#4f7cac)"/>
  <text x="638" y="237" text-anchor="middle" fill="white" font-family="Inter, system-ui, sans-serif" font-size="11" font-weight="700">INPUT</text>
  <path d="M 514 232 L 605 232" fill="none" stroke="var(--interactive-accent,#4f7cac)" stroke-width="3" marker-end="url(#abstract-arrow)"/>
  <text x="560" y="205" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="12" font-weight="700">WIRE</text>
  <text x="560" y="267" text-anchor="middle" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="11">finite table</text>

  <rect x="607" y="395" width="62" height="34" rx="17" fill="var(--color-purple,#8b5cf6)"/>
  <text x="638" y="417" text-anchor="middle" fill="white" font-family="Inter, system-ui, sans-serif" font-size="11" font-weight="700">OUTPUT</text>
  <rect x="451" y="395" width="62" height="34" rx="17" fill="var(--color-purple,#8b5cf6)"/>
  <text x="482" y="417" text-anchor="middle" fill="white" font-family="Inter, system-ui, sans-serif" font-size="11" font-weight="700">INPUT</text>
  <path d="M 606 412 L 515 412" fill="none" stroke="var(--color-purple,#8b5cf6)" stroke-width="3" marker-end="url(#abstract-arrow)"/>
  <text x="560" y="450" text-anchor="middle" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="11">feedback table</text>

  <!-- Timeline -->
  <rect x="72" y="525" width="976" height="54" rx="10" fill="var(--background-secondary,#f8fafc)" stroke="var(--background-modifier-border,#cbd5e1)"/>
  <text x="94" y="558" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="14" font-weight="700">TICK t</text>
  <text x="160" y="558" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="13">read frozen state + delivered inputs → evaluate hazards → race/commit row changes → build outputs → deliver them at tick t+1</text>

  <!-- Legend -->
  <rect x="72" y="616" width="18" height="18" rx="4" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-blue,#3b82f6)" stroke-width="2"/>
  <text x="99" y="630" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="12">box</text>
  <rect x="152" y="616" width="18" height="18" rx="4" fill="var(--background-primary,#fff)" stroke="var(--color-green,#22c55e)" stroke-width="2"/>
  <text x="179" y="630" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="12">table/system</text>
  <line x1="280" y1="625" x2="326" y2="625" stroke="var(--interactive-accent,#4f7cac)" stroke-width="3" marker-end="url(#abstract-arrow)"/>
  <text x="337" y="630" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="12">table-valued wire · one-tick delay</text>
</svg>

### How the pieces relate

1. **Rows are the simulated entities.** Tables have fixed row counts in the current runtime.
2. **Attributes are columns.** They may be enum state, `Real`, `Int`, or references to another table in the same box.
3. **Transitions are row-local rules.** A Boolean guard determines eligibility; a non-negative hazard determines the per-tick firing probability; effects set attributes on the same row.
4. **References and aggregates are internal to a box.** `countBy` and `sizeBy` group one table through a reference and broadcast the result back to rows.
5. **Ports and wires compose boxes.** Output builders produce finite tables. Wires deliver those tables to matching input schemas on the following tick.
6. **Execution is snapshot-isolated tau-leaping.** Guards, aggregates, and rates are frozen at tick start. A row cannot enter a state and leave it again during the same tick.

> [!important] References are not wires
> A reference joins tables **inside one box**. A wire transports an emitted finite table **between box interfaces**. Keeping these concepts separate makes the composition boundary explicit.

---

## 2. Concrete example: reversible two-state chain

This is the smallest useful state-machine example. It has one box, one table, no references, and no wires. Each particle row switches between `A` and `B` using two parameterized hazards.

<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 960 380" width="100%" role="img" aria-labelledby="ctmc-title ctmc-desc" style="max-width:100%;height:auto;background:var(--background-secondary,#f5f7fa);border:1px solid var(--background-modifier-border,#cbd5e1);border-radius:14px">
  <title id="ctmc-title">Reversible two-state chain</title>
  <desc id="ctmc-desc">One chain box contains a particle table. Particle state A changes to B with rate ab and B changes to A with rate ba.</desc>
  <defs>
    <marker id="ctmc-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--interactive-accent,#4f7cac)"/>
    </marker>
  </defs>

  <rect x="35" y="35" width="890" height="300" rx="18" fill="var(--background-primary,#fff)" stroke="var(--color-blue,#3b82f6)" stroke-width="2.5"/>
  <text x="62" y="76" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="22" font-weight="700">BOX: chain</text>
  <text x="62" y="102" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="13">no inputs · no outputs · no wires</text>

  <rect x="76" y="135" width="500" height="160" rx="14" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-green,#22c55e)" stroke-width="2"/>
  <text x="98" y="166" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="17" font-weight="700">TABLE: particle</text>
  <text x="98" y="190" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">phase : {A, B}</text>

  <rect x="132" y="218" width="78" height="52" rx="26" fill="var(--background-primary,#fff)" stroke="var(--color-blue,#3b82f6)" stroke-width="3"/>
  <text x="171" y="251" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="18" font-weight="700">A</text>
  <rect x="436" y="218" width="78" height="52" rx="26" fill="var(--background-primary,#fff)" stroke="var(--color-orange,#f59e0b)" stroke-width="3"/>
  <text x="475" y="251" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="18" font-weight="700">B</text>
  <path d="M 211 234 C 285 187, 363 187, 435 234" fill="none" stroke="var(--interactive-accent,#4f7cac)" stroke-width="2.5" marker-end="url(#ctmc-arrow)"/>
  <text x="323" y="200" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="ui-monospace, monospace" font-size="13">move_ab · hazard rate_ab</text>
  <path d="M 435 257 C 360 304, 285 304, 211 257" fill="none" stroke="var(--color-purple,#8b5cf6)" stroke-width="2.5" marker-end="url(#ctmc-arrow)"/>
  <text x="323" y="294" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="ui-monospace, monospace" font-size="13">move_ba · hazard rate_ba</text>

  <rect x="620" y="135" width="260" height="160" rx="14" fill="var(--background-secondary,#f8fafc)" stroke="var(--background-modifier-border,#94a3b8)" stroke-width="2"/>
  <text x="642" y="166" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="16" font-weight="700">GLOBAL PARAMETERS</text>
  <text x="642" y="199" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="13">rate_ab = 0.4</text>
  <text x="642" y="225" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="13">rate_ba = 0.2</text>
  <text x="642" y="257" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="13">dt = 0.1</text>
  <text x="642" y="280" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="11">numeric initialization: all rows begin in A</text>
</svg>

**What this example isolates**

- A table can be a pure finite-state population with no relational structure.
- Model parameters remain symbolic in the IR and can be overridden per run.
- The arrows specify canonical CTMC hazards, while the current executor advances them on the fixed `dt` tick grid.
- Generic CSV output reports counts for `A` and `B` plus firings for both transitions.

Model files: [`frontend/Sembla/Models.lean`](../frontend/Sembla/Models.lean) · [`examples/reversible_ctmc.json`](../examples/reversible_ctmc.json)

---

## 3. Concrete example: SIS with a grouped aggregate

SIS demonstrates the difference between a table reference and a wire. `person.community` points to a row in the `community` table inside the same box. The runtime groups people by that key once per tick to compute infectious and total community counts.

<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1040 500" width="100%" role="img" aria-labelledby="sis-title sis-desc" style="max-width:100%;height:auto;background:var(--background-secondary,#f5f7fa);border:1px solid var(--background-modifier-border,#cbd5e1);border-radius:14px">
  <title id="sis-title">SIS with importation and community aggregate</title>
  <desc id="sis-desc">An epidemic box contains person and community tables. A person reference joins to community. Grouped infectious and population counts feed the infection hazard.</desc>
  <defs>
    <marker id="sis-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--interactive-accent,#4f7cac)"/>
    </marker>
    <marker id="sis-ref-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--text-muted,#64748b)"/>
    </marker>
  </defs>

  <rect x="36" y="34" width="968" height="420" rx="18" fill="var(--background-primary,#fff)" stroke="var(--color-blue,#3b82f6)" stroke-width="2.5"/>
  <text x="64" y="76" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="22" font-weight="700">BOX: epidemic</text>
  <text x="64" y="101" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="13">internal relational dynamics · no inter-box wire</text>

  <rect x="70" y="130" width="380" height="270" rx="14" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-green,#22c55e)" stroke-width="2"/>
  <text x="94" y="163" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="17" font-weight="700">TABLE: person</text>
  <text x="94" y="188" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">health : {S, I}</text>
  <text x="94" y="211" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">community : Ref community</text>

  <rect x="113" y="246" width="74" height="50" rx="25" fill="var(--background-primary,#fff)" stroke="var(--color-blue,#3b82f6)" stroke-width="3"/>
  <text x="150" y="278" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="17" font-weight="700">S</text>
  <rect x="327" y="246" width="74" height="50" rx="25" fill="var(--background-primary,#fff)" stroke="var(--color-orange,#f59e0b)" stroke-width="3"/>
  <text x="364" y="278" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="17" font-weight="700">I</text>
  <path d="M 188 258 C 232 225, 283 225, 326 258" fill="none" stroke="var(--interactive-accent,#4f7cac)" stroke-width="2.4" marker-end="url(#sis-arrow)"/>
  <text x="257" y="223" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="12" font-weight="700">infect</text>
  <path d="M 326 286 C 283 319, 232 319, 188 286" fill="none" stroke="var(--color-purple,#8b5cf6)" stroke-width="2.4" marker-end="url(#sis-arrow)"/>
  <text x="257" y="335" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="12" font-weight="700">recover · γ</text>
  <text x="94" y="373" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="11">infect hazard = import + β · I_c / N_c</text>

  <rect x="620" y="130" width="318" height="116" rx="14" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-green,#22c55e)" stroke-width="2"/>
  <text x="643" y="163" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="17" font-weight="700">TABLE: community</text>
  <text x="643" y="190" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="12">group rows / join targets</text>
  <text x="643" y="216" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="12">no mutable attributes required</text>
  <path d="M 451 190 C 512 154, 556 154, 619 180" fill="none" stroke="var(--text-muted,#64748b)" stroke-width="2" stroke-dasharray="6 4" marker-end="url(#sis-ref-arrow)"/>
  <text x="530" y="147" text-anchor="middle" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="12">person.community reference</text>

  <rect x="560" y="284" width="378" height="116" rx="14" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-orange,#f59e0b)" stroke-width="2"/>
  <text x="582" y="316" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="16" font-weight="700">PER-TICK AGGREGATE CACHE</text>
  <text x="582" y="344" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">I_c = countBy community (health = I)</text>
  <text x="582" y="369" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="12">N_c = sizeBy community</text>
  <path d="M 560 354 C 500 354, 482 336, 425 302" fill="none" stroke="var(--color-orange,#f59e0b)" stroke-width="2.5" marker-end="url(#sis-arrow)"/>
  <text x="486" y="382" text-anchor="middle" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="11">broadcast I_c/N_c to each person row</text>
</svg>

**What this example adds**

- `person` and `community` remain separate tables with an explicit foreign-key-like reference.
- The aggregate is constructed from the frozen tick-start snapshot, not recomputed independently for every person.
- The positive importation rate makes `I` reachable from homogeneous numeric initialization.
- With `--population N`, all person references initially point to community row `0`, so the CLI smoke run behaves as one well-mixed community.

Model files: [`frontend/Sembla/Models.lean`](../frontend/Sembla/Models.lean) · [`examples/sis_importation.json`](../examples/sis_importation.json)

---

## 4. Concrete example: two-box SIR-policy feedback

This is the composition example. The population and policy boxes own independent state. They communicate only through typed output/input ports. Both wires are delayed by one tick, so a policy change committed at tick `t` first modifies the population hazard at tick `t+1`.

<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1120 590" width="100%" role="img" aria-labelledby="policy-title policy-desc" style="max-width:100%;height:auto;background:var(--background-secondary,#f5f7fa);border:1px solid var(--background-modifier-border,#cbd5e1);border-radius:14px">
  <title id="policy-title">SIR population and policy feedback boxes</title>
  <desc id="policy-desc">The population box sends an infection count table to the policy box. The policy box sends a restriction modifier table back. Both wires have one tick delay.</desc>
  <defs>
    <marker id="policy-arrow-blue" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--color-blue,#3b82f6)"/>
    </marker>
    <marker id="policy-arrow-purple" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--color-purple,#8b5cf6)"/>
    </marker>
  </defs>

  <!-- Population box -->
  <rect x="42" y="76" width="455" height="410" rx="18" fill="var(--background-primary,#fff)" stroke="var(--color-blue,#3b82f6)" stroke-width="2.7"/>
  <text x="68" y="116" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="21" font-weight="700">BOX: population</text>
  <text x="68" y="141" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="12">large stochastic population</text>

  <rect x="76" y="170" width="385" height="190" rx="14" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-green,#22c55e)" stroke-width="2"/>
  <text x="98" y="200" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="16" font-weight="700">TABLES: person + employer</text>
  <text x="98" y="224" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="11">person.health : {S, I, R}</text>
  <text x="98" y="246" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="11">person.employer : Ref employer</text>

  <rect x="110" y="278" width="64" height="44" rx="22" fill="var(--background-primary,#fff)" stroke="var(--color-blue,#3b82f6)" stroke-width="2.5"/>
  <text x="142" y="306" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="15" font-weight="700">S</text>
  <rect x="235" y="278" width="64" height="44" rx="22" fill="var(--background-primary,#fff)" stroke="var(--color-orange,#f59e0b)" stroke-width="2.5"/>
  <text x="267" y="306" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="15" font-weight="700">I</text>
  <rect x="360" y="278" width="64" height="44" rx="22" fill="var(--background-primary,#fff)" stroke="var(--color-green,#22c55e)" stroke-width="2.5"/>
  <text x="392" y="306" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="15" font-weight="700">R</text>
  <path d="M 175 300 L 234 300" fill="none" stroke="var(--color-blue,#3b82f6)" stroke-width="2.5" marker-end="url(#policy-arrow-blue)"/>
  <path d="M 300 300 L 359 300" fill="none" stroke="var(--color-blue,#3b82f6)" stroke-width="2.5" marker-end="url(#policy-arrow-blue)"/>
  <text x="204" y="283" text-anchor="middle" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="10">infect</text>
  <text x="329" y="283" text-anchor="middle" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="10">recover</text>

  <rect x="76" y="386" width="385" height="68" rx="11" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-orange,#f59e0b)"/>
  <text x="96" y="414" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="13" font-weight="700">infection hazard</text>
  <text x="96" y="438" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="11">β · I_work/N_work · (1 + input modifier_offset)</text>

  <!-- Policy box -->
  <rect x="623" y="76" width="455" height="410" rx="18" fill="var(--background-primary,#fff)" stroke="var(--color-purple,#8b5cf6)" stroke-width="2.7"/>
  <text x="649" y="116" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="21" font-weight="700">BOX: policy</text>
  <text x="649" y="141" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="12">small feedback controller</text>

  <rect x="657" y="170" width="387" height="190" rx="14" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-green,#22c55e)" stroke-width="2"/>
  <text x="679" y="200" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="16" font-weight="700">TABLE: controller (1 row)</text>
  <text x="679" y="224" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="11">mode : {Open, Restricted}</text>
  <text x="679" y="246" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="11">modifier : Real</text>

  <rect x="692" y="282" width="104" height="42" rx="21" fill="var(--background-primary,#fff)" stroke="var(--color-green,#22c55e)" stroke-width="2.5"/>
  <text x="744" y="308" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="13" font-weight="700">Open</text>
  <rect x="884" y="282" width="128" height="42" rx="21" fill="var(--background-primary,#fff)" stroke="var(--color-orange,#f59e0b)" stroke-width="2.5"/>
  <text x="948" y="308" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="13" font-weight="700">Restricted</text>
  <path d="M 797 290 C 827 267, 854 267, 883 290" fill="none" stroke="var(--color-purple,#8b5cf6)" stroke-width="2.5" marker-end="url(#policy-arrow-purple)"/>
  <path d="M 883 316 C 854 340, 827 340, 797 316" fill="none" stroke="var(--color-purple,#8b5cf6)" stroke-width="2.5" marker-end="url(#policy-arrow-purple)"/>
  <text x="840" y="262" text-anchor="middle" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="10">infected &gt; 500</text>
  <text x="840" y="354" text-anchor="middle" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="10">infected &lt; 150</text>

  <rect x="657" y="386" width="387" height="68" rx="11" fill="var(--background-secondary,#f8fafc)" stroke="var(--color-orange,#f59e0b)"/>
  <text x="678" y="414" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="13" font-weight="700">controller output</text>
  <text x="678" y="438" fill="var(--text-muted,#64748b)" font-family="ui-monospace, monospace" font-size="11">modifier_offset = modifier − 1</text>

  <!-- Forward wire -->
  <rect x="472" y="183" width="58" height="28" rx="14" fill="var(--color-blue,#3b82f6)"/>
  <text x="501" y="201" text-anchor="middle" fill="white" font-family="Inter, system-ui, sans-serif" font-size="9" font-weight="700">OUTPUT</text>
  <rect x="590" y="183" width="58" height="28" rx="14" fill="var(--color-blue,#3b82f6)"/>
  <text x="619" y="201" text-anchor="middle" fill="white" font-family="Inter, system-ui, sans-serif" font-size="9" font-weight="700">INPUT</text>
  <path d="M 531 197 L 588 197" fill="none" stroke="var(--color-blue,#3b82f6)" stroke-width="3" marker-end="url(#policy-arrow-blue)"/>
  <text x="560" y="168" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="11" font-weight="700">infection_count</text>
  <text x="560" y="225" text-anchor="middle" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="10">{ infected : Int }</text>

  <!-- Feedback wire -->
  <rect x="590" y="399" width="58" height="28" rx="14" fill="var(--color-purple,#8b5cf6)"/>
  <text x="619" y="417" text-anchor="middle" fill="white" font-family="Inter, system-ui, sans-serif" font-size="9" font-weight="700">OUTPUT</text>
  <rect x="472" y="399" width="58" height="28" rx="14" fill="var(--color-purple,#8b5cf6)"/>
  <text x="501" y="417" text-anchor="middle" fill="white" font-family="Inter, system-ui, sans-serif" font-size="9" font-weight="700">INPUT</text>
  <path d="M 589 413 L 532 413" fill="none" stroke="var(--color-purple,#8b5cf6)" stroke-width="3" marker-end="url(#policy-arrow-purple)"/>
  <text x="560" y="449" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="11" font-weight="700">restriction_modifier</text>
  <text x="560" y="469" text-anchor="middle" fill="var(--text-muted,#64748b)" font-family="Inter, system-ui, sans-serif" font-size="10">{ modifier_offset : Real }</text>

  <rect x="194" y="520" width="732" height="42" rx="21" fill="var(--background-primary,#fff)" stroke="var(--background-modifier-border,#94a3b8)" stroke-width="1.5"/>
  <text x="560" y="546" text-anchor="middle" fill="var(--text-normal,#17202a)" font-family="Inter, system-ui, sans-serif" font-size="13" font-weight="700">outputs built after tick t → delivered as input tables for tick t+1</text>
</svg>

**What composition changes**

- Each box remains a valid stateful component with private tables.
- Ports expose table schemas rather than direct access to another box's state.
- The population emits one-row infection summaries; the policy emits one-row modifier summaries.
- The delay makes feedback causal and deterministic: neither box observes the other box's in-progress tick.
- At tick zero, input ports are schema-carrying zero-row tables. The population therefore encodes its neutral modifier as `1 + sum(modifier_offset)`.

Model files: [`frontend/Sembla/Models.lean`](../frontend/Sembla/Models.lean) · [`examples/sir_policy.json`](../examples/sir_policy.json) · [policy walkthrough](examples/sir_policy.md)

---

## 5. Reading the current example catalog

| Example | Boxes | Tables | Internal relation/aggregate | Inter-box wire |
| --- | ---: | --- | --- | --- |
| Reversible CTMC | 1 | `particle` | none | none |
| Radioactive decay chain | 1 | `atom` | none | none |
| SIS with importation | 1 | `person`, `community` | `person.community`; `I_c/N_c` | none |
| SEIRS with waning | 1 | `person`, `community` | `person.community`; `I_c/N_c` | none |
| Noisy voter | 1 | `agent`, `community` | `agent.community`; opposite-opinion fraction | none |
| SIR policy feedback | 2 | `person`, `employer`, `controller` | workplace infection fraction | two delayed feedback wires |

See [canonical finite-state models](examples/canonical-models.md) for runnable commands and formulas.

## 6. Current expressiveness boundary

The diagrams deliberately stay within the implemented semantics:

- fixed row populations;
- enum, integer, real, and reference columns;
- row guards and hazard-rate transitions;
- grouped count/sum aggregates;
- row-local attribute updates;
- typed finite-table outputs and one-tick-delay wires;
- deterministic snapshot-isolated tau-leaping.

The current fast path does **not** yet include changing row counts, exact continuous event ordering, ODE/PDE integrator blocks, or one transition that atomically updates multiple entity rows.
