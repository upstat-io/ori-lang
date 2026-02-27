---
section: "08"
title: "Escape Analysis & Stack Promotion"
status: not-started
goal: "Determine which heap allocations never escape their defining function and promote them to stack allocations, eliminating ARC overhead entirely"
inspired_by:
  - "Go escape analysis (cmd/compile/internal/escape/)"
  - "Java HotSpot scalar replacement (src/hotspot/share/opto/macro.cpp)"
  - "Swift stack promotion (lib/SILOptimizer/Transforms/StackPromotion.cpp)"
  - "Lean4 borrow analysis (src/Lean/Compiler/IR/Borrow.lean)"
depends_on: ["02"]
sections:
  - id: "08.1"
    title: "Intraprocedural Escape Analysis"
    status: not-started
  - id: "08.2"
    title: "Interprocedural Escape Analysis"
    status: not-started
  - id: "08.3"
    title: "Stack Promotion Codegen"
    status: not-started
  - id: "08.4"
    title: "Escape-Aware ARC Pipeline Integration"
    status: not-started
  - id: "08.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Escape Analysis & Stack Promotion

**Status:** Not Started
**Goal:** Values that are created and consumed within a single function are allocated on the stack instead of the heap, eliminating `ori_rc_alloc`, `ori_rc_inc`, `ori_rc_dec`, and `ori_rc_free` entirely. This is the single highest-impact ARC optimization — heap allocation is 10-50× slower than stack allocation.

**Context:** Ori's ARC system (ori_arc) already has liveness analysis and borrow inference. The missing piece is **escape analysis** — determining whether a reference-counted value's pointer is ever stored in a location that outlives the current function (return value, closure capture, global, or another heap object's field).

The ori_arc pipeline currently uses liveness-based analysis for RC insertion, but this is conservative: it treats all heap-allocated values as potentially escaping. True escape analysis would identify values that can be stack-promoted.

**Reference implementations:**
- **Go** `cmd/compile/internal/escape/`: Connection graph-based escape analysis — tracks dataflow from allocations to escape points
- **Swift** `lib/SILOptimizer/Transforms/StackPromotion.cpp`: Walks SIL to check if alloc_ref escapes
- **Java HotSpot** `macro.cpp`: Scalar replacement — replaces heap object with individual fields on stack
- **Lean4** `Borrow.lean`: Parameter ownership inference that implicitly identifies non-escaping borrows

**Depends on:** §02 (triviality classification helps escape analysis — trivial values don't need escape tracking).

---

## 08.1 Intraprocedural Escape Analysis

**File(s):** `compiler/ori_repr/src/escape/mod.rs`, `compiler/ori_repr/src/escape/intraprocedural.rs`

Start with per-function analysis (no cross-function information). This catches the most common patterns: temporary collections, intermediate strings, local structs.

- [ ] Define escape lattice:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
  pub enum EscapeState {
      /// Value definitely does not escape the function
      NoEscape,
      /// Value escapes to a callee but callee doesn't retain it (borrow)
      ArgEscape,
      /// Value escapes the function (returned, stored in global, captured by closure)
      GlobalEscape,
  }
  ```

- [ ] Implement connection graph:
  ```rust
  pub struct ConnectionGraph {
      /// Node per allocation site + parameter + return
      nodes: Vec<CgNode>,
      /// Edges: PointsTo (field → object), Deferred (alias)
      edges: Vec<CgEdge>,
  }

  pub enum CgNode {
      /// Allocation site (heap object creation)
      Alloc { id: AllocId, escape: EscapeState },
      /// Function parameter (escape state depends on callers)
      Param { index: usize, escape: EscapeState },
      /// Function return (always GlobalEscape)
      Return,
      /// Phantom node for unknown destinations
      Unknown,
  }

  pub enum CgEdge {
      /// a.field points to b
      PointsTo { from: NodeId, field: u32, to: NodeId },
      /// a defers to b (alias — same object)
      Deferred { from: NodeId, to: NodeId },
  }
  ```

- [ ] Implement escape propagation:
  ```rust
  pub fn analyze_escapes(func: &CanFunction, pool: &Pool) -> EscapeInfo {
      let mut graph = build_connection_graph(func, pool);

      // Fixed-point: propagate escape states through edges
      let mut changed = true;
      while changed {
          changed = false;
          for edge in &graph.edges {
              let (from_escape, to_escape) = match edge {
                  PointsTo { from, to, .. } => (graph.escape(*from), graph.escape(*to)),
                  Deferred { from, to } => (graph.escape(*from), graph.escape(*to)),
              };
              // If destination escapes, source must also escape
              let merged = from_escape.max(to_escape);
              if merged > graph.escape(edge.source()) {
                  graph.set_escape(edge.source(), merged);
                  changed = true;
              }
          }
      }

      EscapeInfo::from_graph(graph)
  }
  ```

- [ ] Escape sources (what causes GlobalEscape):
  - Value is returned from function
  - Value is stored in a mutable reference parameter
  - Value is captured by a closure that escapes
  - Value is stored in a global variable
  - Value is passed as an owned parameter to an unknown function
  - Value is stored in a heap object's field (that object escapes)

- [ ] Non-escape sinks (what keeps NoEscape):
  - Value is only read (not stored)
  - Value is passed as a borrowed parameter to a known function
  - Value is consumed (last use) within the function
  - Value is used in pattern matching then discarded

---

## 08.2 Interprocedural Escape Analysis

**File(s):** `compiler/ori_repr/src/escape/interprocedural.rs`

Cross-function escape analysis uses function summaries to track which parameters escape.

- [ ] Define function escape summaries:
  ```rust
  pub struct FunctionEscapeSummary {
      /// Which parameters escape?
      pub param_escapes: Vec<EscapeState>,
      /// Does the return value contain any input parameters?
      pub return_aliases: Vec<usize>, // param indices
  }
  ```

- [ ] Compute summaries bottom-up through the call graph:
  - Leaf functions (no callees): direct analysis
  - Non-leaf functions: use callee summaries to refine escape states
  - Recursive functions: conservative (assume all params escape) then refine

- [ ] Apply summaries at call sites:
  ```rust
  // At call site: f(x, y, z)
  // If f's summary says param 0 doesn't escape:
  //   → x does NOT escape through this call
  // If f's summary says param 1 escapes:
  //   → y DOES escape through this call
  ```

- [ ] Integration with ori_arc borrow inference:
  - The borrow inference in `ori_arc::borrow` already computes per-parameter ownership (Borrowed/Owned)
  - `Borrowed` ≈ `ArgEscape` (callee uses but doesn't retain)
  - `Owned` ≈ `GlobalEscape` (callee may retain)
  - Unify these two analyses to avoid redundant computation

---

## 08.3 Stack Promotion Codegen

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`, `compiler/ori_llvm/src/codegen/expr_compiler.rs`

When escape analysis marks an allocation as `NoEscape`, generate stack allocation instead of heap.

- [ ] Replace `ori_rc_alloc` with `alloca`:
  ```llvm
  ; Before (heap):
  %ptr = call ptr @ori_rc_alloc(i64 24, i64 8)
  ; ... use ptr ...
  call void @ori_rc_dec(ptr %ptr, ptr @_ori_drop$42)

  ; After (stack):
  %ptr = alloca [24 x i8], align 8
  ; ... use ptr ...
  ; No rc_dec needed — stack memory is freed automatically
  ```

- [ ] Eliminate all RC operations for stack-promoted values:
  - No `ori_rc_inc` (refcount is meaningless on stack)
  - No `ori_rc_dec` (no free needed)
  - No drop function call (fields are dropped individually at function exit)

- [ ] Handle non-trivial fields in stack-promoted values:
  - If the struct has RC'd fields (e.g., `struct { name: str, age: int }`):
    - The struct itself is on the stack (no RC header)
    - The `str` field still has its own RC (it's a separate heap allocation)
    - At function exit, `ori_rc_dec` the str field (but not the struct)

- [ ] Lifetime extension for stack-promoted values:
  - If the value is live across a function call, the alloca must dominate the call
  - LLVM's alloca in the entry block is lifetime-safe
  - Use `llvm.lifetime.start` / `llvm.lifetime.end` intrinsics for precise scoping

---

## 08.4 Escape-Aware ARC Pipeline Integration

**File(s):** `compiler/ori_arc/src/rc_insert/mod.rs`, `compiler/ori_arc/src/lib.rs`

Feed escape information into the ARC pipeline so it can skip RC operations.

- [ ] Add `EscapeInfo` to `run_arc_pipeline()` parameters:
  ```rust
  pub fn run_arc_pipeline(
      func: &mut ArcFunction,
      classifier: &ArcClassifier,
      sigs: &SignatureMap,
      pool: &Pool,
      interner: &NameInterner,
      escape_info: &EscapeInfo,  // NEW
  ) -> ArcResult { ... }
  ```

- [ ] In `insert_rc_ops_with_ownership()`:
  - Skip `RcInc`/`RcDec` for variables whose allocation site is `NoEscape`
  - Mark these variables as `DerivedOwnership::Fresh` with a `stack_promoted` flag

- [ ] In `detect_reset_reuse_cfg()`:
  - Stack-promoted values are always uniquely owned → always eligible for in-place reuse

---

## 08.5 Completion Checklist

- [ ] `let list = [1, 2, 3]; len(list)` — list is stack-promoted (no heap alloc)
- [ ] `let s = str(42); print(s)` — string is stack-promoted if `print` borrows
- [ ] `let point = Point { x: 1, y: 2 }; point.x + point.y` — struct on stack (no RC header)
- [ ] Values returned from functions are NOT stack-promoted (correctly identified as escaping)
- [ ] Closures that capture values correctly mark those values as escaping
- [ ] `./test-all.sh` green
- [ ] `./scripts/valgrind-aot.sh` clean (no use-after-free from premature stack deallocation)
- [ ] `./scripts/dual-exec-verify.sh` passes (eval and AOT produce identical results)
- [ ] Zero `ori_rc_alloc` calls for functions that only use non-escaping values

**Exit Criteria:** A function that creates a temporary list, computes its length, and returns the length generates ZERO `ori_rc_alloc`/`ori_rc_dec` calls in LLVM IR. Verified by `grep -c "ori_rc" function.ll` returning 0. Valgrind reports 0 heap allocations for this function.
