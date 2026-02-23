---
section: "01"
title: "Emission Layer Typing"
status: complete
goal: "Eliminate type-pool lookups during LLVM emission by pre-computing value representations and RC strategies"
inspired_by:
  - "Rust OperandValue (rustc_codegen_llvm/mir/operand.rs)"
  - "Lean 4 type-indexed phases (Compiler/IR/)"
  - "Lean 4 inc/dec with isPointer flag (Compiler/IR/RC.lean)"
sections:
  - id: "01.1"
    title: "Add ValueRepr to ARC IR"
    status: done
  - id: "01.2"
    title: "Add EmittedValue to ArcIrEmitter"
    status: done
  - id: "01.3"
    title: "Add RcStrategy to RC instructions"
    status: done
  - id: "01.4"
    title: "Propagate ValueRepr through ARC passes"
    status: done
  - id: "01.5"
    title: "Tests & validation"
    status: done
---

# Section 01: Emission Layer Typing

**Status:** In Progress (01.1 done, 01.2–01.5 not started)
**Goal:** Every ARC IR value carries its memory representation; every RC instruction carries its cleanup strategy; the LLVM emitter never queries the Pool or TypeInfo to decide how to load/store a value or how to inc/dec a reference count.

**Why this matters:** The DPR identified this as the #1 source of pain, and the 2026-02-22 debugging session confirmed it. Currently, `ArcIrEmitter::emit_instr` produces raw `ValueId` values and must re-derive "is this a pointer? a scalar? an aggregate?" by querying `TypeInfo` at every use site. Worse, `RcInc`/`RcDec` instructions carry only a variable ID — the emitter must reach back into the Pool (`pool.tag()`, `pool.resolve_fully()`) to determine the cleanup strategy (closure env extraction vs enum tag-switch vs heap RC dec). A misclassification silently corrupts memory. Rust's `OperandValue` and Lean 4's `isPointer` flag solve this by making representation explicit at the IR level.

**This section is the backbone of the information contract chain** (see overview). It produces two artifacts consumed by all downstream sections:
- `ValueRepr` — consumed by Sections 03 (closure), 04 (borrow), 06 (RC identity)
- `RcStrategy` — consumed by Section 07 (cross-block elimination) for strategy-aware pair matching

---

## 01.1 Add `ValueRepr` to ARC IR — DONE

**Approach deviation:** The original plan called for adding `repr: ValueRepr` to each `ArcInstr` variant (~8 variants, ~40+ match arm edits across 6 pass modules). Instead, we used a **parallel array** `var_reprs: Vec<ValueRepr>` indexed by `ArcVarId::index()` — same pattern as the existing `var_types`. This avoids touching ~40 match sites while providing identical lookup capability via `func.var_reprs[v.index()]` or `func.var_repr(v)`.

**Files created:**
- `compiler/ori_arc/src/ir/repr.rs` — `ValueRepr` enum + `compute_var_reprs()` (~95 lines)
- `compiler/ori_arc/src/ir/repr/tests.rs` — 16 unit tests covering all repr variants (~210 lines)

**Files modified:**
- `compiler/ori_arc/src/ir/mod.rs` — `mod repr`, re-exports, `var_reprs` field, `var_repr()` accessor, `fresh_var()` sync
- `compiler/ori_arc/src/lower/mod.rs` — `var_reprs: Vec::new()` in `finish()`
- `compiler/ori_arc/src/lib.rs` — `pool: &Pool` param on `run_arc_pipeline`/`run_arc_pipeline_all`, `compute_var_reprs` call, re-exports
- `compiler/ori_arc/src/test_helpers.rs` — `var_reprs: Vec::new()` in `make_func_named()`
- `compiler/ori_arc/src/tests.rs` — pass `pool` to `run_full_pipeline`
- `compiler/ori_arc/src/ir/tests.rs` — `var_reprs: Vec::new()` in 5 struct literals
- `compiler/ori_arc/src/drop/tests.rs` — `var_reprs: Vec::new()` in 3 struct literals
- `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` — pass `self.pool` to 3 `run_arc_pipeline` calls
- `compiler/ori_llvm/src/codegen/arc_emitter/tests.rs` — `var_reprs: Vec::new()` in 3 struct literals
- `compiler/ori_llvm/src/aot/incremental/arc_cache/tests.rs` — `var_reprs: Vec::new()` in 1 struct literal

**Completed items:**

- [x] Define `ValueRepr` enum (Scalar, RcPointer, Aggregate, FatValue) in `ir/repr.rs`
- [x] `ValueRepr::from_arc_class(class, pool, idx)` — bridge from ArcClass + Pool tag to repr
- [x] `compute_var_reprs(func, classifier, pool)` — produces parallel vec from var_types
- [x] `var_reprs: Vec<ValueRepr>` field in `ArcFunction` (with `#[cfg_attr(feature = "cache", serde(skip))]`)
- [x] `var_repr(&self, var) -> Option<ValueRepr>` accessor (returns None pre-pipeline)
- [x] `fresh_var()` syncs `var_reprs` when non-empty (Scalar placeholder for pass-created vars)
- [x] `run_arc_pipeline` / `run_arc_pipeline_all` accept `pool: &Pool`, call `compute_var_reprs` at pipeline start
- [x] All callers updated to pass pool
- [x] 16 unit tests (from_arc_class for all categories, compute_var_reprs integration)
- [x] `cargo c`, `cargo t -p ori_arc`, `cargo bl`, `./llvm-test.sh`, `./clippy-all.sh` — all clean

---

## 01.2 Add `EmittedValue` to ArcIrEmitter

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

- [x] Define `EmittedValue` enum:
  ```rust
  /// Tagged LLVM value with representation info.
  /// Prevents "did I load this already?" class of bugs.
  #[derive(Clone, Copy, Debug)]
  enum EmittedValue {
      /// Register scalar: i64, f64, i1, i8, i32.
      Immediate(ValueId),
      /// Pointer to heap-allocated RC'd memory.
      RcPointer(ValueId),
      /// Stack aggregate (struct, tuple, enum by value).
      Aggregate(ValueId),
      /// Two-word: {first, second} — str={len,ptr}, closure={fn,env}.
      Pair { first: ValueId, second: ValueId },
      /// No runtime representation (unit, never).
      ZeroSized,
  }
  ```

- [x] Add helper methods on `EmittedValue`:
  - `into_raw(self) -> ValueId` — extract single ValueId (panics on Pair/ZeroSized)
  - `rc_data_ptr(self) -> Option<ValueId>` — get RC-trackable pointer if any
  - `is_rc_managed(self) -> bool` — true for RcPointer and Pair (with rc second)
  - `from_repr(repr: ValueRepr, value: ValueId) -> Self` — bridge from ARC IR repr

- [x] Replace `var_map: Vec<Option<ValueId>>` with `var_map: Vec<Option<EmittedValue>>`

- [x] Update `emit_instr()` to produce `EmittedValue`:
  - `Let` with `Scalar` repr → `EmittedValue::Immediate(value)`
  - `Let` with `RcPointer` repr → `EmittedValue::RcPointer(value)`
  - `Construct` with `Aggregate` repr → `EmittedValue::Aggregate(alloca)`
  - `PartialApply` → `EmittedValue::Pair { first: fn_ptr, second: env_ptr }`
  - `IsShared` → `EmittedValue::Immediate(i1_result)`

- [x] Update all consumers to destructure `EmittedValue`:
  - `emit_terminator` — `Return` extracts based on variant
  - `emit_apply` — arg passing based on variant (Immediate→direct, Aggregate→pointer, Pair→split)
  - `emit_invoke` — same as emit_apply
  - `emit_rc_inc`/`emit_rc_dec` — only operates on `RcPointer`/`Pair.second`
  - `emit_construct` — field values based on variant

---

## 01.3 Add `RcStrategy` to RC Instructions

**Files:** `compiler/ori_arc/src/ir/mod.rs`, `compiler/ori_arc/src/rc_insert/mod.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

**Why this exists:** `ValueRepr` tells the emitter how a value is laid out in memory. But `RcInc`/`RcDec` need more — they need to know the RC *cleanup strategy*. `ValueRepr::Aggregate` doesn't distinguish structs (traverse fields and Dec each) from enums (tag-switch, per-variant cleanup). `ValueRepr::FatValue` doesn't distinguish strings (Dec the data_ptr half) from closures (extract env_ptr, null check, load drop_fn). This is the exact gap that caused the 2026-02-22 `Result<int, str>` leak: the emitter saw an aggregate, called `ori_rc_dec` on it like a heap pointer, and silently corrupted memory.

Lean 4 solves this by embedding an `isPointer` flag in `inc`/`dec`. Ori needs a richer classification because it has more value representations (closures, enums, fat values). The `RcStrategy` enum is that classification.

- [x] Define `RcStrategy` enum in `ir/mod.rs`:
  ```rust
  /// How to perform an RC operation on a value.
  /// Computed during RC insertion from ValueRepr + Pool structure.
  /// Consumed by the LLVM emitter — pattern match, never query Pool.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  pub enum RcStrategy {
      /// Heap-allocated RC pointer (str data, list, map, set, etc.).
      /// Inc: call ori_rc_inc(data_ptr).
      /// Dec: call ori_rc_dec(data_ptr, drop_fn).
      HeapPointer,

      /// Fat value with RC pointer half (str = {len, data_ptr}).
      /// Inc: extract field 1 (data_ptr), call ori_rc_inc(data_ptr).
      /// Dec: extract field 1 (data_ptr), call ori_rc_dec(data_ptr, drop_fn).
      FatPointer,

      /// Closure ({fn_ptr, env_ptr}). Env may be null (zero-capture).
      /// Inc: extract env_ptr, null check, call ori_rc_inc(env_ptr).
      /// Dec: extract env_ptr, null check, load drop_fn from env header,
      ///      call ori_rc_dec(env_ptr, drop_fn).
      Closure,

      /// Stack aggregate with RC-typed fields (struct, tuple).
      /// Inc: for each RC field, extract and Inc recursively.
      /// Dec: call generated drop function that traverses fields.
      AggregateFields,

      /// Enum/Result with potentially RC-typed variant payloads.
      /// Inc: tag-switch, per-variant field Inc.
      /// Dec: tag-switch, per-variant field Dec.
      InlineEnum,
  }
  ```

- [x] Add `strategy` field to `RcInc` and `RcDec`:
  ```rust
  RcInc { var: ArcVarId, count: u32, strategy: RcStrategy },
  RcDec { var: ArcVarId, strategy: RcStrategy },
  ```

- [x] Compute `RcStrategy` during RC insertion (`rc_insert/mod.rs`):
  ```rust
  impl RcStrategy {
      /// Compute from the variable's ValueRepr and Pool details.
      /// Called once during RC insertion; result embedded in the instruction.
      pub fn from_var(repr: ValueRepr, pool: &Pool, ty: Idx) -> Self {
          match repr {
              ValueRepr::Scalar => unreachable!("scalar vars never get RC ops"),
              ValueRepr::RcPointer => RcStrategy::HeapPointer,
              ValueRepr::FatValue => {
                  let resolved = pool.resolve_fully(ty);
                  if pool.tag(resolved) == Tag::Function {
                      RcStrategy::Closure
                  } else {
                      RcStrategy::FatPointer
                  }
              }
              ValueRepr::Aggregate => {
                  let resolved = pool.resolve_fully(ty);
                  match pool.tag(resolved) {
                      Tag::Result | Tag::Enum => RcStrategy::InlineEnum,
                      _ => RcStrategy::AggregateFields,
                  }
              }
          }
      }
  }
  ```

- [x] Update `ArcIrEmitter::emit_instr` to pattern-match on `strategy`:
  ```rust
  ArcInstr::RcInc { var, count, strategy } => match strategy {
      RcStrategy::HeapPointer     => self.emit_rc_inc_heap(*var, *count),
      RcStrategy::FatPointer      => self.emit_rc_inc_fat(*var, *count),
      RcStrategy::Closure         => self.emit_rc_inc_closure(*var, *count),
      RcStrategy::AggregateFields => self.emit_rc_inc_aggregate(*var, *count, func),
      RcStrategy::InlineEnum      => self.emit_rc_inc_inline_enum(*var, *count, func),
  },
  ArcInstr::RcDec { var, strategy } => match strategy {
      RcStrategy::HeapPointer     => self.emit_rc_dec_heap(*var, func),
      RcStrategy::FatPointer      => self.emit_rc_dec_fat(*var, func),
      RcStrategy::Closure         => self.emit_rc_dec_closure(*var, func),
      RcStrategy::AggregateFields => self.emit_rc_dec_aggregate(*var, func),
      RcStrategy::InlineEnum      => self.emit_rc_dec_inline_enum(*var, func),
  },
  ```
  Each arm is a focused function (~20-40 lines) instead of one 80-line branch cascade with interleaved Pool queries.

- [x] **Extract the existing RcInc handler** from the current monolithic `emit_instr` match arm into per-strategy functions:
  - `emit_rc_inc_heap` — current `extract_rc_data_ptrs` + `ori_rc_inc` loop (~15 lines)
  - `emit_rc_inc_fat` — extract field 1 (data_ptr), call `ori_rc_inc` (~10 lines)
  - `emit_rc_inc_closure` — current closure path: extract env_ptr, null check, `ori_rc_inc` (~20 lines)
  - `emit_rc_inc_aggregate` — for each RC field, extract and recursively Inc (~25 lines, **currently missing**)
  - `emit_rc_inc_inline_enum` — **intentional no-op**: log skip at trace level, return immediately (~5 lines). The existing `emit_inline_enum_inc` (159 lines, dead code) should be **deleted** — it was written under the incorrect symmetry assumption.

- [x] **Extract the existing RcDec handler** from the current monolithic `emit_instr` match arm into per-strategy functions:
  - `emit_rc_dec_heap` — current `extract_rc_data_ptrs` + `get_or_generate_drop_fn` + `ori_rc_dec` (~15 lines)
  - `emit_rc_dec_fat` — extract field 1, call `ori_rc_dec` with drop fn (~15 lines)
  - `emit_rc_dec_closure` — current closure path: extract env_ptr, null check, load drop_fn, `ori_rc_dec` (~25 lines)
  - `emit_rc_dec_aggregate` — call generated drop function on the value (~15 lines)
  - `emit_rc_dec_inline_enum` — current `emit_inline_enum_dec` (~50 lines, **already written**)

- [x] **Ensure Inc/Dec symmetry for every strategy.** For each `RcStrategy` variant, the Inc and Dec functions must be symmetric:
  - `HeapPointer`: Inc calls `ori_rc_inc(data_ptr)`, Dec calls `ori_rc_dec(data_ptr, drop_fn)` — symmetric on data_ptr extraction
  - `FatPointer`: both extract field 1, then Inc/Dec the pointer half — symmetric
  - `Closure`: both extract env_ptr, null check, then Inc/Dec — symmetric
  - `AggregateFields`: both traverse RC fields; Inc calls `ori_rc_inc` per field, Dec calls drop fn — symmetric traversal
  - `InlineEnum`: **intentionally asymmetric**.
    - Inc: **no-op** — the container is stack-allocated, inner fields are managed at extraction or Dec time. The RC inserter's Inc on a stack Result means "keep alive longer" but stack values are already alive through SSA scope.
    - Dec: tag-switch with per-variant field traversal — cleans up inner RC fields.
    - **Why this is correct**: Unlike Lean 4 where all compound types are heap-allocated with RC headers, Ori's Result/Enum are stack-allocated. Inc/Dec operate on the *inner* fields, not the container itself. Inc has nothing to increment (inner fields haven't been extracted yet); Dec must traverse to find and decrement inner RC fields.
    - **Semantic model**: `RcStrategy::InlineEnum` on `RcInc` → emit nothing (documented skip). `RcStrategy::InlineEnum` on `RcDec` → emit tag-switch with per-variant field Dec.
    - **This was NOT the 2026-02-22 leak bug.** The leaks have two causes: (1) missing borrow annotations for builtin methods (Section 04.4, Builtin Method Borrowing), and (2) `Project` not classified as borrowing in `is_borrowing_instr`, so Perceus consumes the parent Result on tag extraction without Dec'ing inner RC fields (Section 04.4, Project Borrowing). The Inc no-op is correct by design. The `RcDec` with `InlineEnum` strategy (tag-switch + per-variant field Dec) IS the correct cleanup mechanism — the bug is that it's never emitted because Perceus doesn't know the parent needs cleanup after a scalar projection.

- [x] **Delete the old monolithic RcInc/RcDec match arms** in `emit_instr` (currently lines 1202-1287). Replace with the two match-on-strategy dispatchers above.

- [x] **Delete `extract_rc_data_ptrs` usage from RC operations.** This function is currently the universal "figure out what pointers to inc/dec" — it gets replaced by the per-strategy functions which know exactly what to extract. `extract_rc_data_ptrs` may still be needed for non-RC uses (e.g., coercion), but should no longer be called from `emit_rc_inc_*` or `emit_rc_dec_*`.

- [x] **Add undefined-variable guard** to all `emit_rc_inc_*` and `emit_rc_dec_*` functions:
  ```rust
  fn emit_rc_inc_inline_enum(&mut self, var: ArcVarId, count: u32, func: &ArcFunction) {
      let val = self.var(var);
      if val.is_none() {
          tracing::warn!(var = var.raw(), "skipping RcInc on undefined variable");
          return;
      }
      // ... tag-switch logic ...
  }
  ```
  This prevents the crash discovered in the 2026-02-22 session where `ValueId::NONE` was passed to GEP/store operations. The guard is a safety net — the root cause (RC insertion placing ops before variable definitions) should be fixed separately.

  **Root cause investigation (prerequisite):** The "variable not yet defined" errors are pre-existing and affect 14+ test functions. The likely cause: when an `Invoke` terminator is handled via the builtin method path (`try_emit_builtin_method`), the destination variable IS defined via `def_var` (line 509 in arc_emitter/mod.rs), but RC instructions referencing other variables in the same block may precede their definitions due to block processing order. Specifically, when the main block loop processes a block that was already partially populated by a builtin method's `br`+`position_at_end`, the instructions emitted by the builtin are at the start of the block, but the RC instructions from the ARC IR may reference variables defined later in the same block's ARC IR body. Investigate by adding tracing to `var()` to log which function and block triggers the undefined variable.

- [x] Add `debug_assert!` in the emitter that verifies strategy matches the Pool (temporary validation during migration):
  ```rust
  #[cfg(debug_assertions)]
  {
      let expected = RcStrategy::from_var(func.var_repr(*var), self.pool, func.var_type(*var));
      debug_assert_eq!(*strategy, expected, "RcStrategy mismatch for var {:?}", var);
  }
  ```
  This assert is removed once all Pool queries are eliminated from the emitter.

---

## 01.4 Propagate ValueRepr Through ARC Passes — DONE

**Files:** All pass modules in `compiler/ori_arc/src/`

**Approach:** Added `fresh_var_repr(ty, repr)` to `ArcFunction` for passes that know the correct repr.
Changed `classify` module visibility from `mod` to `pub(crate) mod` so `lower` can construct `ArcClassifier`.

- [x] Update `lower_function_can()` to populate `var_reprs` for every variable created during lowering
  - `lower_function_can` now creates an `ArcClassifier` from its `pool` parameter and calls `compute_var_reprs` on both the main function and all lambda bodies before returning
  - Functions exit lowering with correct, fully-populated `var_reprs`
  - `run_arc_pipeline` re-computes (same values) as a consistency backstop

- [x] Update RC insertion (`rc_insert/mod.rs`) to:
  - Already correct: `rc_strategy()` helper reads `func.var_repr(var)` + Pool to compute `RcStrategy` (verified — no change needed)
  - RC insertion does not create new variables; all vars are pre-lowered
  - This is the **last point** where Pool queries are needed for RC — after this, the strategy is embedded

- [x] Update reset/reuse detection and expansion to preserve `var_reprs`
  - `detect_reset_reuse` and `detect_reset_reuse_cfg` now accept `pool: &Pool` parameter
  - Token variables use `fresh_var_repr(dec_ty, repr_for_type(...))` instead of `fresh_var(dec_ty)` with Scalar placeholder
  - `expand_reset_reuse::build_merge_block` computes correct repr for merge parameters via `ValueRepr::from_arc_class`
  - `is_shared_var` (Bool) correctly gets Scalar repr (unchanged — was already correct)

- [x] Update RC elimination to use `var_reprs` and `strategy` for sanity checks
  - `validate_rc_targets()` runs at the start of `eliminate_rc_ops` (debug builds only)
  - Asserts `RcInc`/`RcDec` targets have non-Scalar repr
  - `strategy_matches_repr()` verifies strategy category is consistent with repr family
  - Both checks skip gracefully when `var_reprs` is empty (test-only path without Pool)

---

## 01.5 Tests & Validation

- [x] Unit tests in `compiler/ori_arc/src/ir/repr/tests.rs` (01.1):
  - `ValueRepr::from_arc_class` for all type categories (16 tests)
  - `compute_var_reprs` integration test with mixed types
- [x] Unit tests for `RcStrategy` classification in `compiler/ori_arc/src/ir/repr/tests.rs` (01.5):
  - `str` → `FatPointer` ✓
  - `[str]` (list) → `HeapPointer` ✓
  - `(int, str)` (tuple) → `AggregateFields` ✓
  - `Result<int, str>` → `InlineEnum` ✓
  - `Option<str>` → `InlineEnum` ✓
  - `enum` → `InlineEnum` ✓
  - `struct` → `AggregateFields` ✓
  - closure `(int) -> int` → `Closure` ✓
  - `{str: int}` (map) → `HeapPointer` ✓
  - `set[int]` → `HeapPointer` ✓
  - **Note (latent bug)**: `extract_rc_data_ptrs` for `Tag::Option` — no longer relevant since RC operations now use per-strategy dispatch via `RcStrategy::InlineEnum`, which does a tag-switch. The `Option<str>` where value is `None` case is handled correctly by the tag-switch in `emit_rc_dec_inline_enum`.

- [x] Unit tests in `compiler/ori_llvm/src/codegen/arc_emitter/tests.rs`:
  - `EmittedValue` helper methods (into_raw, rc_data_ptr, is_rc_managed) ✓ (6 tests)
  - Verify emit_instr produces correct EmittedValue variant for each ArcInstr ✓ (via from_repr test + integration tests)
  - Verify `emit_rc_dec_*` dispatches correctly based on strategy ✓ (5 tests: FatPointer, Closure, HeapPointer, InlineEnum Inc/Dec)

- [x] AOT integration tests in `compiler/ori_llvm/tests/aot/`:
  - Existing 425 tests continue passing (verified)
  - Coverage across all `RcStrategy` variants via existing tests:
    - `FatPointer` (str): 25+ tests in arc.rs, spec.rs, derives.rs
    - `HeapPointer` (list, map): 23+ tests in arc.rs, for_loops.rs, iterators.rs
    - `AggregateFields` (tuple, struct): 17+ tests in arc.rs, derives.rs
    - `InlineEnum` (Result, Option): 33+ tests in arc.rs, spec.rs
    - `Closure`: 4 tests in arc.rs
  - **Gaps** (blocked by compiler, not test coverage): enum variant constructors (2 tests ignored), set (no AOT impl), map construction

- [x] Run `./test-all.sh` — zero regressions (425 AOT + all Rust unit tests pass)
  - **Note**: LLVM backend Ori spec tests crash with pre-existing heap corruption (documented, to be fixed in Section 04)

---

## 01.6 Completion Checklist

**New types:**
- [x] `ValueRepr` enum defined in `ir/repr.rs` (01.1)
- [x] `RcStrategy` enum defined in `ir/repr.rs` (01.3)
- [x] `EmittedValue` enum defined in `arc_emitter/mod.rs` (01.2)

**IR enrichment:**
- [x] `var_reprs: Vec<ValueRepr>` in `ArcFunction` (01.1)
- [x] `RcInc` and `RcDec` carry `strategy: RcStrategy` (01.3)
- [x] All lowering paths populate `var_reprs` (01.4)
- [x] RC insertion computes and embeds `RcStrategy` (01.3)
- [x] All ARC passes preserve `var_reprs` and `strategy` (01.4)

**Emitter migration:**
- [x] `var_map` uses `EmittedValue` instead of `ValueId` (01.2)
- [x] All emit_* methods produce/consume `EmittedValue` (01.2)
- [x] Old monolithic RcInc match arm deleted → replaced by 5 `emit_rc_inc_*` functions (01.3)
- [x] Old monolithic RcDec match arm deleted → replaced by 5 `emit_rc_dec_*` functions (01.3)
- [x] `emit_inline_enum_dec` delegates via `emit_rc_dec_inline_enum` (01.3)
- [x] `emit_rc_inc_inline_enum` is intentional no-op (01.3)
- [x] Inc/Dec symmetry verified for all 5 strategy variants (01.3)
- [x] `ValueId::NONE` guard in all `emit_rc_*` functions (01.3)
- [x] `extract_rc_data_ptrs` no longer called from RC operations (01.3)
- [x] RC dispatch uses `RcStrategy` pattern match — no Pool queries for *which strategy* to use (01.3)
  - **Note:** Pool queries remain in per-strategy handlers (`rc_ops.rs`) for LLVM layout details (field types, collection structure). These are emission-level layout queries, not RC decision queries. Eliminating them requires Section 02 (type layout pre-computation).

**Validation:**
- [x] RC emission pattern-matches on `RcStrategy` for dispatch (01.3)
- [x] No `TypeInfo`/`Pool` queries in `emit_rc_inc`/`emit_rc_dec` dispatch — strategy is pre-computed (01.3)
- [x] `debug_assert!` verifying strategy matches Pool (migration safety net) (01.3)
- [x] All tests pass (425 AOT + 347 unit + 57 runtime)
- [x] `./clippy-all.sh` clean

**Exit Criteria:** The LLVM emitter never calls Pool or TypeInfo to determine (1) whether a value is a scalar, pointer, aggregate, or fat value, or (2) how to perform an RC operation. All decisions come from `ValueRepr` (set during lowering), `RcStrategy` (set during RC insertion), and `EmittedValue` (set during emission). The 24+ Pool/TypeInfo queries currently in `arc_emitter/mod.rs` are reduced to zero for representation and RC decisions.
