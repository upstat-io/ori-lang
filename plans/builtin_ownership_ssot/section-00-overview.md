---
section: "00"
title: "Builtin Ownership Single Source of Truth — Overview"
status: not-started
goal: "Move ownership metadata into ori_ir::MethodDef so every consumer derives its view from one registry"
---

# Section 00: Overview

**Status:** Not Started
**Goal:** Builtin method ownership (`receiver_borrows: bool`) is declared once in `ori_ir::MethodDef`, consumed by all downstream crates. No builtin can exist without an explicit ownership declaration.

---

## Problem

Builtin method ownership metadata is fragmented across 4 independent registries:

| Registry | Location | Entries | Data | Ownership? |
|----------|----------|---------|------|-----------|
| **IR** | `ori_ir/src/builtin_methods/mod.rs` | 162 | `MethodDef` (receiver, name, params, returns, trait) | **NO** |
| **TYPECK** | `ori_types/src/infer/expr/methods.rs` | 398 | `(&str, &str)` tuples | NO |
| **EVAL** | `ori_eval/src/methods/helpers/mod.rs` | 165 | `(&str, &str)` tuples | NO |
| **LLVM** | `ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` | 179 | `BuiltinRegistration` with `receiver_borrowed` | **YES** (only here) |

**The specific bug that exposed this:** `s.len()` is lowered as `Invoke len(s)` in ARC IR, but `len` has no entry in the borrow inference signature map. Borrow inference defaults to all-Owned, codegen compiles `len` inline as a field read (borrowing), nobody decrements, the string leaks.

**Root cause:** Ownership metadata (`receiver_borrowed: bool`) lives in `ori_llvm`'s `BuiltinRegistration`, but `ori_arc`'s borrow inference can't see it because the dependency arrow goes the wrong way (`ori_llvm` depends on `ori_arc`, not vice versa). The current workaround is `borrowing_builtin_names()` in `ori_llvm` which builds an `FxHashSet<Name>` and passes it back up — fragile, name-only (not type-qualified), and requires manual sync.

**Critical finding:** All 179 LLVM codegen entries have `receiver_borrowed: true`. Every current builtin borrows its receiver.

---

## Solution

Move ownership metadata into `ori_ir::builtin_methods::MethodDef` (bottom of crate DAG, visible to all consumers):

```
                ┌─────────┐
                │  ori_ir  │ ← MethodDef.receiver_borrows (SOURCE OF TRUTH)
                └────┬────┘
          ┌──────────┼──────────┐
          │          │          │
     ┌────▼───┐ ┌───▼───┐ ┌───▼────┐
     │ori_arc │ │ori_eval│ │ori_types│
     └────┬───┘ └───────┘ └────────┘
          │
     ┌────▼───┐
     │ori_llvm│  ← NO ownership metadata (only dispatch logic)
     └────────┘
```

Every consumer derives its view from the `ori_ir` registry. No builtin can exist without an explicit ownership declaration — the `bool` field is structural (compile-time enforcement).

---

## Current State: 4 Fragmented Registries (Detailed)

### IR Registry (162 entries, 9 types)

Types covered: int (24), float (18), bool (7), char (6), byte (6), str (16), Duration (18), Size (17), Ordering (11).

Missing 11 types that TYPECK has: Channel, DoubleEndedIterator, Iterator, Option, Result, Set, error, list, map, range, tuple.

### TYPECK Registry (398 entries, 20 types)

Full coverage. Authoritative list of what methods exist on which types from the type checker's perspective. Includes factory methods (`from_hours`), conversion aliases (`as_micros`), predicates (`is_even`), and higher-order methods (`map`, `filter`).

### EVAL Registry (165 entries, 18 types)

Missing Channel and DoubleEndedIterator (Iterator methods dispatched via `CollectionMethodResolver`). Includes operator long-form aliases (`subtract` → `sub`).

### LLVM Codegen Registry (179 entries, 14 types)

Types covered: int, float, bool, char, byte, str, Duration, Size, Ordering, list, map, Set, range, tuple, Option, Result, Iterator.

All 179 entries have `receiver_borrowed: true`.

Distribution across 7 submodule files:
- `primitives.rs`: 25 entries (clone, conversions, abs)
- `collections.rs`: 21 entries (len, is_empty, iter, clone)
- `traits.rs`: 82 entries (equals, compare, hash, comparison predicates)
- `compound_traits.rs`: 16 entries (list/Option/Result/tuple structural traits)
- `iterator.rs`: 15 entries (__iter_next, adapters, consumers)
- `option_result.rs`: 11 entries (is_some, is_none, unwrap, etc.)
- `trampolines.rs`: 0 entries (helper functions, not methods)

---

## Consistency Test Infrastructure

File: `compiler/oric/src/eval/tests/methods/consistency.rs` (1,005 lines)

Existing gap tracking lists that this plan modifies:

| List | Purpose | Size | Impact |
|------|---------|------|--------|
| `COLLECTION_TYPES` | Types missing from IR registry | 11 types | **Eliminated** by Section 02 |
| `EVAL_METHODS_NOT_IN_IR` | Eval methods without IR entry | 23 entries | **Reduced** by Section 02 |
| `TYPECK_METHODS_NOT_IN_IR` | Typeck methods without IR entry | 139 entries | **Reduced** by Section 02 |
| `IR_METHODS_DISPATCHED_VIA_RESOLVERS` | IR methods in eval via resolvers | 10 entries | Unchanged |
| `EVAL_METHODS_NOT_IN_TYPECK` | Eval methods not in typeck | 65 entries | Unchanged |
| `TYPECK_METHODS_NOT_IN_EVAL` | Typeck methods not in eval | ~260 entries | Unchanged |

---

## Implementation Sections

| Section | Title | Files Modified | Key Change |
|---------|-------|----------------|------------|
| 01 | Extend MethodDef with Ownership | `ori_ir/src/builtin_methods/mod.rs` | Add `receiver_borrows: bool` field |
| 02 | Expand IR Registry to All 20 Types | `ori_ir/src/builtin_methods/*.rs`, `consistency.rs` | Add ~236 entries, split into submodules |
| 03 | Wire ori_arc to ori_ir | `ori_arc/src/lib.rs`, 3 call sites | `builtin_borrowing_names()` reads from IR |
| 04 | Remove Ownership from ori_llvm | `ori_llvm/src/codegen/arc_emitter/builtins/*.rs` | Delete `receiver_borrowed`, simplify macro |
| 05 | Enforcement Tests | `builtins/tests.rs`, `consistency.rs` | Structural enforcement test |
| 06 | Legacy Removal & Verification | All files | Grep verification, full test suite |

---

## Exit Criteria

Adding a new builtin method requires:
1. Add `MethodDef` entry in `ori_ir::builtin_methods` with explicit `receiver_borrows` value
2. Add dispatch handler in appropriate `ori_llvm` submodule's `declare_builtins!`
3. The enforcement test catches any codegen handler without a `MethodDef`
4. The `MethodDef` struct requires an explicit `receiver_borrows` value at compile time

**The old `receiver_borrowed` field in `BuiltinRegistration` is completely gone.**
**The old `borrowing_builtin_names()` function in `ori_llvm` is completely gone.**
**Zero traces of the old system remain.**

---

## Verification

1. `cargo c` — every `MethodDef::new()` call compiles (missing `receiver_borrows` = compile error)
2. `cargo t -p ori_ir` — IR registry tests pass
3. `cargo t -p ori_arc` — borrow inference tests pass
4. `./llvm-test.sh` — LLVM tests pass, including new enforcement test
5. `cargo t -p oric` — consistency tests pass with reduced/empty exemption lists
6. `./clippy-all.sh` — no dead code warnings from removed plumbing
7. `./test-all.sh` — full suite green
