---
id: "05"
title: "Callback Capability Propagation"
status: not-started
reviewed: false
goal: "Let callback parameter types declare the capabilities the C runtime will provide; verify at compile time that the registered Ori closure uses ONLY those capabilities. Convert runtime capability-missing failures into compile-time E4019 errors at the registration site."
success_criteria:
  - "Callback parameter types accept `uses Cap1, Cap2` in their function signature."
  - "Missing `uses` on a callback parameter = NO capabilities available (strictest default)."
  - "At registration, the Ori function's `uses` list MUST be a subset of the callback parameter's `uses` list — violation → E4019."
  - "Capability-providing libraries propagate via `with Cap = handler in` at registration, integrating with the existing with...in mechanism."
  - "The whole section is gated on `capability-propagation-completion-proposal.md`'s implementation landing FIRST."
inspired_by:
  - "Koka effect propagation: effect rows on callback types with subset enforcement."
  - "Rust `Send`/`Sync` bounds on callbacks: analogous subset discipline at registration."
  - "OCaml effect handlers: effects as types with structural subtyping."
depends_on:
  - "01"
blocked_by:
  - "capability-propagation-completion-proposal.md — LLVM/AOT capability lowering, stateful handler completion, marker enforcement"
third_party_review:
  status: none
  updated: null
---

# §05 — Callback Capability Propagation

## Context

Per approved proposal §5. **BLOCKED**: cannot begin implementation until `capability-propagation-completion-proposal.md` ships. That draft documents three infrastructure gaps (LLVM/AOT capability support stubbed, stateful handlers partial, marker capability enforcement partial) — any one of them would make §05's compile-time verification unsound.

When the blocker clears, the following subsections execute.

## §05.1 — Unblock verification

- [ ] Verify `capability-propagation-completion` has landed:
  - [ ] LLVM/AOT capability lowering is shipped (check `compiler/ori_llvm/` for `uses` handling).
  - [ ] Stateful `handler(state: S) { ... }` fully implemented in eval + LLVM.
  - [ ] Marker capability E1250 propagation enforced at the type-check layer.
- [ ] If any gap remains, STOP; file a bug per `CLAUDE.md §Bug Handling — ZERO DEFERRAL` and escalate via `AskUserQuestion`.

## §05.2 — Grammar + parser

- [ ] Grammar production (ships with §08):
  ```ebnf
  callback_type = "(" param_list ")" "->" type [ "uses" cap_list ] .
  ```
- [ ] Parser accepts `uses` on callback parameter function types (currently parsed but not propagated).
- [ ] AST: callback parameter types carry an optional `cap_list` field.

## §05.3 — Subset verification at registration

- [ ] In `ori_types/src/check/capabilities/` (new file or extension to existing module), add `fn check_callback_subset(registered_fn: &FunctionSig, callback_param: &CallbackType) -> Result<(), Diagnostic>`.
- [ ] Rule: `registered_fn.uses` ⊆ `callback_param.uses`. Set difference → E4019 listing the missing capabilities on the callback parameter.
- [ ] Applies to every call site that passes an Ori function to a callback parameter. Detected via call-argument type-checking.

## §05.4 — Handler-based capability provision at registration

- [ ] When the C library provides capabilities via a callback argument (e.g., `uv_timer_start` passes a handle that exposes `Clock`), the user can write:
  ```ori
  with Clock = uv_handle_clock(handle) in {
      uv_timer_start(handle: handle, cb: on_tick, ...)
  }
  ```
- [ ] The existing `with Cap = handler in` mechanism handles this — §05 does not add new machinery, only validates the subset check against the with-in-provided scope.

## §05.5 — Matrix tests

- [ ] **Failing tests FIRST:** a program registering a callback that uses `Print` when the callback parameter type declares only `uses Clock` → MUST emit E4019.
- [ ] **Matrix:**
  - **Subset position:** `uses {} ⊆ uses {Clock}` (OK) / `uses {Clock} ⊆ uses {Clock}` (OK) / `uses {Clock, Logger} ⊆ uses {Clock}` (E4019 — Logger missing) / `uses {Clock} ⊆ uses {}` (E4019 — all missing) = 4 cells.
  - **Callback parameter forms:** explicit callback type with `uses` / callback type without `uses` (strictest default) / extern function returning a callback type (propagation) = 3 cells.
  - **with...in provision:** capability provided via with-in AT the call site → subset check passes; capability NOT provided → E4019 = 2 cells.
- [ ] **Semantic pin:** a compilable `uv_timer_start` example with `on_tick uses Clock` and a matching callback parameter compiles clean AND emits the callback registration through the capability-aware lowering (FileCheck for the correct LLVM IR shape once capability-propagation-completion ships).
- [ ] **Negative pin:** any cell where the subset fails MUST emit E4019 at compile time.

## §05.6 — Documentation

- [ ] Update spec §26-ffi.md with §05 surface (co-commit with §08).
- [ ] Update `.claude/rules/ori-syntax.md` Deep FFI section with callback `uses` propagation.

## Completion Checklist

- [ ] §05.1 capability-propagation-completion blocker verified cleared.
- [ ] §05.2 grammar + parser lands.
- [ ] §05.3 subset verification lands with E4019 emission.
- [ ] §05.4 with...in integration verified.
- [ ] §05.5 matrix tests green; semantic pin green; negative pin green.
- [ ] §05.6 spec + `ori-syntax.md` committed.
- [ ] `./test-all.sh`, `./clippy-all.sh`, `./llvm-test.sh` green.
- [ ] `/tpr-review` clean; `/impl-hygiene-review` clean.
- [ ] `/improve-tooling` + `/sync-claude` sweeps.
- [ ] `sections[id=05].status: complete`, `reviewed: true`.

## §05.R — Third Party Review Findings

- None.
