# Priority and Tracking

Current status of the Ori formatter implementation.

## Overall Status

| Tier | Focus | Status |
|------|-------|--------|
| Tier 1 | Foundation | ✅ Complete |
| Tier 2 | Expressions | 🔶 In Progress |
| Tier 3 | Collections & Comments | ⏳ Not started |
| Tier 4 | Integration | ⏳ Not started |

## Phase Status

### Tier 1: Foundation

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 1 | Core Algorithm | ✅ Complete | Width calculator, formatter core, tab conversion, idempotency |
| 2 | Declarations | ✅ Complete | ModuleFormatter with all declaration types, golden tests passing |

### Tier 2: Expressions

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 3 | Expressions | 🔶 In Progress | Calls, chains, conditionals, lambdas, bindings, binary ops (9 golden test suites) |
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

### M1: Basic Formatting (Tier 1) — ✅ Complete

- [x] Width calculation engine
- [x] Two-pass rendering (formatter core)
- [x] Tab-to-space conversion
- [x] Idempotency tests
- [x] Function declarations (basic)
- [x] Type definitions (basic)
- [x] Import statements (basic)
- [x] Width-based breaking for signatures
- [x] Golden tests (7 test categories, 26 test files)

**Exit criteria**: Can format basic Ori programs with declarations

### M2: Expression Formatting (Tier 2) — 🔶 In Progress

- [x] Function calls (golden tests: calls/)
- [x] Method chains (golden tests: chains/)
- [x] Conditionals (golden tests: conditionals/)
- [x] Lambdas (golden tests: lambdas/)
- [x] Binary expressions (golden tests: binary/)
- [x] Bindings (golden tests: bindings/)
- [x] Field/index access (golden tests: access/)
- [ ] Pattern constructs (run, try, match) — Phase 4

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
| Width Calculation | 112 | 112 |
| Formatter Core | 55 | 55 |
| Emitter | 7 | 7 |
| Context | 9 | 9 |
| Tab Conversion | 10 | 10 |
| Declarations | 1 | 1 |
| Golden Tests (Declarations) | 7 | 7 |
| Golden Tests (Expressions) | 9 | 9 |
| Patterns | 0 | 0 |
| Collections | 0 | 0 |
| Comments | 0 | 0 |
| Edge Cases | 0 | 0 |
| **Total** | **204** | **204** |

## Recent Updates

### 2026-01-30: Expression Golden Tests (Phase 3 Started)

Added 9 expression golden test suites with 22 test files:

| Category | Files | Coverage |
|----------|-------|----------|
| calls | 4 | simple, multi_arg, nested, lambda_arg |
| chains | 2 | short, mixed |
| conditionals | 3 | simple, no_else, chained |
| lambdas | 3 | single, multi, no_param |
| binary | 4 | simple, logical, comparison, range |
| bindings | 4 | simple, immutable, destructure_tuple, destructure_struct |
| access | 2 | list, field |
| conversions | 1 | as (placeholder) |
| errors | 1 | propagate (placeholder) |

**Known Parser Limitations** (discovered during testing):
- `as` type conversion syntax not yet implemented in parser
- `div` floor division is a keyword, not usable as function name
- `by` range stepping not implemented
- `$` immutable modifier only valid at module level, not in run patterns
- Parentheses around expressions not preserved in AST (affects range precedence)

**Status**: Phase 3 golden tests passing. Ready to continue with Phase 4 (Patterns).

### 2026-01-29: Golden Tests Complete (Phase 2 Done)

Completed golden test infrastructure for formatter verification:

**Test Harness** (`ori_fmt/tests/golden_tests.rs`):
- Integration tests using ori_lexer and ori_parse as dev-dependencies
- Discovers and runs all `.ori` files in `tests/fmt/` directory
- Supports `.expected` files for non-idempotent transformations
- Comment stripping (comment preservation is Phase 6)
- Whitespace normalization for comparison
- Idempotency testing: format(format(x)) == format(x)

**Test Categories** (7 test suites, 26 test files):

| Category | Files | Coverage |
|----------|-------|----------|
| Functions | 6 | simple, multiline_params, generics, capabilities, where_clauses, visibility |
| Types | 7 | struct_inline, struct_multiline, sum_inline, sum_multiline, alias, generic, derives |
| Traits | 5 | simple, multi_method, defaults, associated, inheritance |
| Impls | 3 | inherent, trait, generic |
| Imports | 6 | simple, relative, alias, private, grouped, reexport |
| Tests | 4 | targeted, free_floating, multi_target, attributes |
| Constants | 2 | simple, public |

**Formatter Fixes**:
- Fixed `format_module()` for proper blank lines between items
- Fixed `format_config()` to output `$name` instead of `let $name`
- Fixed `format_test()` to not output `tests _` for free-floating tests

**Known Limitations** (documented in `.expected` files):
- Parser doesn't support multi-line params (formatter output can't be re-parsed)
- Derive attribute output not fully implemented
- Test attribute output (#skip, #compile_fail, #fail) not preserved

**Status**: Tier 1 (Foundation) complete. Ready for Tier 2 (Expressions).

### 2026-01-29: ModuleFormatter Implementation

Implemented `declarations.rs` (~950 lines) with `ModuleFormatter`:

**New exports**:
- `format_module(module, arena, interner)` → `String`
- `ModuleFormatter` struct for module-level formatting

**Supported declarations**:
- Functions with generics, params, return types, capabilities, where clauses
- Type definitions (structs, sum types, newtypes) with derives
- Trait definitions with methods and associated types
- Impl blocks (inherent and trait impls)
- Test declarations with skip/compile_fail/fail attributes
- Import statements (stdlib grouped first, then relative)
- Config/constant definitions

**Width-based breaking**: Params and struct fields break when exceeding line limit.

### 2026-01-29: Tab Conversion & Idempotency Tests

Completed Phase 1 remaining items:

**Tab-to-Space Conversion** (`lib.rs`):
- Added `tabs_to_spaces()` function for source preprocessing
- Converts tabs to spaces with proper column alignment (4-space tabs)
- 10 comprehensive tests covering edge cases

**Idempotency Tests** (`formatter/tests.rs`):
- Added 44 new formatter tests (idempotency + literal/operator/control flow formatting)
- AST-level idempotency verified: format(AST) produces consistent output
- Full parse-format-parse round-trip deferred to Phase 7 (requires parser integration)

**Status**: Phase 1 nearly complete. Blank line handling deferred to Phase 2 (requires top-level item support).

### 2026-01-29: Formatter Core Implementation

Implemented the two-pass rendering engine in `ori_fmt/src/`:

**New Modules**:

| Module | Lines | Purpose |
|--------|-------|---------|
| `emitter.rs` | ~180 | `Emitter` trait, `StringEmitter`, `FileEmitter` |
| `context.rs` | ~140 | `FormatContext` with column/indent tracking |
| `formatter/mod.rs` | ~1200 | `Formatter` struct with inline/broken/stacked rendering |
| `formatter/tests.rs` | ~65 | Formatter core tests |

**Key Features**:
- Width-based breaking: inline if ≤100 chars, break otherwise
- Always-stacked constructs: `run`, `try`, `match`, `FunctionSeq`
- Independent breaking: nested constructs break based on own width
- Trailing comma handling: required for multi-line, forbidden for single-line
- Indentation: 4 spaces per level

**Exports**:
- `Formatter<I>`: Main formatter struct
- `format_expr()`: Convenience function
- `FormatContext<E>`: Formatting state
- `Emitter` trait + `StringEmitter`, `FileEmitter`
- `MAX_LINE_WIDTH`, `INDENT_WIDTH` constants

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
