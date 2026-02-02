# Ori Formatter v2: Layered Architecture

> **Goal:** Restructure the formatter into a clean 5-layer architecture combining patterns from TypeScript (declarative spacing), Gleam (container packing), Rust (shape tracking), and Ori-specific breaking rules.

## Source Documents

This plan consolidates:
- `scratchpad/formatter-architecture.md` — 5-layer architecture sketch
- `scratchpad/spec-to-architecture-mapping.md` — Spec rules mapped to layers
- `docs/ori_lang/0.1-alpha/spec/16-formatting.md` — Authoritative formatting spec

---

## Architecture Overview

```
Input: ExprArena + source positions
           │
           ▼
    ┌──────────────┐
    │ Layer 1:     │◄── Token spacing (O(1) lookup)
    │ SpaceRules   │    ~35 declarative rules
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │ Layer 2:     │◄── Container decisions
    │ Packing      │    FitOrOnePerLine, AlwaysStacked, etc.
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │ Layer 3:     │◄── Width tracking
    │ Shape        │    flows through recursion
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │ Layer 4:     │◄── Ori-specific rules
    │ Breaking     │    8 special-case rules
    │ Rules        │
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │ Layer 5:     │◄── Main formatter
    │ Orchestration│    try-inline → broken fallback
    └──────────────┘
           │
           ▼
      Output: String
```

---

## Design Decisions (Finalized)

| # | Rule | Decision |
|---|------|----------|
| 1 | **MethodChainRule** | Strict all-or-nothing. All chain elements break together. |
| 2 | **ShortBodyRule** | ~20 character threshold. Under 20 chars stays with yield/do. |
| 3 | **BooleanBreakRule** | 3+ `||` clauses OR exceeds width triggers breaking. |
| 4 | **ChainedElseIfRule** | **Kotlin style** — first `if` stays with assignment, else clauses indented. ⚠️ *Spec update needed* |
| 5 | **NestedForRule** | Rust-style. Each nested for increases indentation. |
| 6 | **ParenthesesRule** | Preserve all. Add when semantically needed, never remove user's parens. |
| 7 | **RunRule** | Top-level = stacked; nested = width-based. All statements on new lines. |
| 8 | **LoopRule** | Complex = contains run/try/match/for. Complex body breaks. |

### Spec Change Required

**ChainedElseIfRule** (Section 04) differs from current spec:

```ori
// Current spec (lines 432-436):
let size =
    if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"

// New decision (Kotlin style):
let size = if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"
```

---

## Rule Count by Layer

| Layer | Count | Description |
|-------|-------|-------------|
| Layer 1 | ~35 | Token spacing rules |
| Layer 2 | ~18 | Construct → Packing mappings |
| Layer 3 | 2 | Shape + config |
| Layer 4 | 8 | Ori-specific breaking rules |
| Layer 5 | 3 | Module-level orchestration |
| **Total** | **~66** | Discrete formatting decisions |

---

## Section Overview

### Tier 1: Core Layers (Sections 1-3)

Must be completed first. The foundation for all formatting decisions.

| Section | Focus | Dependency |
|---------|-------|------------|
| 1 | Token Spacing Rules | None |
| 2 | Container Packing | Section 1 |
| 3 | Shape Tracking | Section 2 |

### Tier 2: Breaking & Orchestration (Sections 4-5)

Ori-specific rules and the main formatter.

| Section | Focus | Dependency |
|---------|-------|------------|
| 4 | Breaking Rules | Section 3 |
| 5 | Formatter Orchestration | Section 4 |

### Tier 3: Validation & Integration (Sections 6-7)

Testing and final polish.

| Section | Focus | Dependency |
|---------|-------|------------|
| 6 | Testing & Validation | Section 5 |
| 7 | Integration & Polish | Section 6 |

---

## Dependency Graph

```
Section 1 (Token Spacing)
    │
    ▼
Section 2 (Packing)
    │
    ▼
Section 3 (Shape)
    │
    ▼
Section 4 (Breaking Rules)
    │
    ▼
Section 5 (Orchestration)
    │
    ▼
Section 6 (Testing)
    │
    ▼
Section 7 (Integration)
```

---

## Migration Strategy

The current formatter in `compiler/ori_fmt/` is functional. This refactor:

1. **Extracts** existing rules into declarative `SpaceRule` entries (Layer 1)
2. **Introduces** `Packing` enum and `determine_packing()` (Layer 2)
3. **Refactors** to use explicit `Shape` instead of ad-hoc width tracking (Layer 3)
4. **Consolidates** Ori-specific rules into documented `BreakingRules` (Layer 4)
5. **Restructures** main formatter to use try-inline-then-break pattern (Layer 5)

### Compatibility

- All existing golden tests must pass
- No user-visible formatting changes (except ChainedElseIfRule per spec update)
- Performance must not regress

---

## Target Example

When complete, this complex example should format correctly with all 8 rules:

```ori
@process_data (
    users: [User],
    config: Config,
) -> Result<[ProcessedUser], Error> uses Http, Logger = run(
    let active_users = for user in users yield if user.active && user.verified
        || user.is_admin
        || user.bypass_check then user
        else continue,
    let results = for user in active_users yield
        for permission in user.permissions yield
            run(
                let validated = (x -> x.validate())(user),
                let transformed = validated
                    .transform()
                    .normalize()
                    .sanitize(),
                let logged = run(print(msg: transformed.to_str()), transformed),
                logged,
            ),
    let final = (for r in results yield r)
        .flatten()
        .filter(x -> x.is_valid())
        .map(x -> x.finalize())
        .collect(),
    Ok(final),
)
```

### Rules Demonstrated

| Rule | Where Applied |
|------|---------------|
| **1. MethodChainRule** | `validated.transform().normalize().sanitize()` — all break |
| **2. ShortBodyRule** | `yield user` stays with yield (under 20 chars) |
| **3. BooleanBreakRule** | 3 `||` clauses break with leading `||` |
| **4. ChainedElseIfRule** | `if ... then user` / `else continue` — Kotlin style |
| **5. NestedForRule** | `for user ... yield` / `for permission ...` — Rust-style indent |
| **6. ParenthesesRule** | `(x -> x.validate())(user)` and `(for r in results yield r)` preserved |
| **7. RunRule** | Top-level `run(` stacked; nested `run(print(...), ...)` inline |
| **8. LoopRule** | (Would break if body contained run/try/match/for) |

---

## Success Criteria

A section is complete when:

1. **Implemented** — Code in `compiler/ori_fmt/`
2. **Tested** — Unit tests for each layer, integration tests for interactions
3. **Documented** — Code comments explaining the layer's responsibility

The plan is complete when:

1. All golden tests pass
2. Target example formats correctly
3. Performance benchmarks show no regression
4. Spec update for ChainedElseIfRule is applied

---

## Quick Reference

| Document | Purpose |
|----------|---------|
| `index.md` | Keyword index for finding sections |
| `00-overview.md` | This file — goals, architecture, dependencies |
| `section-XX-*.md` | Individual section details |

### External References

| Reference | Location |
|-----------|----------|
| Formatting Spec | `docs/ori_lang/0.1-alpha/spec/16-formatting.md` |
| Current Formatter | `compiler/ori_fmt/` |
| Golden Tests | `tests/fmt/` |
