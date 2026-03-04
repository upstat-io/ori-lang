---
section: "01"
title: "Representation IR & Decision Framework"
status: not-started
goal: "Create the ReprPlan data structure that records all narrowing decisions, integrated into the compilation pipeline between type checking and LLVM codegen"
inspired_by:
  - "Lean4 LCNF phase separation (src/Lean/Compiler/LCNF/)"
  - "Zig InternPool layout interning (src/InternPool.zig)"
  - "Roc STLayoutInterner (crates/compiler/mono/src/layout/intern.rs)"
depends_on: []
sections:
  - id: "01.1"
    title: "MachineRepr Enum & ReprPlan Data Structure"
    status: not-started
  - id: "01.2"
    title: "ReprDecision Tracking"
    status: not-started
  - id: "01.3"
    title: "Pipeline Integration Point"
    status: not-started
  - id: "01.4"
    title: "ReprPlan Query Interface"
    status: not-started
  - id: "01.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Representation IR & Decision Framework

**Context:** Today, `ori_llvm::codegen::type_info::info.rs` hardcodes the mapping from `Tag` to LLVM type (e.g., `Tag::Int → i64`). This is scattered across `storage_type()`, `size()`, `alignment()`, and `is_trivial()`. To support narrowing, we need a centralized decision document that multiple analysis passes can populate and codegen can read.

**Reference implementations:**
- **Lean4** `src/Lean/Compiler/LCNF/Types.lean`: Phase-separated IR where semantic types and machine types are distinct data structures
- **Zig** `src/InternPool.zig`: Layout information interned alongside types — each type has pre-computed size/alignment
- **Roc** `crates/compiler/mono/src/layout/intern.rs`: `STLayoutInterner` maps type variables to concrete layouts after monomorphization

**Depends on:** Nothing — this is the foundation.

---

## 01.1 MachineRepr Enum & ReprPlan Data Structure

**File(s):** `compiler/ori_repr/src/lib.rs` (NEW crate), `compiler/ori_repr/src/repr.rs`

The `MachineRepr` enum captures the physical representation chosen for each type. It must be rich enough to express all optimizations in §02-§11 but simple enough that codegen can pattern-match exhaustively.

- [ ] Create new crate `ori_repr` with `Cargo.toml` entry
  - Dependencies: `ori_types` (for Pool, Idx, Tag), `ori_ir` (for function identifiers)
  - No dependency on `ori_llvm` — this is backend-independent

- [ ] Define `MachineRepr` enum:
  ```rust
  /// The physical representation of a type in generated code.
  /// Every Idx in the Pool maps to exactly one MachineRepr.
  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
  pub enum MachineRepr {
      /// Fixed-width integer (narrowed from semantic i64)
      Int { width: IntWidth, signed: bool },
      /// Fixed-width float (narrowed from semantic f64)
      Float { width: FloatWidth },
      /// Boolean (always i1)
      Bool,
      /// Unit (zero-sized in memory, i64(0) as value)
      Unit,
      /// Never (uninhabited)
      Never,
      /// Struct with optimized field layout
      Struct(StructRepr),
      /// Enum with optimized discriminant and payload
      Enum(EnumRepr),
      /// Tuple (treated as anonymous struct)
      Tuple(TupleRepr),
      /// Heap-allocated reference-counted value
      RcPointer(RcRepr),
      /// Fat pointer (ptr + metadata)
      FatPointer(FatRepr),
      /// Function pointer (fn ptr + optional env ptr)
      Closure(ClosureRepr),
      /// Stack-promoted value (was heap, promoted by escape analysis)
      StackPromoted { inner: Box<MachineRepr>, original_rc: bool },
      /// Opaque pointer (iterator, channel — runtime-managed)
      OpaquePtr,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum IntWidth { I8, I16, I32, I64 }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum FloatWidth { F32, F64 }
  ```

- [ ] Define `StructRepr`:
  ```rust
  pub struct StructRepr {
      /// Fields in optimized memory order (may differ from declaration order)
      pub fields: Vec<FieldRepr>,
      /// Total size in bytes (including padding)
      pub size: u32,
      /// Alignment requirement
      pub align: u32,
      /// Whether all fields are trivial (no RC needed)
      pub trivial: bool,
  }

  pub struct FieldRepr {
      /// Original field index (declaration order)
      pub original_index: u32,
      /// Offset in bytes from struct start
      pub offset: u32,
      /// Machine representation of this field
      pub repr: MachineRepr,
  }
  ```

- [ ] Define `EnumRepr`:
  ```rust
  pub struct EnumRepr {
      /// Discriminant representation
      pub tag: EnumTag,
      /// Per-variant payload representations
      pub variants: Vec<VariantRepr>,
      /// Total size including tag and padding
      pub size: u32,
      pub align: u32,
  }

  pub enum EnumTag {
      /// Explicit tag field at offset 0
      Explicit { width: IntWidth },
      /// Niche — tag stored in invalid bit pattern of a field
      Niche { field_index: u32, niche_value: u64 },
      /// No tag needed (single inhabited variant, e.g. newtype)
      None,
  }
  ```

- [ ] Define `RcRepr`:
  ```rust
  pub struct RcRepr {
      /// Width of the reference count header
      pub rc_width: IntWidth,
      /// Whether RC operations are atomic
      pub atomic: bool,
      /// The inner data representation
      pub inner: Box<MachineRepr>,
      /// Whether this is stack-promotable (escape analysis)
      pub stack_promotable: bool,
  }
  ```

---

## 01.2 ReprDecision Tracking

**File(s):** `compiler/ori_repr/src/plan.rs`

Each narrowing decision should be recorded with its justification, so that:
1. Debug output can explain why a type was narrowed
2. Bugs can be traced to the specific analysis that made the decision
3. Later passes can query upstream decisions

- [ ] Define `ReprDecision`:
  ```rust
  #[derive(Debug, Clone)]
  pub struct ReprDecision {
      /// Which analysis pass made this decision
      pub source: DecisionSource,
      /// The semantic type this applies to
      pub type_idx: Idx,
      /// The chosen machine representation
      pub repr: MachineRepr,
      /// Why this representation was chosen (for tracing)
      pub reason: DecisionReason,
  }

  #[derive(Debug, Clone, Copy)]
  pub enum DecisionSource {
      /// §02: Transitive triviality analysis
      Triviality,
      /// §03/§04: Value range → integer narrowing
      IntegerNarrowing,
      /// §03/§05: Precision analysis → float narrowing
      FloatNarrowing,
      /// §06: Struct field reordering
      StructLayout,
      /// §07: Enum niche/discriminant
      EnumRepr,
      /// §08: Escape analysis
      EscapeAnalysis,
      /// §09: ARC header compression
      ArcHeader,
      /// §10: Thread-local ARC
      ThreadLocal,
      /// §11: Collection specialization
      CollectionSpec,
      /// Default: canonical representation (no optimization)
      Canonical,
  }

  #[derive(Debug, Clone)]
  pub enum DecisionReason {
      /// Type is canonically this width (no narrowing applied)
      Canonical,
      /// Value range fits in narrower type
      RangeFits { range: ValueRange, min_width: IntWidth },
      /// All fields are trivial, no RC needed
      TransitivelyTrivial,
      /// Value never escapes function scope
      DoesNotEscape,
      /// Sharing bound is within RC width
      BoundedSharing { max_refs: u32 },
      /// Niche available in field
      NicheAvailable { field: u32, niche: u64 },
      /// Custom reason (for tracing)
      Custom(String),
  }
  ```

- [ ] Define `ReprPlan` — the central data structure:
  ```rust
  pub struct ReprPlan {
      /// Per-type decisions (indexed by Pool Idx)
      decisions: FxHashMap<Idx, ReprDecision>,
      /// Per-function escape info (indexed by function id)
      escape_info: FxHashMap<FunctionId, EscapeInfo>,
      /// Audit trail — all decisions in order
      audit: Vec<ReprDecision>,
  }
  ```

- [ ] Implement builder pattern for populating ReprPlan:
  ```rust
  impl ReprPlan {
      pub fn new() -> Self { ... }

      /// Record a narrowing decision. Later decisions override earlier ones
      /// for the same type, but the audit trail preserves both.
      pub fn set_repr(&mut self, idx: Idx, decision: ReprDecision) { ... }

      /// Query the representation for a type
      pub fn get_repr(&self, idx: Idx) -> Option<&MachineRepr> { ... }

      /// Get the canonical (default, un-narrowed) representation for a tag
      pub fn canonical(tag: Tag) -> MachineRepr { ... }

      /// Dump the audit trail for debugging
      pub fn dump_audit(&self, pool: &Pool) -> String { ... }
  }
  ```

---

## 01.3 Pipeline Integration Point

**File(s):** `compiler/ori_llvm/src/codegen/mod.rs`, `compiler/ori_llvm/src/codegen/type_info/info.rs`

The ReprPlan must be computed AFTER type checking and BEFORE LLVM codegen. The codegen must consume ReprPlan instead of computing representations inline.

- [ ] Add `ori_repr` dependency to `ori_llvm/Cargo.toml`

- [ ] Create the ReprPlan computation entry point:
  ```rust
  // In ori_repr/src/lib.rs
  pub fn compute_repr_plan(pool: &Pool, functions: &[FunctionSig]) -> ReprPlan {
      let mut plan = ReprPlan::new();

      // Phase 1: Set canonical representations for all types
      populate_canonical(&mut plan, pool);

      // Phase 2: Triviality analysis (§02)
      analyze_triviality(&mut plan, pool);

      // Phase 3: Range analysis (§03) → Integer narrowing (§04)
      // → Float narrowing (§05)
      // (added in later sections)

      // Phase 4: Struct layout (§06), Enum repr (§07)
      // (added in later sections)

      // Phase 5: Escape analysis (§08) → ARC header (§09)
      // → Thread-local (§10)
      // (added in later sections)

      // Phase 6: Collection specialization (§11)
      // (added in later sections)

      plan
  }
  ```

- [ ] Modify `TypeLayoutResolver` in `ori_llvm` to accept `&ReprPlan`:
  - Currently: `TypeLayoutResolver::new(pool, interner)` → reads `Tag` directly
  - Target: `TypeLayoutResolver::new(pool, interner, repr_plan)` → reads `MachineRepr` from plan
  - Initially, `ReprPlan` returns canonical representations (zero behavioral change)

- [ ] Wire `ReprPlan` through the LLVM codegen entry points:
  - `compile_module()` creates `ReprPlan`
  - Passes it to `ModuleCompiler`
  - `ModuleCompiler` passes it to `FunctionCompiler`
  - `FunctionCompiler` passes it to `TypeLayoutResolver`

---

## 01.4 ReprPlan Query Interface

**File(s):** `compiler/ori_repr/src/query.rs`

Provide ergonomic query methods that later sections will use:

- [ ] Integer width queries:
  ```rust
  impl ReprPlan {
      /// Get the machine integer width for a type (defaults to I64)
      pub fn int_width(&self, idx: Idx) -> IntWidth { ... }

      /// Get LLVM integer type for a type
      pub fn llvm_int_type(&self, idx: Idx, ctx: &Context) -> IntType { ... }

      /// Is this type trivial (no RC needed)?
      pub fn is_trivial(&self, idx: Idx) -> bool { ... }

      /// Does this value escape its defining function?
      pub fn escapes(&self, func: FunctionId, var: VarId) -> bool { ... }

      /// What RC strategy should be used for this allocation?
      pub fn rc_strategy(&self, idx: Idx) -> RcStrategy { ... }
  }

  pub enum RcStrategy {
      /// No RC needed (trivial or stack-promoted)
      None,
      /// Atomic RC with given header width
      Atomic { width: IntWidth },
      /// Non-atomic RC (thread-local proven)
      NonAtomic { width: IntWidth },
  }
  ```

- [ ] Tracing integration:
  ```rust
  // All ReprPlan queries emit tracing events at trace level
  impl ReprPlan {
      pub fn get_repr_traced(&self, idx: Idx, pool: &Pool) -> &MachineRepr {
          let repr = self.get_repr(idx).unwrap_or(&self.canonical(pool.tag(idx)));
          tracing::trace!(
              type_tag = ?pool.tag(idx),
              repr = ?repr,
              "ReprPlan query"
          );
          repr
      }
  }
  ```

---

## 01.5 Completion Checklist

- [ ] `ori_repr` crate compiles with `cargo check -p ori_repr`
- [ ] `MachineRepr` enum has variants for all §02-§11 optimization targets
- [ ] `ReprPlan` populates canonical representations for all 37 `Tag` variants
- [ ] `TypeLayoutResolver` in `ori_llvm` reads from `ReprPlan` instead of hardcoded `Tag → LLVM` map
- [ ] `./test-all.sh` green — zero behavioral changes (canonical reprs match existing hardcoded ones)
- [ ] Tracing output shows `ReprPlan query` events at `ORI_LOG=ori_repr=trace`
- [ ] No regressions in `./llvm-test.sh` or `cargo st`

**Exit Criteria:** `ori_repr` crate exists, `ReprPlan` is threaded through the entire LLVM codegen pipeline, all existing tests pass with identical behavior, and `ORI_LOG=ori_repr=trace ori build tests/benchmarks/fibonacci.ori` shows `ReprPlan query` events for every type in the program.
