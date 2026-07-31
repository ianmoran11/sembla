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
$\llbracket A\rrbracket$, and a **table** is a finite *indexed* family

$$\mathrm{Tbl}(A) \;=\; \{\,\tau : \mathcal E \rightharpoonup \llbracket A\rrbracket \ \text{finite}\,\}$$

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

$$\llbracket \mathrm{flatten}(d) \rrbracket \;=\; \llbracket d \rrbracket$$

**Theorem (boundary invisibility).** Every read-to-write edge carries exactly
one $D$ — inside a box (read-old/write-new) and across a wire alike. The
semantics of a diagram therefore depends on a path only through its **$D$-count**.
Consequently: grouping boxes into a sub-box, or dissolving one, preserves the
trajectory of the surviving state **iff** it changes no path's $D$-count — which
is what aliases are for, and why they must be zero-delay. Refactoring
invariance is thus a statement about $\mathcal W$ modulo alias-collapse, and it
is checkable, not aspirational.

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

$$\llbracket M,\varphi \rrbracket \;=\; \bigl(\Phi \circ \varphi^{\mathbb N}\bigr) \circ \mathrm{traj}\bigl(U(M,\varphi)\bigr)$$

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

1. $\mathrm{alias} \circ \mathrm{alias} = \mathrm{alias}$; alias is the unit of
   wiring composition.
2. $\mathrm{wire} = D$; semantics is invariant under any diagram rewrite
   preserving $D$-counts along paths (§5).
3. $\llbracket \mathrm{flatten} \rrbracket = \llbracket - \rrbracket$: the
   linker is an operad-substitution homomorphism.
4. $\llbracket - \rrbracket_{\text{state}} = \llbracket U(-) \rrbracket_{\text{state}}$:
   observation erasure (§6).
5. **Lumping** is a coalgebra quotient: a surjection $h : S \to S'$ with
   $u' \circ h = h \circ u$. The group-by rewrite is the search for such an $h$,
   and its correctness is that square commuting — see
   [`frontend/Sembla/Lumping.lean`](../../frontend/Sembla/Lumping.lean).
6. **Kurtz / mean-field** is *not* a homomorphism but a limit: as
   $|\mathcal E| \to \infty$ the population coalgebra converges to an ODE box.
   Agent model and compartmental model are one object at two resolutions.
7. **Product** of boxes is the empty wiring diagram on two holes; nesting is
   substitution. There is no third composition operation.

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
