---
plan: "dpr_aot-codegen-pipeline_02222026"
title: "Design Pattern Review: AOT Codegen Pipeline Architecture"
status: draft
---

# Design Pattern Review: AOT Codegen Pipeline Architecture

## Ori Today

Ori's AOT codegen pipeline is a three-stage system: typed AST (CanExpr) lowers through `ori_arc` to an explicit basic-block IR (`ArcFunction`/`ArcBlock`/`ArcInstr`/`ArcTerminator`), which then emits LLVM IR via `ArcIrEmitter`. The `ori_arc` crate (~5.9k lines of production code across 14 source modules) is backend-independent -- it depends on `ori_types` for the type pool and `ori_ir` for names and operators, but has zero LLVM dependency. The LLVM-specific emission lives in `ori_llvm/src/codegen/arc_emitter/` (~4k lines including builtins) alongside `FunctionCompiler` (~1k lines for two-pass declare/define orchestration). The runtime (`ori_rt`) provides C-ABI functions for RC operations (`ori_rc_alloc/inc/dec/free`), string/collection management, iterators, and panic handling.

What works well is the separation of concerns. The ARC IR is minimal (~30 instruction variants) and well-designed: `ArcInstr` covers let-bindings, function calls (direct, indirect, partial), projections, constructors, and RC operations; `ArcTerminator` covers returns, jumps, branches, switches, invokes with unwinding, and resume. The pass pipeline (`run_arc_pipeline` in `ori_arc/src/lib.rs`) is canonically ordered and each pass has clear preconditions (e.g., `detect_reset_reuse` debug-asserts that no `Reset`/`Reuse` instructions exist yet). Borrow inference follows Lean 4's monotonic fixed-point algorithm with tail-call preservation. The three-way type classification (`Scalar`/`DefiniteRef`/`PossibleRef` via `ArcClassifier`) is computed once and shared across all passes. Drop function generation (`drop_gen.rs`) handles recursive types via pre-caching and generates specialized cleanup for structs, enums, collections, and maps. The `IsShared`, `Set`, and `SetTag` instructions are fully implemented with correct GEP+store patterns.

What is missing or painful falls into three categories. First, the builtin method codegen in `arc_emitter/builtins/` (1.8k lines across 7 submodules) is a growing collection of hand-coded inline LLVM IR with no shared abstraction -- each type's `clone`, `length`, `iter`, `equals`, `compare`, `hash` is individually authored. Second, closures have an impedance mismatch: `PartialApply` in ARC IR is a logical closure, but `ArcIrEmitter` must generate a wrapper function, closure struct, and environment pointer dynamically -- there is no clean intermediate representation for closure layout. Third, method dispatch through `method_functions: FxHashMap<(Name, Name), (FunctionId, FunctionAbi)>` relies on type name round-tripping that has no compile-time validation; `lookup_method_by_unqualified_name` does a linear scan as a fallback, which is both fragile and O(n).

## Prior Art

### Swift -- SIL as the Universal ARC IR

Swift's SIL (Swift Intermediate Language) embeds retain/release as first-class instructions, making them visible to all optimization passes without a separate lowering phase. The ARC optimizer (`lib/SILOptimizer/ARC/`, 5.7k+ lines across 21 files) uses a three-phase architecture: a bottom-up pass discovers releases via a state lattice (`None -> Decremented -> MightBeUsed -> MightBeDecremented`), a top-down pass discovers retains via the complement lattice, and a matching pass pairs them for elimination. The critical innovation is `RCIdentityFunctionInfo`, which normalizes field projections back to canonical root values -- `x.field` is recognized as the same RC identity as `x`, preventing the optimizer from treating them as independent. The "Known Safe" flag detects guarding pairs: when an outer retain/release bracket proves deallocation cannot occur, inner operations become provably redundant. This separation of discovery from pairing makes the optimizer both conservative (finds all candidates) and selective (only eliminates provably safe cases).

### Lean 4 -- Minimal IR with Type-Indexed Phases

Lean 4's approach is the direct ancestor of Ori's design. The LCNF (Lambda Calculus Normal Form) IR uses type-indexed phase markers that prevent phase violations at compile time -- you cannot accidentally mix polymorphic IR with monomorphized IR. Phase 1 (`ExplicitRC.visitFnBody`) inserts `inc`/`dec` instructions via backward live-variable analysis, using a three-way type classification (`isScalar`/`isPossibleRef`/`isDefiniteRef`) that Ori mirrors precisely. Phase 2 (`BorrowInference`) runs fixed-point iteration marking parameters as owned or borrowed. The crucial design choice is that join points are explicit -- block parameters in the LCNF IR serve as merge points where ownership state converges, and the borrow inference handles them naturally through the fixed-point. Lean's `DerivedValMap` tracks parent-child projection chains (the concept Ori implements as `DerivedOwnership::BorrowedFrom`). The entire system is under 1.2k lines of code for borrow inference, proving that the core algorithm is inherently simple when the IR is well-designed.

### Rust -- FunctionCx Visitor Pattern

Rust's MIR-to-LLVM codegen (`compiler/rustc_codegen_llvm/`) provides structural lessons for the emission layer, even though Rust has no ARC system. The key abstraction is `OperandValue`, a four-variant enum: `Ref` (pointer to memory), `Immediate` (scalar in register), `Pair` (wide pointer or two-word value), and `ZeroSized` (no runtime representation). This enum eliminates an entire class of bugs by making value representation explicit at the type level -- codegen cannot accidentally load a pointer as a scalar or vice versa. Each MIR statement/terminator maps to LLVM instructions through a visitor on `FunctionCx`, which carries all codegen context. The pattern avoids the "god struct" problem by delegating type-specific emission to the `OperandValue` enum rather than match arms in the emitter. Rust also demonstrates that two-pass compilation (declare all, then define all) is the standard for handling forward references and mutual recursion.

## Diagnosis: Why Ori's Pipeline Has Been Painful

### Inherently Hard (Every ARC Compiler Deals With This)

**1. Proving "safe to eliminate" requires global analysis.** Swift needs 5.7k lines of bidirectional dataflow; Lean needs fixed-point iteration; Koka's Perceus needs scope-invariant liveness. There is no shortcut. An `RcInc`/`RcDec` pair can only be eliminated when the optimizer can prove that no intervening operation can observe the reference count. This requires alias analysis (does another pointer alias the same allocation?) and control-flow sensitivity (can the `RcDec` be reached without the `RcInc`?). Ori's current intra-block-only elimination (`rc_elim/mod.rs`, 631 lines) is correct but intentionally limited -- extending to cross-block elimination with join-point convergence is the real complexity.

**2. Type classification must be correct or memory corrupts silently.** Swift, Lean, and Ori all use a cached classifier that walks the type graph. A misclassification (treating a `DefiniteRef` as `Scalar`) means a heap pointer goes untracked and the program eventually segfaults or leaks. Ori's `ArcClassifier` handles recursive types via cycle detection (`classifying: RefCell<FxHashSet<Idx>>`) and resolves type variables through the pool -- this is correct but inherently delicate because the same `Idx` can resolve to different concrete types before vs after monomorphization.

**3. Drop function generation is inherently recursive.** A `struct { a: [str], b: {str: [int]} }` needs a drop function that calls `ori_rc_dec` on each string in the list, on each key and value in the map, etc. Lean uses explicit `del x` instructions; Swift uses witness tables; Ori generates specialized LLVM functions (`_ori_drop$N`). The cycle-safety pattern (cache `FunctionId` before generating the body, like `drop_fn_cache.insert(ty, func_id)` in `drop_gen.rs`) is necessary for recursive types and cannot be simplified away.

**4. Closure representations have no universally clean solution.** Every language compiler struggles with closures: Swift uses heap-allocated context with metadata, Lean boxes closures, Rust monomorphizes away most closures. Ori's `PartialApply` requires allocating a closure struct, generating a wrapper function that unpacks captures, and managing the ABI bridge between the wrapper and the original function. This is inherently multi-step regardless of the IR design.

### Made Harder by Design Gaps (Fixable)

**1. No `OperandValue` enum in the emission layer.** When `ArcIrEmitter::emit_instr` processes a `Construct`, it produces a `ValueId` that could be a stack aggregate, a heap pointer, or a register scalar -- but the type system does not distinguish these. The emitter must re-derive the representation by querying `TypeInfo` at every use site. Rust's `OperandValue` enum solves this by tagging every intermediate value with its representation. Adding an equivalent would eliminate an entire class of "did I load or not?" bugs and simplify the 2.2k-line `arc_emitter/mod.rs`.

```rust
/// How an ARC IR value is represented in LLVM.
enum EmittedValue {
    /// Register scalar: i64, f64, i1, i8, i32.
    Immediate(ValueId),
    /// Pointer to heap-allocated RC'd memory (needs RC management).
    RcPointer(ValueId),
    /// Stack aggregate (struct, tuple, enum by value).
    Aggregate(ValueId),
    /// Fat value: two-word representation ({len, ptr} for str, {fn_ptr, env} for closures).
    Pair { first: ValueId, second: ValueId },
    /// No runtime representation (unit, never).
    ZeroSized,
}
```

**2. Builtin method codegen lacks a dispatch trait.** The 7 submodules in `builtins/` (1.8k lines) hand-code the same patterns: extract receiver, match method name, emit inline IR. There is no shared abstraction for "emit a method on type T with args A returning R." Swift avoids this entirely because methods are just functions. Lean avoids it because builtins are runtime calls. Ori's inline codegen is faster than runtime calls but the current implementation is a maintenance burden. A dispatch trait or strategy enum would centralize the boilerplate.

```rust
/// Strategy for emitting a builtin method as inline LLVM IR.
trait BuiltinMethodCodegen {
    /// Emit inline IR for `receiver.method(args...)`, or None to fall back to runtime.
    fn emit(
        &self,
        emitter: &mut ArcIrEmitter,
        method: &str,
        receiver: ValueId,
        receiver_ty: Idx,
        args: &[ValueId],
    ) -> Option<ValueId>;
}
```

**3. Method dispatch has no compile-time sync enforcement.** Three independent registrations (`TYPECK_BUILTIN_METHODS` in `ori_types`, `eval_builtin_method()` in `ori_eval`, and `try_emit_builtin_method()` in `ori_llvm`) can drift. The `consistency.rs` test in `oric` partially covers this via `TYPECK_METHODS_NOT_IN_EVAL`, but there is no equivalent check for the LLVM backend. Every reference compiler (Swift, Lean, Rust) uses a single source-of-truth for method signatures. Adding a test that iterates `TYPECK_BUILTIN_METHODS` and verifies each has a handler in both `ori_eval` and `ori_llvm` would catch drift at compile time.

**4. Borrow signature lookup silently falls back to conservative.** `FunctionCompiler` receives `annotated_sigs: &FxHashMap<Name, AnnotatedSig>` and looks up each function by `Name`. A lookup miss means the function is compiled with all-Owned parameters (no borrow optimization). There is no warning when this happens. Lean 4 avoids this by running borrow inference as part of the compilation pipeline, not as a separate pre-computation step. Adding a `tracing::warn!` on lookup miss and a `debug_assert!` that all compiled functions have signatures would catch this.

**5. `lookup_method_by_unqualified_name` is a linear scan.** The fallback in `ArcIrEmitter::emit_invoke` (line 547) iterates all entries in `method_functions` to find a match by method name alone. This is O(n) per call site. Building a secondary index `method_name -> Vec<(type_name, FunctionId, FunctionAbi)>` during `compile_impls` would make this O(1).

### Not Actually Hard (Just Incomplete)

**1. Closure environment drop functions.** `compute_closure_env_drop` exists in `ori_arc/src/drop/mod.rs` but the codegen in `drop_gen.rs` already handles `DropKind::ClosureEnv(fields)` identically to `DropKind::Fields(fields)`. The gap is in the `PartialApply` emission path, which needs to register the closure type with the drop function cache. This is plumbing, not design.

**2. Iterator method codegen.** The `builtins/iterator.rs` (526 lines) covers the adapter pipeline (`map`, `filter`, `take`, `skip`, `enumerate`, `zip`, `chain`, `collect`, `fold`, `count`, `find`, `any`, `all`, `for_each`). Adding new iterator methods is mechanical: add the trampoline call in `iterator.rs`, declare the runtime function in `runtime_decl/mod.rs`, implement in `ori_rt/src/iterator/`. The pattern is established.

**3. Inter-function RC elimination.** The current `rc_elim` pass is intra-block only. Extending it to handle single-predecessor cross-block pairs and multi-predecessor join points is a well-understood extension of the existing algorithm (the code already has TODO comments for this). Swift's matching pass provides the template.

## Proposed Improvements

### Core Insight

The reference compilers converge on three principles that Ori should adopt:

1. **Value representation should be type-level, not runtime-checked** (from Rust's `OperandValue`). The emission layer should never need to ask "is this a pointer or a scalar?" -- the IR should tell it.

2. **RC identity should be a first-class concept** (from Swift's `RCIdentityFunctionInfo`). Ori's `DerivedOwnership::BorrowedFrom(root)` already captures the projection chain, but this information is not propagated to the RC elimination pass. Making `RcInc`/`RcDec` operate on canonical root identities would enable more eliminations without new dataflow analysis.

3. **The IR should be the single source of truth for codegen contracts** (from Lean 4's type-indexed phases). Every instruction in ARC IR should specify exactly what the emitter must do, with no type-pool lookups needed during emission. Currently, `emit_instr` queries `TypeInfo` for representation decisions; this should be pre-computed during lowering.

The combined insight: Ori's ARC IR is well-designed for *analysis* (the pass pipeline is clean) but under-specified for *emission* (the emitter must re-derive type information). The fix is to enrich the IR's emission-facing metadata, not to restructure the analysis passes.

### Key Design Choices

1. **Add `EmittedValue` enum to the emission layer** (Rust-inspired). Every `ValueId` produced by the emitter should carry its representation tag. This replaces scattered `TypeInfo` lookups with exhaustive pattern matching at use sites. Implementation: add the enum to `arc_emitter/mod.rs`, change `var_map: Vec<Option<ValueId>>` to `var_map: Vec<Option<EmittedValue>>`, update `emit_instr` to produce tagged values, update consumers (`emit_terminator`, `emit_apply`, `emit_construct`) to destructure them.

2. **Pre-compute value representation in ARC IR lowering** (Lean-inspired). Add a `repr: ValueRepr` field to `ArcInstr::Let`, `Apply`, `Construct`, etc. that classifies the result as `Scalar`, `Pointer`, `Aggregate`, or `FatValue`. The lowerer can compute this from `ArcClassifier` + `Pool` tag during `lower_function_can`. The emitter then uses `repr` instead of querying `TypeInfo`. This keeps the ARC IR backend-independent (the repr classification is about memory layout, not LLVM types).

    ```rust
    /// How a value is represented in memory. Computed during lowering,
    /// consumed by the LLVM emitter. Backend-independent.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum ValueRepr {
        /// Fits in a register (i64, f64, i1, i8, i32). No RC.
        Scalar,
        /// Heap-allocated, reference-counted. `data_ptr - 8` is the refcount.
        RcPointer,
        /// Stack aggregate (struct value, tuple value, enum value).
        /// May contain RC'd fields requiring drop.
        Aggregate,
        /// Two-word fat value: {len/fn_ptr, ptr/env_ptr}.
        /// First word is scalar metadata, second is an RC'd pointer.
        FatValue,
    }
    ```

3. **Add RC identity propagation pass** (Swift-inspired). Between RC insertion and elimination, run a pass that normalizes `RcInc(x.field)` to `RcInc(x)` when `x.field` is a projection from a live value. This extends the existing `DerivedOwnership::BorrowedFrom(root)` tracking into the elimination phase. Implementation: new module `ori_arc/src/rc_identity.rs` (~150 lines) that walks each block and substitutes projection variables with their roots in `RcInc`/`RcDec` instructions.

    ```rust
    /// Normalize RC operations to use canonical root identities.
    ///
    /// After this pass, `RcInc(projected_field)` becomes `RcInc(root_owner)`,
    /// enabling the elimination pass to find more Inc/Dec pairs.
    pub fn propagate_rc_identity(
        func: &mut ArcFunction,
        ownership: &[DerivedOwnership],
    ) {
        for block in &mut func.blocks {
            for instr in &mut block.body {
                match instr {
                    ArcInstr::RcInc { var, .. } | ArcInstr::RcDec { var } => {
                        if let Some(root) = find_root(*var, ownership) {
                            *var = root;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn find_root(var: ArcVarId, ownership: &[DerivedOwnership]) -> Option<ArcVarId> {
        match ownership.get(var.index())? {
            DerivedOwnership::BorrowedFrom(root) if *root != var => Some(*root),
            _ => None,
        }
    }
    ```

4. **Add builtin method dispatch table** (replacing scattered match arms). Create a `BuiltinCodegenTable` that maps `(TypeInfo variant, method_name) -> codegen_fn`. The table is built once during `ArcIrEmitter::new()` from a declarative registration, replacing the 7-submodule match cascade. This is Ori-specific -- Swift and Lean don't need it because their builtins are standard functions.

5. **Add sync test for LLVM builtin coverage** (Ori-specific). Extend the existing `consistency.rs` pattern to verify that every `(type, method)` pair in `TYPECK_BUILTIN_METHODS` either has a handler in `try_emit_builtin_method` or is listed in a known-exclusion set. This prevents the drift that the research identified between type checker, evaluator, and codegen.

6. **Hook borrow inference into Salsa** (Ori-unique opportunity). Neither Swift, Lean, nor Rust has incremental borrow inference. Ori's Salsa-based architecture enables it: when a function body changes but its borrow signature doesn't, callers don't need recompilation. Implementation: make `infer_borrows` a Salsa query, with `AnnotatedSig` as the output. The fixed-point iteration runs within a single query invocation; Salsa's memoization handles cross-invocation caching.

### What Makes Ori's Approach Unique

Ori's dual JIT/AOT execution model creates an opportunity no reference compiler exploits: **the same ARC IR feeds both a conservative JIT path and an optimized AOT path**. The interpreter (`ori_eval`) currently does its own RC management via Rust's reference counting, independent of `ori_arc`. But the ARC IR pipeline could serve both: JIT execution uses the IR after RC insertion but before optimization (guaranteed correct, potentially slower), while AOT uses the fully optimized IR. This means the test suite validates both paths against the same semantics.

The expression-based language design (no `return` keyword) actually simplifies ARC analysis: every block has exactly one exit value, and the last expression's value is always the function's return value. This means the lowerer (`lower_function_can`) can always terminate the current block with `Return { value: result_var }` -- there is no early-return complexity. The downside is that nested expressions create deep block chains for match/if/while, but this is mitigated by the basic-block IR's flat structure.

The capability-based effect system (`uses Http`) has an unexploited interaction with ARC: effects imply that a function performs IO, which means its RC operations cannot be reordered past effect boundaries. When Ori adds effect tracking to the ARC IR (needed for `@fbip` enforcement), the ARC optimizer will need effect-aware reordering rules. No reference compiler has this constraint.

Mandatory tests (`@test tests @target`) mean every function's ARC behavior is exercised by design. This is a testing advantage no other compiler has: when the test suite passes, every user function's RC lifecycle has been validated end-to-end.

### Concrete Types & Interfaces

**1. Value representation in ARC IR (enriched metadata):**

```rust
// In ori_arc/src/ir/mod.rs — add to ArcInstr variants that produce values

/// How the result of this instruction should be represented.
/// Computed during lowering from ArcClassifier + Pool tag.
/// Consumed by the LLVM emitter for correct codegen without type-pool lookups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueRepr {
    Scalar,      // Register: i64, f64, i1, i8, i32
    RcPointer,   // Heap-allocated, RC header at data_ptr - 8
    Aggregate,   // Stack aggregate (struct/tuple/enum value-type)
    FatValue,    // Two-word: {metadata, rc_pointer}
}
```

**2. Emitted value in the LLVM layer:**

```rust
// In ori_llvm/src/codegen/arc_emitter/mod.rs

/// Tagged LLVM value with representation info.
/// Prevents "did I load this already?" class of bugs.
#[derive(Clone, Copy, Debug)]
enum EmittedValue {
    Immediate(ValueId),
    RcPointer(ValueId),
    Aggregate(ValueId),
    Pair { first: ValueId, second: ValueId },
    ZeroSized,
}

impl EmittedValue {
    /// Get the raw ValueId, loading from pointer if needed.
    fn into_value(self, builder: &mut IrBuilder, ty: LLVMTypeId) -> ValueId {
        match self {
            Self::Immediate(v) | Self::RcPointer(v) | Self::Aggregate(v) => v,
            Self::Pair { first, .. } => first, // caller decides which half
            Self::ZeroSized => builder.const_i64(0),
        }
    }

    /// Get the RC data pointer (only valid for RcPointer and FatValue).
    fn rc_data_ptr(self) -> Option<ValueId> {
        match self {
            Self::RcPointer(v) => Some(v),
            Self::Pair { second, .. } => Some(second),
            _ => None,
        }
    }
}
```

**3. Codegen contract for builtin methods:**

```rust
// In ori_llvm/src/codegen/arc_emitter/builtins/mod.rs

/// Registration entry for a builtin method's inline codegen.
struct BuiltinEntry {
    type_tag: TypeInfoTag,  // discriminant of TypeInfo
    method: &'static str,
    emit: fn(&mut ArcIrEmitter, &[ValueId], Idx) -> Option<ValueId>,
}

/// Compiled dispatch table, built once per ArcIrEmitter.
/// O(1) lookup by (type_tag, method_name).
struct BuiltinTable {
    entries: FxHashMap<(TypeInfoTag, &'static str), BuiltinEntry>,
}
```

**4. RC identity map for elimination:**

```rust
// In ori_arc/src/rc_identity.rs

/// Maps each variable to its canonical RC identity root.
/// Built from DerivedOwnership in a single pass.
pub struct RcIdentityMap {
    /// identity[var.index()] = canonical root for RC purposes.
    /// Variables that ARE their own root map to themselves.
    identity: Vec<ArcVarId>,
}

impl RcIdentityMap {
    pub fn build(func: &ArcFunction, ownership: &[DerivedOwnership]) -> Self {
        let mut identity: Vec<ArcVarId> = (0..func.var_types.len())
            .map(|i| ArcVarId::new(i as u32))
            .collect();

        for (i, own) in ownership.iter().enumerate() {
            if let DerivedOwnership::BorrowedFrom(root) = own {
                identity[i] = resolve_to_root(*root, &identity);
            }
        }

        Self { identity }
    }

    pub fn root(&self, var: ArcVarId) -> ArcVarId {
        self.identity[var.index()]
    }
}
```

## Implementation Roadmap

### Phase 1: Complete What's Started (unblock current work)

- [ ] **Add `tracing::warn!` on borrow signature lookup miss** in `FunctionCompiler::define_all`. Currently, a missing `AnnotatedSig` silently uses all-Owned parameters. Add a warning and a `debug_assert!` that all compiled function names exist in `annotated_sigs`. (~10 lines in `function_compiler/mod.rs`)

- [ ] **Build secondary method index for O(1) dispatch.** Replace `lookup_method_by_unqualified_name`'s linear scan with a `FxHashMap<Name, Vec<(Name, FunctionId, FunctionAbi)>>` built during `compile_impls`. (~30 lines in `arc_emitter/mod.rs`)

- [ ] **Add LLVM builtin coverage sync test.** Extend `consistency.rs` to verify every `TYPECK_BUILTIN_METHODS` entry has a codegen handler or is in an explicit exclusion list. (~50 lines in `oric/tests/consistency.rs`)

- [ ] **Complete closure environment drop registration.** Ensure `PartialApply` in `emit_instr` registers the closure's type with the drop function cache so `RcDec` of closures generates correct child-field decrements. (~40 lines in `arc_emitter/mod.rs`)

### Phase 2: Architectural Fixes (reduce ongoing pain)

- [ ] **Add `ValueRepr` to ARC IR instructions.** Add the `repr: ValueRepr` field to value-producing `ArcInstr` variants. Compute it in `lower_function_can` from `ArcClassifier` and `Pool` tag. Update all ARC passes to propagate (but not depend on) `repr`. (~200 lines across `ir/mod.rs`, `lower/expr/mod.rs`, pipeline passes)

- [ ] **Add `EmittedValue` enum to `ArcIrEmitter`.** Replace `var_map: Vec<Option<ValueId>>` with `Vec<Option<EmittedValue>>`. Update `emit_instr` to produce tagged values and all consumers (`emit_terminator`, `emit_apply`, `emit_invoke`) to destructure them. The `ValueRepr` from the ARC IR drives the initial tagging. (~300 lines refactoring in `arc_emitter/mod.rs`)

- [ ] **Extract builtin codegen into dispatch table.** Replace the match cascade in `try_emit_builtin_method` with a `BuiltinTable` built during `ArcIrEmitter::new()`. Each submodule registers its entries declaratively. The existing `emit_*` methods become the function pointers in the table. (~150 lines refactoring across `builtins/mod.rs` and submodules)

- [ ] **Add RC identity propagation pass.** New module `ori_arc/src/rc_identity.rs` that normalizes `RcInc`/`RcDec` variables to their canonical root identities. Insert into the pipeline between `reset_reuse` expansion and `rc_elim`. (~150 lines)

### Phase 3: Optimization Quality (improve generated code)

- [ ] **Extend RC elimination to cross-block pairs.** Add single-predecessor cross-block elimination to `rc_elim/mod.rs`: when an `RcInc` in block A flows into block B (the only predecessor) and B has a matching `RcDec` with no intervening use, eliminate the pair. Follows Swift's matching pass pattern but limited to simple CFG shapes. (~200 lines)

- [ ] **Add "Known Safe" guarding pair detection** (Swift-inspired). When an `RcInc`/`RcDec` pair brackets a region where another `RcInc`/`RcDec` on the same identity occurs, the inner pair is provably safe to eliminate (the outer pair guarantees the object stays alive). This is a new sub-pass in `rc_elim` that runs after basic pair elimination. (~200 lines)

- [ ] **Hook `infer_borrows` into Salsa as an incremental query.** Make `AnnotatedSig` a Salsa output. When a function body changes but its borrow signature is unchanged, callers avoid recompilation. This requires making `ArcFunction` Salsa-friendly (it already derives the needed traits via `#[cfg_attr(feature = "cache", ...)]`). (~100 lines in `oric`)

- [ ] **Add `@fbip` enforcement annotation** (Koka-inspired). When a function is annotated `@fbip`, the existing `analyze_fbip` pass promotes missed reuse from diagnostic to compile error. This turns Ori's FBIP from "informational" to "verified." (~50 lines in `ori_arc/src/fbip/mod.rs`, ~30 lines in diagnostic integration)

## The Bottom Line

**The architecture is fundamentally sound.** The pain is not structural -- it comes from two fixable gaps: the emission layer lacks value-representation typing (causing repeated `TypeInfo` lookups and potential load/store confusion), and the method dispatch system lacks compile-time sync enforcement (causing fragile name-based lookups). The ARC analysis pipeline (`ori_arc`) is well-designed, well-tested, and follows established patterns from Lean 4 and Swift. The improvements proposed here are incremental enrichments to an already-working system, not a redesign. The inherently hard problems (global liveness analysis, type classification correctness, recursive drop generation, closure representation) are handled correctly today -- the remaining work is completeness and polish, not architectural repair.

## References

- Swift ARC optimizer: `~/projects/reference_repos/lang_repos/swift/lib/SILOptimizer/ARC/` (21 files, 5.7k+ lines)
  - `ARCSequenceOpts.cpp` -- top-level pass orchestration
  - `GlobalARCSequenceDataflow.cpp` -- bidirectional dataflow
  - `ARCMatchingSet.h` -- Inc/Dec pair matching
  - `RCIdentityAnalysis.h` -- RC identity normalization
- Lean 4 RC passes: `~/projects/reference_repos/lang_repos/lean4/src/Lean/Compiler/IR/`
  - `RC.lean` -- ExplicitRC insertion
  - `Borrow.lean` -- borrow inference fixed-point
  - `ExpandResetReuse.lean` -- constructor reuse expansion
- Rust MIR codegen: `~/projects/reference_repos/lang_repos/rust/compiler/rustc_codegen_llvm/`
  - `mir/operand.rs` -- `OperandValue` enum
  - `mir/mod.rs` -- `FunctionCx` visitor pattern
- Koka Perceus: `~/projects/reference_repos/lang_repos/koka/src/`
  - `Backend/C/Parc.hs` -- Perceus RC insertion
  - `Core/CheckFBIP.hs` -- FBIP verification
- Ori source files studied:
  - `compiler/ori_arc/src/lib.rs` -- pipeline orchestration (182 lines)
  - `compiler/ori_arc/src/ir/mod.rs` -- ARC IR types (701 lines)
  - `compiler/ori_arc/src/lower/mod.rs` -- AST-to-IR lowering (590 lines)
  - `compiler/ori_arc/src/borrow/mod.rs` -- borrow inference (489 lines)
  - `compiler/ori_arc/src/rc_insert/mod.rs` -- RC insertion (737 lines)
  - `compiler/ori_arc/src/rc_elim/mod.rs` -- RC elimination (631 lines)
  - `compiler/ori_arc/src/reset_reuse/mod.rs` -- reset/reuse detection (365 lines)
  - `compiler/ori_arc/src/expand_reuse/mod.rs` -- reuse expansion (544 lines)
  - `compiler/ori_arc/src/classify/mod.rs` -- type classification (246 lines)
  - `compiler/ori_arc/src/drop/mod.rs` -- drop descriptors (420 lines)
  - `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` -- LLVM emission (2,223 lines)
  - `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` -- drop function generation (545 lines)
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` -- builtin dispatch (162 lines)
  - `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` -- two-pass compilation
  - `compiler/ori_llvm/src/codegen/type_info/mod.rs` -- TypeInfo enum
  - `compiler/ori_llvm/src/codegen/runtime_decl/mod.rs` -- runtime declarations
  - `compiler/ori_rt/src/lib.rs` -- runtime library
