# Lexer Hygiene Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Disposable plan** — delete after all fixes land: `rm -rf plans/hygiene-lexer/`

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Bug Fix — Soft Keyword Cache
**File:** `section-01-soft-keyword-bug.md` | **Status:** Complete

```
IdentCache, soft keyword, cache contamination, cook_ident
cooker/mod.rs, cooker/identifier.rs, keywords/mod.rs
BUG-01-001, tokenization correctness, context-sensitive
```

---

### Section 02: Cooker Layer Algorithmic DRY
**File:** `section-02-cooker-dry.md` | **Status:** Not Started

```
algorithmic duplication, template cooking, unescape, numeric cooking
escape_cooking.rs, cook_escape/mod.rs, numeric.rs, duration_size.rs
cook_template_head, cook_template_middle, cook_template_tail, cook_template_complete
unescape_string_v2, unescape_template_v2, resolve_common_escape
cook_int, cook_hex_int, cook_bin_int, cook_duration, cook_size
detect_duration_suffix, detect_size_suffix, higher-order function
```

---

### Section 03: Scanner Layer Algorithmic DRY
**File:** `section-03-scanner-dry.md` | **Status:** Not Started

```
operator scanning, simple_or_compound, algorithmic duplication
raw_scanner/operators.rs, plus, star, percent, caret, at, bang
RawTag, RawToken, cursor, advance, compound operator
```

---

### Section 04: Drift, Gap & Polish
**File:** `section-04-drift-gap-polish.md` | **Status:** Not Started

```
catch-all, exhaustive match, cook(), RawTag dispatch
SOFT_KEYWORDS, could_be_soft_keyword, sync guard
span, make_span, duplicate helper, WASTE
```

---

### Section 05: Cleanup
**File:** `section-05-cleanup.md` | **Status:** Not Started

```
test-all, clippy-all, plan deletion, hygiene verification
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Bug Fix — Soft Keyword Cache | `section-01-soft-keyword-bug.md` |
| 02 | Cooker Layer Algorithmic DRY | `section-02-cooker-dry.md` |
| 03 | Scanner Layer Algorithmic DRY | `section-03-scanner-dry.md` |
| 04 | Drift, Gap & Polish | `section-04-drift-gap-polish.md` |
| 05 | Cleanup | `section-05-cleanup.md` |
