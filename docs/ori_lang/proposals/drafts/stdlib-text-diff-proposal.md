# Proposal: std.text.diff — Structured Text Diffing

**Status:** Draft
**Created:** 2026-04-02
**Author:** Eric (with AI assistance)
**Affects:** Standard library, `library/std/text/diff/`
**Depends on:** stdlib-text-api-proposal.md (approved) — shares Unicode segmentation
**Prior art:** Python `difflib`, Rust `similar` crate, Myers diff algorithm, patience diff

---

## Summary

This proposal defines `std.text.diff` — a submodule of `std.text` for computing structured diffs between texts. Provides line-level, word-level, and character-level diffs using Myers and patience diff algorithms, plus edit script generation for patch-style output.

---

## Motivation

`std.text.similarity` (approved in `std.text`) provides **scalar** metrics: edit distance, Jaro-Winkler similarity, closest match. These answer "how similar?" but not "what changed?"

Structured diffing answers "what changed?" — producing edit scripts, unified diffs, and change hunks. This is needed for:

- **Test output comparison**: Show exactly where actual differs from expected
- **Configuration drift detection**: What changed in a config file?
- **Merge conflict visualization**: Which lines differ?
- **Semantic diff**: Word-level or token-level changes in prose
- **Version control**: `ori diff` for Ori source files

Python's `difflib` (in stdlib) is one of its most useful modules. No other language includes diffing in stdlib.

---

## Scope

### In Scope

- **Line-level diff**: Myers diff algorithm (standard, used by git)
- **Patience diff**: Better results for code with moved blocks
- **Word-level diff**: Show changed words within lines (semantic diff)
- **Character-level diff**: Grapheme-cluster-aware fine-grained diff
- **Edit script generation**: Insert, Delete, Replace, Equal operations
- **Unified diff format**: Standard patch-style output
- **Context diff format**: Show surrounding context

### Out of Scope

- Syntax-aware diff (needs a parser — domain-specific)
- Binary diff
- Three-way merge
- Applying patches (separate concern)

---

## API Sketch

```ori
use std.text.diff { diff_lines, unified_diff, DiffOp }

let old_text = "Hello\nWorld\nFoo"
let new_text = "Hello\nOri\nFoo"

let ops = diff_lines(old: old_text, new: new_text)
// → [Equal("Hello\n"), Delete("World\n"), Insert("Ori\n"), Equal("Foo")]

let patch = unified_diff(old: old_text, new: new_text, old_label: "a/file.ori", new_label: "b/file.ori")
// → "--- a/file.ori\n+++ b/file.ori\n@@ -1,3 +1,3 @@\n Hello\n-World\n+Ori\n Foo\n"
```

---

## Detailed Design

*To be expanded during full proposal development.*
