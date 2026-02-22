---
section: "08"
title: "Salsa-Integrated Borrow Inference"
status: not-started
goal: "Make borrow inference a Salsa query for incremental compilation"
inspired_by:
  - "Ori-unique — neither Swift, Lean, nor Rust has incremental borrow inference"
depends_on: ["04"]
sections:
  - id: "08.1"
    title: "Make ArcFunction Salsa-compatible"
    status: not-started
  - id: "08.2"
    title: "Define borrow inference query"
    status: not-started
  - id: "08.3"
    title: "Integrate with FunctionCompiler"
    status: not-started
  - id: "08.4"
    title: "Tests"
    status: not-started
---

# Section 08: Salsa-Integrated Borrow Inference

**Status:** Not Started
**Goal:** When a function body changes but its borrow signature is unchanged, callers don't need recompilation.

**Context:** This is Ori-unique — no reference compiler has incremental borrow inference. Ori's Salsa-based architecture enables it naturally: `infer_borrows` becomes a Salsa query whose input is the `ArcFunction` and whose output is `AnnotatedSig`. Salsa's memoization caches the result; when a function body changes but produces the same borrow signature, all dependent queries (callers' RC insertion) are short-circuited.

**Depends on:** Section 04 (Borrow Inference Hardening) — the hardened lookup ensures all functions have sigs before this makes them incremental.

---

## 08.1 Make ArcFunction Salsa-Compatible

**File:** `compiler/ori_arc/src/ir/mod.rs`

Salsa query inputs must implement `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`.

- [ ] Verify `ArcFunction` already derives or can derive these traits:
  - `Clone` — needed for Salsa memoization
  - `Eq`/`PartialEq` — needed for change detection
  - `Hash` — needed for Salsa's dependency tracking
  - `Debug` — needed for Salsa's logging
  - Check: `ArcBlock`, `ArcInstr`, `ArcTerminator`, `ArcValue`, `ArcVarId`, `ArcBlockId` all need these

- [ ] If any type is missing a derive, add it (likely `Hash` on some types)

- [ ] If `ArcFunction` is too large to clone/hash efficiently, consider using `Arc<ArcFunction>` as the query input (Salsa handles Arc natively)

---

## 08.2 Define Borrow Inference Query

**File:** `compiler/oric/src/query/mod.rs` (or new `query/arc.rs`)

- [ ] Define the Salsa query:
  ```rust
  /// Infer borrow signatures for a function's ARC IR.
  ///
  /// Input: function name + ARC IR (from lowering query)
  /// Output: AnnotatedSig (which params are Owned vs Borrowed)
  ///
  /// Salsa memoizes this: if a function body changes but the borrow
  /// signature is the same, callers are NOT invalidated.
  #[salsa::tracked]
  pub fn infer_borrow_sig(
      db: &dyn crate::Db,
      func_name: Name,
      arc_func: ArcFunction,
  ) -> AnnotatedSig {
      let sigs = /* collect callee signatures */;
      ori_arc::infer_borrows(&arc_func, &sigs)
  }
  ```

- [ ] Define the lowering query (ARC IR construction):
  ```rust
  #[salsa::tracked]
  pub fn lower_to_arc_ir(
      db: &dyn crate::Db,
      func_name: Name,
  ) -> ArcFunction {
      let can_expr = /* get canonicalized function body */;
      ori_arc::lower_function_can(&can_expr, /* ... */)
  }
  ```

- [ ] Wire the dependency chain:
  ```
  lower_to_arc_ir(f) → ArcFunction
       ↓
  infer_borrow_sig(f) → AnnotatedSig
       ↓
  compile_function(f) → LLVM IR
  ```

---

## 08.3 Integrate with FunctionCompiler

**File:** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

- [ ] Replace the pre-computed `annotated_sigs: FxHashMap<Name, AnnotatedSig>` with Salsa query lookups:
  ```rust
  // Before (batch):
  let sigs = ori_arc::infer_all_borrows(&functions);

  // After (incremental):
  let sig = db.infer_borrow_sig(func_name);
  ```

- [ ] This means `FunctionCompiler` needs access to the Salsa `db` — thread it through or store as a field

- [ ] Handle the fixed-point: borrow inference for mutually recursive functions requires iterating until sigs stabilize. This happens WITHIN a single Salsa query invocation (Salsa handles the outer memoization).

---

## 08.4 Tests

- [ ] Test incremental behavior:
  - Compile program with function `f` calling function `g`
  - Modify `g`'s body without changing its borrow signature
  - Verify `f` is NOT recompiled (Salsa memoization hit)
  - Verify via Salsa's debug logging or a recompilation counter

- [ ] Test invalidation:
  - Modify `g` to change its borrow signature (e.g., a param goes from Borrowed to Owned)
  - Verify `f` IS recompiled

- [ ] Test fixed-point convergence:
  - Mutually recursive functions `f` and `g`
  - Verify borrow inference converges to correct sigs

- [ ] Benchmark: measure compile time with and without Salsa caching on a multi-function program

---

## 08.5 Completion Checklist

- [ ] `ArcFunction` and all sub-types derive `Clone, Eq, PartialEq, Hash, Debug`
- [ ] `lower_to_arc_ir` Salsa query defined
- [ ] `infer_borrow_sig` Salsa query defined
- [ ] `FunctionCompiler` uses Salsa queries instead of batch map
- [ ] Incremental test: body change, same sig → no recompile
- [ ] Invalidation test: sig change → recompile
- [ ] Fixed-point test: mutual recursion converges
- [ ] `./test-all.sh` passes
- [ ] No performance regression on cold compile

**Exit Criteria:** Changing a function's body without changing its borrow signature does NOT trigger recompilation of callers, verified by test.
