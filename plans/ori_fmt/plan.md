# How to Use This Plan

This document explains how to work with the Ori formatter roadmap.

## Execution Rules

### 1. Phase Ordering

Work through phases in numerical order within each tier. Some parallelization is possible:

- **Tier 1** (Phases 1-2): Sequential—Phase 2 depends on Phase 1
- **Tier 2** (Phases 3-4): Sequential—patterns build on expressions
- **Tier 3** (Phases 5-6): Can parallelize—collections and comments are independent
- **Tier 4** (Phases 7-8): Can parallelize—integration and polish are independent

### 2. Task Completion

For each task:
1. Implement the feature
2. Write Rust unit tests
3. Write Ori formatting tests (input → expected output)
4. Verify round-trip (format twice = format once)
5. Check the task checkbox

### 3. Test Requirements

Every formatting feature requires:
- **Unit tests**: In `ori_fmt/src/*/tests.rs`
- **Golden tests**: Input/output pairs in `tests/fmt/`
- **Round-trip**: `format(format(code)) == format(code)`

### 4. Design Document References

Each phase references the authoritative design documents:
- `docs/tooling/formatter/design/` contains all specifications
- If implementation differs from design, update the design doc first
- Design decisions should be documented in proposals if significant

## Task Format

```markdown
- [ ] **Implement**: [Feature description]
  - [ ] **Rust Tests**: `ori_fmt/src/[module]/tests.rs`
  - [ ] **Golden Tests**: `tests/fmt/[category]/[file].ori`
```

## Status Indicators

- `✅ Complete` — All tasks done, all tests pass
- `🔶 Partial` — Some features done, others pending
- `⏳ Not started` — No work begun

## File Organization

```
plans/ori_fmt/
├── 00-overview.md          # This overview
├── plan.md                 # How to use (this file)
├── priority-and-tracking.md # Current status
├── phase-01-core-algorithm.md
├── phase-02-declarations.md
├── phase-03-expressions.md
├── phase-04-patterns.md
├── phase-05-collections.md
├── phase-06-comments.md
├── phase-07-tooling.md
└── phase-08-polish.md
```

## Test Organization

```
tests/fmt/
├── declarations/
│   ├── functions/
│   │   ├── simple.ori
│   │   ├── multiline_params.ori
│   │   ├── generics.ori
│   │   ├── capabilities.ori
│   │   ├── where_clauses.ori
│   │   └── visibility.ori
│   ├── types/
│   │   ├── struct_inline.ori
│   │   ├── struct_multiline.ori
│   │   ├── sum_inline.ori
│   │   ├── sum_multiline.ori
│   │   ├── alias.ori
│   │   ├── generic.ori
│   │   └── derives.ori
│   ├── traits/
│   │   ├── simple.ori
│   │   ├── multi_method.ori
│   │   ├── defaults.ori
│   │   ├── associated.ori
│   │   └── inheritance.ori
│   ├── impls/
│   │   ├── inherent.ori
│   │   ├── trait.ori
│   │   └── generic.ori
│   ├── imports/
│   │   ├── simple.ori
│   │   ├── relative.ori
│   │   ├── alias.ori
│   │   ├── private.ori
│   │   ├── grouped.ori
│   │   └── reexport.ori
│   ├── tests/
│   │   ├── targeted.ori
│   │   ├── free_floating.ori
│   │   ├── multi_target.ori
│   │   └── attributes.ori
│   └── constants/
│       ├── simple.ori
│       └── public.ori
├── expressions/           # Phase 3
│   ├── calls.ori
│   ├── chains.ori
│   ├── conditionals.ori
│   └── lambdas.ori
├── patterns/              # Phase 4
│   ├── run.ori
│   ├── try.ori
│   ├── match.ori
│   └── parallel.ori
├── collections/           # Phase 5
│   ├── lists.ori
│   ├── maps.ori
│   └── structs.ori
├── comments/              # Phase 6
│   ├── regular.ori
│   └── doc.ori
└── edge-cases/            # Phase 8
    ├── nested.ori
    └── complex.ori
```

## Verification Commands

```bash
# Run formatter tests
cargo test -p ori_fmt

# Run golden tests
cargo st tests/fmt/

# Check all formatting
./fmt-all
```

## Key Principles

1. **No configuration**: Single canonical style
2. **Width-based breaking**: 100 char limit, not parameter counts
3. **Semantic preservation**: Only whitespace changes
4. **Idempotent**: Formatting twice equals formatting once
5. **Independent breaking**: Nested constructs break based on own width
