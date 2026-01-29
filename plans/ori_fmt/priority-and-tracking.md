# Priority and Tracking

Current status of the Ori formatter implementation.

## Overall Status

| Tier | Focus | Status |
|------|-------|--------|
| Tier 1 | Foundation | 🔶 Partial |
| Tier 2 | Expressions | ⏳ Not started |
| Tier 3 | Collections & Comments | ⏳ Not started |
| Tier 4 | Integration | ⏳ Not started |

## Phase Status

### Tier 1: Foundation

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 1 | Core Algorithm | 🔶 Partial | Width calculator complete, formatter pending |
| 2 | Declarations | ⏳ Not started | Functions, types, imports |

### Tier 2: Expressions

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 3 | Expressions | ⏳ Not started | Calls, chains, conditionals |
| 4 | Patterns | ⏳ Not started | run, try, match, parallel |

### Tier 3: Collections & Comments

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 5 | Collections | ⏳ Not started | Lists, maps, structs |
| 6 | Comments | ⏳ Not started | Comment handling, doc reordering |

### Tier 4: Integration

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 7 | Tooling | ⏳ Not started | CLI, LSP, WASM |
| 8 | Polish | ⏳ Not started | Edge cases, performance |

## Milestones

### M1: Basic Formatting (Tier 1) — 🔶 Partial

- [x] Width calculation engine
- [ ] Two-pass rendering
- [ ] Function declarations
- [ ] Type definitions
- [ ] Import statements

**Exit criteria**: Can format basic Ori programs with declarations

### M2: Expression Formatting (Tier 2) — ⏳ Not started

- [ ] Function calls
- [ ] Method chains
- [ ] Conditionals
- [ ] Pattern constructs (run, try, match)

**Exit criteria**: Can format programs with complex expressions

### M3: Full Language Support (Tier 3) — ⏳ Not started

- [ ] All collection types
- [ ] Comment preservation
- [ ] Doc comment reordering

**Exit criteria**: Can format any valid Ori program

### M4: Production Ready (Tier 4) — ⏳ Not started

- [ ] CLI integration (`ori fmt`)
- [ ] LSP format-on-save
- [ ] WASM for playground
- [ ] Performance optimization

**Exit criteria**: Ready for production use

## Dependencies on Compiler

The formatter depends on:
- **Parser**: AST with span information
- **Comment extraction**: Comments associated with AST nodes

Current parser status: ✅ Complete (spans included)

## Test Coverage

| Category | Tests | Passing |
|----------|-------|---------|
| Width Calculation | 49+ | 49+ |
| Declarations | 0 | 0 |
| Expressions | 0 | 0 |
| Patterns | 0 | 0 |
| Collections | 0 | 0 |
| Comments | 0 | 0 |
| Edge Cases | 0 | 0 |
| **Total** | **49+** | **49+** |

## Recent Updates

### 2026-01-29: Width Calculator Refactoring

**Plan**: `~/.claude/plans/breezy-watching-quasar.md`

Completed code review fixes for `ori_fmt/src/width/`:

**Critical Fixes**:
- Fixed `ExprKind::Error` returning 0 (now returns `ALWAYS_STACKED`)
- Fixed `Lt`/`Gt` operators returning width 2 (now returns 1)

**Module Split** (960 lines → 11 focused modules):

| Module | Lines | Purpose |
|--------|-------|---------|
| `mod.rs` | 439 | WidthCalculator + calculate_width |
| `helpers.rs` | 113 | `accumulate_widths`, `decimal_digit_count` |
| `literals.rs` | 197 | int/float/bool/string/char width |
| `compounds.rs` | 102 | duration/size width |
| `operators.rs` | 119 | binary/unary op width |
| `patterns.rs` | 225 | binding_pattern_width |
| `calls.rs` | 94 | call expression widths |
| `collections.rs` | 117 | list/tuple/map/struct/range widths |
| `control.rs` | 197 | control flow widths |
| `wrappers.rs` | 97 | Ok/Err/Some/Try/Await/Loop widths |
| `tests.rs` | 962 | All width tests |

**DRY Improvements**:
- Shared `accumulate_widths` helper for iteration patterns
- Shared `decimal_digit_count` for int/duration/size widths
- All clippy warnings resolved with `#[expect]` annotations
