---
section: "06"
title: ARC IR & Borrow Inference
status: in-progress
goal: Lower typed AST to an intermediate ARC IR with explicit control flow, then infer which function parameters can be borrowed (no RC needed) vs owned (RC needed)
sections:
  - id: "06.0"
    title: ARC IR Definition
    status: in-progress
  - id: "06.1"
    title: Ownership Model
    status: partial
  - id: "06.2"
    title: Iterative Borrow Inference Algorithm
    status: not-started
  - id: "06.3"
    title: Integration with Function Signatures
    status: not-started
---

# Section 06: ARC IR & Borrow Inference

**Status:** In Progress — 06.0 AST→ARC IR lowering complete (all expression kinds, control flow, patterns, closures). 06.2 borrow inference not started.
**Goal:** Lower typed AST to an intermediate ARC IR with basic blocks and explicit control flow, then infer which function parameters are "borrowed" (the caller retains ownership, callee doesn't need to inc/dec) vs "owned" (ownership transfers to callee). Borrowed parameters eliminate entire classes of RC operations.

**Crate:** `ori_arc` (no LLVM dependency).

**Reference compilers:**
- **Lean 4** `src/Lean/Compiler/IR/Borrow.lean` -- Iterative refinement: start all borrowed, mark consumed as owned until fixed point. LCNF basic-block IR.
- **Lean 4** `src/Lean/Compiler/LCNF/` -- LCNF (Lambda Calculus Normal Form) with explicit join points and basic blocks
- **Koka** `src/Core/Borrowed.hs` -- `ParamInfo` enum (Own/Borrow) from type signatures + usage analysis. Core IR with explicit control flow.
- **Swift** `include/swift/SIL/SILValue.h` -- OwnershipKind lattice with Guaranteed (borrowed) and Owned. SIL has basic blocks.

---

## 06.0 ARC IR Definition

**Error handling:** ARC pipeline passes report errors via `Vec<ArcProblem>` accumulation. `ArcProblem` is defined in `ori_arc` (not in `oric`) to avoid a circular dependency — `ori_arc` cannot depend on `oric`. Each pass function accepts `&mut Vec<ArcProblem>`. Non-fatal errors are accumulated and lowering continues with best-effort output. This allows the pipeline to produce partial results and report multiple diagnostics rather than aborting on the first error. At the `oric` boundary, `ArcProblem` values are converted to `CodegenProblem` (Section 15) for unified diagnostic reporting.

**Problem:** The typed AST (expression tree) has implicit control flow — `if`/`match`/`loop`/`for` are nested expressions, not basic blocks with explicit branches. Liveness analysis, borrow inference, and RC insertion all require reasoning about control flow paths. Doing this directly on the expression tree is fragile and error-prone (every algorithm must reinvent control flow traversal).

**Solution:** Lower the typed AST to an intermediate **ARC IR** with basic blocks and explicit control flow BEFORE running any ARC analysis. This follows the proven approach from:
- **Lean 4 LCNF** — Lambda Calculus Normal Form with join points
- **Koka Core** — Core IR with explicit cases and branches
- **Swift SIL** — Structured IL with basic blocks and ownership semantics

The ARC IR is the representation that all ARC analysis passes operate on (borrow inference, RC insertion, RC elimination, reuse analysis).

```rust
/// ARC IR: basic-block form for ARC analysis.
///
/// Lives in `ori_arc`. No LLVM dependency.
/// Lowered from the typed AST before ARC passes run.

/// Opaque variable ID within an ARC function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArcVarId(u32);

/// Opaque block ID within an ARC function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArcBlockId(u32);

/// A function parameter with ownership annotation.
pub struct ArcParam {
    pub var: ArcVarId,
    pub ty: Idx,
    pub ownership: Ownership,  // Initially Borrowed, refined by borrow inference
}

/// A value expression (right-hand side of a Let binding).
pub enum ArcValue {
    Var(ArcVarId),
    Literal(LitValue),
    PrimOp { op: PrimOp, args: Vec<ArcVarId> },
}

/// **Note:** `PrimOp` and `LitValue` (used in `ArcValue`) are defined during implementation.
/// `PrimOp` mirrors `BinaryOp`/`UnaryOp` from `ori_ir` — primitive operations that don't
/// involve allocation or ownership transfer. `LitValue` mirrors literal `ExprKind` variants
/// (Int, Float, Bool, Char, etc.) — compile-time constant values.

/// What kind of constructor is being applied.
pub enum CtorKind {
    Struct(Name),
    EnumVariant { enum_name: Name, variant: u32 },
    Tuple,
    ListLiteral,
    MapLiteral,
    SetLiteral,
    Closure { func: Name },
}

/// A function in ARC IR form.
pub struct ArcFunction {
    pub name: Name,
    pub params: Vec<ArcParam>,
    pub return_type: Idx,
    pub blocks: Vec<ArcBlock>,
    pub entry: ArcBlockId,
    /// Variable type information for ARC analysis.
    /// Indexed by ArcVarId.0 — dense Vec is faster than HashMap for
    /// sequential IDs. All ArcVarIds within a function are assigned
    /// contiguously starting from 0.
    pub var_types: Vec<Idx>,
    /// Source span information for debug info preservation (Section 13).
    /// Indexed by (block_id, instruction_index). Stored as a Vec of
    /// per-block span Vecs, indexed by ArcBlockId.0 then instruction index.
    /// Kept as a side table rather than a field on ArcInstr to keep
    /// the instruction enum small and cache-friendly.
    pub spans: Vec<Vec<Option<Span>>>,
}

impl ArcFunction {
    /// Look up the type of a variable.
    pub fn var_type(&self, var: ArcVarId) -> Idx {
        self.var_types[var.0 as usize]
    }
}

/// A basic block with a sequence of instructions and a terminator.
pub struct ArcBlock {
    pub id: ArcBlockId,
    pub params: Vec<(ArcVarId, Idx)>,  // Block parameters (phi-like join semantics)
    pub body: Vec<ArcInstr>,
    pub terminator: ArcTerminator,
}

/// An instruction in ARC IR (no control flow — that's in terminators).
pub enum ArcInstr {
    /// Bind a variable to an expression result.
    Let { dst: ArcVarId, ty: Idx, value: ArcValue },
    /// Apply a known function (direct call).
    Apply { dst: ArcVarId, ty: Idx, func: Name, args: Vec<ArcVarId> },
    /// Apply an unknown function (indirect call through closure).
    /// Both closure and all args must be Owned — the callee is unknown,
    /// so we cannot know its borrow signature.
    ApplyIndirect { dst: ArcVarId, ty: Idx, closure: ArcVarId, args: Vec<ArcVarId> },
    /// Create a partial application (closure capturing some arguments).
    /// Result is Owned. All captured args become Owned (stored in env struct).
    PartialApply { dst: ArcVarId, ty: Idx, func: Name, args: Vec<ArcVarId> },
    /// Project a field from a struct/tuple.
    Project { dst: ArcVarId, ty: Idx, value: ArcVarId, field: u32 },
    /// Construct a value (struct, enum variant, tuple, list, etc.)
    Construct { dst: ArcVarId, ty: Idx, ctor: CtorKind, args: Vec<ArcVarId> },
    /// RC operations (inserted by Section 07, not present after initial lowering)
    RcInc { var: ArcVarId, count: u32 },
    RcDec { var: ArcVarId },
    /// Test whether a value is shared (refcount > 1).
    /// Inserted by Section 09 during reset/reuse expansion.
    IsShared { dst: ArcVarId, var: ArcVarId },
    /// In-place field mutation on a uniquely-owned constructor.
    /// Inserted by Section 09 during reset/reuse expansion (fast path).
    Set { base: ArcVarId, field: u32, value: ArcVarId },
    /// Update the tag/discriminant on a uniquely-owned enum value.
    /// Inserted by Section 09 during reset/reuse expansion (fast path)
    /// when the new constructor is a different variant of the same enum type.
    SetTag { base: ArcVarId, tag: u64 },
    /// Intermediate: prepare a value for potential reuse.
    /// Inserted by Section 07 when `dec x` + `Construct same_type` is detected.
    /// Expanded by Section 09 into IsShared + Branch (fast/slow paths).
    /// Does NOT exist in the final ARC IR after Section 09 runs.
    Reset { var: ArcVarId, token: ArcVarId },
    /// Intermediate: reuse a Reset'd allocation for a new constructor.
    /// Inserted by Section 07 alongside Reset.
    /// Expanded by Section 09 into Set (fast) or Construct (slow) paths.
    /// Does NOT exist in the final ARC IR after Section 09 runs.
    Reuse { token: ArcVarId, dst: ArcVarId, ty: Idx, ctor: CtorKind, args: Vec<ArcVarId> },
}

/// A block terminator — explicit control flow.
pub enum ArcTerminator {
    /// Return a value from the function.
    Return { value: ArcVarId },
    /// Unconditional branch to another block.
    Jump { target: ArcBlockId, args: Vec<ArcVarId> },
    /// Conditional branch.
    Branch { cond: ArcVarId, then_block: ArcBlockId, else_block: ArcBlockId },
    /// Multi-way branch (match/switch on tag).
    Switch { scrutinee: ArcVarId, cases: Vec<(u64, ArcBlockId)>, default: ArcBlockId },
    /// Invoke a potentially-panicking function with an unwind destination.
    /// Used for panic cleanup (Section 07.5). At LLVM emission, this becomes
    /// an `invoke` instruction with a landing pad.
    /// Post-0.1-alpha: panic cleanup. Not generated by any pass in 0.1-alpha.
    /// Match arms should use unreachable!().
    Invoke {
        dst: ArcVarId,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        normal: ArcBlockId,   // Continue here on success
        unwind: ArcBlockId,   // Jump here on panic (cleanup block)
    },
    /// Resume unwinding after cleanup. Used in cleanup blocks (Section 07.5).
    /// At LLVM emission, this becomes a `resume` instruction.
    /// Post-0.1-alpha: panic cleanup. Not generated by any pass in 0.1-alpha.
    /// Match arms should use unreachable!().
    Resume,
    /// Unreachable (after panic, never type).
    Unreachable,
}
```

**Block parameters** serve as join semantics (like phi nodes). When two branches merge, the target block declares parameters that receive values from each predecessor. This avoids mutable variables in the IR and makes dataflow explicit.

**AST-to-ARC-IR lowering complexity note:** The general AST-to-ARC-IR lowering algorithm is the most complex piece of this section and will be specified in detail during implementation. This section defines the target IR (ArcFunction, ArcBlock, ArcInstr, ArcTerminator), the lowering rules for each expression kind (below), and the `ArcIrBuilder` API (see below). Section 10 provides the pattern matching lowering specifically. The full lowering algorithm must handle: expression flattening into SSA-style `Let` bindings, control flow desugaring (if/match/loop/for into basic blocks), scope management, closure capture analysis, and break/continue target resolution.

**Pure SSA form (Lean 4 LCNF style):** The ARC IR is in pure SSA form with block parameters. There are NO mutable variables in the ARC IR. All values are bound exactly once via `Let` instructions or block parameters. Mutable variables from the AST (`let mut x = a; x = b; use(x)`) are lowered into SSA form: each assignment creates a new binding, and control flow merges use block parameters (phi-like). This follows Lean 4's LCNF representation where every variable is immutable and join points use block parameters.

**Lowering from AST to ARC IR:**
- `if cond then a else b` → branch to then_block/else_block, both jump to merge_block with result as block parameter
- `match` → switch on tag, each arm is a block, all jump to merge_block
- `loop`/`for` → loop header block with back-edge from loop body
- `let x = expr; rest` → `Let` instruction followed by rest
- `let mut x = a; x = b; use(x)` → SSA: `Let { dst: x_0, value: a }; Let { dst: x_1, value: b }; use(x_1)`. Each assignment creates a new ArcVarId. When control flow merges, block parameters join the different versions.
- `expr?` (try/propagate) → desugared to match before ARC IR lowering: `match expr { Ok(v) -> v, Err(e) -> { cleanup; return Err(e) } }`. The cleanup in the Err branch is handled by normal RC insertion (Section 07) — all live RC'd variables at the return point get Dec'd.
- Nested expressions are flattened into sequences of `Let` bindings
- **Unsupported expression kinds** (`Await`, `WithCapability`, `FunctionSeq`, `FunctionExp`): These produce a diagnostic error during ARC IR lowering in 0.1-alpha. They are not supported in the initial implementation and will be addressed in future versions.

**`ArcIrBuilder` API:** `ArcIrBuilder` is the builder API for constructing ARC IR during AST lowering. It provides methods for: `new_block()`, `emit_project()`, `emit_switch()`, `emit_branch()`, `emit_jump()`, `bind_variable()`, `compile_expr()`, `emit_unreachable()`, `emit_project_tag()`, `emit_project_named()`, `emit_list_index()`, and `position_at()`. Full API defined during implementation. Used by Section 10 (decision tree emission) and the general AST-to-ARC-IR lowering pass.

- [x] Define `ArcVarId`, `ArcBlockId` newtypes
- [x] Define `ArcParam`, `ArcValue`, `CtorKind` types
- [x] Define `ArcFunction`, `ArcBlock`, `ArcInstr`, `ArcTerminator` types
- [x] Define `Reset`/`Reuse` intermediate variants on `ArcInstr` (expanded by Section 09)
- [x] Define `IsShared`/`Set` variants on `ArcInstr` (produced by Section 09 expansion)
- [x] Implement `ApplyIndirect` for indirect calls through closures
- [x] Implement `PartialApply` for partial application / closure creation
- [x] Implement `Invoke` terminator for potentially-panicking calls (Section 07.5) *(post-0.1-alpha)*
- [x] Implement AST → ARC IR lowering for all expression kinds
- [x] Handle loops (loop header block, back-edge, break as jump to exit block)
- [x] Handle closures (captured variables become explicit parameters)
- [x] Define `ArcIrBuilder` with block creation, instruction emission, and variable binding methods
- [x] Populate `var_types` during AST-to-ARC-IR lowering
- [x] Populate `spans` side table during lowering for debug info preservation
- [x] Define `PrimOp` enum mirroring BinaryOp/UnaryOp from ori_ir
- [x] Define `LitValue` enum mirroring literal ExprKind variants
- [x] Implement `SetTag` instruction for enum variant tag updates during reuse
- [ ] Add `#[cfg_attr(feature = "cache", derive(Serialize, Deserialize))]` to all ARC IR types for incremental compilation cache (Section 12.3)
- [x] Implement Assign lowering into pure SSA form (new binding per assignment, block parameters at merges)
- [x] Implement Try/? desugaring to match (Ok/Err) before ARC IR lowering
- [x] Emit diagnostic error for unsupported expression kinds (Await, WithCapability, FunctionSeq, FunctionExp)
- [x] Test: round-trip simple programs through ARC IR and verify structure
- [x] Test: mutable variable assignment correctly lowers to SSA with block parameters

---

## 06.1 Ownership Model

```rust
/// Parameter ownership annotation.
///
/// Determines whether a function parameter needs reference counting
/// at the call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ownership {
    /// Caller retains ownership. Callee receives a borrowed reference.
    /// No inc before call, no dec after call.
    /// Safe when: parameter is only read, not stored or returned.
    Borrowed,

    /// Ownership transfers to callee. Callee is responsible for the value.
    /// Caller incs before call (if value is used after).
    /// Callee decs when done (if not returning the value).
    Owned,
}

/// Function parameter with ownership annotation.
pub struct AnnotatedParam {
    pub name: Name,
    pub ty: Idx,
    pub ownership: Ownership,
}

/// Function signature with ownership annotations.
///
/// Return values are always Owned (callee transfers ownership to caller).
/// This follows the Lean 4 and Koka convention. No analysis is needed for
/// return ownership — it is a fixed invariant of the calling convention.
pub struct AnnotatedSig {
    pub params: Vec<AnnotatedParam>,
    pub return_type: Idx,
    // return_ownership is not a field: return values are always Owned by convention.
    // The callee produces a value with refcount 1 and the caller takes ownership.
}
```

**Rules from Lean 4:**
- A parameter is borrowed if it is only *read* (not stored, not returned, not passed to an owning position).
- A parameter is owned if it is consumed: stored into a data structure, returned, or passed to another function in an owning position.

- [x] Define `Ownership` enum
- [x] Define `AnnotatedParam` and `AnnotatedSig`
- [ ] Integrate with ArcClass: Scalar parameters always effectively "borrowed" (no RC)

## 06.2 Iterative Borrow Inference Algorithm

**Operates on ARC IR** (Section 06.0), not the expression tree. The basic-block form makes parameter use analysis straightforward — just scan instructions in each block.

**Algorithm (from Lean 4):**

```
1. Initialize: All parameters with ArcClass != Scalar start as Borrowed.
   Scalar parameters (ArcClass::Scalar) are never tracked for ownership — they
   have no reference count and never need RC operations.
2. For each ARC function's blocks:
   a. Scan all instructions for parameter uses
   b. If a parameter is:
      - Used in Return terminator → mark Owned
      - Used in Construct instruction (stored into data structure) → mark Owned
      - Passed to Apply in an Owned position → mark Owned
      - Passed to ApplyIndirect (any position) → mark Owned
        (unknown callee — cannot know borrow signature)
      - Passed to PartialApply (any position) → mark Owned
        (captured into closure env struct)
      - Only used in Project/comparison → remains Borrowed
        (BUT see bidirectional projection propagation below)
3. Repeat step 2 until no parameter changes (fixed point)
```

The fixed point is needed because function A's parameter ownership depends on function B's, which depends on A's (mutual recursion).

**Bidirectional ownership propagation for projections** (from Lean 4's `collectExpr` for `.proj`):

When a `Project` instruction extracts a field from a value, and the projected result becomes Owned (because it is returned, stored, or passed to an owning position), the **source** of the projection must also become Owned. Without this rule, a borrowed struct could have its field extracted and returned — but the struct might be freed by the caller before the field is used, causing use-after-free.

```
Rule: If Project { dst, value, .. } and dst is Owned → mark value as Owned
```

This propagation is transitive: if `a.x.y` is Owned, then `a.x` and `a` must both be Owned. The fixed-point iteration handles this naturally — each round propagates one level, and iteration continues until stable.

**Tail call preservation** (from Lean 4's `preserveTailCall`):

When a function tail-calls itself (or another function) and passes a parameter that is currently Borrowed but the callee expects it Owned, the parameter must be promoted to Owned. Without this, the RC insertion pass would insert a Dec *after* the tail call, which would break the tail call optimization (the Dec would require a stack frame to exist after the call returns).

```
Rule: If a tail-call passes parameter P to an Owned position,
      and P is currently Borrowed → mark P as Owned
```

This is checked during the terminator scan: if a `Return` is preceded by an `Apply` whose result is the returned value (tail position), check whether any arguments need ownership promotion.

```rust
/// Infer borrow annotations for all functions in a module.
///
/// Operates on ARC IR (basic-block form). Each function's blocks
/// are scanned for parameter usage patterns.
pub fn infer_borrows(
    functions: &[ArcFunction],
    pool: &Pool,
) -> FxHashMap<Name, AnnotatedSig> {
    let mut sigs = initialize_all_borrowed(functions, pool);
    let mut changed = true;

    while changed {
        changed = false;
        for func in functions {
            if update_ownership(func, pool, &mut sigs) {
                changed = true;
            }
        }
    }

    sigs
}

fn update_ownership(
    func: &ArcFunction,
    pool: &Pool,
    sigs: &mut FxHashMap<Name, AnnotatedSig>,
) -> bool {
    let mut changed = false;
    // Note: pseudocode. Actual implementation must clone sig or use index-based
    // access to avoid simultaneous &/&mut borrow of sigs (the `mark_owned` calls
    // below take &mut sigs while sig is an & into sigs).
    let sig = &sigs[&func.name];

    // Walk all blocks and instructions, check each parameter use
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Apply { args, func: callee, .. } => {
                    for (i, &arg) in args.iter().enumerate() {
                        // Map ArcVarId to parameter index: find arg's position
                        // in func.params (ArcVarId is NOT the same as param index).
                        let param_idx = func.params.iter()
                            .position(|p| p.var == arg);
                        if let Some(pidx) = param_idx {
                            if sig.params[pidx].ownership == Ownership::Borrowed {
                                if sigs[callee].params[i].ownership == Ownership::Owned {
                                    mark_owned(sigs, &func.name, arg);
                                    changed = true;
                                }
                            }
                        }
                    }
                }
                ArcInstr::ApplyIndirect { closure, args, .. } => {
                    // Unknown callee — all arguments and closure must be Owned
                    if is_param(*closure, func) {
                        let pidx = func.params.iter().position(|p| p.var == *closure).unwrap();
                        if sig.params[pidx].ownership == Ownership::Borrowed {
                            mark_owned(sigs, &func.name, *closure);
                            changed = true;
                        }
                    }
                    for &arg in args {
                        if is_param(arg, func) {
                            let pidx = func.params.iter().position(|p| p.var == arg).unwrap();
                            if sig.params[pidx].ownership == Ownership::Borrowed {
                                mark_owned(sigs, &func.name, arg);
                                changed = true;
                            }
                        }
                    }
                }
                ArcInstr::PartialApply { args, .. } => {
                    // All captured args stored in closure env — must be Owned
                    for &arg in args {
                        if is_param(arg, func) {
                            let pidx = func.params.iter().position(|p| p.var == arg).unwrap();
                            if sig.params[pidx].ownership == Ownership::Borrowed {
                                mark_owned(sigs, &func.name, arg);
                                changed = true;
                            }
                        }
                    }
                }
                ArcInstr::Construct { args, .. } => {
                    for &arg in args {
                        if is_param(arg, func) {
                            let pidx = func.params.iter().position(|p| p.var == arg).unwrap();
                            if sig.params[pidx].ownership == Ownership::Borrowed {
                                mark_owned(sigs, &func.name, arg);
                                changed = true;
                            }
                        }
                    }
                }
                ArcInstr::Project { dst, value, .. } => {
                    // Bidirectional propagation: if dst becomes Owned,
                    // source must also be Owned (see projection ownership rule)
                    //
                    // is_owned_var(v): Returns true if the variable is
                    // (a) a function parameter with Ownership::Owned, or
                    // (b) any non-parameter variable (all local variables
                    //     are implicitly Owned — they hold ownership of
                    //     their values from the point of definition).
                    if is_owned_var(*dst, func, sigs) {
                        if is_param(*value, func) {
                            let pidx = func.params.iter().position(|p| p.var == *value).unwrap();
                            if sig.params[pidx].ownership == Ownership::Borrowed {
                                mark_owned(sigs, &func.name, *value);
                                changed = true;
                            }
                        }
                    }
                }
                ArcInstr::Let { .. } => {
                    // A Let binding is a read-only alias. In particular,
                    // Let { dst: y, value: Var(x) } is a semantic alias —
                    // no RC operation is implied by the Let itself. RC
                    // insertion (Section 07) determines inc/dec based on
                    // liveness of y and x independently.
                }
                _ => {}
            }
        }
        // Check terminator for Return uses
        if let ArcTerminator::Return { value } = &block.terminator {
            if is_param(*value, func) {
                let pidx = func.params.iter().position(|p| p.var == *value).unwrap();
                if sig.params[pidx].ownership == Ownership::Borrowed {
                    mark_owned(sigs, &func.name, *value);
                    changed = true;
                }
            }
        }
    }

    changed
}
```

- [ ] Implement `initialize_all_borrowed()` -- skip Scalar parameters
- [ ] Implement instruction scanning over ARC IR blocks
- [ ] Implement `update_ownership()` with use-kind analysis
- [ ] Implement bidirectional projection ownership propagation
- [ ] Implement `ApplyIndirect`/`PartialApply` ownership rules (all args Owned)
- [ ] Implement tail call preservation (promote Borrowed→Owned when needed for TCO)
- [ ] Implement fixed-point iteration
- [ ] Verify that fixed-point iteration correctly handles recursive and mutually recursive functions (monotonic Borrowed→Owned convergence)
- [ ] Handle closures: captured variables are always Owned (stored in env struct)
- [ ] Test: pure functions should have all parameters Borrowed
- [ ] Test: projection ownership propagation (return field from param → param becomes Owned)

**Closure capture RC strategy** (Lean 4 / Koka approach):

Closure environments are RC'd **as a unit**. When a closure captures variables `a`, `b`, `c`, the lowering creates an env struct `{ a, b, c }` which is a single RC-managed heap object. The closure value on the stack is `{ fn_ptr, env_ptr }` where `env_ptr` points to the RC'd env struct.

- `PartialApply` creates the env struct, increments each captured variable (they are being stored into the struct), and returns the closure value
- Dropping a closure = Dec the env refcount; if it reaches zero, run the specialized drop function for the env struct (which Decs each captured variable inside it)
- This avoids per-capture RC tracking at the call site — one Inc/Dec per closure, not one per captured variable
- The specialized drop function for the env struct is generated per closure type (Section 07.4)

## 06.3 Integration with Function Signatures

```rust
/// After borrow inference, annotate function signatures for codegen.
///
/// Codegen uses these annotations to decide:
/// - Whether to emit inc before a function call
/// - Whether to emit dec after a function call
/// - Whether the callee needs to dec its parameters
pub fn annotate_module(
    module: &Module,
    borrow_info: &FxHashMap<Name, AnnotatedSig>,
) -> AnnotatedModule {
    // Attach ownership info to each function
    // This flows into Section 07 (RC insertion)
}
```

### Ownership to ParamPassing Bridge

Borrow inference produces `Ownership::Borrowed` / `Ownership::Owned` annotations per parameter. These flow into Section 04's `ParamPassing` enum during LLVM IR emission:

| Ownership | ArcClass | ParamPassing |
|-----------|----------|-------------|
| Borrowed | PossibleRef / DefiniteRef | `ParamPassing::Reference` — callee receives a pointer; no RC at call site |
| Owned | PossibleRef / DefiniteRef | `ParamPassing::Direct` or `ParamPassing::Indirect` (by size, per Section 04.2) — normal RC semantics |
| (any) | Scalar | `ParamPassing::Direct` — no RC regardless of ownership annotation |

The mapping is applied in `ori_llvm` when building LLVM function signatures. Section 04's `compute_param_passing()` takes the `Ownership` annotation (from borrow inference) and the `ArcClass` (from Section 05) as inputs. When `Ownership::Borrowed` and `ArcClass != Scalar`, it produces `ParamPassing::Reference` — the parameter is passed as a pointer, and the caller retains ownership (no inc before call, no dec after call). This is the mechanism by which borrow inference eliminates RC operations at call sites.

- [ ] Implement module annotation with borrow results
- [ ] Wire borrow info into FunctionSig (Section 04)
- [ ] Implement `compute_param_passing()` bridge: Ownership::Borrowed + non-Scalar → ParamPassing::Reference
- [ ] Ensure borrow info persists through codegen pipeline

---

**Exit Criteria:** Every function parameter has a correct Borrowed/Owned annotation. Pure read-only parameters are Borrowed. Parameters that escape are Owned.
