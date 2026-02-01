# Phase 8: Edge Cases & Polish

**Goal**: Handle comprehensive edge cases, optimize performance, and ensure production readiness.

> **DESIGN**: `docs/tooling/formatter/design/appendices/A-edge-cases.md`

## Phase Status: 🔶 Partial

## 8.1 Deeply Nested Constructs

- [x] **Implement**: Deeply nested function calls
  - [x] **Golden Tests**: `tests/fmt/edge-cases/nested/calls.ori`
- [x] **Implement**: Deeply nested conditionals (chained else-if)
  - [x] **Golden Tests**: `tests/fmt/edge-cases/nested/conditionals.ori`
- [x] **Implement**: Deeply nested match expressions
  - [x] **Golden Tests**: `tests/fmt/edge-cases/nested/match.ori`
- [x] **Implement**: Deeply nested collections
  - [x] **Golden Tests**: `tests/fmt/edge-cases/nested/collections.ori`
- [x] **Implement**: Mixed nesting (calls inside match inside run)
  - [x] **Golden Tests**: `tests/fmt/edge-cases/nested/mixed.ori`

## 8.2 Long Identifiers

- [x] **Implement**: Long function names
  - [x] **Golden Tests**: `tests/fmt/edge-cases/long/function_names.ori`
- [x] **Implement**: Long parameter names
  - [x] **Golden Tests**: `tests/fmt/edge-cases/long/param_names.ori`
- [x] **Implement**: Long type names
  - [x] **Golden Tests**: `tests/fmt/edge-cases/long/type_names.ori`
- [x] **Implement**: Long chains of field access
  - [x] **Golden Tests**: `tests/fmt/edge-cases/long/field_chains.ori`

## 8.3 Boundary Cases

- [x] **Implement**: Exactly 100 character lines (no break)
  - [x] **Golden Tests**: `tests/fmt/edge-cases/boundary/exact_100.ori`
- [x] **Implement**: 101 character lines (must break)
  - [x] **Golden Tests**: `tests/fmt/edge-cases/boundary/101_chars.ori`
  - [x] **Bug Fix**: Function params now break when signature + short body exceeds 100 chars
- [x] **Implement**: Very long string literals (>100 chars)
  - [x] **Golden Tests**: `tests/fmt/edge-cases/boundary/long_strings.ori`
- [x] **Implement**: Very long single tokens
  - [x] **Golden Tests**: `tests/fmt/edge-cases/boundary/long_tokens.ori`

## 8.4 Empty and Minimal Constructs

- [x] **Implement**: Empty file
  - [x] **Golden Tests**: `tests/fmt/edge-cases/empty/file.ori`
- [x] **Implement**: File with only comments
  - [x] **Golden Tests**: `tests/fmt/edge-cases/empty/only_comments.ori`
- [x] **Implement**: File with only imports
  - [x] **Golden Tests**: `tests/fmt/edge-cases/empty/only_imports.ori`
- [x] **Implement**: Empty function body
  - [x] **Golden Tests**: `tests/fmt/edge-cases/empty/function.ori`
- [x] **Implement**: Empty struct
  - [x] **Golden Tests**: `tests/fmt/edge-cases/empty/struct.ori`
- [x] **Implement**: Empty trait
  - [x] **Golden Tests**: `tests/fmt/edge-cases/empty/trait.ori`
- [x] **Implement**: Empty impl
  - [x] **Golden Tests**: `tests/fmt/edge-cases/empty/impl.ori`

## 8.5 Unicode Handling

- [ ] **Blocked**: Unicode in identifiers (requires lexer changes)
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/unicode/identifiers.ori`
  - Note: Lexer only supports ASCII identifiers `[a-zA-Z_][a-zA-Z0-9_]*`
- [x] **Implement**: Unicode in strings
  - [x] **Golden Tests**: `tests/fmt/edge-cases/unicode/strings.ori`
- [x] **Implement**: Emoji in strings
  - [x] **Golden Tests**: `tests/fmt/edge-cases/unicode/emoji.ori`
- [x] **Implement**: Width calculation for multi-byte characters
  - [x] **Rust Tests**: `width::helpers::tests::test_char_display_width_*`
  - [x] **Rust Tests**: `width::literals::tests::test_string_width_cjk`, `test_char_width_cjk`
- [x] **Implement**: RTL text in strings
  - [x] **Golden Tests**: `tests/fmt/edge-cases/unicode/rtl.ori`

## 8.6 Whitespace Edge Cases

- [x] **Implement**: Tabs in input (convert to spaces)
  - [x] **Golden Tests**: `tests/fmt/edge-cases/whitespace/tabs.ori`
- [x] **Implement**: Mixed tabs and spaces
  - [x] **Golden Tests**: `tests/fmt/edge-cases/whitespace/mixed.ori`
- [x] **Implement**: Trailing whitespace (remove)
  - [x] **Golden Tests**: `tests/fmt/edge-cases/whitespace/trailing.ori`
- [x] **Implement**: Multiple blank lines (collapse)
  - [x] **Golden Tests**: `tests/fmt/edge-cases/whitespace/blank_lines.ori`
- [x] **Implement**: No final newline (add one)
  - [x] **Golden Tests**: `tests/fmt/edge-cases/whitespace/no_newline.ori`
- [x] **Implement**: Multiple final newlines (collapse to one)
  - [x] **Golden Tests**: `tests/fmt/edge-cases/whitespace/multi_newline.ori`

## 8.7 Complex Real-World Examples

- [x] **Implement**: Full module with imports, types, functions, tests
  - [x] **Golden Tests**: `tests/fmt/edge-cases/real/full_module.ori`
- [x] **Implement**: HTTP client example
  - [x] **Golden Tests**: `tests/fmt/edge-cases/real/http_client.ori`
- [x] **Implement**: Data processing pipeline
  - [x] **Golden Tests**: `tests/fmt/edge-cases/real/pipeline.ori`
- [x] **Implement**: Concurrent task orchestration
  - [x] **Golden Tests**: `tests/fmt/edge-cases/real/concurrent.ori`
- [x] **Implement**: Large file for performance testing (5700+ lines)
  - [x] **Golden Tests**: `tests/fmt/edge-cases/real/large_file.ori`

## 8.8 Performance Testing

- [x] **Implement**: Large file benchmark (10,000+ lines)
  - [x] **Benchmark**: Format time under 1 second ✅ (2.75ms for 10k lines)
- [x] **Implement**: Many small files benchmark (1,000+ files)
  - [x] **Benchmark**: All files under 10 seconds ✅ (8.6ms sequential, 3.6ms parallel)
- [x] **Implement**: Memory usage benchmark
  - [x] **Benchmark**: Memory under 100MB for large files ✅ (minimal allocation)
- [ ] **Implement**: Incremental format benchmark
  - [ ] **Benchmark**: Small change formats under 100ms

**Benchmark Results** (Criterion, release mode):
- 10 functions: 12.5µs
- 100 functions: 66µs
- 1000 functions: 595µs
- 5,707 lines: 1.7ms
- 10,105 lines: 2.75ms
- 1000 files parallel: 3.6ms (2.4x faster than sequential)

**Comparison with rustfmt:**
- ori fmt core: ~0.27µs/line (10k lines in 2.75ms)
- rustfmt CLI: ~4µs/line (20k lines in 80ms)
- ori fmt core is ~15x faster per line than rustfmt CLI throughput

## 8.9 Idempotence Verification

- [x] **Implement**: Round-trip test suite
  - [x] **Tests**: `format(format(code)) == format(code)` for all .ori files
  - [x] **File**: `compiler/ori_fmt/tests/idempotence_tests.rs`
  - [x] **Coverage**: tests/spec/, tests/run-pass/, tests/fmt/, library/
- [x] **Implement**: Fuzz testing for idempotence
  - [x] **Tests**: Random valid Ori code remains idempotent
  - [x] **File**: `compiler/ori_fmt/tests/property_tests.rs`
  - [x] **Coverage**: 47 property-based tests using proptest
- [x] **Implement**: Property-based testing
  - [x] **Tests**: Generated expressions, functions, types, traits, impls
  - [x] **Coverage**: Literals, binary ops, method chains, field access, lambdas, run/match/for patterns, generics, where clauses, capabilities, unicode strings, nested constructs

**Bugs Fixed During Idempotence Work:**
- [x] **Fix**: Attribute syntax now matches grammar.ebnf (`#attr(...)` not `#[attr(...)]`)
  - Parser accepts both syntaxes for backwards compatibility
  - Formatter outputs spec-correct syntax
- [x] **Fix**: Single-element tuples now preserve trailing comma (`(x,)` not `(x)`)
  - Applies to tuple expressions, binding patterns, and match patterns
- [x] **Fix**: Method receivers now wrap in parentheses when needed
  - Binary ops, unary ops, ranges, lambdas, conditionals get parens as receivers
  - Example: `(0..10).iter()` not `0..10.iter()`

## 8.10 Error Messages

- [x] **Implement**: Clear error for unparseable input
  - [x] **Tests**: Error message includes location (`test_format_parse_error_contains_location`)
  - [x] **Feature**: Shows file path, line:column, source snippet, underline
- [x] **Implement**: Suggestions for common mistakes
  - [x] **Tests**: Helpful suggestions shown (`test_get_suggestion_*`)
  - [x] **Coverage**: E0001 (string), E0004 (char), E0005 (escape), E1001-E1007, E1011
- [x] **Implement**: Informative messaging for syntax errors
  - [x] **Feature**: Summary note explaining that syntax must be fixed before formatting
  - [x] **Note**: Partial formatting not implemented (same as gofmt, rustfmt)

## 8.11 Documentation

- [x] **Write**: User guide for `ori fmt`
  - [x] **File**: `docs/tooling/formatter/user-guide.md`
  - [x] **Coverage**: CLI usage, options, examples, CI integration, ignore files
- [x] **Write**: Integration guide for editors
  - [x] **File**: `docs/tooling/formatter/integration.md`
  - [x] **Coverage**: VS Code, Neovim, Emacs, Helix, Sublime, JetBrains, CI/CD, pre-commit
- [x] **Write**: Troubleshooting guide
  - [x] **File**: `docs/tooling/formatter/troubleshooting.md`
  - [x] **Coverage**: Parse errors, unexpected changes, CI failures, platform issues
- [x] **Write**: Style guide (what the formatter enforces)
  - [x] **File**: `docs/tooling/formatter/style-guide.md`
  - [x] **Coverage**: All spacing rules, breaking rules, always-stacked constructs

## Completion Checklist

- [x] All edge case tests pass
- [x] Performance benchmarks met
- [x] Idempotence verified
- [x] Error messages are helpful
- [x] Documentation complete
- [ ] Production deployment checklist verified
