---
section: "04"
title: "Borrow Inference Hardening"
status: not-started
goal: "Eliminate silent failures in borrow inference integration with codegen"
sections:
  - id: "04.1"
    title: "Warn on borrow signature lookup miss"
    status: not-started
  - id: "04.2"
    title: "Build O(1) method dispatch index"
    status: not-started
  - id: "04.3"
    title: "Add debug_assert coverage"
    status: not-started
---

# Section 04: Borrow Inference Hardening

**Status:** Not Started
**Goal:** No silent fallbacks — every borrow signature miss is logged, every method lookup is O(1).

**Context:** `FunctionCompiler` receives `annotated_sigs: &FxHashMap<Name, AnnotatedSig>` and looks up each function by `Name`. A miss means the function compiles with all-Owned parameters (no borrow optimization) — silently. This is correct but wasteful, and misses indicate pipeline bugs. Additionally, `lookup_method_by_unqualified_name` does a linear scan as fallback.

---

## 04.1 Warn on Borrow Signature Lookup Miss

**File:** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

- [ ] In `define_all()`, when looking up a function's `AnnotatedSig`:
  ```rust
  let sig = match annotated_sigs.get(&func_name) {
      Some(sig) => sig,
      None => {
          tracing::warn!(
              func = %func_name,
              "borrow signature missing — compiling with all-Owned params"
          );
          &default_all_owned_sig
      }
  };
  ```

- [ ] Add `debug_assert!` that all functions being compiled have entries:
  ```rust
  #[cfg(debug_assertions)]
  {
      let missing: Vec<_> = functions_to_compile
          .iter()
          .filter(|name| !annotated_sigs.contains_key(name))
          .collect();
      debug_assert!(
          missing.is_empty(),
          "functions missing borrow sigs: {:?}",
          missing
      );
  }
  ```

- [ ] Verify this fires correctly by temporarily removing a signature and checking the warning appears

---

## 04.2 Build O(1) Method Dispatch Index

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

- [ ] Add secondary index alongside `method_functions`:
  ```rust
  /// method_name → Vec<(type_name, FunctionId, FunctionAbi)>
  /// Built during compile_impls for O(1) unqualified lookup.
  method_by_name: FxHashMap<Name, Vec<(Name, FunctionId, FunctionAbi)>>,
  ```

- [ ] Populate during `compile_impls()`:
  ```rust
  for ((type_name, method_name), (func_id, abi)) in &self.method_functions {
      self.method_by_name
          .entry(*method_name)
          .or_default()
          .push((*type_name, *func_id, abi.clone()));
  }
  ```

- [ ] Replace `lookup_method_by_unqualified_name` linear scan with index lookup:
  ```rust
  fn lookup_method_by_unqualified_name(&self, method: Name) -> Option<(FunctionId, &FunctionAbi)> {
      let entries = self.method_by_name.get(&method)?;
      // If exactly one match, use it. If multiple, need type disambiguation.
      match entries.as_slice() {
          [(_, func_id, abi)] => Some((*func_id, abi)),
          multiple => {
              tracing::warn!(method = %method, count = multiple.len(), "ambiguous unqualified method lookup");
              // Fall back to first match (current behavior)
              multiple.first().map(|(_, fid, abi)| (*fid, abi))
          }
      }
  }
  ```

---

## 04.3 Add Debug Assert Coverage

**Files:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`, `function_compiler/mod.rs`

- [ ] Assert that `type_idx_to_name` lookups never fail:
  ```rust
  debug_assert!(
      self.type_idx_to_name.contains_key(&receiver_idx),
      "receiver type Idx {:?} not in type_idx_to_name map",
      receiver_idx
  );
  ```

- [ ] Assert that method_functions lookups with qualified name succeed when expected:
  ```rust
  debug_assert!(
      self.method_functions.contains_key(&(type_name, method_name)),
      "method ({}, {}) not registered in method_functions",
      type_name, method_name
  );
  ```

- [ ] Run `./llvm-test.sh` with debug assertions enabled to verify no assertions fire

---

## 04.4 Completion Checklist

- [ ] `tracing::warn!` on every borrow sig lookup miss
- [ ] `debug_assert!` that all compiled functions have sigs (debug builds)
- [ ] `method_by_name` secondary index built during `compile_impls`
- [ ] `lookup_method_by_unqualified_name` is O(1) via index
- [ ] Debug assertions on `type_idx_to_name` and `method_functions` lookups
- [ ] No regression in `./llvm-test.sh`
- [ ] No spurious warnings in normal compilation

**Exit Criteria:** `ORI_LOG=ori_llvm=warn ori build examples/hello.ori` produces no borrow-miss warnings. Linear scans eliminated.
