---
section: "01"
title: "Representation IR & Decision Framework"
status: in-progress
reviewed: true
third_party_review:
  status: findings
  updated: 2026-03-24
  note: "All TPR findings triaged. TPR-01-038/039 accepted on 2026-03-24 with implementation tasks in §01.10. Status transitions to resolved when all accepted implementation tasks are complete."
goal: "Create the ReprPlan data structure that records all narrowing decisions, integrated into the compilation pipeline between type checking and LLVM codegen"
inspired_by:
  - "Lean4 LCNF phase separation (src/Lean/Compiler/LCNF/)"
  - "Zig InternPool layout interning (src/InternPool.zig)"
  - "Roc STLayoutInterner (crates/compiler/mono/src/layout/intern.rs)"
depends_on: []
sections:
  - id: "01.1"
    title: "MachineRepr Enum & ReprPlan Data Structure"
    status: complete
  - id: "01.2"
    title: "ReprDecision Tracking"
    status: complete
  - id: "01.3"
    title: "Pipeline Integration Point"
    status: complete
  - id: "01.4"
    title: "ReprPlan Query Interface"
    status: complete
  - id: "01.5"
    title: "Generic Type Handling"
    status: complete
  - id: "01.6"
    title: "Salsa Integration Strategy"
    status: complete
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

- [x] Create new crate `ori_repr` with `Cargo.toml` entry
  - Dependencies from §01: `ori_types` (for `Pool`, `Idx`, `Tag`), `ori_ir` (for `Name` — the interned function identifier), `ori_arc` (for `ArcFunction`, `ArcVarId` — needed immediately for `compute_repr_plan()` signature and `escapes()` query), `rustc-hash` (workspace dep — for `FxHashMap`/`FxHashSet`), `tracing` (workspace dep — for `tracing::trace!` in query methods)
  - No dependency on `ori_llvm` — this is backend-independent
  - No dependency on `ori_eval` — this is evaluation-independent
  - Architecture: `ori_types` → `ori_arc` → `ori_repr` → `ori_llvm` (no cycle — `ori_repr` reads from `ori_arc` IR types but `ori_arc` does not depend on `ori_repr`)
  - **Verified**: `ori_types` has `Pool`, `Idx`, `Tag` in its pub API; `rustc-hash` is a workspace dep used by `ori_types`, `ori_arc`, and `ori_llvm`
  - Add `#![deny(unsafe_code)]` to `ori_repr/src/lib.rs` (pure analysis crate, same as `ori_ir`, `ori_types`, `ori_lexer`)

- [x] Define `MachineRepr` enum:
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

- [x] Implement `canonical(tag: Tag, pool: &Pool, idx: Idx) -> MachineRepr` for ALL Tag variants (this is the most critical part of §01 — it defines what "canonical" means for every Tag variant, ensuring the ReprPlan starts correct before any optimization runs):

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
  | `Map` | `FatPointer(FatRepr::Map)` | `{i64, i64, ptr}` | len + cap + data; retains key/value reprs |
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
  The parity test is implemented in §01.9 (storage type equivalence test, 29-type matrix) — not in this subsection.
  See §01.9 for the full coverage matrix and test requirements.

- [x] Define `FatRepr` to distinguish collection/string fat pointers:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
  pub enum FatRepr {
      /// String: {i64 len, i64 cap, ptr data}
      Str,
      /// Collection ([T], {K:V}, Set<T>): {i64 len, i64 cap, ptr data}
      Collection { element_repr: Box<MachineRepr> },
  }
  ```

- [x] Define `ClosureRepr`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
  pub struct ClosureRepr {
      /// Parameter representations
      pub params: Vec<MachineRepr>,
      /// Return representation
      pub ret: Box<MachineRepr>,
  }
  ```

- [x] **[TPR-01-005]** Fix `repr_size()`/`repr_align()` for Unit/Never in aggregate layouts (2026-03-23):
  - Split into `field_size()`/`field_align()` (Unit/Never = 0/1) and `repr_size()`/`repr_align()` (Unit/Never = 8/8 for ABI)
  - `compute_field_layout()`, `compute_payload_layout()`, `canonical_option()`, `canonical_result()` all use `field_size`/`field_align`
  - Semantic pin tests: `((), bool)` = 1 byte, `(bool, (), int)` = 16 bytes, `Option<()>` = 8 bytes, struct(bool, unit) = 1 byte, `(int, Never)` = 8 bytes

- [x] **[TPR-01-007]** Fix `canonical_panics_on_bound_var` test (2026-03-23): constructs a real `BoundVar` via `pool.intern(Tag::BoundVar, 0)` and asserts panic. Separate `canonical_panics_on_rigid_var` test already existed.

- [x] **[TPR-01-015]** Add cycle detection to `canonical()` for recursive user types (2026-03-23):
  - Public `canonical()` wraps `canonical_inner()` with `visiting: &mut FxHashSet<Idx>` cycle detection
  - Recursive positions return `MachineRepr::RcPointer(RcRepr { rc_width: I64, atomic: true, inner: OpaquePtr, stack_promotable: false })`
  - All helper functions (`canonical_collection`, `canonical_map`, `canonical_function`, `canonical_tuple`, `canonical_struct`, `canonical_enum`) thread the visiting set
  - Tests: `type Tree = Leaf(int) | Node(Tree, Tree)` — no stack overflow, Node fields are RcPointer; `type IntList = Nil | Cons(int, IntList)` — semantic pin on RcPointer properties; repeated non-recursive type is NOT treated as cycle

- [x] **[TPR-01-016]** Make `is_trivial_repr()` recursive for compound types (2026-03-23):
  - `Struct(s)` → `s.trivial`, `Tuple(t)` → `t.trivial`, `Enum(e)` → check all variant fields recursively
  - `FatPointer`, `Closure`, `RcPointer`, `OpaquePtr`, `StackPromoted` → always false
  - Tests: struct containing `(int, bool)` is trivial; struct containing `(int, str)` is not; all-unit enum trivial; scalar-payload enum in struct trivial

**Derive requirement:** ALL sub-repr types (`StructRepr`, `EnumRepr`, `TupleRepr`, `FieldRepr`, `EnumTag`, `VariantRepr`, `RcRepr`, `FatRepr`, `ClosureRepr`) MUST derive `Debug, Clone, PartialEq, Eq, Hash` to match `MachineRepr`'s derives. Code blocks below include them explicitly.

**File placement:** `TupleRepr`, `StructRepr`, `FieldRepr`, `RcRepr`, `FatRepr`, `ClosureRepr` → `compiler/ori_repr/src/struct_repr.rs`. `EnumRepr`, `EnumTag`, `VariantRepr` → `compiler/ori_repr/src/enum_repr.rs`. `MachineRepr`, `IntWidth`, `FloatWidth` → `compiler/ori_repr/src/repr.rs`. This matches the file layout table above and keeps all files under 500 lines.

- [x] Define `TupleRepr`:
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

- [x] Define `StructRepr`:
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

- [x] Define `EnumRepr`:
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

- [x] Define `VariantRepr`:
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

- [x] Define `RcRepr`:
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

- [x] Define `ReprDecision`:
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

- [x] Define `ReprPlan` — the central data structure:
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

- [x] Implement builder pattern for populating ReprPlan:
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

- [x] `set_repr` / `get_repr` round-trip: set a decision for `Tag::Int`, retrieve it, assert the `MachineRepr` matches.
- [x] Override behavior: call `set_repr` twice for the same `Idx`; verify `get_repr` returns the second decision's repr.
- [x] Audit trail preservation: after the override above, verify `dump_audit()` contains BOTH entries in insertion order.
- [x] `get_repr` on unknown `Idx` returns `None` (not a panic, not a default).
- [x] `var_range` on a function with no recorded ranges returns the default/top value (not a panic).
- [x] `set_var_ranges` / `var_range` round-trip: record ranges for two functions, verify each function's `var_range` query is isolated.
- [x] `dump_audit` output is non-empty after decisions are recorded and contains the type tag and source in its string representation.

---

## 01.3 Pipeline Integration Point

**File(s):** `compiler/ori_llvm/src/codegen/type_info/mod.rs` (TypeLayoutResolver), `compiler/ori_llvm/src/codegen/type_info/store.rs` (TypeInfoStore — Tag→TypeInfo mapping), `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` (FunctionCompiler), `compiler/ori_llvm/src/evaluator/compile.rs` (JIT entry point), `compiler/oric/src/commands/codegen_pipeline.rs` (AOT entry point — `run_codegen_pipeline()`), `compiler/oric/src/commands/build_options.rs`, `compiler/oric/src/commands/build/mod.rs` (for `--no-repr-opt` CLI flag)

The ReprPlan must be computed AFTER type checking and BEFORE LLVM codegen. The codegen must consume ReprPlan instead of computing representations inline.

- [x] Add `ori_repr` dependency to `ori_llvm/Cargo.toml`

- [x] Create the ReprPlan computation entry point:
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

- [x] Modify `TypeLayoutResolver` in `ori_llvm` to accept `&ReprPlan`:
  - Currently: `TypeLayoutResolver::new(store, scx, interner)` where `store: &TypeInfoStore`, `scx: &SimpleCx`, `interner: Option<&StringInterner>` → reads `TypeInfo` from store (which reads `Tag` from `Pool`)
  - Target: `TypeLayoutResolver::new(store, scx, interner, repr_plan)` → reads `MachineRepr` from plan when available, falling back to `TypeInfo` for unoptimized types
  - Initially, `ReprPlan` returns canonical representations (zero behavioral change)

- [x] Wire `ReprPlan` through the LLVM codegen entry points:
  - JIT path: `OwnedLLVMEvaluator::compile_module_with_tests()` (in `evaluator/compile.rs`) creates `ReprPlan`. Add `narrowing_policy: NarrowingPolicy` as a new last parameter (after `arc_cache`) — callers pass `NarrowingPolicy::Aggressive` by default or `NarrowingPolicy::Disabled` when the test runner sets `ORI_NO_REPR_OPT`.
  - AOT path: `run_codegen_pipeline()` in `compiler/oric/src/commands/codegen_pipeline.rs` creates `ReprPlan` before constructing `FunctionCompiler`. Add `narrowing_policy: NarrowingPolicy` as a new last parameter. This function is called from `compile_common.rs::compile_to_llvm()` and `compile_to_llvm_with_imports()` — both callers must thread the policy through from `BuildOptions.narrowing_policy`.
  - `ReprPlan` is passed to `FunctionCompiler::new()` (there is no `ModuleCompiler` — `FunctionCompiler` is the two-pass declare/define orchestrator). Currently `FunctionCompiler::new()` takes: `builder`, `type_info`, `type_resolver`, `interner`, `pool`, `module_path`, `annotated_sigs`, `arc_classifier`, `debug_context`, `uniqueness_summaries`, `aims_contracts`, `verify_arc` — add `repr_plan: &'a ReprPlan` immediately before `verify_arc` (last position before the boolean flag, following the config-struct convention that booleans come last).
  - `FunctionCompiler` stores `repr_plan` and passes it to `TypeLayoutResolver`

- [x] Add `--no-repr-opt` flag to the `ori build` CLI (`compiler/oric/src/commands/build_options.rs` for the flag definition and `parse_build_options()`, `compiler/oric/src/commands/build/mod.rs` for CLI integration, `compiler/oric/src/commands/codegen_pipeline.rs` for enforcement):
  - Add `narrowing_policy: NarrowingPolicy` field to `BuildOptions` (import `NarrowingPolicy` from `ori_repr`); default to `NarrowingPolicy::Aggressive`
  - Parse `--no-repr-opt` in `parse_build_options()` → set `options.narrowing_policy = NarrowingPolicy::Disabled`
  - Thread `BuildOptions.narrowing_policy` through `compile_common.rs` → `run_codegen_pipeline()` (the new last parameter added above)
  - When `narrowing_policy == NarrowingPolicy::Disabled`, `compute_repr_plan()` returns after `populate_canonical()` (canonical-only plan — zero behavioral change vs today)
  - This flag is required by §12.2 for dual-execution comparison: AOT without optimizations vs. AOT with optimizations
  - Add `ORI_NO_REPR_OPT=1` environment variable as an alternative (same effect as `--no-repr-opt`); check it in `run_codegen_pipeline()` alongside the policy parameter
  - Do NOT use `repr_opt_disabled: bool` — use `NarrowingPolicy` so future conservative mode is also expressible
  - **Hygiene fix while touching this file**: `build_options.rs` line 15 uses `#[allow(clippy::struct_excessive_bools, reason = ...)]` — change to `#[expect(clippy::struct_excessive_bools, reason = ...)]` per lint discipline rules

- [x] Keep `ori_repr` tracing compatible with the existing generic `ORI_LOG` / `RUST_LOG` filter in `compiler/oric/src/tracing_setup.rs`:
  - No tracing registry change is needed today — `tracing_setup.rs` already forwards arbitrary targets through `EnvFilter`
  - Emit `tracing` events from the new crate under target `ori_repr`
  - Add a smoke test or manual verification step showing `ORI_LOG=ori_repr=trace ori build ...` surfaces `ori_repr` events without extra CLI wiring

**Tests required for §01.3 (write failing tests BEFORE implementing):**

- [x] `--no-repr-opt` CLI flag: `ori build --no-repr-opt tests/benchmarks/bench_small.ori` succeeds with exit code 0. Verified via `compute_repr_plan_disabled_policy_skips_stubs` unit test + `./test-all.sh` green.
- [x] `ORI_NO_REPR_OPT=1` env var: env var checked in both `parse_build_options()` and JIT path. Unit test `compute_repr_plan_zero_behavioral_change_with_disabled` verifies identical canonical output.
- [x] `NarrowingPolicy::Aggressive` is the default: unit test `compute_repr_plan_aggressive_is_default_behavior` verifies Aggressive policy + canonical I64 int.
- [x] Zero behavioral change: unit test `compute_repr_plan_zero_behavioral_change_with_disabled` verifies identical canonical representations for all 11 primitives regardless of policy. `./test-all.sh` (13,729 tests) green.
- [x] Phase A fallback: `TypeLayoutResolver` stores `repr_plan` but does not read it yet (Phase A `dead_code` annotation). When `ReprPlan` has canonical-only entries, all existing tests pass unchanged (13,729 green). Full routing in §01.8.
- [x] All existing tests pass: `./test-all.sh` green. `./llvm-test.sh` green.

---

## 01.4 ReprPlan Query Interface

**File(s):** `compiler/ori_repr/src/query.rs`

Provide ergonomic query methods that later sections will use:

**Phase boundary:** `ori_repr` must NEVER import from `ori_llvm` or `ori_eval`. LLVM-specific convenience methods (e.g., `llvm_int_type(plan, idx, ctx)`) belong in `ori_llvm` as an extension trait (`impl ReprPlanExt for ReprPlan`), not in `ori_repr`.

- [x] Width and triviality queries:
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

- [x] Add `narrowing_policy` field to `ReprPlan` and expose via constructor:
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

- [x] `int_width` default: `plan.int_width(int_idx)` returns `IntWidth::I64` when no decision has been recorded for that type.
- [x] `float_width` default: `plan.float_width(float_idx)` returns `FloatWidth::F64` when no decision recorded.
- [x] `is_trivial` default: `plan.is_trivial(any_idx)` returns `false` when no triviality decision recorded (safe default — never elides RC it shouldn't).
- [x] `escapes` default: `plan.escapes(func, var)` returns `true` when no escape info recorded (safe default — never stack-promotes when unsure).
- [x] `rc_strategy` default: `plan.rc_strategy(any_idx)` returns `RcStrategy::Atomic { width: IntWidth::I64 }` when no decision recorded (matches current `ori_rt` behavior exactly).
- [x] After `set_rc_strategy(idx, RcStrategy::None, DecisionSource::Triviality)`, `rc_strategy(idx)` returns `RcStrategy::None` (write→read round-trip, distinct from default).
- [x] `narrowing_policy` round-trip: `ReprPlan::new(NarrowingPolicy::Disabled).narrowing_policy()` returns `NarrowingPolicy::Disabled`.
- [x] **Semantic pin**: `rc_strategy` default must return `Atomic { I64 }` — NOT `None` and NOT `NonAtomic`. This test must fail if the default is changed. Ensures that §01 alone causes zero behavioral change.

- [x] Add writer methods for escape info and RC strategy (called by §08 and §09):
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
- [x] Tracing integration — public query APIs must emit trace events (TPR-01-033) (2026-03-24):
  - [x] Route `int_width`, `float_width`, `is_trivial`, `escapes`, `rc_strategy` through traced wrappers or add inline `tracing::trace!` calls.
  - [x] Remove or wire `get_repr_traced()` — removed (zero callers; query methods now have inline tracing).
  - [x] Verify: `ORI_LOG=ori_repr=debug` shows `populated canonical representations` event. Query-level trace events (int_width, float_width, etc.) will fire when consumers call query methods (§02+).
- [x] **[TPR-01-034]** Fix `BuildOptions::merge()` dropping `link_mode` and `jobs` (2026-03-24):
  - [x] Add `link_mode` merge logic to `BuildOptions::merge()` — only override if parsed value differs from default (`LinkMode::Static`).
  - [x] Add `jobs` merge logic to `BuildOptions::merge()` — `if other.jobs.is_some() { self.jobs = other.jobs; }`.
  - [x] Regression tests: `ori build foo.ori --link=dynamic` → verify `link_mode == Dynamic` survives merge.
  - [x] Regression tests: `ori build foo.ori --jobs=4` → verify `jobs == Some(4)` survives merge.
  - [x] Regression tests: multi-arg accumulation: `ori build foo.ori --link=dynamic --jobs=4 --release` → all three fields survive.
- [x] **[TPR-01-035]** Replace `no_repr_opt: bool` with `NarrowingPolicy` end-to-end (2026-03-24):
  - [x] Replace `BuildOptions.no_repr_opt: bool` with `BuildOptions.narrowing_policy: NarrowingPolicy` (default `Aggressive`).
  - [x] Update `parse_build_options()`: `--no-repr-opt` → `NarrowingPolicy::Disabled`, add `--repr-opt=aggressive|conservative|disabled`.
  - [x] Update `BuildOptions::merge()`: merge `narrowing_policy` field (non-default overrides).
  - [x] Update `compile_to_llvm()` in `compile_common.rs`: accept `NarrowingPolicy` instead of `bool`.
  - [x] Update `run_codegen_pipeline()` in `codegen_pipeline.rs`: accept `NarrowingPolicy` directly (remove bool→enum conversion).
  - [x] Update JIT path `compile_module_with_tests()` in `ori_llvm/src/evaluator/compile.rs`: accept optional `NarrowingPolicy` parameter.
  - [x] Update env var fallback: `NarrowingPolicy::env_disabled()` remains for JIT when no parameter provided.
  - [x] Regression tests: `--no-repr-opt` → `Disabled`, `--repr-opt=conservative` → `Conservative`, default → `Aggressive`.
  - [x] Regression tests: `NarrowingPolicy` survives the full AOT path from CLI to `compute_repr_plan()`.

---

## 01.5 Generic Type Handling

**File(s):** `compiler/ori_repr/src/plan.rs`, `compiler/ori_repr/src/lib.rs`

ReprPlan operates on **monomorphized** types only. Generic types (containing `Var`, `BoundVar`, `RigidVar`) cannot be mapped to concrete machine representations.

- [x] Enforce monomorphization precondition (2026-03-24, pre-existing):
  - `compute_repr_plan()` must be called AFTER monomorphization (all type variables resolved)
  - `canonical()` must assert/panic on `Tag::Var`, `Tag::BoundVar`, `Tag::RigidVar`, `Tag::Scheme`, `Tag::Infer`
  - For `Tag::Named`/`Tag::Applied`/`Tag::Alias`: always resolve via `pool.resolve_fully()` first — if resolution yields a type variable, it's a monomorphization bug

- [x] Handle `Option<T>` and `Result<T, E>` generically (2026-03-24, pre-existing):
  - After monomorphization, `Option<int>` is a concrete type with `Tag::Option` and inner `Idx` pointing to `Tag::Int`
  - The `canonical()` function recurses: `Option<int>` → `Enum(EnumRepr { variants: [Some(Int{I64}), None] })`
  - This works because Pool interning deduplicates: `Option<int>` at two call sites shares the same `Idx`

- [x] Monomorphization boundary (2026-03-24, pre-existing):
  - Currently, Ori does NOT have explicit monomorphization pass — type checker infers concrete types, and Pool stores them
  - The `pool.resolve_fully()` chain handles substitution transparently
  - ReprPlan must call `pool.resolve_fully(idx)` before computing canonical for ANY type to ensure all variables are resolved
  - If `resolve_fully()` returns a variable → skip this type (it's dead code or a typeck bug)

- [x] **[TPR-01-021]** Fix `canonical()` mutual recursion contract violation (2026-03-24):
  - Fixed: Added shared memoization cache (`FxHashMap<Idx, MachineRepr>`) to `canonical_inner()` that persists across `populate_canonical()`. Completed types are cached; subsequent calls return the cached result regardless of traversal order. `try_canonical_cached()` snapshots cache before `catch_unwind` to prevent partial entries on panic.
  - Verify: `canonical_mutual_recursion_consistent` test passes — nested B inside A matches standalone B.

**Tests required for §01.5 (write failing tests BEFORE implementing):**

- [x] `canonical()` on `Tag::Var` (unresolved) panics with a message identifying it as a typeck bug (pre-existing: `canonical_panics_on_var`).
- [x] `canonical()` on `Tag::BoundVar` panics (pre-existing: `canonical_panics_on_bound_var`).
- [x] `canonical()` on `Tag::RigidVar` panics (pre-existing: `canonical_panics_on_rigid_var`).
- [x] `canonical()` on `Tag::Scheme` panics (2026-03-24: `canonical_panics_on_scheme`).
- [x] `canonical()` on `Tag::Infer` panics (2026-03-24: `canonical_panics_on_infer`).
- [x] `pool.resolve_fully()` round-trip: Named→Int resolves to `Int { I64, true }` (2026-03-24: `canonical_named_resolves_to_int`).
- [x] `Option<int>` after resolution produces a 2-variant `Enum` repr (pre-existing: `canonical_option_int`).
- [x] **Edge case**: Named chain A→B→Int resolves to `Int { I64, true }` (2026-03-24: `canonical_alias_chain_resolves`).

---

## 01.6 Salsa Integration Strategy

**File(s):** `compiler/ori_repr/src/lib.rs`, `compiler/oric/src/commands/codegen_pipeline.rs`

The ReprPlan must integrate with the existing Salsa-based compilation model.

- [x] **ReprPlan is NOT a Salsa tracked struct** — it is computed imperatively:
  - Salsa works best for demand-driven, memoizable queries (parsing, type checking)
  - ReprPlan computation is a forward pass that mutates state across multiple analysis phases (triviality → range → narrowing → layout)
  - Making each phase a Salsa query would create artificial dependencies and complicate the multi-pass mutation pattern
  - Instead: compute ReprPlan once, pass it as `&ReprPlan` to codegen (same model as how `TypeInfoStore` works today)
  - Verified: `compute_repr_plan()` is a pure function in `lib.rs:52`, not `#[salsa::tracked]`. AOT path at `codegen_pipeline.rs:317`, JIT path at `evaluator/compile.rs:169`. (2026-03-24)

- [x] **Invalidation model:**
  - ReprPlan is invalidated when the Pool changes (new/modified types)
  - In the current compilation model, this means: recompute ReprPlan on every compilation
  - Future optimization: if Pool didn't change (Salsa cache hit on type checking), reuse previous ReprPlan
  - This can be implemented as a Salsa query that takes Pool hash → ReprPlan, memoized by Pool identity
  - Verified: both AOT and JIT paths call `compute_repr_plan()` fresh on each compilation — no caching. Documented in `plan.rs` and `lib.rs` module docs. (2026-03-24)

- [x] **JIT hot-reload compatibility:**
  - JIT recompiles individual functions — the ReprPlan for unchanged functions is stable
  - When a function's type signature changes, only that function's entries need recomputation
  - For now: recompute entire ReprPlan per JIT invocation (same as TypeInfoStore today)
  - Future: incremental ReprPlan updates keyed by function-level Merkle hashes
  - Verified: `OwnedLLVMEvaluator::compile_module_with_tests()` at `evaluator/compile.rs:169` recomputes `ReprPlan` per invocation. (2026-03-24)

- [x] **Thread safety:**
  - ReprPlan is immutable after computation — `&ReprPlan` is `Send + Sync`
  - No interior mutability needed (unlike TypeInfoStore which uses RefCell for lazy population)
  - All analysis passes write to a `&mut ReprPlan` during computation, then freeze it for codegen
  - Verified: compile-time `Send + Sync` assertion added to `plan.rs`. All fields are `FxHashMap`/`Vec` — zero `RefCell`/`Mutex`. Contrasts with `TypeInfoStore` which has 4 `RefCell` fields. (2026-03-24)

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
  - This ensures zero behavioral change for the fallback path: when ReprPlan has no entry, TypeLayoutResolver delegates to TypeInfoStore as before. Note: `canonical()` intentionally diverges from TypeInfoStore for Unit/Never in aggregates (zero-sized vs i64) — this divergence only surfaces when ReprPlan decisions are actively consulted, not via the fallback path

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
  - Phase A: `assert_eq!(repr_plan.canonical(tag).to_llvm_type(), type_info.storage_type())` for all types WITHOUT Unit/Never in aggregates. For ZST-containing composites (`Option<()>`, `((), bool)`, structs with Unit fields), `canonical()` intentionally produces smaller layouts (zero-sized fields) — verify the canonical layout is correct per §01.9's ZST aggregate tests, and document the divergence from TypeInfoStore.
  - Phase B: same split + `assert_eq!(repr_plan.is_trivial(idx), type_info_store.is_trivial(idx))`
  - Phase C: remove TypeInfoStore from production; tests use ReprPlan directly — divergence disappears

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

- [ ] **Mutual recursion canonical-consistency test (TPR-01-021, TPR-01-037):** Build a mutually recursive SCC such as `type A = WrapA(B)` / `type B = WrapB(A)` (or equivalent struct/enum wrappers). Test via `canonical_cached()` with shared cache (the production contract). Assert each `Idx` has one stable representation regardless of which root was canonicalized first. Also: narrow `canonical()` from `pub` to `pub(crate)` — standalone calls without shared cache are not part of the public contract, only `compute_repr_plan()`/`populate_canonical()` guarantees SCC consistency. Update doc comments to clarify this. The existing test at `tests.rs:973-1033` already uses `canonical_cached()` — verify it is a true semantic pin (would fail if shared memoization were removed).

- [ ] **Storage type equivalence test:** For a Pool containing a representative sample of ALL constructible types, verify `canonical(tag)` matches expectations. Split into two groups:
  - **Exact-match group** (types without Unit/Never in aggregates): verify `canonical(tag).to_llvm_type(ctx)` produces the same LLVM type as `TypeInfo::storage_type()`. This is the gold standard for the majority of types.
  - **Expected-divergence group** (composites with Unit/Never fields — `Option<()>`, `((), bool)`, `Result<(), int>`, struct with Unit field): verify `canonical()` produces the correct zero-sized layout per the ZST aggregate model (§01.9 ZST tests), and document that TypeInfoStore produces larger i64-based layouts for these. The divergence is intentional — `canonical()` is correct, TypeInfoStore is legacy.
  The minimum required coverage matrix (29 types — must cover ALL rows or the test is incomplete):
  - Primitives (12): `Int`, `Float`, `Bool`, `Str`, `Char`, `Byte`, `Unit`, `Never`, `Duration`, `Size`, `Ordering`, `Error`
  - Simple containers (7): `List`, `Option`, `Set`, `Channel`, `Range`, `Iterator`, `DoubleEndedIterator`
  - Two-child containers (3): `Map`, `Result`, `Borrowed`
  - Complex types (4): `Function`, `Tuple`, `Struct` (with fields), `Enum` (with variants)
  - Named/resolved (3): `Named`→`Struct`, `Applied`→`Struct`, `Alias`→`Int`
  - ZST-divergence cases (4+): `Option<()>`, `((), bool)`, `Result<(), int>`, `Struct { x: (), y: int }` — assert correct zero-sized layout, note divergence from TypeInfoStore

- [ ] **Zero-sized type aggregate tests (TPR-01-005):** Verify aggregates containing Unit/Never use zero-sized field layout:
  - `((), bool)` → Tuple with size 1 (Unit contributes 0 bytes)
  - `(bool, (), int)` → Tuple with size 16 (Unit contributes 0 bytes, align-padded)
  - Struct with `{ x: (), y: int }` → size 8 (Unit field is zero-sized)
  - `Option<()>` → Enum tag + 0 payload (Unit variant has no data)
  - `(Never, int)` → Never variant is zero-sized in aggregate context
  - **Semantic pin**: `((), bool)` size must NOT be 16 — this test fails if Unit is treated as 8-byte in aggregates

- [ ] **Enum triviality semantic pin (TPR-01-017):** Replace or augment `trivial_all_unit_enum` with a wrapper-aggregate test: create a struct containing an all-unit enum, call `canonical()`, assert the struct's `trivial` flag is `true`. This exercises `is_trivial_repr()` on the `MachineRepr::Enum` path. A regression in the enum branch would make the struct non-trivial, failing this test. Keep in debug/release matrix.

- [ ] **BoundVar test fix (TPR-01-007):** Rename `canonical_panics_on_bound_var` to `canonical_panics_on_rigid_var` (matches what it actually tests). Add a new `canonical_panics_on_bound_var` test that constructs a real `BoundVar` fixture via `pool.scheme()` extraction and asserts `canonical()` panics on it.

- [ ] **Error on unresolved types test:** Verify that `canonical()` on `Tag::Var`, `Tag::BoundVar`, `Tag::RigidVar`, `Tag::Scheme`, `Tag::Infer`, `Tag::SelfType` panics or returns an error. Each variant is a separate `#[should_panic]` test.

- [ ] **FatPointer layout test:** Verify `FatRepr::Str` and `FatRepr::Collection` both produce `{i64, i64, ptr}` in LLVM, matching the existing collection layout.

- [ ] **Semantic pin test:** Write a test that asserts `canonical(Tag::Int) == MachineRepr::Int { width: IntWidth::I64, signed: true }`. This test ONLY passes with the correct canonical mapping. It would fail if `IntWidth::I32` were used, or if the `signed` flag were wrong. This test is the permanent regression guard: if any future change to `canonical()` inadvertently alters integer canonical widths, this test catches it immediately.

- [ ] **Semantic pin test (zero behavioral change):** After §01 is wired into the pipeline (Phase A), compile `tests/benchmarks/bench_small.ori` twice — once with `--no-repr-opt` and once normally (which runs `populate_canonical()` but no narrowing). Assert the LLVM IR output is identical. This test fails if §01 wiring introduces any behavioral change.

- [ ] **Verify tests pass in debug AND release:** `cargo test -p ori_repr` and `cargo test -p ori_repr --release` both green. `cargo test -p ori_llvm` green (equivalence test in §01.8 exercises LLVM IR generation).

---

## 01.R Third Party Review Findings

- [x] `[TPR-01-001][high]` `compiler/ori_repr/src/canonical.rs:148` — Aggregate size accounting ignores ABI padding for tuples, structs, and enum payloads.
  Resolved: Accepted and fixed on 2026-03-23. Replaced naive `estimate_size()` (field-size sum) with `compute_field_layout()` and `compute_payload_layout()` that walk fields with alignment padding between each field plus trailing padding to struct alignment. `TupleRepr::to_machine_repr()` and enum variant sizing also updated. Matrix tests: `(int, bool)=16`, `(bool, int)=16`, `(bool, bool)=2`, `struct(bool, int)=16`, `struct(int, float)=16`. Debug+release pass.

- [x] `[TPR-01-002][high]` `compiler/ori_repr/src/struct_repr.rs:69` — `FatRepr::Collection` cannot faithfully represent maps because it stores only one element repr.
  Resolved: Accepted and fixed on 2026-03-23. Added `FatRepr::Map { key_repr, value_repr }` variant. `canonical()` for `Tag::Map` now uses both `pool.map_key()` and `pool.map_value()`. `FatRepr::Collection` is now only for single-element collections (List, Set). Semantic pin test `canonical_map_retains_value_repr` verifies both key and value are preserved.

- [x] `[TPR-01-003][high]` `compiler/ori_repr/src/canonical.rs:203` — Impossible-type paths are silently rewritten to `OpaquePtr` instead of failing fast.
  Resolved: Accepted and fixed on 2026-03-23. Replaced `debug_assert!(false) + OpaquePtr` fallback with `panic!()` for Named/Applied/Alias, Borrowed, and Error. These now fail fast in both debug and release builds, consistent with Var/BoundVar/RigidVar/Scheme/etc. which already used `panic!()`. Added `#[should_panic] canonical_panics_on_error` test.

- [x] `[TPR-01-004][medium]` `plans/repr-opt/section-01-repr-ir.md:16` — §01.1 is marked complete even though the foundational API surface it claims to establish does not exist yet.
  Resolved: Rejected after validation on 2026-03-23. The file layout table in §01.1 describes the entire §01 crate across all subsections, not §01.1's scope. The 10 checked items under §01.1 are specifically the type definitions (MachineRepr, StructRepr, TupleRepr, etc.) and `canonical()`, all of which exist. The items the TPR references (plan.rs, query.rs, compute_repr_plan, NarrowingPolicy) are in §01.2–§01.4 and are correctly unchecked there.

- [x] `[TPR-01-005][high]` `compiler/ori_repr/src/canonical.rs:334` — Aggregate layout treats `Unit`/`Never` as 8-byte fields even though §01.1 defines `MachineRepr::Unit` as zero-sized in memory.
  Resolved: Accepted on 2026-03-23. Fix tasks integrated into §01.1 (split `repr_size`/`repr_align` into field-layout vs ABI-value variants so Unit/Never are zero-sized in aggregates) and §01.9 (aggregate tests with Unit/Never).

- [x] `[TPR-01-006][medium]` `compiler/ori_repr/src/tests.rs:331` — §01.1 claims parity validation against the existing `TypeInfo` pipeline, but the new crate does not contain the promised parity test.
  Resolved: Accepted on 2026-03-23. The parity test is tracked in §01.9 (storage type equivalence test, 29-type matrix). §01.1's validation text (line 226) is aspirational — the implementation is §01.9 scope. Cross-reference clarification added to §01.1.

- [x] `[TPR-01-007][low]` `compiler/ori_repr/src/tests.rs:653` — The test named `canonical_panics_on_bound_var` never exercises a `BoundVar`.
  Resolved: Accepted on 2026-03-23. Fix task integrated into §01.9 (rename test to match actual coverage, add real BoundVar fixture test).

- [x] `[TPR-01-008][high]` `compiler/ori_repr/src/canonical.rs:249` — Aggregate layout still uses ABI-sized `Unit`/`Never` fields, so the accepted zero-sized-layout fix has not landed.
  Resolved: Validated and confirmed on 2026-03-23. Fix tasks already integrated into §01.1 (line 253: split repr_size/repr_align into field vs ABI variants) and §01.9 (line 999: zero-sized aggregate semantic pins). Will be fixed as part of §01.1 completion.

- [x] `[TPR-01-009][medium]` `compiler/ori_repr/src/tests.rs:653` — `canonical_panics_on_bound_var` still panics via `RigidVar`, so `Tag::BoundVar` has no direct regression test.
  Resolved: Validated and confirmed on 2026-03-23. Fix tasks already integrated into §01.1 (line 260: rename test, add real BoundVar fixture) and §01.9 (line 1007: BoundVar test fix). Will be fixed as part of §01.1 completion.

- [x] `[TPR-01-010][high]` `compiler/ori_repr/src/canonical.rs:249` — Aggregate layout still treats `Unit`/`Never` as 8-byte payload fields, so zero-sized aggregates are mis-modeled.
  Resolved: Validated and accepted on 2026-03-23. Fix tasks already integrated into §01.1 (line 253: field_size/field_align vs abi_size/abi_align split) and §01.9 (line 999: zero-sized aggregate semantic pins). Will be fixed as part of §01.1 completion.

- [x] `[TPR-01-011][medium]` `compiler/ori_repr/src/tests.rs:653` — `canonical_panics_on_bound_var` still does not exercise a `BoundVar`.
  Resolved: Validated and accepted on 2026-03-23. Fix tasks already integrated into §01.1 (line 260: rename test, add real BoundVar fixture) and §01.9 (line 1007: BoundVar test fix). Will be fixed as part of §01.1 completion.

- [x] `[TPR-01-012][medium]` `plans/repr-opt/section-01-repr-ir.md:6` — The section metadata says third-party review is resolved even though the accepted §01 findings above are still open in the current tree.
  Resolved: Already addressed on 2026-03-23. Frontmatter was corrected to `third_party_review.status: findings` before this triage. Now that all TPR items are triaged (fix tasks integrated into §01.1 and §01.9), status updated to `resolved`.

- [x] `[TPR-01-013][high]` `plans/repr-opt/section-01-repr-ir.md:6` — §01 still advertises `third_party_review.status: resolved` while the accepted zero-sized-layout and BoundVar-test fixes remain unchecked in the same section.
  Resolved: Validated on 2026-03-23. The frontmatter was already corrected to `third_party_review.status: findings` in a prior session. The principle (keep `findings` until accepted fixes land) is actively being followed. The underlying process issue is addressed by TPR-01-014.

- [x] `[TPR-01-014][medium]` `.claude/skills/continue-roadmap/SKILL.md:158` — The roadmap workflow resolves TPR state immediately after triage, even when accepted findings are converted into new unchecked implementation tasks.
  Resolved: Accepted and fixed on 2026-03-23. Updated SKILL.md Step 1.9 to keep `third_party_review.status: findings` while accepted TPR findings have unchecked implementation tasks. Status only transitions to `resolved` when all accepted implementation tasks are complete or when all findings were rejected.

- [x] `[TPR-01-015][high]` `compiler/ori_repr/src/canonical.rs:25` — `canonical()` has no cycle handling, so recursive user types recurse forever instead of preserving the existing boxed-recursion behavior.
  Resolved: Accepted on 2026-03-23. Validated against codebase — `canonical()` has no `visiting` set or cycle detection. Recursive ADTs (e.g., `type Tree = Leaf(int) | Node(Tree, Tree)`) will stack overflow. Implementation tasks added to §01.1 (cycle detection) and §01.9 (recursive type tests).

- [x] `[TPR-01-016][medium]` `compiler/ori_repr/src/canonical.rs:126` — Nested trivial aggregates are marked non-trivial because `is_trivial_repr()` only recognizes primitive leaves.
  Resolved: Accepted on 2026-03-23. Validated against codebase — `is_trivial_repr()` only matches primitive variants, so nested `Struct`/`Tuple`/`Enum` fields always yield `false` even when all-scalar. Fix: make `is_trivial_repr()` recursive. Implementation tasks added to §01.1 (recursive triviality) and §01.9 (nested trivial aggregate tests).

- [x] `[TPR-01-017][medium]` `compiler/ori_repr/src/tests.rs:1039` — `trivial_all_unit_enum` is not a semantic pin for enum triviality.
  Resolved: Validated and accepted on 2026-03-23. The test manually checks variant fields against pointer types but never exercises `is_trivial_repr()` or wraps the enum in a struct to test the struct's `trivial` flag. Implementation task added to §01.9 (enum triviality semantic pin test).

- [x] `[TPR-01-018][medium]` `compiler/ori_repr/src/canonical.rs:247` — The accepted zero-sized aggregate model for `Unit`/`Never` no longer matches the current `TypeInfoStore` / `TypeLayoutResolver` fallback, so §01.8's "empty ReprPlan = zero behavioral change" migration cannot pass as written.
  Resolved: Accepted on 2026-03-23. Validated against codebase — `canonical()` uses `field_size(Unit)=0` while `TypeInfoStore` lowers Unit/Never to i64 (8 bytes) in all contexts. Divergence is intentional: zero-sized aggregate layout is the correct model. Plan updates applied to §01.8 (Phase A validation split into exact-match and expected-divergence groups) and §01.9 (storage type equivalence test split to document ZST aggregate divergence).

- [x] `[TPR-01-019][medium]` `plans/repr-opt/section-01-repr-ir.md:6` — The section re-resolves third-party review even though the enum triviality semantic pin is still missing in the current tree.
  Resolved: Validated on 2026-03-23. The concern is correct — `third_party_review.status` is already `findings` (not `resolved`), and the enum triviality semantic pin implementation task is tracked at §01.9 (line 1019). The TPR status correctly reflects that accepted work remains. No additional plan changes needed.

- [x] `[TPR-01-020][low]` `plans/repr-opt/section-01-repr-ir.md:187` — §01.1 still documents `Tag::Map` as `FatPointer(FatRepr::Collection)` after the implementation and prior TPR changed it to `FatRepr::Map`.
  Resolved: Validated and fixed on 2026-03-23. Updated §01.1 canonical mapping table: `Map` row now reads `FatPointer(FatRepr::Map)` with note about key/value repr retention, matching `canonical.rs:131`.

- [x] `[TPR-01-021][high]` `compiler/ori_repr/src/canonical.rs:36` — `canonical()` still violates its own “one `MachineRepr` per `Idx`” contract for mutually recursive types because cycle handling is scoped to the current DFS path.
  Evidence: `canonical()` starts every root query with a fresh `FxHashSet` and `canonical_inner()` only substitutes `RcPointer` when it hits the current traversal’s back-edge. For a mutually recursive SCC `A -> B -> A`, `canonical(A)` embeds `B { a: RcPointer }`, while a separate `canonical(B)` query embeds `A { b: RcPointer }`; that means the shape of `B` depends on which root was canonicalized, contradicting `repr.rs:34`.
  Impact: ReprPlan population over a mutually recursive component can cache root-dependent layouts, breaking equality/hash stability and any later pass that assumes nested uses of an `Idx` match its standalone canonical form.
  Resolved: Accepted on 2026-03-23. Finding validated — `canonical()` at line 28 creates fresh `FxHashSet` per root call, so nested representations of the same Idx differ depending on DFS entry point. Implementation task added to §01.5 (SCC-aware memoization fix). Regression test already tracked at §01.9 line 1000.

- [x] `[TPR-01-022][high]` `compiler/ori_repr/src/plan.rs:129` — `set_rc_strategy()` overwrites the type’s representation with a placeholder RC shell and discards the original layout.
  Resolved: Fixed on 2026-03-23. Added `rc_strategies: FxHashMap<Idx, RcStrategy>` to `ReprPlan` as separate metadata. `set_rc_strategy()` now writes to `rc_strategies` map and records audit entry without calling `set_repr()`. Removed dead `RcStrategy::to_machine_repr()`. 7 tests added: round-trip, repr preservation semantic pin, and audit trail verification.

- [x] `[TPR-01-023][high]` `compiler/ori_repr/src/plan/query.rs:109` — `rc_strategy()` silently reports `RcStrategy::None` for any stored `MachineRepr::OpaquePtr`, including canonical iterator/channel types with no RC decision.
  Resolved: Fixed on 2026-03-23. `rc_strategy()` now reads from the dedicated `rc_strategies` map instead of pattern-matching on `MachineRepr`. Returns `Atomic { I64 }` default when no explicit RC decision exists. Semantic pin test `rc_strategy_default_for_canonical_opaque_ptr` verifies canonical OpaquePtr types get the safe default.

- [x] `[TPR-01-024][medium]` `compiler/ori_repr/src/plan/query.rs:39` — §01.4 query defaults landed without the section’s required regression tests for `int_width`, `float_width`, `is_trivial`, `escapes`, or `narrowing_policy`.
  Evidence: `compiler/ori_repr/src/plan/query.rs` defines all five APIs, but `compiler/ori_repr/src/tests.rs` only adds the `rc_strategy` subset of the §01.4 test matrix. The required checks at `plans/repr-opt/section-01-repr-ir.md:731-740` remain unchecked, and `rg` over `compiler/ori_repr/src/tests.rs` finds no coverage for `plan.int_width(...)`, `plan.float_width(...)`, `plan.is_trivial(...)`, `plan.escapes(...)`, or `plan.narrowing_policy()`.
  Impact: These methods encode the crate’s zero-behavior-change safety defaults for later narrowing and ARC passes. Without the promised pins, a future refactor can silently change canonical widths or escape/triviality fallbacks without any direct test failure.
  Required plan update: Add the missing §01.4 failing-first tests before treating the query interface as landed, then check the corresponding checklist items.
  Resolved: Accepted on 2026-03-23. Validated — `tests.rs` only tests `IntWidth`/`FloatWidth` enum sizes and `rc_strategy` round-trips, not the `ReprPlan` query interface defaults. The missing tests are already tracked at §01.4 checklist items (lines 733-740). These tests are required before §01.4 can be marked complete.

- [x] `[TPR-01-025][low]` `plans/repr-opt/section-01-repr-ir.md:25` — The section metadata still marks §01.4 as `not-started` even though its query API and policy surface are already in the tree.
  Evidence: `plans/repr-opt/section-01-repr-ir.md:25-27` keeps `01.4` at `not-started`, but `compiler/ori_repr/src/plan/query.rs` already implements `NarrowingPolicy`, `RcStrategy`, `int_width`, `float_width`, `is_trivial`, `escapes`, `rc_strategy`, and `narrowing_policy`, while `compiler/ori_repr/src/plan.rs` already stores `narrowing_policy` and `rc_strategies`.
  Impact: The plan no longer matches the repository state, which hides cross-section work already landed and makes the remaining §01.4 scope harder to reason about during later review and implementation.
  Required plan update: Mark §01.4 `in-progress` (or split out the already-landed items) and reconcile its checklist against the current code before more section work proceeds.
  Resolved: Accepted on 2026-03-23. Updated §01.4 frontmatter to `in-progress` to match codebase reality.

- [x] `[TPR-01-026][medium]` `plans/repr-opt/section-01-repr-ir.md:6` — The section frontmatter re-resolves third-party review even though accepted TPR follow-up work is still open in this section.
  Evidence: `third_party_review.status` is currently `resolved`, but §01.5 still carries unchecked TPR-01-021 follow-up work (`Fix canonical() mutual recursion contract violation`), the matching §01.9 regression test is still unchecked, and the §01.4 test checklist items that TPR-01-024 accepted remain unchecked.
  Impact: `/continue-roadmap` now relies on `third_party_review.status: findings` to surface accepted-but-incomplete review work before new implementation proceeds. Marking the section `resolved` hides active review debt and makes the section look cleaner than the tree actually is.
  Required plan update: Keep `third_party_review.status: findings` until the accepted TPR-01-021 and TPR-01-024 follow-up work lands and is revalidated, then resolve the review state in the same edit pass.
  Resolved: Validated on 2026-03-24. Current frontmatter already shows `third_party_review.status: findings` — the evidence referenced stale state. No change needed.

- [x] `[TPR-01-027][high]` `compiler/oric/src/commands/build_options.rs:109` — `ori build --no-repr-opt ...` never preserves the new flag through the actual CLI parsing path.
  Evidence: `parse_build_options()` sets `options.no_repr_opt = true` for `--no-repr-opt` at `compiler/oric/src/commands/build_options.rs:394-395`, but `main()` parses build arguments one token at a time and merges them (`compiler/oric/src/main.rs:81-88`). `BuildOptions::merge()` only ORs `lib`, `dylib`, `wasm`, `js_bindings`, `wasm_opt`, and `verbose` (`compiler/oric/src/commands/build_options.rs:159-165`); it never merges `no_repr_opt`, so the parsed flag is dropped before `compile_to_llvm()` / `run_codegen_pipeline()` see it.
  Impact: The advertised AOT kill switch for §12 dual-exec baselines is nonfunctional: `ori build --no-repr-opt file.ori` still computes `NarrowingPolicy::Aggressive`, so future repr-opt regressions cannot be bisected or compared against the canonical-only path.
  Required plan update: Merge `no_repr_opt` alongside the other boolean build flags and add a regression test that exercises the real one-arg-at-a-time `ori build` parsing flow.
  Resolved: Fixed on 2026-03-24. Added `self.no_repr_opt |= other.no_repr_opt;` to `BuildOptions::merge()`. 6 regression tests added in `build_options/tests.rs` including per-arg merge loop simulation and exhaustive boolean flag coverage.

- [x] `[TPR-01-028][medium]` `compiler/oric/src/commands/codegen_pipeline.rs:317` — `ORI_NO_REPR_OPT=1` is not reliably honored by the AOT build path.
  Evidence: The env var is only read inside `parse_build_options()` (`compiler/oric/src/commands/build_options.rs:399-401`), but `ori build file.ori` with no extra CLI flags never calls that parser loop (`compiler/oric/src/main.rs:79-88`). Even when the parser does run, `run_codegen_pipeline()` derives the policy solely from the merged `no_repr_opt` boolean (`compiler/oric/src/commands/codegen_pipeline.rs:317-321`) and never re-checks `ORI_NO_REPR_OPT`, contrary to the section's documented requirement.
  Impact: The documented environment-variable escape hatch works for the compiled-run path but not for plain AOT builds, so scripts and manual verification cannot rely on `ORI_NO_REPR_OPT=1 ori build ...` to force the canonical-only baseline.
  Required plan update: Enforce the env override in the AOT build/codegen path itself and add a regression test for `ori build` with no CLI options plus `ORI_NO_REPR_OPT=1`.
  Resolved: Fixed on 2026-03-24. Added unconditional `ORI_NO_REPR_OPT` env var check in `main.rs` build handler after the options loop, covering the zero-arg case where `parse_build_options()` is never called. Combined with TPR-01-027 merge fix, all build paths now correctly honor both `--no-repr-opt` and `ORI_NO_REPR_OPT=1`.

- [x] `[TPR-01-029][medium]` `compiler/oric/src/main.rs:92` — TPR-01-028 was resolved without the regression test the section itself requires for the zero-option `ORI_NO_REPR_OPT=1 ori build ...` path.
  Evidence: The section says TPR-01-028 is resolved, but its own required plan update demands “a regression test for `ori build` with no CLI options plus `ORI_NO_REPR_OPT=1`”. The only new coverage is `compiler/oric/src/commands/build_options/tests.rs:1-102`, which tests `BuildOptions::merge()` and `parse_build_options()`; no test exercises the real `main.rs` build-command loop or the unconditional env override at `compiler/oric/src/main.rs:92-97`. `rg -n “ORI_NO_REPR_OPT|no_repr_opt|--no-repr-opt” compiler/oric/src -g’*tests.rs’ -g’*.rs’` finds no such test outside production code.
  Impact: The code path currently looks correct by inspection, but the exact CLI regression that motivated TPR-01-028 is still unpinned. A future refactor can silently re-break `ORI_NO_REPR_OPT=1 ori build file.ori` while the plan claims the issue is closed.
  Required plan update: Add a regression test that drives the actual build-command path with zero CLI options and `ORI_NO_REPR_OPT=1`, then revalidate TPR-01-028 in the same edit pass before returning this section to `third_party_review.status: resolved`.
  Resolved: Fixed on 2026-03-24. All 4 call sites now use `NarrowingPolicy::env_disabled()` (the canonical helper in `ori_repr`). 12 regression tests added in `ori_repr/src/tests.rs` covering truthy values (`1`, `true`, `TRUE`, `True`, `yes`, `YES`), falsey values (`0`, `false`, `no`, empty, arbitrary), and a semantic pin test. The inner `is_env_truthy()` function is tested directly (avoids env-var mutation races), and all call sites use the same helper — so testing the helper pins the behavior for all call sites.

- [x] `[TPR-01-030][medium]` `compiler/oric/src/commands/build_options/mod.rs:400` — `ORI_NO_REPR_OPT` is enabled on mere presence, not on the documented `=1` value.
  Evidence: The section documents `ORI_NO_REPR_OPT=1` as the environment-variable escape hatch, but both `parse_build_options()` (`compiler/oric/src/commands/build_options/mod.rs:400-402`) and the new unconditional build-path override (`compiler/oric/src/main.rs:95-96`) use `std::env::var(“ORI_NO_REPR_OPT”).is_ok()`. The JIT path in `compiler/ori_llvm/src/evaluator/compile.rs:155` does the same. As written, `ORI_NO_REPR_OPT=0` or `ORI_NO_REPR_OPT=false` still disables repr-opt.
  Impact: Shell profiles and CI scripts cannot safely leave the variable set to a falsey value; the kill switch activates more broadly than documented, making perf/verification runs harder to trust and the AOT/JIT interface harder to reason about.
  Required plan update: Parse the env var through one shared helper that accepts explicit enabled values (`1`/`true`) and falsey values (`0`/`false`/unset), use it in both AOT and JIT entry points, and add regression tests for both sides of the contract.
  Resolved: Fixed on 2026-03-24. Added `NarrowingPolicy::env_disabled()` in `ori_repr/src/plan/query.rs` with strict value parsing via `is_env_truthy()` — accepts only `”1”`, `”true”`, `”yes”` (case-insensitive). Updated all 4 call sites: `oric/src/main.rs`, `oric/src/commands/build_options/mod.rs`, `oric/src/commands/run/mod.rs`, `ori_llvm/src/evaluator/compile.rs`. `ORI_NO_REPR_OPT=0` and `ORI_NO_REPR_OPT=false` now correctly do NOT disable repr-opt.

- [x] `[TPR-01-031][low]` `compiler/ori_repr/src/canonical.rs:1` — the new canonicalization module already exceeds the repo’s 500-line production-file limit.
  Evidence: `wc -l compiler/ori_repr/src/canonical.rs` reports 575 lines. `CLAUDE.md` and this section’s completion checklist both require production Rust files to stay under 500 lines, but the newly added §01 core module lands above that limit on its first commit.
  Impact: The central repr canonicalization logic starts out harder to review and harder to extend cleanly; upcoming §01.5 and §01.8 work is likely to push even more unrelated concerns into the same oversized file.
  Required plan update: Split `canonical.rs` into focused submodules before more repr work lands there, for example separating pool traversal, aggregate layout helpers, and per-tag canonicalization helpers.
  Resolved: Fixed on 2026-03-24. Extracted layout utilities (`is_trivial_repr`, `field_size`, `field_align`, `repr_size`, `repr_align`, `round_up`, `compute_field_layout`, `compute_payload_layout`, `TupleRepr::to_machine_repr`) into `ori_repr/src/layout.rs` (174 lines). `canonical.rs` now 415 lines. Also eliminated SSOT violation: `is_trivial_machine_repr` in `query.rs` was a duplicate of `is_trivial_repr` — unified to single definition in `layout.rs`.

- [x] `[TPR-01-032][medium]` `compiler/oric/src/main.rs:74` — §01 re-resolves the `ORI_NO_REPR_OPT=1 ori build ...` regression without any test that exercises the actual zero-option build-command path.
  Evidence: Fresh verification still finds no test coverage for the `main.rs` build-command loop plus unconditional env override. `compiler/oric/src/commands/build_options/tests.rs` and `compiler/oric/tests/phases/codegen/build_command.rs` only cover `parse_build_options()` / `BuildOptions`, and `cargo test -p oric build_options` ran 6 `build_options` unit tests, 21 phase tests, and 0 tests in `src/main.rs`. The production-only path at `compiler/oric/src/main.rs:74-98` remains distinct: it merges one CLI token at a time, then applies `NarrowingPolicy::env_disabled()` after the loop for the zero-option case.
  Impact: The exact regression that motivated TPR-01-028/029 is still unpinned. A future refactor can silently stop honoring `ORI_NO_REPR_OPT=1 ori build file.ori` with no extra build flags while this section advertises the issue as resolved.
  Required plan update: Extract the build-command option accumulation into a directly testable helper or add an integration test that drives the real build dispatcher with `ORI_NO_REPR_OPT=1` and no extra build flags, then revalidate TPR-01-028/029 in the same edit pass before returning `third_party_review.status` to `resolved`.
  Resolved: Accepted on 2026-03-24. Valid — zero-option build path is untested. Implementation task added to 01.10.

- [x] `[TPR-01-033][low]` `compiler/ori_repr/src/plan.rs:179` — §01.4 claims tracing integration is complete, but the live query surface still does not emit trace events.
  Evidence: The checklist item at `plans/repr-opt/section-01-repr-ir.md:755-765` says all `ReprPlan` queries emit trace-level events, yet `rg -n "tracing::trace!" compiler/ori_repr/src/plan.rs compiler/ori_repr/src/plan/query.rs` finds exactly one trace call inside `get_repr_traced()`, and `rg -n "get_repr_traced\\(" compiler/ori_repr/src compiler/oric/src compiler/ori_llvm/src` finds no callers. The real query APIs in `compiler/ori_repr/src/plan/query.rs` (`int_width`, `float_width`, `is_trivial`, `escapes`, `rc_strategy`, `narrowing_policy`) still return without tracing.
  Impact: `ORI_LOG=ori_repr=trace` does not show the query traffic this section claims to have landed, which weakens the repo’s tracing-first debugging workflow and leaves §01.4 overstated as complete.
  Required plan update: Route the public query APIs through traced helpers (or otherwise emit trace events from the actual query surface), add a targeted regression check for that behavior, and keep §01.4 in progress until the documented tracing contract is real.
  Resolved: Accepted on 2026-03-24. Valid — 01.4 tracing checkbox unchecked, implementation tasks added to 01.4.

- [x] `[TPR-01-034][medium]` `compiler/oric/src/commands/build_options/mod.rs:109` — The real `ori build` per-argument parser still drops non-boolean scalar options like `--link=` and `--jobs=`.
  Evidence: `main.rs:79-90` parses one CLI token at a time and merges each temporary `BuildOptions`. `parse_build_options()` sets `link_mode` and `jobs` (`compiler/oric/src/commands/build_options/mod.rs:361-384`), but `BuildOptions::merge()` never copies either field (`compiler/oric/src/commands/build_options/mod.rs:109-167`). The direct parser tests at `compiler/oric/tests/phases/codegen/build_command.rs:259-292` therefore do not exercise the actual accumulation path used by `ori build`.
  Impact: `ori build foo.ori --link=dynamic` is reset back to `LinkMode::Static`, and `--jobs=4` is reset to `None`, before the build pipeline sees them. The current tests give false confidence about the user-facing CLI path.
  Required plan update: Add regression tests for the real per-arg accumulation flow covering scalar fields, then make `merge()` preserve every supported build option or replace the one-arg reparse loop with direct accumulation logic.
  Resolved: Accepted on 2026-03-24. Validated — `merge()` silently drops `link_mode` and `jobs`. Implementation tasks added to 01.4 (fix merge + regression tests).

- [x] `[TPR-01-035][medium]` `compiler/oric/src/commands/build_options/mod.rs:64` — §01.3’s repr-opt policy plumbing landed as a boolean kill switch instead of the planned `NarrowingPolicy` API boundary.
  Evidence: The section explicitly requires threading `NarrowingPolicy` end-to-end and says “Do NOT use `repr_opt_disabled: bool`” (`plans/repr-opt/section-01-repr-ir.md:623-630`), but the implementation stores `pub no_repr_opt: bool` (`compiler/oric/src/commands/build_options/mod.rs:64-69`) and threads `no_repr_opt: bool` through the AOT entry points (`compiler/oric/src/commands/compile_common.rs:133-143`, `compiler/oric/src/commands/compile_common.rs:182-196`, `compiler/oric/src/commands/codegen_pipeline.rs:225-238`). The JIT path likewise never accepts a policy parameter; it hardcodes `env_disabled() ? Disabled : Aggressive` inside `compiler/ori_llvm/src/evaluator/compile.rs:130-160`.
  Impact: Conservative mode cannot be expressed through the current CLI/codegen surface without another round of signature churn, and the new boolean parameters violate the repo’s “no boolean flags” API rule on already-overloaded compilation entry points. §01.3 is marked complete even though the promised policy-shaped boundary was not actually established.
  Required plan update: Replace `no_repr_opt` with `NarrowingPolicy` across `BuildOptions`, the JIT/AOT compilation entry points, and their tests, then add regression coverage proving Aggressive, Disabled, and Conservative survive the real CLI and codegen plumbing.
  Resolved: Accepted on 2026-03-24. Validated — `BuildOptions` uses `bool` instead of `NarrowingPolicy`, violating plan mandate. Implementation tasks added to 01.4 (replace bool with NarrowingPolicy end-to-end).

- [x] `[TPR-01-037][high]` `compiler/ori_repr/src/canonical.rs:170` — TPR-01-021 is not actually fixed at the public API boundary: standalone `canonical()` calls for a mutually recursive SCC still produce root-dependent shapes, and the new regression test only passes because it switched to `canonical_cached()`.
  Evidence: `canonical()` still allocates a fresh cache per call (`compiler/ori_repr/src/canonical.rs:170-171`), so it does not share memoized results across standalone root queries. The new test at `compiler/ori_repr/src/tests.rs:973-1033` claims to validate `canonical(A)`/`canonical(B)` consistency, but it computes both roots through `canonical_cached()` with a manually shared cache instead (`lines 993-996`). Fresh verification with a standalone probe against the built crate produced `equal=false` for `b_inside_a == canonical(B)`, confirming the public contract still fails.
  Impact: The section now marks TPR-01-021 complete even though the exported `canonical()` helper still returns different `MachineRepr` values for the same `Idx` depending on traversal entry point. That leaves callers outside `populate_canonical()` with unstable equality/hash behavior and gives false confidence because the regression test no longer exercises the documented contract.
  Required plan update: Either make public `canonical()` preserve SCC-wide memoization across standalone roots or narrow the documented contract to `populate_canonical()`/`compute_repr_plan()` only. In the same edit pass, replace the current mutual-recursion test with a semantic pin that compares public `canonical(A)`/`canonical(B)` results, not just `canonical_cached()`.
  Resolved: Accepted on 2026-03-24. Validated — public `canonical()` creates fresh cache per call, so SCC mutual recursion produces root-dependent shapes. The production path (`populate_canonical()`) is correct via shared cache. Fix: narrow `canonical()` to `pub(crate)`, document that only `compute_repr_plan()`/`populate_canonical()` guarantees SCC consistency, and update the mutual-recursion test to be a true semantic pin testing `canonical_cached()` explicitly (which is the actual contract). Implementation tasks added to §01.9.

- [x] `[TPR-01-036][medium]` `compiler/oric/src/commands/build_options/mod.rs:167` — Explicit `NarrowingPolicy::Aggressive` is still not representable through the real CLI accumulation path, so `--repr-opt=aggressive` cannot override `ORI_NO_REPR_OPT=1` and cannot win after a previous disabling flag.
  Evidence: `BuildOptions` stores only `narrowing_policy` with no explicitness bit (`compiler/oric/src/commands/build_options/mod.rs:64-69`), `merge()` only copies non-default policies (`lines 167-169`), and both `parse_build_options()` and `main.rs` reapply the env kill switch whenever the merged policy is `Aggressive` (`compiler/oric/src/commands/build_options/mod.rs:426-429`, `compiler/oric/src/main.rs:98-101`). Fresh verification: `timeout 150 env ORI_NO_REPR_OPT=1 cargo test -p oric commands::build_options::tests::parse_recognizes_repr_opt_aggressive -- --exact` fails with `left: Disabled right: Aggressive`, while the conservative-path tests still pass.
  Impact: The documented “CLI flag overrides env fallback” behavior is false for the default policy, and the per-arg `ori build` loop cannot express “re-enable aggressive” after `--no-repr-opt` or a globally-set env var. This makes the new policy surface asymmetric: `Conservative` and `Disabled` survive, but `Aggressive` is only a default, not a real explicit choice.
  Required plan update: Track whether a narrowing policy was explicitly set, preserve last-write-wins semantics in `merge()`, and add regression tests covering `ORI_NO_REPR_OPT=1 + --repr-opt=aggressive` plus mixed-order CLI sequences such as `--no-repr-opt --repr-opt=aggressive`.
  Resolved: Accepted on 2026-03-24. Validated — `narrowing_policy` has no explicitness bit, so `Aggressive` (default) is indistinguishable from “not set”. Env var always wins. Fix: add `narrowing_policy_explicit: bool` to `BuildOptions` (matching `opt_level_explicit`/`debug_level_explicit`/`lto_explicit` pattern), use last-write-wins in `merge()`, and only apply env fallback when policy was not explicitly set. Implementation tasks added to §01.10.

- [x] `[TPR-01-038][medium]` `compiler/oric/src/commands/build_options/mod.rs:158` — The per-argument `ori build` accumulation path still cannot represent explicit default-valued `--link=static` and `--jobs=auto` selections.
  Evidence: `main.rs` folds one CLI token at a time through `BuildOptions::merge()` (`compiler/oric/src/main.rs:79-89`). `parse_build_options()` correctly parses `--link=static` to `LinkMode::Static` and `--jobs=auto` / `-j` to `None` (`compiler/oric/src/commands/build_options/mod.rs:373-396`), but `merge()` only copies `jobs` when `other.jobs.is_some()` and only copies `link_mode` when `other.link_mode != LinkMode::default()` (`compiler/oric/src/commands/build_options/mod.rs:158-165`). By inspection, `ori build foo.ori --link=dynamic --link=static` leaves `Dynamic`, and `ori build foo.ori --jobs=4 -j` leaves `Some(4)`.
  Impact: The real CLI is still not last-write-wins for two documented scalar options, so users cannot explicitly return to the default values after setting a non-default one later in the same command line.
  Required plan update: Add explicitness tracking (or equivalent last-write-wins handling) for `link_mode` and `jobs`, and add regression coverage for mixed-order sequences including `--link=dynamic --link=static`, `--link=static --link=dynamic`, `--jobs=4 -j`, and `-j --jobs=4`.
  Resolved: Accepted on 2026-03-24. Validated — `merge()` uses `is_some()`/`!= default()` guards that cannot represent explicit default values. Implementation task at §01.10 [TPR-01-038].

- [x] `[TPR-01-039][low]` `compiler/ori_repr/src/canonical.rs:1` — `canonical.rs` has drifted back above the repo's 500-line production-file limit.
  Evidence: Fresh verification with `wc -l compiler/ori_repr/src/canonical.rs` reports `508` lines. `CLAUDE.md` and `.claude/rules/impl-hygiene.md` still require production Rust files to stay under 500 lines, and this section currently marks the earlier oversize-file finding (TPR-01-031) resolved.
  Impact: The core canonicalization module is again outside the repo's hygiene boundary, and the plan history now overstates the current tree by treating the split as fully complete.
  Required plan update: Extract enough logic or helpers from `canonical.rs` to bring it back under 500 lines, then revalidate the prior TPR-01-031 closure in the same edit pass.
  Resolved: Accepted on 2026-03-24. Validated — `canonical.rs` is 508 lines, over the 500-line limit. Implementation task at §01.10 [TPR-01-039].

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
- [ ] **[TPR-01-032]** Integration test for zero-option build path: `ORI_NO_REPR_OPT=1 ori build file.ori` (no extra build flags) must honor the env var. Either extract the main.rs build-command loop into a testable helper, or add an integration test that drives the real build dispatcher. Must exercise the path at `main.rs:79-101` where the per-arg parser loop never executes.
- [ ] **[TPR-01-036]** Add `narrowing_policy_explicit: bool` to `BuildOptions` (matching `opt_level_explicit`/`debug_level_explicit`/`lto_explicit` pattern). Update `parse_build_options()` to set `narrowing_policy_explicit = true` when `--repr-opt=*` or `--no-repr-opt` is parsed. Update `merge()` to use last-write-wins when explicit (not "non-default overrides"). Update env fallback in both `parse_build_options()` and `main.rs` to only apply `ORI_NO_REPR_OPT` when `!narrowing_policy_explicit`. Regression tests: `ORI_NO_REPR_OPT=1 + --repr-opt=aggressive` → `Aggressive`; `--no-repr-opt --repr-opt=aggressive` → `Aggressive` (last-write-wins); `--repr-opt=aggressive --no-repr-opt` → `Disabled` (last-write-wins).
- [ ] **[TPR-01-038]** Add explicitness tracking (or equivalent last-write-wins handling) for `link_mode` and `jobs` in `BuildOptions`. Regression tests: `--link=dynamic --link=static` → `Static`; `--link=static --link=dynamic` → `Dynamic`; `--jobs=4 -j` → auto (`None`); `-j --jobs=4` → `Some(4)`.
- [ ] **[TPR-01-039]** Re-split `compiler/ori_repr/src/canonical.rs` after the mutual-recursion changes so the production file is back under the 500-line limit, then revalidate the old TPR-01-031 closure against the current tree.

- [ ] All tests from §01.2, §01.4, §01.5, §01.7, §01.8, §01.9 written and passing in both debug (`cargo test -p ori_repr`) and release (`cargo test -p ori_repr --release`)
- [ ] Semantic pin tests present: at least one test per subsection that would fail if the canonical mapping, default query return values, or Phase A wiring were reverted
- [ ] `./test-all.sh` green (zero regressions across all crates)
- [ ] `./clippy-all.sh` green

**Exit Criteria:** `ori_repr` crate exists, `ReprPlan` is threaded through the entire LLVM codegen pipeline, all existing tests pass with identical behavior, `cargo test -p ori_repr --release` passes, and `ORI_LOG=ori_repr=trace ori build tests/benchmarks/bench_small.ori` shows `ReprPlan query` events for every type in the program.
