---
section: "02"
title: "TagAccess Full Migration + Derive Fix"
status: not-started
reviewed: false
goal: "Migrate ALL 88 codegen consumer sites to use TagAccess + EnumLayoutInfo, eliminating every hardcoded enum field index — fixing the active tagged-pointer miscompile in derive enum bodies"
inspired_by:
  - "Swift GenEnum.cpp — single dispatch point for all enum layout strategies"
  - "Rust rustc_codegen_ssa::mir::operand — layout-aware operand access"
depends_on: ["01"]
success_criteria:
  - "Zero hardcoded extract_value(..., 0/1) for enum tag/payload outside tag_access/"
  - "Derive enum bodies (enum_eq, enum_comparable, enum_hashable) handle ALL EnumTag variants correctly"
  - "build_option_struct/build_result_struct use EnumLayoutInfo for indices"
  - "AOT tests verify derive correctness for Explicit, TaggedPtr, and Niche encodings"
  - "All 17,000+ tests pass in debug and release"
sections:
  - id: "02.1"
    title: "Derive Enum Bodies Fix (URGENT)"
    status: not-started
  - id: "02.2"
    title: "Option/Result Builtins Migration"
    status: not-started
  - id: "02.3"
    title: "Remaining Consumer Migration + Packing Consolidation"
    status: not-started
  - id: "02.4"
    title: "Completion Checklist"
    status: not-started
third_party_review:
  status: none
  updated: null
---

# Section 02: TagAccess Full Migration + Derive Fix

**Context:** 66 codegen consumer sites across 10+ files hardcode enum field indices (`extract_value(..., 0)` for tag, `extract_value(..., 1)` for payload). This includes an **active miscompile**: derive enum bodies (enum_eq.rs, enum_comparable.rs, enum_hashable.rs) assume `{tag, payload}` struct layout but `TAGGED_PTR_CODEGEN_READY` is already true — user-defined enums can qualify for tagged-pointer layout TODAY. When they do, derives produce wrong code (Eq returns false, Comparable returns Equal, Hashable panics). §02.1 fixes this urgently.

**Approach:** TagAccess already exists in `compiler/ori_llvm/src/codegen/arc_emitter/tag_access/mod.rs` with 22 consumer sites using it correctly. This section migrates the remaining 66 sites. TagAccess becomes a thin LLVM adapter that reads `EnumLayoutInfo` from `ori_repr` (§01) and translates queries into LLVM builder calls.

**Phase ordering:** §02.1 (derive fix) is URGENT and must be done first. §02.2 (builtins) and §02.3 (remaining) follow.

---

## 02.1 Derive Enum Bodies Fix (URGENT — Active Miscompile)

**File(s):** `compiler/ori_llvm/src/codegen/derive_codegen/enum_bodies/enum_eq.rs`, `enum_comparable.rs`, `enum_hashable.rs`

**Why URGENT:** `TAGGED_PTR_CODEGEN_READY = true` (verified at `canonical/type_repr.rs:251`). User-defined enums like `type MaybeIter = Empty | Holds(it: Iterator<int>)` qualify for tagged-pointer layout via `can_use_tagged_pointer()`. When derive Eq/Comparable/Hashable runs on such enums, the code does `extract_value(self_val, 0, "eq.tag.self")` — but the LLVM type for tagged-pointer enums is `i64` (not `{i64, payload}`), so `extract_value(..., 0)` either extracts nothing or produces garbage. Eq returns false on the failed extraction; Comparable returns Equal; Hashable panics at `enum_hashable.rs:86`.

- [ ] **Write failing TDD matrix FIRST** — AOT tests in `compiler/ori_llvm/tests/aot/`:
  - `enum_derive_tagged_ptr.rs`: Test `#derive(Eq)` on a tagged-pointer-eligible enum → currently produces wrong result. Pin: after fix, equality compares correctly for both the unit and pointer variants.
  - `enum_derive_explicit_tag.rs`: Regression pin — existing explicit-tag derives still work correctly after the migration.
  - Matrix: Eq × Comparable × Hashable for Explicit, TaggedPtr, (Niche gated by NICHE_CODEGEN_READY=false)

- [ ] **Add TagAccess-aware dispatch to each derive file.** Before the existing `extract_value(..., 0)` tag extraction, query `EnumLayoutInfo` to determine the tag encoding strategy. Dispatch:
  - `Explicit` → existing code path (extract tag at `tag_gep_index`, payload at `payload_gep_index`)
  - `TaggedPtr` → use `tagged_ptr_decode_tag()` and `tagged_ptr_decode_ptr()` from TagAccess
  - `Niche` → extract the niche field and compare against the niche value (gated by NICHE_CODEGEN_READY)
  - `None` → single variant, no tag comparison needed

- [ ] **Migrate enum_eq.rs** (lines 34-35, 118-122, 195-200):
  - Replace `extract_value(self_val, 0, "eq.tag.self")` with TagAccess-aware tag extraction
  - Replace `extract_value(self_val, 1, "eq.*.payload")` and `struct_gep(..., 1, ...)` with payload access via TagAccess
  - Replace `i64_offset += field_bytes.div_ceil(8).max(1)` with `slot_count()` from §01

- [ ] **Migrate enum_comparable.rs** — same pattern as enum_eq.rs (lines 31-34, 102-107, 206-207)

- [ ] **Migrate enum_hashable.rs** — same pattern (lines 36, 78-85, 165-166)

- [ ] **Verify** `./test-all.sh` passes in both debug and release. `ORI_CHECK_LEAKS=1` on derive-related test programs.

- [ ] **Subsection close-out (02.1)** — Run `/improve-tooling` retrospectively. Commit separately.
- [ ] `/sync-claude` **section-close doc sync** — verify Claude artifacts across all section commits. Map changed crates to rules files, check CLAUDE.md, canon.md. Fix drift NOW.
- [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 02.2 Option/Result Builtins Migration

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs` (15 sites), `option_result_helpers.rs` (10 sites), `compound_type_impls/option.rs` (6 sites), `compound_type_impls/result.rs` (5 sites), `debug_helpers.rs` (3 sites), `collections/list_builtins/helpers.rs` (2 sites)

**Note:** Option and Result have a runtime ABI contract — `ori_rt` writes `{i64 tag, T payload}` via C functions. The `EnumLayoutInfo` for Option/Result will have `is_runtime_abi: true`, which means TagAccess must use the runtime-compatible encoding regardless of what niche analysis says. This contract is enforced by `canonical_option()` and `canonical_result()` using `IntWidth::I64` tags while `NICHE_CODEGEN_READY = false`.

- [ ] **Migrate `build_option_struct()` and `build_result_struct()`** in `option_result_helpers.rs`:
  - Read `tag_gep_index` and `payload_gep_index` from `EnumLayoutInfo` instead of hardcoding 0 and 1
  - Use `tag_encoding.tag_width()` instead of hardcoding `i64_ty`
  - These 2 functions transitively fix 9 call sites that use them

- [ ] **Migrate `resolve_type_for_option()` and `resolve_type_for_result()`** in `option_result_monadic.rs`:
  - Read struct layout from `EnumLayoutInfo` instead of constructing `{i64, T}` inline

- [ ] **Migrate tag extraction sites** in `option_result.rs` (15 sites), `compound_type_impls/option.rs` (6), `result.rs` (5), `debug_helpers.rs` (3), `list_builtins/helpers.rs` (2):
  - Replace `extract_value(receiver, 0, "opt.tag")` with `tag_access.extract_tag(receiver)` or equivalent
  - Replace `extract_value(receiver, 1, "opt.payload")` with `tag_access.extract_payload(receiver, variant_idx)`

- [ ] **AOT regression tests**: Run existing Option/Result AOT tests. Add 2 new tests:
  - `option_result_abi_contract.rs`: Verify Option<int> and Result<str, Error> roundtrip correctly through runtime C functions (ori_list_first, ori_map_get)
  - Verify the is_runtime_abi flag prevents niche optimization from changing Option/Result layout

- [ ] **Subsection close-out (02.2)** — Run `/improve-tooling` retrospectively. Commit separately.
- [ ] `/sync-claude` **section-close doc sync** — verify Claude artifacts across all section commits. Map changed crates to rules files, check CLAUDE.md, canon.md. Fix drift NOW.
- [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 02.3 Remaining Consumer Migration + Packing Consolidation

**File(s):** `instr_dispatch.rs` (1 payload site), `rc_helpers.rs` (1 payload site), `drop_enum.rs` (1 packing site), `abi/mod.rs` (2 packing sites), `type_info/enum_layout.rs` (2 packing sites)

- [ ] **Migrate remaining hardcoded payload access**:
  - `instr_dispatch.rs:140`: `extract_value(val, 1, "proj.payload")` → TagAccess payload extraction
  - `rc_helpers.rs:89`: `extract_value(val, 1, "rc.opt_inner")` → TagAccess payload extraction

- [ ] **Replace scattered packing computations** with `ori_repr::layout::slot_padded_size()` and `slot_count()` from §01:
  - `drop_enum.rs:380`: `current += field_size.div_ceil(8).saturating_mul(8)` → `current += slot_padded_size(field_size)`
  - `abi/mod.rs:178,268`: `size.div_ceil(8) * 8` → `slot_padded_size(size)`
  - `enum_layout.rs:100,117`: `size.div_ceil(8) * 8` and `max_payload_bytes.div_ceil(8)` → `slot_padded_size()` and `slot_count()`

- [ ] **Run grep audit** to verify ZERO remaining scattered computations:
  ```bash
  rg 'div_ceil\(8\)' compiler/ --glob '!**/ori_repr/**' --glob '!**/tests*'
  ```
  Must return zero matches.

- [ ] **Run grep audit** for hardcoded enum indices:
  ```bash
  rg 'extract_value\(.*,\s*[01]\s*,' compiler/ori_llvm/ --glob '!**/tag_access/**' --glob '!**/tests*'
  ```
  Review every match — must be struct field access (legitimate) not enum tag/payload access.

- [ ] **Subsection close-out (02.3)** — Run `/improve-tooling` retrospectively. Commit separately.
- [ ] `/sync-claude` **section-close doc sync** — verify Claude artifacts across all section commits. Map changed crates to rules files, check CLAUDE.md, canon.md. Fix drift NOW.
- [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 02.4 Completion Checklist

- [ ] All 66 hardcoded consumer sites migrated to TagAccess + EnumLayoutInfo
- [ ] Derive enum bodies handle ALL EnumTag variants (Explicit, TaggedPtr, Niche gated, None)
- [ ] Active tagged-pointer miscompile fixed and pinned with AOT test
- [ ] `build_option_struct` / `build_result_struct` use EnumLayoutInfo
- [ ] Grep audit confirms zero scattered packing computations outside ori_repr
- [ ] Grep audit confirms zero hardcoded enum indices outside tag_access
- [ ] `./test-all.sh` green in debug and release
- [ ] `./clippy-all.sh` green
- [ ] `ORI_CHECK_LEAKS=1` clean on derive and Option/Result test programs
- [ ] `/tpr-review` passed — independent review clean
- [ ] `/impl-hygiene-review` passed — MUST run AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` section-close sweep completed

---

## 02.R Third Party Review Findings

- None.
