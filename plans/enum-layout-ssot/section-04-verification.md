---
section: "04"
title: "Verification & Testing"
status: not-started
reviewed: false
goal: "Replace source-text niche-helper tests with emitter-driven IR verification, add codegen gate audit ensuring no TagAccess bypass, and run full integration verification"
inspired_by:
  - "Swift SIL tests — IR-level retain/release verification"
  - "Koka golden files — exact IR shape matching"
  - "Rust tests/crashes/ — regression corpus for ICE cases"
depends_on: ["02", "03"]
success_criteria:
  - "Emitter-driven IR tests exercise actual LLVM IR emission for niche helpers"
  - "Codegen gate audit script detects any TagAccess bypass"
  - "All mission success criteria from 00-overview.md verified"
  - "Dual-exec parity confirmed for all new tests"
  - "ORI_CHECK_LEAKS=1 clean on all enum-related programs"
sections:
  - id: "04.1"
    title: "Emitter-Driven Niche Tests"
    status: not-started
  - id: "04.2"
    title: "Codegen Gate Audit"
    status: not-started
  - id: "04.3"
    title: "Integration Verification"
    status: not-started
  - id: "04.4"
    title: "Completion Checklist"
    status: not-started
third_party_review:
  status: none
  updated: null
---

# Section 04: Verification & Testing

**Context:** TPR-07-006-codex confirmed that niche-helper tests in `option_result_helpers/tests.rs` use `include_str!` source-text matching instead of actual IR emission — they test the text of the source file, not the LLVM IR the helpers produce. This section replaces those tests with emitter-driven verification and adds a codegen gate audit to prevent future TagAccess bypass.

**Note on finding 6 (niche stubs):** The niche monadic stubs (`emit_option_niche`, `emit_result_niche`) still return `None` — this plan does NOT implement them. Niche stub implementation is deferred to repr-opt §07.2's NICHE_CODEGEN_READY gate flip. This section fixes the TESTS (finding 9), not the stubs (finding 6). The emitter-driven tests directly invoke the niche helpers from a synthetic harness, bypassing the gate, so they exercise the dead-code path as a regression guard.

---

## 04.1 Emitter-Driven Niche Tests

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers/tests.rs` (rewrite)

The implementation plan from TPR-07-018 (already documented in `plans/repr-opt/section-07-enum-repr.md` lines 754-768) describes the exact test architecture. This subsection executes it.

- [ ] **Read the existing test file** to understand what it currently does (source-text assertions via `include_str!`)

- [ ] **Build synthetic emitter harness** following the pattern from `compiler/ori_llvm/src/codegen/arc_emitter/tests.rs` (`drop_fn_trivial_generates_rc_free` pattern):
  1. Construct a `Pool` with `pool.option(Idx::STR)` for Option<str>, `pool.result(Idx::STR, Idx::STR)` for Result<str, str>
  2. Create LLVM `Context`, `SimpleCx`, `IrBuilder`
  3. Declare runtime functions via `declare_runtime_functions` or similar
  4. Create a host function with no params, position builder at entry
  5. Construct synthetic `TagEncoding::new(EnumTag::Niche { field_index: 0, niche_value: 0, niche_variant_idx: 1 }, 2)` for Option
  6. Allocate synthetic receiver via `builder.const_zero_ty(opt_str_llvm_ty)`

- [ ] **Write emitter-driven tests** (replace ALL source-text assertions):
  - `emit_option_niche_unwrap_contains_panic_and_rc_inc`: Call `em.emit_option_niche("unwrap", ...)`, assert IR contains `"ori_panic"` AND `"ori_str_rc_inc"`
  - `emit_option_niche_expect_contains_panic_and_rc_inc`: Same for `expect`
  - `emit_option_niche_unwrap_or_contains_conditional_rc_inc`: `unwrap_or` has RC inc on the conditional merge path (no panic)
  - `emit_result_niche_unwrap_and_unwrap_err_differ`: Assert IR for `Result.unwrap` and `Result.unwrap_err` are NOT identical (proves the collapsed-arm bug is fixed, not just textually present)
  - `emit_result_niche_methods`: Test all 5 Result methods produce valid IR

- [ ] **Negative pin**: Test that invoking a niche helper with a NON-niche encoding (explicit tag) returns `None` — the gate bypass is only for niche-encoded types

- [ ] **Remove include_str! assertions** from the test file — they are replaced by the emitter-driven tests. The file keeps its location (`option_result_helpers/tests.rs`) but the content is entirely rewritten.

- [ ] **Subsection close-out (04.1)** — Run `/improve-tooling` retrospectively. Commit separately.
- [ ] `/sync-claude` **section-close doc sync** — verify Claude artifacts across all section commits. Map changed crates to rules files, check CLAUDE.md, canon.md. Fix drift NOW.
- [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 04.2 Codegen Gate Audit

**File(s):** `diagnostics/` (new script or extension to existing tool)

- [ ] **Create `diagnostics/enum-layout-audit.sh`** — a script that detects TagAccess bypass:
  ```bash
  #!/bin/bash
  # Detect hardcoded enum field access that bypasses TagAccess.
  # Exit 0 if clean, exit 1 if violations found.
  
  # Check for hardcoded i64-slot packing outside ori_repr
  PACKING=$(rg 'div_ceil\(8\)' compiler/ --glob '!**/ori_repr/**' --glob '!**/tests*' -c)
  
  # Check for hardcoded enum indices outside tag_access
  # (filter legitimate struct field access via heuristic: enum names in the label)
  INDICES=$(rg 'extract_value\(.*,\s*[01]\s*,.*\b(tag|payload|opt\.|res\.)\b' \
    compiler/ori_llvm/ --glob '!**/tag_access/**' --glob '!**/tests*' -c)
  ```
  This script becomes a gate for future development — run it in CI or as part of `/impl-hygiene-review` to prevent regression.

- [ ] **Verify the audit passes** after §02 and §03 are complete — zero violations

- [ ] **Add to `./clippy-all.sh` or document as a manual gate** (depending on false-positive rate)

- [ ] **Subsection close-out (04.2)** — Run `/improve-tooling` retrospectively. Commit separately.
- [ ] `/sync-claude` **section-close doc sync** — verify Claude artifacts across all section commits. Map changed crates to rules files, check CLAUDE.md, canon.md. Fix drift NOW.
- [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 04.3 Integration Verification

- [ ] **Run `./test-all.sh`** — all 17,000+ tests pass in both debug and release
- [ ] **Run `./clippy-all.sh`** — clean
- [ ] **Run `diagnostics/valgrind-aot.sh`** on all enum-related test programs — clean
- [ ] **Run `ORI_CHECK_LEAKS=1`** on:
  - All `iterator_drop` AOT tests (16+ tests)
  - All derive-related enum AOT tests (new from §02.1)
  - All Option/Result spec tests
- [ ] **Dual-execution parity**: Verify interpreter and LLVM produce identical results for all new tests added in §02 and §03
- [ ] **Verify all mission success criteria** from `00-overview.md`:
  - [ ] `rg 'div_ceil\(8\)' compiler/ --glob '!**/ori_repr/**' --glob '!**/tests*'` returns zero
  - [ ] Hardcoded enum index grep audit clean
  - [ ] Derive enum bodies handle all EnumTag variants (verified by AOT tests)
  - [ ] `is_take_project` is MachineRepr-driven (verified by unit tests)
  - [ ] `debug_assert!` enforced (verified by debug-mode test suite pass)
  - [ ] Emitter-driven niche tests working (verified by `cargo test -p ori_llvm option_result_helpers`)

- [ ] **Subsection close-out (04.3)** — Run `/improve-tooling` retrospectively. Commit separately.
- [ ] `/sync-claude` **section-close doc sync** — verify Claude artifacts across all section commits. Map changed crates to rules files, check CLAUDE.md, canon.md. Fix drift NOW.
- [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 04.4 Completion Checklist

- [ ] Emitter-driven niche-helper tests replace all source-text assertions
- [ ] Codegen gate audit script created and passing
- [ ] All mission success criteria verified with specific commands
- [ ] Dual-exec parity confirmed
- [ ] Leak check clean on all enum programs
- [ ] Valgrind clean on enum test programs
- [ ] `./test-all.sh` green in debug and release
- [ ] `./clippy-all.sh` green
- [ ] `/tpr-review` passed — independent review clean
- [ ] `/impl-hygiene-review` passed — MUST run AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` section-close sweep completed
- [ ] All plan annotations removed (per CLAUDE.md plan annotation cleanup requirement)

---

## 04.R Third Party Review Findings

- None.
