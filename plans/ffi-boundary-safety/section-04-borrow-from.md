---
id: "04"
title: "#borrow_from(param) — Lifetime Annotations at the Boundary"
status: not-started
reviewed: false
goal: "Let extern declarations express 'this returned pointer borrows from THIS named parameter'; enforce at compile time that the return does not outlive the parameter's scope. Integrate with AIMS via Locality::Borrowed(p) — NO new lattice dimension."
success_criteria:
  - "`#borrow_from(p)` on an extern item declares the return value's locality as Locality::Borrowed(p)."
  - "A scope analysis pass rejects (E4018) any use of the return value outside the named parameter's live range."
  - "Multi-parameter `#borrow_from(a, b)` requires the return's scope to be enclosed by the INTERSECTION of every named parameter's scope."
  - "Struct-field or array-element paths (`#borrow_from(vec.data)`) are REJECTED at parse time — only top-level parameter names allowed in this revision."
  - "`.to_owned()` is a stdlib escape hatch (`library/std/ffi/`) extending the borrow to an owned copy."
  - "E4017 fires when `#borrow_from` names a non-parameter."
  - "E4024 fires when `#borrow_from` combines with incompatible `owned`/`borrowed` return annotations."
inspired_by:
  - "Rust lifetimes: rejected as a language-level construct (approved Deep FFI explicitly avoids lifetime parameters); §04 achieves the same safety via an attribute pointing at a parameter name — no lifetime variable."
  - "Swift __bridge_retained / __bridge_transfer: similar boundary-annotation pattern."
  - "OxCaml locality modes: `local_` annotation inspired Locality::Borrowed(p) as an AIMS dimension extension."
depends_on:
  - "01"
third_party_review:
  status: none
  updated: null
---

# §04 — `#borrow_from(param)` — Lifetime Annotations

## Context

Per approved proposal §4. Extends the AIMS `Locality` dimension (`aims-rules.md §1.5`) with parameter-bound borrow tracking. Does NOT introduce a new lattice dimension — `Locality::Borrowed(ArcVarId)` parameterizes the existing enum.

## §04.1 — Lattice extension design log FIRST (gate)

Before implementation, write `plans/ffi-boundary-safety/section-04-design-log.md` answering:

- [ ] Current `Locality` has 5 values (`BlockLocal`, `FunctionLocal`, `ArgEscaping`, `HeapEscaping`, `Unknown`). `Borrowed(p)` — where does it sit in the order? Default: `Borrowed(p) ≤ FunctionLocal` (the borrow is bound to one function's parameter scope, which is strictly inside the function's lifetime).
- [ ] Join rules: `Borrowed(p) ⊔ Borrowed(q)` with `p ≠ q` → `FunctionLocal` (union of two distinct parameter scopes is no narrower than the function).
- [ ] Height bound: single `Borrowed(_)` tag; N parameters = N distinct states + 4 existing = 5 + N; L-5 requires finite height — verify per-function N is bounded (parameter count is finite per function).
- [ ] Canonicalization: interaction with CN-8 (`Access = Borrowed ∧ Locality > FunctionLocal ⟹ Locality := FunctionLocal`). `Borrowed(p) ≤ FunctionLocal`, so CN-8 does not widen it.
- [ ] Cite rule anchors verbatim (`aims-rules.md §1.5`, CN-8, IA-4).

## §04.2 — Grammar + parser

- [ ] Grammar production (ships with §08):
  ```ebnf
  extern_item_attr = "#borrow_from" "(" identifier { "," identifier } ")" | ... (existing) .
  ```
- [ ] Parser: `#borrow_from(p)` and `#borrow_from(a, b)` accepted on extern function declarations.
- [ ] Validation at parse time: each identifier MUST be a top-level parameter name of the same function. Non-parameter name → E4017. Struct-field or array-element paths (`p.field`, `v.data`) → parse-time error with "only top-level parameter names allowed."

## §04.3 — AIMS lattice extension

- [ ] Add `Locality::Borrowed(ArcVarId)` variant. ArcVarId references the parameter at the callee's IR level.
- [ ] Update `aims/lattice/` join table, partial order, canonicalization (per §04.1's design-log decisions).
- [ ] Update TF-6 (`refine(CONSERVATIVE, callee.return_contract)`): when the callee's `ReturnContract.locality` is `Borrowed(param_idx)`, the caller's result gets `Locality::Borrowed(arg_at_param_idx)`.
- [ ] Update `verify_coherence` (VF-3 oracle) to re-derive `Locality::Borrowed(p)` from realized IR.

## §04.4 — Scope analyzer

- [ ] In `ori_types/src/check/`, add a scope-outlives-check pass that runs after AIMS analysis for FFI-returned borrowed values.
- [ ] Algorithm: for every SSA variable with `Locality::Borrowed(p)`, compute its lexical scope. For every USE of the variable, verify `p` is in scope at that use site. If not → E4018.
- [ ] Multi-parameter: for `#borrow_from(a, b)`, the return's scope must be enclosed by `intersection(scope(a), scope(b))`. Any use outside that intersection → E4018.
- [ ] Test-specific implementation note: scope analysis uses the existing `CanExpr` scope graph (each `Let` / `Match` arm / `Block` introduces a scope); no new CFG machinery.

## §04.5 — `.to_owned()` escape hatch

- [ ] Add `@to_owned()` method on the `BorrowedView<T, ParamName>` opaque type. Implementation: forces a copy into Ori-managed memory (existing approved-Deep-FFI `borrowed str` copy semantics).
- [ ] Method emits an eager copy; AIMS sees the result as `Owned` with `Uniqueness::Unique`, unbinding it from the parameter scope.
- [ ] Signature: `@to_owned (self) -> T uses FFI("lib")` — capability-parameterized per the calling extern block.

## §04.6 — Stdlib glue

- [ ] Add `type BorrowedView<T, ParamName>` to `library/std/ffi/borrow.ori` (NEW file, co-created with §07's `library/std/ffi/` module).
- [ ] The compiler auto-wraps returns annotated with `#borrow_from` as `BorrowedView`; the user interacts via `.to_owned()` or direct use (compiler tracks the Locality::Borrowed(p)).

## §04.7 — Matrix tests

- [ ] **Failing tests FIRST:** a program where a `#borrow_from(stmt)`-annotated return is used after `stmt` drops — MUST emit E4018.
- [ ] **Matrix:**
  - **Scope pattern:** return used BEFORE param scope ends (OK) / return used AT param scope exit (OK) / return used AFTER param scope exit (E4018) / return stored in a longer-scoped variable (E4018) = 4 cells.
  - **Parameter position:** first / middle / last positional param = 3 cells.
  - **Multi-param:** `#borrow_from(a, b)` with `a` scope ending first (use after = E4018) / `b` scope ending first (use after = E4018) / both scopes encompassing use (OK) = 3 cells.
  - **Invalid combos:** `owned` + `#borrow_from(p)` → E4024; `borrowed` + `#borrow_from(p)` → OK (refines borrowed's default); `#borrow_from(nonparam)` → E4017; `#borrow_from(p.field)` → parse error = 4 cells.
  - **.to_owned() escape:** calling `.to_owned()` extends the lifetime → use after source scope exit OK after copy = 1 cell.
- [ ] **Semantic pin:** a `sqlite3_column_text(stmt).to_owned()` pattern compiles clean AND produces a copy (FileCheck for memcpy in LLVM IR); without `.to_owned()`, compiles clean only if used within `stmt`'s scope.
- [ ] **Negative pin:** canonical outlive pattern — `let text = { let stmt = ...; column_text(stmt) };` MUST emit E4018.

## §04.8 — AIMS rule documentation

- [ ] Update `.claude/rules/aims-rules.md §1.5` Locality table to include `Borrowed(p)` with its ordering, join rules, canonicalization interactions. Cite §04.3 for the lattice change.
- [ ] Update `.claude/rules/arc.md` if the Locality dimension surface requires a §"Dimensions in flight" note.

## Completion Checklist

- [ ] §04.1 design log signed off.
- [ ] §04.2 grammar + parser lands.
- [ ] §04.3 Locality lattice extension lands with join/partial-order/canonicalization tests (cargo test -p ori_arc -- lattice::prop_tests).
- [ ] §04.4 scope analyzer lands with E4018 emission.
- [ ] §04.5 `.to_owned()` works across extern "c" and extern "js" blocks.
- [ ] §04.6 stdlib glue in place.
- [ ] §04.7 matrix tests green; semantic pin green; negative pin green.
- [ ] §04.8 aims-rules.md Locality table updated.
- [ ] `./test-all.sh`, `./clippy-all.sh`, `./llvm-test.sh` green.
- [ ] VF-3 oracle cross-check passes on `#borrow_from`-annotated programs.
- [ ] `/tpr-review` clean; `/impl-hygiene-review` clean.
- [ ] `/improve-tooling` + `/sync-claude` sweeps.
- [ ] `sections[id=04].status: complete`, `reviewed: true`.

## §04.R — Third Party Review Findings

- None.
