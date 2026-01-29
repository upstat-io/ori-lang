# Phase 8: Edge Cases & Polish

**Goal**: Handle comprehensive edge cases, optimize performance, and ensure production readiness.

> **DESIGN**: `docs/tooling/formatter/design/appendices/A-edge-cases.md`

## Phase Status: ⏳ Not Started

## 8.1 Deeply Nested Constructs

- [ ] **Implement**: Deeply nested function calls
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/nested/calls.ori`
  ```ori
  outer(
      arg: middle(
          arg: inner(
              arg: value,
          ),
      ),
  )
  ```
- [ ] **Implement**: Deeply nested conditionals
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/nested/conditionals.ori`
- [ ] **Implement**: Deeply nested match expressions
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/nested/match.ori`
- [ ] **Implement**: Deeply nested collections
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/nested/collections.ori`
- [ ] **Implement**: Mixed nesting (calls inside match inside run)
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/nested/mixed.ori`

## 8.2 Long Identifiers

- [ ] **Implement**: Long function names
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/long/function_names.ori`
- [ ] **Implement**: Long parameter names
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/long/param_names.ori`
- [ ] **Implement**: Long type names
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/long/type_names.ori`
- [ ] **Implement**: Long chains of field access
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/long/field_chains.ori`

## 8.3 Boundary Cases

- [ ] **Implement**: Exactly 100 character lines (no break)
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/boundary/exact_100.ori`
- [ ] **Implement**: 101 character lines (must break)
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/boundary/101_chars.ori`
- [ ] **Implement**: Very long string literals (>100 chars)
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/boundary/long_strings.ori`
- [ ] **Implement**: Very long single tokens
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/boundary/long_tokens.ori`

## 8.4 Empty and Minimal Constructs

- [ ] **Implement**: Empty file
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/empty/file.ori`
- [ ] **Implement**: File with only comments
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/empty/only_comments.ori`
- [ ] **Implement**: File with only imports
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/empty/only_imports.ori`
- [ ] **Implement**: Empty function body
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/empty/function.ori`
- [ ] **Implement**: Empty struct
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/empty/struct.ori`
- [ ] **Implement**: Empty trait
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/empty/trait.ori`
- [ ] **Implement**: Empty impl
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/empty/impl.ori`

## 8.5 Unicode Handling

- [ ] **Implement**: Unicode in identifiers
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/unicode/identifiers.ori`
- [ ] **Implement**: Unicode in strings
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/unicode/strings.ori`
- [ ] **Implement**: Emoji in strings
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/unicode/emoji.ori`
- [ ] **Implement**: Width calculation for multi-byte characters
  - [ ] **Rust Tests**: Correct width for CJK, emoji
- [ ] **Implement**: RTL text in strings
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/unicode/rtl.ori`

## 8.6 Whitespace Edge Cases

- [ ] **Implement**: Tabs in input (convert to spaces)
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/whitespace/tabs.ori`
- [ ] **Implement**: Mixed tabs and spaces
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/whitespace/mixed.ori`
- [ ] **Implement**: Trailing whitespace (remove)
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/whitespace/trailing.ori`
- [ ] **Implement**: Multiple blank lines (collapse)
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/whitespace/blank_lines.ori`
- [ ] **Implement**: No final newline (add one)
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/whitespace/no_newline.ori`
- [ ] **Implement**: Multiple final newlines (collapse to one)
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/whitespace/multi_newline.ori`

## 8.7 Complex Real-World Examples

- [ ] **Implement**: Full module with imports, types, functions, tests
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/real/full_module.ori`
- [ ] **Implement**: HTTP client example
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/real/http_client.ori`
- [ ] **Implement**: Data processing pipeline
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/real/pipeline.ori`
- [ ] **Implement**: Concurrent task orchestration
  - [ ] **Golden Tests**: `tests/fmt/edge-cases/real/concurrent.ori`

## 8.8 Performance Testing

- [ ] **Implement**: Large file benchmark (10,000+ lines)
  - [ ] **Benchmark**: Format time under 1 second
- [ ] **Implement**: Many small files benchmark (1,000+ files)
  - [ ] **Benchmark**: All files under 10 seconds
- [ ] **Implement**: Memory usage benchmark
  - [ ] **Benchmark**: Memory under 100MB for large files
- [ ] **Implement**: Incremental format benchmark
  - [ ] **Benchmark**: Small change formats under 100ms

## 8.9 Idempotence Verification

- [ ] **Implement**: Round-trip test suite
  - [ ] **Tests**: `format(format(code)) == format(code)` for all examples
- [ ] **Implement**: Fuzz testing for idempotence
  - [ ] **Tests**: Random valid Ori code remains idempotent
- [ ] **Implement**: Property-based testing
  - [ ] **Tests**: AST equivalence before and after formatting

## 8.10 Error Messages

- [ ] **Implement**: Clear error for unparseable input
  - [ ] **Tests**: Error message includes location
- [ ] **Implement**: Suggestions for common mistakes
  - [ ] **Tests**: Helpful suggestions shown
- [ ] **Implement**: Partial format output (format what we can)
  - [ ] **Tests**: Valid portions formatted

## 8.11 Documentation

- [ ] **Write**: User guide for `ori fmt`
- [ ] **Write**: Integration guide for editors
- [ ] **Write**: Troubleshooting guide
- [ ] **Write**: Style guide (what the formatter enforces)

## Completion Checklist

- [ ] All edge case tests pass
- [ ] Performance benchmarks met
- [ ] Idempotence verified
- [ ] Error messages are helpful
- [ ] Documentation complete
- [ ] Production deployment checklist verified
