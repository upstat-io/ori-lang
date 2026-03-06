---
section: "07"
title: "Constant Deduplication"
status: complete
goal: "Identical string constants share a single global — zero duplicates in emitted IR"
inspired_by:
  - "Rust rustc_codegen_llvm/common.rs — interns all string constants via const_str()"
  - "LLVM LangRef — unnamed_addr + linkonce_odr for constant merging"
depends_on: []
sections:
  - id: "07.1"
    title: "String Constant Interning"
    status: complete
---

# Section 07: Constant Deduplication

**Status:** Complete
**Goal:** Each unique string constant (e.g., `"integer overflow on addition\00"`) is emitted as a single LLVM global, shared across all use sites. Zero duplicate constant strings in emitted IR.

**Context:** The codegen emits identical overflow message strings as separate globals for each overflow check site. J2 has 2 duplicates, J7 has 6, J9 has 7, J12 has 6. While LLVM's linker may merge `unnamed_addr` constants at link time, the IR is unnecessarily verbose and the duplicate creation wastes module-level resources.

**Journeys affected:** J2, J3, J4, J6, J7, J8, J9, J10, J11, J12. (10 of 12 journeys — this is the single most pervasive finding.)

**Reference implementations:**
- **Rust** `rustc_codegen_llvm/common.rs`: Uses `const_str()` which interns all string constants — same string → same global.
- **LLVM** itself: `unnamed_addr` constants with identical content are candidates for COMDAT folding at link time, but emitting one global from the start is strictly better.

---

## 07.1 String Constant Interning

**File(s):** `compiler/ori_llvm/src/codegen/ir_builder/constants.rs`, `compiler/ori_llvm/src/codegen/ir_builder/checked_ops.rs`

The duplicate globals came from `build_global_string_ptr()`, not `const_string()`. `const_string()` creates inline byte arrays; `build_global_string_ptr()` creates named global string pointers — the ones that duplicated. It's called from:
- `compiler/ori_llvm/src/codegen/ir_builder/checked_ops.rs` — overflow panic messages (extracted from `arithmetic.rs`). Was the primary source of duplicates: `emit_checked_binop()` called the raw inkwell `self.builder.build_global_string_ptr()` directly. Fixed to use `self.build_global_string_ptr()` (the IrBuilder wrapper with dedup cache).
- `compiler/ori_llvm/src/codegen/arc_emitter/value_emission.rs` (line 45) — string literal emission (uses `IrBuilder` wrapper — benefits from cache automatically)
- `compiler/ori_llvm/src/codegen/derive_codegen/string_helpers.rs` (line 30) — derive codegen string construction (uses `fc.builder_mut().build_global_string_ptr()` which IS the IrBuilder wrapper — benefits from cache automatically)

**Correction from original plan:** Only 1 of 3 call sites bypassed the IrBuilder wrapper (arithmetic.rs), not 2. `string_helpers.rs` already went through the IrBuilder via `FunctionCompiler::builder_mut()`.

**Implementation:** Added `global_strings: FxHashMap<String, ValueId>` to `IrBuilder`. `build_global_string_ptr()` checks cache by content before creating globals. New globals marked with `unnamed_addr` (Global) for linker-level COMDAT folding.

- [x] **Split `arithmetic.rs`** into submodules BEFORE other §07 work (513 → 352 lines in `arithmetic.rs` + 162 lines in `checked_ops.rs`)
- [x] Add a `FxHashMap<String, ValueId>` to the IR builder codegen state
- [x] Modify `build_global_string_ptr()` in `constants.rs` to check cache before creating globals
- [x] Refactor `emit_checked_binop()` in `checked_ops.rs` to use `IrBuilder::build_global_string_ptr()` instead of raw inkwell `self.builder.build_global_string_ptr()` — also refactored panic call to use `self.call()`
- [x] `derive_codegen/string_helpers.rs` already uses IrBuilder wrapper (via `fc.builder_mut()`) — no refactoring needed (plan was incorrect about bypass)
- [x] Cache key uses full byte content (the `value: &str` parameter), not the display label (`name`)
- [x] Mark deduplicated globals with `unnamed_addr` (Global) to enable linker-level COMDAT folding
- [x] Verify: J7 IR has exactly 1 `"integer overflow on addition\00"` global (was 6)
- [x] Verify: J9 IR has exactly 1 of each overflow message (was 7)
- [x] Count: All 12 journeys now have exactly 1 global per unique overflow message — zero duplicates
- [x] Unit test in IrBuilder: `global_string_ptr_dedup_same_content`, `global_string_ptr_different_content_distinct`, `global_string_ptr_unnamed_addr`

### 07.1 Completion Checklist

- [x] String constant cache implemented in IR builder (`global_strings: FxHashMap<String, ValueId>`)
- [x] `build_global_string_ptr()` deduplicates by content, not by name
- [x] Count of global definitions for each overflow string is 1 per module
- [x] No duplicate `@.str.*` globals with identical content
- [x] J7 IR has exactly 1 `"integer overflow on addition\00"` global
- [x] J9 IR has exactly 1 of each overflow message
- [x] Deduplicated globals have `unnamed_addr` for linker-level folding
- [x] IR test: program with 3 overflow sites has 1 overflow message global (not 3) — `global_string_ptr_dedup_same_content`
- [x] `./test-all.sh` green (12,067 tests, 0 failures)
- [x] `./clippy-all.sh` green
- [x] No regressions in `cargo test -p ori_llvm` (428 tests, 3 new)

---

## Section 07 Exit Criteria

For any program, `ORI_DUMP_AFTER_LLVM=1` shows at most one global per unique string value. No duplicated string constants in emitted IR for any of the 12 code journeys. ✓ Verified.
