---
section: "05"
title: "Builtin Method Architecture"
status: not-started
goal: "Replace scattered builtin match cascades with declarative dispatch table + sync enforcement"
sections:
  - id: "05.1"
    title: "Define BuiltinTable abstraction"
    status: not-started
  - id: "05.2"
    title: "Migrate existing builtins to table"
    status: not-started
  - id: "05.3"
    title: "Add LLVM builtin coverage sync test"
    status: not-started
  - id: "05.4"
    title: "Completion checklist"
    status: not-started
---

# Section 05: Builtin Method Architecture

**Status:** Not Started
**Goal:** Builtin method codegen uses a declarative dispatch table. Every `TYPECK_BUILTIN_METHODS` entry is proven to have a codegen handler.

**Context:** The 7 submodules in `arc_emitter/builtins/` (2,425 lines total) hand-code the same patterns: extract receiver, match method name, emit inline IR. There's no shared abstraction. The DPR identified that three independent registrations (`TYPECK_BUILTIN_METHODS` in `ori_types`, `eval_builtin_method()` in `ori_eval`, and `try_emit_builtin_method()` in `ori_llvm`) can silently drift. Swift avoids this because methods are just functions; Lean avoids it because builtins are runtime calls. Ori's inline codegen is faster but needs structure.

---

## 05.1 Define BuiltinTable Abstraction

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`

- [ ] Define the registration types:
  ```rust
  /// A single builtin method's inline codegen handler.
  type BuiltinEmitFn = fn(
      emitter: &mut ArcIrEmitter,
      receiver: EmittedValue,
      receiver_ty: Idx,
      args: &[EmittedValue],
  ) -> EmittedValue;

  /// Registration entry for one (type, method) pair.
  struct BuiltinEntry {
      type_name: &'static str,
      method_name: &'static str,
      emit: BuiltinEmitFn,
  }

  /// Compiled dispatch table. O(1) lookup by (type_name, method_name).
  pub struct BuiltinTable {
      entries: FxHashMap<(&'static str, &'static str), BuiltinEmitFn>,
  }
  ```

- [ ] Implement `BuiltinTable::build()` that collects all registrations:
  ```rust
  impl BuiltinTable {
      pub fn build() -> Self {
          let mut entries = FxHashMap::default();
          // Each submodule registers its entries
          primitives::register(&mut entries);
          collections::register(&mut entries);
          iterator::register(&mut entries);
          option_result::register(&mut entries);
          traits::register(&mut entries);
          compound_traits::register(&mut entries);
          trampolines::register(&mut entries);
          Self { entries }
      }

      pub fn lookup(&self, type_name: &str, method: &str) -> Option<&BuiltinEmitFn> {
          self.entries.get(&(type_name, method))
      }
  }
  ```

- [ ] Store `BuiltinTable` in `ArcIrEmitter`, built once during `new()`

---

## 05.2 Migrate Existing Builtins to Table

**Files:** All `builtins/*.rs` files

For each submodule, add a `register()` function that populates the table:

- [ ] `builtins/primitives.rs` — `register()` for int/float/bool/char/byte methods
  - Extract each match arm into a standalone `fn emit_int_abs(...)`, `fn emit_int_to_str(...)`, etc.
  - Register: `("int", "abs") → emit_int_abs`, `("int", "to_str") → emit_int_to_str`, ...

- [ ] `builtins/collections.rs` — `register()` for list/map/set methods
  - `("List", "length") → emit_list_length`, `("List", "push") → emit_list_push`, ...

- [ ] `builtins/iterator.rs` — `register()` for iterator adapter methods
  - `("Iterator", "map") → emit_iter_map`, `("Iterator", "filter") → emit_iter_filter`, ...

- [ ] `builtins/option_result.rs` — `register()` for Option/Result methods
  - `("Option", "unwrap") → emit_option_unwrap`, `("Option", "map") → emit_option_map`, ...

- [ ] `builtins/traits.rs` — `register()` for trait method dispatch (Eq, Comparable, Hashable, Printable, Clone)

- [ ] `builtins/compound_traits.rs` — `register()` for derived trait methods on compound types

- [ ] `builtins/trampolines.rs` — `register()` for trampoline function generation

- [ ] Replace the match cascade in `try_emit_builtin_method()` with `self.builtin_table.lookup(type_name, method_name)`

- [ ] **Cross-reference with Section 04.4**: When migrating builtins to the table, also register their borrowing semantics. Each `BuiltinEntry` should declare whether the receiver is borrowed:
  ```rust
  struct BuiltinEntry {
      type_name: &'static str,
      method_name: &'static str,
      emit: BuiltinEmitFn,
      receiver_borrowed: bool,  // NEW — feeds into annotated_sigs
  }
  ```
  This ensures the BuiltinTable is the single source of truth for both codegen dispatch AND ownership annotation — no manual sync between Section 04.4's sigs and Section 05's dispatch.

**Migration strategy:** Do this incrementally — one submodule at a time. After each, run `./llvm-test.sh` to verify no regression. The existing emit functions become the registered handlers; only the dispatch mechanism changes.

---

## 05.3 Add LLVM Builtin Coverage Sync Test

**File:** `compiler/oric/tests/consistency.rs`

- [ ] Add a test that verifies coverage:
  ```rust
  #[test]
  fn llvm_builtin_methods_coverage() {
      let typeck_methods = TYPECK_BUILTIN_METHODS; // from ori_types
      let builtin_table = BuiltinTable::build();
      let exclusions = LLVM_BUILTIN_EXCLUSIONS; // methods handled via runtime, not inline

      let mut missing = Vec::new();
      for (type_name, method_name) in typeck_methods {
          if !builtin_table.has(type_name, method_name)
              && !exclusions.contains(&(type_name, method_name))
          {
              missing.push((type_name, method_name));
          }
      }

      assert!(
          missing.is_empty(),
          "TYPECK_BUILTIN_METHODS entries missing LLVM codegen handlers: {:?}",
          missing
      );
  }
  ```

- [ ] Define `LLVM_BUILTIN_EXCLUSIONS` — methods intentionally dispatched via runtime calls rather than inline codegen (e.g., complex iterator consumers)

- [ ] Add companion test: `builtin_table_entries_are_in_typeck`:
  ```rust
  #[test]
  fn builtin_table_entries_are_in_typeck() {
      // Every entry in the builtin table must correspond to a TYPECK_BUILTIN_METHODS entry
      // Prevents orphan codegen handlers that the type checker doesn't know about
  }
  ```

---

## 05.4 Completion Checklist

- [ ] `BuiltinTable` struct defined with O(1) lookup
- [ ] All 7 submodules migrated to `register()` pattern
- [ ] `try_emit_builtin_method()` uses table lookup, no match cascade
- [ ] `llvm_builtin_methods_coverage` test passes
- [ ] `builtin_table_entries_are_in_typeck` test passes
- [ ] `LLVM_BUILTIN_EXCLUSIONS` documented with rationale for each
- [ ] No behavioral change — all existing AOT tests pass
- [ ] `./test-all.sh` and `./llvm-test.sh` green

**Exit Criteria:** Adding a new builtin method requires: (1) add to `TYPECK_BUILTIN_METHODS`, (2) add `register()` call in the appropriate submodule, (3) the sync test catches any missing registration.
