# Priority and Tracking

Current status of the Ori formatter implementation.

## Overall Status

| Tier | Focus | Status |
|------|-------|--------|
| Tier 1 | Foundation | ✅ Complete |
| Tier 2 | Expressions | ✅ Complete |
| Tier 3 | Collections & Comments | ✅ Complete |
| Tier 4 | Integration | 🔶 Partial |

## Section Status

### Tier 1: Foundation

| Section | Name | Status | Notes |
|-------|------|--------|-------|
| 1 | Core Algorithm | ✅ Complete | Width calculator, formatter core, tab conversion, idempotency |
| 2 | Declarations | ✅ Complete | ModuleFormatter with all declaration types, golden tests passing |

### Tier 2: Expressions

| Section | Name | Status | Notes |
|-------|------|--------|-------|
| 3 | Expressions | ✅ Complete | Calls, chains, conditionals, lambdas, bindings, binary ops (9 golden test suites) |
| 4 | Patterns | ✅ Complete | run, try, match, for (4 golden test suites, 15 test files). loop(...) not yet supported by parser |

### Tier 3: Collections & Comments

| Section | Name | Status | Notes |
|-------|------|--------|-------|
| 5 | Collections | ✅ Complete | Lists, maps, tuples, structs, ranges (5 golden test suites, 14 test files). Spread operators not yet in parser |
| 6 | Comments | ✅ Complete | Doc comment reordering, @param/@field ordering, edge cases (3 golden test suites, 13 test files). Comments inside bodies deferred |

### Tier 4: Integration

| Section | Name | Status | Notes |
|-------|------|--------|-------|
| 7 | Tooling | 🔶 Partial | CLI complete (ori fmt), LSP and WASM pending |
| 8 | Polish | ✅ Complete | Edge cases, idempotence, fuzz tests (171), error messages, documentation (4 guides) |

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

### M2: Expression Formatting (Tier 2) — ✅ Complete

- [x] Function calls (golden tests: calls/)
- [x] Method chains (golden tests: chains/)
- [x] Conditionals (golden tests: conditionals/)
- [x] Lambdas (golden tests: lambdas/)
- [x] Binary expressions (golden tests: binary/)
- [x] Bindings (golden tests: bindings/)
- [x] Field/index access (golden tests: access/)
- [x] Pattern constructs (golden tests: patterns/run, patterns/try, patterns/match, patterns/for)

**Exit criteria**: Can format programs with complex expressions

### M3: Full Language Support (Tier 3) — ✅ Complete

- [x] All collection types (lists, maps, tuples, structs, ranges)
- [x] Comment preservation (comments before declarations)
- [x] Doc comment reordering (Description → Param/Field → Warning → Example)
- [x] @param order matches function signature
- [x] @field order matches struct fields
- [x] Edge cases (empty comments, EOF comments, only-comment files, mixed)
- [ ] Comments inside function bodies (deferred - requires expression-level tracking)

**Exit criteria**: Can format any valid Ori program with declaration-level comments

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
| ori_fmt Unit Tests | 215 | 215 |
| ori_fmt Golden Tests | 35 | 35 |
| ori_fmt Idempotence Tests | 5 | 5 |
| ori_fmt Property Tests | 171 | 171 |
| ori_fmt Incremental Tests | 13 | 13 |
| ori_fmt Doc Tests | 1 | 1 |
| **Total** | **440** | **440** |

## Recent Updates

### 2026-01-30: WASM Playground Formatter Integration

Added direct formatter integration to the WASM playground module:

**Changes**:
- Added `ori_fmt` dependency to `playground/wasm/Cargo.toml`
- Added `format_ori()` WASM binding in `playground/wasm/src/lib.rs`
- Returns JSON: `{ success: bool, formatted: Option<String>, error: Option<String> }`
- Includes TODO comment noting this will switch to LSP-based formatting once `ori_lsp` is implemented

**API**:
```javascript
// Call from JavaScript
const result = JSON.parse(format_ori(source));
if (result.success) {
    editor.setValue(result.formatted);
} else {
    showError(result.error);
}
```

**Status**: Direct integration is temporary. Will migrate to LSP once `ori_lsp` is implemented.

### 2026-01-30: Section 7.5 Incremental Formatting Complete

Implemented incremental formatting API for formatting only changed declarations:

**New Module** (`compiler/ori_fmt/src/incremental.rs`):
- `format_incremental()` - Format only declarations overlapping with changed region
- `apply_regions()` - Apply formatted regions to source text
- `FormattedRegion` - A region of formatted text with its original position
- `IncrementalResult` - Enum with Regions, FullFormatNeeded, or NoChangeNeeded

**Features**:
- Detects which declarations overlap with a changed byte range
- Formats only affected declarations
- Preserves comments associated with declarations
- Falls back to full format for imports/configs (order-sensitive)
- Includes preceding comments in formatted region

**Tests** (`compiler/ori_fmt/tests/incremental_tests.rs`):
- 13 integration tests covering:
  - Empty modules
  - Changes in functions, types, traits, impls
  - Changes between declarations (whitespace)
  - Import/config changes (require full format)
  - Comment preservation
  - Multiple overlapping declarations

**Benchmarks** (`compiler/oric/benches/formatter.rs`):
- `bench_incremental_vs_full` - Compare incremental vs full for 10-1000 functions
- `bench_incremental_large_file` - 2000 function file comparison

**Benchmark Results**:
- 1000 functions: Full 622µs, Incremental 424µs (~32% faster)
- 2000 functions: Full 2.2ms, Incremental 1.74ms (~21% faster)
- Note: Full speedup requires incremental parsing (future work)

**API Design Decisions**:
- Minimum unit is a complete top-level declaration
- Imports and configs require full format (they're block-formatted)
- Re-parses entire file (incremental parsing would improve this)

### 2026-01-30: Section 8.11 Documentation Complete

Created comprehensive user-facing documentation for the formatter:

**New Documentation Files** (`docs/tooling/formatter/`):

1. **User Guide** (`user-guide.md`):
   - CLI command reference and options
   - Usage patterns (format all, CI, preview, stdin)
   - `.orifmtignore` file documentation
   - Exit codes and error handling

2. **Integration Guide** (`integration.md`):
   - VS Code (Run on Save, custom tasks)
   - Neovim (autocmds, conform.nvim)
   - Emacs (format-all, reformatter.el)
   - Helix (languages.toml)
   - Sublime Text (Format on Save, build system)
   - JetBrains IDEs (external tools, file watchers)
   - CI/CD (GitHub Actions, GitLab CI, pre-commit hooks)

3. **Troubleshooting Guide** (`troubleshooting.md`):
   - Parse error diagnosis and solutions
   - Unexpected formatting changes
   - CI failures and exit codes
   - Platform-specific issues (Windows line endings, PATH, permissions)
   - Error message reference table

4. **Style Guide** (`style-guide.md`):
   - Philosophy (zero-config, deterministic, width-driven)
   - Core rules (indentation, line width, trailing commas)
   - Spacing rules (all contexts)
   - Width-based breaking rules
   - Always-stacked constructs
   - Comment formatting

**Status**: Section 8 (Polish) complete. Tier 4 (Integration) remains partial (LSP, WASM pending).

### 2026-01-30: Section 8.9 Fuzz Testing Complete

Implemented comprehensive property-based fuzz testing for formatter idempotence:

**New Test Suite** (`compiler/ori_fmt/tests/property_tests.rs`):
- 171 property-based and unit tests using proptest crate
- Tests cover: expressions, functions, types, traits, impls, generics, capabilities
- Strategies for: literals, binary ops, method chains, field access, lambdas
- Pattern constructs: run, match, for expressions
- Edge cases: long expressions, unicode strings, nested constructs

**Bug Fixed During Fuzz Testing:**
- Parser didn't accept binary operators at line start (after newline)
- Fix: Added `skip_newlines()` in `parse_binary_level!` macro
- File: `compiler/ori_parse/src/grammar/expr/mod.rs`
- Enables formatted code like `"a"\n<= "b"` to re-parse correctly

**Property Test Categories (58 proptest-based tests):**
- Basic: expr, function, type, const, module idempotence (5 tests)
- Collections: list, tuple idempotence (2 tests)
- Nesting: nested expressions, conditionals, collections (3 tests)
- Extended: method chains, field access, lambdas, patterns (8 tests)
- Declarations: traits, impls, generics, where clauses, capabilities (6 tests)
- Edge cases: long expressions, unicode, comparison/logical chains (8 tests)
- Structure: large structs, sum types, many params/generics (5 tests)
- Literals: duration, size literals (2 tests)
- Imports: import statements (1 test)
- Types: complex types, function types, tuple types, generic structs (5 tests)
- Operators: bitwise chains, shift chains (2 tests)
- Modules: many functions, mixed declarations (2 tests)
- Public: public function declarations (1 test)
- Tests: test declarations (1 test)

**Unit Test Categories (113 deterministic tests):**
- Literals: zero, negative, large int, float, scientific, strings, chars, escapes (16 tests)
- Operators: bitwise &|^, shift <<>>, modulo, unary !, precedence, mixed (12 tests)
- Collections: nested lists/tuples, struct literals, ranges (10 tests)
- Control flow: if/else chains, nested if, for with filter, nested for (5 tests)
- Pattern constructs: run, try, match with patterns (10 tests)
- Function calls: simple, nested, with lambda, method chains, index access (9 tests)
- Lambdas: no-param, single-param, multi-param, typed, nested, complex body (6 tests)
- Type definitions: empty struct, single field, many fields, sum types, generics, derives (11 tests)
- Traits and impls: empty trait, single method, multiple methods, defaults, inheritance, inherent impl, trait impl, generic impl (8 tests)
- Function signatures: no params, single param, many params, void return, generics, bounds, where clauses, capabilities, public (14 tests)
- Imports: single, multiple, alias, relative (4 tests)
- Constants: int, float, string, bool, public (5 tests)
- Comments: single, doc, param, multiple (4 tests)
- Line width: long chains, long function names, long param names, long type annotations, very long expressions (5 tests)
- Complex: full module, complex expression composition, deeply nested everything (3 tests)

### 2026-01-30: Section 8.10 Error Messages Complete

Implemented rich error messages for formatter parse failures:

**New Features** (`compiler/oric/src/commands/fmt.rs`):
- Clear error display with file path, line:column location
- Source code snippet with line numbers and underlines
- Contextual suggestions for common mistakes (13 error patterns covered)
- ANSI color support for terminal output (auto-detected)
- Summary note explaining syntax must be fixed before formatting

**Error Patterns with Suggestions**:
- E0001: Unterminated string → suggests closing quote
- E0004: Unterminated char → suggests single quote syntax
- E0005: Invalid escape → lists valid escape sequences
- E1001: Unexpected token → context-specific (missing `=`, `,`, `)`, `}`, `]`, `:`)
- E1002: Expected expression → explains expression required
- E1003: Unclosed delimiter → suggests checking brackets
- E1004-E1007: Function definition errors → shows correct syntax
- E1011: Named arguments → explains named argument syntax

**Tests Added**: 13 new unit tests for error formatting

**Design Decision**: Partial formatting (format what we can) not implemented, matching gofmt/rustfmt behavior which require valid syntax.

### 2026-01-30: Section 8.9 Idempotence Testing Complete

Implemented comprehensive round-trip idempotence testing (`format(format(code)) == format(code)`):

**New Test Suite** (`compiler/ori_fmt/tests/idempotence_tests.rs`):
- Tests all .ori files in tests/spec/, tests/run-pass/, tests/fmt/, library/
- Comprehensive coverage with 952+ test files verified
- Uses comment-preserving formatting path

**Bugs Fixed During Testing**:

1. **Attribute Syntax** - Parser and formatter now use spec-correct `#attr(...)`:
   - Fixed `ori_parse/src/grammar/attr.rs` to accept both `#attr(...)` and `#[attr(...)]`
   - Fixed `ori_fmt/src/declarations.rs` to output `#attr(...)` per grammar.ebnf
   - Updated golden test expected files for attributes and derives

2. **Single-Element Tuples** - Now preserve trailing comma `(x,)`:
   - Fixed inline tuple formatting in `formatter/mod.rs`
   - Fixed binding pattern formatting in `emit_binding_pattern()`
   - Fixed match pattern formatting in `emit_match_pattern()`
   - Updated width calculations in `collections.rs` and `patterns.rs`

3. **Method Receiver Precedence** - Wrap in parens when needed:
   - Added `needs_receiver_parens()` helper for precedence checking
   - Binary ops, unary ops, ranges, lambdas, conditionals get parens as receivers
   - Example: `(0..10).iter()` not `0..10.iter()` which would misbind

**Remaining Section 8 Work**:
- Fuzz testing for idempotence
- Property-based testing (AST equivalence)
- Error messages (section 8.10)
- Documentation (section 8.11)

### 2026-01-30: Section 7.5 & 8.8 Performance Complete

Implemented performance benchmarks and parallel processing:

**Benchmark Infrastructure** (`compiler/oric/benches/formatter.rs`):
- Criterion-based benchmarks for formatter performance
- Scaling tests: 10/50/100/500/1000 functions
- Large file tests: 5.7k and 10k line files
- Parallel vs sequential comparison (1000 files)

**Performance Results:**
- 10k lines: 2.75ms (target was <1 second) ✅
- 1000 files: 3.6ms parallel, 8.6ms sequential (2.4x speedup)
- Comparison: ori fmt core is ~15x faster per line than rustfmt CLI

**Parallel Processing** (`compiler/oric/src/commands/fmt.rs`):
- Directory formatting now uses rayon for parallel file processing
- Atomic counters for thread-safe result aggregation
- Files collected first, then processed in parallel

**Remaining Performance Work:**
- Incremental formatting (only changed regions)

### 2026-01-30: Section 8.7 Real-World Examples Complete

Added 5 real-world example golden tests:

**New Test Files**:
- `edge-cases/real/full_module.ori` - Complete module with imports, types, functions, tests
- `edge-cases/real/http_client.ori` - HTTP client with capabilities (uses Http, Logger)
- `edge-cases/real/pipeline.ori` - Data processing pipeline with validation/transformation
- `edge-cases/real/concurrent.ori` - Concurrent task orchestration (parallel, spawn, timeout)
- `edge-cases/real/large_file.ori` - Performance testing file (5700+ lines)

**Parser Limitations Discovered**:
- Multi-line sum types with leading `|` cannot be re-parsed (formatter produces, parser rejects)
- `?` error propagation operator not in expression position (use `try` pattern instead)
- Sum types with 4+ variants break to multi-line format

**Remaining Section 8 Work**:
- Performance benchmarks (section 8.8)
- Idempotence verification (section 8.9)
- Error messages (section 8.10)
- Documentation (section 8.11)

### 2026-01-30: Section 8 Boundary Cases & Unicode Width

Completed remaining 8.3 (Boundary Cases) and 8.5 (Unicode Handling) tasks:

**New Test Files**:
- `edge-cases/boundary/long_strings.ori` - Very long string literals (>100 chars)
- `edge-cases/boundary/long_tokens.ori` - Very long identifiers and numbers
- `edge-cases/unicode/rtl.ori` - RTL text (Arabic, Hebrew) in strings

**Multi-byte Width Calculation**:
- Added `char_display_width()` helper in `width/helpers.rs`
- Handles CJK (width 2), emoji (width 2), combining marks (width 0)
- Updated `string_width()` and `char_width()` to use display width
- Added 14 new tests for character width calculation

**Blocked**:
- Unicode identifiers blocked by lexer (ASCII-only: `[a-zA-Z_][a-zA-Z0-9_]*`)

### 2026-01-30: Section 8 Edge Cases Continued

Added more edge case golden tests:

**New Test Files**:
- `edge-cases/nested/match.ori` - Deeply nested match expressions
- `edge-cases/long/function_names.ori` - Long function identifiers
- `edge-cases/long/param_names.ori` - Long parameter identifiers
- `edge-cases/long/type_names.ori` - Long type identifiers
- `edge-cases/long/field_chains.ori` - Long field access chains
- `edge-cases/empty/only_comments.ori` - File containing only comments
- `edge-cases/whitespace/mixed.ori` - Mixed tabs and spaces (with .expected)

**Test Harness Update**:
- Added `golden_tests_edge_cases_long` test entry for the new `long/` directory

**Remaining Section 8 Work**:
- Long strings/tokens (section 8.3)
- Unicode identifiers & RTL (section 8.5)
- Real-world examples (section 8.7)
- Performance benchmarks (section 8.8)
- Idempotence verification (section 8.9)
- Error messages (section 8.10)
- Documentation (section 8.11)

### 2026-01-30: Section 8 Edge Cases Started

Added 5 edge case golden test suites with 17 test files:

**New Test Categories**:
- `edge-cases/empty/` - 6 files: Empty file, imports only, empty function/struct/trait/impl
- `edge-cases/whitespace/` - 5 files: Tabs, trailing, blank lines, newlines
- `edge-cases/boundary/` - 2 files: Exact 100 chars, 101 chars (breaks params)
- `edge-cases/nested/` - 4 files: Nested calls, conditionals, collections, mixed
- `edge-cases/unicode/` - 2 files: Unicode strings, emoji

**Bug Fix**: Function param breaking now accounts for body width
- When function signature + short body (≤20 chars) exceeds 100 chars, params break first
- Prevents ugly mid-expression breaks like `x\n+ y`
- Long bodies (>20 chars) break naturally at semantic points (else, operators, etc.)
- Implementation: `calculate_function_trailing_width()` in declarations.rs

**Remaining Section 8 Work**:
- Long identifiers (section 8.2)
- Long strings/tokens (section 8.3)
- Multi-byte width calculation (section 8.5)
- Real-world examples (section 8.7)
- Performance benchmarks (section 8.8)
- Idempotence verification (section 8.9)

### 2026-01-30: Section 7 CLI Integration Complete

Implemented the `ori fmt` command with full CLI integration:

**New Features**:
- `ori fmt <file>` - Format a single file
- `ori fmt <directory>` - Format all .ori files recursively
- `ori fmt .` - Format current directory (default)
- `ori fmt --check` - Check mode (exit 1 if files would be formatted)
- `ori fmt --diff` - Show diff output instead of modifying files
- `ori fmt --help` - Show usage information

**Implementation**:
- `compiler/oric/src/commands/fmt.rs` - New command module (~300 lines)
- Uses `ori_lexer::lex_with_comments()` for comment-preserving lexing
- Uses `ori_parse::parse()` for parsing
- Uses `ori_fmt::format_module_with_comments()` for formatting
- Graceful error handling for parse errors, missing files, permission errors
- Recursive directory traversal with default ignores (hidden files, target/, node_modules/)

**CLI Help**:
```
ori fmt [options] [paths...]

Options:
  --check      Check if files are formatted (exit 1 if not)
  --diff       Show diff output instead of modifying files
  --stdin      Read from stdin, write to stdout
  --no-ignore  Ignore .orifmtignore files and format everything
  --help       Show this help message
```

**Remaining Work** (Section 7):
- LSP integration (textDocument/formatting)
- WASM compilation for playground
- CI integration documentation

### 2026-01-30: Section 7 CLI Completion

Completed the remaining CLI features for `ori fmt`:

**New Features**:
- `ori fmt --stdin` - Read from stdin, write to stdout (for piping)
- `.orifmtignore` file support - Exclude paths from formatting with glob patterns
- `ori fmt --no-ignore` - Format everything, ignoring .orifmtignore files

**Pattern Support in .orifmtignore**:
- `**/*.test.ori` - Match any path with .test.ori extension
- `*.tmp` - Match single directory level
- `generated/` - Exclude entire directory
- `# comments` - Lines starting with # are ignored

**Default Ignores** (unless --no-ignore):
- Hidden files and directories (starting with `.`)
- `target/` directory
- `node_modules/` directory

### 2026-01-30: Section 6 Comment Formatting Complete

Completed Section 6 with full doc comment reordering and edge case handling:

**New Features**:
- Doc comment reordering: Description → Param/Field → Warning → Example
- `@param` order now matches function signature (reordered automatically)
- `@field` order now matches struct field order (reordered automatically)
- Trailing comments at EOF preserved with blank line separator
- Mixed doc/regular comments handled (doc comments sorted first, regular preserved)

**New Golden Tests** (13 test files across 3 suites):
- `comments/doc/reorder.ori` - Out-of-order doc comment sorting
- `comments/doc/param_order.ori` - @param reordering to match signature
- `comments/doc/field_order.ori` - @field reordering to match struct
- `comments/edge/empty.ori` - Empty comment lines
- `comments/edge/eof.ori` - Comments at end of file
- `comments/edge/only_comments.ori` - File with only comments
- `comments/edge/mixed.ori` - Mixed doc and regular comments

**New API**:
- `CommentIndex::take_comments_before_function()` - @param reordering
- `CommentIndex::take_comments_before_type()` - @field reordering
- `emit_comments_before_function()` in ModuleFormatter
- `emit_comments_before_type()` in ModuleFormatter

**Deferred** (requires expression-level tracking):
- Comments inside function bodies
- Inline comment conversion (move to own line)

**Status**: Section 6 complete. Tier 3 complete. Ready for Tier 4 (Integration).

### 2026-01-30: Comment Preservation Infrastructure (Section 6 Started)

Added comment preservation infrastructure:

**ori_ir additions**:
- `Comment` type with span, content, and kind
- `CommentKind` enum: Regular, DocDescription, DocParam, DocField, DocWarning, DocExample
- `CommentList` wrapper with Salsa-compatible traits

**ori_lexer additions**:
- `lex_with_comments()` function returns `LexOutput { tokens, comments }`
- `classify_and_normalize_comment()` for doc comment detection
- Comment content normalization (space after `//`)

**ori_fmt additions**:
- `comments` module with `CommentIndex` for position-based lookup
- `format_module_with_comments()` that preserves comments
- Doc comment sort order support (Description → Param/Field → Warning → Example)
- Helper functions for `@param` and `@field` reordering

**Golden Tests**:
- `tests/fmt/comments/regular/` - Regular comment tests (2 files)
- `tests/fmt/comments/doc/` - Doc comment tests (3 files)

**Remaining Work** (Section 6 completion):
- Doc comment reordering when out of order
- `@param` order matching function signature
- `@field` order matching struct fields
- Comments inside function bodies
- Edge cases (empty comments, EOF comments)

### 2026-01-30: Collection Golden Tests (Section 5 Complete)

Added 5 collection golden test suites with 14 test files:

| Category | Files | Coverage |
|----------|-------|----------|
| lists | 5 | empty, short, simple_wrap, nested, nested_break |
| maps | 3 | empty, short, multi |
| tuples | 3 | unit, short, long |
| structs | 4 | empty, short, long, shorthand |
| ranges | 2 | exclusive, inclusive |

**Formatter Improvements**:
- Added tuple broken format (one item per line when exceeds width)
- Added complexity detection for list items (simple wrap vs complex one-per-line)
- Fixed empty struct formatting (no spaces: `Empty {}`)
- Fixed starting column for body formatting (expressions now respect line position)

**Parser Improvements**:
- Added multi-line support in lists: `skip_newlines()` after `[`, after commas, before `]`
- Added multi-line support in tuples: `skip_newlines()` after `(`, after commas, before `)`

**Compiler Dependencies** (Section 15C - not formatter limitations):
- Spread operators (`...`) - see `plans/roadmap/section-15C-literals-operators.md`
- Stepped ranges (`by`) - see `plans/roadmap/section-15C-literals-operators.md`

**Status**: Section 5 (Collections) complete. Ready for Section 6 (Comments).

### 2026-01-30: Pattern Golden Tests (Section 4 Complete)

Added 4 pattern golden test suites with 15 test files:

| Category | Files | Coverage |
|----------|-------|----------|
| run | 2 | simple, mutable (assignments) |
| try | 1 | simple (error propagation) |
| match | 9 | simple, variant, guards, or_pattern, struct, tuple, list, range, at_pattern |
| for | 2 | do (imperative), yield (collection) |

**Parser Limitations** (discovered during testing):
- `loop(...)` pattern not yet implemented in parser (spec defines it, parser doesn't support)
- `$` prefix for immutable bindings only valid in pattern names, not `let $x`

**Formatter Verification**:
- Mutable bindings: default `let x = ...`, no `let mut` syntax
- Immutable bindings: `let $x = ...` with `$` in pattern name
- All pattern constructs (run, try, match, for) format correctly

**Status**: Tier 2 (Expressions) complete. Section 3 and Section 4 done. Ready for Tier 3 (Collections & Comments).

### 2026-01-30: Expression Golden Tests (Section 3 Complete)

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

**Status**: Section 3 complete. Ready for Section 4 (Patterns).

### 2026-01-29: Golden Tests Complete (Section 2 Done)

Completed golden test infrastructure for formatter verification:

**Test Harness** (`ori_fmt/tests/golden_tests.rs`):
- Integration tests using ori_lexer and ori_parse as dev-dependencies
- Discovers and runs all `.ori` files in `tests/fmt/` directory
- Supports `.expected` files for non-idempotent transformations
- Comment stripping (comment preservation is Section 6)
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

Completed Section 1 remaining items:

**Tab-to-Space Conversion** (`lib.rs`):
- Added `tabs_to_spaces()` function for source preprocessing
- Converts tabs to spaces with proper column alignment (4-space tabs)
- 10 comprehensive tests covering edge cases

**Idempotency Tests** (`formatter/tests.rs`):
- Added 44 new formatter tests (idempotency + literal/operator/control flow formatting)
- AST-level idempotency verified: format(AST) produces consistent output
- Full parse-format-parse round-trip deferred to Section 7 (requires parser integration)

**Status**: Section 1 nearly complete. Blank line handling deferred to Section 2 (requires top-level item support).

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
