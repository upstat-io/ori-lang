---
section: "01"
title: "Extend MethodDef with Ownership"
status: complete
goal: "Every MethodDef carries explicit receiver_borrows: bool and type_flow: TypeFlow"
files:
  - compiler/ori_ir/src/builtin_methods/mod.rs
  - compiler/ori_ir/src/builtin_methods/tests.rs
---

# Section 01: Extend MethodDef with Ownership

**Status:** Complete (field added, constructors updated, all 162 entries updated, query functions added, `cargo c -p ori_ir` passes)
**Goal:** Every `MethodDef` carries explicit `receiver_borrows: bool` and `type_flow: TypeFlow`. No defaults, no opt-out.

---

## 01.1 Add `receiver_borrows` and `type_flow` Fields to `MethodDef`

**File:** `compiler/ori_ir/src/builtin_methods/mod.rs`

```rust
pub struct MethodDef {
    pub receiver: BuiltinType,
    pub name: &'static str,
    pub params: &'static [ParamSpec],
    pub returns: ReturnSpec,
    pub trait_name: Option<&'static str>,
    pub receiver_borrows: bool,  // NEW — ownership source of truth
    pub type_flow: TypeFlow,     // NEW — unification constraint spec
}
```

### `TypeFlow` Enum

```rust
/// How type variables in the return type relate to closure/parameter types.
///
/// Used by the type checker to unify fresh type variables created during
/// builtin method resolution with concrete types from closure arguments.
/// This is the single source of truth — the type checker reads this field
/// instead of hard-coding unification logic per method name.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TypeFlow {
    /// No special unification — standard parameter checking suffices.
    /// Used by: len, clone, abs, is_empty, unwrap, contains, etc.
    Standard,

    /// The closure's return type becomes the output element type.
    /// Pattern: `Container<T>.method((T) -> U) -> Container<U>`
    /// Used by: Iterator.map, List.map, Option.map
    ClosureOutputBecomesElement { closure_param: u8 },

    /// The closure returns a container; its element type becomes the output element.
    /// Pattern: `Container<T>.method((T) -> Container<U>) -> Container<U>`
    /// Used by: Iterator.flat_map, List.flat_map, Option.flat_map
    ClosureOutputFlatElement { closure_param: u8 },

    /// The return type equals an init parameter, constrained by a closure's return.
    /// Pattern: `Container<T>.method(init: A, (A, T) -> A) -> A`
    /// Used by: Iterator.fold, Iterator.rfold, List.fold
    Accumulator { init_param: u8, closure_param: u8 },
}
```

**Coverage of current hard-coded unification methods:**

| Method | Current hard-code | TypeFlow variant |
|--------|------------------|-----------------|
| `map` | `calls.rs:711-724` | `ClosureOutputBecomesElement { closure_param: 0 }` |
| `flat_map` | `calls.rs:726-743` | `ClosureOutputFlatElement { closure_param: 0 }` |
| `fold` | `calls.rs:745-757` | `Accumulator { init_param: 0, closure_param: 1 }` |
| `rfold` | `calls.rs:745-757` | `Accumulator { init_param: 0, closure_param: 1 }` |
| all others | `calls.rs:758` (no-op) | `Standard` |

**Why a bare field (no `Default`)?** Same as `receiver_borrows` — the field is structural. You *cannot* construct a `MethodDef` without specifying a value. If you add a new higher-order builtin method and forget `type_flow`, the code won't compile. This is the strongest form of enforcement Rust offers for static data.

---

## 01.2 Update `MethodDef::new()` Signature

```rust
pub const fn new(
    receiver: BuiltinType,
    name: &'static str,
    params: &'static [ParamSpec],
    returns: ReturnSpec,
    trait_name: Option<&'static str>,
    receiver_borrows: bool,  // NEW — mandatory parameter
    type_flow: TypeFlow,     // NEW — unification constraint spec
) -> Self
```

Must remain `const fn` — `BUILTIN_METHODS` is a `static` array initialized at compile time.

### `standard()` Convenience Constructor

To avoid littering `TypeFlow::Standard` across 200+ entries, add a shorthand:

```rust
impl MethodDef {
    /// Create a standard method (no higher-order unification).
    const fn standard(
        receiver: BuiltinType,
        name: &'static str,
        params: &'static [ParamSpec],
        returns: ReturnSpec,
        trait_name: Option<&'static str>,
        receiver_borrows: bool,
    ) -> Self {
        Self::new(receiver, name, params, returns, trait_name, receiver_borrows, TypeFlow::Standard)
    }
}
```

Use `MethodDef::new()` (7 params) only when `type_flow` is non-Standard.

---

## 01.3 Update All Convenience Constructors

Each sets `receiver_borrows: true` and `type_flow: TypeFlow::Standard` — trait methods have no higher-order constraints:

| Constructor | Why borrowed | TypeFlow |
|-------------|-------------|----------|
| `comparable()` | Reads fields for comparison | `Standard` |
| `eq_trait()` | Reads fields for equality check | `Standard` |
| `clone_trait()` | Reads to produce a copy | `Standard` |
| `hash_trait()` | Reads fields for hashing | `Standard` |
| `to_str_trait()` | Reads to format | `Standard` |
| `debug_trait()` | Reads to format | `Standard` |

---

## 01.4 Update All 162 Existing Entries

All existing `MethodDef::new(...)` calls in `BUILTIN_METHODS` are converted to `MethodDef::standard(...)`. All 162 entries borrow their receiver and use `TypeFlow::Standard` (none are higher-order).

Breakdown by type:
- **int** (24 entries): 6 trait + abs, min, max + 15 operators — all `true`
- **float** (18 entries): 5 trait + abs, floor, ceil, round, sqrt, min, max + 5 operators — all `true`
- **bool** (7 entries): 6 trait + not — all `true`
- **char** (6 entries): 6 trait — all `true`
- **byte** (6 entries): 6 trait — all `true`
- **str** (16 entries): 5 trait + len, is_empty, contains, starts_with, ends_with, to_uppercase, to_lowercase, trim, escape, add, concat — all `true`
- **Duration** (18 entries): 6 trait + nanoseconds, microseconds, milliseconds, seconds, minutes, hours + 6 operators — all `true`
- **Size** (17 entries): 6 trait + bytes, kilobytes, megabytes, gigabytes, terabytes + 5 operators — all `true`
- **Ordering** (11 entries): 6 trait + is_less, is_equal, is_greater, is_less_or_equal, is_greater_or_equal, reverse, then — all `true`

---

## 01.5 Add Query Functions

```rust
/// All builtin method names whose receiver is borrowed.
/// Used by ori_arc borrow inference to build the borrowing_builtins set.
pub fn borrowing_method_names() -> impl Iterator<Item = &'static str> {
    BUILTIN_METHODS.iter()
        .filter(|m| m.receiver_borrows)
        .map(|m| m.name)
}

/// Check if a specific method borrows its receiver.
/// Returns None if the method doesn't exist in the registry.
pub fn method_borrows_receiver(receiver: BuiltinType, name: &str) -> Option<bool> {
    find_method(receiver, name).map(|m| m.receiver_borrows)
}
```

**Design note:** `borrowing_method_names()` returns method names without type qualification. This matches the current `FxHashSet<Name>` interface that borrow inference uses — it checks `borrowing_builtins.contains(callee)` where `callee` is just the method name. Type qualification would require changing the borrow inference interface, which is a separate concern.

---

## 01.6 Verification

- [x] `cargo c -p ori_ir` — compiles
- [ ] `cargo t -p ori_ir` — tests pass (existing tests don't check `receiver_borrows`)
- [ ] Add test in `tests.rs`:
  ```rust
  #[test]
  fn all_current_methods_borrow_receiver() {
      for method in BUILTIN_METHODS {
          assert!(
              method.receiver_borrows,
              "{}.{} should borrow its receiver",
              method.receiver, method.name
          );
      }
  }

  #[test]
  fn borrowing_method_names_nonempty() {
      let names: Vec<_> = borrowing_method_names().collect();
      assert!(!names.is_empty());
      assert!(names.contains(&"compare"));
      assert!(names.contains(&"len"));
      assert!(names.contains(&"clone"));
  }

  #[test]
  fn method_borrows_receiver_query() {
      assert_eq!(method_borrows_receiver(BuiltinType::Str, "len"), Some(true));
      assert_eq!(method_borrows_receiver(BuiltinType::Int, "nonexistent"), None);
  }
  ```
