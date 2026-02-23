---
section: "04"
title: "Remove Ownership from ori_llvm"
status: not-started
goal: "ori_llvm no longer stores ownership metadata — only dispatch logic"
files:
  - compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs
  - compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs
  - compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections.rs
  - compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs
  - compiler/ori_llvm/src/codegen/arc_emitter/builtins/compound_traits.rs
  - compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs
  - compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs
  - compiler/ori_llvm/src/codegen/arc_emitter/builtins/trampolines.rs
---

# Section 04: Remove Ownership from ori_llvm

**Status:** Not Started
**Goal:** `ori_llvm` no longer stores ownership metadata. `BuiltinRegistration` tracks only `(type_name, method_name)` — dispatch logic only. The `receiver_borrowed` field and `borrowing_builtin_names()` function are deleted.

**Prerequisite:** Section 03 must be complete — all call sites must point to `ori_arc::builtin_borrowing_names()` before removing the `ori_llvm` source.

---

## 04.1 Remove `receiver_borrowed` from `BuiltinRegistration`

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`

```rust
// BEFORE:
pub(crate) struct BuiltinRegistration {
    pub type_name: &'static str,
    pub method_name: &'static str,
    pub receiver_borrowed: bool,   // ← DELETE THIS
}

// AFTER:
pub(crate) struct BuiltinRegistration {
    pub type_name: &'static str,
    pub method_name: &'static str,
}
```

---

## 04.2 Simplify `declare_builtins!` Macro

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`

The macro currently accepts `borrow: $borrow:expr` as a third tuple element:

```rust
// BEFORE:
macro_rules! declare_builtins {
    ($emitter:ident, $ctx:ident;
     $( ($type_name:expr, $method:expr, borrow: $borrow:expr) => $body:expr ),* $(,)?) => {
        // dispatch function ...
        pub(super) const REGISTERED: &[super::BuiltinRegistration] = &[
            $(super::BuiltinRegistration {
                type_name: $type_name,
                method_name: $method,
                receiver_borrowed: $borrow,  // ← DELETE
            },)*
        ];
    };
}

// AFTER:
macro_rules! declare_builtins {
    ($emitter:ident, $ctx:ident;
     $( ($type_name:expr, $method:expr) => $body:expr ),* $(,)?) => {
        #[allow(dead_code, unused_variables)]
        pub(super) fn dispatch<'scx: 'ctx, 'ctx>(
            $emitter: &mut $crate::codegen::arc_emitter::ArcIrEmitter<'_, 'scx, 'ctx, '_>,
            $ctx: &super::BuiltinCtx<'_>,
        ) -> Option<$crate::codegen::value_id::ValueId> {
            match ($ctx.type_name, $ctx.method) {
                $(($type_name, $method) => $body,)*
                _ => None,
            }
        }

        pub(super) const REGISTERED: &[super::BuiltinRegistration] = &[
            $(super::BuiltinRegistration {
                type_name: $type_name,
                method_name: $method,
            },)*
        ];
    };
}
```

---

## 04.3 Delete `borrowing_builtin_names()` Function

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`

Delete the entire function (lines 253-275):

```rust
// DELETE THIS ENTIRE FUNCTION:
pub fn borrowing_builtin_names(
    interner: &ori_ir::StringInterner,
) -> rustc_hash::FxHashSet<Name> {
    let table = builtin_table();
    let mut names = rustc_hash::FxHashSet::default();
    for (&type_name, methods) in &table.entries {
        if type_name == "Iterator" {
            continue;
        }
        for (&method_name, reg) in methods {
            if !reg.receiver_borrowed {
                continue;
            }
            if method_name == "iter" {
                continue;
            }
            names.insert(interner.intern(method_name));
        }
    }
    names
}
```

Also update `BuiltinTable` doc comment (line 159) — remove reference to `receiver_borrowed`:

```rust
// BEFORE:
/// - `receiver_borrowed` metadata for ARC ownership inference

// AFTER:
// (delete this line entirely — ownership is no longer tracked here)
```

---

## 04.4 Update All 7 Submodule `declare_builtins!` Invocations

Each submodule currently has entries like:
```rust
("int", "abs", borrow: true) => emitter.emit_int_abs(ctx),
```

Change to:
```rust
("int", "abs") => emitter.emit_int_abs(ctx),
```

### primitives.rs — 25 entries

Remove `borrow: true` from all 25 entries. Example:

```rust
// BEFORE:
("int", "clone", borrow: true) => Some(ctx.arg_vals[0]),
("int", "to_int", borrow: true) => Some(ctx.arg_vals[0]),

// AFTER:
("int", "clone") => Some(ctx.arg_vals[0]),
("int", "to_int") => Some(ctx.arg_vals[0]),
```

### collections.rs — 21 entries

Remove `borrow: true` from all 21 entries.

### traits.rs — 82 entries

Remove `borrow: true` from all 82 entries. This is the largest submodule.

### compound_traits.rs — 16 entries

Remove `borrow: true` from all 16 entries.

### iterator.rs — 15 entries

Remove `borrow: true` from all 15 entries.

### option_result.rs — 11 entries

Remove `borrow: true` from all 11 entries.

### trampolines.rs — 0 entries

No changes needed (empty `REGISTERED` array), but update macro syntax if the macro signature changed.

---

## 04.5 Update BuiltinTable Methods

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`

No structural changes to `BuiltinTable` methods. The `lookup()` method returns `&BuiltinRegistration`, which no longer has `receiver_borrowed`. Callers that accessed `receiver_borrowed` through `lookup()` should have been removed in Section 03.

**Verify:** No remaining code accesses `reg.receiver_borrowed` after these changes.

---

## 04.6 Implementation Order

1. Delete `borrowing_builtin_names()` function
2. Remove `receiver_borrowed` from `BuiltinRegistration` struct
3. Update `declare_builtins!` macro signature
4. Update all 7 submodule invocations (mechanical: find-replace `, borrow: true` with empty)
5. Remove `receiver_borrowed` references from `BuiltinTable` doc comments
6. `cargo c -p ori_llvm` — verify compilation
7. `cargo t -p ori_llvm` — verify tests pass (sync tests will need Section 05 updates)

**Note:** The sync tests in `builtins/tests.rs` don't reference `receiver_borrowed` directly — they test `(type_name, method_name)` pairs. So they should compile after the field removal without modification.

---

## 04.7 Verification

- [ ] `cargo c -p ori_llvm` — compiles without `receiver_borrowed`
- [ ] `./llvm-test.sh` — all LLVM tests pass
- [ ] `grep -r "receiver_borrowed" compiler/ori_llvm/` — returns nothing
- [ ] `grep -r "borrow:" compiler/ori_llvm/src/codegen/arc_emitter/builtins/` — returns nothing (no more `borrow:` in macro calls)
- [ ] `grep -r "borrowing_builtin_names" compiler/ori_llvm/` — returns nothing
- [ ] No dead code warnings from removed field/function
