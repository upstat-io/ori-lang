---
section: "03"
title: "Wire ori_arc to ori_ir"
status: not-started
goal: "ori_arc reads builtin ownership from ori_ir instead of receiving it from ori_llvm"
files:
  - compiler/ori_arc/src/lib.rs
  - compiler/oric/src/commands/compile_common.rs
  - compiler/ori_llvm/src/evaluator.rs
  - compiler/ori_llvm/src/codegen/function_compiler/mod.rs
---

# Section 03: Wire ori_arc to ori_ir

**Status:** Not Started
**Goal:** `ori_arc` reads builtin ownership from `ori_ir` instead of receiving it from `ori_llvm`. The dependency arrow is corrected: `ori_ir` → `ori_arc` (same direction), not `ori_llvm` → callers (backwards).

---

## 03.0 Current Data Flow (Wrong)

```
                     ┌─────────────┐
                     │   ori_llvm   │
                     │              │
                     │ borrowing_   │
                     │ builtin_     │──→ FxHashSet<Name>
                     │ names()      │
                     └──────┬───────┘
                            │
            ┌───────────────┼───────────────┐
            │               │               │
   compile_common.rs   evaluator.rs   function_compiler.rs
   (lines 184, 219)    (line 376)     (line 106)
```

**Problem:** The `FxHashSet<Name>` comes from `ori_llvm`, which is downstream of `ori_arc`. This creates a hidden upward dependency: `ori_arc` doesn't depend on `ori_llvm`, but its callers must reach into `ori_llvm` to get data that logically belongs to `ori_arc`.

## 03.1 Target Data Flow (Correct)

```
      ┌──────────┐
      │  ori_ir   │  ← MethodDef.receiver_borrows (SOURCE OF TRUTH)
      └─────┬─────┘
            │
      ┌─────▼─────┐
      │  ori_arc   │
      │            │
      │ builtin_   │──→ FxHashSet<Name>
      │ borrowing_ │
      │ names()    │
      └─────┬──────┘
            │
   ┌────────┼────────┐
   │        │        │
  compile  eval   func_compiler
  _common  uator  /mod.rs
```

---

## 03.2 Add `builtin_borrowing_names()` to ori_arc

**File:** `compiler/ori_arc/src/lib.rs`

Add a public function that builds the borrowing builtins set from `ori_ir`:

```rust
/// Build the set of builtin method names that borrow their receiver.
///
/// Source of truth: `ori_ir::builtin_methods::MethodDef.receiver_borrows`.
///
/// **Excluded:** Iterator methods and `.iter()` — these create derived values
/// that reference the receiver's data. The ARC pipeline can't model these
/// hidden dependencies, so they must use Owned semantics (the runtime handles
/// internal RC management).
pub fn builtin_borrowing_names(interner: &ori_ir::StringInterner) -> FxHashSet<Name> {
    use ori_ir::builtin_methods::borrowing_method_names;

    borrowing_method_names()
        .filter(|name| {
            // Skip .iter() — creates iterator with hidden dependency on receiver
            *name != "iter"
        })
        .map(|name| interner.intern(name))
        .collect()
}
```

### Critical: Iterator Exclusion Logic

The current `borrowing_builtin_names()` in `ori_llvm` has two exclusion rules (lines 259-270 of `builtins/mod.rs`):

1. **Skip all Iterator type methods** — `if type_name == "Iterator" { continue; }` — iterators consume/transform, not borrow
2. **Skip `.iter()` method on any type** — `if method_name == "iter" { continue; }` — creates derived value referencing receiver

The new function in `ori_arc` must replicate both exclusions. However, `borrowing_method_names()` returns method names without type qualification. This means:

- **`.iter()` exclusion**: Filter by name — `*name != "iter"` — works directly
- **Iterator type exclusion**: Since `borrowing_method_names()` iterates ALL methods from ALL types, Iterator methods like `next`, `count`, `collect`, `map`, `filter` etc. will be included if they have `receiver_borrows: true`. BUT the same method names exist on other types (e.g., `list.map`, `list.filter`).

**Resolution:** The Iterator exclusion in the current code works because it's type-qualified (the `BuiltinTable` has `type_name` per entry). The `borrowing_method_names()` approach is name-only. Two options:

**Option A:** Add a type-qualified query to `ori_ir`:
```rust
/// All (type, method) pairs that borrow their receiver.
pub fn borrowing_method_pairs() -> impl Iterator<Item = (BuiltinType, &'static str)> {
    BUILTIN_METHODS.iter()
        .filter(|m| m.receiver_borrows)
        .map(|m| (m.receiver, m.name))
}
```

Then in `ori_arc`:
```rust
pub fn builtin_borrowing_names(interner: &ori_ir::StringInterner) -> FxHashSet<Name> {
    ori_ir::builtin_methods::borrowing_method_pairs()
        .filter(|(ty, name)| {
            // Skip Iterator type methods — consume/transform, not borrow
            *ty != BuiltinType::Iterator
            && *ty != BuiltinType::DoubleEndedIterator
            // Skip .iter() — creates iterator with hidden dependency on receiver
            && *name != "iter"
        })
        .map(|(_, name)| interner.intern(name))
        .collect()
}
```

**Option B:** Accept name-only (current behavior matches):
The current `borrowing_builtin_names()` in `ori_llvm` collects names from non-Iterator types. If `list.count` and `Iterator.count` both borrow, the name `"count"` is in the set regardless — it's included from `list` even though `Iterator` is skipped. This is fine because `ori_arc` only checks `borrowing_builtins.contains(callee)` where `callee` is a name. A method called `count` on any type being treated as borrowing is correct.

The only risk: an Iterator-only method name (like `__iter_next`, `chain`, `cycle`) that should NOT be in the borrowing set. But these don't overlap with other type methods.

**Decision: Option A** — type-qualified is cleaner and matches the original semantics exactly. But if implementation complexity is high, Option B is acceptable.

---

## 03.3 Replace Call Sites

### Call Site 1: `compile_common.rs` line 184-185

**File:** `compiler/oric/src/commands/compile_common.rs`

```rust
// BEFORE:
let borrowing_builtins =
    ori_llvm::codegen::arc_emitter::borrowing_builtin_names(interner);

// AFTER:
let borrowing_builtins = ori_arc::builtin_borrowing_names(interner);
```

### Call Site 2: `compile_common.rs` line 219-220

**File:** `compiler/oric/src/commands/compile_common.rs`

Same replacement in the cache path:

```rust
// BEFORE:
let borrowing_builtins =
    ori_llvm::codegen::arc_emitter::borrowing_builtin_names(interner);

// AFTER:
let borrowing_builtins = ori_arc::builtin_borrowing_names(interner);
```

### Call Site 3: `evaluator.rs` line 376-377

**File:** `compiler/ori_llvm/src/evaluator.rs`

```rust
// BEFORE:
let borrowing_builtins =
    crate::codegen::arc_emitter::borrowing_builtin_names(interner);

// AFTER:
let borrowing_builtins = ori_arc::builtin_borrowing_names(interner);
```

### Call Site 4: `function_compiler/mod.rs` line 106-107

**File:** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

```rust
// BEFORE:
let borrowing_builtins =
    crate::codegen::arc_emitter::borrowing_builtin_names(interner);

// AFTER:
let borrowing_builtins = ori_arc::builtin_borrowing_names(interner);
```

---

## 03.4 No Changes Required

These files already have the correct interface and don't need modification:

| File | Why No Changes |
|------|---------------|
| `ori_arc/src/borrow/mod.rs` | `infer_borrows()` accepts `&FxHashSet<Name>` — interface unchanged |
| `ori_arc/src/rc_insert/mod.rs` | `compute_arg_ownership()` accepts `&FxHashSet<Name>` — interface unchanged |
| `ori_arc/src/ownership.rs` | Ownership types unchanged |

---

## 03.5 Dependency Verification

After this change, the crate dependency for borrowing builtins becomes:

```
ori_ir (MethodDef.receiver_borrows)
  ↓
ori_arc (builtin_borrowing_names → FxHashSet<Name>)
  ↓
oric / ori_llvm (consumers of the set)
```

**Verify:** `ori_arc` already depends on `ori_ir` (for `Name`, `BinaryOp`, etc.). No new crate dependency is introduced. Check `compiler/ori_arc/Cargo.toml` — `ori_ir` should already be listed.

---

## 03.6 TypeFlow — Not Consumed by ARC

`type_flow` is consumed by `ori_types` (type checker), NOT by `ori_arc` (borrow inference). The ARC pipeline only reads `receiver_borrows`. The `type_flow` field coexists on `MethodDef` without interference — `ori_arc` simply ignores it.

This separation is clean: ownership (`receiver_borrows`) flows down to ARC/codegen, while type unification constraints (`type_flow`) flow to the type checker. Two consumers, one source of truth, zero coupling between them.

---

## 03.7 Verification

- [ ] `cargo c -p ori_arc` — new function compiles
- [ ] `cargo c -p oric` — call site replacements compile
- [ ] `cargo c -p ori_llvm` — call site replacement compiles
- [ ] `cargo t -p ori_arc` — borrow inference tests pass (set contents unchanged)
- [ ] `./llvm-test.sh` — LLVM tests pass (same ownership decisions)
- [ ] `cargo t -p oric` — consistency tests pass
- [ ] **Behavior invariant:** The `FxHashSet<Name>` produced by `ori_arc::builtin_borrowing_names()` must contain exactly the same names as `ori_llvm::codegen::arc_emitter::borrowing_builtin_names()` — write a temporary comparison test if needed
