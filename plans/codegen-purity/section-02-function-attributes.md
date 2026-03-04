---
section: "02"
title: "Function Attributes"
status: not-started
goal: "Every function and runtime declaration has complete, correct LLVM attributes — noreturn, nounwind, noundef"
inspired_by:
  - "Rust rustc_codegen_llvm/declare.rs — applies nounwind/noreturn/noundef systematically"
  - "Zig src/codegen.zig — tracks side effects and exception behavior per function"
depends_on: []
sections:
  - id: "02.1"
    title: "noreturn on Panic Functions"
    status: not-started
  - id: "02.2"
    title: "nounwind on C main Wrapper"
    status: not-started
  - id: "02.3"
    title: "nounwind on Derived Trait Methods"
    status: not-started
  - id: "02.4"
    title: "nounwind on Runtime Declarations"
    status: not-started
  - id: "02.5"
    title: "nounwind on Indirect-Call Functions"
    status: not-started
  - id: "02.6"
    title: "noundef on Integer Parameters"
    status: not-started
  - id: "02.7"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Function Attributes

**Status:** Not Started
**Goal:** Every function emitted by the compiler has complete LLVM attributes: `noreturn` on functions that never return, `nounwind` on functions that cannot throw, `noundef` on parameters that are always defined. No function is missing an attribute it qualifies for.

**Context:** A two-pass fixed-point nounwind analysis already exists in `nounwind.rs` (prepare → analyze → emit). This infrastructure correctly identifies and marks user functions as `nounwind`. However, it misses several categories outside its analysis scope: the C `main` wrapper, derived trait methods (emitted by `derive_codegen`), and certain runtime declarations. Missing `noreturn` on `ori_panic_cstr` prevents LLVM from eliminating dead code after panic calls. Missing `noundef` on scalar parameters leaves value-range optimizations on the table.

**Existing infrastructure:** `compute_nounwind_set()` in `nounwind.rs` handles user functions via fixed-point iteration. The work here extends coverage to categories currently outside the analysis scope.

**Journeys affected:** M-2 (J1, J5), L-1 (J1, J5, J6), L-2 (J11), L-3 (J9), L-11 (J1), L-12 (J5).

**Reference implementations:**
- **Rust** `rustc_codegen_llvm/declare.rs`: Systematically applies attributes during function declaration.
- **Zig** `src/codegen.zig`: Every function declaration includes complete attribute metadata.

---

## 02.1 noreturn on Panic Functions

**File(s):** `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

`ori_panic_cstr` is declared with `cold` but not `noreturn`. Since this function always aborts/unwinds and never returns to its caller, LLVM should know this to:
- Eliminate unreachable code after panic calls (§06 synergy)
- Improve branch prediction hints
- Enable dead code analysis in the optimizer

**Infrastructure gap:** The `Attr` enum currently has `Nounwind`, `Cold`, `NoaliasReturn`, and `MemArgmemRW`. There is **no `Noreturn` variant** — it must be added and wired through runtime declaration emission in `compiler/ori_llvm/src/codegen/runtime_decl/mod.rs`.

**Critical distinction:** `noreturn` (function never returns to caller) and `nounwind` (function never throws/unwinds) are **independent** LLVM attributes. `ori_panic_cstr` should get `noreturn` but **MUST NOT** get `nounwind` (runtime table comments already document that panic paths must unwind for RC cleanup).

- [ ] Add `Noreturn` variant to the `Attr` enum in `runtime_functions.rs`
- [ ] Wire `Attr::Noreturn` through the declaration machinery to emit the LLVM `noreturn` function attribute
- [ ] Add `Attr::Noreturn` to `ori_panic_cstr` declaration (alongside existing `Attr::Cold`)
- [ ] Add `Attr::Noreturn` to any other panic-path runtime functions (e.g., `ori_panic`)
- [ ] Do NOT add `Attr::Nounwind` to panic functions — they must unwind for RC cleanup
- [ ] Verify: IR for `panic()` calls shows `call void @ori_panic_cstr(...) #noreturn`
- [ ] Verify: call sites to proven-`noreturn` panic functions terminate the path (`unreachable` or equivalent no-return terminator)

---

## 02.2 nounwind on C main Wrapper

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

The C `main()` wrapper function calls `_ori_main` (which is typically nounwind) but the wrapper itself is never marked `nounwind`. This causes unnecessary exception table generation.

- [ ] Mark the C `main` wrapper as `nounwind` when `_ori_main` is nounwind
- [ ] Verify: `main` function in IR has `nounwind` attribute

---

## 02.3 nounwind on Derived Trait Methods

**File(s):** `compiler/ori_llvm/src/codegen/derive_codegen/mod.rs`, `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs`

Derived methods (`$eq`, `$compare`, `$hash`, `$clone`, `$debug`, `$to_str`) are emitted by `derive_codegen` outside the standard nounwind fixed-point analysis pipeline. Methods like `$eq` perform only pure comparisons (`extractvalue`, `icmp`, `load`, `switch`, `br`) and should be marked `nounwind`.

Two approaches:
- **(a)** Include derived methods in the nounwind fixed-point analysis (preferred — single source of truth)
- **(b)** Mark derived methods as `nounwind` in `derive_codegen` directly (simpler but duplicates logic)

- [ ] Choose approach and implement
- [ ] Verify: `$eq` methods in J11 IR have `nounwind` attribute
- [ ] Verify: `$compare`, `$hash` methods also get `nounwind` where applicable

---

## 02.4 nounwind on Runtime Declarations

**File(s):** `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

`ori_str_from_raw` is declared without `nounwind`, while `ori_str_len` and `ori_rc_dec` have it. This prevents functions that call `ori_str_from_raw` (like string comparison) from being marked `nounwind`.

- [ ] Audit all runtime function declarations for missing `nounwind`
- [ ] Add `nounwind` to `ori_str_from_raw` and any other safe runtime functions
- [ ] For each runtime function left without `nounwind`, add or confirm rationale (may panic, may allocate, or otherwise may unwind)
- [ ] Verify: Functions calling `ori_str_from_raw` can now be marked `nounwind` by the fixed-point analysis

---

## 02.5 nounwind on Indirect Closure Calls

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs`

Indirect calls through closure function pointers (e.g., `ApplyIndirect` in ARC IR) are conservatively excluded from nounwind analysis because the callee isn't statically known. This is correct in general but pessimistic when all actual callees are known to be nounwind at the module level.

**Note:** The original finding referenced `_ori_apply`, but this function does not exist in the current codebase. The issue applies to all indirect closure calls emitted by `ArcIrEmitter::emit_instr()` for `ApplyIndirect` instructions.

- [ ] Decide policy explicitly: conservative (document limitation) vs interprocedural proof
- [ ] If implementing interprocedural proof, require whole-module evidence that all possible callees are nounwind
- [ ] Add negative test where one closure target may unwind; indirect calls must remain without `nounwind`

---

## 02.6 noundef on Integer Parameters

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs`

Ori has no undefined scalar values at language level. Adding `noundef` tells LLVM that passing `undef`/`poison` is UB, enabling additional optimization and cleanup opportunities.

- [ ] Add `noundef` to scalar ABI parameters (`i64`, `i1`, `double`) first
- [ ] Add `noundef` to scalar return values where guaranteed defined
- [ ] Do not blanket-annotate aggregate/pointer values without proof obligations
- [ ] Verify: IR shows `noundef i64 %param` in function signatures
- [ ] Verify: No test regressions (noundef should be a pure optimization hint)

---

## 02.7 Completion Checklist

- [ ] `ori_panic_cstr` has `noreturn` attribute
- [ ] C `main` wrapper has `nounwind` when `_ori_main` is nounwind
- [ ] All derived trait methods include `nounwind` where applicable
- [ ] All safe runtime declarations have `nounwind`
- [ ] Runtime declaration table has an explicit yes/no nounwind rationale for every panic-capable runtime function
- [ ] `noundef` on proven-defined scalar parameters and scalar returns
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`
- [ ] `opt-21 -passes=verify` clean on representative journey IR

**Exit Criteria:** `grep` for function declarations in emitted IR shows: all panic functions have `noreturn`, all pure functions have `nounwind`, all integer parameters have `noundef`. Zero attribute gaps across all 12 code journeys.
