---
id: "08"
title: "Spec, Grammar, and ori-syntax.md Sync"
status: not-started
reviewed: false
goal: "Propagate every attribute, grammar production, and type/method addition from §01–§07 to the authoritative surface: spec clause 26 (FFI), grammar.ebnf Annex A, and .claude/rules/ori-syntax.md. Cross-cutting — ships incrementally with each implementation section; §08 is the final aggregate check."
success_criteria:
  - "Every new attribute (#header, #header_path, #borrow_from, #verify_layout, #strict) appears in the spec with normative text (shall/NOTE/EXAMPLE per `.claude/rules/spec.md`)."
  - "Every new grammar production (extern_block_attr additions, extern_item_attr additions, callback_type uses list, handler_attr) appears in grammar.ebnf."
  - "Every new type/method (BorrowedView, Trace, ReplayDivergenceError, TraceFormat, record, replay, to_owned) appears in the appropriate stdlib doc surface AND in `.claude/rules/ori-syntax.md` quick-reference."
  - "`/sync-spec` and `/sync-grammar` both run clean on the aggregate change set at plan close."
  - "Errata block already added to approved deep-ffi-proposal.md (done at /review-draft-proposal time — this section verifies it persists)."
inspired_by: []
depends_on:
  - "01"
  - "02"
  - "03"
  - "04"
  - "05"
  - "06"
  - "07"
third_party_review:
  status: none
  updated: null
---

# §08 — Spec, Grammar, and `ori-syntax.md` Sync

## Context

The approved proposal's §Spec & Grammar Impact lists the concrete additions. §08's job is coordination: ensure each implementation section co-commits its own spec surface, and run `/sync-spec` + `/sync-grammar` at plan close to catch aggregate drift.

## §08.1 — Per-section spec surface (reminders)

Each implementation section is responsible for its own spec co-commit. §08 tracks the aggregate:

| Section | Spec surface | Grammar surface | ori-syntax.md surface |
|---------|-------------|-----------------|----------------------|
| §01 | §26-ffi.md subsection "AIMS at the FFI boundary" | — (no new grammar) | `.claude/rules/aims-rules.md §5` note (done in §01.6) |
| §02 | §26-ffi.md subsection "Header-assisted extern blocks" | extern_block_attr additions | Deep FFI section: `#header`, `#header_path`, type mapping table |
| §03 | §26-ffi.md subsection "Struct layout verification" | struct_attr additions | Deep FFI section: `#verify_layout` |
| §04 | §26-ffi.md subsection "Borrow-from lifetime annotations" | extern_item_attr additions | Deep FFI section: `#borrow_from`; aims-rules.md §1.5 Locality update (done in §04.8) |
| §05 | §26-ffi.md subsection "Callback capability propagation" | callback_type uses list | Deep FFI section: callback `uses` list |
| §06 | — (diagnostic rules only) | — | `.claude/rules/diagnostic.md` E40xx range note |
| §07 | §26-ffi.md subsection "Handler mocking formalization + record/replay" | handler_attr `#strict` | Deep FFI section: `#strict`, record/replay, `std.ffi.trace` types |

## §08.2 — Full-aggregate `/sync-spec` run

At plan close (after §01–§07 all complete):

- [ ] Run `Skill(skill: "sync-spec")` on the aggregate diff since plan start.
- [ ] Verify every synced claim pairs a Tier-A (intent) citation with a Tier-B (corroboration) citation per `/sync-docs` structured-pairing rule.
- [ ] Fix any Fact-Pairing holes before commit.

## §08.3 — Full-aggregate `/sync-grammar` run

- [ ] Run `Skill(skill: "sync-grammar")` on the aggregate grammar.ebnf change set.
- [ ] Verify every new production has a spec clause citing it per §08.1.
- [ ] Run `cargo st` (Ori spec tests) to confirm parser accepts every example embedded in spec clauses.

## §08.4 — `ori-syntax.md` quick reference

- [ ] Verify `.claude/rules/ori-syntax.md` Deep FFI section now includes:
  - [ ] `#header("file.h")` and `#header_path("path")` block-level attrs.
  - [ ] `#verify_layout("file.h")` struct attr.
  - [ ] `#borrow_from(p)` and `#borrow_from(a, b)` extern-item attrs.
  - [ ] Callback type `uses` list syntax.
  - [ ] `#strict` handler attr.
  - [ ] `std.ffi.trace` types: `Trace`, `ReplayDivergenceError`, `TraceFormat`.
  - [ ] `std.ffi.trace` functions: `record(to:, format:)`, `replay(from:)`.
  - [ ] `BorrowedView<T, ParamName>.to_owned()`.
- [ ] NO regressions on existing Deep FFI surface entries.

## §08.5 — Errata persistence check

- [ ] Verify `docs/ori_lang/proposals/approved/deep-ffi-proposal.md` retains the errata block added during `/review-draft-proposal` on 2026-04-20 (see `docs/ori_lang/proposals/approved/ffi-boundary-safety-proposal.md §Spec & Grammar Impact § Errata on Approved Proposal`).
- [ ] If a later edit has removed or mangled the errata, restore it.

## §08.6 — Superseded-proposal status check

- [ ] Verify `docs/ori_lang/proposals/superseded/c-header-import-proposal.md` retains:
  - [ ] `Status: Superseded`
  - [ ] `Superseded By: ffi-boundary-safety-proposal.md (approved 2026-04-20)`
  - [ ] Supersession Notice block at the top
- [ ] No draft-status regression.

## §08.7 — Cross-cutting propagation audit

- [ ] Grep the repo for stale references to `@cImport`, `c-header-import-proposal`, or the rejected Zig-style namespace pattern in rules / skills / docs / commit history of active plans.
- [ ] For each STALE reference: update to point at `#header(...)` + the approved proposal. For each ADJACENT reference: review — likely no change needed.

## Completion Checklist

- [ ] §08.1 per-section co-commits verified (spot-check the audit table).
- [ ] §08.2 /sync-spec full-aggregate run clean.
- [ ] §08.3 /sync-grammar full-aggregate run clean; cargo st green.
- [ ] §08.4 ori-syntax.md Deep FFI section complete.
- [ ] §08.5 deep-ffi-proposal.md errata persists.
- [ ] §08.6 c-header-import-proposal.md superseded status intact.
- [ ] §08.7 propagation audit complete; STALE references fixed.
- [ ] `./test-all.sh`, `cargo st` green.
- [ ] `/tpr-review` clean on the aggregate doc/spec/grammar surface.
- [ ] `sections[id=08].status: complete`, `reviewed: true`.
- [ ] Plan frontmatter `status: complete`; overview success criteria 6 (spec/grammar/syntax sync) checked.

## §08.R — Third Party Review Findings

- None.
