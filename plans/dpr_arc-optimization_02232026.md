---
plan: "dpr_arc-optimization_02232026"
title: "Design Pattern Review: ARC Builtin Ownership Safety"
status: draft
---

# Design Pattern Review: ARC Builtin Ownership Safety

## Ori Today

Ori's ARC pipeline transforms a typed AST through a series of SSA-based passes: lowering (`ori_arc/lower/`) produces basic-block `ArcFunction`s, borrow inference (`ori_arc/borrow/mod.rs:infer_borrows`) runs a Lean 4-style fixed-point algorithm to classify each function parameter as `Borrowed` or `Owned`, RC insertion (`ori_arc/rc_insert/mod.rs`) places `RcInc`/`RcDec` instructions via backward liveness analysis, and RC elimination (`ori_arc/rc_elim/`) cancels redundant pairs. The pipeline is well-structured: `AnnotatedSig` in `ownership/mod.rs` carries per-parameter ownership, `DerivedOwnership` extends tracking to all local variables via a single SSA forward pass, and the whole sequence is orchestrated by `run_arc_pipeline_all` in `lib.rs`. The Perceus model is faithfully implemented with proper handling of borrowed parameters, projection propagation, and tail-call preservation.

The critical gap is that borrow inference and RC insertion operate on a `FxHashMap<Name, AnnotatedSig>` that contains **only user-defined Ori functions**. When borrow inference encounters `Apply { func: len, ... }` at line 258 of `borrow/mod.rs`, it checks `sigs.get(callee)` and finds nothing — `len` is a builtin method, not a user function. The fallback at line 268 marks all arguments as Owned. Similarly, `compute_arg_ownership` in `rc_insert/mod.rs` at line 990 checks for the `ori_` external prefix, fails (builtins use bare names like `len`, not `ori_len`), and returns `vec![ArgOwnership::Owned; arg_count]` at line 998. The result: unnecessary `RcInc`/`RcDec` pairs surround what should be zero-cost borrowing operations.

Meanwhile, the LLVM codegen layer has complete knowledge of builtin ownership. The `declare_builtins!` macro in `ori_llvm/codegen/arc_emitter/builtins/mod.rs` generates both dispatch functions and `REGISTERED` arrays containing `BuiltinRegistration { receiver_borrowed: bool }` metadata. Every builtin in the codebase today is marked `borrow: true` — they all borrow their receivers. But this metadata lives exclusively in `ori_llvm` and is invisible to `ori_arc`. The `BuiltinTable` (lazy singleton at line 234 of `builtins/mod.rs`) provides O(1) lookup by `(type_name, method_name)`, but only for LLVM codegen dispatch, not for borrow inference. Three separate registries exist with no shared truth: the type checker's `TYPECK_BUILTIN_METHODS`, the lowerer's inline tag-check detection in `lower/calls/mod.rs`, and the LLVM `BuiltinTable`. Borrow inference has access to none of them.

## Prior Art

### Swift -- Ownership Conventions in Function Types

Swift encodes ownership semantics directly in SIL function types via `ParameterConvention` and `ResultConvention` enums. Every SIL function — user-defined, builtin, or runtime — carries its ownership contract in its type signature. When `String.count` is lowered from AST to SIL, it receives `@guaranteed` (borrow) convention on its `self` parameter during the lowering phase, making it indistinguishable from user code at the optimization level. The SIL ownership verifier then validates all uses uniformly.

This approach eliminates the "invisible builtin" class of bugs entirely: if a callable exists in SIL, it has ownership metadata. The cost is a more sophisticated lowering phase that must assign correct conventions for every callable, but the payoff is that no downstream pass ever needs to special-case builtins. There is no registry, no lookup table, no fallback — ownership is structural, embedded in the IR itself.

### Lean 4 -- Closed Registry with ParamMap

Lean 4 maintains a `ParamMap` registry mapping `FunId` to `Array Param` where each `Param` carries a `borrow : Bool` flag. Both user-defined functions (whose borrow status is inferred via fixed-point iteration in `collectDecls`) and external/builtin functions (whose borrow status is explicitly declared via `@[extern]` attributes) populate the same map. The system is **closed**: if a function is not in the registry, compilation fails. There are no defaults.

When analyzing `f ys` at line 366 of `RC.lean`, the code calls `getDecl ctx f` and retrieves `decl.params` with borrow info included. For the special case of `Array.getInternal`, Lean detects the function by name and selects the borrowed variant (lines 369-372). The registry approach requires each builtin's borrow signature to be registered explicitly, creating a synchronization burden, but it prevents silent misclassification — a missing entry is a compile error, not a silent performance regression.

### Koka -- Extracted Borrow Map with Unsafe Defaults

Koka builds a unified `Borrowed` map (`NameMap` from function names to `([ParamInfo], Fip)` pairs) by extracting borrow info from Core-level IR definitions and extern declarations. User functions and externals are processed by the same extraction functions (`extractBorrowDefs` and `extractBorrowExternals`), merged into a single lookup table. The design is **open with consumption defaults**: if `borrowedLookup name borrowed` fails to find an entry, the caller assumes all parameters are consuming.

This is the simplest approach — borrow info flows naturally from definitions — but the open default is the exact vulnerability Ori currently exhibits. An unregistered builtin silently gets all-Owned treatment, producing correct but suboptimal code. The failure mode is a performance bug rather than a compilation error, making it difficult to detect.

## Proposed Best-of-Breed Design

### Core Idea

Combine Lean 4's closed-registry discipline with Swift's structural approach, adapted to Ori's existing `declare_builtins!` infrastructure. The key insight is that Ori already has the right data in the right place — the `BuiltinRegistration` structs in `ori_llvm` contain `receiver_borrowed: bool` — it just needs to flow upstream to where borrow inference and RC insertion make their decisions.

Rather than duplicating builtin metadata in `ori_arc` (which would create yet another unsynchronized registry), we extract the ownership specification into a new shared type in `ori_arc` that both the ARC pipeline and LLVM codegen consume. The `declare_builtins!` macro already enforces that registration and dispatch cannot drift; we extend this guarantee to include ownership semantics. The system becomes closed: adding a builtin to LLVM codegen without declaring its ownership contract produces a compile-time error, not a silent ARC leak.

### Key Design Choices

1. **Single source of truth for builtin ownership lives in `ori_arc`, not `ori_llvm`.** (Inspired by Lean 4's `ParamMap` living in the IR layer, not the codegen layer.) The `BuiltinOwnershipMap` type is defined in `ori_arc/src/ownership/builtins.rs` and populated by a `register_builtin_sigs()` function. This inverts the current dependency: instead of `ori_llvm` owning builtin metadata that `ori_arc` cannot see, `ori_arc` owns the ownership contract and `ori_llvm` consumes it alongside its codegen dispatch.

2. **Builtin signatures are injected into the same `FxHashMap<Name, AnnotatedSig>` that user functions use.** (Inspired by Koka's unified `Borrowed` map.) Borrow inference already handles the sigs map correctly — the only problem is that builtins are absent. By pre-populating the map with builtin entries before `infer_borrows` runs, all existing logic in `update_ownership` (line 234 of `borrow/mod.rs`) and `compute_arg_ownership` (line 983 of `rc_insert/mod.rs`) works without modification. Zero changes to the fixed-point algorithm.

3. **Closed system: compile-time enforcement that every builtin declares ownership.** (Inspired by Lean 4's "missing entry = compilation failure" philosophy, rejecting Koka's unsafe defaults.) The `declare_builtins!` macro already requires `borrow: <expr>` for each entry. We add a compile-time `const` assertion in `ori_llvm` that every `BuiltinRegistration` has a corresponding entry in the `BuiltinOwnershipMap`. A `#[cfg(test)]` sync test (extending the existing `TYPECK_BUILTIN_METHODS` sync pattern) verifies bidirectional coverage: every codegen builtin has an ownership entry, and every ownership entry has a codegen handler.

4. **Per-parameter ownership, not just receiver-borrowed.** (Inspired by Swift's per-parameter `ParameterConvention`.) The current `receiver_borrowed: bool` only describes the first parameter. Methods like `concat(self, other)` or `equals(self, other)` take two reference-typed arguments. The new `BuiltinParamOwnership` type carries a `SmallVec<[Ownership; 4]>` for all parameters, enabling borrow inference to correctly mark multi-parameter builtins. For the common case (single receiver, borrowed), a convenience constructor keeps declarations concise.

5. **Builtin name resolution uses the string interner, matching lowering's canonicalization.** (Addresses Ori's specific constraint: method calls like `s.len()` are canonicalized to `len(s)` during lowering, producing `Apply { func: len, ... }` where `len` is an interned `Name`.) The ownership map keys on `Name` (interned), not `&'static str`, ensuring lookup in borrow inference matches the exact interned name the lowerer emits. A `hydrate()` function converts static string entries to interned names at pipeline startup.

6. **Salsa-compatible: the builtin ownership map is deterministic and `Clone + Eq + Hash`.** (Addresses Ori's incremental compilation constraint.) The map is built from `const` data with no IO, no randomness, and no function pointers. It can be cached across Salsa revisions without invalidation concerns. The `hydrate()` call happens once per compilation unit, not per query.

### What Makes Ori's Approach Unique

Ori's expression-based, ARC-managed design creates a specific opportunity that none of the reference compilers exploit: **builtins that are compiled inline (field reads, tag checks, type conversions) should never appear in the sigs map at all — they should be lowered to non-call IR during the lowering phase itself.**

The lowerer already does this for tag checks (`is_ok`, `is_err`, `is_some`, `is_none`) in `lower/calls/mod.rs:emit_tag_check` — it lowers them to `Project + PrimOp(Eq)` instead of emitting an `Apply`. This is the ideal pattern: by the time borrow inference runs, there is no `Apply` instruction to misclassify. The tag-check builtins are invisible to borrow inference because they don't exist as calls.

The proposed design exploits this in two tiers:

- **Tier 1 (lowering-eliminated):** Builtins that compile to field reads (`len`, `is_empty`, `capacity`) or identity operations (`clone` on scalars) are lowered directly to `Project` / `Let { value: Var(_) }` instructions during `lower_method_call`. These never reach borrow inference as calls. This is the Swift approach — ownership is structural in the IR — applied selectively to the cheapest builtins. Currently only tag checks use this path; we extend it to cover all pure-projection builtins.

- **Tier 2 (registry-covered):** Builtins that compile to runtime calls (`to_str`, `concat`, `iter`, `str.hash`) or complex inline sequences (multi-instruction trait methods) remain as `Apply` instructions and are covered by the `BuiltinOwnershipMap` registry in the sigs map. This is the Lean 4 approach — explicit registration in a closed map.

Ori's capability-based effect system adds another unique dimension: builtins are pure (no effects), so their ownership contract is static and unconditional. Unlike Swift where a method might have different ownership conventions depending on whether it mutates, Ori builtins always borrow because they never mutate their receivers. This means the registry is simpler — no conditional ownership, no effect-dependent conventions — and `Borrowed` is the correct default for the receiver parameter of every builtin.

### Concrete Types & Interfaces

```rust
// ori_arc/src/ownership/builtins.rs

use smallvec::SmallVec;
use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashMap;
use crate::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
use ori_types::Idx;

/// Per-parameter ownership for a builtin method.
///
/// Stored as static data, hydrated into `AnnotatedSig` entries at pipeline
/// startup. The `SmallVec` avoids heap allocation for the common 1-2 param case.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BuiltinParamOwnership {
    /// Canonical method name (e.g., "len", "concat", "equals").
    pub method: &'static str,
    /// Per-parameter ownership. Index 0 is the receiver (self).
    /// Most builtins borrow all params; `concat` borrows both; `iter` borrows receiver.
    pub params: SmallVec<[Ownership; 4]>,
    /// Return type hint for the AnnotatedSig. `None` means use Idx::UNIT.
    pub return_type: Option<Idx>,
}

impl BuiltinParamOwnership {
    /// Convenience: single-receiver builtin that borrows self.
    pub const fn borrow_receiver(method: &'static str) -> Self {
        Self {
            method,
            params: SmallVec::from_const([Ownership::Borrowed]),
            return_type: None,
        }
    }

    /// Two-parameter builtin (self + other), both borrowed.
    pub const fn borrow_two(method: &'static str) -> Self {
        Self {
            method,
            params: SmallVec::from_const([Ownership::Borrowed, Ownership::Borrowed]),
            return_type: None,
        }
    }
}

/// Static registry of all builtin method ownership contracts.
///
/// Populated by `builtin_ownership_entries()` from const data.
/// Hydrated into `FxHashMap<Name, AnnotatedSig>` by `hydrate_builtin_sigs()`.
///
/// The entries here are the SINGLE SOURCE OF TRUTH for builtin ownership.
/// `ori_llvm`'s `declare_builtins!` macro generates codegen dispatch;
/// this module generates ARC ownership contracts. Both must stay in sync,
/// enforced by the `builtin_ownership_sync` test.
pub fn builtin_ownership_entries() -> &'static [BuiltinParamOwnership] {
    use Ownership::Borrowed;
    static ENTRIES: &[BuiltinParamOwnership] = &[
        // Collection methods (receiver borrows)
        BuiltinParamOwnership::borrow_receiver("len"),
        BuiltinParamOwnership::borrow_receiver("length"),
        BuiltinParamOwnership::borrow_receiver("is_empty"),
        BuiltinParamOwnership::borrow_receiver("capacity"),
        BuiltinParamOwnership::borrow_receiver("iter"),
        BuiltinParamOwnership::borrow_receiver("clone"),
        // Two-param trait methods (self + other, both borrow)
        BuiltinParamOwnership::borrow_two("equals"),
        BuiltinParamOwnership::borrow_two("is_equal"),
        BuiltinParamOwnership::borrow_two("compare"),
        BuiltinParamOwnership::borrow_two("is_less"),
        BuiltinParamOwnership::borrow_two("is_greater"),
        BuiltinParamOwnership::borrow_two("is_less_or_equal"),
        BuiltinParamOwnership::borrow_two("is_greater_or_equal"),
        // Unary trait/conversion methods (receiver borrows)
        BuiltinParamOwnership::borrow_receiver("hash"),
        BuiltinParamOwnership::borrow_receiver("to_str"),
        BuiltinParamOwnership::borrow_receiver("to_int"),
        BuiltinParamOwnership::borrow_receiver("to_float"),
        BuiltinParamOwnership::borrow_receiver("abs"),
        BuiltinParamOwnership::borrow_receiver("byte"),
        BuiltinParamOwnership::borrow_receiver("f"),
        BuiltinParamOwnership::borrow_receiver("into"),
        BuiltinParamOwnership::borrow_receiver("reverse"),
        // String-specific
        BuiltinParamOwnership::borrow_two("concat"),
        // Option/Result (receiver borrows)
        BuiltinParamOwnership::borrow_receiver("is_some"),
        BuiltinParamOwnership::borrow_receiver("is_none"),
        BuiltinParamOwnership::borrow_receiver("is_ok"),
        BuiltinParamOwnership::borrow_receiver("is_err"),
        BuiltinParamOwnership::borrow_receiver("unwrap"),
        BuiltinParamOwnership::borrow_receiver("unwrap_err"),
        BuiltinParamOwnership::borrow_two("unwrap_or"),
    ];
    ENTRIES
}

/// Hydrate static builtin entries into interned `AnnotatedSig` entries.
///
/// Merges into the existing `sigs` map. Builtin entries do NOT overwrite
/// user-defined functions (user code wins if there's a name collision,
/// which shouldn't happen but is defensive).
///
/// Called once at pipeline startup, before `infer_borrows`.
pub fn hydrate_builtin_sigs(
    sigs: &mut FxHashMap<Name, AnnotatedSig>,
    interner: &StringInterner,
) {
    for entry in builtin_ownership_entries() {
        let name = interner.intern(entry.method);

        // Don't overwrite user-defined functions.
        if sigs.contains_key(&name) {
            continue;
        }

        let params = entry
            .params
            .iter()
            .enumerate()
            .map(|(i, &ownership)| AnnotatedParam {
                name: interner.intern(&format!("__builtin_param_{i}")),
                ty: Idx::UNIT, // Placeholder — borrow inference only reads ownership.
                ownership,
            })
            .collect();

        sigs.insert(
            name,
            AnnotatedSig {
                params,
                return_type: entry.return_type.unwrap_or(Idx::UNIT),
            },
        );
    }
}
```

```rust
// Integration point: ori_llvm/src/evaluator.rs (and compile_common.rs)
// After infer_borrows, before run_arc_pipeline_all:

let mut sigs = ori_arc::infer_borrows(&arc_functions, &classifier);
ori_arc::ownership::builtins::hydrate_builtin_sigs(&mut sigs, interner);
// Now sigs contains both user functions and builtins.
// run_arc_pipeline_all will see builtin entries in the sigs map.
```

```rust
// Tier 1: Lowering-phase elimination for projection builtins
// ori_arc/src/lower/calls/mod.rs — extend lower_method_call

/// Try to lower a pure-projection builtin inline.
///
/// Methods that compile to a single field read (len, is_empty, capacity)
/// are lowered directly to Project + optional PrimOp, bypassing Apply
/// entirely. This eliminates them from borrow inference's view.
fn try_lower_projection_builtin(
    &mut self,
    receiver: CanId,
    method: Name,
    receiver_ty: Idx,
    ty: Idx,
    span: Span,
) -> Option<ArcVarId> {
    let method_str = self.name_str(method);
    let resolved = self.pool.resolve_fully(receiver_ty);
    let tag = self.pool.tag(resolved);

    match (method_str, tag) {
        // str.len / str.length → Project field 0 (i64 len from {i64, ptr})
        ("len" | "length", Tag::Str) => {
            let recv_var = self.lower_expr(receiver);
            Some(self.builder.emit_project(Idx::INT, recv_var, 0, Some(span)))
        }
        // list.len / list.length → Project field 0 (i64 len from {i64, i64, ptr})
        ("len" | "length", Tag::List) => {
            let recv_var = self.lower_expr(receiver);
            Some(self.builder.emit_project(Idx::INT, recv_var, 0, Some(span)))
        }
        // map.len / set.len → Project field 0
        ("len" | "length", Tag::Map | Tag::Set) => {
            let recv_var = self.lower_expr(receiver);
            Some(self.builder.emit_project(Idx::INT, recv_var, 0, Some(span)))
        }
        // str.is_empty → len == 0
        ("is_empty", Tag::Str | Tag::List) => {
            let recv_var = self.lower_expr(receiver);
            let len = self.builder.emit_project(Idx::INT, recv_var, 0, Some(span));
            let zero = self.builder.emit_let(
                Idx::INT,
                crate::ir::ArcValue::Literal(crate::ir::LitValue::Int(0)),
                None,
            );
            Some(self.builder.emit_let(
                Idx::BOOL,
                crate::ir::ArcValue::PrimOp {
                    op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Eq),
                    args: vec![len, zero],
                },
                Some(span),
            ))
        }
        _ => None,
    }
}
```

```rust
// Sync test: ori_llvm/src/codegen/arc_emitter/builtins/tests.rs

#[test]
fn builtin_ownership_sync() {
    // Every codegen builtin must have an ownership entry.
    let table = super::builtin_table();
    let ownership_entries: HashSet<&str> = ori_arc::ownership::builtins::builtin_ownership_entries()
        .iter()
        .map(|e| e.method)
        .collect();

    let mut missing_ownership = Vec::new();
    for (type_name, method_name) in table.all_registered() {
        if !ownership_entries.contains(method_name) {
            missing_ownership.push((type_name, method_name));
        }
    }

    assert!(
        missing_ownership.is_empty(),
        "Codegen builtins missing ownership entries: {missing_ownership:?}\n\
         Add entries to ori_arc::ownership::builtins::builtin_ownership_entries()"
    );
}
```

## Implementation Roadmap

### Phase 1: Foundation (Ownership Registry)
- [ ] Create `ori_arc/src/ownership/builtins.rs` with `BuiltinParamOwnership`, `builtin_ownership_entries()`, and `hydrate_builtin_sigs()`
- [ ] Add `pub mod builtins;` to `ori_arc/src/ownership/mod.rs` and export from `ori_arc/src/lib.rs`
- [ ] Wire `hydrate_builtin_sigs()` into `ori_llvm/src/evaluator.rs` after `infer_borrows` call (line 376)
- [ ] Wire `hydrate_builtin_sigs()` into `oric/src/commands/compile_common.rs` after `infer_borrows` call (line 184)
- [ ] Add sync test in `ori_llvm/src/codegen/arc_emitter/builtins/tests.rs` verifying all `BuiltinTable` entries have ownership entries
- [ ] Add unit tests in `ori_arc/src/ownership/tests.rs` verifying `hydrate_builtin_sigs` populates the map correctly and doesn't overwrite user functions

### Phase 2: Lowering Elimination (Tier 1 Builtins)
- [ ] Implement `try_lower_projection_builtin()` in `ori_arc/src/lower/calls/mod.rs` for `len`/`length`/`is_empty` on str, list, map, set
- [ ] Call `try_lower_projection_builtin()` at the top of `lower_method_call()`, before the `emit_call_or_invoke` path
- [ ] Add unit tests in `ori_arc/src/lower/calls/tests.rs` verifying `len` on str/list lowers to `Project` not `Apply`
- [ ] Ensure LLVM codegen still handles these correctly (the `Project` instruction is already codegen'd as `extract_value`)
- [ ] Add spec test `tests/spec/arc/builtin_borrow.ori` exercising `len`/`is_empty` in multi-use contexts to verify no unnecessary RC ops

### Phase 3: Hardening and Cleanup
- [ ] Remove the `#[allow(dead_code)]` from `BuiltinRegistration::receiver_borrowed` since it is now consumed by the ownership sync test
- [ ] Add a `#[cfg(test)]` exhaustiveness check: iterate `builtin_ownership_entries()` and verify each method name resolves via the interner and exists in the `BuiltinTable`
- [ ] Audit `compute_arg_ownership()` in `rc_insert/mod.rs` — the "unknown non-external: conservative owned" path at line 998 should now only trigger for truly unknown functions (user functions in other modules not yet compiled); add a `tracing::warn!` when it fires during full-module compilation to catch future regressions
- [ ] Measure RC operation count before and after on a representative program (e.g., the collection-heavy spec tests) to quantify the improvement
- [ ] Document the two-tier builtin ownership model in `ori_arc/src/ownership/mod.rs` module docs

## References

### Ori Source Files Studied
- `compiler/ori_arc/src/borrow/mod.rs` — Fixed-point borrow inference (lines 56-73: `infer_borrows`, lines 253-272: `Apply` handling with unknown callee fallback)
- `compiler/ori_arc/src/rc_insert/mod.rs` — RC insertion and `compute_arg_ownership` (lines 983-1019: callee ownership lookup with external detection)
- `compiler/ori_arc/src/ir/mod.rs` — ARC IR types (`ArcInstr::Apply`, `ArgOwnership`, `ArcFunction`)
- `compiler/ori_arc/src/ownership/mod.rs` — `Ownership`, `DerivedOwnership`, `AnnotatedSig`, `AnnotatedParam`
- `compiler/ori_arc/src/lib.rs` — Pipeline orchestration (`run_arc_pipeline`, `run_arc_pipeline_all`)
- `compiler/ori_arc/src/lower/calls/mod.rs` — Call lowering and tag-check inline elimination
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` — `declare_builtins!` macro, `BuiltinTable`, `BuiltinRegistration`
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs` — Primitive builtin registrations
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — Collection builtin registrations (`len`, `is_empty`, `iter`)
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs` — Trait method registrations (`equals`, `compare`, `hash`)
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs` — Option/Result builtin registrations
- `compiler/ori_llvm/src/evaluator.rs` — JIT pipeline (line 376: `infer_borrows` call site)
- `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` — Two-pass compilation with borrow sig verification (lines 263-278)
- `compiler/oric/src/commands/compile_common.rs` — AOT pipeline (line 184: `infer_borrows` call site)

### Reference Compiler Files
- Swift: `lib/SILOptimizer/ARC/` — ARC optimization passes; `include/swift/AST/Ownership.h` — `ParameterConvention` enum
- Lean 4: `src/Lean/Compiler/IR/Borrow.lean` — Fixed-point borrow inference with `ParamMap` registry (lines 302-309: iteration, lines 366-372: builtin special-casing)
- Lean 4: `src/Lean/Compiler/IR/RC.lean` — RC insertion consuming `ParamMap` entries (line 192: environment lookup for externals)
- Koka: `src/Core/Borrowed.hs` — `extractBorrowDefs` and `extractBorrowExternals` building unified `Borrowed` map (lines 102-106: external extraction)
