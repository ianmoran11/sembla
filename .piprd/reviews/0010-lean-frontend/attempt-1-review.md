# PRD 0010 Review — Attempt 1

## Assessment

**REVISE**

## Blocking issue

The surface elaborators validate against caller-supplied pseudo-contexts rather than the actual system/model declarations. `system%` accepts an arbitrary `targets[...]` list; `transition%` accepts independently duplicated `attrs[...]`, `params[...]`, and `inputs[...]` lists; `box%` and `model%` do not reconcile those lists with their contained tables or parameter declarations. As a result, a nonexistent reference target can compile if manually included in `targets[...]`, and an undeclared parameter can compile if manually included in `params[...]`. The checked-in models duplicate their table schemas and parameter names in these independent lists.

This means the current negative tests demonstrate failures only when a name is omitted from a manually supplied allow-list. They do not guarantee that references are declared by an actual system or that parameter expressions correspond to the model's actual `ParamDecl` block. This fails the contextual elaboration and positioned undeclared-name requirement.

## Acceptance criteria

1. **PASS:** Pinned dependency-free Lean package builds.
2. **PASS:** Exported SIR validates.
3. **PASS:** Both exported models match fixtures through validated canonical `diff-ir` comparison.
4. **PASS:** Fixed-seed fixture/export runs have identical hash output and CSV bytes.
5. **REVISE:** Four positioned diagnostics exist, but validation is against manually supplied pseudo-contexts rather than actual declarations.
6. **PASS for fixtures:** Exported params and priors match, and hazards contain symbolic parameter expressions.
7. **PASS:** Frontend documentation covers setup, DSL, export, and parity.
8. **PASS:** Repository checks work with Lean and emit a successful skip warning without Lake.

All other inspected specification requirements pass. No files were edited during review beyond these managed review artifacts.
