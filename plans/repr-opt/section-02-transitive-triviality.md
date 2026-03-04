---
section: "02"
title: "Transitive Triviality & ARC Elision"
status: not-started
goal: "Classify compound types as trivial when all transitive children are scalar, eliding all ARC operations for these types"
inspired_by:
  - "Swift SIL trivial type classification (lib/SIL/SILType.cpp)"
  - "Lean4 isPossibleRef/isDefiniteRef (src/Lean/Compiler/IR/RC.lean)"
  - "Ori ori_arc::classify::ArcClassifier (compiler/ori_arc/src/classify/mod.rs)"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Unify Triviality Classification"
    status: not-started
  - id: "02.2"
    title: "Transitive Walk with Cycle Detection"
    status: not-started
  - id: "02.3"
    title: "ARC Elision in ori_arc Pipeline"
    status: not-started
  - id: "02.4"
    title: "Drop Function Elision"
    status: not-started
  - id: "02.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Transitive Triviality & ARC Elision

**Context:** Two independent triviality systems exist today:
1. `ori_arc::classify::ArcClassifier` — used during ARC IR lowering, classifies as `Scalar`/`DefiniteRef`/`PossibleRef`
2. `ori_llvm::codegen::type_info::TypeInfoStore::is_trivial()` — used during LLVM codegen, walks type tree

These MUST agree, but they use different algorithms and data structures. Unifying them ensures no redundant RC operations on types that can never hold heap references.

**Reference implementations:**
- **Swift** `lib/SIL/SILType.cpp`: `isTrivial()` walks type structure recursively, caches results
- **Lean4** `src/Lean/Compiler/IR/RC.lean`: `VarInfo.isPossibleRef` / `isDefiniteRef` — two-bit classification
- **ori_arc** `compiler/ori_arc/src/classify/mod.rs`: `ArcClassifier` with `FxHashMap` cache + cycle detection

**Depends on:** §01 (ReprPlan stores triviality decisions).

---

## 02.1 Unify Triviality Classification

**File(s):** `compiler/ori_repr/src/triviality.rs`, `compiler/ori_arc/src/classify/mod.rs`

Today, `ArcClassifier` and `TypeInfoStore::is_trivial()` duplicate logic. We need a single source of truth.

- [ ] Move the triviality classification algorithm into `ori_repr`:
  ```rust
  // ori_repr/src/triviality.rs
  pub enum Triviality {
      /// No heap references anywhere in the type tree
      Trivial,
      /// Contains at least one heap reference (str, [T], etc.)
      NonTrivial,
      /// Contains unresolved type variables — must assume non-trivial
      Unknown,
  }

  pub fn classify_triviality(idx: Idx, pool: &Pool) -> Triviality {
      let mut visited = FxHashSet::default();
      classify_recursive(idx, pool, &mut visited)
  }
  ```

- [ ] Make `ArcClassifier::classify()` delegate to `ori_repr::classify_triviality()`:
  - `Trivial` → `ArcClass::Scalar`
  - `NonTrivial` → `ArcClass::DefiniteRef`
  - `Unknown` → `ArcClass::PossibleRef`

- [ ] Make `TypeInfoStore::is_trivial()` delegate to `ReprPlan::is_trivial()`:
  - `ReprPlan` caches the result from the triviality pass
  - Codegen never re-computes triviality

- [ ] Add consistency test: for every type that `ArcClassifier` classifies as `Scalar`, `ReprPlan::is_trivial()` must return `true`, and vice versa

---

## 02.2 Transitive Walk with Cycle Detection

**File(s):** `compiler/ori_repr/src/triviality.rs`

The recursive walk must handle all compound types and detect cycles (recursive structs/enums).

- [ ] Implement transitive classification:
  ```rust
  fn classify_recursive(
      idx: Idx,
      pool: &Pool,
      visiting: &mut FxHashSet<Idx>,
  ) -> Triviality {
      let resolved = pool.resolve_fully(idx);
      let tag = pool.tag(resolved);

      // Fast path: primitives
      match tag {
          Tag::Int | Tag::Float | Tag::Bool | Tag::Char | Tag::Byte
          | Tag::Unit | Tag::Never | Tag::Duration | Tag::Size
          | Tag::Ordering => return Triviality::Trivial,

          // Always heap-allocated
          Tag::Str | Tag::List | Tag::Map | Tag::Set
          | Tag::Channel | Tag::Iterator => return Triviality::NonTrivial,

          // Unresolved
          Tag::Var | Tag::Error => return Triviality::Unknown,

          _ => {} // compound types — recurse
      }

      // Cycle detection
      if !visiting.insert(resolved) {
          // Recursive type — must be heap-allocated (requires indirection)
          return Triviality::NonTrivial;
      }

      let result = match tag {
          Tag::Option => classify_recursive(pool.data_as_idx(resolved), pool, visiting),
          Tag::Result => {
              let ok = classify_recursive(pool.extra(resolved, 0), pool, visiting);
              let err = classify_recursive(pool.extra(resolved, 1), pool, visiting);
              merge_triviality(ok, err)
          }
          Tag::Tuple => {
              let mut result = Triviality::Trivial;
              for i in 0..pool.extra_len(resolved) {
                  result = merge_triviality(
                      result,
                      classify_recursive(pool.extra(resolved, i), pool, visiting),
                  );
                  if result == Triviality::NonTrivial { break; }
              }
              result
          }
          Tag::Struct => {
              // Walk all fields
              classify_struct_fields(resolved, pool, visiting)
          }
          Tag::Enum => {
              // Walk all variant fields
              classify_enum_variants(resolved, pool, visiting)
          }
          Tag::Function => Triviality::NonTrivial, // closures capture heap refs
          Tag::Range => Triviality::Trivial, // Range only holds scalars
          _ => Triviality::Unknown,
      };

      visiting.remove(&resolved);
      result
  }

  fn merge_triviality(a: Triviality, b: Triviality) -> Triviality {
      match (a, b) {
          (Triviality::NonTrivial, _) | (_, Triviality::NonTrivial) => Triviality::NonTrivial,
          (Triviality::Unknown, _) | (_, Triviality::Unknown) => Triviality::Unknown,
          _ => Triviality::Trivial,
      }
  }
  ```

- [ ] Write tests covering:
  - `Option<int>` → Trivial
  - `Option<str>` → NonTrivial
  - `Option<Option<int>>` → Trivial
  - `(int, float, bool)` → Trivial
  - `(int, str)` → NonTrivial
  - `Result<int, Ordering>` → Trivial
  - `Result<int, str>` → NonTrivial
  - `struct Point { x: int, y: int }` → Trivial
  - `struct Named { name: str, age: int }` → NonTrivial
  - Recursive struct → NonTrivial
  - `[int]` → NonTrivial (list itself is heap-allocated)
  - Range → Trivial

---

## 02.3 ARC Elision in ori_arc Pipeline

**File(s):** `compiler/ori_arc/src/rc_insert/mod.rs`, `compiler/ori_arc/src/classify/mod.rs`

When the triviality pass marks a type as Trivial, the ARC pipeline must skip ALL RC operations for values of that type.

- [ ] In `rc_insert`, check triviality before inserting `RcInc`/`RcDec`:
  - If the variable's type is `Trivial` in the `ReprPlan`, skip RC insertion entirely
  - This must work for compound types: `Option<int>` variables must skip RC even though `Option` itself has a generic form that might need RC

- [ ] In `compute_var_reprs()`, set `ValueRepr::Scalar` for all trivial types:
  - Currently, `Option<T>` always gets `ValueRepr::Aggregate` regardless of T
  - With triviality, `Option<int>` should get `ValueRepr::Scalar` (no RC fields)

- [ ] Update `drop/mod.rs` — `compute_drop_info()` should return `None` for trivial types:
  - Currently, it may generate `DropKind::Fields([])` for structs with no RC fields
  - Should return `None` (no drop function needed at all)

---

## 02.4 Drop Function Elision

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs`

When `compute_drop_info()` returns `None`, the LLVM drop function generator must NOT emit a drop function. This saves code size and eliminates dead function definitions.

- [ ] In `get_or_create_drop_fn()`, return a null function pointer for trivial types
- [ ] In `ori_rc_dec` call sites, skip the call entirely when drop_fn is null
- [ ] Verify: `ORI_LOG=ori_llvm=debug ori build` should NOT emit `_ori_drop$` functions for `Option<int>`, `(int, float)`, `Result<int, bool>`, etc.

---

## 02.5 Completion Checklist

- [ ] Single `classify_triviality()` function in `ori_repr` is the sole source of truth
- [ ] `ArcClassifier` delegates to `ori_repr` — no duplicate logic
- [ ] `TypeInfoStore::is_trivial()` delegates to `ReprPlan` — no duplicate logic
- [ ] `Option<int>`, `(int, float, bool)`, `Result<int, Ordering>` generate ZERO `ori_rc_inc`/`ori_rc_dec` calls in LLVM IR
- [ ] No `_ori_drop$` functions emitted for trivial compound types
- [ ] Consistency test: `ArcClassifier` and `ReprPlan` agree on every type in the Pool
- [ ] `./test-all.sh` green
- [ ] `./llvm-test.sh` green
- [ ] `./diagnostics/valgrind-aot.sh` clean (no leaks introduced by elision)

**Exit Criteria:** `ori build` on a program using `Option<int>`, `(int, float)`, and `struct Point { x: int, y: int }` produces LLVM IR with zero `ori_rc_*` calls for these types, verified by `grep -c "ori_rc" output.ll` returning 0 for trivial-only programs.
