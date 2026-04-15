---
section: "04"
title: "Codegen Defense-in-Depth Assertions"
status: not-started
reviewed: false
goal: "Insert `debug_assert!`-guarded calls to `assert_no_unresolved_type_vars` at the four codegen integration sites so that any `Tag::Var` surviving the typeck → ARC → codegen boundary is caught immediately at the codegen seam, with a clear ICE and diagnostic rather than silent LLVM IR corruption."
success_criteria:
  - "New module `compiler/ori_arc/src/ir/validate.rs` exists and exports `pub fn assert_no_unresolved_type_vars(pool: &ori_types::Pool, func: &ArcFunction) -> Result<(), String>` — verifiable via `grep -rn 'pub fn assert_no_unresolved_type_vars' compiler/ori_arc/src/ir/validate.rs` returning exactly one hit."
  - "`compiler/ori_arc/src/ir/mod.rs` declares `pub mod validate;` — verifiable via `grep -n 'pub mod validate' compiler/ori_arc/src/ir/mod.rs` returning one hit."
  - "Integration site 1 (prepare_all_cached): `compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs` contains a `debug_assert!` call to `assert_no_unresolved_type_vars` for the primary function before `self.prepare_arc_function(...)` — verifiable via `grep -n 'assert_no_unresolved_type_vars' compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs` returning at least two hits (one for the main function, one for each lambda)."
  - "Integration site 2 (prepare_mono_cached): the same file contains `assert_no_unresolved_type_vars` for the monomorphized function — same grep returns additional hits."
  - "Integration site 3 (JIT pre-mono, compile_all_functions): `compiler/ori_llvm/src/evaluator/compile.rs` contains `assert_no_unresolved_type_vars` calls inserted in the mono-function loop at lines ~230-236 — verifiable via `grep -n 'assert_no_unresolved_type_vars' compiler/ori_llvm/src/evaluator/compile.rs` returning at least one hit."
  - "Integration site 4 (AOT pre-mono, codegen_pipeline.rs): `compiler/oric/src/commands/codegen_pipeline.rs` contains `assert_no_unresolved_type_vars` calls in the mono loop — verifiable via `grep -n 'assert_no_unresolved_type_vars' compiler/oric/src/commands/codegen_pipeline.rs` returning at least one hit."
  - "Release-path tracing: every call-site has a companion `tracing::error!` that fires (without the `debug_assert!`) in release builds when a `Tag::Var` is detected, and returns the ICE result up the call stack — verifiable via `grep -n 'tracing::error!' compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs` returning hits and the ICE path returning `Err` or calling `self.builder.record_codegen_error()`."
  - "`timeout 150 ./test-all.sh` is green after landing (debug and release). Since Section 03 has not yet landed, the `debug_assert!` guards must not fire on any currently-valid program — verifiable by confirming green CI before and after Section 04."
inspired_by:
  - "Rust `rustc_middle::mir::visit::TyContext` — every MIR visitor receives the type context and `debug_assert!`s that types are fully resolved at traversal boundaries; the pattern here mirrors that per-function pre-emission gate."
  - "Swift `SILVerifier` — the Swift compiler runs a multi-checkpoint IR verifier that includes ownership and type checks before and after SIL optimization passes; Section 04 is a single-checkpoint analog for the ARC IR → LLVM IR handoff."
  - "Koka `Core.Check` — Koka's backend verifies that no `TVar` escapes into the final core IR before monomorphisation; the `assert_no_unresolved_type_vars` helper is a direct structural equivalent."
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "New `ori_arc::ir::validate` module"
    status: not-started
  - id: "04.2"
    title: "Integration site 1 & 2 — `prepare_all_cached` / `prepare_mono_cached`"
    status: not-started
  - id: "04.3"
    title: "Integration site 3 — JIT `compile_all_functions` mono-function loop"
    status: not-started
  - id: "04.TPR"
    title: "TPR checkpoint after 04.3"
    status: not-started
  - id: "04.4"
    title: "Integration site 4 — AOT `codegen_pipeline.rs` mono-function loop"
    status: not-started
  - id: "04.5"
    title: "Unit tests for `assert_no_unresolved_type_vars`"
    status: not-started
  - id: "04.R"
    title: "Close-out"
    status: not-started
  - id: "04.N"
    title: "Completion checklist"
    status: not-started
---

## Context — Why This Section Exists

Sections 01–03 of this plan form the **producer side** of the typeck PC-2 phase contract
(`impl-hygiene.md §Cross-Phase Invariant Contracts`):

| Section | Producer-side responsibility |
|---------|------------------------------|
| 01 | Stop empty-list `Tag::Var` from being generalized in the first place |
| 02 | Add a validator module in `ori_types` that detects surviving `Tag::Var`s and emits E2005 |
| 03 | Wire the validator into the bodies pass so every function body is checked before ARC |

Section 04 is the **consumer side** — a defense-in-depth backstop at the codegen seam.
`codegen-rules.md §VR-1` mandates per-function LLVM IR verification after emission. The
assertion added here is the analogous gate one step earlier: before the ARC function is
handed to the two-pass declare/define pipeline, verify that no `Tag::Var` index is present
in `ArcFunction.var_types`. If one is, something upstream (either the typeck bodies pass or
the ARC lowerer itself) violated the `impl-hygiene.md §Cross-Phase Invariant Contracts` row:

> Type Checker → Codegen | All type variables resolved | No `Idx` with `Tag::Var` in typed IR

`codegen-rules.md §TR-2` states this invariant directly:

> All type indices SHALL be fully resolved via `pool.resolve_fully(idx)` before LLVM type
> construction. Unresolved type variables (`Tag::Var`) SHALL NOT reach codegen — their
> presence is a type checker bug.

The assertion added in this section makes that invariant **self-enforcing** in debug builds
and **tracing-visible** in release builds, so any regression that lets a `Tag::Var` slip
through is caught immediately with a clear function name and variable index instead of a
cryptic LLVM verification failure.

### Why Section 04 Depends on Section 03

The assertions added here are correct only if the producer side has fixed all legitimately-
typed programs. Before Section 03 lands, the bodies pass does not yet call the validator,
so empty-list `Tag::Var`s that are valid program constructs (e.g. `let x = []` where the
element type is resolved later by an argument to the same function) may still survive into
the ARC IR. Enabling the `debug_assert!` before Section 03 lands would produce spurious
assertion failures on such programs.

The dependency is therefore load-bearing, not organizational: **do not merge Section 04
before Section 03 is merged and all existing spec tests are green.**

---

## 04.1 — New `ori_arc::ir::validate` Module

### Motivation

The assertion helper must live in `ori_arc` rather than `ori_llvm` because:

1. The check is about the **ARC IR** (`ArcFunction.var_types: Vec<Idx>`), which is owned by
   `ori_arc`. Placing validation logic in `ori_arc` keeps the cross-phase invariant with its
   owner crate.
2. `ori_llvm` is downstream of `ori_arc` in the dependency graph; a function in `ori_arc`
   can be called from both `ori_llvm` sites (JIT and AOT) without introducing new
   cross-crate dependencies.
3. Future AIMS verification passes that need the same check (e.g., the `ORI_VERIFY_ARC=1`
   path) can call the same function.

### File to Create

**`compiler/ori_arc/src/ir/validate.rs`** — new file. Target: ~60 lines (excluding the
`tests` declaration).

### Implementation

```rust
//! Validation utilities for ARC IR correctness.
//!
//! Provides post-lowering checks that enforce cross-phase invariant contracts
//! before the ARC IR is handed to LLVM codegen.
//!
//! # Cross-Phase Invariant
//!
//! Per `impl-hygiene.md §Cross-Phase Invariant Contracts`:
//!
//! > Type Checker → Codegen | All type variables resolved |
//! > No `Idx` with `Tag::Var` in typed IR
//!
//! Per `codegen-rules.md §TR-2`:
//!
//! > All type indices SHALL be fully resolved via `pool.resolve_fully(idx)`
//! > before LLVM type construction. Unresolved type variables (`Tag::Var`)
//! > SHALL NOT reach codegen.
//!
//! The functions in this module make that invariant self-enforcing.

use ori_ir::Name;
use ori_types::{Idx, Pool, Tag};

use crate::ir::{ArcFunction, ArcVarId};

/// Check that no variable in `func.var_types` is an unresolved `Tag::Var`.
///
/// Returns `Ok(())` when the invariant holds. Returns `Err(String)` with a
/// diagnostic message that includes the function name and the first offending
/// variable index when the invariant is violated.
///
/// # When to Call
///
/// Call this immediately before handing the `ArcFunction` to the LLVM codegen
/// pipeline. Callers typically wrap this in `debug_assert!` for the fast path
/// and emit `tracing::error!` on the slow / release path.
///
/// # Relationship to Section 03
///
/// This check is a consumer-side backstop. The producer-side enforcement lives
/// in `ori_types::check::validator` (Section 03 of the
/// `empty-container-typeck-phase-contract` plan). Both must be present for
/// full defense-in-depth.
pub fn assert_no_unresolved_type_vars(
    pool: &Pool,
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
) -> Result<(), String> {
    for (raw_idx, &ty) in func.var_types.iter().enumerate() {
        if is_tag_var(pool, ty) {
            let var_id = ArcVarId::from_raw(raw_idx as u32);
            let fn_name = interner.lookup(func.name);
            return Err(format!(
                "Tag::Var reached codegen: function `{fn_name}`, \
                 ArcVarId({raw}) has unresolved type index {ty:?}. \
                 This is a typeck PC-2 contract violation \
                 (impl-hygiene.md §Cross-Phase Invariant Contracts, \
                 codegen-rules.md §TR-2).",
                raw = var_id.raw(),
                ty = ty,
            ));
        }
    }
    Ok(())
}

/// Returns `true` when `idx` resolves to `Tag::Var` after full resolution.
///
/// Uses `pool.tag(idx)` rather than `pool.resolve_fully(idx)` because
/// `var_types` entries are already-lowered indices — we want to know if the
/// stored index IS a `Tag::Var`, not whether it resolves through aliases to
/// one.
#[inline]
fn is_tag_var(pool: &Pool, idx: Idx) -> bool {
    matches!(pool.tag(idx), Tag::Var(_))
}

#[cfg(test)]
mod tests;
```

### Wire Into `ori_arc::ir::mod.rs`

Add `pub mod validate;` to `compiler/ori_arc/src/ir/mod.rs`. The existing `mod.rs` already
declares submodules; add the new one in alphabetical order with the other public submodules.
The `ArcVarId::from_raw` constructor may not exist — check the existing API and either add
it or use `ArcVarId(raw_idx as u32)` directly (the struct is `#[repr(transparent)]` over
`u32`).

Verify the actual `ArcVarId` API before writing:

```
compiler/ori_arc/src/ir/mod.rs lines 64–100 — see ArcVarId::raw() (line 88),
ArcVarId::index() (line 87). Add from_raw(u32) -> Self if missing.
```

### Re-Export From `ori_arc`

Add to `compiler/ori_arc/src/lib.rs`:

```rust
pub use ir::validate::assert_no_unresolved_type_vars;
```

This makes the call sites in `ori_llvm` and `oric` as clean as
`ori_arc::assert_no_unresolved_type_vars(...)` without needing the full path.

### Dependency on `StringInterner`

The diagnostic message includes the function name. `ArcFunction.name` is a `Name` (an
interned string index). Rendering it requires an `&StringInterner`. All call sites in
`ori_llvm` have an `interner` field on `FunctionCompiler` (`self.interner`). The AOT call
site in `codegen_pipeline.rs` has `interner` as a parameter. The signature must include the
interner to produce a useful error message.

**Alternative**: if threading the interner is problematic for any call site, omit the name
and only report the raw `Name` index. The diagnostic is less friendly but still identifies
the bug. Prefer the full-name version.

---

## 04.2 — Integration Sites 1 & 2: `prepare_all_cached` / `prepare_mono_cached`

These are the primary per-function seams in the two-pass declare/define pipeline. Both
functions remove `(arc_func, lambdas)` from `arc_cache` and then pass them to
`self.prepare_arc_function(...)` which runs the full AIMS pipeline. The check must fire
BEFORE `prepare_arc_function` receives the function, so any surviving `Tag::Var` is
surfaced with the original ARC IR rather than after the AIMS pass has mutated it.

### File

`compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs`

### Verified Signature: `prepare_all_cached` (lines 22–88)

```rust
pub fn prepare_all_cached(
    &mut self,
    module_functions: &[Function],
    function_sigs: &[FunctionSig],
    canon: &CanonResult,
    arc_cache: &mut FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
) -> Vec<PreparedFunction>
```

The `arc_func` comes from `arc_cache.remove(&func.name)` at line 59. The call to
`self.prepare_arc_function(func.name, func_id, &abi, arc_func, lambdas)` is at line 84.
`self.interner` is the `StringInterner`.

### Assertion Pattern: Main Function

Insert immediately before the `self.prepare_arc_function(...)` call at line 84:

```rust
// Spec: Codegen-rules.md §TR-2 — no Tag::Var in ARC IR at codegen boundary.
// Debug: hard-assert (ICE in debug builds on contract violation).
// Release: log error and record codegen failure (soft path, same observable failure
//          as LLVM verification failure but with a clearer message).
if let Err(msg) = ori_arc::assert_no_unresolved_type_vars(
    self.pool, &arc_func, self.interner,
) {
    tracing::error!(
        contract_violation = true,
        %msg,
        "Tag::Var in ARC IR violates PC-2 contract (codegen-rules.md §TR-2)"
    );
    debug_assert!(false, "{}", msg);
    self.builder.record_codegen_error();
    // Skip preparation for this function; the LLVM module will fail verification
    // which is the correct user-visible failure mode.
    continue;
}
```

### Assertion Pattern: Lambdas

Lambdas (the `Vec<ori_arc::ArcFunction>` second element of the cache pair) are also handed
to `prepare_arc_function` indirectly. Add a similar loop after the main-function assertion:

```rust
for lambda in &lambdas {
    if let Err(msg) = ori_arc::assert_no_unresolved_type_vars(
        self.pool, lambda, self.interner,
    ) {
        tracing::error!(
            contract_violation = true,
            %msg,
            "Tag::Var in lambda ARC IR violates PC-2 contract"
        );
        debug_assert!(false, "{}", msg);
        self.builder.record_codegen_error();
    }
}
```

The `record_codegen_error` on a lambda does not `continue` the outer loop — the lambda
check is informational (the main function's lambda count is a bug too, but the main function
itself may be valid). The LLVM emission for the lambda will fail at `fn_val.verify(true)`
which is the downstream gate.

### Verified Signature: `prepare_mono_cached` (lines 95–136)

```rust
pub fn prepare_mono_cached(
    &mut self,
    mono_functions: &[crate::monomorphize::MonoFunction],
    canon: &CanonResult,
    arc_cache: &mut FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
) -> Vec<PreparedFunction>
```

The `arc_func` comes from `arc_cache.remove(&mono_fn.mangled_name)` at line 115. Apply the
exact same assertion pattern as `prepare_all_cached` before the call to
`self.prepare_arc_function(...)`.

### Why `process_arc_function` Is NOT the Primary Site

`process_arc_function` at `define_phase.rs:315` runs the AIMS pipeline, which MODIFIES
`arc_func` in place. Inserting the assertion there would check the function AFTER
`lower_function_can` has already processed it. The prepare-phase seams are earlier and
check the function as it arrives from the ARC cache or from inline lowering, before any
modification.

`process_arc_function` is a valid SECONDARY site for a `debug_assert!` if future work needs
to verify the invariant holds after AIMS realization, but Section 04 does not add assertions
there to keep the diff minimal and the dependency on Section 03 clean.

---

## 04.3 — Integration Site 3: JIT `compile_all_functions` Mono Loop

### File

`compiler/ori_llvm/src/evaluator/compile.rs`

### Verified Context (lines 230–296)

```rust
let mut mono_functions = crate::monomorphize::collect_mono_functions(
    mono_instances,
    function_sigs,
    interner,
    self.pool,
);
mono_functions.extend(imported_mono_functions);

let (uniqueness_summaries, aims_contracts) =
    Self::run_interprocedural_analyses(arc_cache, &classifier, interner);
```

Monomorphized functions are lowered into `arc_cache` earlier in the function (at
`compile_all_functions`'s monomorphization loop, which mirrors the AOT path). By line 230
the mono functions are in `arc_cache` and are about to be handed to `prepare_mono_cached`
(called via `fc.prepare_all_cached(...)` and `fc.prepare_mono_cached(...)` at lines 296–297
and 300–305 respectively).

The JIT path calls `prepare_all_cached` and `prepare_mono_cached` on the `FunctionCompiler`
after construction at line 243. The assertions at the prepare-phase seams (Section 04.2)
cover these call sites automatically — the JIT `FunctionCompiler` uses the same
`prepare_all_cached` / `prepare_mono_cached` implementations.

However, there is an additional JIT-specific mono-lowering loop at the top of
`compile_all_functions` where functions are lowered via `arc_cache.insert(...)`. That loop
is the point at which `ArcFunction`s are written into the cache. Asserting there would check
the function immediately after lowering, before any AIMS processing.

### Insert After the JIT Mono Loop

The JIT mono lowering produces `(arc_fn, lambdas)` pairs. After each `arc_cache.insert(...)`,
add:

```rust
// PC-2 contract: no Tag::Var should survive ARC lowering into the cache.
// Debug: hard-assert; Release: log and continue (downstream verify() will catch it).
if let Err(msg) = ori_arc::assert_no_unresolved_type_vars(
    self.pool, &arc_fn, interner,
) {
    tracing::error!(
        contract_violation = true,
        %msg,
        "Tag::Var in JIT mono ARC IR (codegen-rules.md §TR-2)"
    );
    debug_assert!(false, "{}", msg);
}
```

Note: `self.pool` is the pool field on the `Evaluator` struct, accessed as `self.pool` in
the `compile_all_functions` method. Confirm this accessor before writing.

### TPR Checkpoint After 04.3

A TPR review is required after subsections 04.1 through 04.3 are implemented. The reviewer
must verify:

1. The helper function signature is correct and the diagnostic message is actionable.
2. The `debug_assert!` / `tracing::error!` / `record_codegen_error` layering follows the
   `codegen-rules.md §VR-1` pattern used elsewhere in the codebase.
3. No assertion fires against the existing spec test corpus (the corpus must stay green).
4. The Section 03 dependency note is correct — enabling assertions before Section 03 lands
   would produce spurious failures on valid programs.

---

## 04.TPR — TPR Checkpoint

> **Status**: `not-started`  
> Invoke `/tpr-review` with scope: `section-04-codegen-assertions.md §§04.1–04.3`.  
> Reviewers must:
> - Read `impl-hygiene.md §Cross-Phase Invariant Contracts`
> - Read `codegen-rules.md §VR-1` and `§TR-2`
> - Verify `assert_no_unresolved_type_vars` implementation in `ori_arc::ir::validate`
> - Verify integration sites 1, 2, and 3 follow the layered assertion pattern
> - Confirm no existing spec test fails
> - Confirm the Section 03 dependency note is correct

---

## 04.4 — Integration Site 4: AOT `codegen_pipeline.rs` Mono Loop

### File

`compiler/oric/src/commands/codegen_pipeline.rs`

### Verified Context (lines 108–130)

```rust
// Lower monomorphized generic functions.
let mono_functions = ori_llvm::monomorphize::collect_mono_functions(
    mono_instances,
    function_sigs,
    interner,
    pool,
);
for mono_fn in &mono_functions {
    let (arc_fn, lambdas) = crate::arc_lowering::lower_to_arc(
        mono_fn.mangled_name,
        &mono_fn.sig,
        mono_fn.original_name,
        canon,
        interner,
        pool,
        &mut arc_problems,
        Some(&mono_fn.body_type_map),
    );
    arc_cache.insert(arc_fn.name, (arc_fn, lambdas));
}
```

The AOT path calls `lower_to_arc` which produces `(arc_fn, lambdas)` and then inserts them
into `arc_cache`. The `pool` parameter is the bare `&Pool` passed into the outer function.
The `interner` is also a parameter.

### Insert After `arc_cache.insert`

```rust
// PC-2 contract: no Tag::Var should survive ARC lowering.
if let Err(msg) = ori_arc::assert_no_unresolved_type_vars(pool, &arc_fn, interner) {
    tracing::error!(
        contract_violation = true,
        %msg,
        "Tag::Var in AOT mono ARC IR (codegen-rules.md §TR-2)"
    );
    debug_assert!(false, "{}", msg);
    // The downstream arc_problems check (line 132) will report LLVM errors;
    // the tracing::error! above provides additional context.
}
```

Unlike the `ori_llvm`-internal sites, the AOT pipeline does not have access to
`self.builder.record_codegen_error()` at this point — the `FunctionCompiler` has not been
constructed yet. The `tracing::error!` is the release-path signal; the `debug_assert!` is
the debug-path hard failure. The downstream `emit_codegen_diagnostics` at line 136 will
handle the user-visible diagnostic when the resulting IR fails LLVM verification.

---

## 04.5 — Unit Tests for `assert_no_unresolved_type_vars`

### File

`compiler/ori_arc/src/ir/validate/tests.rs` — sibling of `validate.rs`, declared as
`#[cfg(test)] mod tests;` at the bottom of `validate.rs`.

### Test Matrix

The tests must cover:

| Case | Expected |
|------|----------|
| Empty `var_types` | `Ok(())` |
| All `var_types` are fully resolved non-Var indices | `Ok(())` |
| One `var_types` entry is `Tag::Var(_)` | `Err(...)` containing "Tag::Var reached codegen" |
| Multiple `var_types`, first is non-Var, second is `Tag::Var` | `Err(...)` naming the second variable |
| Multiple `var_types`, all `Tag::Var` | `Err(...)` naming the first (earliest) violator |

### Test Fixture Strategy

Construct a minimal `Pool` and `ArcFunction` in each test. The easiest approach:

1. `let pool = Pool::new(interner)` or equivalent constructor.
2. Allocate an `int` index via `pool.int_type()` or the pre-interned primitive index
   (`types.md §TY-5`).
3. Allocate a `Tag::Var` via `pool.fresh_var()` (which requires a `UnifyEngine` scope).
4. Build an `ArcFunction` with `var_types` set to combinations of the above.

If constructing a full `Pool` with `fresh_var()` is heavyweight, use `pool.tag(idx)` to
find a `Tag::Var` by allocating one through the unify engine, or by directly using the
pool's intern path to produce a known `Tag::Var` index.

Consult sibling tests in `compiler/ori_arc/src/` for the established pattern for building
minimal `Pool` instances in unit tests.

### Naming Convention

Follow `impl-hygiene.md §Test Function Naming` — names must be behavioral:

```rust
fn test_valid_function_with_resolved_types_passes_assertion()
fn test_empty_var_types_passes_assertion()
fn test_single_unresolved_var_returns_error_with_function_name()
fn test_second_var_entry_unresolved_names_that_variable_in_error()
fn test_all_vars_unresolved_names_first_violator()
```

---

## 04.R — Close-Out

> **Status**: `not-started`

Close-out tasks:

- [ ] Run `timeout 150 ./test-all.sh` in both debug and release and confirm green
- [ ] Run `timeout 150 cargo test -p ori_arc` and confirm green (covers `validate/tests.rs`)
- [ ] Run `timeout 150 cargo test -p ori_llvm` and confirm green
- [ ] Run `grep -rn 'assert_no_unresolved_type_vars' compiler/` to confirm exactly four
      call sites (prepare_all_cached × 2, compile_all_functions × 1, codegen_pipeline × 1)
- [ ] Run `grep -n 'pub mod validate' compiler/ori_arc/src/ir/mod.rs` returns one hit
- [ ] Run `grep -n 'pub fn assert_no_unresolved_type_vars' compiler/ori_arc/src/ir/validate.rs` returns one hit
- [ ] Confirm no spec test produces a `Tag::Var reached codegen` tracing::error! in either build
- [ ] Run `/impl-hygiene-review` scoped to the Section 04 diff
- [ ] Annotate all new code with plan comments (`// Section 04`) to be swept by the
      `section-04-codegen-assertions.md` close-out pass at plan completion
- [ ] Update this section's `status` to `complete`

---

## 04.N — Completion Checklist

- [ ] `ori_arc::ir::validate` module exists with `assert_no_unresolved_type_vars`
- [ ] `ori_arc` re-exports `assert_no_unresolved_type_vars` from `lib.rs`
- [ ] `prepare_all_cached` asserts for main function and each lambda
- [ ] `prepare_mono_cached` asserts for the mono function
- [ ] JIT `compile_all_functions` mono loop asserts after each `arc_cache.insert`
- [ ] AOT `codegen_pipeline.rs` mono loop asserts after each `arc_cache.insert`
- [ ] Unit tests in `validate/tests.rs` cover the five-row matrix above
- [ ] All assertions follow the layered pattern: `debug_assert!` + `tracing::error!` +
      `record_codegen_error()` (or equivalent for the AOT path that has no builder)
- [ ] `CLAUDE.md §Stabilization Discipline` — one semantic pin test exists that would
      fail if any assertion site is removed (covered by the unit test matrix)
- [ ] `codegen-rules.md §VR-1` parity — the assertion layering mirrors the
      per-function LLVM IR verification pattern
- [ ] `/tpr-review` on §§04.1–04.3 passed (04.TPR status updated)
- [ ] `/tpr-review` final pass on full section 04 diff passed
- [ ] `/impl-hygiene-review` passed
- [ ] `timeout 150 ./test-all.sh` green (debug and release)
- [ ] Section status updated to `complete`
