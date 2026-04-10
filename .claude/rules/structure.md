---
paths:
  - "**/*.rs"
---

# File & Module Organization

Extracted from `impl-hygiene.md` — these are the structural organization rules for Rust source files.

## File Organization

- **500-line limit**: source files (excluding tests); exceeding = **BLOAT** finding
- **Proactive split**: split at ~450 lines if you know more code is coming. Don't wait until over the limit.
- **Single responsibility per file**: one logical operation or one type family. Anti-pattern: `utils.rs`, `helpers.rs`, `misc.rs`. Every file name describes its domain.
- **Submodule extraction**: logical group exceeding ~200 lines -> sibling submodule; parent `mod.rs` = dispatch hub
- **Directory structure**: mirrors the logical phase/pass structure
- **Split when touching**: touching a file over 500 lines without splitting = finding
- **Tests in sibling `tests.rs`**: `#[cfg(test)] mod tests;` declaration only -- body in sibling file
- **Section markers**: plain `// Section name` on its own line, preceded by blank line. No decorative characters. If sections exceed ~200 lines, extract to submodule instead.
- **Banner removal**: if you touch a file with decorative banners (`// ===`, `// ---`), remove them.

### File Layout (top to bottom)

1. `//!` module docs
2. `mod` declarations
3. Imports (3 groups, blank-line separated: external -> crate -> relative, alphabetical within)
4. Type aliases
5. Type definitions (structs, enums)
6. Inherent `impl` blocks (immediately after their type)
7. Trait `impl` blocks (immediately after inherent impls)
8. Free functions
9. `#[cfg(test)] mod tests;` at bottom

### Module Roles

- `lib.rs` is an **index**: `//!` doc, `mod` declarations, `pub use` re-exports -- no function bodies. Strict, no exceptions.
- `mod.rs` **dispatches**: routes to submodules, holds shared private items
- Leaf files **implement**: actual logic lives here

### Crate Organization

- Each crate has a single documented purpose
- Module nesting max 4 levels (e.g., `ori_types::check::registration::traits`). Deeper = missing abstraction.
- If a crate has >50 source files, consider splitting
- Shared utilities live in dedicated crates (`ori_diagnostic`, `ori_patterns`, `ori_ir`). No `utils` modules in phase crates. If 3+ crates need the same utility, extract to a shared crate.

### Import Hygiene

- 3 groups separated by blank lines: external -> crate -> relative, alphabetical within
- No glob imports (`use foo::*`) except in test modules and preludes
- No unused imports
- Re-export only types that are part of your crate's public API contract. Consumers import from the crate that owns the type.

## Impl Block Method Ordering

1. **Constructors**: `new`, `with_*`, `from_*`, factory methods
2. **Accessors**: getters, `as_*` (cheap ref conversions)
3. **Predicates**: `is_*`, `has_*`, `can_*`, `contains`
4. **Public operations**: the main thing this type does
5. **Conversion/consumption**: `into_*`, `to_*`
6. **Private helpers**: in call-order grouping, not alphabetical

Within each group: pub before pub(crate) before private (loose).

## Struct/Enum Ordering

**Struct fields:**
1. Primary data (core state)
2. Secondary/derived data
3. Configuration/options
4. Flags/booleans last

Inline comments on struct fields when purpose isn't obvious.

**Enum variants:** ordered by frequency/importance (common first) or logically grouped (keywords together, operators together). Match arms follow the enum's declaration order.
