---
paths:
  - "*.rs"
---

# Code Hygiene Rules

## File Organization (top to bottom)

1. `//!` module docs
2. `mod` declarations
3. Imports (see Import Rules)
4. Type aliases
5. Type definitions (structs, enums)
6. Inherent `impl` blocks (immediately after their type)
7. Trait `impl` blocks (immediately after inherent impls)
8. Free functions
9. `#[cfg(test)] mod tests;` at bottom (declaration only — body in sibling `tests.rs`)

**Allowed `#[cfg(test)]` in source**: helper fns needing private access, test-only imports, const assertions, `pub(crate) mod test_helpers;`

## Import Organization (3 groups, blank-line separated)

1. External crate imports (alphabetical)
2. Internal crate imports (`crate::`, grouped by module)
3. Relative imports (`super::`, local re-exports)

## Impl Block Method Ordering

1. **Constructors**: `new`, `with_*`, `from_*`, factory methods
2. **Accessors**: getters, `as_*` (cheap ref conversions)
3. **Predicates**: `is_*`, `has_*`, `can_*`, `contains`
4. **Public operations**: the main thing this type does
5. **Conversion/consumption**: `into_*`, `to_*`
6. **Private helpers**: in call-order grouping, not alphabetical

Within each group: pub before pub(crate) before private (loose).

## Naming

**Functions** — verb-based prefixes:
- Predicates: `is_*`, `has_*`, `can_*`
- Conversions: `into_*` (consuming), `to_*` (borrowing), `as_*` (cheap ref), `from_*` (construct)
- Processing: `cook_*` (lexer), `parse_*` (parser), `check_*` (typeck), `eval_*` (evaluator)
- Consumption: `eat_*` (advance past), `skip_*` (advance+discard)
- Factory: `new`, `with_*`

**Variables** — scope-scaled:
- 1 char in <= 3 lines: `c`, `i`, `n`, `b`
- 2-4 chars in <= 15 lines: `ch`, `tok`, `pos`, `len`, `src`, `buf`, `err`, `kw`
- Descriptive in larger scopes: `token_span`, `base_offset`, `content_str`

## Struct/Enum Field Ordering

1. Primary data (core state)
2. Secondary/derived data
3. Configuration/options
4. Flags/booleans last

Inline comments on struct fields when purpose isn't obvious.

## Comments

- `//!` module doc on every file; `///` on all `pub` items
- Comment WHY, not WHAT; `debug_assert!` for preconditions
- No decorative banners (`// ===`, `// ---`, `// ***`)
- No comments restating code, no commented-out code, no bare `// TODO`
- Section labels in large enums/matches: plain `// Section name`

## Derive vs Manual

- **Derive** when impl is standard (field-by-field equality, hash, debug)
- **Manual** only when behavior differs from derive
- If you can't articulate WHY manual differs, use derive

## Visibility

- Private by default; minimize pub surface
- `pub(crate)` for cross-module internal use
- No dead pub items; no dead code

## File Size

- **500 line limit** for source files (excluding `tests.rs`)
- Exceeding 500 lines: **split first**, don't add then plan to split
- Touching a file over 500 lines: take the opportunity to split
- Split by extracting logical groups into submodules
- Tests always in sibling `tests.rs` (use `scripts/extract_tests.py`)

## Style

- No `#[allow(clippy)]` without `reason = "..."` (use `#[expect]` when possible)
- Functions < 100 lines (target < 50; dispatch tables exempt)
- Consistent patterns across similar code within same file
- No dead/commented-out code
