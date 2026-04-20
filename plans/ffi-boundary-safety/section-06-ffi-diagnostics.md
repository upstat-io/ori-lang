---
id: "06"
title: "FFI-Aware Diagnostics"
status: not-started
reviewed: false
goal: "Every E40xx FFI-family diagnostic produced by §01–§05 carries a secondary span pointing at the relevant C declaration when `#header(...)` is in use. Users never have to manually cross-reference Ori errors against C headers."
success_criteria:
  - "All E40{15..25} error codes shipped with error-doc pages in `compiler/ori_diagnostic/src/errors/`."
  - "Struct layout mismatch, signature mismatch, missing #free, invalid error protocol, callback capability mismatch all emit secondary spans pointing at the C header when `#header(...)` is in use."
  - "System header paths in diagnostics show library origin (e.g., `/usr/include/sqlite3.h (libsqlite3-dev)`) when resolvable."
  - "Diagnostics degrade gracefully when `#header(...)` is NOT in use — single-span errors, no C references, NO regression vs current UX."
  - "libclang is NOT required at diagnostic-render time — extracted declarations are cached in build artifacts."
inspired_by:
  - "Rust rustc secondary spans: `compiler/rustc_errors/src/diagnostic.rs`."
  - "Elm error messages: diff-style pointing at both user code and prior context."
  - "Swift ClangImporter diagnostics: dual-span Swift vs ObjC mismatch reporting."
depends_on:
  - "01"
  - "02"
third_party_review:
  status: none
  updated: null
---

# §06 — FFI-Aware Diagnostics

## Context

Per approved proposal §6. Incremental — each diagnostic lands with the section that emits it. §06 as a plan subsection is the coordination point: ensures the secondary-span infrastructure is in place early so §01–§05 emit consistent diagnostics. The content itself lands distributed across §01 (no E40xx codes), §02 (E4016, E4020, E4021), §03 (E4015, E4025), §04 (E4017, E4018, E4024), §05 (E4019), §07 (E4022, E4023).

## §06.1 — Secondary-span infrastructure

- [ ] Check `compiler/ori_diagnostic/src/` for existing secondary-span rendering support (likely present — `Diagnostic::with_label()` supports multiple spans). Confirm.
- [ ] Add a `SpanOrigin::CHeader { path, line, col, library_origin: Option<String> }` variant to the span source tagging (if not already present), allowing the renderer to prefix secondary spans with `C header:` rather than `note:`.
- [ ] Implement library-origin resolution: when a system include path (`/usr/include/sqlite3.h`) is used, query `dpkg -S` (Linux) / `pkg-config --libs` (cross-platform) / similar to identify the owning package. Cache per-header.
- [ ] No runtime libclang invocation for diagnostic rendering — spans are baked into diagnostic payloads at the point of error detection (which has libclang in context via §02's cache).

## §06.2 — Error code pages

- [ ] Write `compiler/ori_diagnostic/src/errors/E40{15..25}.md` — each page explains the error, shows example triggering code, shows the fix.
- [ ] E4015 — Struct layout mismatch (§03).
- [ ] E4016 — Signature mismatch between Ori and C header (§02).
- [ ] E4017 — #borrow_from references a non-parameter identifier (§04).
- [ ] E4018 — Borrowed return outlives its source parameter's scope (§04).
- [ ] E4019 — Callback capability subset violation (§05).
- [ ] E4020 — #header path not resolvable (§02).
- [ ] E4021 — C header parse error bubbled from libclang (§02).
- [ ] E4022 — #strict handler: call to unmocked function (§07).
- [ ] E4023 — Replay divergence from recorded trace (§07).
- [ ] E4024 — Invalid combination of owned / borrowed / #borrow_from (§04).
- [ ] E4025 — #verify_layout target struct not found in header (§03).
- [ ] Register error codes in `compiler/ori_diagnostic/src/error_code/mod.rs` alongside existing E4001–E4005.
- [ ] Update `compiler/ori_diagnostic/src/error_code/tests.rs` with the new codes.

## §06.3 — Graceful degradation

- [ ] When an E40xx diagnostic fires but `#header(...)` is NOT in use (e.g., hand-written extern with explicit signatures), emit the primary span only — NO C header reference, NO attempt to parse a header.
- [ ] Regression check: existing approved-Deep-FFI programs (no `#header`) produce diagnostics byte-identical to pre-§06 output (except for error code numbering which is additive).

## §06.4 — Matrix tests

- [ ] **Failing tests FIRST:** a struct layout mismatch under `#header(...)` MUST emit E4015 with both spans. Under hand-written extern, the same diagnostic MUST emit single-span only.
- [ ] **Matrix:**
  - **Error code coverage:** one test per E40{15..25} code = 11 cells.
  - **Secondary span presence:** `{#header in use}` × `{hand-written}` = 2 cells per code.
  - **Library origin:** system header (`/usr/include/*`) with resolvable owner / system header without resolvable owner / user-provided header path = 3 cells.
- [ ] **Semantic pin:** an E4015 diagnostic output contains both primary and secondary spans when header is present; does NOT when absent.
- [ ] **Cache validation:** consecutive compiles of the same program produce identical diagnostics without re-invoking libclang.

## §06.5 — Rule documentation

- [ ] Add E40xx FFI subrange entry to `.claude/rules/diagnostic.md` error-code range table; cite this plan.
- [ ] No changes to `impl-hygiene.md` diagnostic rules — §06 uses existing structured-construction patterns.

## Completion Checklist

- [ ] §06.1 secondary-span infrastructure works; library-origin resolution caches.
- [ ] §06.2 all 11 error-doc pages written; registered; tests updated.
- [ ] §06.3 graceful degradation verified — no regression on hand-written extern diagnostics.
- [ ] §06.4 matrix tests green; semantic pin green; cache validation passes.
- [ ] §06.5 diagnostic.md updated with E40xx FFI subrange entry.
- [ ] `./test-all.sh`, `./clippy-all.sh` green.
- [ ] `/tpr-review` clean; `/impl-hygiene-review` clean.
- [ ] `/improve-tooling` + `/sync-claude` sweeps.
- [ ] `sections[id=06].status: complete`, `reviewed: true`.

## §06.R — Third Party Review Findings

- None.
