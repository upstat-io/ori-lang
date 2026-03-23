---
section: "01"
title: "Representation IR & Decision Framework"
status: not-started
reviewed: true
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

**File layout** (~1,230 production lines across 10 files, all under the 500-line limit):

| File | Contents | Est. Lines |
|------|----------|-----------|
| `lib.rs` | Module declarations, `pub use` re-exports, `compute_repr_plan()`, pass stubs | ~60 |
| `repr.rs` | `MachineRepr` enum, `IntWidth`, `FloatWidth` | ~120 |
| `struct_repr.rs` | `StructRepr`, `TupleRepr`, `FieldRepr`, `RcRepr`, `FatRepr`, `ClosureRepr` | ~200 |
| `enum_repr.rs` | `EnumRepr`, `EnumTag`, `VariantRepr` (+ `VariantRepr::is_pointer`) | ~100 |
| `plan.rs` | `ReprPlan` struct + builder + writer methods (`set_repr`, `set_var_ranges`, `set_escape_info`, `set_rc_strategy`) + `NarrowingPolicy` | ~320 |
| `query.rs` | Ergonomic query interface (`int_width`, `float_width`, `is_trivial`, `escapes`, `rc_strategy`, `RcStrategy`) + tracing | ~200 |
| `repr_attrs.rs` | `ReprAttribute` enum + validation | ~100 |
| `canonical.rs` | `canonical(tag)` mapping for all `Tag` variants | ~200 |
| `range/mod.rs` | **Placeholder only in §01** — exports `pub struct ValueRange;` so `DecisionReason::RangeFits` compiles. Replaced in §03. | ~10 |
| `escape/mod.rs` | **Placeholder only in §01** — exports `pub struct EscapeInfo;` so `ReprPlan::escape_info` compiles. Replaced in §08. | ~10 |
| `tests.rs` | All tests (sibling to `lib.rs` — tests exempt from 500-line limit) | unlimited |

The `MachineRepr` enum captures the physical representation chosen for each type. It must be rich enough to express all optimizations in §02-§11 but simple enough that codegen can pattern-match exhaustively.

- [ ] Create new crate `ori_repr` with `Cargo.toml` entry
  - Dependencies from §01: `ori_types` (for `Pool`, `Idx`, `Tag`), `ori_ir` (for `Name` — the interned function identifier), `ori_arc` (for `ArcFunction`, `ArcVarId` — needed immediately for `compute_repr_plan()` signature and `escapes()` query), `rustc-hash` (workspace dep — for `FxHashMap`/`FxHashSet`), `tracing` (workspace dep — for `tracing::trace!` in query methods)
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
      /// Stack-promoted value (was heap, promoted by escape analysis).
      /// `had_rc` records whether the original heap allocation used RC headers —
      /// needed so drop codegen knows whether to emit a stack-local destructor.
      StackPromoted { inner: Box<MachineRepr>, had_rc: bool },
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

**File placement:** `TupleRepr`, `StructRepr`, `FieldRepr`, `RcRepr`, `FatRepr`, `ClosureRepr` → `compiler/ori_repr/src/struct_repr.rs`. `EnumRepr`, `EnumTag`, `VariantRepr` → `compiler/ori_repr/src/enum_repr.rs`. `MachineRepr`, `IntWidth`, `FloatWidth` → `compiler/ori_repr/src/repr.rs`. This matches the file layout table above and keeps all files under 500 lines.

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
      /// Original field name (interned via Name from ori_ir).
      /// Required by §06 for: (1) debug symbol emission (DWARF needs names),
      /// (2) C-ABI verification (declaration order must match for #repr("c")),
      /// (3) tracing output (audit trail logs field names, not indices).
      pub name: Name,
      /// Original field index in declaration order (0-based).
      /// `fields[i].original_index` tells codegen the source order after
      /// §06 may have reordered `fields` by alignment/size.
      pub original_index: u32,
      /// Offset in bytes from struct start (set by §06 layout algorithm)
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

  /// Reason for a narrowing decision — used in audit trail and debug tracing.
  ///
  /// NOTE: `ValueRange` is defined in §03 (`ori_repr/src/range/mod.rs`).
  /// Until §03 is implemented, `ori_repr/src/range/mod.rs` must export a
  /// placeholder: `pub struct ValueRange;` (or `pub type ValueRange = ();`).
  /// `DecisionReason::RangeFits` MUST compile from day one so the crate builds.
  /// The placeholder is replaced by the real type in §03. See §01.10 checklist
  /// item "ValueRange placeholder" for the required stub.
  #[derive(Debug, Clone)]
  pub enum DecisionReason {
      /// Type is canonically this width (no narrowing applied)
      Canonical,
      /// Value range fits in narrower type.
      /// `range` is the computed ValueRange from §03; `min_width` is the
      /// narrowest IntWidth that covers the range.
      RangeFits { range: ValueRange, min_width: IntWidth },
      /// All fields are trivial, no RC needed
      TransitivelyTrivial,
      /// Value never escapes function scope (from §08 escape analysis)
      DoesNotEscape,
      /// Sharing bound is within RC width (from §09 sharing analysis)
      BoundedSharing { max_refs: u32 },
      /// Niche available in field (from §07 enum niche analysis)
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
      /// Returns the "unknown / unconstrained" range if no range was recorded.
      /// In §01's placeholder, this returns the default `ValueRange` value.
      /// In §03's real implementation, this returns `ValueRange::Top` (no constraints).
      pub fn var_range(&self, func: Name, var: ArcVarId) -> ValueRange { ... }

      /// Dump the audit trail for debugging
      pub fn dump_audit(&self, pool: &Pool) -> String { ... }
  }
  ```

**Tests required for §01.2 (add to `tests.rs`, write failing tests BEFORE implementing):**

- [ ] `set_repr` / `get_repr` round-trip: set a decision for `Tag::Int`, retrieve it, assert the `MachineRepr` matches.
- [ ] Override behavior: call `set_repr` twice for the same `Idx`; verify `get_repr` returns the second decision's repr.
- [ ] Audit trail preservation: after the override above, verify `dump_audit()` contains BOTH entries in insertion order.
- [ ] `get_repr` on unknown `Idx` returns `None` (not a panic, not a default).
- [ ] `var_range` on a function with no recorded ranges returns the default/top value (not a panic).
- [ ] `set_var_ranges` / `var_range` round-trip: record ranges for two functions, verify each function's `var_range` query is isolated.
- [ ] `dump_audit` output is non-empty after decisions are recorded and contains the type tag and source in its string representation.

---

## 01.3 Pipeline Integration Point

**File(s):** `compiler/ori_llvm/src/codegen/type_info/mod.rs` (TypeLayoutResolver), `compiler/ori_llvm/src/codegen/type_info/store.rs` (TypeInfoStore — Tag→TypeInfo mapping), `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` (FunctionCompiler), `compiler/ori_llvm/src/evaluator/compile.rs` (JIT entry point), `compiler/oric/src/commands/codegen_pipeline.rs` (AOT entry point — `run_codegen_pipeline()`), `compiler/oric/src/commands/build_options.rs`, `compiler/oric/src/commands/build/mod.rs` (for `--no-repr-opt` CLI flag)

The ReprPlan must be computed AFTER type checking and BEFORE LLVM codegen. The codegen must consume ReprPlan instead of computing representations inline.

- [ ] Add `ori_repr` dependency to `ori_llvm/Cargo.toml`

- [ ] Create the ReprPlan computation entry point:
  ```rust
  // In ori_repr/src/lib.rs
  //
  // `arc_functions`: all ArcFunction values from the ARC pipeline — needed by
  //   §03 (range analysis per ArcFunction) and §08 (escape analysis per ArcFunction).
  //   §01 itself does not use them, but the signature must be established now so
  //   later sections can add their passes without changing the call sites in oric.
  //
  // `policy`: from --no-repr-opt / ORI_NO_REPR_OPT — see NarrowingPolicy in §01.4.
  pub fn compute_repr_plan(
      pool: &Pool,
      arc_functions: &[ArcFunction],
      policy: NarrowingPolicy,
  ) -> ReprPlan {
      let mut plan = ReprPlan::new(policy);

      // Phase 1: Set canonical representations for all types (§01)
      populate_canonical(&mut plan, pool);

      // Phase 2: Triviality analysis (§02)
      // Stub: analyze_triviality(&mut plan, pool);
      // Added in §02 — see analyze_triviality() in ori_repr/src/triviality/mod.rs

      // Phase 3: Range analysis (§03) → Integer narrowing (§04)
      //   → Float narrowing (§05)
      // Stub: analyze_ranges(&mut plan, pool, arc_functions);
      //       apply_integer_narrowing(&mut plan, pool);
      //       apply_float_narrowing(&mut plan, pool);
      // Added in §03, §04, §05

      // Phase 4: Struct layout (§06), Enum repr (§07)
      // Stub: compute_struct_layouts(&mut plan, pool);
      //       compute_enum_reprs(&mut plan, pool);
      // Added in §06, §07

      // Phase 5: Escape analysis (§08) → ARC header (§09)
      //   → Thread-local (§10)
      // Stub: analyze_escape(&mut plan, pool, arc_functions);
      //       compress_arc_headers(&mut plan, pool);
      //       apply_thread_local_arc(&mut plan, pool, arc_functions);
      // Added in §08, §09, §10

      // Phase 6: Collection specialization (§11)
      // Stub: specialize_collections(&mut plan, pool);
      // Added in §11

      plan
  }
  ```

  **Stub function requirement:** To keep `ori_repr` immediately compilable and allow
  §02-§11 to be developed independently, §01 must provide empty stub functions for
  each pass that `compute_repr_plan()` will eventually call. Each stub lives in its
  own module (created by the corresponding section). For §01, add these to `lib.rs`
  or the module root with `#[allow(dead_code)]`:
  ```rust
  // Stubs — replaced by real implementations in §02-§11
  fn analyze_triviality(_plan: &mut ReprPlan, _pool: &Pool) {}
  fn analyze_ranges(_plan: &mut ReprPlan, _pool: &Pool, _fns: &[ArcFunction]) {}
  fn apply_integer_narrowing(_plan: &mut ReprPlan, _pool: &Pool) {}
  fn apply_float_narrowing(_plan: &mut ReprPlan, _pool: &Pool) {}
  fn compute_struct_layouts(_plan: &mut ReprPlan, _pool: &Pool) {}
  fn compute_enum_reprs(_plan: &mut ReprPlan, _pool: &Pool) {}
  fn analyze_escape(_plan: &mut ReprPlan, _pool: &Pool, _fns: &[ArcFunction]) {}
  fn compress_arc_headers(_plan: &mut ReprPlan, _pool: &Pool) {}
  fn apply_thread_local_arc(_plan: &mut ReprPlan, _pool: &Pool, _fns: &[ArcFunction]) {}
  fn specialize_collections(_plan: &mut ReprPlan, _pool: &Pool) {}
  ```

- [ ] Modify `TypeLayoutResolver` in `ori_llvm` to accept `&ReprPlan`:
  - Currently: `TypeLayoutResolver::new(store, scx, interner)` where `store: &TypeInfoStore`, `scx: &SimpleCx`, `interner: Option<&StringInterner>` → reads `TypeInfo` from store (which reads `Tag` from `Pool`)
  - Target: `TypeLayoutResolver::new(store, scx, interner, repr_plan)` → reads `MachineRepr` from plan when available, falling back to `TypeInfo` for unoptimized types
  - Initially, `ReprPlan` returns canonical representations (zero behavioral change)

- [ ] Wire `ReprPlan` through the LLVM codegen entry points:
  - JIT path: `OwnedLLVMEvaluator::compile_module_with_tests()` (in `evaluator/compile.rs`) creates `ReprPlan`. Add `narrowing_policy: NarrowingPolicy` as a new last parameter (after `arc_cache`) — callers pass `NarrowingPolicy::Aggressive` by default or `NarrowingPolicy::Disabled` when the test runner sets `ORI_NO_REPR_OPT`.
  - AOT path: `run_codegen_pipeline()` in `compiler/oric/src/commands/codegen_pipeline.rs` creates `ReprPlan` before constructing `FunctionCompiler`. Add `narrowing_policy: NarrowingPolicy` as a new last parameter. This function is called from `compile_common.rs::compile_to_llvm()` and `compile_to_llvm_with_imports()` — both callers must thread the policy through from `BuildOptions.narrowing_policy`.
  - `ReprPlan` is passed to `FunctionCompiler::new()` (there is no `ModuleCompiler` — `FunctionCompiler` is the two-pass declare/define orchestrator). Currently `FunctionCompiler::new()` takes: `builder`, `type_info`, `type_resolver`, `interner`, `pool`, `module_path`, `annotated_sigs`, `arc_classifier`, `debug_context`, `uniqueness_summaries`, `aims_contracts`, `verify_arc` — add `repr_plan: &'a ReprPlan` immediately before `verify_arc` (last position before the boolean flag, following the config-struct convention that booleans come last).
  - `FunctionCompiler` stores `repr_plan` and passes it to `TypeLayoutResolver`

- [ ] Add `--no-repr-opt` flag to the `ori build` CLI (`compiler/oric/src/commands/build_options.rs` for the flag definition and `parse_build_options()`, `compiler/oric/src/commands/build/mod.rs` for CLI integration, `compiler/oric/src/commands/codegen_pipeline.rs` for enforcement):
  - Add `narrowing_policy: NarrowingPolicy` field to `BuildOptions` (import `NarrowingPolicy` from `ori_repr`); default to `NarrowingPolicy::Aggressive`
  - Parse `--no-repr-opt` in `parse_build_options()` → set `options.narrowing_policy = NarrowingPolicy::Disabled`
  - Thread `BuildOptions.narrowing_policy` through `compile_common.rs` → `run_codegen_pipeline()` (the new last parameter added above)
  - When `narrowing_policy == NarrowingPolicy::Disabled`, `compute_repr_plan()` returns after `populate_canonical()` (canonical-only plan — zero behavioral change vs today)
  - This flag is required by §12.2 for dual-execution comparison: AOT without optimizations vs. AOT with optimizations
  - Add `ORI_NO_REPR_OPT=1` environment variable as an alternative (same effect as `--no-repr-opt`); check it in `run_codegen_pipeline()` alongside the policy parameter
  - Do NOT use `repr_opt_disabled: bool` — use `NarrowingPolicy` so future conservative mode is also expressible
  - **Hygiene fix while touching this file**: `build_options.rs` line 15 uses `#[allow(clippy::struct_excessive_bools, reason = ...)]` — change to `#[expect(clippy::struct_excessive_bools, reason = ...)]` per lint discipline rules

- [ ] Keep `ori_repr` tracing compatible with the existing generic `ORI_LOG` / `RUST_LOG` filter in `compiler/oric/src/tracing_setup.rs`:
  - No tracing registry change is needed today — `tracing_setup.rs` already forwards arbitrary targets through `EnvFilter`
  - Emit `tracing` events from the new crate under target `ori_repr`
  - Add a smoke test or manual verification step showing `ORI_LOG=ori_repr=trace ori build ...` surfaces `ori_repr` events without extra CLI wiring

**Tests required for §01.3 (write failing tests BEFORE implementing):**

- [ ] `--no-repr-opt` CLI flag: `ori build --no-repr-opt tests/benchmarks/bench_small.ori` succeeds with exit code 0. Verify (via `ORI_LOG=ori_repr=trace`) that `compute_repr_plan()` returns after `populate_canonical()` without calling any narrowing stubs.
- [ ] `ORI_NO_REPR_OPT=1` env var: same program built with the env var produces byte-for-byte identical output to `--no-repr-opt`.
- [ ] `NarrowingPolicy::Aggressive` is the default: building without either flag results in `NarrowingPolicy::Aggressive` (verified via tracing output or a unit test on `BuildOptions` default).
- [ ] Zero behavioral change: a representative `.ori` program compiled with and without `--no-repr-opt` produces identical runtime output (same as the dual-exec goal in §12, but exercised in unit form here). Use `tests/benchmarks/bench_small.ori` or any existing AOT test.
- [ ] Phase A fallback: when `ReprPlan` has no entry for a type, `TypeLayoutResolver` falls back to `TypeInfoStore` and produces the same LLVM type as before. Write a Rust unit test that builds a minimal `ReprPlan` with no decisions and verifies `TypeLayoutResolver` output matches a direct `TypeInfoStore` query for each of the 12 primitive tags.
- [ ] All existing tests pass: `./test-all.sh` green. `./llvm-test.sh` green.

---

## 01.4 ReprPlan Query Interface

**File(s):** `compiler/ori_repr/src/query.rs`

Provide ergonomic query methods that later sections will use:

**Phase boundary:** `ori_repr` must NEVER import from `ori_llvm` or `ori_eval`. LLVM-specific convenience methods (e.g., `llvm_int_type(plan, idx, ctx)`) belong in `ori_llvm` as an extension trait (`impl ReprPlanExt for ReprPlan`), not in `ori_repr`.

- [ ] Width and triviality queries:
  ```rust
  impl ReprPlan {
      /// Get the machine integer width for a type (defaults to I64).
      /// Used by §04 (integer narrowing) and §06 (struct layout).
      pub fn int_width(&self, idx: Idx) -> IntWidth { ... }

      /// Get the machine float width for a type (defaults to F64).
      /// Used by §05 (float narrowing) and §07 (enum niche analysis).
      pub fn float_width(&self, idx: Idx) -> FloatWidth { ... }

      // NOTE: LLVM-specific methods like `llvm_int_type(idx, ctx) -> IntType`
      // belong in ori_llvm (e.g., as an extension trait or helper), not in
      // ori_repr, since ori_repr must remain backend-independent.

      /// Is this type trivial (no RC needed)?
      /// Used by §02 (triviality), §08 (escape), §09 (header compression).
      pub fn is_trivial(&self, idx: Idx) -> bool { ... }

      /// Does this value escape its defining function?
      /// `var` is an `ArcVarId` from `ori_arc::ir` — `ori_repr` depends on
      /// `ori_arc` already (for ArcFunction), so this import is clean.
      /// Used by §08 (escape analysis) and §09 (header compression).
      pub fn escapes(&self, func: Name, var: ArcVarId) -> bool { ... }

      /// What RC strategy should be used for this allocation?
      /// Populated by §09 (ARC header compression) and §10 (thread-local ARC).
      /// Returns `RcStrategy::Atomic { width: I64 }` if no decision recorded.
      pub fn rc_strategy(&self, idx: Idx) -> RcStrategy { ... }
  }

  /// Plan-level narrowing policy — set via `--no-repr-opt` or `ORI_NO_REPR_OPT`.
  /// Stored in `ReprPlan` at construction time; every analysis pass checks it.
  /// §04 uses `NarrowingPolicy::Disabled` to skip integer narrowing.
  /// §05 uses it to skip float narrowing.
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum NarrowingPolicy {
      /// All narrowing optimizations enabled (default)
      Aggressive,
      /// Conservative: only narrow when provably safe, no field narrowing
      Conservative,
      /// All narrowing disabled (--no-repr-opt / ORI_NO_REPR_OPT)
      Disabled,
  }

  /// RC strategy for an allocation, set by §09 and §10.
  /// Default (no decision recorded): `Atomic { width: I64 }` — matches
  /// current `ori_rt` behavior exactly, so §01 is a zero-behavioral-change pass.
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum RcStrategy {
      /// No RC needed (trivial or stack-promoted)
      None,
      /// Atomic RC with given header width (thread-shared allocations)
      Atomic { width: IntWidth },
      /// Non-atomic RC (thread-local proven by §10)
      NonAtomic { width: IntWidth },
  }
  ```

- [ ] Add `narrowing_policy` field to `ReprPlan` and expose via constructor:
  ```rust
  pub struct ReprPlan {
      // ... existing fields ...
      /// Narrowing policy — set from --no-repr-opt / ORI_NO_REPR_OPT.
      /// Every analysis pass that performs narrowing MUST check this first.
      narrowing_policy: NarrowingPolicy,
  }

  impl ReprPlan {
      pub fn new(policy: NarrowingPolicy) -> Self { ... }
      pub fn narrowing_policy(&self) -> NarrowingPolicy { self.narrowing_policy }
  }
  ```

**Tests required for §01.4 (write failing tests BEFORE implementing):**

- [ ] `int_width` default: `plan.int_width(int_idx)` returns `IntWidth::I64` when no decision has been recorded for that type.
- [ ] `float_width` default: `plan.float_width(float_idx)` returns `FloatWidth::F64` when no decision recorded.
- [ ] `is_trivial` default: `plan.is_trivial(any_idx)` returns `false` when no triviality decision recorded (safe default — never elides RC it shouldn't).
- [ ] `escapes` default: `plan.escapes(func, var)` returns `true` when no escape info recorded (safe default — never stack-promotes when unsure).
- [ ] `rc_strategy` default: `plan.rc_strategy(any_idx)` returns `RcStrategy::Atomic { width: IntWidth::I64 }` when no decision recorded (matches current `ori_rt` behavior exactly).
- [ ] After `set_rc_strategy(idx, RcStrategy::None, DecisionSource::Triviality)`, `rc_strategy(idx)` returns `RcStrategy::None` (write→read round-trip, distinct from default).
- [ ] `narrowing_policy` round-trip: `ReprPlan::new(NarrowingPolicy::Disabled).narrowing_policy()` returns `NarrowingPolicy::Disabled`.
- [ ] **Semantic pin**: `rc_strategy` default must return `Atomic { I64 }` — NOT `None` and NOT `NonAtomic`. This test must fail if the default is changed. Ensures that §01 alone causes zero behavioral change.

- [ ] Add writer methods for escape info and RC strategy (called by §08 and §09):
  ```rust
  impl ReprPlan {
      /// Record escape info for a function's variables (called by §08).
      /// Replaces the previously empty `escape_info` entry for this function.
      pub fn set_escape_info(&mut self, func: Name, info: EscapeInfo) { ... }

      /// Record the RC strategy for a type (called by §09, §10).
      /// Stores by updating the `MachineRepr::RcPointer` entry for `idx` and
      /// recording a `ReprDecision` with source ArcHeader or ThreadLocal.
      pub fn set_rc_strategy(&mut self, idx: Idx, strategy: RcStrategy, source: DecisionSource) { ... }
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

**Tests required for §01.5 (write failing tests BEFORE implementing):**

- [ ] `canonical()` on `Tag::Var` (unresolved) panics with a message identifying it as a typeck bug (not a silent incorrect result).
- [ ] `canonical()` on `Tag::BoundVar` panics (should never reach codegen).
- [ ] `canonical()` on `Tag::RigidVar` panics (should never reach codegen).
- [ ] `canonical()` on `Tag::Scheme` panics.
- [ ] `canonical()` on `Tag::Infer` panics (unresolved inference variable).
- [ ] `pool.resolve_fully()` round-trip: a `Tag::Named` pointing to `Tag::Int` resolves to `Int { I64, true }` — same as calling `canonical(Tag::Int)` directly.
- [ ] `Option<int>` after resolution produces a 2-variant `Enum` repr with the inner variant holding `Int { I64, true }` — verifies that `pool.resolve_fully()` is called recursively into container inner types.
- [ ] **Edge case**: a `Tag::Named` with a chain of two aliases (`A = B = int`) resolves to `Int { I64, true }` (not `Named` or `Alias`).

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

**File(s):** `compiler/ori_repr/src/repr_attrs.rs`, `compiler/ori_ir/src/ast/items/types.rs` (TypeDecl — needs new field), `compiler/ori_parse/src/grammar/item/type_decl.rs` (parser — needs to wire attrs.repr), `compiler/ori_types/src/check/registration/user_types.rs` (needs to propagate repr through type registration)

The spec (Clause 26 — FFI) defines layout attributes that override the canonical representation:
- `#repr("c")` — C-compatible layout, no field reordering
- `#repr("packed")` — No padding, alignment = 1
- `#repr("transparent")` — Same layout as single field (newtypes)
- `#repr("aligned", N)` — Minimum N-byte alignment (power of two)

These must be threaded into ReprPlan to prevent optimizations from violating user intent.

**Current state — PIPELINE GAP:** `ori_parse` parses `#repr` into `ParsedAttrs.repr: Option<ReprAttr>` (defined in `compiler/ori_parse/src/grammar/attr/mod.rs`). However, the parser does NOT store `repr` in `TypeDecl` (only `derives` is wired). `TypeDecl` in `compiler/ori_ir/src/ast/items/types.rs` has no `repr` field. The `ReprAttr` enum carries `#[allow(dead_code, reason = "variants used when codegen consumes repr attributes")]` — confirming the gap. §01.7 must close this gap end-to-end before `populate_canonical()` can read repr attributes.

**Ordering constraint:** The three GAP-CLOSE steps below modify `ori_ir`, `ori_parse`, and `ori_types` — crates that `ori_repr` will depend on. Implement and `cargo check` these steps BEFORE creating the `ori_repr` crate. If `ori_repr` is created first without `TypeDecl.repr` present, `populate_canonical()` cannot query repr attributes and its implementation will be incomplete from day one.

**Pipeline gap steps required (before the ReprPlan steps below):**

- [ ] **[GAP-CLOSE]** Add `repr: Option<ori_parse::ReprAttr>` to `TypeDecl` in `compiler/ori_ir/src/ast/items/types.rs`:
  ```rust
  // Note: ori_ir depends on ori_parse for TypeDecl's repr field.
  // Alternatively, define a parallel ReprAttrKind in ori_ir to avoid the dep.
  pub struct TypeDecl {
      // ... existing fields ...
      /// Repr attribute from #repr("c"), #repr("packed"), etc.
      pub repr: Option<ReprAttr>,  // ReprAttr defined in ori_ir or re-exported from ori_parse
  }
  ```
  **Preferred approach:** Define a `ReprAttrKind` enum in `ori_ir` (parallel to `ori_parse::ReprAttr`) to avoid creating a dependency from `ori_ir` on `ori_parse` (which would invert the architecture). The parser converts `ori_parse::ReprAttr` → `ori_ir::ReprAttrKind` during AST construction.
  - **Hygiene fix while touching `ori_parse/src/grammar/attr/mod.rs`**: Change `#[allow(dead_code, reason = ...)]` on `ReprAttr` to `#[expect(dead_code, reason = "variants consumed by codegen via ori_ir::ReprAttrKind once §01.7 GAP-CLOSE lands")]` — lint discipline requires `#[expect]` not bare `#[allow]`.

- [ ] **[GAP-CLOSE]** Wire `attrs.repr` through the parser in `compiler/ori_parse/src/grammar/item/type_decl.rs`:
  ```rust
  // In parse_type_decl_body(), add to the TypeDecl constructor:
  ParseOutcome::consumed_ok(TypeDecl {
      // ... existing fields ...
      repr: attrs.repr.map(|r| convert_repr_attr(r)),
  })
  ```

- [ ] **[GAP-CLOSE]** Flow `TypeDecl.repr` through `ori_types` type registration in `compiler/ori_types/src/check/registration/user_types.rs` so it is accessible to `populate_canonical()`. Options: store in `TypeRegistry` keyed by type name/`Idx`, or pass directly to `ori_repr` during plan construction.

- [ ] Define `ReprAttribute` enum in `ori_repr`:
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
      Aligned(u64),
      /// C + Aligned combined (#repr("c") + #repr("aligned", N))
      CAligned(u64),
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

- [ ] Populate during `populate_canonical()` after pipeline gap is closed:
  - After gap-close steps above, `TypeDecl.repr` is available during type registration
  - During canonical population, read the attribute from the type registry and store in `repr_attrs`
  - Validate: `#repr("transparent")` requires exactly one non-ZST field
  - Validate: `#repr("aligned", N)` requires N is a power of two
  - Validate: `#repr("packed")` cannot combine with `#repr("aligned", N)` or `#repr("c")`

**Tests required for §01.7 (write failing tests BEFORE implementing — matrix covers all 4 valid attrs + invalid combos):**

- [ ] `#repr("c")` on a two-field struct: parsed, stored in `ReprPlan.repr_attrs`, `ReprAttribute::C` retrieved via a query.
- [ ] `#repr("packed")` on a struct: `ReprAttribute::Packed` stored and retrieved.
- [ ] `#repr("transparent")` on a single-field newtype struct: `ReprAttribute::Transparent` stored.
- [ ] `#repr("transparent")` on a zero-field struct: validation produces an error (not a panic, not silent success).
- [ ] `#repr("transparent")` on a two-field struct: validation produces an error.
- [ ] `#repr("aligned", 8)` on a struct: `ReprAttribute::Aligned(8)` stored.
- [ ] `#repr("aligned", 3)` — 3 is not a power of two: validation produces an error.
- [ ] `#repr("aligned", 0)` — 0 is not a valid alignment: validation produces an error.
- [ ] `#repr("packed")` + `#repr("aligned", 4)` combined: validation produces an error (cannot combine).
- [ ] `#repr("packed")` + `#repr("c")` combined: validation produces an error.
- [ ] `#repr("c")` + `#repr("aligned", 16)` combined: stored as `ReprAttribute::CAligned(16)` (valid combination).
- [ ] Struct with no `#repr` attribute: `repr_attrs` has no entry for that type (or `ReprAttribute::Default`), and all optimizations are permitted.
- [ ] **Semantic pin for #repr("c")**: a struct with `#repr("c")` must not have its fields reordered by §06. This test establishes the contract: `populate_canonical()` stores `C` in `repr_attrs`, and a subsequent check of `plan.repr_attrs.get(idx)` returns `Some(ReprAttribute::C)`. The actual reorder-blocking is §06 work, but the storage must be correct from §01.

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

**Tests required for §01.8 Phase A (write failing tests BEFORE implementing):**

- [ ] Phase A fallback for each of the 12 primitive tags: construct a `ReprPlan` with no decisions and a `TypeLayoutResolver` wired with both `ReprPlan` (empty) and `TypeInfoStore` (live). For each primitive, verify `TypeLayoutResolver` produces the same LLVM type as `TypeInfoStore` alone.
- [ ] Phase A fallback for composite types (Option, Result, Tuple, Struct, Enum): same as above for the 5 composite type categories.
- [ ] Phase A override: populate `ReprPlan` with a canonical decision for `Tag::Int` (same as what TypeInfoStore would return). Verify `TypeLayoutResolver` uses the `ReprPlan` path (not the fallback) and produces the same result. This establishes that the override path is exercised, not just the fallback path.
- [ ] Phase A with `None` ReprPlan: verify `TypeLayoutResolver::new(store, scx, interner, None)` works correctly (all lookups go through `TypeInfoStore`). This is the backward-compatibility test for existing tests that don't create a ReprPlan.
- [ ] **Semantic pin**: `TypeLayoutResolver` with an empty `ReprPlan` must produce IDENTICAL output to `TypeLayoutResolver` with no `ReprPlan` (i.e., Phase A adds zero behavioral change). Write a test that builds a small `.ori` program, compiles it with Phase A wired, and asserts the LLVM IR is byte-for-byte identical to the pre-Phase-A IR.

---

## 01.9 Canonical Representation Tests

**File(s):** `compiler/ori_repr/src/tests.rs` (sibling to `lib.rs` — `#[cfg(test)] mod tests;` declaration in `lib.rs`, no inline test modules)

Canonical representations are the foundation — if they're wrong, every optimization built on them is wrong.

**TDD ordering:** Write ALL tests in this section BEFORE writing any production code for §01. All tests must fail (crate does not exist). Implement the crate, verify tests pass unchanged. If any test requires modification to pass, the implementation is wrong — fix the implementation, not the test.

**Debug AND release:** After initial passing in debug, run `cargo test -p ori_repr --release` to confirm all tests pass in release mode as well.

- [ ] **Write failing tests first** — `cargo test -p ori_repr` fails with "crate not found" before any production code exists. This is the required starting state.

- [ ] **Primitive roundtrip test:** For each of the 12 primitive Tags (Int, Float, Bool, Str, Char, Byte, Unit, Never, Duration, Size, Ordering, Error), verify `canonical()` produces the expected MachineRepr variant. Every row is a separate `assert_eq!` in the test — missing a row is a gap in the matrix.

- [ ] **Composite type tests:**
  - `Option<int>` → `Enum` with 2 variants, inner is `Int { I64, true }`
  - `Result<int, str>` → `Enum` with 2 variants
  - `(int, bool)` → `Tuple` with 2 elements
  - `[int]` → `FatPointer(Collection { Int { I64, true } })`
  - `{str: int}` → `FatPointer(Collection { ... })`
  - `Set<int>` → `FatPointer(Collection { Int { I64, true } })`

- [ ] **Named type resolution test:** Create a `Named` type pointing to a `Struct`, verify `canonical()` resolves through to the struct's repr.

- [ ] **Storage type equivalence test:** For a Pool containing a representative sample of ALL constructible types, verify that `canonical(tag).to_llvm_type(ctx)` produces the same LLVM type as the existing `TypeInfo::storage_type()`. This is the gold standard: new system must match old system exactly before any optimizations run.
  The minimum required coverage matrix (29 types — must cover ALL rows or the test is incomplete):
  - Primitives (12): `Int`, `Float`, `Bool`, `Str`, `Char`, `Byte`, `Unit`, `Never`, `Duration`, `Size`, `Ordering`, `Error`
  - Simple containers (7): `List`, `Option`, `Set`, `Channel`, `Range`, `Iterator`, `DoubleEndedIterator`
  - Two-child containers (3): `Map`, `Result`, `Borrowed`
  - Complex types (4): `Function`, `Tuple`, `Struct` (with fields), `Enum` (with variants)
  - Named/resolved (3): `Named`→`Struct`, `Applied`→`Struct`, `Alias`→`Int`

- [ ] **Error on unresolved types test:** Verify that `canonical()` on `Tag::Var`, `Tag::BoundVar`, `Tag::RigidVar`, `Tag::Scheme`, `Tag::Infer`, `Tag::SelfType` panics or returns an error. Each variant is a separate `#[should_panic]` test.

- [ ] **FatPointer layout test:** Verify `FatRepr::Str` and `FatRepr::Collection` both produce `{i64, i64, ptr}` in LLVM, matching the existing collection layout.

- [ ] **Semantic pin test:** Write a test that asserts `canonical(Tag::Int) == MachineRepr::Int { width: IntWidth::I64, signed: true }`. This test ONLY passes with the correct canonical mapping. It would fail if `IntWidth::I32` were used, or if the `signed` flag were wrong. This test is the permanent regression guard: if any future change to `canonical()` inadvertently alters integer canonical widths, this test catches it immediately.

- [ ] **Semantic pin test (zero behavioral change):** After §01 is wired into the pipeline (Phase A), compile `tests/benchmarks/bench_small.ori` twice — once with `--no-repr-opt` and once normally (which runs `populate_canonical()` but no narrowing). Assert the LLVM IR output is identical. This test fails if §01 wiring introduces any behavioral change.

- [ ] **Verify tests pass in debug AND release:** `cargo test -p ori_repr` and `cargo test -p ori_repr --release` both green. `cargo test -p ori_llvm` green (equivalence test in §01.8 exercises LLVM IR generation).

---

## 01.10 Completion Checklist

**TDD ordering:** Write ALL tests from §01.2, §01.3, §01.4, §01.5, §01.7, §01.8, and §01.9 BEFORE creating the `ori_repr` crate. All tests must fail (crate does not exist). Create the crate, implement the types, verify tests pass unchanged. Only then proceed to wiring into the pipeline (§01.3). If any test requires modification to pass, the implementation is wrong — fix the implementation, not the test.

- [ ] Write failing tests BEFORE implementation (see §01.9 for the full test list)
- [ ] `ori_repr` added to workspace `Cargo.toml` `[members]` list
- [ ] `ori_repr` added to root workspace `Cargo.toml` `[workspace.dependencies]` as a path dep so downstream crates can reference it with `ori_repr = { workspace = true }` — both entries required
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

**Implementation sequence (must follow this order):**
1. Close the `#repr` pipeline gap (§01.7 GAP-CLOSE steps) — these modify `ori_ir`, `ori_parse`, `ori_types`
2. `cargo check --workspace` green after GAP-CLOSE (before creating `ori_repr`)
3. Create `ori_repr` crate + implement types + tests
4. Add `ori_repr` to workspace + add `ori_llvm/Cargo.toml` dep
5. Wire `ReprPlan` through codegen pipeline
6. Final test run

- [ ] `#repr` pipeline gap closed FIRST: `TypeDecl` in `ori_ir` has `repr: Option<ReprAttrKind>` field, parser wires `attrs.repr`, `ori_types` registration propagates it — `cargo check --workspace` green before proceeding
- [ ] `#repr` attributes (c, packed, transparent, aligned) are parsed and stored in ReprPlan
- [ ] Generic types handled correctly: all type variables resolved before canonical computation
- [ ] Salsa integration: ReprPlan computed imperatively, passed as `&ReprPlan` to codegen
- [ ] `ori_repr` added to `ori_llvm/Cargo.toml` as `ori_repr = { workspace = true }` — required before `cargo check -p ori_llvm` will work with the new import
- [ ] Migration Phase A complete: TypeLayoutResolver accepts optional ReprPlan, falls back to TypeInfoStore
- [ ] `TypeLayoutResolver` in `ori_llvm` reads from `ReprPlan` instead of hardcoded `Tag → LLVM` map
- [ ] Storage type equivalence test passes: canonical representations match existing TypeInfo for all types (29-type matrix from §01.9)
- [ ] `./test-all.sh` green — zero behavioral changes (canonical reprs match existing hardcoded ones)
- [ ] `./clippy-all.sh` green
- [ ] Tracing output shows `ReprPlan query` events at `ORI_LOG=ori_repr=trace`
- [ ] No regressions in `./llvm-test.sh` or `cargo st`
- [ ] **`ValueRange` placeholder:** `ori_repr/src/range/mod.rs` exists and exports `pub struct ValueRange;` (or `pub type ValueRange = ();`) so that `DecisionReason::RangeFits` compiles immediately. This stub is replaced in §03. Checklist item: `cargo check -p ori_repr` passes with the placeholder in place.
- [ ] **`EscapeInfo` placeholder:** `ori_repr/src/escape/mod.rs` exists and exports `pub struct EscapeInfo;` so that `ReprPlan::escape_info: FxHashMap<Name, EscapeInfo>` compiles immediately. Replaced in §08.
- [ ] **`float_width()` query defined** in `query.rs`: `pub fn float_width(&self, idx: Idx) -> FloatWidth` — returns `F64` by default (canonical). Required by §05 (float narrowing) and §07 (f32 niche analysis).
- [ ] **`NarrowingPolicy` enum defined** in `query.rs` or `plan.rs`: `Aggressive`, `Conservative`, `Disabled`. `ReprPlan::new(policy: NarrowingPolicy)` accepts it. `--no-repr-opt` passes `NarrowingPolicy::Disabled`. Required by §04 (integer narrowing) and §05 (float narrowing).
- [ ] **`escapes()` uses `ArcVarId`** (not `VarId`): `pub fn escapes(&self, func: Name, var: ArcVarId) -> bool`. Import `ArcVarId` from `ori_arc::ir`. This is already correct because `ori_repr` depends on `ori_arc`. Verify there is no stray `VarId` type reference.
- [ ] **`FieldRepr.name` field present**: `pub name: Name` on `FieldRepr` (for §06 debug symbols and C-ABI reorder verification). Verify `canonical()` for structs populates this from the type registry field names.
- [ ] **`set_escape_info()` and `set_rc_strategy()` writer methods defined** in `plan.rs` — needed by §08 and §09 to write their results back into `ReprPlan`. Both must be `pub`.
- [ ] **`compute_repr_plan()` signature** accepts `arc_functions: &[ArcFunction]` (from `ori_arc::ir`) in addition to `pool: &Pool` and `policy: NarrowingPolicy`. The `arc_functions` parameter is unused in §01 but the signature is established now to avoid a breaking API change when §03 and §08 add their passes.
- [ ] **All pass stubs defined** in `ori_repr/src/lib.rs`: `analyze_triviality`, `analyze_ranges`, `apply_integer_narrowing`, `apply_float_narrowing`, `compute_struct_layouts`, `compute_enum_reprs`, `analyze_escape`, `compress_arc_headers`, `apply_thread_local_arc`, `specialize_collections` — each takes the appropriate parameters, each body is empty `{}`. These compile-check the future call sites in `compute_repr_plan()` without behavioral change.

**Hygiene fixes to apply along the way (found during §01 codebase scan):**

- [ ] **[DRIFT]** `compiler/ori_llvm/src/codegen/type_info/info.rs` — `TypeInfo::storage_type()` returns silent placeholder values (`{i64, i64}`) for `Option`, `Result`, `Tuple`, `Struct`, `Enum` with no `debug_assert!` or `todo!()`. These are documented as "placeholder — resolved via TypeInfoStore" but there is no invariant enforcement. When §01 (Phase A) adds the ReprPlan fallback, add `debug_assert!(false, "TypeInfo::storage_type() called on Option/Result/Tuple/Struct/Enum — use TypeLayoutResolver instead")` to the placeholder arms so misuse is caught in debug builds. Do this when touching `info.rs` in §01.3.
- [ ] **[WASTE]** `compiler/ori_llvm/src/codegen/type_info/store.rs` — `TypeInfoStore` has `triviality_cache: RefCell<FxHashMap<Idx, bool>>` and `classifying_trivial: RefCell<FxHashSet<Idx>>` fields. These become dead code when §02 lands (triviality migrates to `ReprPlan`). Note them with `// TODO(repr-opt §02): remove triviality_cache and classifying_trivial fields when §02 is complete` comments when touching this file in §01.3. The actual removal is §01.8 Phase B work.
- [ ] **[LINT]** `compiler/oric/src/commands/build_options.rs` line 15 — `#[allow(clippy::struct_excessive_bools, reason = ...)]` must be `#[expect(clippy::struct_excessive_bools, reason = ...)]`. Fix when touching this file in §01.3 (`--no-repr-opt` flag addition).
- [ ] **[LINT]** `compiler/ori_parse/src/grammar/attr/mod.rs` — `#[allow(dead_code, reason = ...)]` on `ReprAttr` must be `#[expect(dead_code, reason = ...)]`. Fix when touching this file in §01.7 GAP-CLOSE.

- [ ] All tests from §01.2, §01.4, §01.5, §01.7, §01.8, §01.9 written and passing in both debug (`cargo test -p ori_repr`) and release (`cargo test -p ori_repr --release`)
- [ ] Semantic pin tests present: at least one test per subsection that would fail if the canonical mapping, default query return values, or Phase A wiring were reverted
- [ ] `./test-all.sh` green (zero regressions across all crates)
- [ ] `./clippy-all.sh` green

**Exit Criteria:** `ori_repr` crate exists, `ReprPlan` is threaded through the entire LLVM codegen pipeline, all existing tests pass with identical behavior, `cargo test -p ori_repr --release` passes, and `ORI_LOG=ori_repr=trace ori build tests/benchmarks/bench_small.ori` shows `ReprPlan query` events for every type in the program.
