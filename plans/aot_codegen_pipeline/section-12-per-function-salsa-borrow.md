---
section: "12"
title: "Per-Function Salsa Borrow Inference"
status: not_started
goal: "Refactor whole-program fixed-point borrow inference to per-function Salsa queries for incremental recompilation"
inspired_by:
  - "Lean 4 src/Lean/Compiler/IR/Borrow.lean (whole-program fixed-point — what we're moving AWAY from)"
  - "rust-analyzer hir-ty/src/db.rs (#[salsa::cycle] for trait solving — SCC-based incrementality)"
  - "Swift SILOptimizer/ARC/RCIdentityAnalysis (per-function ARC analysis cached independently)"
depends_on: ["04", "08", "09", "10", "11"]
prerequisite: "ALL prior sections complete, ALL tests passing, pipeline fully stable"
sections:
  - id: "12.1"
    title: "Extract call graph from ARC IR"
    status: not_started
  - id: "12.2"
    title: "Compute SCCs (strongly connected components)"
    status: not_started
  - id: "12.3"
    title: "Define Salsa-tracked input types for ARC functions"
    status: not_started
  - id: "12.4"
    title: "Implement per-SCC borrow inference query"
    status: not_started
  - id: "12.5"
    title: "Single-function fast path (non-recursive)"
    status: not_started
  - id: "12.6"
    title: "Cycle recovery for mutual recursion"
    status: not_started
  - id: "12.7"
    title: "Signature stability check (early cutoff)"
    status: not_started
  - id: "12.8"
    title: "Migrate BorrowSigCache to per-function granularity"
    status: not_started
  - id: "12.9"
    title: "Wire SCC-based inference into compilation pipeline"
    status: not_started
  - id: "12.10"
    title: "Incremental invalidation strategy"
    status: not_started
  - id: "12.11"
    title: "Testing — correctness parity"
    status: not_started
  - id: "12.12"
    title: "Testing — incremental behavior"
    status: not_started
  - id: "12.13"
    title: "Performance benchmarking and regression gates"
    status: not_started
  - id: "12.14"
    title: "Watch-mode integration"
    status: not_started
  - id: "12.15"
    title: "Remove whole-program fallback"
    status: not_started
---

# Section 12: Per-Function Salsa Borrow Inference

**Status:** Not Started
**Goal:** Replace the whole-program fixed-point `infer_borrows()` with per-function Salsa-tracked queries, enabling incremental recompilation — when a function body changes but its borrow signature doesn't, no callers need reanalysis.

**Prerequisite:** ALL prior sections (01–11) complete, ALL tests passing, pipeline fully stable. This refactor touches the core analysis infrastructure and must land on solid ground.

**Context:** The current `infer_borrows()` (borrow/mod.rs) follows Lean 4's approach: iterate over ALL functions in a module until no parameter changes ownership (Borrowed→Owned monotonic convergence). This works correctly but defeats incrementality — changing ANY function body re-runs borrow inference for the ENTIRE module, even if most signatures are unchanged.

**Why this is hard:**

1. **Fixed-point iteration requires all functions simultaneously.** A calls B, B's sig affects A's params. Mutual recursion creates cycles that Salsa must handle.
2. **FxHashMap<Name, AnnotatedSig> can't be a Salsa return type** (no Eq/Hash for HashMap). But individual `AnnotatedSig` CAN — it derives `Clone, Eq, PartialEq, Hash, Debug`.
3. **The classifier trait object** (`&dyn ArcClassification`) isn't Salsa-compatible. Must be factored out or wrapped.
4. **Feature gating** — `ori_arc` types live behind `#[cfg(feature = "llvm")]`, complicating the Salsa trait surface.

**Architecture chosen: SCC-based decomposition.**

Instead of one monolithic fixed-point, decompose into strongly connected components (SCCs) of the call graph:

- **Single-function SCCs** (the vast majority — non-recursive functions): per-function Salsa query, maximum incrementality. No fixed-point needed.
- **Multi-function SCCs** (mutually recursive functions): run the existing fixed-point algorithm WITHIN the SCC. The SCC's combined result is one Salsa query.
- **Inter-SCC dependencies**: Salsa's automatic invalidation handles these. If SCC-B's output changes, SCC-A (which calls into B) is automatically re-queried.

This is the same strategy rust-analyzer uses for trait solving — group work into SCCs, handle cycles within groups, let Salsa manage inter-group incrementality.

**Key insight:** Salsa's cycle detection + recovery handles the fixed-point convergence that the manual `while changed` loop does today. For non-recursive functions, there are no cycles — Salsa computes once and memoizes. For recursive functions, Salsa detects the cycle, invokes recovery (conservative all-Owned), then re-evaluates. The monotonicity guarantee (Borrowed→Owned only) ensures convergence.

**Existing infrastructure to reuse:**

| Component | Location | Reuse |
|-----------|----------|-------|
| `FunctionDependencyGraph` | `ori_llvm/aot/incremental/function_deps/mod.rs` | Call graph extraction pattern, `callers_of()`, `collect_transitive_callers()` |
| `BorrowSigCache` | `oric/db/mod.rs` | Evolves from file-level to function-level keying |
| `AnnotatedSig` derives | `ori_arc/ownership/mod.rs` | Already `Clone, Eq, Hash, Debug` — Salsa-compatible |
| `ArcFunction` derives | `ori_arc/ir/mod.rs` | Already all Salsa-required derives |
| `compute_postorder` | `ori_arc/graph/mod.rs` | Basis for Tarjan's SCC (postorder DFS) |

---

## 12.1 Extract Call Graph from ARC IR

**File:** `compiler/ori_arc/src/graph/call_graph.rs` (NEW)

Build a function-level call graph from lowered ARC IR. This is the foundation for SCC computation.

**Why a new module?** The existing `graph/mod.rs` handles intra-function CFG (block predecessors, dominators). The call graph is inter-function — different level of abstraction, different data structure.

- [ ] Define `CallGraph` struct:
  ```rust
  /// Inter-function call graph extracted from ARC IR.
  ///
  /// Nodes are function names (`Name`). Edges are direct calls
  /// (Apply, Invoke, PartialApply with known callee).
  pub struct CallGraph {
      /// Function → set of functions it directly calls.
      callees: FxHashMap<Name, FxHashSet<Name>>,
      /// Reverse index: function → set of functions that call it.
      callers: FxHashMap<Name, FxHashSet<Name>>,
      /// All function names in the graph (including leaf functions with no calls).
      functions: FxHashSet<Name>,
  }
  ```

- [ ] Implement `CallGraph::build(functions: &[ArcFunction]) -> CallGraph`:
  Walk each function's blocks, extract callees from:
  - `ArcInstr::Apply { func, .. }` — direct call
  - `ArcInstr::PartialApply { func, .. }` — partial application (callee known)
  - `ArcTerminator::Invoke { func, .. }` — direct call (may unwind)
  - Skip `ApplyIndirect` — unknown callee, handled conservatively at inference time
  - Build both forward (`callees`) and reverse (`callers`) indexes in a single pass

- [ ] Implement accessor methods:
  - `callees_of(name: Name) -> &FxHashSet<Name>` — who does this function call?
  - `callers_of(name: Name) -> &FxHashSet<Name>` — who calls this function?
  - `functions() -> impl Iterator<Item = Name>` — all nodes
  - `is_recursive(name: Name) -> bool` — does it appear in its own callee set?
  - `is_leaf(name: Name) -> bool` — no callees (or only external callees not in graph)

- [ ] Handle external callees gracefully:
  External functions (`ori_*` runtime, C FFI) won't be in the function set. `callees_of` returns names that may not be graph nodes — this is intentional. SCC computation only considers functions IN the graph. External callees are invisible to borrow inference (they use the all-Owned conservative path from Section 04).

- [ ] Register module: add `pub mod call_graph;` to `graph/mod.rs`

**Tests** (`compiler/ori_arc/src/graph/call_graph/tests.rs`):
- `empty_graph`: No functions → empty graph
- `single_function_no_calls`: Leaf function → no callees, no callers
- `direct_call_chain`: A→B→C → callees/callers correct, no cycles
- `mutual_recursion`: A→B, B→A → both appear in each other's callee sets
- `self_recursion`: A→A → `is_recursive(A)` true
- `partial_apply_tracked`: PartialApply callee appears in callees
- `invoke_tracked`: Invoke callee appears in callees
- `indirect_call_ignored`: ApplyIndirect does NOT add to callees
- `external_callee_in_set`: External function name appears in callees but not in `functions()`
- `reverse_index_consistent`: For every edge A→B in callees, B→A exists in callers

---

## 12.2 Compute SCCs (Strongly Connected Components)

**File:** `compiler/ori_arc/src/graph/scc.rs` (NEW)

Implement Tarjan's algorithm to find SCCs in the call graph. Each SCC becomes one unit of borrow inference.

**Why Tarjan's?** It produces SCCs in reverse topological order (callees before callers), which is exactly the evaluation order we need — infer callees first, then callers can read their stable signatures. Tarjan's runs in O(V + E) — one DFS traversal.

- [ ] Implement `compute_sccs(graph: &CallGraph) -> Vec<Scc>`:
  ```rust
  /// A strongly connected component of the call graph.
  #[derive(Clone, Debug, PartialEq, Eq, Hash)]
  pub struct Scc {
      /// Function names in this SCC. Single-element for non-recursive functions.
      /// Multiple elements for mutually recursive function groups.
      pub members: Vec<Name>,
  }

  impl Scc {
      /// True if this SCC contains mutual recursion (2+ members)
      /// or self-recursion (1 member that calls itself).
      pub fn is_recursive(&self, graph: &CallGraph) -> bool {
          self.members.len() > 1
              || (self.members.len() == 1 && graph.is_recursive(self.members[0]))
      }
  }
  ```

- [ ] Implement Tarjan's SCC algorithm:
  Standard Tarjan's with `index`, `lowlink`, `on_stack` arrays. Reference: CLRS Chapter 22.5 or Lean 4's `src/Lean/Compiler/LCNF/ToLCNF.lean` for a functional variant.

  ```rust
  struct TarjanState {
      index_counter: u32,
      stack: Vec<Name>,
      on_stack: FxHashSet<Name>,
      indices: FxHashMap<Name, u32>,
      lowlinks: FxHashMap<Name, u32>,
      sccs: Vec<Scc>,
  }
  ```

  The output `Vec<Scc>` is in **reverse topological order** — leaf SCCs (no outgoing cross-SCC edges) first, root SCCs last. This is the natural output of Tarjan's algorithm.

- [ ] Add `topological_order(sccs: &[Scc]) -> Vec<&Scc>`:
  Reverse the Tarjan output to get **forward topological order** (callees before callers). This is the order in which we evaluate borrow inference queries — when we process SCC-A, all SCCs that A calls into have already been computed.

- [ ] Handle edge cases:
  - Functions not in the graph (external) → not included in any SCC
  - Self-recursive function with no other calls → SCC of size 1, `is_recursive = true`
  - Disconnected functions (no calls to/from any other function) → SCC of size 1 each

- [ ] Register module: add `pub mod scc;` to `graph/mod.rs`

**Tests** (`compiler/ori_arc/src/graph/scc/tests.rs`):
- `no_functions`: Empty graph → no SCCs
- `single_leaf`: One function, no calls → one SCC of size 1
- `linear_chain`: A→B→C → three SCCs of size 1, topological order [C, B, A]
- `simple_cycle`: A→B, B→A → one SCC of size 2
- `diamond`: A→B, A→C, B→D, C→D → four SCCs of size 1, D before B/C before A
- `self_recursive`: A→A → one SCC of size 1, `is_recursive = true`
- `mixed_recursive_and_linear`: A→B→C, B→A, D→C → SCC({A,B}), SCC({C}), SCC({D})
- `topological_order_callees_first`: Verify callees appear before callers
- `all_functions_covered`: Every function in the graph appears in exactly one SCC

---

## 12.3 Define Salsa-Tracked Input Types for ARC Functions

**File:** `compiler/oric/src/query/arc_queries.rs` (NEW)

Create Salsa-compatible wrappers for the ARC IR types that feed into borrow inference queries.

**Design constraint:** `ArcFunction` already derives all Salsa-required traits (`Clone, Eq, Hash, Debug`), but it's defined in `ori_arc` (which doesn't depend on Salsa). We need Salsa-tracked types in `oric` that wrap or reference ARC IR.

**Approach:** Use Salsa `#[salsa::input]` for the lowered ARC IR (set once per compilation), and `#[salsa::tracked]` for computed results.

- [ ] Define `ArcModuleInput` — Salsa input holding all lowered functions for a file:
  ```rust
  /// Salsa input: lowered ARC IR for one source file.
  ///
  /// Set once during the lowering phase. Salsa tracks whether the
  /// content changes between compilations — if it doesn't, all
  /// dependent queries are skipped.
  #[salsa::input]
  pub struct ArcModuleInput {
      /// Source file path (for cache keying and diagnostics).
      #[id]
      pub path: PathBuf,
      /// Lowered ARC functions, keyed by function name.
      /// Using Vec<(Name, ArcFunction)> instead of FxHashMap because
      /// Vec satisfies Salsa's Eq + Hash requirements (element-wise comparison).
      pub functions: Vec<(Name, ArcFunction)>,
      /// Pre-computed call graph for this module.
      pub call_graph: CallGraph,
      /// Pre-computed SCCs in topological order (callees first).
      pub sccs: Vec<Scc>,
  }
  ```

  **Why Vec<(Name, ArcFunction)> instead of FxHashMap?** Salsa input types must be `Eq + Hash`. `Vec` gets these from its elements (both `Name` and `ArcFunction` already derive them). `FxHashMap` does not implement `Eq` or `Hash`. The vec must be sorted by `Name` for deterministic comparison.

  **Note:** `CallGraph` and `Scc` must also derive `Clone, Eq, PartialEq, Hash, Debug` to be stored in a Salsa input. This is straightforward — they're small data structures with hashable contents.

- [ ] Define `BorrowSigResult` — Salsa-compatible per-SCC output:
  ```rust
  /// Per-SCC borrow inference result. Stored as a sorted Vec for
  /// deterministic Salsa comparison (enables early cutoff).
  #[derive(Clone, Debug, PartialEq, Eq, Hash)]
  pub struct BorrowSigResult {
      /// Annotated signatures, sorted by Name for deterministic Eq/Hash.
      pub sigs: Vec<(Name, AnnotatedSig)>,
  }
  ```

  **Why sorted Vec?** Salsa uses `Eq` to detect whether a query result changed (early cutoff). If the result is a `Vec<(Name, AnnotatedSig)>` in consistent sorted order, element-wise equality comparison is deterministic and cheap. This is the standard Salsa pattern for map-like results.

- [ ] Add helper methods:
  - `BorrowSigResult::get(name: Name) -> Option<&AnnotatedSig>` — binary search on sorted vec
  - `BorrowSigResult::into_map(self) -> FxHashMap<Name, AnnotatedSig>` — convert for downstream consumers
  - `BorrowSigResult::from_map(map: FxHashMap<Name, AnnotatedSig>) -> Self` — convert from inference output (sorts by Name)

- [ ] Verify `CallGraph` and `Scc` derive requirements:
  - `CallGraph` contains `FxHashMap` and `FxHashSet` — these do NOT derive `Eq`/`Hash`
  - **Resolution:** Either (a) store call graph edges as sorted `Vec` inside the Salsa input, or (b) use a content hash for the call graph rather than structural equality
  - Recommended: (a) — define `SalsaCallGraph` with `Vec<(Name, Vec<Name>)>` (sorted) for the Salsa input, keep `CallGraph` with `FxHashMap` for runtime use, add conversion methods

- [ ] Register module: add `pub mod arc_queries;` to `query/mod.rs`

**Tests** (`compiler/oric/src/query/arc_queries/tests.rs`):
- `borrow_sig_result_sorted`: `from_map` produces sorted output
- `borrow_sig_result_get`: Binary search finds correct entry
- `borrow_sig_result_eq`: Same sigs in different insertion order → equal after `from_map`
- `arc_module_input_roundtrip`: Create input, read back functions

---

## 12.4 Implement Per-SCC Borrow Inference Query

**File:** `compiler/oric/src/query/arc_queries.rs`

The core Salsa query: given an SCC (one or more mutually recursive functions), compute their borrow signatures.

- [ ] Define the per-SCC tracked query:
  ```rust
  /// Compute borrow signatures for one SCC of the call graph.
  ///
  /// For single-function SCCs (non-recursive): runs single-pass analysis.
  /// For multi-function SCCs (mutually recursive): runs fixed-point iteration
  /// within the SCC, consulting callee signatures from other SCCs via Salsa.
  ///
  /// Salsa tracks dependencies automatically: if a callee SCC's result changes,
  /// this query is invalidated and re-executed.
  #[salsa::tracked]
  pub fn infer_borrow_scc(
      db: &dyn crate::Db,
      module: ArcModuleInput,
      scc_index: u32,
  ) -> BorrowSigResult {
      // 1. Extract the SCC's member functions from the module
      // 2. Collect callee signatures from other (already-computed) SCCs
      // 3. Run borrow inference (single-pass or fixed-point)
      // 4. Return sorted BorrowSigResult
  }
  ```

  **Why `scc_index` instead of `Scc`?** The SCC is derived from the module's call graph (stored in `ArcModuleInput`). Using the index avoids duplicating the SCC data in the query key. The query reads `module.sccs(db)[scc_index]` to get the SCC.

- [ ] Implement callee signature collection:
  ```rust
  /// Collect borrow signatures for all callees OUTSIDE this SCC.
  ///
  /// For each function in the SCC, find its callees. For callees in OTHER SCCs,
  /// query their SCC's borrow result (triggering Salsa dependency tracking).
  /// For callees in THIS SCC, skip (handled by internal fixed-point).
  /// For external callees (not in any SCC), skip (all-Owned conservative path).
  fn collect_external_callee_sigs(
      db: &dyn crate::Db,
      module: ArcModuleInput,
      scc_members: &FxHashSet<Name>,
      scc_index_map: &FxHashMap<Name, u32>,
  ) -> FxHashMap<Name, AnnotatedSig> {
      let mut sigs = FxHashMap::default();
      for &member in scc_members {
          for &callee in module.call_graph(db).callees_of(member) {
              if scc_members.contains(&callee) {
                  continue; // Internal to SCC — handled by fixed-point
              }
              if let Some(&callee_scc_idx) = scc_index_map.get(&callee) {
                  // Query the callee's SCC (Salsa dependency created here!)
                  let callee_result = infer_borrow_scc(db, module, callee_scc_idx);
                  if let Some(sig) = callee_result.get(callee) {
                      sigs.insert(callee, sig.clone());
                  }
              }
              // External callees: not in any SCC, all-Owned (no entry needed)
          }
      }
      sigs
  }
  ```

  **This is where Salsa's magic happens.** The call to `infer_borrow_scc(db, module, callee_scc_idx)` creates a Salsa dependency edge. If the callee SCC's result changes in a future compilation, Salsa automatically invalidates this SCC's query.

- [ ] Implement the inference dispatch:
  ```rust
  // Inside infer_borrow_scc:
  let scc = &module.sccs(db)[scc_index as usize];
  let scc_members: FxHashSet<Name> = scc.members.iter().copied().collect();

  // Collect external callee sigs (triggers Salsa dependencies)
  let mut sigs = collect_external_callee_sigs(db, module, &scc_members, &scc_index_map);

  // Extract this SCC's ArcFunctions
  let scc_functions: Vec<&ArcFunction> = /* from module.functions(db) */;

  if scc.is_recursive(&call_graph) {
      // Fixed-point within SCC (existing algorithm, scoped to SCC members)
      infer_borrow_fixed_point(&scc_functions, &mut sigs, classifier)
  } else {
      // Single-pass: non-recursive function, callees already resolved
      infer_borrow_single(&scc_functions[0], &sigs, classifier)
  }
  ```

- [ ] Add tracing instrumentation:
  - `debug!` on SCC entry (members, is_recursive)
  - `debug!` on callee sig collection (how many external deps)
  - `trace!` on fixed-point iterations (iteration count, which params changed)
  - `debug!` on result (final signature summary)

---

## 12.5 Single-Function Fast Path (Non-Recursive)

**File:** `compiler/ori_arc/src/borrow/mod.rs`

Extract the single-function inference logic from the existing `update_ownership` into a standalone public function. This is the fast path for the vast majority of functions.

- [ ] Define `infer_borrow_single`:
  ```rust
  /// Infer borrow annotations for a single non-recursive function.
  ///
  /// Unlike [`infer_borrows`] which iterates all functions to a fixed point,
  /// this function runs ONE pass over a single function. Callee signatures
  /// must be pre-resolved and passed in `external_sigs`.
  ///
  /// This is correct for non-recursive functions because:
  /// 1. All callees' sigs are already finalized (topological order)
  /// 2. No self-calls means no need for iteration
  /// 3. One pass over instructions is sufficient to determine all param ownership
  pub fn infer_borrow_single(
      func: &ArcFunction,
      external_sigs: &FxHashMap<Name, AnnotatedSig>,
      classifier: &dyn ArcClassification,
  ) -> AnnotatedSig {
      // Initialize this function's params as all-Borrowed (optimistic)
      let mut sig = initialize_single_borrowed(func, classifier);
      // Run one pass of update_ownership logic
      update_single(func, &mut sig, external_sigs);
      sig
  }
  ```

- [ ] Extract `initialize_single_borrowed` from `initialize_all_borrowed`:
  Same logic but for one function, returning a single `AnnotatedSig` instead of a map.

- [ ] Extract `update_single` from `update_ownership`:
  Same instruction-scanning logic but:
  - Reads callee sigs from `external_sigs` (pre-collected, immutable)
  - No need for the clone-to-avoid-borrow dance (no shared mutable map)
  - No `changed` flag needed (single pass, no iteration)
  - Returns the finalized `AnnotatedSig`

- [ ] Keep the existing `infer_borrows` as-is for now:
  It's still used by the SCC fixed-point path (12.6) and as the fallback until migration is complete (12.15). Mark it with a doc comment noting it will be deprecated.

**Tests** (`compiler/ori_arc/src/borrow/tests.rs`):
- `single_leaf_function`: No calls → all ref-typed params stay Borrowed
- `single_with_construct`: Param stored in struct → promoted to Owned
- `single_with_return`: Param returned → promoted to Owned
- `single_with_callee_owned`: Param passed to callee's Owned position → promoted to Owned
- `single_with_callee_borrowed`: Param passed to callee's Borrowed position → stays Borrowed
- `single_alias_chain`: `v1 = v0; Construct(v1)` where v0 is param → v0 promoted to Owned
- `single_scalar_param`: Scalar param → Owned regardless (no RC)
- `single_matches_whole_program`: For non-recursive functions, verify `infer_borrow_single` produces the same result as `infer_borrows` (property test)

---

## 12.6 Cycle Recovery for Mutual Recursion

**File:** `compiler/ori_arc/src/borrow/mod.rs` + `compiler/oric/src/query/arc_queries.rs`

Handle mutually recursive functions (multi-member SCCs) using the existing fixed-point algorithm, scoped to just the SCC members.

- [ ] Define `infer_borrow_fixed_point` (scoped to SCC):
  ```rust
  /// Infer borrow annotations for a set of mutually recursive functions.
  ///
  /// Runs the same fixed-point algorithm as [`infer_borrows`], but scoped to
  /// only the functions in this SCC. External callee signatures are pre-resolved
  /// and immutable — only this SCC's signatures are iterated.
  ///
  /// # Arguments
  ///
  /// * `scc_functions` — the mutually recursive functions in this SCC
  /// * `external_sigs` — pre-resolved signatures of callees outside this SCC
  /// * `classifier` — type classifier for scalar detection
  pub fn infer_borrow_fixed_point(
      scc_functions: &[&ArcFunction],
      external_sigs: &FxHashMap<Name, AnnotatedSig>,
      classifier: &dyn ArcClassification,
  ) -> FxHashMap<Name, AnnotatedSig> {
      // 1. Initialize SCC members as all-Borrowed
      let mut sigs = initialize_scc_borrowed(scc_functions, classifier);

      // 2. Merge external sigs (read-only, never mutated)
      //    Create a combined view for callee lookups
      let combined = CombinedSigs::new(&mut sigs, external_sigs);

      // 3. Fixed-point iteration (scoped to SCC members only)
      let mut changed = true;
      let mut iteration = 0u32;
      while changed {
          changed = false;
          for func in scc_functions {
              if update_ownership_scoped(func, &mut combined) {
                  changed = true;
              }
          }
          iteration += 1;
          tracing::trace!(iteration, "borrow SCC fixed-point iteration");
          debug_assert!(
              iteration <= scc_functions.iter().map(|f| f.params.len()).sum::<usize>() as u32 + 1,
              "fixed-point should converge in at most N_params iterations"
          );
      }
      tracing::debug!(iterations = iteration, "borrow SCC converged");

      sigs
  }
  ```

- [ ] Implement `CombinedSigs` view:
  ```rust
  /// Read-through view that checks SCC-local sigs first, then external sigs.
  /// SCC-local sigs are mutable (updated during iteration).
  /// External sigs are immutable (pre-resolved from other SCCs).
  struct CombinedSigs<'a> {
      local: &'a mut FxHashMap<Name, AnnotatedSig>,
      external: &'a FxHashMap<Name, AnnotatedSig>,
  }

  impl CombinedSigs<'_> {
      fn get(&self, name: &Name) -> Option<&AnnotatedSig> {
          self.local.get(name).or_else(|| self.external.get(name))
      }
  }
  ```

- [ ] Implement `update_ownership_scoped`:
  Identical to existing `update_ownership` but uses `CombinedSigs` for callee lookups instead of the single shared `sigs` map. This ensures external callee sigs are immutable while SCC-local sigs are mutable.

- [ ] Add Salsa cycle recovery (for the Salsa query path):
  ```rust
  /// Cycle recovery: if Salsa detects a query cycle (shouldn't happen with
  /// proper topological evaluation, but defensive), return conservative
  /// all-Owned signatures for all functions in the SCC.
  ///
  /// This is correct but suboptimal — the SCC-based approach should prevent
  /// cycles from ever reaching here. If this fires, it indicates a bug in
  /// topological ordering.
  fn borrow_scc_cycle_recovery(
      db: &dyn crate::Db,
      _cycle: &salsa::Cycle,
      module: ArcModuleInput,
      scc_index: u32,
  ) -> BorrowSigResult {
      tracing::warn!(scc_index, "borrow inference cycle recovery triggered — this indicates a topological ordering bug");
      // Return all-Owned for every function in the SCC
      let scc = &module.sccs(db)[scc_index as usize];
      let functions = module.functions(db);
      let sigs: Vec<(Name, AnnotatedSig)> = scc.members.iter()
          .filter_map(|&name| {
              functions.iter()
                  .find(|(n, _)| *n == name)
                  .map(|(n, func)| (*n, all_owned_sig(func)))
          })
          .collect();
      BorrowSigResult { sigs }
  }
  ```

  Wire this into the Salsa query:
  ```rust
  #[salsa::tracked(cycle_fn = borrow_scc_cycle_recovery)]
  pub fn infer_borrow_scc(...) -> BorrowSigResult { ... }
  ```

**Tests** (`compiler/ori_arc/src/borrow/tests.rs`):
- `fixed_point_mutual_recursion`: A calls B, B calls A → both converge correctly
- `fixed_point_with_external_callee`: SCC members call external function with Owned param → correct promotion
- `fixed_point_matches_whole_program`: For mutually recursive functions, verify `infer_borrow_fixed_point` produces the same result as `infer_borrows` (property test — this is the critical correctness check)
- `combined_sigs_local_priority`: Local sig takes precedence over external sig for same name
- `convergence_bound`: Fixed-point converges in ≤ N_params iterations
- `cycle_recovery_all_owned`: Recovery function returns all-Owned for all SCC members

---

## 12.7 Signature Stability Check (Early Cutoff)

**File:** `compiler/oric/src/query/arc_queries.rs`

Salsa's early cutoff means: if a query re-executes but produces the SAME result as before, dependents are NOT invalidated. This is the key incrementality win — a function body can change freely as long as its borrow signature stays the same.

- [ ] Verify `AnnotatedSig` equality is correct for early cutoff:
  `AnnotatedSig` derives `Eq`. Two sigs are equal if they have the same params (same names, types, ownership) and same return type. This is exactly what we want — if a function's body changes but its borrow annotation stays the same, the `Eq` check returns `true` and Salsa skips dependent re-evaluation.

  **Verify:** `Name` uses interned equality (fast). `Idx` is a `u32` wrapper (fast). `Ownership` is a 2-variant enum (fast). `AnnotatedParam` is 3 fields (fast). `AnnotatedSig` is a `Vec<AnnotatedParam>` + `Idx` (proportional to param count, typically ≤ 10). This is cheap enough for Salsa's per-query comparison.

- [ ] Verify `BorrowSigResult` equality is deterministic:
  Since `sigs` is sorted by `Name`, element-wise `Vec` comparison is deterministic. Two `BorrowSigResult` values are equal iff they contain the same set of function signatures.

- [ ] Add a tracing event for early cutoff observation:
  ```rust
  // In the query body, before returning:
  tracing::debug!(
      scc_index,
      member_count = scc.members.len(),
      "borrow SCC inference complete"
  );
  // Salsa's early cutoff happens AFTER the query returns — we can't observe it
  // directly. But we CAN observe cache hits vs re-executions via Salsa's built-in
  // event system or by comparing execution counts in tests.
  ```

- [ ] Document the early cutoff contract:
  Add a module-level doc comment explaining WHY this matters: "Function `foo` changes from `x + 1` to `x + 2`. Both versions have the same borrow signature (param `x` is Borrowed in both). Salsa re-runs `infer_borrow_scc` for foo's SCC, gets the same `BorrowSigResult`, and skips re-evaluating all of foo's callers."

**Tests:**
- `early_cutoff_body_change_same_sig`: Change function body without affecting borrow sig → verify dependent SCC NOT re-queried (use Salsa's event log or execution counter)
- `no_cutoff_sig_change`: Change function body so borrow sig changes → verify dependent SCC IS re-queried
- `deterministic_result_ordering`: Same functions in different processing order → same `BorrowSigResult`

---

## 12.8 Migrate BorrowSigCache to Per-Function Granularity

**File:** `compiler/oric/src/db/mod.rs`

The current `BorrowSigCache` stores `FxHashMap<Name, AnnotatedSig>` per file. With per-function Salsa queries, the cache granularity shifts — Salsa handles per-SCC memoization automatically. The side-cache's role changes from "avoid re-running the whole pipeline" to "provide a fast collection point for downstream consumers."

- [ ] Evaluate whether `BorrowSigCache` is still needed:
  With Salsa queries, borrow inference results are automatically memoized. Downstream consumers (FunctionCompiler, ArcEmitter) still need a `FxHashMap<Name, AnnotatedSig>` — but this can be assembled by collecting per-SCC query results. The question is whether this collection is cheap enough to skip caching, or whether we still want a file-level aggregate cache.

  **Decision criteria:**
  - If assembly cost is O(number of SCCs) with small constant → remove `BorrowSigCache`, assemble on demand
  - If assembly cost is significant (many SCCs, deep call graph) → keep `BorrowSigCache` as an aggregate cache populated lazily from Salsa queries

- [ ] If keeping: adapt `BorrowSigCache` to be populated from Salsa queries:
  ```rust
  /// Assemble the full borrow sig map for a file by collecting all SCC results.
  /// Caches the assembled map for subsequent accesses within the same session.
  pub fn borrow_sigs_for_file(
      db: &CompilerDb,
      module: ArcModuleInput,
  ) -> Arc<FxHashMap<Name, AnnotatedSig>> {
      if let Some(cached) = db.borrow_sig_cache().get(&module.path(db)) {
          return cached;
      }
      let mut sigs = FxHashMap::default();
      for (i, _scc) in module.sccs(db).iter().enumerate() {
          let result = infer_borrow_scc(db, module, i as u32);
          for (name, sig) in result.sigs {
              sigs.insert(name, sig);
          }
      }
      let sigs = Arc::new(sigs);
      db.borrow_sig_cache().store(module.path(db).clone(), Arc::clone(&sigs));
      sigs
  }
  ```

- [ ] If removing: replace all `borrow_sig_cache()` call sites with direct Salsa query collection:
  ```rust
  fn collect_borrow_sigs(
      db: &dyn crate::Db,
      module: ArcModuleInput,
  ) -> FxHashMap<Name, AnnotatedSig> {
      let mut sigs = FxHashMap::default();
      for (i, _scc) in module.sccs(db).iter().enumerate() {
          let result = infer_borrow_scc(db, module, i as u32);
          for (name, sig) in result.sigs {
              sigs.insert(name, sig);
          }
      }
      sigs
  }
  ```

- [ ] Update invalidation logic:
  - With Salsa: invalidation happens automatically when `ArcModuleInput` changes
  - `BorrowSigCache::invalidate()` (if kept) should be called when `ArcModuleInput` is re-set
  - With Salsa, explicitly clearing the side-cache when the input changes ensures consistency

**Tests:**
- `assembled_sigs_match_whole_program`: Collected per-SCC sigs match whole-program `infer_borrows` output
- `cache_invalidation_on_input_change`: Changing `ArcModuleInput` clears or invalidates the aggregate cache

---

## 12.9 Wire SCC-Based Inference into Compilation Pipeline

**File:** `compiler/oric/src/commands/compile_common.rs`

Replace the direct `infer_borrows()` call with the SCC-based Salsa query path.

- [ ] Modify `run_borrow_inference()` (lines 127-185):
  Current flow:
  ```
  lower functions → infer_borrows(all_functions) → return sigs
  ```
  New flow:
  ```
  lower functions → build CallGraph → compute SCCs → create ArcModuleInput →
  for each SCC in topological order: infer_borrow_scc(db, module, scc_index) →
  collect into FxHashMap → return sigs
  ```

- [ ] Modify `run_arc_pipeline_cached()` (lines 197-252):
  Update to use `ArcModuleInput` as the cache key. The lowered functions and call graph are stored in the Salsa input, eliminating the separate ARC IR cache layer for borrow inference.

- [ ] Modify `compile_to_llvm()` and `compile_to_llvm_with_imports()`:
  Replace `BorrowSigCache` lookups with Salsa query collection (or keep the aggregate cache if decision in 12.8 favors it).

- [ ] Handle the `ArcClassification` problem:
  `infer_borrows` takes `&dyn ArcClassification` (a trait object). Salsa queries can't take trait objects. Options:
  - **(a) Pre-compute scalar classification**: Before creating `ArcModuleInput`, pre-compute `is_scalar` for every variable type used in the module's functions. Store as `FxHashMap<Idx, bool>` in `ArcModuleInput`. The query reads from this map instead of calling the trait.
  - **(b) Thread the classifier through the database**: Add a method to `CompilerDb` that creates the classifier. The Salsa query accesses it via `db.downcast_ref::<CompilerDb>()`.
  - **(c) Use a Salsa input for the classifier**: Create a Salsa input that wraps the classification data.
  - **Recommended: (a)** — simplest, most Salsa-friendly, no trait objects in queries.

  ```rust
  #[salsa::input]
  pub struct ArcModuleInput {
      #[id]
      pub path: PathBuf,
      pub functions: Vec<(Name, ArcFunction)>,
      pub call_graph: SalsaCallGraph,
      pub sccs: Vec<Scc>,
      /// Pre-computed scalar classification for all types used in these functions.
      /// Eliminates the need for `&dyn ArcClassification` in Salsa queries.
      pub scalar_types: Vec<(Idx, bool)>,  // sorted for Eq/Hash determinism
  }
  ```

- [ ] Wire classifier pre-computation:
  ```rust
  fn precompute_scalar_types(
      functions: &[ArcFunction],
      classifier: &dyn ArcClassification,
  ) -> Vec<(Idx, bool)> {
      let mut types: FxHashSet<Idx> = FxHashSet::default();
      for func in functions {
          for param in &func.params {
              types.insert(param.ty);
          }
          for ty in &func.var_types {
              types.insert(*ty);
          }
      }
      let mut result: Vec<(Idx, bool)> = types.into_iter()
          .map(|idx| (idx, classifier.is_scalar(idx)))
          .collect();
      result.sort_by_key(|(idx, _)| idx.raw());
      result
  }
  ```

- [ ] Add a feature flag for gradual rollout:
  ```rust
  /// Use SCC-based per-function Salsa queries for borrow inference.
  /// When false, falls back to whole-program `infer_borrows()`.
  const USE_SALSA_BORROW_INFERENCE: bool = cfg!(feature = "salsa-borrow");
  ```
  Add `salsa-borrow` feature to `oric/Cargo.toml`. Initially disabled. Enable after 12.11 and 12.12 pass.

**Tests:**
- `pipeline_produces_same_sigs`: With and without `salsa-borrow` feature, same input → same borrow sigs
- `pipeline_e2e_simple`: Single file, no recursion → compiles correctly with Salsa path
- `pipeline_e2e_mutual_recursion`: File with mutual recursion → compiles correctly
- `pipeline_e2e_multi_file`: Multi-file compilation → correct cross-file borrow inference

---

## 12.10 Incremental Invalidation Strategy

**File:** `compiler/oric/src/db/mod.rs`, `compiler/oric/src/query/arc_queries.rs`

Define how changes propagate through the Salsa query graph.

- [ ] Document the invalidation cascade:
  ```
  Source file changes
    → SourceFile input updated
      → tokens() re-computes (early cutoff if tokenization unchanged)
        → parsed() re-computes (early cutoff if AST unchanged)
          → typed() re-computes (early cutoff if typed IR unchanged)
            → ArcModuleInput re-set (triggers if typed output changed)
              → infer_borrow_scc() re-computes per affected SCC
                → early cutoff: if borrow sig unchanged, callers NOT invalidated
                  → run_arc_pipeline() only re-runs for functions with changed sigs
  ```

- [ ] Implement `ArcModuleInput` update logic:
  When `typed()` produces new output, re-lower to ARC IR, rebuild call graph, recompute SCCs, and update the `ArcModuleInput`. Salsa compares the new input to the old — if functions are unchanged, no downstream re-evaluation.

  **Granularity optimization:** Instead of one `ArcModuleInput` per file, consider one `ArcFunctionInput` per function:
  ```rust
  #[salsa::tracked]
  struct ArcFunctionInput {
      #[id]
      name: Name,
      body: ArcFunction,
      scalar_types: Vec<(Idx, bool)>,
  }
  ```
  This gives maximum Salsa granularity — changing one function only invalidates the SCCs it participates in. But it requires tracking function-to-SCC membership separately.

  **Decision:** Start with per-module `ArcModuleInput` (simpler). Migrate to per-function inputs in a follow-up if profiling shows the module-level granularity is too coarse.

- [ ] Handle SCC membership changes:
  If a code change adds/removes a call edge, the SCC structure itself may change (two separate SCCs merge, or one SCC splits). Since SCCs are stored in `ArcModuleInput`, any change to the call graph re-sets the entire module input. All SCCs in the module are re-evaluated, but most will produce the same result (early cutoff).

  **Optimization opportunity (future):** Diff the old and new SCC structures, only re-set affected SCCs. This is complex and probably not worth it initially.

- [ ] Cross-file borrow inference:
  Currently borrow inference is per-file. With Salsa, cross-file borrow inference becomes possible: if file A imports and calls functions from file B, A's borrow inference query could depend on B's function signatures.

  **Approach:** Initially, cross-file calls are treated as external (all-Owned conservative). This preserves the current behavior. A follow-up section could add cross-file Salsa dependencies.

**Tests:**
- `invalidation_only_affected_sccs`: Change one function → only its SCC re-evaluates
- `scc_restructure_handled`: Add a call edge that merges two SCCs → correct re-evaluation
- `cross_file_conservative`: Cross-file callee → treated as external (all-Owned)

---

## 12.11 Testing — Correctness Parity

**Files:** `compiler/ori_arc/src/borrow/tests.rs`, `compiler/oric/tests/phases/codegen/`

Verify that the SCC-based approach produces IDENTICAL results to the whole-program approach for every test case.

- [ ] Property test: whole-program vs SCC-based equivalence:
  ```rust
  /// For any set of ARC functions, verify that:
  /// 1. infer_borrows(all_functions) produces a signature map
  /// 2. SCC-based inference (build graph → compute SCCs → per-SCC inference) produces a signature map
  /// 3. Both maps are identical
  #[test]
  fn whole_program_vs_scc_equivalence() {
      for test_case in ALL_BORROW_TEST_CASES {
          let whole_program = infer_borrows(&test_case.functions, &test_case.classifier);
          let scc_based = infer_borrows_scc_based(&test_case.functions, &test_case.classifier);
          assert_eq!(whole_program, scc_based, "mismatch for test case: {}", test_case.name);
      }
  }
  ```

- [ ] Enumerate test cases covering all borrow patterns:
  - **Linear call chain:** A→B→C, all params Borrowed
  - **Linear with ownership transfer:** A→B, B stores param → B's param Owned, A's param stays Borrowed
  - **Mutual recursion:** A↔B, both pass params to each other's Owned positions
  - **Self-recursion:** A→A, param passed to own Owned position
  - **Diamond dependency:** A→B, A→C, B→D, C→D
  - **Construct in leaf:** Leaf function constructs struct from param → param Owned
  - **Return param:** Function returns its own param → param Owned
  - **Alias chain:** v1=v0; Construct(v1) → v0 promoted
  - **Project bidirectional:** Project result is Owned → source promoted
  - **Tail call preservation:** A tail-calls B with Borrowed param in B's Owned position → A's param promoted
  - **External callee:** Function calls unknown callee → all args Owned
  - **Mixed recursive and non-recursive:** SCC with 2 recursive functions + 3 non-recursive callees

- [ ] Run FULL test suite with SCC-based path:
  ```bash
  # With salsa-borrow feature enabled:
  cargo test --features salsa-borrow
  ./llvm-test.sh  # if LLVM feature supports it
  cargo st        # all spec tests
  ./test-all.sh   # everything
  ```

- [ ] Verify zero regressions against the non-Salsa path:
  Every test that passes with whole-program inference must pass with SCC-based inference. No exceptions.

---

## 12.12 Testing — Incremental Behavior

**Files:** `compiler/oric/tests/incremental/borrow_inference.rs` (NEW)

Verify that the Salsa integration actually provides incremental benefits.

- [ ] Test: unchanged function → no re-inference:
  1. Create `ArcModuleInput` with functions A, B, C
  2. Query `infer_borrow_scc` for all SCCs
  3. Re-create `ArcModuleInput` with SAME functions
  4. Query again — verify Salsa cache hits (no re-execution)

- [ ] Test: changed body, same sig → no caller re-inference:
  1. Function A calls B. B's body: `return x + 1`
  2. Query all SCCs → A gets B's sig (param x: Borrowed)
  3. Change B's body to `return x + 2` (same sig!)
  4. Re-query → B's SCC re-executes, produces same result → early cutoff → A NOT re-queried

- [ ] Test: changed body, different sig → caller re-infers:
  1. Function A calls B. B's body: `return x + 1` (x: Borrowed)
  2. Query all SCCs
  3. Change B's body to `Construct([x])` (x: now Owned!)
  4. Re-query → B's SCC re-executes, produces DIFFERENT result → A's SCC invalidated → A re-queried

- [ ] Test: new function added → only new SCC evaluated:
  1. Module has functions A, B
  2. Query all SCCs
  3. Add function C (calls A)
  4. Re-query → A's SCC NOT re-queried (input unchanged), C's new SCC evaluated

- [ ] Test: mutual recursion SCC stability:
  1. Functions A↔B form an SCC
  2. Query the SCC → fixed-point converges
  3. Change A's body (same sig) → SCC re-runs fixed-point → same result → early cutoff

- [ ] Measure execution counts:
  Use Salsa's event system or manual counters to verify exactly which queries re-executed.

---

## 12.13 Performance Benchmarking and Regression Gates

**File:** `compiler/oric/benches/borrow_inference.rs` (NEW)

Ensure the SCC-based approach doesn't regress cold-compile performance and provides measurable incremental wins.

- [ ] Benchmark: cold compile (no cache):
  Compare wall-clock time of whole-program `infer_borrows` vs SCC-based Salsa queries for:
  - Small module (5 functions)
  - Medium module (50 functions)
  - Large module (200+ functions)
  - Module with deep mutual recursion (SCC of 10+ functions)

  **Acceptable overhead:** ≤ 5% slower on cold compile (SCC computation + Salsa bookkeeping vs raw fixed-point). The incremental wins far outweigh a small cold-compile cost.

- [ ] Benchmark: warm compile (incremental):
  1. Compile module with Salsa borrow inference
  2. Change ONE function body (same borrow sig)
  3. Measure borrow inference time on re-compile
  - **Target:** ≥ 50% faster than cold compile for medium modules (only changed SCC re-evaluates)

- [ ] Benchmark: SCC computation overhead:
  Measure call graph extraction + Tarjan's SCC in isolation. This runs once per module per compilation. Should be < 1ms for 200 functions.

- [ ] Add regression gate:
  ```rust
  // In CI (if criterion benchmarks are in CI):
  // Assert cold-compile borrow inference < 2x of whole-program baseline
  // Assert incremental (same-sig change) < 0.5x of cold-compile
  ```

- [ ] Memory profile:
  Salsa stores memoized results. Verify per-SCC memoization doesn't cause significant memory growth compared to the single `FxHashMap<Name, AnnotatedSig>` cache. For 200 functions with ~10 SCCs, memory overhead should be negligible.

---

## 12.14 Watch-Mode Integration

**File:** `compiler/oric/src/commands/watch.rs` (future), `compiler/oric/src/db/mod.rs`

The ultimate payoff: watch-mode reuses the Salsa database across compilations, enabling true incremental borrow inference.

- [ ] Verify `CompilerDb` supports session reuse:
  Currently each `ori build` creates a fresh `CompilerDb`. Watch-mode would keep the `CompilerDb` alive across file changes. Salsa handles invalidation automatically — changing a `SourceFile` input triggers cascading re-evaluation through the query DAG.

- [ ] Update `ArcModuleInput` on file change:
  When `typed()` re-runs for a changed file, re-lower to ARC IR and re-set the `ArcModuleInput`. Salsa diffs the old and new input — unchanged functions produce cache hits in their SCCs.

- [ ] Handle the end-to-end incremental tests deferred from Section 08:
  The tests deferred in Section 08.4 (end-to-end incremental compilation) become possible once watch-mode and Salsa borrow inference are both in place:
  - [ ] Compile file → modify body (same sig) → recompile → verify borrow sig cache HIT
  - [ ] Compile file → modify body (different sig) → recompile → verify borrow sig cache MISS
  - [ ] Benchmark: compile time improvement from Salsa caching on multi-file program

- [ ] Verify no stale state across watch cycles:
  - File A changes → A's Salsa inputs updated → A's queries re-run
  - File B (imports A) → B's borrow queries depend on A's sigs → B re-evaluated if A's sigs changed
  - File C (no relation to A) → C's queries untouched

---

## 12.15 Remove Whole-Program Fallback

**File:** `compiler/ori_arc/src/borrow/mod.rs`, `compiler/oric/src/commands/compile_common.rs`

Once SCC-based inference is stable and all tests pass, remove the whole-program code path.

- [ ] Remove the `salsa-borrow` feature flag — make SCC-based the only path
- [ ] Deprecate and remove `infer_borrows()`:
  The public API of `ori_arc::borrow` changes from:
  ```rust
  // OLD:
  pub fn infer_borrows(functions: &[ArcFunction], classifier: &dyn ArcClassification) -> FxHashMap<Name, AnnotatedSig>
  ```
  to:
  ```rust
  // NEW:
  pub fn infer_borrow_single(func: &ArcFunction, external_sigs: &FxHashMap<Name, AnnotatedSig>, classifier: &dyn ArcClassification) -> AnnotatedSig
  pub fn infer_borrow_fixed_point(scc_functions: &[&ArcFunction], external_sigs: &FxHashMap<Name, AnnotatedSig>, classifier: &dyn ArcClassification) -> FxHashMap<Name, AnnotatedSig>
  ```

- [ ] Remove `BorrowSigCache` if Salsa memoization is sufficient (decision from 12.8)
- [ ] Remove the whole-program fixed-point loop from `borrow/mod.rs`
- [ ] Update `run_arc_pipeline_cached` to use only the Salsa path
- [ ] Update all tests to use the new API
- [ ] Final full test suite run: `./test-all.sh` — zero regressions
- [ ] Update Section 08 documentation to reflect the migration from side-cache to Salsa queries

---

## 12.16 Completion Checklist

- [ ] `CallGraph` struct with forward and reverse indexes (`graph/call_graph.rs`)
- [ ] `compute_sccs` with Tarjan's algorithm (`graph/scc.rs`)
- [ ] `ArcModuleInput` Salsa input type with functions, call graph, SCCs, scalar types
- [ ] `BorrowSigResult` Salsa-compatible output type (sorted Vec for deterministic Eq)
- [ ] `infer_borrow_scc` Salsa tracked query with per-SCC dispatch
- [ ] `infer_borrow_single` fast path for non-recursive functions
- [ ] `infer_borrow_fixed_point` for mutually recursive SCCs
- [ ] `CombinedSigs` view for reading local + external sigs during fixed-point
- [ ] Salsa `cycle_fn` recovery returning conservative all-Owned
- [ ] Early cutoff verified: same sig → no caller re-evaluation
- [ ] `ArcClassification` eliminated from query path (pre-computed scalar types)
- [ ] Pipeline wired: `compile_to_llvm()` uses SCC-based Salsa queries
- [ ] Feature flag `salsa-borrow` for gradual rollout
- [ ] Correctness parity: SCC-based matches whole-program for ALL test cases
- [ ] Incremental behavior verified: cache hits, early cutoffs, selective re-evaluation
- [ ] Performance: cold-compile ≤ 5% overhead, incremental ≥ 50% faster
- [ ] Watch-mode integration tested
- [ ] Whole-program fallback removed
- [ ] `./test-all.sh` passes with zero regressions
- [ ] No memory regression from Salsa memoization overhead

**Exit Criteria:** Borrow inference is fully incremental via Salsa. Changing a function body that doesn't affect its borrow signature triggers ZERO re-analysis of callers. The SCC decomposition preserves correctness for mutually recursive functions. Cold-compile performance is within 5% of the whole-program baseline. The `BorrowSigCache` side-cache is either eliminated (Salsa replaces it) or reduced to an aggregate convenience layer.
