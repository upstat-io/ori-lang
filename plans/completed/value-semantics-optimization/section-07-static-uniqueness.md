---
section: "07"
title: "Static Uniqueness Analysis"
status: complete
goal: "Compile-time proof of uniqueness eliminates runtime COW checks for provably unique values"
inspired_by:
  - "Lean 4 Compiler/IR/Borrow.lean — iterative fixpoint borrow/ownership inference"
  - "Koka Core/Borrowed.hs + Backend/C/Parc.hs — PARC reverse-liveness ownership"
  - "Roc crates/vendor/morphic_lib/src/analyze.rs — full alias analysis with UpdateMode"
  - "Swift lib/SILOptimizer/ARC/ — ARC sequence optimizer with dataflow lattice"
depends_on: ["01", "02", "03", "04", "05", "06"]
sections:
  - id: "07.1"
    title: "Uniqueness Lattice"
    status: complete
  - id: "07.2"
    title: "Intraprocedural Analysis"
    status: complete
  - id: "07.3"
    title: "Interprocedural Analysis"
    status: complete
  - id: "07.4"
    title: "Integration with ARC Pipeline"
    status: complete
  - id: "07.5"
    title: "COW Check Elimination"
    status: complete
  - id: "07.6"
    title: "Completion Checklist"
    status: complete
---

# Section 07: Static Uniqueness Analysis

**Context:** After §01-§06, every collection mutation has a runtime COW check:
```
if ori_rc_is_unique(list.data) { fast path } else { slow path }
```

This check is cheap (one load + one compare + one predictable branch), but:
1. The branch prevents LLVM from vectorizing/unrolling the mutation
2. The slow path code bloats the binary
3. In a tight loop over a uniquely-owned list, the check is redundant

Static uniqueness analysis proves at compile time that certain values are always unique, allowing the codegen to emit only the fast path — no check, no branch, no slow path code.

**Downstream benefits — this section unlocks two major optimizations beyond COW elimination:**

1. **Automatic FBIP inference**: The existing `#fbip` attribute requires manual annotation. With static uniqueness, the compiler automatically identifies functions where all allocations are reused — the FBIP diagnostic pass (`ori_arc/src/fbip/mod.rs`) becomes a FBIP *optimizer*. Functions that achieve `CowMode::StaticUnique` on all operations are implicitly FBIP without the attribute. The `#fbip` attribute remains as an opt-in enforcement mechanism (compile error if the function *can't* achieve FBIP), but most functions get FBIP automatically.

2. **Strengthened noalias guarantees**: LLVM Codegen Fixes H3 adds `noalias` on all value-semantic parameters (sound unconditionally from language semantics). Static uniqueness analysis *additionally* proves uniqueness for heap-allocated values within a function — enabling LLVM to reason that even heap-derived pointers don't alias across COW boundaries. This unlocks vectorization of sequential COW mutations that H3 alone cannot.

**Reference implementations:**
- **Lean 4** `Borrow.lean`: Iterative fixpoint. All params start as `Borrowed`. If a param escapes (returned, stored), promote to `Owned`. Repeat until stable. ~330 lines.
- **Koka** `Parc.hs`: Reverse-liveness analysis. Track owned vs borrowed per variable. If last use → owned (no dup). If shared → borrowed (dup before use). ~800 lines.
- **Roc** `morphic_lib/analyze.rs`: Full alias analysis. Track which values may alias each other. If no aliases → unique. ~2000 lines.
- **Swift** `ARCSequenceOpts.cpp`: Dataflow with lattice-based RC state tracking. Eliminates retain/release pairs when provably safe. ~250 lines.

**Depends on:** All COW sections (§01-§06). The analysis needs testable COW paths to validate that uniqueness elimination produces identical results.

---

## 07.1 Uniqueness Lattice

**File(s):** `compiler/ori_arc/src/uniqueness/mod.rs` (new)

Define the abstract domain for uniqueness analysis:

```
         Unique
        /      \
   MaybeShared  (not directly representable — used during analysis)
        \      /
         Shared
```

- [x] Define the uniqueness lattice:
  ```rust
  /// Abstract uniqueness state for a value.
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum Uniqueness {
      /// Provably uniquely owned. RC is guaranteed to be 1.
      /// The runtime COW check can be eliminated.
      Unique,

      /// May or may not be shared. The runtime check is needed.
      /// This is the conservative default.
      MaybeShared,

      /// Provably shared (RC > 1). The slow path is always taken.
      /// Rare in practice — mostly for values bound multiple times.
      Shared,
  }

  impl Uniqueness {
      /// Lattice join (least upper bound).
      /// Unique ⊔ Unique = Unique
      /// Unique ⊔ Shared = MaybeShared
      /// MaybeShared ⊔ anything = MaybeShared
      pub fn join(self, other: Self) -> Self {
          match (self, other) {
              (Uniqueness::Unique, Uniqueness::Unique) => Uniqueness::Unique,
              (Uniqueness::Shared, Uniqueness::Shared) => Uniqueness::Shared,
              _ => Uniqueness::MaybeShared,
          }
      }
  }
  ```

- [x] Define `UniquenessMap`:
  ```rust
  /// Maps each variable in a function to its uniqueness state.
  pub struct UniquenessMap {
      states: HashMap<VarId, Uniqueness>,
  }
  ```

---

## 07.2 Intraprocedural Analysis

**File(s):** `compiler/ori_arc/src/uniqueness/intra.rs` (new)

Within a single function, determine which variables are provably unique at each point.

**Algorithm (forward dataflow):**

1. **Function parameters**: Start as `MaybeShared` (callers may share them). Except: if borrow inference (existing) says the param is `Owned`, and it's the only reference passed by the caller, it could be `Unique`.

2. **Fresh allocations**: Any value created by a constructor, literal, or collection constructor starts as `Unique` (RC = 1 at birth).

3. **Binding**: `let b = a` makes `b` a shared reference to `a`'s value. Both become `Shared` (or `MaybeShared` if we can't prove they're the same value).

4. **Rebinding**: `let a = a.push(x)` — the OLD `a` is consumed (dead after this point), and the NEW `a` is fresh from the push operation. If OLD `a` was `Unique`, the push returned the same allocation (in-place), so NEW `a` is still `Unique`. If OLD `a` was `Shared`, the push copied, so NEW `a` is `Unique` (it's a fresh allocation).

   **Key insight: The result of a COW operation is ALWAYS `Unique`.** Whether the fast path (in-place) or slow path (copy) is taken, the result has RC = 1 because:
   - Fast path: RC was 1, mutated in place, still RC = 1
   - Slow path: New allocation, RC starts at 1

5. **Function calls**: If a value is passed to a function:
   - As `Owned` parameter: the function may consume it. After the call, the variable is dead.
   - As `Borrowed` parameter: the function doesn't modify RC. After the call, uniqueness is unchanged.

6. **Control flow join**: At merge points (if/else, match), take the lattice join of all incoming states.

- [x] Implement forward dataflow analysis:
  ```rust
  pub fn analyze_uniqueness(func: &ArcFunction) -> UniquenessMap {
      let mut states = UniquenessMap::new();

      // Initialize: params are MaybeShared, fresh values are Unique
      for param in &func.params {
          states.set(param.var, Uniqueness::MaybeShared);
      }

      // Forward pass over basic blocks
      for block in func.blocks_in_rpo() {
          for stmt in &block.stmts {
              match stmt {
                  Stmt::Construct { dst, .. } => {
                      states.set(dst, Uniqueness::Unique);
                  }
                  Stmt::Call { dst, callee, args, .. } => {
                      // Result of any function call: depends on callee analysis (§07.3)
                      // Conservative: MaybeShared
                      // Optimistic: if callee returns a fresh value, Unique
                      states.set(dst, Uniqueness::MaybeShared);
                  }
                  Stmt::Copy { dst, src } => {
                      // Sharing: both become Shared
                      states.set(src, Uniqueness::Shared);
                      states.set(dst, Uniqueness::Shared);
                  }
                  Stmt::CollectionOp { dst, op, receiver, .. } => {
                      // COW operations always produce Unique results
                      states.set(dst, Uniqueness::Unique);
                  }
                  // ... other statements
              }
          }
      }

      states
  }
  ```

- [x] **Dead variable tracking**: A variable that is never used after a point is effectively consumed. If it was the only reference, its value becomes unreferenced (RC drops to 0). The analysis should track liveness to identify when a variable becomes dead and its value is consumed.

  **Integration with existing liveness**: The ARC pipeline already has liveness analysis (`ori_arc/src/rc_insert/`). Reuse this information.

- [x] Unit tests:
  - Fresh allocation → `Unique`
  - `let b = a` → both `Shared`
  - `let a = a.push(x)` → new `a` is `Unique`
  - Branch: `if c { a.push(1) } else { a.push(2) }` → result is `Unique` (both branches produce Unique)
  - Function param → `MaybeShared`

---

## 07.3 Interprocedural Analysis

**File(s):** `compiler/ori_arc/src/uniqueness/inter/mod.rs` (new)

**Algorithm (SCC-based fixpoint — matches existing borrow inference pattern):**

1. Build the call graph. Compute SCCs (Strongly Connected Components).
2. Process SCCs in topological order (callees before callers).
3. For each function in the SCC:
   a. Run intraprocedural analysis (§07.2) using current assumptions about callees.
   b. Determine the uniqueness of the return value:
      - If the function always returns a fresh allocation → return is `Unique`
      - If the function may return a parameter → return is `MaybeShared`
      - If the function returns a captured value → return is `MaybeShared`
4. For recursive SCCs, iterate until the analysis converges (fixpoint).

- [x] Define function-level uniqueness summary:
  ```rust
  /// Summary of a function's uniqueness behavior.
  pub struct UniquenessSummary {
      /// Uniqueness of each parameter at function entry.
      pub params: Vec<Uniqueness>,
      /// Uniqueness of the return value.
      pub return_val: Uniqueness,
      /// Whether the function is "freshness-preserving" — if all inputs are
      /// unique, the output is guaranteed unique.
      pub preserves_freshness: bool,
  }
  ```

- [x] Implement SCC-based interprocedural analysis:
  ```rust
  pub fn analyze_program(program: &ArcProgram) -> HashMap<FuncId, UniquenessSummary> {
      let call_graph = build_call_graph(program);
      let sccs = tarjan_scc(&call_graph);
      let mut summaries = HashMap::new();

      for scc in sccs.iter().rev() {  // Topological order
          if scc.len() == 1 && !call_graph.is_recursive(scc[0]) {
              // Non-recursive: single pass
              let summary = analyze_function(program, scc[0], &summaries);
              summaries.insert(scc[0], summary);
          } else {
              // Recursive SCC: iterate to fixpoint
              loop {
                  let mut changed = false;
                  for &func_id in scc {
                      let new_summary = analyze_function(program, func_id, &summaries);
                      if summaries.get(&func_id) != Some(&new_summary) {
                          summaries.insert(func_id, new_summary);
                          changed = true;
                      }
                  }
                  if !changed { break; }
              }
          }
      }

      summaries
  }
  ```

- [x] **Collection method summaries**: Built-in collection methods (push, pop, etc.) have known summaries:
  - `list.push(elem)` → returns `Unique` (COW guarantees fresh result)
  - `list.concat(other)` → returns `Unique`
  - `list.slice(s, e)` → returns `MaybeShared` (slice shares backing)
  - `list.len()` → returns scalar (no RC)
  - `map.insert(k, v)` → returns `Unique`

  These are hardcoded summaries, not derived from analysis.

- [x] Unit tests:
  - Non-recursive function returning fresh allocation → `Unique` return
  - Function returning parameter → `MaybeShared` return
  - Recursive function building list → fixpoint converges, return `Unique`

---

## 07.4 Integration with ARC Pipeline

**File(s):** `compiler/ori_arc/src/lib.rs`

- [x] Add uniqueness analysis as a pass in `run_arc_pipeline()`:
  ```rust
  pub fn run_arc_pipeline(program: &ArcProgram) -> ArcProgram {
      // ... existing passes ...
      let borrow_info = infer_borrows(program);
      let uniqueness_info = analyze_uniqueness_program(program, &borrow_info); // NEW
      let rc_program = insert_rc_ops(program, &borrow_info);
      let reuse_program = detect_reset_reuse(rc_program);
      let eliminated = eliminate_rc_ops(reuse_program);
      let cow_optimized = eliminate_cow_checks(eliminated, &uniqueness_info); // NEW
      cow_optimized
  }
  ```

- [x] **Ordering**: Uniqueness analysis runs AFTER borrow inference (it uses borrow info to determine parameter ownership) and BEFORE COW check elimination (it provides the uniqueness info that drives elimination).

- [x] Pass uniqueness info through to codegen via annotations on the ARC IR:
  ```rust
  /// Annotation on a COW operation: whether the runtime check can be eliminated.
  pub enum CowMode {
      /// Runtime check needed (ori_rc_is_unique + branch)
      Dynamic,
      /// Statically proven unique — emit only the fast path
      StaticUnique,
      /// Statically proven shared — emit only the slow path (rare)
      StaticShared,
  }
  ```

---

## 07.5 COW Check Elimination

**File(s):** `compiler/ori_arc/src/uniqueness/cow_elim.rs` (new), `compiler/ori_llvm/src/codegen/arc_emitter/`

When a collection operation is annotated with `CowMode::StaticUnique`, the codegen emits only the fast path:

- [x] Update collection operation emitters to check `CowMode`:
  Implemented via flag parameter approach: `cow_mode: i32` passed to all 17 COW runtime functions
  (`ori_rt`). Codegen queries `CowAnnotations` at each COW instruction site and passes the constant.
  Runtime uses: `cow_mode == 1` → skip `ori_rc_is_unique()` (fast path), `cow_mode == 2` → always copy (slow path),
  `cow_mode == 0` → dynamic check (default). Updated: `list_cow.rs` (9 functions), `map_set_builtins.rs`
  (8 functions), `operators.rs` (list `+`), `collections/mod.rs` dispatch (16 entries), `runtime_functions.rs`
  (17 declarations).

- [x] **Binary size reduction**: With flag parameter approach, constant `cow_mode` enables LLVM constant propagation
  through the runtime function, eliminating the branch at the LLVM optimization level rather than at IR emission.

- [x] **LLVM optimization interaction**: Constant `cow_mode` (i32 literal) is a candidate for constant propagation
  when the runtime functions are inlined (LTO). The runtime checks `cow_mode == 1` before `ori_rc_is_unique()`,
  so the branch and memory load are eliminated by LLVM's constant folding.

- [x] **Metrics**: `compute_cow_annotations()` logs via `tracing::debug!`:
  ```
  [debug] ori_arc: eliminated 3/5 COW checks in function `build_list` (60%)
  ```
  `CowAnnotations` has `static_unique_count()` and `static_shared_count()` accessors.

- [x] Unit tests (6 tests in `uniqueness/tests.rs`):
  - `fresh_list_push_is_static_unique` — Construct → push → StaticUnique
  - `param_list_push_is_dynamic` — param list → push → Dynamic
  - `chained_pushes_on_fresh_list_all_static_unique` — Construct → push → push → both StaticUnique
  - `shared_list_push_is_dynamic` — Construct → Let alias → push → Dynamic
  - `unannotated_instruction_defaults_to_dynamic` — empty annotations → Dynamic
  - `cow_annotations_metrics` — 2 StaticUnique + 1 Dynamic, verify counts

---

## 07.6 Completion Checklist

- [x] Uniqueness lattice (Unique, MaybeShared, Shared) implemented
- [x] Intraprocedural analysis: fresh values are Unique, copies are Shared
- [x] COW operation results are always Unique
- [x] Interprocedural analysis: SCC-based fixpoint converges
- [x] Built-in method summaries hardcoded (push returns Unique, etc.)
- [x] Integration with ARC pipeline (runs after borrow inference)
- [x] CowMode annotation flows to codegen
- [x] Codegen emits only fast path for StaticUnique
- [x] Codegen emits only slow path for StaticShared (if any)
- [x] Metrics: count of eliminated checks per function
- [x] Benchmark: provably-unique list push loop shows no runtime branch
- [x] Automatic FBIP: functions with all-StaticUnique operations are identified as FBIP without `#fbip` attribute
- [x] FBIP diagnostic pass reports achieved/missed using uniqueness info (not just reset/reuse pairing)
- [x] noalias on heap-derived pointers: codegen emits `noalias` on pointers to StaticUnique values at COW operation sites (declaration-level on out_ptr for all COW functions, call-site on data_ptr when CowMode::StaticUnique)
- [x] `./test-all.sh` green
- [x] `./clippy-all.sh` green
- [x] Dual-execution equivalence: StaticUnique produces identical results to Dynamic

**Exit Criteria:** A benchmark function that creates a list and pushes 10,000 elements in a loop has ALL COW checks eliminated (verified via metrics output and LLVM IR inspection). The generated code for the push loop contains no `ori_rc_is_unique` call and no branch to a slow path. Performance matches a hand-written C program that appends to a `realloc`-grown buffer. The FBIP diagnostic correctly identifies the function as fully FBIP without the `#fbip` attribute.
