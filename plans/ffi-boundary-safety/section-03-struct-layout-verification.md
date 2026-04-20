---
id: "03"
title: "Compile-Time Struct Layout Verification"
status: not-started
reviewed: false
goal: "Verify every `#repr(\"c\")` Ori struct against its matching C header struct layout at compile time; emit E4015 with field-by-field diff on mismatch. Prevent silent memory corruption from library-upgrade-driven layout drift."
success_criteria:
  - "When a module has `extern \"c\" from \"lib\" #header(\"h.h\") { ... }` present, every `#repr(\"c\")` type in the same module is cross-checked against the header's struct with matching name."
  - "Mismatch (field count, field type, alignment, size) produces E4015 with primary span on the Ori type and secondary span on the C struct at the matching header line."
  - "Name-based field matching is preferred; positional fallback emits a warning noting the name divergence."
  - "Anonymous unions and bit-fields produce a 'complex layout — verify manually' advisory pointing at the relevant clang AST node rather than a hard error."
  - "Verification is opt-in via `#header(...)` OR explicit `#verify_layout(\"h.h\")` — does NOT impose runtime cost."
  - "E4025 fires when `#verify_layout(\"h.h\")` is applied but the named struct is not present in the header."
inspired_by:
  - "Rust `cxx` crate: shared-type definitions cross-verified at build time."
  - "Swift Clang Importer: automatic struct bridging with layout matching."
  - "Koka, Lean 4: no direct analog — §03 is a new capability for effect-system languages."
depends_on:
  - "01"
  - "02"
third_party_review:
  status: none
  updated: null
---

# §03 — Compile-Time Struct Layout Verification

## Context

Per approved proposal §3. Leverages the libclang pipeline from §02; adds a layout-diff pass run after `ori_repr` computes the `ReprPlan` for the Ori struct.

## §03.1 — `#verify_layout` attribute grammar + parser

- [ ] Grammar production (ships with §08 sync):
  ```ebnf
  struct_attr = "#verify_layout" "(" string_literal ")" | ... (existing) .
  ```
- [ ] Parser accepts `#verify_layout("header.h")` on struct declarations. Recorded on AST.
- [ ] Validation: `#verify_layout` requires the struct to have `#repr("c")` (soft requirement — other repr kinds would not match a C header meaningfully).

## §03.2 — Layout diff engine

- [ ] In `compiler/ori_repr/` (or a new `compiler/ori_cheader/layout.rs` — decided in design phase), add `fn diff_layouts(ori_plan: &ReprPlan, c_struct: &CStructDecl) -> Vec<LayoutDiscrepancy>`.
- [ ] `LayoutDiscrepancy` cases: field-count-mismatch, field-name-mismatch, field-type-mismatch, alignment-mismatch, size-mismatch.
- [ ] Field matching: primary by name (both sides have named fields); positional fallback with warning if Ori field names diverge from C names.
- [ ] Anonymous unions, bit-fields → emit "complex layout — verify manually" advisory (warning, not error). Cite the clang AST node's path in the header for the user to inspect.
- [ ] Runs during Salsa type-checking pass after `ori_repr::compute_repr_plan_with_interner` but before codegen (consumes the computed `ReprPlan`, runs in-memory, cacheable).

## §03.3 — Diagnostic emission

- [ ] E4015 primary span on Ori struct declaration; secondary span on C header struct declaration (file + line + column from libclang).
- [ ] Output format: field-by-field diff listing extra/missing fields with their names and types; NO version-attribution (per proposal §3 rule — the compiler has no library-version-history database).
- [ ] E4025 when `#verify_layout` target struct name is absent from the header.
- [ ] Diagnostic text adheres to `.claude/rules/diagnostic.md` structured-construction rule.

## §03.4 — Matrix tests

- [ ] **Failing tests FIRST:** a test Ori program declaring a `#repr("c")` struct with one field MISSING vs a fixture header containing the extra field — MUST emit E4015 with correct spans.
- [ ] **Matrix:**
  - **Mismatch kind × Field count:** `{missing field, extra field, wrong type, wrong alignment, wrong size}` × `{first, middle, last position}` = 15 cells.
  - **Field naming:** matching names / diverging names (positional match + warning) / anonymous union / bit-field = 4 cells.
  - **Opt-in path:** `{#header + #repr("c")}` / `{#verify_layout only}` / `{neither → no verification, no error}` = 3 cells.
- [ ] **Semantic pin:** a known-good Ori struct matching a fixture header exactly compiles clean. A minor modification (add a field in Ori but not in header) produces E4015. A reversion makes the error disappear.
- [ ] **Negative pin:** a program with `#verify_layout` naming a non-existent struct MUST emit E4025 at compile time.

## §03.5 — Advisory cases

- [ ] Anonymous unions → advisory pointing at the C header union declaration.
- [ ] Bit-fields → advisory listing bit-field offsets that cannot be compared structurally.
- [ ] `#repr("packed")` Ori struct vs non-packed C struct → explicit mismatch, NOT an advisory.

## Completion Checklist

- [ ] §03.1 grammar + parser + AST change.
- [ ] §03.2 layout diff engine ships with unit tests.
- [ ] §03.3 E4015 + E4025 emit with correct spans.
- [ ] §03.4 matrix tests green (15 + 4 + 3 cells), semantic pin green, negative pin green.
- [ ] §03.5 advisories don't false-positive as errors.
- [ ] `./test-all.sh`, `./clippy-all.sh`, `./llvm-test.sh` green.
- [ ] `/tpr-review` clean. `/impl-hygiene-review` clean.
- [ ] `/improve-tooling` + `/sync-claude` sweeps run.
- [ ] `sections[id=03].status: complete`, `reviewed: true`.

## §03.R — Third Party Review Findings

- None.
