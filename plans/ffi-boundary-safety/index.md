---
plan: "ffi-boundary-safety"
reroute: true
name: "ffi-boundary-safety"
full_name: "FFI Boundary Safety — Deep FFI Part 2"
status: queued
order: 14
---

# FFI Boundary Safety — Keyword Index

Discovery index for `plans/ffi-boundary-safety/`. Each section below lists the identifiers, file paths, and concepts a reader is most likely to grep for when landing on this plan.

## §01 — AIMS FFI Contract Extension

- Files: `compiler/ori_arc/src/aims/interprocedural/extract.rs`, `compiler/ori_arc/src/aims/contract/mod.rs`, `compiler/ori_arc/src/aims/contract/context.rs`
- Types: `MemoryContract`, `ParamContract`, `ReturnContract`, `EffectSummary`, `FipContract`
- Concepts: interprocedural SCC fixpoint, `ori_from_ffi`, FFI contract fragment, Invariant #5 (unified model), `aims-rules.md §5` interprocedural contracts
- Spec/Rule anchors: `canon.md §7.1`, `arc.md §"Non-Negotiable Invariants"`, `aims-rules.md §§5–7`, `CLAUDE.md §AIMS`

## §02 — Header-Assisted Extern Blocks — `#header(...)`

- Files: `compiler/ori_parse/src/grammar/item/extern_def.rs`, new crate (proposed: `compiler/ori_cheader/`) wrapping libclang
- Attributes: `#header("file.h")`, `#header_path("path")`
- Concepts: libclang AST walk, explicit function list, signature auto-derivation, C type → Ori type mapping table
- Distinctions: vs `@cImport` (namespace, rejected), vs `ori bindgen` (external tool, deferred)
- Dependencies: pkg-config resolution order (explicit > pkg-config > system includes > E4020)

## §03 — Compile-Time Struct Layout Verification

- Attributes: `#verify_layout("file.h")`
- Concepts: field-by-field layout diff, C struct matching by name then position, anonymous unions/bitfields → "complex layout" warning
- Error: E4015 struct layout mismatch
- Depends on: §02 libclang pipeline

## §04 — `#borrow_from(param)` — Lifetime Annotations

- Attributes: `#borrow_from(p)`, `#borrow_from(a, b)` multi-param
- Types: `BorrowedView<T, ParamName>` (opaque; compiler-constructed)
- Concepts: `Locality::Borrowed(p)` AIMS dimension extension, single-level borrow (no struct-field paths), `.to_owned()` copy escape, scope intersection for multi-param
- Errors: E4017 (non-parameter named), E4018 (outlives source), E4024 (invalid combination with `owned`/`borrowed`)
- Reference: `aims-rules.md §1.5` Locality, `utf8_common_prefix` example in proposal §4

## §05 — Callback Capability Propagation

- Concepts: callback parameter type with `uses Cap1, Cap2`, subset verification at registration, handler-based capability provision at registration site
- Errors: E4019 capability subset violation
- BLOCKED_BY: `capability-propagation-completion-proposal.md` (draft) — LLVM/AOT capability support, stateful handler completion, marker enforcement
- Examples: `uv_timer_start` with `uses Clock` callback

## §06 — FFI-Aware Diagnostics

- Files: `compiler/ori_diagnostic/src/errors/E40{15..25}.md` (new), secondary-span rendering in `ori_diagnostic`
- Error codes: E4015 (layout mismatch), E4016 (signature mismatch), E4017 (#borrow_from non-param), E4018 (borrowed-outlives-source), E4019 (capability subset), E4020 (header not resolvable), E4021 (header parse error), E4022 (strict handler unmocked call), E4023 (replay divergence), E4024 (invalid owned/borrowed/#borrow_from combination), E4025 (#verify_layout target struct not found)
- Concepts: C header path resolution in diagnostics (`/usr/include/sqlite3.h (libsqlite3-dev)`), libclang caching
- Rule: `.claude/rules/diagnostic.md`

## §07 — Handler-Mocking Formalization

- 7a `#strict` attribute on handlers — rejects unmocked calls at compile time
- 7b stateful mocks with `handler(state: S)` — already in approved stateful-mock-testing
- 7c record/replay via `std.ffi.trace`
- Types: `Trace`, `ReplayDivergenceError`, `TraceFormat` (Text/Binary)
- Functions: `record(to:, format:)`, `replay(from:)`
- New file: `library/std/ffi/trace.ori`
- Concepts: deterministic-by-construction replay (D4), ReplayDivergenceError diff, text-serialized traces for human-reviewable diffs

## §08 — Spec / Grammar / ori-syntax Sync

- Files: `docs/ori_lang/v2026/spec/26-ffi.md`, `docs/ori_lang/v2026/spec/grammar.ebnf`, `.claude/rules/ori-syntax.md`
- Grammar additions: `extern_block_attr`, `extern_item_attr`, `callback_type`, `handler_attr`
- Scoping: cross-cutting — each implementation section co-commits its spec surface
- Gate: `/sync-spec` + `/sync-grammar` for full-aggregate check at plan close

## Cross-Plan References

- `plans/repr-opt/section-06-struct-layout.md` — repr system §03 layout verification shares infrastructure with
- `plans/deep-safety/` — capability work where §05 blocker lives
- `plans/llvm-verification-tooling/` — AIMS verification test suites §01 will extend
