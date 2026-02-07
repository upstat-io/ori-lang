---
section: "09"
title: Salsa & IDE Integration
status: complete
goal: "Integrate V2 lexer with Salsa incremental queries, formatter metadata extraction, and IDE support"
sections:
  - id: "09.1"
    title: Salsa Query Integration
    status: complete
  - id: "09.2"
    title: LexOutput Phase Output
    status: complete
  - id: "09.3"
    title: TriviaCollector
    status: complete
  - id: "09.4"
    title: Doc Comment Classification
    status: complete
  - id: "09.5"
    title: Tests
    status: complete
---

# Section 09: Salsa & IDE Integration

**Status:** Complete (2026-02-07)
**Goal:** Integrate the V2 lexer with Ori's Salsa-based incremental compilation pipeline, preserving early cutoff semantics. Provide structured trivia collection and doc comment classification for the formatter and IDE.

## Completion Summary

All subsections implemented across 8 commits on the `dev` branch:

| Commit | SHA | Description |
|--------|-----|-------------|
| 1 | `a9cf950` | Position-independent TokenList Hash/Eq for Salsa early cutoff |
| 2 | `2b01675` | LexOutput Salsa trait impls (Eq, Hash, Debug) |
| 3 | `0272fa1` | Add DocMember variant to CommentKind (additive) |
| 4 | `264a582` | Lexer classification for `* name:` syntax |
| 5 | `619cc28` | Formatter handles DocMember in comment reordering |
| 6 | `2f54b41` | Migrate test files from @param/@field to * name: syntax |
| 7 | `46a4d18` | Remove DocParam/DocField; legacy markers emit DocMember |
| 8 | `af12975` | Add tokens_with_metadata() Salsa query |

### Design Decisions Made

1. **Storage pattern**: Kept hybrid AoS+tags (not full SoA). The position-independent Hash/Eq on the existing hybrid structure provides early cutoff without a storage rewrite.

2. **Position-independent hashing**: `TokenList::Hash` and `TokenList::PartialEq` now compare only `TokenKind` + `TokenFlags`, skipping `Span` positions. This enables Salsa early cutoff when whitespace-only edits shift positions but don't change semantics.

3. **TriviaCollector**: Kept inline trivia collection in `lex_with_comments()` rather than extracting to a separate struct. The inline approach is simpler and well-tested.

4. **Blank line behavior**: Comments reset the blank line counter (comments are content, not trivia). Matches existing behavior.

5. **Doc comment migration**: `DocParam`/`DocField` replaced by unified `DocMember`. Legacy `@param`/`@field` markers emit `DocMember` (not deprecated — just reclassified). The `*` marker uses `* name: description` format.

6. **LexOutput Salsa traits**: Added manual `PartialEq`, `Eq`, `Hash`, `Debug` impls to the existing `LexOutput` struct (no `LexOutputV2` needed).

> **REFERENCE**: Ori's Salsa `tokens()` query in `compiler/oric/src/query/mod.rs`; Gleam's `ModuleExtra` for formatter metadata.

---

## 09.1 Salsa Query Integration

- [x] Position-independent `TokenList` Hash/Eq (Commit 1)
  - Hash only `TokenKind` + `TokenFlags`, skip `Span` positions
  - Enables early cutoff when whitespace-only edits shift positions
- [x] `LexOutput` derives Salsa-required traits: `Clone, Eq, PartialEq, Hash, Debug` (Commit 2)
- [x] `tokens()` Salsa query works with V2 lexer (already implemented)
- [x] `tokens_with_metadata()` Salsa query for formatter/IDE path (Commit 8)
- [x] Early cutoff verified: extra-spaces edit triggers `tokens()` re-execution but `parsed()` is skipped (Commit 8 test)
- [x] Comment-only edit verified: `tokens()` cuts off, `tokens_with_metadata()` recomputes (Commit 8 test)

---

## 09.2 LexOutput Phase Output

- [x] `LexOutput` is the phase output type with `tokens`, `comments`, `blank_lines`, `newlines`, `errors`, `warnings` fields
- [x] `LexOutput` has manual Salsa trait impls (Commit 2)
- [x] `LexOutput` is immutable after creation
- [x] Errors are accumulated, not fatal
- [x] `into_metadata()` and `into_parts()` provide formatter/parser integration

---

## 09.3 TriviaCollector

**Decision**: Kept inline trivia collection (no separate `TriviaCollector` struct).

- [x] Inline trivia collection in `lex_with_comments()` works correctly
- [x] Comments reset blank line counter (comments are content)
- [x] Newline positions tracked accurately
- [x] `ModuleExtra` produced correctly for formatter

---

## 09.4 Doc Comment Classification

- [x] `DocMember` variant added to `CommentKind` (Commit 3), replacing `DocParam`/`DocField` (Commit 7)
- [x] `* name:` syntax classified as `DocMember` (Commit 4)
- [x] Legacy `@param`/`@field` markers emit `DocMember` (Commit 7)
- [x] `!` marker classified as `DocWarning` (pre-existing)
- [x] `>` marker classified as `DocExample` (pre-existing)
- [x] Regular comments classified as `Regular` (pre-existing)
- [x] Formatter updated to handle `DocMember` in comment reordering (Commit 5)
- [x] `extract_member_name_any()` handles both `* name:` and legacy `@param`/`@field` formats (Commit 7)
- [x] All test files migrated to new syntax (Commit 6)

### Not Yet Implemented (Deferred)

- [ ] Detached doc comment warnings (Gleam pattern) — deferred to future work
- [ ] `DocDescription` for unmarked comments before declarations — deferred to future work

---

## 09.5 Tests

- [x] `position_independent_hash` / `different_kinds_not_equal` tests (Commit 1)
- [x] `lex_output_salsa_traits` test (Commit 2)
- [x] `test_doc_member_sort_order` / `test_doc_member_is_doc` tests (Commit 3)
- [x] Doc `* name:` classification tests (Commit 4)
- [x] `test_extract_member_name` / `test_reorder_member_param_comments` / `test_reorder_member_field_comments` (Commit 5)
- [x] All 10 .ori fixtures + 3 Rust test files migrated (Commit 6)
- [x] Legacy @param/@field → DocMember tests (Commit 7)
- [x] `test_tokens_with_metadata_returns_comments` (Commit 8)
- [x] `test_tokens_with_metadata_comment_only_edit` (Commit 8)
- [x] `test_tokens_early_cutoff_on_whitespace_edit` (Commit 8)
- [x] `./test-all.sh` passes: 8769 tests, 0 failures

---

## 09.6 Completion Checklist

### Prerequisites
- [x] Doc comment marker alignment — RESOLVED by approved proposal and implementation
- [x] Storage pattern decision — kept hybrid AoS+tags with position-independent Hash/Eq

### Core Implementation
- [x] `LexOutput` phase output with Salsa traits
- [x] Position-independent `TokenList` Hash/Eq
- [x] `tokens_with_metadata()` Salsa query
- [x] Early cutoff verified

### Doc Comments
- [x] `DocMember` replaces `DocParam`/`DocField`
- [x] `* name:` syntax recognized
- [x] Legacy `@param`/`@field` emit `DocMember`
- [x] Formatter handles unified `DocMember`

### Integration & Testing
- [x] Formatter works with updated metadata
- [x] `./test-all.sh` passes (8769 tests)

**Exit Criteria:** Met. The V2 lexer integrates with Salsa queries via `LexOutput`. Early cutoff works correctly with position-independent hashing. Doc comments are classified per approved proposal (`*`, `!`, `>`). Full test suite passes.
