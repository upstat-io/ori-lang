---
paths:
  - "**/*.rs"
---

# Naming & Visibility

Extracted from `impl-hygiene.md` -- naming conventions, visibility rules, and documentation/comment standards.

## Naming

**Functions** -- verb-based prefixes:
- Predicates: `is_*`, `has_*`, `can_*`
- Conversions: `into_*` (consuming), `to_*` (borrowing), `as_*` (cheap ref), `from_*` (construct)
- Processing: `cook_*` (lexer), `parse_*` (parser), `check_*` (typeck), `eval_*` (evaluator), `emit_*` (codegen)
- Consumption: `eat_*` (advance past), `skip_*` (advance+discard)
- Resolution: `resolve_*`, `lookup_*`, `fresh_*`
- Factory: `new`, `with_*`

**Variables** -- scope-scaled:
- 1 char in <= 3 lines: `c`, `i`, `n`, `b`
- 2-4 chars in <= 15 lines: `ch`, `tok`, `pos`, `len`, `src`, `buf`, `err`, `kw`
- Descriptive in larger scopes: `token_span`, `base_offset`, `content_str`

**Constants**: `SCREAMING_SNAKE_CASE`, descriptive names.
**Type aliases**: `PascalCase`, suffix with purpose.
**Modules**: `snake_case`, noun-based.
**Crates**: `ori_` prefix.
**Generic parameters**: `T`/`E`/`K`/`V` for standard patterns; descriptive names when 2+ type params or domain-specific meaning. Never bare `T` with 3+ type params.

## Visibility

- Private by default; minimize pub surface
- `pub(crate)` for cross-module internal use
- `pub(super)` for parent-module access; prefer narrowest visibility that works
- No dead pub items; no dead code
- Items pub only for testing: `#[cfg(test)] pub` or `pub(crate)` with `// test-only` comment
- `#[non_exhaustive]` for public library APIs only. Internal compiler enums should be exhaustively matched -- the compiler error on new variants catches missing match arms.

## Comments

- `//!` module doc on every file; `///` on all `pub` items
- All pub types and functions get `///` docs; use `` [`TypeName`] `` for cross-references; no docs that just restate the function name
- Comment WHY, not WHAT; `debug_assert!` for preconditions
- **Anti-patterns**: `// increment counter` (restates code), `// TODO` without context, `// This is a hack` without explaining the proper fix
- No decorative banners (`// ===`, `// ---`, `// ***`)
- No comments restating code, no commented-out code ever (use version control), no bare `// TODO`
- TODOs: format `// TODO(phase): description` -- e.g., `// TODO(typeck): handle generic associated types`. Every TODO references a plan or roadmap item. No orphan TODOs.
- Section labels in large enums/matches: plain `// Section name`
- **Spec citations required**: code implementing grammar rules, operator semantics, type rules, or language semantics must cite the spec clause. Format: `// Spec: Clause N.M -- description`
- **Plan annotations are temporary scaffolding**: Annotations referencing plans (`TPR-04-005`, `CROSS-04-014`, `section-04-name`) are allowed during active plan execution -- they aid navigation. They MUST be removed when the plan completes. Stale annotations from completed plans are hygiene violations (**DRIFT** category). Run `.claude/skills/impl-hygiene-review/plan-annotations.sh` to scan. Only **spec references** (`Spec: Clause N.M`) are permanent.
