---
section: "01"
title: "Representation IR & Decision Framework"
status: not-started
reviewed: false
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
    title: "Generic Type Handling"
    status: not-started
  - id: "01.6"
    title: "Salsa Integration Strategy"
    status: not-started
  - id: "01.7"
    title: "#repr Attribute Integration"
    status: not-started
  - id: "01.8"
    title: "Migration Strategy: TypeInfoStore → ReprPlan"
    status: not-started
  - id: "01.9"
    title: "Canonical Representation Tests"
    status: not-started
  - id: "01.10"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Representation IR & Decision Framework

**Context:** Today, `ori_llvm::codegen::type_info::store.rs` maps `Tag` to `TypeInfo` (e.g., `Tag::Int → TypeInfo::Int`) in `compute_type_info_inner()`, and `info.rs` maps `TypeInfo` to LLVM types (e.g., `TypeInfo::Int → i64`) in `storage_type()`, with companion methods `size()`, `alignment()`, and `is_trivial()`. To support narrowing, we need a centralized decision document that multiple analysis passes can populate and codegen can read.

**Reference implementations:**
- **Lean4** `src/Lean/Compiler/LCNF/Types.lean`: Phase-separated IR where semantic types and machine types are distinct data structures
- **Zig** `src/InternPool.zig`: Layout information interned alongside types — each type has pre-computed size/alignment
- **Roc** `crates/compiler/mono/src/layout/intern.rs`: `STLayoutInterner` maps type variables to concrete layouts after monomorphization

**Depends on:** Nothing — this is the foundation.

---

## 01.1 MachineRepr Enum & ReprPlan Data Structure

**File(s):** `compiler/ori_repr/src/lib.rs` (NEW crate), `compiler/ori_repr/src/repr.rs`

**File layout** (~1,130 production lines across 6 files, all under the 500-line limit):

| File | Contents | Est. Lines |
|------|----------|-----------|
| `lib.rs` | Module declarations, `pub use` re-exports | ~30 |
| `repr.rs` | `MachineRepr` enum + sub-repr types (`StructRepr`, `EnumRepr`, etc.) | ~350 |
| `plan.rs` | `ReprPlan` struct + builder + query methods | ~300 |
| `query.rs` | Ergonomic query interface (`int_width`, `is_trivial`, `escapes`, `rc_strategy`) | ~150 |
| `repr_attrs.rs` | `ReprAttribute` enum + validation | ~100 |
| `canonical.rs` | `canonical(tag)` mapping for all `Tag` variants | ~200 |
| `tests.rs` | All tests (sibling to `lib.rs` — tests exempt from 500-line limit) | unlimited |

The `MachineRepr` enum captures the physical representation chosen for each type. It must be rich enough to express all optimizations in §02-§11 but simple enough that codegen can pattern-match exhaustively.

- [ ] Create new crate `ori_repr` with `Cargo.toml` entry
  - Dependencies: `ori_types` (for `Pool`, `Idx`, `Tag`), `ori_ir` (for `Name` — the interned function identifier), `rustc-hash` (workspace dep — for `FxHashMap`/`FxHashSet`)
  - Dependencies (added by later sections): `ori_arc` (for `ArcFunction`, `ArcVarId` — used by §03 range analysis and §08 escape analysis)
  - No dependency on `ori_llvm` — this is backend-independent
  - No dependency on `ori_eval` — this is evaluation-independent
  - Architecture: `ori_types` → `ori_arc` → `ori_repr` → `ori_llvm` (no cycle — `ori_repr` reads from `ori_arc` IR types but `ori_arc` does not depend on `ori_repr`)
  - **Verified**: `ori_types` has `Pool`, `Idx`, `Tag` in its pub API; `rustc-hash` is a workspace dep used by `ori_types`, `ori_arc`, and `ori_llvm`
  - Add `#![deny(unsafe_code)]` to `ori_repr/src/lib.rs` (pure analysis crate, same as `ori_ir`, `ori_types`, `ori_lexer`)

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
      /// Unicode scalar value (always i32 — 0..=0x10FFFF)
      Char,
      /// 8-bit unsigned byte (always i8)
      Byte,
      /// Duration in nanoseconds (always i64)
      Duration,
      /// Memory size in bytes (always i64)
      Size,
      /// Comparison ordering (always i8: Less=0, Equal=1, Greater=2)
      Ordering,
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
      /// Fat pointer (ptr + metadata) — used for str, [T], {K:V}, Set<T>
      FatPointer(FatRepr),
      /// Function pointer (fn ptr + optional env ptr)
      Closure(ClosureRepr),
      /// Range (always {i64 start, i64 end, i64 step, i64 inclusive})
      Range,
      /// Stack-promoted value (was heap, promoted by escape analysis)
      StackPromoted { inner: Box<MachineRepr>, original_rc: bool },
      /// Opaque pointer (iterator, channel — runtime-managed)
      OpaquePtr,
  }
  // NOTE: Box<MachineRepr> in StackPromoted, FatRepr::Collection,
  // RcRepr::inner, and ClosureRepr::ret causes heap allocation per type.
  // Acceptable: MachineRepr is computed once per type during ReprPlan
  // construction (not per-expression), the plan is immutable after
  // construction, and recursive types require indirection. If profiling
  // shows this matters, consider interning via MachineReprId indices.
  //
  // Add after implementation:
  //   const _: () = assert!(std::mem::size_of::<MachineRepr>() <= 48);

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum IntWidth { I8, I16, I32, I64 }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum FloatWidth { F32, F64 }
  ```

- [ ] Implement `canonical(tag: Tag, pool: &Pool, idx: Idx) -> MachineRepr` for ALL Tag variants (this is the most critical part of §01 — it defines what "canonical" means for every Tag variant, ensuring the ReprPlan starts correct before any optimization runs):

  **Primitives (0-11):**
  | Tag | Canonical MachineRepr | LLVM Type | Notes |
  |-----|----------------------|-----------|-------|
  | `Int` | `Int { width: I64, signed: true }` | `i64` | |
  | `Float` | `Float { width: F64 }` | `double` | |
  | `Bool` | `Bool` | `i1` | |
  | `Str` | `FatPointer(FatRepr::Str)` | `{i64, i64, ptr}` | len + cap + data |
  | `Char` | `Char` | `i32` | Unicode scalar |
  | `Byte` | `Byte` | `i8` | Unsigned |
  | `Unit` | `Unit` | `i64` | LLVM void workaround |
  | `Never` | `Never` | `i64` | LLVM void workaround |
  | `Error` | Panic/unreachable | — | Should never reach codegen |
  | `Duration` | `Duration` | `i64` | Nanoseconds |
  | `Size` | `Size` | `i64` | Bytes |
  | `Ordering` | `Ordering` | `i8` | 0/1/2 |

  **Simple containers (16-22):**
  | Tag | Canonical MachineRepr | LLVM Type | Notes |
  |-----|----------------------|-----------|-------|
  | `List` | `FatPointer(FatRepr::Collection)` | `{i64, i64, ptr}` | len + cap + data |
  | `Option` | `Enum(...)` | `{i64, payload}` | Recurse into inner |
  | `Set` | `FatPointer(FatRepr::Collection)` | `{i64, i64, ptr}` | len + cap + data |
  | `Channel` | `OpaquePtr` | `ptr` | Runtime-managed |
  | `Range` | `Range` | `{i64, i64, i64, i64}` | start/end/step/incl |
  | `Iterator` | `OpaquePtr` | `ptr` | Runtime-managed |
  | `DoubleEndedIterator` | `OpaquePtr` | `ptr` | Runtime-managed |

  **Two-child containers (32-34):**
  | Tag | Canonical MachineRepr | LLVM Type | Notes |
  |-----|----------------------|-----------|-------|
  | `Map` | `FatPointer(FatRepr::Collection)` | `{i64, i64, ptr}` | len + cap + data |
  | `Result` | `Enum(...)` | `{i64, max(ok,err)}` | Recurse into ok/err |
  | `Borrowed` | Reserved — error if reached | — | Future use |

  **Complex types (48-51):**
  | Tag | Canonical MachineRepr | Notes |
  |-----|----------------------|-------|
  | `Function` | `Closure(ClosureRepr)` | fn ptr + optional env ptr |
  | `Tuple` | `Tuple(TupleRepr)` | Recurse into elements |
  | `Struct` | `Struct(StructRepr)` | Recurse into fields |
  | `Enum` | `Enum(EnumRepr)` | Recurse into variants |

  **Named/resolved types (80-82):**
  | Tag | Canonical MachineRepr | Notes |
  |-----|----------------------|-------|
  | `Named` | `pool.resolve_fully(idx)` → recurse | Must resolve first — includes newtypes (`type UserId = int`) and FFI types (`CPtr`, `c_int`) |
  | `Applied` | `pool.resolve_fully(idx)` → recurse | Must resolve first |
  | `Alias` | `pool.resolve_fully(idx)` → recurse | Must resolve first |

  **Newtype handling:** `type UserId = int` uses `Tag::Named` in the Pool. `resolve_fully()` follows the Named→concrete chain, so `canonical()` transparently handles newtypes by recursing into the underlying type. The TypeRegistry stores `TypeKind::Newtype { underlying }` for semantic purposes (`.inner` access), but `canonical()` only needs the Pool-level resolution. No special case needed.

  **FFI types:** `CPtr`, `JsValue`, `c_int`, `c_char`, etc. are named types in the FFI prelude, not Pool primitives. They resolve via `Tag::Named` → concrete. `CPtr` resolves to an opaque pointer (`MachineRepr::OpaquePtr`). C numeric types resolve to their corresponding primitives. No special case needed.

  **Type variables (96-98) — MUST NOT reach canonical:**
  | Tag | Behavior | Notes |
  |-----|----------|-------|
  | `Var` | Follow link chain via `pool.resolve_fully()` | If unresolved → panic (typeck bug) |
  | `BoundVar` | Error — should be monomorphized | Typeck bug if reached |
  | `RigidVar` | Error — should be monomorphized | Typeck bug if reached |

  **Scheme/Special (112, 240-255) — MUST NOT reach canonical:**
  | Tag | Behavior |
  |-----|----------|
  | `Scheme` | Error — should be instantiated |
  | `Projection` | Error — should be resolved |
  | `ModuleNs` | Error — not a value type |
  | `Infer` | Error — should be resolved |
  | `SelfType` | Error — should be resolved |

  **Validation:** The canonical mapping MUST produce the same LLVM types as the
  existing `TypeInfo::storage_type()` → `compute_type_info_inner()` pipeline.
  A dedicated test iterates all types in a test Pool and asserts `canonical(tag).to_llvm_type() == TypeInfo::storage_type()`.

- [ ] Define `FatRepr` to distinguish collection/string fat pointers:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
  pub enum FatRepr {
      /// String: {i64 len, i64 cap, ptr data}
      Str,
      /// Collection ([T], {K:V}, Set<T>): {i64 len, i64 cap, ptr data}
      Collection { element_repr: Box<MachineRepr> },
  }
  ```

- [ ] Define `ClosureRepr`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
  pub struct ClosureRepr {
      /// Parameter representations
      pub params: Vec<MachineRepr>,
      /// Return representation
      pub ret: Box<MachineRepr>,
  }
  ```

**Derive requirement:** ALL sub-repr types (`StructRepr`, `EnumRepr`, `TupleRepr`, `FieldRepr`, `EnumTag`, `VariantRepr`, `RcRepr`, `FatRepr`, `ClosureRepr`) MUST derive `Debug, Clone, PartialEq, Eq, Hash` to match `MachineRepr`'s derives. Code blocks below include them explicitly.

- [ ] Define `TupleRepr`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
  pub struct TupleRepr {
      /// Element representations in optimized memory order
      pub elements: Vec<FieldRepr>,
      pub size: u32,
      pub align: u32,
      pub trivial: bool,
  }
  ```

- [ ] Define `StructRepr`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
  pub struct EnumRepr {
      /// Discriminant representation
      pub tag: EnumTag,
      /// Per-variant payload representations
      pub variants: Vec<VariantRepr>,
      /// Total size including tag and padding
      pub size: u32,
      pub align: u32,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum EnumTag {
      /// Explicit tag field at offset 0
      Explicit { width: IntWidth },
      /// Niche — tag stored in invalid bit pattern of a field
      Niche { field_index: u32, niche_value: u64 },
      /// No tag needed (single inhabited variant, e.g. newtype)
      None,
  }
  ```

- [ ] Define `VariantRepr`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
  pub struct VariantRepr {
      /// Variant name (interned)
      pub name: Name,
      /// Field representations (empty for unit variants)
      pub fields: Vec<MachineRepr>,
      /// Size of this variant's payload (excluding tag)
      pub size: u32,
      /// Alignment of this variant's payload
      pub alignment: u32,
  }

  impl VariantRepr {
      /// Whether this variant is a pointer type (for tagged pointer optimization)
      pub fn is_pointer(&self) -> bool {
          self.fields.len() == 1
              && matches!(
                  &self.fields[0],
                  MachineRepr::RcPointer(_) | MachineRepr::FatPointer(_) | MachineRepr::OpaquePtr
              )
      }
  }
  ```

- [ ] Define `RcRepr`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
  // FxHashMap from `rustc-hash` crate (workspace dep): `use rustc_hash::FxHashMap;`
  // Functions are identified by Name (from ori_ir), not FunctionId (ori_llvm-specific).
  pub struct ReprPlan {
      /// Per-type decisions (indexed by Pool Idx)
      decisions: FxHashMap<Idx, ReprDecision>,
      /// Per-type #repr attributes (only for structs/enums with explicit attrs)
      /// See §01.7 for ReprAttribute enum definition.
      repr_attrs: FxHashMap<Idx, ReprAttribute>,
      /// Per-function escape info (indexed by function Name)
      /// NOTE: EscapeInfo is defined in §08 (escape/mod.rs). This field is
      /// empty until §08 populates it. Initially use `type EscapeInfo = ();`
      /// as a placeholder, replaced when §08 is implemented.
      escape_info: FxHashMap<Name, EscapeInfo>,
      /// Per-function, per-variable ranges from §03 range analysis.
      /// Key: function Name → (ArcVarId → ValueRange).
      /// Populated by §03, consumed by §04 (integer narrowing for locals/fields).
      /// NOTE: ValueRange is defined in §03 (range/mod.rs). Use `type ValueRange = ();`
      /// as a placeholder until §03 is implemented.
      function_var_ranges: FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>>,
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

      /// Get the canonical (un-narrowed) representation for a tag
      pub fn canonical(tag: Tag) -> MachineRepr { ... }

      /// Record per-variable range analysis results for a function (§03 output).
      /// Called after `range_fixpoint()` completes for a function.
      pub fn set_var_ranges(
          &mut self,
          func: Name,
          ranges: FxHashMap<ArcVarId, ValueRange>,
      ) { ... }

      /// Get the range for a variable in a function (from §03 range analysis).
      /// Returns ValueRange::Top if no range was recorded.
      pub fn var_range(&self, func: Name, var: ArcVarId) -> ValueRange { ... }

      /// Dump the audit trail for debugging
      pub fn dump_audit(&self, pool: &Pool) -> String { ... }
  }
  ```

---

## 01.3 Pipeline Integration Point

**File(s):** `compiler/ori_llvm/src/codegen/type_info/mod.rs` (TypeLayoutResolver), `compiler/ori_llvm/src/codegen/type_info/store.rs` (TypeInfoStore — Tag→TypeInfo mapping), `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` (FunctionCompiler), `compiler/ori_llvm/src/evaluator/compile.rs` (JIT entry point), `compiler/oric/src/commands/build/mod.rs`, `compiler/oric/src/commands/build_options.rs`

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
  - Currently: `TypeLayoutResolver::new(store, scx, interner)` where `store: &TypeInfoStore`, `scx: &SimpleCx`, `interner: Option<&StringInterner>` → reads `TypeInfo` from store (which reads `Tag` from `Pool`)
  - Target: `TypeLayoutResolver::new(store, scx, interner, repr_plan)` → reads `MachineRepr` from plan when available, falling back to `TypeInfo` for unoptimized types
  - Initially, `ReprPlan` returns canonical representations (zero behavioral change)

- [ ] Wire `ReprPlan` through the LLVM codegen entry points:
  - JIT path: `OwnedLLVMEvaluator::compile_module_with_tests()` (in `evaluator/compile.rs`) creates `ReprPlan`
  - AOT path: the AOT build pipeline creates `ReprPlan` before constructing `FunctionCompiler`
  - `ReprPlan` is passed to `FunctionCompiler::new()` (there is no `ModuleCompiler` — `FunctionCompiler` is the two-pass declare/define orchestrator)
  - `FunctionCompiler` passes it to `TypeLayoutResolver`

- [ ] Add `--no-repr-opt` flag to the `ori build` CLI (`compiler/oric/src/commands/build/mod.rs` + `compiler/oric/src/commands/build_options.rs`):
  - When set, `compute_repr_plan()` still runs but immediately returns after `populate_canonical()` (canonical-only plan — same as current behavior)
  - This flag is required by §12.2 for dual-execution comparison: AOT without optimizations vs. AOT with optimizations
  - The flag name must be consistent: `--no-repr-opt` in CLI, `repr_opt_disabled: bool` in build config
  - Add `ORI_NO_REPR_OPT=1` environment variable as an alternative (same effect as `--no-repr-opt`)

- [ ] Keep `ori_repr` tracing compatible with the existing generic `ORI_LOG` / `RUST_LOG` filter in `compiler/oric/src/tracing_setup.rs`:
  - No tracing registry change is needed today — `tracing_setup.rs` already forwards arbitrary targets through `EnvFilter`
  - Emit `tracing` events from the new crate under target `ori_repr`
  - Add a smoke test or manual verification step showing `ORI_LOG=ori_repr=trace ori build ...` surfaces `ori_repr` events without extra CLI wiring

---

## 01.4 ReprPlan Query Interface

**File(s):** `compiler/ori_repr/src/query.rs`

Provide ergonomic query methods that later sections will use:

**Phase boundary:** `ori_repr` must NEVER import from `ori_llvm` or `ori_eval`. LLVM-specific convenience methods (e.g., `llvm_int_type(plan, idx, ctx)`) belong in `ori_llvm` as an extension trait (`impl ReprPlanExt for ReprPlan`), not in `ori_repr`.

- [ ] Integer width queries:
  ```rust
  impl ReprPlan {
      /// Get the machine integer width for a type (defaults to I64)
      pub fn int_width(&self, idx: Idx) -> IntWidth { ... }

      // NOTE: LLVM-specific methods like `llvm_int_type(idx, ctx) -> IntType`
      // belong in ori_llvm (e.g., as an extension trait or helper), not in
      // ori_repr, since ori_repr must remain backend-independent.

      /// Is this type trivial (no RC needed)?
      pub fn is_trivial(&self, idx: Idx) -> bool { ... }

      /// Does this value escape its defining function?
      pub fn escapes(&self, func: Name, var: VarId) -> bool { ... }

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

## 01.5 Generic Type Handling

**File(s):** `compiler/ori_repr/src/plan.rs`, `compiler/ori_repr/src/lib.rs`

ReprPlan operates on **monomorphized** types only. Generic types (containing `Var`, `BoundVar`, `RigidVar`) cannot be mapped to concrete machine representations.

- [ ] Enforce monomorphization precondition:
  - `compute_repr_plan()` must be called AFTER monomorphization (all type variables resolved)
  - `canonical()` must assert/panic on `Tag::Var`, `Tag::BoundVar`, `Tag::RigidVar`, `Tag::Scheme`, `Tag::Infer`
  - For `Tag::Named`/`Tag::Applied`/`Tag::Alias`: always resolve via `pool.resolve_fully()` first — if resolution yields a type variable, it's a monomorphization bug

- [ ] Handle `Option<T>` and `Result<T, E>` generically:
  - After monomorphization, `Option<int>` is a concrete type with `Tag::Option` and inner `Idx` pointing to `Tag::Int`
  - The `canonical()` function recurses: `Option<int>` → `Enum(EnumRepr { variants: [Some(Int{I64}), None] })`
  - This works because Pool interning deduplicates: `Option<int>` at two call sites shares the same `Idx`

- [ ] Monomorphization boundary:
  - Currently, Ori does NOT have explicit monomorphization pass — type checker infers concrete types, and Pool stores them
  - The `pool.resolve_fully()` chain handles substitution transparently
  - ReprPlan must call `pool.resolve_fully(idx)` before computing canonical for ANY type to ensure all variables are resolved
  - If `resolve_fully()` returns a variable → skip this type (it's dead code or a typeck bug)

---

## 01.6 Salsa Integration Strategy

**File(s):** `compiler/ori_repr/src/lib.rs`, `compiler/oric/src/commands/codegen_pipeline.rs`

The ReprPlan must integrate with the existing Salsa-based compilation model.

- [ ] **ReprPlan is NOT a Salsa tracked struct** — it is computed imperatively:
  - Salsa works best for demand-driven, memoizable queries (parsing, type checking)
  - ReprPlan computation is a forward pass that mutates state across multiple analysis phases (triviality → range → narrowing → layout)
  - Making each phase a Salsa query would create artificial dependencies and complicate the multi-pass mutation pattern
  - Instead: compute ReprPlan once, pass it as `&ReprPlan` to codegen (same model as how `TypeInfoStore` works today)

- [ ] **Invalidation model:**
  - ReprPlan is invalidated when the Pool changes (new/modified types)
  - In the current compilation model, this means: recompute ReprPlan on every compilation
  - Future optimization: if Pool didn't change (Salsa cache hit on type checking), reuse previous ReprPlan
  - This can be implemented as a Salsa query that takes Pool hash → ReprPlan, memoized by Pool identity

- [ ] **JIT hot-reload compatibility:**
  - JIT recompiles individual functions — the ReprPlan for unchanged functions is stable
  - When a function's type signature changes, only that function's entries need recomputation
  - For now: recompute entire ReprPlan per JIT invocation (same as TypeInfoStore today)
  - Future: incremental ReprPlan updates keyed by function-level Merkle hashes

- [ ] **Thread safety:**
  - ReprPlan is immutable after computation — `&ReprPlan` is `Send + Sync`
  - No interior mutability needed (unlike TypeInfoStore which uses RefCell for lazy population)
  - All analysis passes write to a `&mut ReprPlan` during computation, then freeze it for codegen

---

## 01.7 `#repr` Attribute Integration

**File(s):** `compiler/ori_repr/src/repr_attrs.rs`

The spec (Clause 26 — FFI) defines layout attributes that override the canonical representation:
- `#repr("c")` — C-compatible layout, no field reordering
- `#repr("packed")` — No padding, alignment = 1
- `#repr("transparent")` — Same layout as single field (newtypes)
- `#repr("aligned", N)` — Minimum N-byte alignment (power of two)

These must be threaded into ReprPlan to prevent optimizations from violating user intent.

- [ ] Define `ReprAttribute` enum:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
  pub enum ReprAttribute {
      /// Default Ori layout — field reordering and narrowing permitted
      Default,
      /// C-compatible layout — declaration order, platform ABI alignment
      C,
      /// No padding — alignment = 1, may require unaligned loads
      Packed,
      /// Transparent — same layout as the single field
      Transparent,
      /// Minimum alignment (power of two), may combine with C
      Aligned(u32),
      /// C + Aligned combined (#repr("c") + #repr("aligned", N))
      CAligned(u32),
  }
  ```

- [ ] Store `ReprAttribute` per struct/enum in ReprPlan (already included in the `ReprPlan` struct definition in §01.2):
  ```rust
  /// Per-type #repr attributes (only for structs/enums with explicit attrs)
  repr_attrs: FxHashMap<Idx, ReprAttribute>,
  ```

- [ ] Gate optimization passes on `ReprAttribute`:
  - `ReprAttribute::C` → §06 field reordering DISABLED, §04 field narrowing DISABLED
  - `ReprAttribute::Packed` → §06 padding DISABLED, alignment = 1
  - `ReprAttribute::Transparent` → struct is erased to its single field's MachineRepr
  - `ReprAttribute::Aligned(N)` → struct alignment ≥ N (overrides computed alignment)
  - `ReprAttribute::Default` → all optimizations permitted

- [ ] Parse `#repr` from the IR and populate during `populate_canonical()`:
  - The parser already stores `#repr` attributes on struct declarations
  - During canonical population, read the attribute and store in `repr_attrs`
  - Validate: `#repr("transparent")` requires exactly one non-ZST field
  - Validate: `#repr("aligned", N)` requires N is a power of two
  - Validate: `#repr("packed")` cannot combine with `#repr("aligned", N)` or `#repr("c")`

---

## 01.8 Migration Strategy: TypeInfoStore → ReprPlan

**File(s):** `compiler/ori_llvm/src/codegen/type_info/store.rs`, `compiler/ori_llvm/src/codegen/type_info/info.rs`

The existing `TypeInfoStore` and `TypeInfo` enum must coexist with `ReprPlan` during migration. The goal is gradual adoption, not a big-bang replacement.

- [ ] **Phase A — Parallel operation (§01 scope):**
  - `TypeLayoutResolver` accepts optional `&ReprPlan`
  - When `ReprPlan` is `Some`, consult it first; if no decision exists for a type, fall back to `TypeInfoStore`
  - When `ReprPlan` is `None` (e.g., in tests that don't create one), use `TypeInfoStore` exclusively
  - This ensures zero behavioral change: ReprPlan returns canonical representations, which match TypeInfoStore exactly

- [ ] **Phase B — Triviality unification (§02 scope):**
  - `TypeInfoStore::is_trivial()` delegates to `ReprPlan::is_trivial()` when available
  - `TypeInfoStore::classify_trivial()` becomes dead code and is removed
  - `triviality_cache` and `classifying_trivial` fields removed from TypeInfoStore

- [ ] **Phase C — Full migration (§06/§07 scope):**
  - `TypeLayoutResolver::storage_type()` reads from `ReprPlan` for ALL types
  - `TypeInfoStore::compute_type_info_inner()` is no longer called from production code
  - `TypeInfo` enum is retained only as a compatibility adapter for tests that don't use ReprPlan
  - Eventually, `TypeInfo` becomes `#[cfg(test)]` only

- [ ] **Validation at each phase:**
  - Phase A: `assert_eq!(repr_plan.canonical(tag).to_llvm_type(), type_info.storage_type())` for all types
  - Phase B: same assertion + `assert_eq!(repr_plan.is_trivial(idx), type_info_store.is_trivial(idx))`
  - Phase C: remove TypeInfoStore from production; tests use ReprPlan directly

---

## 01.9 Canonical Representation Tests

**File(s):** `compiler/ori_repr/src/tests.rs` (sibling to `lib.rs` — `#[cfg(test)] mod tests;` declaration in `lib.rs`, no inline test modules)

Canonical representations are the foundation — if they're wrong, every optimization built on them is wrong.

- [ ] **Primitive roundtrip test:** For each of the 12 primitive Tags (Int, Float, Bool, Str, Char, Byte, Unit, Never, Duration, Size, Ordering, Error), verify `canonical()` produces the expected MachineRepr variant.

- [ ] **Composite type tests:**
  - `Option<int>` → `Enum` with 2 variants, inner is `Int { I64, true }`
  - `Result<int, str>` → `Enum` with 2 variants
  - `(int, bool)` → `Tuple` with 2 elements
  - `[int]` → `FatPointer(Collection { Int { I64, true } })`
  - `{str: int}` → `FatPointer(Collection { ... })`
  - `Set<int>` → `FatPointer(Collection { Int { I64, true } })`

- [ ] **Named type resolution test:** Create a `Named` type pointing to a `Struct`, verify `canonical()` resolves through to the struct's repr.

- [ ] **Storage type equivalence test:** For a Pool containing a representative sample of all constructible types, verify that `canonical(tag).to_llvm_type(ctx)` produces the same LLVM type as the existing `TypeInfo::storage_type()`. This is the gold standard: new system must match old system exactly before any optimizations run.

- [ ] **Error on unresolved types test:** Verify that `canonical()` on `Tag::Var`, `Tag::BoundVar`, `Tag::RigidVar`, `Tag::Scheme`, `Tag::Infer`, `Tag::SelfType` panics or returns an error.

- [ ] **FatPointer layout test:** Verify `FatRepr::Str` and `FatRepr::Collection` both produce `{i64, i64, ptr}` in LLVM, matching the existing collection layout.

---

## 01.10 Completion Checklist

**TDD ordering:** Write §01.9 canonical representation tests BEFORE creating the `ori_repr` crate. All tests must fail (crate does not exist). Create the crate, implement the types, verify tests pass unchanged. Only then proceed to wiring into the pipeline (§01.3).

- [ ] Write failing tests BEFORE implementation (see §01.9 for the full test list)
- [ ] `ori_repr` added to workspace `Cargo.toml` `[members]` list
- [ ] `ori_repr` added to root workspace `Cargo.toml` `[workspace.dependencies]` as a path dep so downstream crates can reference it
- [ ] `ori_repr` crate compiles with `cargo check -p ori_repr`
- [ ] `#![deny(unsafe_code)]` in `ori_repr/src/lib.rs` (pure analysis crate — no unsafe needed)
- [ ] `//!` module doc on every `.rs` file in `ori_repr/src/` (required by hygiene rules)
- [ ] `///` doc on all `pub` types and functions (required by hygiene rules)
- [ ] No production source file exceeds 500 lines (tests.rs exempt)
- [ ] Tests in sibling `tests.rs` with `#[cfg(test)] mod tests;` in `lib.rs` — no inline test modules
- [ ] `MachineRepr` enum has variants for ALL type kinds: Int, Float, Bool, Char, Byte, Duration, Size, Ordering, Unit, Never, Struct, Enum, Tuple, RcPointer, FatPointer, Closure, Range, StackPromoted, OpaquePtr
- [ ] `ReprPlan` populates canonical representations for all reachable `Tag` variants:
  - Primitives (12): Int, Float, Bool, Str, Char, Byte, Unit, Never, Error, Duration, Size, Ordering
  - Simple containers (7): List, Option, Set, Channel, Range, Iterator, DoubleEndedIterator
  - Two-child (3): Map, Result, Borrowed (reserved)
  - Complex (4): Function, Tuple, Struct, Enum
  - Named (3): Named, Applied, Alias (resolve-through)
  - Variables (3): Var, BoundVar, RigidVar (must be resolved or error)
  - Scheme/Special (5): Scheme, Projection, ModuleNs, Infer, SelfType (error if reached)
- [ ] `#repr` attributes (c, packed, transparent, aligned) are parsed and stored in ReprPlan
- [ ] Generic types handled correctly: all type variables resolved before canonical computation
- [ ] Salsa integration: ReprPlan computed imperatively, passed as `&ReprPlan` to codegen
- [ ] Migration Phase A complete: TypeLayoutResolver accepts optional ReprPlan, falls back to TypeInfoStore
- [ ] `TypeLayoutResolver` in `ori_llvm` reads from `ReprPlan` instead of hardcoded `Tag → LLVM` map
- [ ] Storage type equivalence test passes: canonical representations match existing TypeInfo for all types
- [ ] `./test-all.sh` green — zero behavioral changes (canonical reprs match existing hardcoded ones)
- [ ] `./clippy-all.sh` green
- [ ] Tracing output shows `ReprPlan query` events at `ORI_LOG=ori_repr=trace`
- [ ] No regressions in `./llvm-test.sh` or `cargo st`

**Exit Criteria:** `ori_repr` crate exists, `ReprPlan` is threaded through the entire LLVM codegen pipeline, all existing tests pass with identical behavior, and `ORI_LOG=ori_repr=trace ori build tests/benchmarks/bench_small.ori` shows `ReprPlan query` events for every type in the program.
