# The model algebra

A compact mathematical notation for Sembla models, pitched at the level of
**boxes, wires, ticks, and observation**. Queries, expression syntax, prior
families, and serialization are deliberately opaque at this altitude: they are
the *contents* of the symbols below, never their structure.

Companion to [`DESIGN.md`](../../DESIGN.md) §4–§5, which states the same
commitments in prose. This document is the notation those commitments should be
reasoned in; [`frontend/`](../../frontend/) is where they become Lean.

---

## 0. The definition, in one line

> A model is a **delay-wired diagram of Moore lenses over finite indexed
> tables**, parameterized by θ, indexed by an entropy field ω, interpreted in a
> numeric algebra 𝔸, and observed through a **sink**.

Everything below unfolds that sentence.

---

## 1. The ambient context

Three things are fixed for the whole of a run and never appear as state. Write
the triple

$$E \;=\; (\theta,\ \omega,\ \mathbb{A})$$

- **θ ∈ Θ** — the parameter vector. Θ is an opaque set. Priors are a measure on
  Θ and play no part in the dynamics; they select θ, they do not evolve.
- **ω ∈ Ω** — the *entropy field*. Fix a coordinate set
  $C = \mathbb{N}^4 \ni (t, r, e, k)$ (tick, rule, entity, draw). Then
  $\Omega = [0,1)^{C}$, and a seed is a point $\sigma \mapsto \omega_\sigma \in \Omega$.
- **𝔸** — the numeric interpretation: an ordered field-like carrier together
  with a *reduction* $\textstyle\sum_{\mathbb A}$ on finite indexed families of a
  commutative monoid. Ideal: $\mathbb A = \mathbb R$. Executed:
  $\mathbb A = \mathbb F$ (IEEE `f64`) plus a canonicalizing reduction order.

The single most useful consequence of writing randomness as an *ambient
constant field* rather than a stream: **a model is a deterministic dynamical
system**, and all probability lives in the map $\sigma \mapsto \omega_\sigma$.

---

## 2. Interfaces

Let $\mathcal S$ be a set of schemas. A schema $A$ denotes a row set
$[\![ A ]\!]$, and a **table** is a finite *indexed* family

$$\mathrm{Tbl}(A) \;=\; \{\,\tau : \mathcal E \rightharpoonup [\![ A ]\!] \ \text{finite}\,\}$$

with $\mathcal E$ the entity-identifier set. Tables are indexed, not ordered:
row position is not part of the data, which is what makes every operation below
order-free by construction rather than by discipline.

An **interface** is a finite family of schemas. Interfaces form a symmetric
monoidal category under disjoint union, and $\mathrm{Tbl}$ extends to it
pointwise: $\mathrm{Tbl}(I) = \prod_{p \in I}\mathrm{Tbl}(I_p)$.

---

## 3. Boxes

A **box** $b : I \to O$ is

$$b \;=\; \bigl(S_b,\ s^0_b,\ u_b,\ y_b\bigr)
\qquad
\begin{aligned}
u_b &: E \to S_b \times \mathrm{Tbl}(I) \to S_b \\
y_b &: E \to S_b \to \mathrm{Tbl}(O)
\end{aligned}$$

$S_b$ is the box's ACSet-valued state; at this altitude it is just a set.

The **Moore condition** — $y_b$ depends on state alone, never on the current
input — is load-bearing, not cosmetic. It is exactly what makes feedback loops
compose without solving a fixpoint equation, and therefore what makes §5 total.

Write $\mathbf{Box}(I,O)$ for the boxes of that signature.

---

## 4. Inside a box: race and resolve

$u_b$ is not opaque; it has a fixed shape. A box carries a finite **rule set**
$R$. Each $r \in R$ is a quadruple

$$r \;=\; (\,g_r,\ \lambda_r,\ \rho_r,\ \delta_r\,)$$

| symbol | reads as | type |
|---|---|---|
| $g_r$ | enabledness (guard) | $S \times \mathrm{Tbl}(I) \to \mathcal P(\mathcal E)$ |
| $\lambda_r$ | clock (hazard) | instance $\to \mathbb A_{\ge 0}$ |
| $\rho_r$ | claims | instance $\to \mathcal P_{\text{fin}}(\mathcal R)$ |
| $\delta_r$ | effect | instance $\to (S \to S)$ |

with $\mathcal R$ a set of contested resources and $c : \mathcal R \to \mathbb N$
their capacities ($c \equiv 1$ in v0.1).

Fix a tick width $\Delta t$ and a tick index $t$. The **race time** of instance
$(r,e)$ is the coordinate-pure draw

$$T_{r,e} \;=\; -\ln\bigl(\omega(t, r, e, 0)\bigr) \,/\, \lambda_r(e),
\qquad T_{r,e} = \infty \ \text{ when } \lambda_r(e) \le 0$$

and the tick map is three lines:

$$
\begin{aligned}
F &\;=\; \{\,(r,e)\ :\ e \in g_r(s,\mathrm{in}),\ T_{r,e} < \Delta t\,\}
  &&\text{candidates}\\[2pt]
W &\;=\; \mathrm{sel}_{\prec,\,c}(F,\ \rho)
  &&\text{winners}\\[2pt]
u_b(s,\mathrm{in}) &\;=\; \Bigl(\textstyle\bigcirc_{(r,e)\in W}\ \delta_r(e)\Bigr)(s)
  &&\text{commit}
\end{aligned}
$$

$\mathrm{sel}_{\prec,c}$ is a **choice function on the claim hypergraph**: for
each resource $\varrho$, retain the $c(\varrho)$ $\prec$-least claimants and
defer the rest to the next tick. $\prec$ is any *total* order on instances;
canonically $\prec\ =\ \mathrm{lex}(T, r, e)$, or $\mathrm{lex}(\kappa, r, e)$
for a declared key $\kappa$.

This is the whole conflict story, and it makes the vocabulary collapse
pleasingly:

- a **queue discipline** is a choice of $\prec$ (FIFO = arrival time, priority
  = $(\text{severity},\text{arrival})$, random service = a Philox key);
- **capacity** is $c$, and top-$k$ selection is still a commutative merge;
- **saturation** is $|F| - |W|$ per resource — a number, hence a diagnostic.

> **Side condition (independence).** $\bigcirc$ is written without an order, so
> it is well defined only if distinct winners' effects commute. The claim
> discipline is meant to discharge this statically: *claims must cover writes*.
> In v0.1 it is discharged **dynamically** — coincident writes to one cell abort
> the tick (`detect_double_writes`). Closing that gap is the natural
> type-level obligation for the Lean layer, and the honest way to state today's
> position is: the algebra is partial, and the runtime detects its domain.

**Ideal versus executed.** The $\Delta t \to 0$ limit of the above is the jump
chain of the CTMC with generator $Q = \sum_r \lambda_r$; the tick map is its
τ-leap, with local error $O(\Delta t^2)$. $\Delta t$ is therefore a *semantic*
parameter of the model, on the same footing as θ, not a performance knob.

---

## 5. Wiring

Two connective constructs, and they differ by exactly one delay.

$$\textbf{alias}\ =\ \mathrm{id} \qquad\qquad \textbf{wire}\ =\ D$$

An **alias** (a port exposure) renames a child boundary at the parent boundary:
identity on streams, zero delay. A **wire** carries a mailbox and therefore one
tick of delay, $D$.

Given inner boxes $b_1,\dots,b_n$ and a wiring $W$ — a function assigning each
inner input port and each outer output port a source among the inner outputs and
outer inputs — the composite box is

$$
\begin{aligned}
S &\;=\; \prod_j S_j \;\times\; \prod_{w \in W} M_w,
  \qquad M_w = \mathrm{Tbl}(A_w),\quad m^0_w = \varnothing \\[4pt]
s'_j &\;=\; u_j\bigl(s_j,\ m|_j \uplus \mathrm{in}|_j\bigr) \\[2pt]
m'_w &\;=\; y_{\mathrm{src}(w)}\bigl(s'_{\mathrm{src}(w)}\bigr) \\[2pt]
y(s,m) &\;=\; \bigl(y_{\mathrm{src}(w)}(s)\bigr)_{w\ \text{exposed}}
\end{aligned}
$$

**Theorem (closure).** The composite is again an element of
$\mathbf{Box}(I,O)$. Hence $\mathbf{Box}$ is an **algebra for the operad
$\mathcal W$ of directed wiring diagrams with delayed edges**, and *a composed
system is a box* is a theorem rather than a slogan. The linker is precisely
$\mathcal W$'s substitution map, and its correctness statement is one equation:

$$[\![ \mathrm{flatten}(d) ]\!] \;=\; [\![ d ]\!]$$

**Theorem (boundary invisibility).** Every read-to-write edge carries exactly
one $D$ — inside a box (read-old/write-new) and across a wire alike. The
*law* of a diagram therefore depends on a path only through its **$D$-count**.
Consequently: grouping boxes into a sub-box, or dissolving one, preserves the
trajectory of the surviving state **iff** it changes no path's $D$-count — which
is what aliases are for, and why they must be zero-delay. Refactoring
invariance is thus a statement about $\mathcal W$ modulo alias-collapse, and it
is checkable, not aspirational.

**Coordinates are diagram-derived.** A composite assigns each rule an
*occurrence* — the slash-joined instance path from the root — and the
**coordinate map** hashes it (`sembla-ir/src/identity.rs`, `rule_word`):

$$\iota(r) \;=\; \mathrm{H}\bigl(\text{path}(r)\ \#\ \text{name}(r)\bigr) \in \mathbb N,
\qquad \mathrm{H} = \mathrm{SHA256}_{[0,4)} \ \text{under domain \texttt{sembla.rule-word/v1}}$$

$\iota(r)$ *is* the second Philox counter word, so the entropy a rule sees
depends on where it sits in the hierarchy. This splits boundary invisibility
into two theorems that must not be conflated:

| level | determined by | invariant under |
|---|---|---|
| **law** — the distribution | $D$-counts along paths | any rewrite preserving $D$-counts |
| **path** — the realized trajectory | $D$-counts **and** $\iota$ | the above, *and* preserving occurrences |

A refactor that preserves $D$-counts but moves a declaration across a composite
boundary therefore preserves the law and changes the sample path. §11 measures
this.

**Heterogeneous fidelity.** Nothing above constrains how $u_j$ is *computed*.
A τ-leaped population box, an exactly-simulated Gillespie box, and an internally
sub-stepping ODE box are all just elements of $\mathbf{Box}(I,O)$. This is the
entire justification for the composition layer: it is the only place where
per-box scheduler choice can live without becoming a second semantics.

---

## 6. Observation as a sink

An observed model is a pair $(M, \varphi)$ with

$$\varphi : S \to V \qquad \Phi : V^{\mathbb N} \to \tilde V$$

($\varphi$ = views, per-tick projections of committed state; $\Phi$ = summaries,
folds over the tick-indexed family). Let $U$ be the erasure functor
$U(M,\varphi) = M$.

**Sink axiom.** The semantics *factors*:

$$[\![ M,\varphi ]\!] \;=\; \bigl(\Phi \circ \varphi^{\mathbb N}\bigr) \circ \mathrm{traj}\bigl(U(M,\varphi)\bigr)$$

Read it as: $\varphi$ and $\Phi$ appear only to the left of $\mathrm{traj}$, so
no arrow runs from the observation algebra back into $(\Theta, I, R, \prec, u)$.
Everything DESIGN §4.6 asserts is a corollary — enabling, filtering, or
serializing an observation cannot move state, draws, coordinates, or winners,
and two runs differing only in $\varphi$ have bitwise-equal state hashes,
because they have the same $U$.

---

## 7. Runs

$$\mathrm{Run}(M,\sigma,\theta,\mathbb A, T) \;=\; \bigl(s_t\bigr)_{t \le T},
\qquad s_0 = s^0,\quad s_{t+1} = u^{(t)}_{(\theta,\ \omega_\sigma,\ \mathbb A)}\bigl(s_t,\ \varnothing\bigr)$$

(The root box is closed: no external inputs.)

**The contract is a typing statement.** $\mathrm{Run}$ is a *function* of
$(M,\sigma,\theta,\mathbb A)$ and of nothing else. "Reproducibility is a
semantic property, not a runtime flag" says exactly that this signature has no
further arguments — no wall clock, no host path, no schedule, no attempt count.
The run manifest is the obligation to *record the left-hand side*.

**CRN is definitional.** Two runs sharing $\sigma$ share the field
$\omega_\sigma$ pointwise. So $\mathrm{Run}(M,\sigma,\theta_1)$ and
$\mathrm{Run}(M',\sigma,\theta_2)$ are automatically coupled on a common
probability space whenever their rule/entity coordinates agree; paired
counterfactuals and paired sensitivity are corollaries of the coordinate
convention, not features. The same rule applied one level up says an
experiment's per-run seed must be a function of the run's *semantic* coordinate,
never its position in the matrix — otherwise inserting a case perturbs
$\omega$ for every other case.

**Order-freedom, and where levels bite.** $u$ is built from (a) pointwise maps
over indexed families, (b) commutative-monoid reductions, (c) a choice function
under a total order. None mentions an evaluation order, so over
$\mathbb A = \mathbb R$ every schedule computes the same value. Over
$\mathbb A = \mathbb F$, (a) and (c) stay exact and only (b) is order-sensitive.
**Determinism levels are therefore precisely a choice of
$\textstyle\sum_{\mathbb F}$ canonicalization**, and the combinatorial content
$(F, W)$ is level-invariant exactly as long as reduction order does not perturb
a $\lambda_r$ across the $T_{r,e} < \Delta t$ boundary or a $\prec$-comparison.
Stating the caveat is the point: it names the only place a level can change a
result.

---

## 8. The equational theory

What can be proved at this altitude, without ever opening a query:

1. **Aliases collapse.** A chain of exposures is an exposure and still carries
   no $D$: $\mathrm{alias} \circ \mathrm{alias} = \mathrm{alias}$, with
   $\mathrm{alias} = \mathrm{id}$ on streams. Linked plans record the chain,
   not a composite delay (§11.7).
2. $\mathrm{wire} = D$; semantics is invariant under any diagram rewrite
   preserving $D$-counts along paths (§5).
3. $[\![ \mathrm{flatten}(\cdot) ]\!] = [\![ \cdot ]\!]$: the
   linker is an operad-substitution homomorphism.
4. $[\![ \cdot ]\!]_{\mathrm{state}} = [\![ U(\cdot) ]\!]_{\mathrm{state}}$:
   observation erasure (§6).
5. **Lumping** is a coalgebra quotient: a surjection $h : S \to S'$ with
   $u' \circ h = h \circ u$. The group-by rewrite is the search for such an $h$,
   and its correctness is that square commuting — see
   [`frontend/Sembla/Lumping.lean`](../../frontend/Sembla/Lumping.lean).
6. **Kurtz / mean-field** is *not* a homomorphism but a limit: as
   $|\mathcal E| \to \infty$ the population coalgebra converges to an ODE box.
   Agent model and compartmental model are one object at two resolutions.
7. **There are two operations, not nine.** Fill a wiring diagram with boxes,
   and substitute a filled diagram into a hole. Product is the empty diagram on
   two holes; §11's nine forms are all *shapes* of those two, never additional
   operations.

---

## 9. Compact grammar

The whole notation, dense enough to write on one line each:

$$
\begin{aligned}
M &::= \langle\, \Theta,\ \Delta t,\ \{b_i\},\ W,\ \Phi \,\rangle
  &&\text{model}\\
b &::= \langle\, S,\ s^0,\ R,\ I,\ O,\ y,\ \varphi \,\rangle
  &&\text{box}\\
r &::= \langle\, g,\ \lambda,\ \rho,\ \delta \,\rangle
  &&\text{rule}\\
W &\subseteq (\text{out ports}) \times (\text{in ports})
  &&\text{wires, each } {=}\ D\\
E &::= (\theta,\ \omega,\ \mathbb A)
  &&\text{ambient, constant}
\end{aligned}
$$

**Worked micro-example (SIR, whole model).**

$$
b_{\mathrm{pop}} :\ \varnothing \to \{\mathrm{count}\},\qquad
S = \mathrm{Tbl}(\text{health}{:}\{S,I,R\},\ \text{work}{:}\ \mathrm{Ref})
$$
$$
\begin{aligned}
r_{\mathrm{inf}} &= \bigl(\ \text{health}=S,\quad
  \beta\cdot \tfrac{\#\{I \text{ at work}\}}{\#\{\text{at work}\}},\quad
  \varnothing,\quad \text{health} \mathbin{:=} I\ \bigr)\\
r_{\mathrm{rec}} &= \bigl(\ \text{health}=I,\quad \gamma,\quad
  \varnothing,\quad \text{health} \mathbin{:=} R\ \bigr)
\end{aligned}
$$

Two rules, no claims (nothing is contested), no wires. Adding a policy box is
one more $b$ and two elements of $W$ — and by §5 that costs exactly one tick of
delay in each direction, which is the *only* semantic consequence of having
drawn the boundary there.

---

## 10. Five more models

The SIR box exercises almost none of the algebra. These five walk the ladder.
The last column is the honest part.

| # | Model | Domain | Stresses | Status |
|---|---|---|---|---|
| 1 | Decay chain | physics | nothing — the floor | runs (`examples/radioactive_decay_chain.json`) |
| 2 | Forest fire | spatial ecology | aggregate hazards; the expressiveness cliff | patch form runs, stencil form does not |
| 3 | Court | justice | $\rho$, $c$, $\prec$ — and the partiality wall | **aborts as written**; runs once claimed (both verified) |
| 4 | Labour matching | economics | claims covering `Ref` writes | runs |
| 5 | Regional health | public policy | $\mathcal W$, $D$-counts, hybrid schedulers | composition path |

### 10.1 Decay chain — the floor

$$b : \varnothing \to \varnothing, \qquad
S = \mathrm{Tbl}\bigl(\text{nuclide}{:}\{\text{Parent},\text{Daughter},\text{Stable}\}\bigr)$$
$$
\begin{aligned}
r_{\text{parent}} &= (\ \text{nuclide}{=}\text{Parent},\ \ \lambda_{\text p},\ \ \varnothing,\ \
  \text{nuclide}{:=}\text{Daughter}\ )\\
r_{\text{daughter}} &= (\ \text{nuclide}{=}\text{Daughter},\ \ \lambda_{\text d},\ \ \varnothing,\ \
  \text{nuclide}{:=}\text{Stable}\ )
\end{aligned}
$$

$I = O = W = \varnothing$ and $\rho \equiv \varnothing$, so $\mathrm{sel}$ is the
identity and the tick collapses to $u = \bigcirc_F \delta$ — independent racing,
nothing else. Three things worth naming because they are *absences*: the box has
no ports at all; each $\lambda$ is a bare parameter, so no aggregate is ever
built; and $\text{Stable}$ is absorbing purely because no rule guards on it.
**Absorption is the lack of a rule, not a construct.**

### 10.2 Forest fire — where the abstraction hides a cliff

$$S = \mathrm{Tbl}\bigl(\text{state}{:}\{E,T,B\},\ \
\text{patch}{:}\mathrm{Ref}\ \mathrm{Patch}\bigr)$$
$$
\begin{aligned}
r_{\text{ignite}} &= (\ \text{state}{=}T,\ \ \alpha + \beta\, n_B(e),\ \
  \varnothing,\ \ \text{state}{:=}B\ )\\
r_{\text{burn}} &= (\ \text{state}{=}B,\ \ \mu,\ \ \varnothing,\ \
  \text{state}{:=}E\ )\\
r_{\text{grow}} &= (\ \text{state}{=}E,\ \ \gamma,\ \ \varnothing,\ \
  \text{state}{:=}T\ )
\end{aligned}
$$

with $n_B(e) = \#\{\, e' : \mathrm{patch}(e') = \mathrm{patch}(e),\
\mathrm{state}(e') = B \,\}$. Still $\rho \equiv \varnothing$; the only new
content is that $\lambda_{\text{ignite}}$ reads an aggregate.

**The cliff.** $n_B$ as written is a single equijoin on one declared foreign key
— burning cells *sharing a patch*, i.e. mean-field within a patch — and that
runs (`sembla-runtime/src/eval.rs`, `build_aggregate`). A true four-neighbour stencil
wants $n_B(e) = \#\{e' \in \mathcal N(e) : \dots\}$ over a lattice adjacency:
four joins, where v0.1 has one.

Note *where* the restriction lives. Not in $\lambda_r$ — the algebra lets that
be any function of $(s,\mathrm{in})$ — but in the expression language *inside*
$\lambda_r$. This is the abstraction earning its keep and costing something in
the same breath: §4 cannot see the cliff, which is exactly why the cliff needs
documenting somewhere else.

### 10.3 Court — contention, and the partiality wall

$$S = \mathrm{Tbl}\bigl(\mathrm{Case}: \text{stage}{:}\{W,H,D\},\
\text{sev}{:}\mathbb Z,\ \text{arr}{:}\mathbb R,\
\text{judge}{:}\mathrm{Ref}\ \mathrm{Judge}\bigr) \times \mathrm{Tbl}(\mathrm{Judge})$$
$$
\begin{aligned}
r_{\text{hear}} &= (\ \text{stage}{=}W,\ \ \eta,\ \ \{\,\text{judge}(e)\,\},\ \
  \text{stage}{:=}H\ ),
  &&\prec\ =\ \mathrm{lex}(-\text{sev},\ \text{arr},\ r,\ e)\\
r_{\text{settle}} &= (\ \text{stage}{=}W,\ \ \zeta,\ \ \varnothing,\ \
  \text{stage}{:=}D\ )\\
r_{\text{decide}} &= (\ \text{stage}{=}H,\ \ \vartheta,\ \ \varnothing,\ \
  \text{stage}{:=}D\ )
\end{aligned}
$$

with $c(\text{judge}) = k$.

**The queue discipline is the whole of $\prec$.** Priority-then-FIFO as written;
plain FIFO is $\mathrm{lex}(\text{arr},r,e)$; random service is
$\mathrm{lex}(\omega(t,r,e,1),r,e)$. Nothing else in the model moves. And
$|F| - |W|$ per judge is the saturation number — at $c \equiv 1$ it is precisely
the τ-leap's one-event-per-resource-per-tick bias, made visible rather than
silent.

**As written, this model does not run.** $r_{\text{hear}}$ and
$r_{\text{settle}}$ share the guard $\text{stage}{=}W$ and both write
$\text{stage}$, and neither claims anything that would separate them. On a row
where both land in $F$ we have
$\delta_{\text{hear}} \circ \delta_{\text{settle}} \neq \delta_{\text{settle}} \circ \delta_{\text{hear}}$
and $\bigcirc$ is undefined. That is the §4 side condition, hit by roughly the
second model anyone would write in this domain. Verified with a minimal
analogue — a second $S \to I$ exit bolted onto `sis_importation` — where
`validate` **accepts** the model and the run aborts at tick 1 with
`double write to epidemic.person.health[0] by transition 'infect' (rule 0) and
transition 'import' (rule 2)`.

> **The identity claim — available, but not the default.** In the ideal CTMC,
> competing exits from one state resolve by argmin *automatically* — that is
> what racing clocks are. In the τ-leap they resolve only if something is
> claimed. The fix in the algebra is the **identity claim**
> $\rho_r(e) \ni \mathrm{id}(e)$: every competing exit claims one resource the
> row uniquely owns, $\mathrm{sel}$ takes the argmin, and the within-tick race
> is recovered.
>
> **This is writable today**, as a modelling pattern rather than a language
> feature: add a companion resource table and a `Ref` column pointing each row
> at its own resource row, then have every competing exit contest that column.
> `crates/sembla-cli/tests/fixtures/contest_competing_exits.json` is exactly
> that — `exit_a` and `exit_b` share a guard, both write `occupancy`, and both
> contest `self_attr slot_resource` under `race_time`. It validates, runs, and
> reports saturation. The demographic model uses the same idiom across
> `die_young` / `die_adult` / `die_old` / `emigrate` / `internal_depart`.
>
> So the gap is not expressiveness but **defaulting**:
> $\rho_r \supseteq \mathrm{wr}(\delta_r)$ ought to hold by construction, and
> instead it is something a modeller must remember. Forgetting it is statically
> accepted and fatal at tick 1.

### 10.4 Labour matching — claims that cover writes

$$S = \mathrm{Tbl}\bigl(\mathrm{Worker}: \text{status}{:}\{\text{seeking},\text{employed}\},\
\text{emp}{:}\mathrm{Ref}\ \mathrm{Firm},\ \text{target}{:}\mathrm{Ref}\ \mathrm{Firm}\bigr)
\times \mathrm{Tbl}(\mathrm{Firm})$$
$$
\begin{aligned}
r_{\text{hire}} &= (\ \text{status}{=}\text{seeking},\ \ \nu,\ \
  \{\,\text{target}(e)\,\},\ \ \text{emp}{:=}\text{target}(e);\ \text{status}{:=}\text{employed}\ )\\
r_{\text{quit}} &= (\ \text{status}{=}\text{employed},\ \ \chi,\ \ \varnothing,\ \
  \text{status}{:=}\text{seeking}\ )
\end{aligned}
$$

Note the shape of $\delta_{\text{hire}}$: v0.1 effects write **expressions over
the firing row only** — there is no binder that searches for a firm. So the firm
a worker may take is carried on the row as $\text{target}$, and matching is a
race for it. That is not a stylistic choice; it is the only encoding the effect
language admits.

The claim and the write are then the *same object*:
$\rho_{\text{hire}} = \{\text{target}(e)\}$ and $\delta_{\text{hire}}$ writes
$\text{target}(e)$. This is the one place v0.1 enforces
coverage **statically** — a `SetAttr` to a `Ref` attribute is rejected unless
some claim's resource is structurally equal to the written value
(`sembla-ir/src/validate.rs`).

So 10.4 is the case the checker was built for and 10.3 is the case it does not
cover. Stated sharply: **v0.1 types claims against `Ref` writes, not against
effects in general** — which is exactly why 10.3's two Enum writes slipped
through. Guards here are disjoint so nothing double-writes, and two workers
sharing a $\text{target}$ is a genuine contest that $\mathrm{sel}$ settles by
$\prec$, deferring the loser a tick.

### 10.5 Regional health — the operad doing work

Three regions, each a composite; one national policy box.

$$\mathrm{Region}_i = \bigl[\ \mathrm{Pop}_i \xrightarrow{\ \text{cases}\ }
\mathrm{Clinic}_i\ \bigr],
\qquad
H = \bigl[\ \{\mathrm{Region}_i\}_{i \le 3},\ \mathrm{Nat}\ \bigr]$$

wired $\mathrm{Region}_i.\text{load} \to \mathrm{Nat}.\text{load}_i$ and
$\mathrm{Nat}.\text{restr} \to \mathrm{Region}_i.\text{restr}$, with
$\mathrm{Clinic}_i.\text{load} \rightsquigarrow \mathrm{Region}_i.\text{load}$
and $\mathrm{Region}_i.\text{restr} \rightsquigarrow \mathrm{Pop}_i.\text{restr}$
as **aliases**.

$\mathrm{Nat}$ needs three *distinct* inbound ports
$\text{load}_1,\text{load}_2,\text{load}_3$ rather than one shared port, and that
is forced, not stylistic: fan-in is rejected (§11.5), so aggregating the regions
is $\mathrm{Nat}$'s own job and its merge is written in $\lambda$ where a reader
can see it. The outbound direction is unconstrained — one $\text{restr}$ output
fans out to all three (§11.4).

Now §5 does arithmetic — count $D$s along paths:

| path | wires | aliases | delay |
|---|---|---|---|
| $\mathrm{Pop}_i \to \mathrm{Clinic}_i$ | 1 | 0 | 1 tick |
| $\mathrm{Pop}_i \to \mathrm{Clinic}_i \to \mathrm{Nat}$ | 2 | 1 | 2 ticks |
| $\mathrm{Nat} \to \mathrm{Pop}_i$ | 1 | 1 | 1 tick |
| policy round trip | 3 | 2 | **3 ticks** |

No unfolding and no simulation: exposures contribute nothing, wires contribute
one each. Merging $\mathrm{Pop}_i$ and $\mathrm{Clinic}_i$ into one box
**deletes a $D$** and is therefore a different model, not a tidier one.

Dissolving $\mathrm{Region}_i$ into $H$ is the subtler case, and it is **not**
free. Its boundary is aliases, so $D$-counts survive and the *law* is unchanged;
but the occurrence path of every rule inside it shortens from
`occ:region_i/pop` to `occ:pop`, so $\iota$ moves and every draw changes. Same
distribution, different sample path — measured in §11.9.

Finally, $u_{\mathrm{Pop}_i}$ may be τ-leaped on GPU while
$u_{\mathrm{Clinic}_i}$ runs exact-sequential on CPU. §5's closure theorem never
inspects how $u_j$ is computed, so the hybrid costs no new semantics — only a
manifest recording which backend ran which box.

---

## 11. Composition, case by case

The source language offers four constructs — `instance`, `wire`, `expose`,
`hide` — and by §8.7 only two operations underlie them. What follows is the
complete list of *shapes* those operations take, including one that is
deliberately absent.

| # | Form | Source | Algebra | $D$ | v0.1 |
|---|---|---|---|---|---|
| 11.1 | product | two `instance`, unwired | $\otimes$ | — | yes |
| 11.2 | series | `wire` | $\triangleright$ | $+1$ | yes |
| 11.3 | feedback | `wire` closing a cycle | trace | $+1$ each | yes |
| 11.4 | fan-out | one output, many `wire` | copy $\Delta$ | $+1$ each | yes |
| 11.5 | fan-in | two `wire` to one input | merge $\nabla$ | — | **rejected** |
| 11.6 | unit / counit | unwired input; `hide` | $\varnothing$ / discard $\varepsilon$ | — | yes |
| 11.7 | alias | `expose` | $\mathrm{id}$ | $0$ | yes |
| 11.8 | nesting | composite `instance` | operad substitution | $0$ | yes, well-founded |
| 11.9 | replication | repeated `instance` | symmetry — **broken by $\iota$** | $0$ | yes |

Two structural constraints frame the table. Wires are typed by **schema
equality**, not compatibility (`validate_wire`), so $\otimes$ and $\triangleright$
are partial operations on interfaces. And **cycles in the wiring graph are
allowed while cycles in the nesting relation are not**: feedback is the point of
§11.3, whereas a component containing itself is rejected as
`.recursiveDefinition` (`Composition/Link.lean`). The operad is over a
well-founded tree of definitions whose wires may loop freely.

### 11.1 Product — non-interference is bitwise

$$b_1 \otimes b_2 = \bigl(S_1 \times S_2,\ (u_1 \times u_2),\ (y_1 \times y_2)\bigr),
\qquad W = \varnothing$$

Two instances, no wires. Because $u$ is a pairing and $\iota$ is per-occurrence,
the factors cannot see each other **and cannot perturb each other's draws**. The
composition guide documents exactly this contrast: `compare
solo_population.plan.json independent_epidemic_policy.plan.json` gives
population views and firing trajectories that are *exactly* equal, not merely
statistically similar. Product is the degenerate wiring diagram, and
non-interference is a consequence of $\iota$ being occurrence-keyed, not a test
that happened to pass.

### 11.2 Series — the unit of delay

$$b_1 \triangleright b_2 : \qquad m'_w = y_1(s'_1), \qquad u_2 \text{ reads } m_w$$

One wire, one $D$. Everything else in §5 is bookkeeping on top of this.

### 11.3 Feedback — where Moore-ness is load-bearing

$$\mathrm{tr}(b): \quad \mathrm{Pop} \xrightarrow{\ \text{cases}\ } \mathrm{Policy}
\xrightarrow{\ \text{restriction}\ } \mathrm{Pop}$$

A cycle in $W$. In a Mealy setting this would demand a fixpoint $x = f(x)$ —
possibly unsolvable, possibly non-unique. Under the Moore condition each $y_j$
reads only stored state, so the loop is *already* broken and the trace is total.
This is the single technical reason feedback needs no side conditions, and why
§3's Moore clause was called load-bearing.

Cost: 2 wires = 2 ticks round trip. The repo pins this at a fixed seed —
restriction thresholds 500 and 1000 give identical population columns at ticks 0
and 1, with the first difference at tick 2, after both wires have carried the
counterfactual.

### 11.4 Fan-out — copy is free, and delayed

$$y_{\mathrm{Nat}}(s) \longrightarrow \{\,\mathrm{Region}_1.\text{restr},\
\mathrm{Region}_2.\text{restr},\ \mathrm{Region}_3.\text{restr}\,\}$$

One output may source many wires; each carries the same table and its own $D$.
This is the comonoid copy $\Delta$, and it is exact — the table is a value, so
there is no aliasing question to answer. Validation constrains only the `to`
side, so fan-out is unrestricted, and `demo_national_network` exercises it:
`east/north/population.infection_count` feeds both `dashboard.east_north_cases`
and `east/coordinator.north_cases`. Surveillance and control tap the same signal
without interacting.

### 11.5 Fan-in — the operation that is missing on purpose

Two wires into one input are **rejected**:
`multiple wires target input '<box>.<port>'`. So $W$ is a *function* from inputs
to sources, which is what §5 assumed.

Why it must be rejected is the interesting part. A merge
$\nabla : \mathrm{Tbl}(A) \times \mathrm{Tbl}(A) \to \mathrm{Tbl}(A)$ needs
either a canonical order (which §2 discarded — tables are indexed, not ordered)
or a commutative monoid on tables: union? multiset sum? keyed upsert? Those are
*different models*, and the choice would be invisible in the wiring diagram.
**Fan-in is absent because the merge is a semantic decision that a wire cannot
carry.** The honest encoding makes it explicit: give the receiver two input
ports and let its own $\lambda$ or $y$ combine them, where the monoid is written
down and reviewable.

### 11.6 Unit and counit — the empty table and `hide`

An **unwired input** is not an error; it is the empty table, and $\varnothing$ is
the monoidal unit (`build_next_inputs` seeds every port empty and only wired
ports are overwritten). A **hidden output** is never built at all, since only
wired outputs are constructed.

Both are semantically free, and for the same reason as observation (§6): outputs
are pure functions of committed state and consume no coordinates, so building one
or not cannot move $F$, $W$, or $\omega$. `hide` is to the interface what erasure
$U$ is to observation — a projection the meaning function ignores.

### 11.7 Alias — the only zero-delay connective

$$\mathrm{Clinic}_i.\text{load} \rightsquigarrow \mathrm{Region}_i.\text{load}$$

`expose` renames a child boundary at the parent boundary. It contributes no
mailbox and no $D$, which is exactly why §5's path arithmetic works: aliases are
identities and drop out of the count. In a linked plan they appear as a
`boundary` entry recording the *chain* of exposures, e.g.
`["expose:regional_infection_count", "expose:infection_count"]` — a two-level
rename that is still zero delay.

### 11.8 Nesting — substitution, and the flatten equation

A composite instantiated inside a composite. The linker's `flatten` is the
operad's substitution map, and its correctness is
$[\![ \mathrm{flatten}(d) ]\!] = [\![ d ]\!]$ (§5). Crucially the linker
*records* occurrence chains as it flattens, so `flatten` preserves $\iota$ **by
construction** and is bitwise-faithful.

Hand-refactoring is the opposite case: moving a declaration to a different level
yields a different $\iota$. That is why the composition guide calls such edits
*identity-changing migrations even when the visible scientific structure looks
similar* — and it is measured next.

### 11.9 Replication, and how $\iota$ breaks the symmetry

Three instances of one component are three distinct occurrence paths, hence three
distinct rule words, hence three independent draw streams. Good — identical
regions should not share shocks. But the same mechanism means **the diagram's
symmetry group does not act on $\omega$**: permuting instance *IDs* permutes
draws, even though permuting display labels or declaration order does not.

Three checked-in fixtures make the whole $\iota$ story a measurement rather than
an argument. Rule word for the population box's `infect` transition:

| plan | wires | occurrence | rule word |
|---|---|---|---|
| `independent_epidemic_policy` | 0 | `occ:population` | 2501600445 |
| `epidemic_policy` | 2 | `occ:population` | **2501600445** |
| `regional_response` | 2 | `occ:epidemic/population` | 3914077761 |

Read across the first two rows: **adding or removing wires does not move
coordinates.** Read down to the third: **adding one level of nesting does.** So
the CRN *coupling* survives rewiring and survives adding sibling boxes under
$\otimes$, but not re-nesting. That is the practical rule, and the sharp form of
§7's caveat: paired counterfactuals are exact only between models whose
occurrence chains agree.

> **Two invariance claims that are easy to conflate.** "Moving a box boundary
> never changes observable semantics" (DESIGN §4.4) is a statement about the
> **law**; it survives any rewrite preserving $D$-counts, aliases included. It
> does **not** survive re-nesting at the level of the **sample path**, because
> $\iota$ is occurrence-keyed — same distribution, different bytes, different
> state hash. This has teeth for the run contract (§7): a re-nested model is a
> *different* run under the same $(\sigma,\theta)$, and no amount of alias
> discipline recovers its hashes. Refactoring is free scientifically and never
> free reproducibly.

---

## 12. What kind of algebra composition is

§11 lists the forms; this section says what they *are*. The classification does
real work — three entries in that table are each forced by a single structural
fact, not chosen.

### 12.1 Two levels: a syntax operad, a semantics algebra

Composition has a syntax and a semantics, and identifying them is the source of
most confusion about delay.

- **Syntax.** $\mathcal W$, the coloured operad of directed wiring diagrams.
  Colours are interfaces; an $n$-ary operation is a diagram with $n$ holes and a
  boundary; operadic composition is **substitution** into a hole.
- **Semantics.** $\mathbf{Box}$ carries a $\mathcal W$-algebra structure: each
  $n$-hole diagram induces $\prod_i \mathbf{Box}(I_i,O_i) \to \mathbf{Box}(I,O)$,
  namely §5's formula.

Closure (§5) *is* the statement that this map lands back in $\mathbf{Box}$. The
linker realises substitution and `flatten` computes the normal form. Every entry
in §11 is then one of three things: an operation of $\mathcal W$ (11.1–11.7), a
substitution (11.8–11.9), or absent (11.5).

### 12.2 The wire category is cartesian — settling fan-out and fan-in together

Interfaces and wires form a symmetric monoidal category
$(\mathcal I, \otimes, \varnothing)$. What decides §11.4 and §11.5 is the
structure the objects carry.

Every $\mathrm{Tbl}(A)$ has a canonical **cocommutative comonoid**

$$\Delta : \mathrm{Tbl}(A) \to \mathrm{Tbl}(A) \otimes \mathrm{Tbl}(A),
\qquad \varepsilon : \mathrm{Tbl}(A) \to \varnothing$$

— duplication and discard, satisfying coassociativity, cocommutativity and
counitality. This is automatic rather than designed: a table is a *value*, so
copying is definable and no morphism can distinguish a copy from the original.
Hence, in one stroke:

- **fan-out is free and needs no declaration** (§11.4) — $\Delta$ is natural;
- **`hide` and unwired outputs are free** (§11.6) — $\varepsilon$ is natural;
- an unwired input is the monoidal unit $\varnothing$.

What does *not* exist is the dual. There is no canonical

$$\nabla : \mathrm{Tbl}(A) \otimes \mathrm{Tbl}(A) \to \mathrm{Tbl}(A)$$

because a merge must choose — union, multiset sum, keyed upsert, left-biased
override — and these are different monoids, some not even commutative.

> **The classification.** The wire category is **cartesian**: copy and discard
> are free and natural. It is **not cocartesian**: there is no canonical merge.
> Fan-out and fan-in are therefore not symmetric options implemented
> asymmetrically by accident — they sit on opposite sides of a structural
> asymmetry in the category of tables, and §11.5's rejection is the only honest
> reading of it.

### 12.3 Feedback, not trace: where yanking fails

Give every morphism a delay degree

$$\deg(\mathrm{alias}) = 0, \qquad \deg(\mathrm{wire}) = 1,
\qquad \deg(g \circ f) = \deg f + \deg g$$

so $\mathcal I$ is graded by $(\mathbb N, +)$. A loop is **guarded** when its
degree is $\ge 1$, and in Sembla every loop is guarded: every wire is a mailbox,
and §3's Moore condition forbids a degree-zero cycle. Guardedness supplies the
fixpoint for free — the loop equation is already solved by the previous tick's
mailbox, so there is nothing to solve. **Totality of feedback is a corollary of
the grading, not an extra axiom.**

But this is *not* a traced monoidal category. Naturality, dinaturality,
vanishing and superposition all hold. **Yanking fails:**

$$\mathrm{Tr}(\sigma) \;=\; D \;\neq\; \mathrm{id}$$

Feeding a wire straight back gives one tick of delay, not the identity. The
structure keeping every trace axiom except yanking, with a distinguished delay
object, is what the literature calls a **feedback category**
(Katis–Sabadini–Walters); Sembla's composition layer is one, and the mailbox is
exactly the object that breaks yanking.

Worth saying out loud, because the failure of yanking *is* the one-tick delay
people trip over. It is not an implementation artefact to be optimised away
later: recovering a genuine trace would require solving fixpoints, hence Mealy
boxes, hence losing §3's totality.

### 12.4 What the grading classifies

Boundary invisibility (§5), restated: the law-semantics factors through $\deg$.
Two diagrams whose every source-to-sink path carries equal degree have equal law,
and aliases are precisely the degree-zero generators — which is why they are the
only safe refactoring tool.

> **No composition-preserving rewrite can remove a delay.** Any rewrite lowering
> a path's degree changes the law. "Merge two boxes to cut latency" is not an
> optimisation; it is a different model.

### 12.5 Recursion and feedback need different fixpoints

§11 records that cycles in the wiring graph are allowed and cycles in the nesting
relation are not. The theory says why, and it is not arbitrary.

| | cycle in | needs a fixpoint over | provided by |
|---|---|---|---|
| **feedback** | the diagram $W$ | **streams** — mailbox contents | $D$: guarded, hence unique |
| **recursion** | the definition tree | **diagrams** — the unfolding itself | nothing |

A component containing itself denotes an infinite unfolding: you would need a
colimit of $\mathcal W$-terms, and the composite state
$S = \prod_j S_j \times \prod_w M_w$ would be an infinite product. Delay cannot
help, because the cycle is in the *syntax*, not in time. So $\mathcal W$ must be
free on a well-founded set of definitions, and `.recursiveDefinition` is that
well-foundedness check.

**Loops in time are fine; loops in structure are not.**

### 12.6 Two semantics, and where flattening is faithful

Write $T$ for an authored operad term and $|T|$ for its normal form under
substitution — the flat diagram. Then

$$[\![ - ]\!]_{\mathrm{law}} \ \text{factors through } |T|,
\qquad\quad [\![ - ]\!]_{\mathrm{path}} \ \text{does not.}$$

The path-semantics depends on $\iota$, which hashes occurrence paths: data of the
*term*, not of its normal form. Two consequences that look contradictory until
the levels are separated:

- **`flatten` is faithful** (§11.8). The linker records occurrences as it
  normalises, carrying $T$'s data through, so $\iota$ survives by construction.
- **Hand-refactoring is not.** A different term with the same normal form has the
  same law and different bytes.

Stated once: $\mathbf{Box}$ is a $\mathcal W$-algebra at the level of laws, and
only a **free-term** algebra at the level of realisations. §11.9's rule-word
table is that sentence, measured.

### 12.7 What the theory does not include

Four absences, each a real boundary rather than a gap awaiting code:

1. **No coproduct.** There is no "either $b_1$ or $b_2$" — no branching, no
   conditional wiring. The algebra has $\otimes$ and substitution and nothing
   else (§8.7).
2. **No dependent or dynamic wiring.** $W$ is fixed before the run; a model
   cannot rewire itself, and instance counts are static. Replication is $n$
   substitutions written out, not a parametrised family.
3. **One clock.** $\Delta t$ is a field of the *model*, not of a box, and
   components do not declare it. Composition is therefore strictly synchronous:
   **there is no multi-rate composition**, and wiring a slow box to a fast one
   means both move to the finer $\Delta t$. Internal sub-stepping — an ODE box
   integrating within a tick — is invisible to the algebra because it lives
   inside $u_j$, but the *exposed* rate is global.
4. **One scheduler domain, so far.** The plan format carries a partition of
   leaves into scheduler domains, but every fixture populates exactly one:
   `domain:global` under `tau_leap`. §5's heterogeneous-fidelity claim is a
   property of the algebra — $\mathbf{Box}$ never inspects how $u_j$ computes —
   and is so far unexercised in the artefacts.

Item 3 is the sharpest. It is easy to read DESIGN §4.4's "boxes may run different
schedulers on different hardware" as multi-rate composition. It is not: the tick
barrier is global, and the operad has exactly one clock.

---

## 13. Limits, colimits, and why composition is not a pushout

"Can I glue two boxes along a shared boundary?" has a definite answer — **no** —
but every construction the question invokes is present, one level down. Sorting
out which lives where is worth doing, because the two traditions are easy to
conflate and they make opposite trades.

### 13.1 Two traditions for composing open systems

| | **Structured / decorated cospans** | **Wiring diagrams (operad)** |
|---|---|---|
| An open system is | $L \to X \leftarrow R$ — interfaces *included into* the system | a box with typed ports; interfaces are message types |
| Composition is | **pushout** — glue along the shared boundary | fill the holes of a diagram |
| Boundary entities are | **identified** — literally the same rows | never identified — tables are copied as values |
| Composite state is | a **colimit** | a **product** |
| Associated with | Baez–Courser, Fong; Catlab's open ACSets | Spivak; Vagner–Spivak–Lerman |

Sembla uses **ACSets for state** — the same data structure as the cospan
tradition — and **wiring diagrams for composition**. That is a fork in the road,
deliberately taken, and §5's formula is the evidence: the composite state is
$\prod_j S_j \times \prod_w M_w$, a product with mailboxes bolted on, not a
quotient of a disjoint union.

### 13.2 The pushout that is not being taken

Were boxes to share a boundary $B$ with legs $B \to S_1$ and $B \to S_2$,
composition would be

$$S_1 +_B S_2 \;=\; \mathrm{colim}\bigl(S_1 \leftarrow B \to S_2\bigr)$$

and rows in the image of $B$ would afterwards be **one** row. The population
box's person 4192003 and the court box's person 4192003 would be the same
entity; a write from either box would land in the same cell.

Sembla never does this. Box states are disjoint, and cross-box reference to the
same individual travels as a **key value inside a message table**, never as
shared identity. Two consequences:

1. **No box can write another box's state.** There is nothing to contend over
   across a boundary, so §4's conflict resolution is always box-local.
2. **Composition cannot introduce a double write.** The partiality of §4 is
   box-local too — wiring two safe boxes together is always safe.

### 13.3 Sharing and delay are the same decision

Under pushout composition, glued entities *are* the same object. There is no
mailbox between them, hence nothing to delay: a write by one box is visible to
the other immediately, because it is the same cell. That is exactly what §5's
uniform one-tick rule forbids. The two are therefore not independent choices:

| Choose | Get | Lose |
|---|---|---|
| **Glue** (pushout) | shared entities, instant visibility | uniform delay, hence boundary invisibility; the state *type* now depends on the gluing |
| **Message** (product + mailbox) | movable boundaries, box-local conflicts, parallel safety | one tick per hop; cross-box identity is by key, not by identity |

Sembla's uniform delay, the order-freedom of §7, and the refusal to share state
are **one decision wearing three hats**. The tick cost is the price of the
refactoring freedom.

### 13.4 Where pullbacks actually live: inside the tick

Every aggregate declares two foreign keys into a **common** table — validation
demands "aggregate join attributes must both be Ref attributes to the same
table". That is precisely a **cospan**

$$Q \ \xrightarrow{\ q\ }\ G \ \xleftarrow{\ f\ }\ T$$

with $Q$ the querying table, $T$ the target table, $G$ their shared key target.
The aggregate is that cospan's **pullback**, followed by a dependent sum along
the projection:

$$Q \times_G T \ \longrightarrow\ Q, \qquad \mathrm{Agg} \;=\; \Delta_q\,\Sigma_f$$

In SIR: $Q = T = \mathrm{Person}$, $G = \mathrm{Employer}$, both legs the
`employer` column. The pullback is *pairs of people sharing an employer*;
$\Sigma_f$ counts the infected per employer; $\Delta_q$ has each person read the
count for their own employer.

> **Cospans and pullbacks are in Sembla — they compose *data within a tick*, not
> *boxes across a diagram*.** DESIGN §4.2's "join on declared keys only" reads,
> categorically, as: you may take the pullback of a cospan whose legs are
> declared foreign keys, and nothing else.

This makes the expressiveness cliff of §10.2 exact. **You get one pullback square
per aggregate.** A four-neighbour lattice stencil is a limit over a four-legged
diagram, so it is out of reach — not because lattices are hard, but because the
expression language admits a single span.

### 13.5 Where colimits actually live: state-space rewrites, not composition

- **Lumping** (§8.5) is a **coequalizer** — the quotient $h : S \to S'$
  collapsing states with equal aggregate rates. That the lumped chain agrees
  with the original is exactly the statement that this quotient is a coalgebra
  homomorphism.
- **Birth** would be a **pushout** along an inclusion: extending an ACSet by new
  rows glues $\varnothing \hookrightarrow \{\text{new}\}$ onto the current state.
  Births are deferred behind a flag, but that is the shape they would take — and
  it explains why deterministic ID allocation is the hard part, since the
  pushout is canonical only once the coproduct injection is pinned down.
- **Population construction** is a colimit of parts, but it happens outside the
  runtime, so the algebra never sees it.

The pattern: colimits build or quotient state *inside one box*. They never join
two boxes.

### 13.6 Which construction, where

| Construction | Appears as | Does not appear as |
|---|---|---|
| **Product** (limit) | composite state $\prod_j S_j \times \prod_w M_w$; with coalgebra homomorphisms as morphisms, the unwired $\otimes$ carries projections and is the categorical product | — |
| **Pullback** (limit) | every aggregate — one square, once per expression | more than one square per aggregate |
| **Cospan** | a declared pair of FKs into a common table | the shape of box composition |
| **Pushout** (colimit) | would be entity birth; a proposed elaboration-time schema merge would add a second use | composition of *boxes*, ever |
| **Coequalizer** (colimit) | lumping / coarse-graining | composition, ever |
| **Coproduct** | — | branching composition (§12.7) |
| **Operad substitution** | nesting — and it is *not* a (co)limit | — |

One sentence: **limits inside a tick, colimits inside a box, and neither between
boxes — between boxes there is only the operad.**

### 13.7 What would change under cospan composition

**Gained.** Genuinely shared individuals. A person could sit in the labour box
and the justice box under one identity, and cross-domain linkage — mother-linked
births, household references, paired migration — would be structural rather than
key-passing.

**Lost.** The uniform delay, hence boundary invisibility (§13.3). Also the
box-local conflict story: two boxes could claim the same row, so §4's resolution
would have to become global, which is precisely what the order-free/GPU-shape
rule exists to prevent.

**Changed.** The state space would depend on the gluing, so refactoring would
change the *type* of the state rather than only its coordinates — and §5's
law/path distinction would collapse, because both would move at once.

The trade is not exotic-versus-familiar. Shared mutable entities and order-free
parallel execution are hard to have together, and Sembla chose the second. That
choice is what §13.3 says you are paying a tick for.
