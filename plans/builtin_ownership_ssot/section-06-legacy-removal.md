---
section: "06"
title: "Legacy Removal & Verification"
status: not-started
goal: "Zero traces of the old ownership system remain"
files:
  - (all modified files from Sections 01-05)
---

# Section 06: Legacy Removal & Verification

**Status:** Not Started
**Goal:** No trace of the old `receiver_borrowed` / `borrowing_builtin_names` plumbing remains. Full test suite green. The migration is complete.

---

## 06.1 Grep Verification Checklist

Each grep must return **zero results** (excluding plan documentation and git history):

### `receiver_borrowed` (old field name)

```bash
grep -r "receiver_borrowed" compiler/ --include='*.rs'
# Expected: 0 results
# Was in: BuiltinRegistration struct, declare_builtins! macro, BuiltinTable doc comment
```

### `borrowing_builtin_names` in ori_llvm (old function)

```bash
grep -r "borrowing_builtin_names" compiler/ori_llvm/ --include='*.rs'
# Expected: 0 results
# Was in: builtins/mod.rs (definition), evaluator.rs, function_compiler/mod.rs (call sites)
```

### `borrow:` in declare_builtins! syntax

```bash
grep -r "borrow:" compiler/ori_llvm/src/codegen/arc_emitter/builtins/ --include='*.rs'
# Expected: 0 results
# Was in: all 7 submodule files (macro invocation syntax)
```

### Old call pattern in oric

```bash
grep -r "ori_llvm::codegen::arc_emitter::borrowing_builtin_names" compiler/oric/ --include='*.rs'
# Expected: 0 results
# Was in: compile_common.rs (2 call sites)
```

### Verify new function is the sole source

```bash
grep -r "borrowing_builtin_names\|builtin_borrowing_names" compiler/ --include='*.rs'
# Expected: only ori_arc/src/lib.rs (definition) and call sites in oric/ori_llvm
```

### Hard-coded method names in unify_higher_order_constraints (replaced by TypeFlow)

```bash
grep -n '"map"\|"flat_map"\|"fold"\|"rfold"' compiler/ori_types/src/infer/expr/calls.rs
# Expected: 0 results (replaced by TypeFlow dispatch from registry)
```

---

## 06.2 Dead Code Check

```bash
./clippy-all.sh 2>&1 | grep -E "dead_code|unused"
# Expected: No warnings from removed plumbing
```

Specific items to verify are NOT flagged as dead:
- `BuiltinRegistration` struct (still used for dispatch table)
- `BuiltinTable` (still used for sync tests and early rejection)
- `builtin_table()` function (still used)
- `borrowing_method_names()` in ori_ir (consumed by ori_arc)
- `method_borrows_receiver()` in ori_ir (consumed by tests and potentially other crates)

Specific items that SHOULD be gone (flagged if accidentally left):
- Old `borrowing_builtin_names()` in ori_llvm
- Old `receiver_borrowed` field accessors

---

## 06.3 Formatting

```bash
./fmt-all.sh
# Expected: No formatting changes (code was formatted during implementation)
```

---

## 06.4 Full Test Suite

Run in order of increasing scope:

### Unit tests (fast, targeted)

```bash
cargo t -p ori_ir                    # IR registry tests
cargo t -p ori_arc                   # Borrow inference tests
```

### LLVM tests (medium, codegen-specific)

```bash
./llvm-test.sh                       # LLVM unit + integration tests
```

### Integration tests (broad, end-to-end)

```bash
cargo t -p oric                      # Consistency tests, phase tests
```

### Full suite (comprehensive)

```bash
./test-all.sh                        # Everything
```

### Release verification (catches FastISel differences)

```bash
cargo blr && ./test-all.sh           # Build release, test release
```

---

## 06.5 Documentation Updates

### Update `.claude/rules/ir.md`

Add to the "Key Files" section:

```
- `builtin_methods/`: Builtin method registry (MethodDef) — single source of truth for ownership
```

Update the "Source of Truth" reference:

```
## Builtin Method Ownership (Source of Truth)

`builtin_methods/mod.rs` defines `MethodDef.receiver_borrows` — the **canonical ownership
declaration** for all builtin methods. This field is consumed by:
- `ori_arc::builtin_borrowing_names()` — builds the borrow inference set
- `ori_llvm` codegen — indirectly, via the annotated signatures from borrow inference

**DO NOT** add ownership metadata in `ori_llvm` or any other crate. All ownership is
declared once in `MethodDef`.
```

### Update `plans/aot_codegen_pipeline/section-05-builtin-architecture.md`

Add exit criteria reference:

```
- Builtin method ownership is now single-sourced in `ori_ir::MethodDef.receiver_borrows`.
  See `plans/builtin_ownership_ssot/` for the migration details.
```

### Update `plans/aot_codegen_pipeline/section-04-borrow-hardening.md`

Add note:

```
- Builtin ownership metadata has been moved from `ori_llvm::BuiltinRegistration` to
  `ori_ir::MethodDef.receiver_borrows` — borrow inference now reads from `ori_ir` directly.
```

---

## 06.6 Exit Criteria

All of these must be true for the migration to be complete:

### Structural Guarantees

- [ ] `MethodDef.receiver_borrows` is a required `bool` field (no default, no opt-out)
- [ ] `MethodDef::new()` requires `receiver_borrows` parameter (compile-time enforcement)
- [ ] `every_codegen_builtin_has_ir_method_def` test ensures codegen → MethodDef → ownership

### Legacy Removal

- [ ] `receiver_borrowed` field is completely gone from `BuiltinRegistration`
- [ ] `borrowing_builtin_names()` function is completely gone from `ori_llvm`
- [ ] `borrow:` syntax is completely gone from all `declare_builtins!` invocations
- [ ] No call site references `ori_llvm` for borrowing builtin names

### Correct Behavior

- [ ] `ori_arc::builtin_borrowing_names()` produces the same set as the old function
- [ ] Borrow inference makes the same ownership decisions (no regressions)
- [ ] ARC codegen emits the same LLVM IR (no behavior changes)
- [ ] All existing tests pass without modification

### Single Source of Truth

- [ ] Adding a new builtin method requires:
  1. `MethodDef` entry in `ori_ir::builtin_methods` with explicit `receiver_borrows` and `type_flow`
  2. Dispatch handler in `ori_llvm` `declare_builtins!`
  3. Enforcement test catches missing MethodDef
- [ ] No other location stores ownership metadata
- [ ] No other location stores type unification constraints (no hard-coded method name matches in `calls.rs`)
- [ ] The crate dependency for ownership is: `ori_ir` → `ori_arc` → consumers
- [ ] The crate dependency for type flow is: `ori_ir` → `ori_types` (direct lookup)

---

## 06.7 Post-Migration: What Changes Look Like

### Before (old system) — Adding `str.trim_start()`:

1. Add `MethodDef::new(Str, "trim_start", ...)` in `ori_ir` — **no ownership**
2. Add `("str", "trim_start", borrow: true)` in `ori_llvm/builtins/collections.rs` — **ownership here**
3. Hope they stay in sync — **no enforcement**

### After (new system) — Adding `str.trim_start()`:

1. Add `MethodDef::new(Str, "trim_start", ..., true)` in `ori_ir` — **ownership declared**
2. Add `("str", "trim_start")` in `ori_llvm/builtins/collections.rs` — **dispatch only**
3. `every_codegen_builtin_has_ir_method_def` test catches if step 1 is missing — **enforced**
4. Forgetting `receiver_borrows` in step 1 = compile error — **structural guarantee**
