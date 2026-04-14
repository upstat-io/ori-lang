---
section: "02"
title: "Validator Module — validate_body_types()"
status: not-started
reviewed: false
goal: >
  Introduce ori_types::check::validators, a new public submodule of the check
  crate, containing validate_body_types() — the producer-side enforcement of
  typeck.md PC-2 ("no Tag::Var in any type-bearing IR position"). The function
  walks body expr_types after InferEngine body-checking completes, identifies
  any surviving unbound Tag::Var per expression, and emits E2005 (AmbiguousType)
  for each one. Section 03 will thread the call into the bodies pass; Sections
  01 and 02 are independent and can land in either order.
success_criteria:
  - "ori_types::check::validators is declared as pub mod in check/mod.rs"
  - "lib.rs promotes mod check to pub mod check"
  - "validate_body_types() exists with the exact signature specified in §02.1"
  - "TF-5 gate: HAS_VAR flag short-circuits the walk for trivially clean types"
  - "HAS_ERROR cascade suppression: types flagged HAS_ERROR are skipped"
  - "Scheme-aware walk: Tag::BoundVar inside a scheme is not flagged; unbound Tag::Var is"
  - "Tag-dispatch child recursion covers all compound tags (no pool.children() call)"
  - "Diagnostics are emitted in ExprIndex order (pass determinism, impl-hygiene.md §Pass determinism)"
  - "Unit tests in check/validators/tests.rs cover all five matrix cells"
  - "cargo test -p ori_types passes with the new module present"
inspired_by:
  - "typeck.md §PC-2 — Output Contract (no Tag::Var in typed IR)"
  - "typeck.md §DI-1 — E2005 AmbiguousType"
  - "types.md §TF-5 — Fast-Path Gates"
  - "types.md §TK-3 — Error Tag as Poison"
  - "types.md §SC-1 — Scheme Layout"
  - "impl-hygiene.md §Pass Composition — Pass determinism"
  - "impl-hygiene.md §Cross-Phase Invariant Contracts (Type Checker → Codegen)"
  - "check/object_safety.rs — closest existing validation pattern"
  - "infer/context.rs:64 — take_errors() idiom"
depends_on: []
third_party_review: pending
sections:
  - "02.1 — Validator signature and public contract"
  - "02.2 — Core algorithm: tagged child dispatch and scheme-aware walk"
  - "02.3 — lib.rs and check/mod.rs wiring"
  - "02.4 — Unit test matrix"
---

# Section 02 — Validator Module: `validate_body_types()`

## Context

Section 01 (value restriction) and this section are independent. Both feed into
Section 03, which threads the new producer-side enforcement into the bodies pass.

The type checker's output contract (typeck.md PC-2) requires that no `Tag::Var`
survive in any type position of the typed IR. Today this contract is documented
but not enforced at the producer boundary — the only enforcement that exists is
downstream (consumers `debug_assert!` on entry). This section introduces the
missing producer-side validator so that violations are caught inside `ori_types`
with a proper E2005 diagnostic, before the IR is handed off to eval, ARC, and
codegen.

The validator lives in a new public submodule `ori_types::check::validators`
modelled on the existing `check/object_safety.rs` pattern: a standalone public
function that accepts a context abstraction trait, walks type positions
systematically, and accumulates diagnostics rather than bailing. Section 03 will
call it from `check/bodies/mod.rs` after `engine.take_errors()` drains the
inference accumulator.

---

## 02.1 — Validator Signature and Public Contract

### 02.1.1 — New files

- `compiler/ori_types/src/check/validators/mod.rs` — module root; exports
  `validate_body_types`
- `compiler/ori_types/src/check/validators/tests.rs` — unit tests (§02.4)

### 02.1.2 — Canonical signature

```rust
/// Validate that no unbound `Tag::Var` survives in `expr_types` after body
/// inference completes, enforcing typeck.md PC-2 ("no Tag::Var in any
/// type-bearing IR position").
///
/// For every `(ExprIndex, Idx)` pair in `expr_types`, this function performs a
/// recursive walk of the type tree. When an unbound `Tag::Var` is encountered
/// that is not bound by an enclosing `Tag::Scheme`, it emits
/// `TypeCheckError::ambiguous_type` (E2005) against the expression's span.
///
/// Diagnostics are emitted in ascending `ExprIndex` order (impl-hygiene.md
/// §Pass determinism).
///
/// Fast paths (impl-hygiene.md §TF-5):
/// - A type with `!TypeFlags::HAS_VAR` is trivially clean; the walk is skipped.
/// - A type with `TypeFlags::HAS_ERROR` is poisoned; the walk is skipped to
///   avoid cascade diagnostics (types.md §TK-3, impl-hygiene.md §Error Recovery
///   Monotonicity).
///
/// # Parameters
/// - `pool` — the type pool for tag/data/flags queries
/// - `expr_types` — map from expression index to resolved `Idx`; typically
///   the `InferOutput::expr_types` field populated during body inference
/// - `span_of` — function mapping an `ExprIndex` to the source `Span` for
///   diagnostic attribution
/// - `errors` — mutable accumulator; new `TypeCheckError` values are pushed here
pub fn validate_body_types(
    pool: &Pool,
    expr_types: &FxHashMap<ExprIndex, Idx>,
    span_of: &dyn Fn(ExprIndex) -> Span,
    errors: &mut Vec<TypeCheckError>,
) {
    // Collect entries sorted by ExprIndex for deterministic diagnostic order
    // (impl-hygiene.md §Pass determinism — FxHashMap iteration is non-deterministic).
    let mut entries: Vec<(ExprIndex, Idx)> = expr_types.iter()
        .map(|(&k, &v)| (k, v))
        .collect();
    entries.sort_unstable_by_key(|(idx, _)| *idx);

    for (expr_idx, ty) in entries {
        // TF-5 fast-path gate: skip types that cannot contain unbound vars.
        let flags = pool.flags(ty);
        if !flags.contains(TypeFlags::HAS_VAR) {
            continue;
        }
        // HAS_ERROR cascade suppression (types.md §TK-3).
        if flags.contains(TypeFlags::HAS_ERROR) {
            continue;
        }

        collect_unbound_vars(pool, ty, &[], errors, span_of(expr_idx));
    }
}
```

### 02.1.3 — Imports

The module needs:

```rust
use rustc_hash::FxHashMap;

use crate::{
    ExprIndex,
    Pool,
    TypeCheckError,
    TypeFlags,
};
use ori_ir::Span;
use ori_types::idx::Idx;   // or crate::idx::Idx depending on module path
```

Adjust paths to match the existing import conventions in `check/object_safety.rs`.

---

## 02.2 — Core Algorithm: Tagged Child Dispatch and Scheme-Aware Walk

### 02.2.1 — Scheme-aware walk helper

The pool has no `pool.children(ty)` method. Children are accessed via tag-specific
accessors on `Pool` (see `pool/accessors.rs`). The walk must dispatch by tag.

**Verified divergence from user instructions**: the instructions listed a
hypothetical `pool.children(ty)` call. The real API requires tag dispatch. The
walk is also affected by:
- `pool.data(ty)` returns `u32` — for `Tag::Var`, this IS the `var_id` (not a
  separate `pool.var_id()` accessor, which does not exist).
- `pool.scheme_vars(idx)` returns `&[u32]` (a slice), NOT `Vec<u32>` as the
  instructions stated.

```rust
/// Recursively walk the type tree rooted at `ty`, collecting each unbound
/// `Tag::Var` that is not bound by an enclosing `Tag::Scheme`.
///
/// `bound_vars` is the set of var_ids declared by enclosing schemes; any
/// `Tag::Var` whose var_id appears in this set is a bound variable inside a
/// scheme and SHALL NOT be flagged (types.md §SC-1).
///
/// An unbound var that is also `VarState::Generalized` is acceptable — it
/// was generalized during an enclosing let-binding and should not be flagged.
/// Only `VarState::Unbound` surviving vars are PC-2 violations.
fn collect_unbound_vars(
    pool: &Pool,
    ty: Idx,
    bound_vars: &[u32],
    errors: &mut Vec<TypeCheckError>,
    span: Span,
) {
    // TF-5 inner guard: if no HAS_VAR, no work to do.
    if !pool.flags(ty).contains(TypeFlags::HAS_VAR) {
        return;
    }

    match pool.tag(ty) {
        // --- Type variables ---
        Tag::Var => {
            let var_id = pool.data(ty); // u32 — the var_id (verified: pool.data())
            // Check if this var is bound by an enclosing scheme.
            if bound_vars.contains(&var_id) {
                return;
            }
            // Only Unbound vars are PC-2 violations. Generalized vars are fine.
            match pool.var_state(var_id) {
                VarState::Unbound { .. } => {
                    errors.push(TypeCheckError::ambiguous_type(
                        span,
                        var_id,
                        "expression".to_string(),
                    ));
                }
                VarState::Link { target } => {
                    // Follow the link chain.
                    collect_unbound_vars(pool, *target, bound_vars, errors, span);
                }
                VarState::Generalized { .. } | VarState::Rigid { .. } => {
                    // Generalized or rigid: not an ambiguous-type violation.
                }
            }
        }

        // --- Scheme: extend bound_vars with this scheme's declared vars ---
        Tag::Scheme => {
            // Spec: types.md §SC-1 — scheme extra = [var_count, var_id_1, ..., body_idx]
            let scheme_vars: &[u32] = pool.scheme_vars(ty); // returns &[u32]
            let body: Idx = pool.scheme_body(ty);
            // Merge with any outer bound_vars.
            let mut new_bound: Vec<u32> = bound_vars.to_vec();
            new_bound.extend_from_slice(scheme_vars);
            collect_unbound_vars(pool, body, &new_bound, errors, span);
        }

        // --- Simple containers (single child in data field) ---
        Tag::List | Tag::Option | Tag::Set | Tag::Range
        | Tag::Iterator | Tag::DoubleEndedIterator => {
            let child = Idx::from_raw(pool.data(ty));
            collect_unbound_vars(pool, child, bound_vars, errors, span);
        }

        // --- Two-child containers ---
        Tag::Map => {
            collect_unbound_vars(pool, pool.map_key(ty), bound_vars, errors, span);
            collect_unbound_vars(pool, pool.map_value(ty), bound_vars, errors, span);
        }
        Tag::Result => {
            collect_unbound_vars(pool, pool.result_ok(ty), bound_vars, errors, span);
            collect_unbound_vars(pool, pool.result_err(ty), bound_vars, errors, span);
        }

        // --- Function: params + return ---
        Tag::Function => {
            for param in pool.function_params(ty) {
                collect_unbound_vars(pool, param, bound_vars, errors, span);
            }
            collect_unbound_vars(pool, pool.function_return(ty), bound_vars, errors, span);
        }

        // --- Tuple: all elements ---
        Tag::Tuple => {
            for elem in pool.tuple_elems(ty) {
                collect_unbound_vars(pool, elem, bound_vars, errors, span);
            }
        }

        // --- Primitives, Error, Named, BoundVar, RigidVar — no walk needed ---
        // Primitives have no children (TK-1 range 0..16).
        // Tag::Error: HAS_ERROR would have short-circuited above.
        // Tag::Named, Tag::Applied, Tag::Alias, Tag::Struct, Tag::Enum:
        //   these should not carry HAS_VAR in a well-formed post-inference IR
        //   (they resolve to concrete types). If they did somehow have HAS_VAR,
        //   we'd need to recurse into their fields. For now, skip to be safe —
        //   struct/enum field types are separate Idx entries in the pool checked
        //   independently when the field expressions are typed.
        // Tag::BoundVar, Tag::RigidVar: not unbound vars.
        _ => {}
    }
}
```

### 02.2.2 — Implementation notes

**`Idx::from_raw(u32)`**: verify this constructor exists in `ori_types/src/idx/`.
If the constructor is private or named differently, use whatever the real API is.
For simple-container tags, `data` IS the child's `raw()` value (types.md §TY-4,
Appendix B: Tag-to-Data Decoding Table, "data = child_idx.raw()"). So
`Idx::from_raw(pool.data(ty))` is the correct construction.

**`pool.function_params(ty) -> Vec<Idx>`** and
**`pool.tuple_elems(ty) -> Vec<Idx>`**: these return owned `Vec<Idx>` per the
verified API surface in `pool/accessors.rs`. The walk clones no pool data.

**`VarState` import**: `VarState` is in `ori_types::pool` or `ori_types::unify`.
Check the existing import pattern in `check/object_safety.rs` or the pool module.

**Recursion depth**: the walk is bounded by the pool's structural depth, which
is bounded by type complexity in real programs. For pathologically deep generic
types, consider adding a depth counter (max 256, per impl-hygiene.md §Panic &
Assertion). For the initial implementation, omit the counter and add it as a
follow-up if needed — but track it as a potential hardening item.

---

## 02.3 — lib.rs and check/mod.rs Wiring

### 02.3.1 — Promote `mod check` to `pub mod check` in lib.rs

**File**: `compiler/ori_types/src/lib.rs`

Current state (verified):
```rust
// Line 16:
mod check;
// Line 25:
pub mod reporting;  // precedent for the promotion
```

Change:
```rust
// Line 16:
pub mod check;
```

**Why**: Section 03 will call `validate_body_types` from within the `check`
submodule (same crate), so `pub` is not strictly required for that call. However,
the validator is also useful for driver-level verification passes and downstream
testing. The `reporting` module at line 25 is already `pub` — this follows the
same pattern.

### 02.3.2 — Add `pub mod validators` to check/mod.rs

**File**: `compiler/ori_types/src/check/mod.rs`

Current state: private submodule declarations for `accessors`, `api`, `bodies`,
`exports`, `imports`, `object_safety`, `registration`, `scope`, `signatures`,
`well_known` plus cfg-test `mod integration_tests` and `mod tests`.

Add after the existing `mod object_safety;` declaration (alphabetical ordering
within the private mods; validators comes after signatures):

```rust
pub mod validators;
```

The declaration is `pub` so that the bodies pass (same crate, different module)
and external consumers can access `check::validators::validate_body_types`
directly.

### 02.3.3 — Add `#[cfg(test)] mod tests;` in validators/mod.rs

At the bottom of `validators/mod.rs`:

```rust
#[cfg(test)] mod tests;
```

Body in `validators/tests.rs`.

---

## 02.4 — Unit Test Matrix

Five test cells cover the essential axes:

| Cell | Scenario | Expected |
|------|----------|----------|
| T1 | Fresh unbound `Tag::Var` in expr_types | E2005 emitted, var_id recorded |
| T2 | Fully-resolved `int` type (no HAS_VAR) | No errors (TF-5 gate fires) |
| T3 | Type containing `Tag::Error` (HAS_ERROR) | No errors (cascade suppression) |
| T4 | `Tag::Var` under a `Tag::Scheme` whose vars list includes that var_id | No errors (bound variable) |
| T5 | `Tag::Var` captured from outer scope inside a scheme (var_id NOT in scheme vars) | E2005 emitted |

These cells pin the two fast-path gates (T2, T3) and the scheme-aware distinction
(T4 vs T5), plus the base case (T1).

### 02.4.1 — Test scaffolding pattern

Tests construct a minimal `Pool`, allocate vars and types directly, build a
synthetic `expr_types` map, invoke `validate_body_types`, and assert on the
emitted `errors` vec. The `span_of` closure can return `Span::new(0, 1)` for all
expressions in unit tests.

```rust
// In validators/tests.rs:

use crate::{Pool, TypeCheckError, TypeFlags};
use crate::check::validators::validate_body_types;
use ori_ir::Span;
use rustc_hash::FxHashMap;

/// Unbound var in expr_types produces one E2005 diagnostic.
#[test]
fn validate_body_types_with_unbound_var_emits_ambiguous_type() {
    let mut pool = Pool::new();
    let var_idx = pool.fresh_var_for_test(); // allocate a Var with Unbound state
    let mut expr_types = FxHashMap::default();
    expr_types.insert(0usize, var_idx);
    let mut errors = Vec::new();
    validate_body_types(&pool, &expr_types, &|_| Span::new(0, 1), &mut errors);
    assert_eq!(errors.len(), 1);
    // Verify the error is E2005.
    assert!(matches!(
        errors[0].kind(),
        crate::TypeErrorKind::AmbiguousType { .. }
    ));
}

/// Fully resolved type (int) produces no diagnostics — TF-5 HAS_VAR gate fires.
#[test]
fn validate_body_types_with_resolved_int_produces_no_errors() {
    let pool = Pool::new();
    let mut expr_types = FxHashMap::default();
    expr_types.insert(0usize, crate::idx::Idx::INT);
    let mut errors = Vec::new();
    validate_body_types(&pool, &expr_types, &|_| Span::new(0, 1), &mut errors);
    assert!(errors.is_empty());
}

/// Type flagged HAS_ERROR is skipped — cascade suppression.
#[test]
fn validate_body_types_with_error_type_produces_no_errors() {
    let pool = Pool::new();
    let mut expr_types = FxHashMap::default();
    expr_types.insert(0usize, crate::idx::Idx::ERROR);
    let mut errors = Vec::new();
    validate_body_types(&pool, &expr_types, &|_| Span::new(0, 1), &mut errors);
    assert!(errors.is_empty());
}

/// Var bound inside a scheme is not an ambiguous type — it's a scheme parameter.
#[test]
fn validate_body_types_with_var_bound_inside_scheme_produces_no_errors() {
    let mut pool = Pool::new();
    // Build: Scheme([var_id], Var(var_id)) — a trivial ∀α. α scheme.
    let scheme_idx = pool.scheme_with_bound_var_for_test();
    let mut expr_types = FxHashMap::default();
    expr_types.insert(0usize, scheme_idx);
    let mut errors = Vec::new();
    validate_body_types(&pool, &expr_types, &|_| Span::new(0, 1), &mut errors);
    assert!(errors.is_empty());
}

/// Var captured from outer scope inside a scheme (not in scheme's vars list)
/// is an ambiguous type.
#[test]
fn validate_body_types_with_outer_var_inside_scheme_emits_ambiguous_type() {
    let mut pool = Pool::new();
    // Build: outer_var is Unbound; scheme has a DIFFERENT var as its bound var;
    // scheme body references outer_var (not bound by this scheme).
    let scheme_idx = pool.scheme_with_escaped_var_for_test();
    let mut expr_types = FxHashMap::default();
    expr_types.insert(0usize, scheme_idx);
    let mut errors = Vec::new();
    validate_body_types(&pool, &expr_types, &|_| Span::new(0, 1), &mut errors);
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind(),
        crate::TypeErrorKind::AmbiguousType { .. }
    ));
}
```

### 02.4.2 — Test helper methods on Pool

The tests above use `pool.fresh_var_for_test()`,
`pool.scheme_with_bound_var_for_test()`, and
`pool.scheme_with_escaped_var_for_test()`. These are `#[cfg(test)]` helpers on
`Pool` (or free functions in `validators/tests.rs` that call the real pool API
directly to construct the types). Prefer implementing them directly in
`validators/tests.rs` using the real pool construction API rather than adding
test helpers on `Pool` itself — that keeps the test surface local to the
validator module and avoids polluting the `Pool` public interface.

Inspect `pool/mod.rs` and `pool/construct/` to identify the lowest-level
construction calls that can build a `Tag::Var` with `VarState::Unbound` and a
`Tag::Scheme` with a specific `vars` list.

---

## Close-out Checklist

At section completion, before marking `status: complete`:

- [ ] `cargo test -p ori_types` passes (no regressions)
- [ ] `cargo clippy -p ori_types` clean (no new warnings)
- [ ] `/tpr-review` run on this section — findings resolved
- [ ] `/impl-hygiene-review` run — no new LEAK / DRIFT / GAP findings
- [ ] `diagnostics/repo-hygiene.sh --check` clean (no untracked temp files)

---

## 02.R — Open Findings

*Populated by TPR; empty at authoring time.*

---

## 02.N — Completion Notes

*Populated at close-out.*

---

## API Divergences from User Instructions

The following divergences from the instructions were identified by reading the
actual source files. The plan above uses the verified real API throughout:

1. **`pool.children(ty)` does not exist.** The instructions listed this as the
   child-recursion API. The real pool has tag-specific accessors only. The walk
   in §02.2 dispatches by `pool.tag(ty)` and calls the appropriate accessor per
   tag (`pool.list_elem()`, `pool.function_params()`, etc.).

2. **`pool.var_id(ty)` does not exist.** The instructions listed `pool.var_id(ty)`
   as the accessor for the var_id of a `Tag::Var`. The real API is `pool.data(ty)`
   which returns the `u32` var_id for any tag (for `Tag::Var`, `data` IS the
   var_id per types.md Appendix B).

3. **`pool.scheme_vars(idx)` returns `&[u32]`, not `Vec<u32>`.** The instructions
   said `Vec<u32>`. The real signature (verified at `pool/accessors.rs:267`) is
   `pub fn scheme_vars(&self, idx: Idx) -> &[u32]`. The walk uses `.contains()`
   and `extend_from_slice()` on a slice, not Vec methods.

4. **`TypeCheckError::ambiguous_type` is confirmed.** The instructions flagged
   this as "possibly hypothetical". It is real: `check_error/mod.rs:236` has
   `pub fn ambiguous_type(span: Span, var_id: u32, context_desc: String) -> Self`.

5. **`lib.rs:16` has `mod check;` (private), not `pub mod check;`.** The section
   §02.3.1 adds the `pub` promotion. The `pub mod reporting;` at line 25 is the
   precedent.

6. **`ExprIndex` is a type alias `usize`**, not a newtype (verified at
   `infer/mod.rs:56`). The `FxHashMap<ExprIndex, Idx>` uses `usize` keys; sorting
   by `*idx` works directly.
