# Parser Improvements Index

> **Maintenance Notice:** Update this index when adding/modifying sections. This plan captures best practices from Rust, Go, TypeScript, Zig, Gleam, Elm, and Roc parsers.

Quick-reference keyword index for finding parser improvement sections.

---

## How to Use

1. **Search this file** (Ctrl+F / Cmd+F) for keywords
2. **Find the section ID** in the keyword cluster
3. **Open the section file**: `plans/ori_parse_improvements/section-{ID}-*.md`

---

## Keyword Clusters by Section

### Section 01: Quick Wins
**File:** `section-01-quick-wins.md` | **Phase:** 1 | **Status:** Not Started

```
token bitset, bitfield, fast set, recovery set, stopset
scratch buffer, temporary collection, reusable allocation
error variants, rich errors, contextual hints, error types
ParseErrorKind, expected tokens, diagnostic quality
Go parser, Zig scratch, Gleam errors, Roc errors
```

---

### Section 02: Medium-Term Improvements
**File:** `section-02-medium-term.md` | **Phase:** 2 | **Status:** Not Started

```
speculative parsing, snapshot, restore, backtracking
two-tier storage, inline small, overflow extra_data
series combinator, list parsing, comma-separated
Rust snapshots, TypeScript speculative, Zig node storage
```

---

### Section 03: Long-Term Architecture
**File:** `section-03-long-term.md` | **Phase:** 3 | **Status:** Not Started

```
incremental parsing, IDE support, reuse subtree
language server, LSP, syntax cursor, node reuse
SIMD preprocessing, aligned buffers, fast tokenization
TypeScript incremental, Roc SIMD, node stability
```

---

## Quick Reference

| ID | Title | Phase | Effort | Impact | File |
|----|-------|-------|--------|--------|------|
| 01 | Quick Wins | 1 | Low | High | `section-01-quick-wins.md` |
| 02 | Medium-Term Improvements | 2 | Medium | Medium | `section-02-medium-term.md` |
| 03 | Long-Term Architecture | 3 | High | High | `section-03-long-term.md` |

---

## Reference Parsers

| Language | Key Patterns | Reference Path |
|----------|--------------|----------------|
| **Rust** | Restrictions bitflags, snapshot/restore, token ungluing | `~/lang_repos/rust/compiler/rustc_parse/` |
| **Go** | Token bitsets, embedded scanner, precedence climbing | `~/lang_repos/golang/src/cmd/compile/internal/syntax/` |
| **TypeScript** | Incremental parsing, speculative parsing, context flags | `~/lang_repos/typescript/src/compiler/parser.ts` |
| **Zig** | Flat SoA AST, two-tier storage, scratch buffer | `~/lang_repos/zig/lib/std/zig/` |
| **Gleam** | Series combinator, rich error types, deferred collection | `~/lang_repos/gleam/compiler-core/src/parse/` |
| **Elm** | CPS combinators, indentation tracking, fail-fast | `~/lang_repos/elm/compiler/src/Parse/` |
| **Roc** | Progress tracking, arena allocation, SIMD prep | `~/lang_repos/roc/crates/compiler/parse/` |

---

## Maintenance Guidelines

When updating this plan:

1. **Adding items to a section**: Add relevant keywords to that section's cluster
2. **Creating a new section**: Add a new keyword cluster block
3. **Removing a section**: Remove the corresponding keyword cluster and table entry
4. **Marking complete**: Update status in both section file and this index
