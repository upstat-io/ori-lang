---
plan: "dpr_arc-optimization_02212026"
title: "Design Pattern Review: ARC Optimization"
status: draft
---

# Design Pattern Review: ARC Optimization

## Ori Today

Ori's ARC system lives in the `ori_arc` crate (~2.6k lines of production code, plus extensive tests) and is modeled directly after Lean 4's LCNF-based approach. The architecture is sound: a backend-independent basic-block IR (`ArcFunction`, `ArcBlock`, `ArcInstr`, `ArcTerminator`) feeds a well-ordered pipeline of analysis passes. Type classification (`ArcClassifier` implementing the `ArcClassification` trait) provides the foundational three-way lattice (`Scalar`/`DefiniteRef`/`PossibleRef`) that all subsequent passes depend on. The pipeline is canonically ordered in `run_arc_pipeline()`: derived ownership inference, dominator tree construction, refined liveness computation, RC insertion, reset/reuse detection, reuse expansion, and RC elimination.

The implementation is genuinely well-engineered. Borrow inference (`infer_borrows`) uses Lean 4's monotonic fixed-point algorithm with tail-call preservation. Derived ownership (`infer_derived_ownership`) extends borrow tracking to all SSA variables via a single forward pass, classifying each as `Owned`, `BorrowedFrom(root)`, or `Fresh`. RC insertion (`insert_rc_ops_with_ownership`) performs a backward walk with liveness-driven placement, handling borrowed-derived variables at owned positions and closure capture analysis. Reset/reuse detection (`detect_reset_reuse_cfg`) operates both intra-block and cross-block using dominator trees and refined liveness (which distinguishes live-for-use from live-for-drop). RC elimination runs three phases: intra-block bidirectional dataflow, single-predecessor cross-block pairs, and multi-predecessor join-point elimination. The FBIP diagnostic pass (`analyze_fbip`) provides Koka-inspired reporting on achieved vs missed reuse opportunities. Drop descriptors (`DropInfo`/`DropKind`) declaratively specify per-type cleanup, keeping the analysis backend-independent.

The gaps are concentrated in two areas. First, the LLVM codegen layer (`arc_emitter.rs`, ~1027 lines) is functional but has significant stubs: `IsShared` always emits `false` (assumes unique), `Reuse` falls back to fresh `Construct`, `PartialApply` produces null closure environments, and `RcDec` passes a null drop function. Second, there is no inter-function RC elimination -- identical Inc/Dec pairs that span call boundaries are not optimized, and the optimizer cannot see through function calls to eliminate redundant RC traffic.

**Completed (2026-02-21 hygiene review):** The runtime's `ori_rc_inc`/`ori_rc_dec` now use `AtomicI64::fetch_add(1, Relaxed)` / `fetch_sub(1, Release)` with `Acquire` fence before drop, matching Swift's `swift_retain`/`swift_release` and Rust's `Arc` ordering. A `--single-threaded` feature flag selects non-atomic operations for programs that don't use task parallelism. Additionally, `ori_rc_dec`'s `drop_fn` invocation is wrapped in `catch_unwind` + `abort` to enforce the `nounwind` contract declared in LLVM IR — if a drop function panics, the process aborts cleanly rather than producing UB by unwinding through a `nounwind` boundary. A `debug_assert!(prev > 0)` catches use-after-free bugs in debug builds.

## Prior Art

### Swift -- Dual-Pass Lattice Dataflow

Swift's ARC optimizer (`lib/SILOptimizer/ARC/`) is the most mature production system, using separate bottom-up and top-down dataflow passes that discover retain/release pairs via lattice-state machines. The key innovation is the `BlotMapVector<T, 4>` data structure that enables O(1) erasure while preserving iteration order, and the "Known Safe" flag that detects nested retain/release pairs where the outer retain proves deallocation cannot occur. The system handles arbitrary control flow via conservative lattice merging at join points -- no proof complexity, just monotonic state transitions. RC identity normalization traces field projections back to canonical root values via `RCIdentityFunctionInfo`, preventing the optimizer from treating `x.field` as a different identity from `x`. At 5.7K+ lines across 21 files, it is production-proven but overkill for Ori's current scale.

What matters for Ori: the RC identity normalization concept (tracing projections to roots) is exactly what Ori's `DerivedOwnership::BorrowedFrom(root)` already captures, but Swift goes further by using this as a general equivalence relation across all optimization passes, not just borrow inference. The "Known Safe" flag -- the insight that an outer Inc/Dec pair guarantees inner operations cannot trigger deallocation -- is a powerful optimization that Ori's elimination pass does not yet exploit.

### Lean 4 -- Two-Phase ExplicitRC + BorrowInference

Lean 4's approach (`src/Lean/Compiler/IR/RC.lean` + `Borrow.lean`) is the direct ancestor of Ori's design. Phase 1 inserts `inc`/`dec` via live-variable analysis with type classification (`isScalar`/`isPossibleRef`/`isDefiniteRef`). Phase 2 runs fixpoint borrow inference, marking parameters as `Borrowed` when the callee never stores or returns them. The `DerivedValMap` tracks parent-child projection chains -- the same concept Ori implements as `DerivedOwnership::BorrowedFrom`. Reset/reuse (`ExpandResetReuse.lean`) uses an `isShared` runtime check to select between in-place mutation and fresh allocation. The persistent object optimization (immutable values with refcount pinned to prevent deallocation) is unique to Lean's functional model.

What matters for Ori: Lean's architecture validates Ori's existing design choices, but Lean's implementation is tighter -- its borrow inference operates at the LCNF level where join points are explicit, making the fixed-point cleaner. Ori should adopt Lean's join-point discipline where block parameters serve as explicit merge points, and ensure the borrow inference handles these correctly (Ori already uses block parameters, but the connection to join-point semantics could be made more explicit in the analysis).

### Koka -- Perceus Direct Insertion

Koka's Perceus algorithm (`Parc.hs`) takes the most direct approach: a single reverse-order tree walk inserts `dup`/`drop` operations as expressions are consumed, with pre-computed `ParamInfo` separating borrowed parameters from owned ones. The `CheckFBIP.hs` validator (32K lines) is a diagnostic pass that verifies functional-but-in-place invariants -- checking that constructor-only code achieves allocation reuse. Koka's `Fip` (Functional In-Place) annotation system lets developers declare that a function must achieve full reuse, turning missed reuse from an optimization miss into a compile error.

What matters for Ori: Ori already has the FBIP diagnostic pass (`analyze_fbip`) inspired by `CheckFBIP.hs`, but lacks the enforcement angle -- an `@fbip` annotation on functions that promotes missed reuse to a compiler error. The Perceus insight that borrowed/owned sets are scope-invariant (avoiding control flow merging complexity) is partially captured by Ori's `DerivedOwnership` forward pass, but Koka's reverse-order tree walk achieves this without building an SSA IR first. For Ori, the SSA IR is the right choice (it enables the full pass pipeline), but the scope-invariant property should be verified as an invariant rather than just an implementation detail.

## Proposed Best-of-Breed Design

### Core Idea

Ori's ARC system is already well-architected on the Lean 4 foundation. The proposal focuses on three categories of improvement: (1) closing the LLVM codegen gaps to make the existing pipeline production-ready, (2) adopting Swift's "Known Safe" RC identity normalization to strengthen elimination, and (3) adding Koka's enforcement annotations to turn FBIP from diagnostic to verification. The guiding principle is that Ori's dual-execution model (JIT + AOT) creates a unique opportunity: the JIT path can use conservative RC (correctness-first, simpler codegen) while the AOT path applies the full optimization pipeline, and both are verified against the same test suite.

Rather than a ground-up redesign, this proposal enhances the existing pass pipeline with three new sub-passes and completes the codegen layer. The pipeline becomes: borrow inference -> derived ownership -> liveness -> RC insertion -> **RC identity propagation (new)** -> reset/reuse detection -> reuse expansion -> **known-safe elimination (new)** -> RC elimination -> **FBIP enforcement (enhanced)**. Each addition is independently valuable and can be landed incrementally.

### Key Design Choices

1. **Atomic refcount operations in `ori_rt`** (Swift-inspired) — **DONE.** `ori_rc_inc` uses `AtomicI64::fetch_add(1, Relaxed)`, `ori_rc_dec` uses `fetch_sub(1, Release)` + `Acquire` fence before drop. Feature flag `single-threaded` selects non-atomic fast path. Additionally: `drop_fn` calls are guarded by `catch_unwind` + `abort` (enforces the `nounwind` contract), and `debug_assert!(prev > 0)` catches use-after-free in debug builds.

2. **RC identity normalization via `RcIdentity` map** (Swift-inspired). Swift's `RCIdentityFunctionInfo` traces projections to canonical roots. Ori's `DerivedOwnership::BorrowedFrom(root)` captures this for borrow tracking, but the elimination pass (`eliminate_rc_ops_dataflow`) only uses it for Phase 2 (ownership-redundant removal). A new `RcIdentityMap` should be computed once (from `DerivedOwnership`) and consulted in all elimination phases, enabling the optimizer to recognize that `RcInc(x.field)` is equivalent to `RcInc(x)` when `x.field` is `BorrowedFrom(x)`.

3. **Known-safe pair detection** (Swift-inspired). When an `RcInc(x)` at position P guarantees `x` stays alive until position Q, any `RcInc(y); RcDec(y)` pair where `y` is derived from `x` and both operations fall within [P, Q] can be eliminated without checking for intervening uses. This is Swift's "Known Safe" optimization. Ori can implement this by extending `EliminationCandidate` with a `known_safe: bool` flag and adding a pre-pass that identifies "guarding" Inc/Dec pairs.

4. **Drop function wiring in codegen** (completing the Lean 4 pattern). The `DropInfo`/`DropKind` descriptors in `ori_arc::drop` are fully implemented but not yet consumed by `arc_emitter.rs` -- `RcDec` currently passes `null` as `drop_fn`. Each `DropKind` variant maps to a specific LLVM IR generation pattern: `Trivial` generates a direct `ori_rc_free` call; `Fields` generates per-field GEP + recursive `RcDec`; `Enum` generates a switch on tag + per-variant field Dec; `Collection` generates an iteration loop; `Map` generates key/value iteration. These drop functions are generated once per type and cached by mangled name.

5. **`IsShared` inline RC check in codegen** (completing the Lean 4 pattern). The `IsShared` instruction currently emits `const false` (always unique). The correct implementation loads the refcount at `data_ptr - 8`, compares against 1, and branches. This is a 3-instruction sequence (GEP, load, icmp) that should be inlined, not a function call, to avoid the overhead of a call for every reuse check.

6. **FBIP enforcement annotations** (Koka-inspired). Koka's `@fip` annotation turns missed reuse from warning to error. Ori should support `@fbip` on function declarations, meaning "this function must achieve full in-place reuse for all its allocations." The existing `analyze_fbip` pass produces `FbipReport` with `achieved` and `missed` lists -- enforcement simply checks that `missed` is empty when the annotation is present, and emits an `E-level` diagnostic for each `MissedReuse`.

7. **Inter-function RC specialization via Salsa** (Ori-unique). None of the reference compilers have Ori's Salsa-based incremental compilation. When a callee's borrow signature changes (e.g., a parameter goes from `Owned` to `Borrowed`), Salsa can automatically invalidate and recompute only the affected callers' RC insertion. This means the borrow inference results should be stored as a Salsa query output, and RC insertion should be a dependent query. The existing `FxHashMap<Name, AnnotatedSig>` is the right data structure -- it just needs to be wrapped in a Salsa tracked struct.

8. **Batched Inc optimization** (Lean 4 pattern). The existing `RcInc { var, count: u32 }` supports batched increments, but `count > 1` is treated conservatively by the elimination pass (invalidated rather than reduced). The elimination should reduce `count` by 1 when a matching Dec is found, only removing the instruction when `count` reaches 0. This handles the common pattern of passing the same value to multiple owned positions.

### What Makes Ori's Approach Unique

**Dual JIT/AOT verification.** No reference compiler runs the same ARC-optimized code through both an interpreter-backed JIT and a full AOT pipeline against the same test suite. Ori's mandatory test requirement means every function's RC behavior is verified under both execution models. This creates a powerful correctness guarantee: if the JIT (with conservative RC) produces the same results as the AOT (with full optimization), the optimizations are correct. The implication for design is that the JIT path does not need the full optimization pipeline -- it can use `insert_rc_ops` without `eliminate_rc_ops` or `reset_reuse` -- while the AOT path applies everything. Correctness regressions surface immediately in the test runner.

**Expression-based last-value semantics.** Ori's expression-based design (no `return` keyword, last expression is the block's value) means every block has exactly one "result" value that flows outward. This constrains the control flow in ways that benefit ARC analysis: there are no early returns to inject surprise Dec operations, no goto-based control flow to create unreachable RC paths, and every if/match branch produces a value that merges at a common successor. This makes liveness analysis simpler (fewer "live-for-drop" variables) and reset/reuse detection more predictable (the result value is always the last thing constructed).

**Capability-based effect isolation.** When Ori's capability system (`uses Http`, `with Http = Mock in`) is fully implemented, it creates a unique ARC optimization opportunity: capability-free functions are guaranteed to not perform IO, not spawn tasks, and not interact with shared state. This means the ARC optimizer can be more aggressive with these functions -- for example, proving that a capability-free function that takes `Borrowed` parameters cannot cause deallocation of any value reachable from those parameters (no side-effecting callback can run during the function's execution). This is stronger than what Swift, Lean 4, or Koka can prove.

**Mandatory tests as verification infrastructure.** Ori requires tests for every function (except `@main`). This means the FBIP enforcement annotation (`@fbip`) has teeth -- every function annotated with `@fbip` has tests that exercise its allocation patterns. Combined with the dual JIT/AOT execution, this creates a three-way verification: (1) the FBIP analysis proves reuse at the IR level, (2) the JIT confirms correctness under conservative RC, and (3) the AOT confirms correctness under optimized RC. No reference compiler has this level of integrated verification.

### Concrete Types & Interfaces

The following types and interfaces are **new** -- they extend the existing `ori_arc` infrastructure without replacing anything.

```rust
// ── RC Identity Propagation (new sub-pass) ──────────────────────────

/// Maps each variable to its canonical RC identity root.
///
/// Built from `DerivedOwnership` in a single pass. Two variables with the
/// same `RcIdentity` are provably aliases of the same heap object --
/// Inc/Dec operations on either affect the same refcount.
///
/// Extends the existing `DerivedOwnership::BorrowedFrom(root)` to create
/// a transitive closure: if v1 borrows from v2 and v2 borrows from v3,
/// all three share the same `RcIdentity`.
///
/// Inspired by Swift's `RCIdentityFunctionInfo`.
pub struct RcIdentityMap {
    /// Maps `ArcVarId::index()` -> canonical root `ArcVarId`.
    /// Variables not in the map are their own identity.
    roots: Vec<ArcVarId>,
}

impl RcIdentityMap {
    /// Build from derived ownership vector.
    ///
    /// Follows `BorrowedFrom` chains to their transitive root.
    /// `Fresh` and `Owned` variables are their own root.
    pub fn build(ownership: &[DerivedOwnership]) -> Self {
        let mut roots: Vec<ArcVarId> = (0..ownership.len())
            .map(|i| ArcVarId::new(i as u32))
            .collect();

        for (i, own) in ownership.iter().enumerate() {
            if let DerivedOwnership::BorrowedFrom(source) = own {
                // Chase to transitive root
                let mut root = *source;
                while let DerivedOwnership::BorrowedFrom(parent) =
                    ownership[root.index()]
                {
                    if parent == root { break; } // cycle guard
                    root = parent;
                }
                roots[i] = root;
            }
        }

        Self { roots }
    }

    /// Get the canonical RC identity for a variable.
    #[inline]
    pub fn root(&self, var: ArcVarId) -> ArcVarId {
        self.roots.get(var.index())
            .copied()
            .unwrap_or(var)
    }

    /// Do two variables share the same RC identity?
    #[inline]
    pub fn same_identity(&self, a: ArcVarId, b: ArcVarId) -> bool {
        self.root(a) == self.root(b)
    }
}

// ── Known-Safe Pair Detection (new sub-pass) ─────────────────────────

/// A "guarding" Inc/Dec interval within a block.
///
/// If `RcInc(guard)` at `inc_pos` is paired with `RcDec(guard)` at
/// `dec_pos`, then any Inc/Dec pair on a variable derived from `guard`
/// that falls entirely within [inc_pos, dec_pos] can be eliminated
/// without checking for intervening uses -- the guarding pair guarantees
/// the object stays alive.
///
/// Inspired by Swift's "Known Safe" optimization in
/// `BottomUpRefCountState`.
struct GuardInterval {
    /// The guarding variable (canonical RC identity).
    guard: ArcVarId,
    /// Instruction index of the guarding RcInc.
    inc_pos: usize,
    /// Instruction index of the guarding RcDec.
    dec_pos: usize,
}

/// Find known-safe elimination candidates within a guarding interval.
///
/// For each `GuardInterval`, any `RcInc(x); ...; RcDec(x)` where
/// `identity_map.root(x) == guard` and both positions fall within
/// `[inc_pos, dec_pos]` is safe to eliminate regardless of intervening
/// uses, because the guard keeps the object alive.
fn find_known_safe_candidates(
    body: &[ArcInstr],
    block_idx: usize,
    guards: &[GuardInterval],
    identity_map: &RcIdentityMap,
) -> Vec<EliminationCandidate> {
    let mut candidates = Vec::new();

    for guard in guards {
        for (j, instr) in body.iter().enumerate() {
            if j <= guard.inc_pos || j >= guard.dec_pos {
                continue;
            }

            let var = match instr {
                ArcInstr::RcInc { var, count: 1 } => *var,
                _ => continue,
            };

            if identity_map.root(var) != guard.guard {
                continue;
            }

            // Scan forward for matching Dec within the guard interval
            for (k, later) in body.iter().enumerate().skip(j + 1) {
                if k >= guard.dec_pos {
                    break;
                }
                if let ArcInstr::RcDec { var: dec_var } = later {
                    if *dec_var == var {
                        candidates.push(EliminationCandidate {
                            var,
                            block: block_idx,
                            inc_pos: j,
                            dec_pos: k,
                        });
                        break;
                    }
                }
            }
        }
    }

    candidates
}

// ── FBIP Enforcement (enhanced) ─────────────────────────────────────

/// Enforcement level for FBIP analysis.
///
/// When a function is annotated `@fbip`, missed reuse opportunities
/// become compile errors instead of diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FbipEnforcement {
    /// Diagnostic only -- report achieved and missed reuse.
    Diagnostic,
    /// Enforced -- missed reuse is a compile error.
    Required,
}

/// Extended FBIP report with enforcement support.
pub struct FbipResult {
    pub report: FbipReport,
    pub enforcement: FbipEnforcement,
    /// Diagnostics to emit (warnings for Diagnostic, errors for Required).
    pub diagnostics: Vec<FbipDiagnostic>,
}

/// A single FBIP diagnostic (warning or error depending on enforcement).
pub struct FbipDiagnostic {
    pub missed: MissedReuse,
    pub severity: FbipSeverity,
    pub suggestion: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FbipSeverity {
    Warning,
    Error,
}

/// Check FBIP enforcement and produce diagnostics.
pub fn check_fbip_enforcement(
    report: FbipReport,
    enforcement: FbipEnforcement,
) -> FbipResult {
    let diagnostics: Vec<FbipDiagnostic> = report
        .missed
        .iter()
        .map(|missed| {
            let severity = match enforcement {
                FbipEnforcement::Diagnostic => FbipSeverity::Warning,
                FbipEnforcement::Required => FbipSeverity::Error,
            };
            let suggestion = match &missed.reason {
                MissedReuseReason::TypeMismatch { .. } =>
                    "restructure to produce the same type that is consumed".into(),
                MissedReuseReason::IntermediateUse { .. } =>
                    "move the use before the decrement or copy the value".into(),
                MissedReuseReason::NoDominance =>
                    "restructure so the allocation site dominates the constructor".into(),
                MissedReuseReason::PossiblyShared =>
                    "ensure the value has a unique reference at the decrement point".into(),
                MissedReuseReason::NoMatchingConstruct =>
                    "ensure a constructor of the same type exists in dominated scope".into(),
            };
            FbipDiagnostic {
                missed: missed.clone(),
                severity,
                suggestion,
            }
        })
        .collect();

    FbipResult {
        report,
        enforcement,
        diagnostics,
    }
}

// ── Atomic Refcount (runtime enhancement) — IMPLEMENTED ─────────────
//
// Implemented in ori_rt/src/lib.rs (commit b94a753d + hygiene review).
// See ori_rc_inc() and ori_rc_dec() for the live code.
//
// Key details beyond the original proposal:
// - `ori_rc_dec` wraps `drop_fn` in `catch_unwind` + `abort` to enforce
//   the `nounwind` contract declared in LLVM IR (runtime_decl/mod.rs:144).
// - `debug_assert!(prev > 0)` catches use-after-free in debug builds.
// - `ori_rc_alloc` initializes via `AtomicI64::new(1)` (multi-threaded)
//   or plain `i64` write (single-threaded). No ordering needed on init
//   since the allocation isn't yet visible to other threads.

// ── Batched Inc Reduction (elimination enhancement) ─────────────────

/// Reduce a batched `RcInc` count instead of eliminating the entire
/// instruction. Returns `true` if the instruction should be removed
/// (count reached 0).
fn reduce_inc_count(instr: &mut ArcInstr) -> bool {
    if let ArcInstr::RcInc { count, .. } = instr {
        *count = count.saturating_sub(1);
        *count == 0
    } else {
        false
    }
}

// ── Enhanced Pipeline (updated run_arc_pipeline) ────────────────────

/// The enhanced ARC optimization pipeline.
///
/// Additions marked with (NEW):
/// 1. Derived ownership inference
/// 2. RC identity map construction (NEW)
/// 3. Dominator tree
/// 4. Refined liveness
/// 5. RC insertion
/// 6. Reset/reuse detection
/// 7. Reuse expansion
/// 8. Known-safe elimination (NEW)
/// 9. RC elimination (enhanced with identity map)
/// 10. FBIP enforcement check (NEW, optional)
// pub fn run_arc_pipeline_v2(
//     func: &mut ArcFunction,
//     classifier: &dyn ArcClassification,
//     sigs: &FxHashMap<Name, AnnotatedSig>,
//     fbip_enforcement: Option<FbipEnforcement>,
// ) -> Option<FbipResult> {
//     let ownership = borrow::infer_derived_ownership(func, sigs);
//     let identity_map = RcIdentityMap::build(&ownership);  // NEW
//     let dom_tree = graph::DominatorTree::build(func);
//     let (refined, liveness) = liveness::compute_refined_liveness(func, classifier);
//     rc_insert::insert_rc_ops_with_ownership(func, classifier, &liveness, &ownership, sigs);
//     reset_reuse::detect_reset_reuse_cfg(func, classifier, &dom_tree, &refined);
//     expand_reuse::expand_reset_reuse(func, classifier);
//     // NEW: known-safe elimination before general elimination
//     eliminate_known_safe(func, &identity_map);
//     // Enhanced: pass identity map to elimination
//     rc_elim::eliminate_rc_ops_dataflow(func, &ownership);
//     // NEW: FBIP enforcement
//     fbip_enforcement.map(|enforcement| {
//         let report = fbip::analyze_fbip(func, classifier, &dom_tree, &refined);
//         check_fbip_enforcement(report, enforcement)
//     })
// }
```

## Implementation Roadmap

### Phase 1: Foundation (codegen completeness)

- [x] **Atomic refcount in `ori_rt`** *(done: b94a753d + hygiene review 2026-02-21)*: `ori_rc_inc`/`ori_rc_dec` use `AtomicI64` with `Relaxed`/`Release` ordering. Feature flag `single-threaded` for non-atomic fast path. Drop function calls guarded by `catch_unwind` + `abort` (nounwind enforcement). Debug assert for use-after-free.
- [x] **Runtime hygiene fixes** *(done: hygiene review 2026-02-21)*:
  - `static mut ORI_PANIC_TRAMPOLINE` → `AtomicPtr<()>` with `Relaxed` ordering (eliminates data-race UB)
  - `ori_str_from_bool` now heap-allocates via `OriStr::from_owned()` (uniform ownership with `from_int`/`from_float`)
  - `ori_assert*` functions now route through `ori_panic_cstr` on failure (JIT: longjmp, AOT: unwind)
  - Panic/assertion functions use `extern "C-unwind"` (allows unwinding through FFI boundary)
  - List allocation uses `Layout::from_size_align(total, 8)` (minimum 8-byte alignment matching `ori_alloc`)
- [ ] **Drop function generation in `arc_emitter.rs`**: Wire `DropInfo`/`DropKind` from `ori_arc::drop` into `emit_instr` for `RcDec`. Generate per-type LLVM IR drop functions: `Trivial` -> `ori_rc_free`; `Fields` -> GEP+load+recursive Dec; `Enum` -> switch+per-variant Dec; `Collection` -> iteration loop; `Map` -> key/value iteration loop; `ClosureEnv` -> same as Fields. Cache generated functions by mangled type name (`_ori_drop$TypeName`). **Note:** Generated drop functions must be `nounwind` at the LLVM level — `ori_rc_dec`'s `call_drop_fn` enforces this with abort-on-panic.
- [ ] **`IsShared` inline check in `arc_emitter.rs`**: Replace `const_bool(false)` with: `let rc_ptr = gep(data_ptr, -8); let rc_val = load(i64, rc_ptr); let is_shared = icmp_sgt(rc_val, 1)`. This is the gate for reset/reuse fast-path correctness.
- [ ] **`Reuse` emission in `arc_emitter.rs`**: On the fast path (after `IsShared` returns false), emit `Set` instructions for field mutation and `SetTag` for variant changes. The slow path (shared) emits `RcDec` + `Construct` as it does today. This completes the reuse expansion codegen.
- [ ] **`PartialApply` closure environment**: Generate proper environment struct allocation via `ori_rc_alloc`, pack captured variables via GEP+store, and emit wrapper function that unpacks env + forwards to the actual callee. Currently emits null pointers.

### Phase 2: Core (optimization enhancements)

- [ ] **`RcIdentityMap` construction**: Implement the `RcIdentityMap::build()` function that chases `BorrowedFrom` chains to transitive roots. Add unit tests verifying transitive closure (a borrows from b borrows from c -> all share root c). Single file: `ori_arc/src/rc_identity.rs` (~80 lines).
- [ ] **Known-safe pair elimination**: Implement `find_known_safe_candidates()` as a pre-pass within `eliminate_rc_ops_dataflow`. Identify "guarding" Inc/Dec intervals per block, then eliminate inner pairs on derived variables without checking for intervening uses. Add test cases: nested struct access, linked-list traversal with field extraction, closure capture within a guarded scope. New file: `ori_arc/src/rc_elim/known_safe.rs` (~120 lines).
- [ ] **Batched Inc reduction**: Update `apply_eliminations` in `rc_elim` to reduce `count` by 1 instead of removing the entire `RcInc` when `count > 1`. Add test case: `RcInc { var: x, count: 3 }; RcDec { var: x }` -> `RcInc { var: x, count: 2 }`.
- [ ] **Identity-aware elimination**: Thread `RcIdentityMap` through `eliminate_rc_ops_dataflow`. In Phase 2 (ownership elimination), use `identity_map.same_identity(var, source)` instead of only checking `BorrowedFrom` directly -- this handles transitive chains that the current implementation misses.
- [ ] **Inter-function borrow signature caching via Salsa**: Wrap `FxHashMap<Name, AnnotatedSig>` in a Salsa tracked struct so that changes to a callee's borrow signature automatically invalidate callers' RC insertion. This prevents stale RC ops when editing a function that many others call.

### Phase 3: Polish (verification and enforcement)

- [ ] **FBIP enforcement annotation**: Add `@fbip` as a recognized function annotation in `ori_ir`. Thread through parser -> type checker -> ARC pipeline. In `run_arc_pipeline`, check the annotation and call `check_fbip_enforcement` with `FbipEnforcement::Required`. Emit `E-level` diagnostic for each `MissedReuse` when enforced.
- [ ] **Dual-execution RC verification**: Add an integration test mode that runs each `@test` function through both JIT (with `insert_rc_ops` only, no elimination) and AOT (with full pipeline), comparing outputs. Any divergence indicates an optimization bug. Wire into `./test-all.sh`.
- [ ] **Capability-aware borrow inference**: When the capability system is implemented, extend `infer_borrows` to recognize capability-free functions. A capability-free callee that borrows a parameter is guaranteed not to cause deallocation of any reachable value, enabling the caller to skip `RcInc` even when passing to "owned" positions in non-escaping contexts.
- [ ] **RC operation statistics**: Add `--emit-arc-stats` flag that dumps per-function RC operation counts (insertions, eliminations, reuse achieved, reuse missed) in JSON format. Useful for regression tracking and optimization tuning.
- [ ] **Cross-function RC elimination (future)**: After inter-function borrow caching is stable, implement cross-call-boundary elimination. When a callee is known to immediately Dec a parameter that the caller just Inc'd (visible through the borrow signature), elide both operations. This requires annotating the call site with the callee's parameter ownership and is the most complex optimization in the pipeline.

## References

### Swift
- `~/projects/reference_repos/lang_repos/swift/lib/SILOptimizer/ARC/` -- `ARCSequenceOpts.cpp`, `GlobalARCSequenceDataflow.cpp`, `RCStateTransition.h`
- `~/projects/reference_repos/lang_repos/swift/lib/SILOptimizer/ARC/ARCMatchingSet.h` -- Inc/Dec pairing
- `~/projects/reference_repos/lang_repos/swift/include/swift/SIL/OwnershipUtils.h` -- ownership SSA
- `~/projects/reference_repos/lang_repos/swift/lib/SIL/Utils/OwnershipUtils.cpp` -- RC identity analysis

### Lean 4
- `~/projects/reference_repos/lang_repos/lean4/src/Lean/Compiler/IR/RC.lean` -- ExplicitRC insertion
- `~/projects/reference_repos/lang_repos/lean4/src/Lean/Compiler/IR/Borrow.lean` -- borrow inference
- `~/projects/reference_repos/lang_repos/lean4/src/Lean/Compiler/IR/ExpandResetReuse.lean` -- constructor reuse
- `~/projects/reference_repos/lang_repos/lean4/src/Lean/Compiler/IR/LiveVars.lean` -- liveness analysis

### Koka
- `~/projects/reference_repos/lang_repos/koka/src/Backend/C/Parc.hs` -- Perceus RC insertion
- `~/projects/reference_repos/lang_repos/koka/src/Core/CheckFBIP.hs` -- FBIP validation
- `~/projects/reference_repos/lang_repos/koka/src/Backend/C/ParcReuse.hs` -- reuse analysis

### Ori (current implementation)
- `compiler/ori_arc/src/lib.rs` -- pipeline orchestration, `ArcClass`, `ArcClassification` trait
- `compiler/ori_arc/src/ir/mod.rs` -- `ArcFunction`, `ArcInstr`, `ArcTerminator` (14 instruction variants)
- `compiler/ori_arc/src/classify/mod.rs` -- `ArcClassifier` with cache and cycle detection
- `compiler/ori_arc/src/borrow/mod.rs` -- `infer_borrows` (fixed-point), `infer_derived_ownership` (SSA forward pass)
- `compiler/ori_arc/src/ownership/mod.rs` -- `Ownership`, `DerivedOwnership`, `AnnotatedSig`
- `compiler/ori_arc/src/liveness/mod.rs` -- `compute_liveness`, `compute_refined_liveness`, `RefinedLiveness`
- `compiler/ori_arc/src/rc_insert/mod.rs` -- `insert_rc_ops_with_ownership`, edge cleanup, trampoline blocks
- `compiler/ori_arc/src/rc_elim/mod.rs` -- bidirectional intra-block, cross-block, join-point elimination
- `compiler/ori_arc/src/reset_reuse/mod.rs` -- `detect_reset_reuse_cfg`, intra-block + cross-block (dominator + refined liveness)
- `compiler/ori_arc/src/expand_reuse/mod.rs` -- `expand_reset_reuse`, fast/slow path, self-set elimination, projection-increment erasure
- `compiler/ori_arc/src/fbip/mod.rs` -- `analyze_fbip`, `FbipReport`, `MissedReuseReason`
- `compiler/ori_arc/src/drop/mod.rs` -- `DropInfo`, `DropKind`, `compute_drop_info`, `collect_drop_infos`
- `compiler/ori_arc/src/graph/mod.rs` -- `DominatorTree` (CHK algorithm), predecessors, postorder
- `compiler/ori_arc/src/lower/mod.rs` -- `lower_function_can`, `ArcIrBuilder`, AST -> ARC IR lowering
- `compiler/ori_llvm/src/codegen/arc_emitter.rs` -- `ArcIrEmitter`, ARC IR -> LLVM IR translation
- `compiler/ori_rt/src/lib.rs` -- `ori_rc_alloc`, `ori_rc_inc` (atomic), `ori_rc_dec` (atomic + `call_drop_fn` nounwind guard), `ori_rc_free`; panic/assert functions use `extern "C-unwind"`; `ORI_PANIC_TRAMPOLINE` is `AtomicPtr<()>`
