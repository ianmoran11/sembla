# PRD 0002: IR schema, serde types, and validator

## Context

The IR is Sembla's central contract (`DESIGN.md` §2): the Lean frontend emits
it, the Rust runtime executes it, and **seed + IR + determinism level ⇒
reproducible results**. It is a JSON document describing a model: boxes with
columnar schemas (`DESIGN.md` §4.1), hazard-rate transitions (§4.3), contested
resources (§5.1), and wires between boxes (§4.4). This PRD defines the format,
the Rust types, and validation. Execution comes later (PRDs 0004–0007).

## Goal

`sembla-ir` parses, validates, and round-trips IR JSON files, rejecting
malformed models with precise errors, with checked-in golden fixtures as the
compatibility contract.

## Specification

Serde-derived Rust types in `sembla-ir`, JSON as the wire format (tagged
enums; exact tag spelling is the implementer's choice but is frozen by the
golden fixtures once checked in). The model structure:

- `Model { name, dt: f64, params: Vec<ParamDecl>, boxes: Vec<Box>,
  wires: Vec<Wire> }` — `dt` is the tick width in model-time units (§4.3: a
  semantic parameter).
- `ParamDecl { name, ty: Real | Int, default, prior: Option<Prior> }` —
  parameters are first-class and per-run constant (`DESIGN.md` §4.1): the
  run contract is **seed + IR + θ + level**, so parameter values are NEVER
  inlined into expressions. `Prior { family: Normal | LogNormal | Uniform,
  args: Vec<f64> }` is declarative metadata in this PRD (arity-validated:
  2 args each; Uniform requires lo < hi); it is sampled by PRD 0013 and
  rendered by PRD 0011.
- `Box { name, tables: Vec<Table>, transitions: Vec<Transition>,
  inputs: Vec<PortDecl>, outputs: Vec<OutputDecl> }`
- `Table { name, size_hint: u64, attrs: Vec<Attr> }`;
  `Attr { name, ty: Real | Int | Enum { variants: Vec<String> } |
  Ref { table: String } }`
- `Transition { name, table, guard: Expr, hazard: Expr,
  effects: Vec<Effect>, contests: Vec<ResourceClaim> }` — `guard` must be
  Bool-typed; `hazard` Real-typed (a rate λ ≥ 0 per model-time unit, §4.3).
- `Effect::SetAttr { attr, value: Expr }` — writes are to the transition's
  own row only in v0.1 (no birth/death, no cross-row writes except via
  contested resources, which in v0.1 still only gate self-writes).
- `ResourceClaim { resource: Expr (Ref-typed), ordering: RaceTime |
  Key(Expr) }` — declares that this transition contests the referenced
  entity; at most one contested transition per resource fires per tick
  (§5.1). `RaceTime` resolves by sampled firing time; `Key` by the given
  expression, ascending.
- `Expr` (typed, first-order, allocation-free — §4.2): literals (real, int,
  bool, enum variant); `Param(name)` (resolves to the declared parameter's
  type); `SelfAttr(name)`; arithmetic `(+ − × ÷)`, comparison,
  boolean ops; `EnumIs { attr, variant }`; `Input { port, agg }` (aggregate
  over this tick's received input table — used from PRD 0007);
  `Agg { op: Count | Sum, table, on: (fk_attr must equal self's fk_attr),
  filter: Expr }` — the group-by aggregate form (the `infect` pattern,
  `DESIGN.md` §7). No recursion, no user functions, no unbounded joins:
  aggregates join only through declared Ref attributes.
- `PortDecl { name, schema: Vec<Attr> }`;
  `OutputDecl { name, schema, builder: OutputBuilder }` where
  `OutputBuilder` is restricted in v0.1 to per-table aggregates
  (e.g. one row of counts/sums per tick).
- `Wire { from: (box, output), to: (box, input) }`.

Validation (error messages must name the offending path):

- All name references resolve (tables, attrs, enum variants, boxes, ports,
  params); duplicate param names rejected; prior arity/argument rules
  enforced.
- Expression type-checking per the rules above.
- Wire schema compatibility (output schema == input schema).
- `contests` coverage: transitions whose `resource` refers to a Ref attr must
  claim it; duplicate claims within one transition rejected.
- `rule_id` (u32) assigned by declaration order across the whole model during
  validation and exposed on the validated form (`README.md` conventions).

## Deliverables

- Types, parser, validator, and canonical serializer in `sembla-ir`.
- `examples/two_state.json`: a valid minimal model — one box, one `Person`
  table (enum attr `mood: {Calm, Agitated}`), two hazard transitions between
  the states whose rates are declared params (one with a `LogNormal` prior,
  one prior-less) referenced via `Param`, no wires. Used by all later
  runtime PRDs.
- `examples/invalid/*.json`: at least 5 fixtures each triggering a distinct
  validator error.
- Golden tests: parse → validate → serialize → byte-compare for the valid
  fixture; each invalid fixture asserts its specific error.
- `sembla validate <path>` CLI subcommand (exit 0/1, error to stderr).

## Non-goals

Execution semantics, RNG, Lean emission, birth/death, scheduled clocks,
Level B/C determinism.

## Acceptance criteria

1. `cargo test --workspace` passes, including golden round-trip and all
   invalid-fixture tests.
2. `cargo run -p sembla-cli -- validate examples/two_state.json` exits 0;
   running it on each `examples/invalid/*.json` exits 1 with an error message
   naming the offending element.
3. Serialization is canonical: parse→serialize→parse→serialize produces
   byte-identical output (test exists and passes).
4. The `Agg` expression form supports the `infect` pattern: a test constructs
   "count rows in my table sharing my `employer` Ref where `health` is `I`"
   and it type-checks.
5. `rule_id` assignment is by declaration order and covered by a unit test.
6. Params: `Param` references type-check against declarations; an unresolved
   param name, a duplicate declaration, and a bad prior arity each produce a
   named validator error (invalid fixtures cover at least one of these);
   the valid golden fixture contains a params block with a prior.
7. Rustdoc on `Model`, `ParamDecl`, `Transition`, `Expr`, and
   `ResourceClaim` explains semantics with references to `DESIGN.md`
   sections (§4.1, §4.2, §4.3, §5.1), including the rule that parameter
   values are never inlined into the IR.
