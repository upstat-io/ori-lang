# Proposal: Grammar Synchronization Formalization

**Status:** Draft
**Author:** Eric
**Created:** 2026-01-30

---

## Summary

Formalize the relationship between the grammar specification (`grammar.ebnf`), the Rust compiler implementation, and Ori language tests. Establish `grammar.ebnf` as the single source of truth for syntax, with automated tooling to detect and report discrepancies.

---

## Motivation

### The Problem

The Ori compiler has multiple sources that define syntax:

| Source | Role | Current State |
|--------|------|---------------|
| `grammar.ebnf` | Formal grammar specification | Authoritative, but not verified against implementation |
| Rust parser | Actual parsing implementation | May drift from grammar |
| Ori tests | Conformance validation | Coverage unknown |

These sources can drift apart, leading to subtle bugs. For example:

**Current discrepancy found:** The `div` (floor division) operator is:
- ✅ Documented in `grammar.ebnf` line 319: `mul_expr = unary_expr { ( "*" | "/" | "%" | "div" ) unary_expr } .`
- ✅ Tokenized by the lexer (`TokenKind::Div`)
- ✅ Handled by the type checker (`BinaryOp::FloorDiv`)
- ✅ Evaluated by the evaluator
- ❌ **Missing from the parser's `match_multiplicative_op()` function**

This means `10 div 3` would fail to parse despite being valid according to the grammar.

### Why This Matters

1. **Grammar defines the language** — If grammar.ebnf says `div` is valid syntax, it must parse
2. **Tests validate conformance** — Every grammar production should have test coverage
3. **Single source of truth** — Developers should trust grammar.ebnf without checking Rust code

### Goals

1. **Formalize operator precedence** — Canonical table derived from grammar.ebnf
2. **Verify parser implementation** — Automated checks that parser matches grammar
3. **Ensure test coverage** — Every operator and precedence level has tests
4. **Provide tooling** — A `sync-grammar` command that checks all three sources

---

## Design

### Operator Precedence Table (Canonical)

Derived from `grammar.ebnf` lines 303-319, precedence from **lowest to highest**:

| Level | Operators | Grammar Production | Associativity |
|-------|-----------|-------------------|---------------|
| 1 | `??` | `coalesce_expr` | Left |
| 2 | `\|\|` | `or_expr` | Left |
| 3 | `&&` | `and_expr` | Left |
| 4 | `\|` | `bit_or_expr` | Left |
| 5 | `^` | `bit_xor_expr` | Left |
| 6 | `&` | `bit_and_expr` | Left |
| 7 | `==` `!=` | `eq_expr` | Left |
| 8 | `<` `>` `<=` `>=` | `cmp_expr` | Left |
| 9 | `..` `..=` (with optional `by`) | `range_expr` | Non-assoc |
| 10 | `<<` `>>` | `shift_expr` | Left |
| 11 | `+` `-` | `add_expr` | Left |
| 12 | `*` `/` `%` `div` | `mul_expr` | Left |
| 13 | `!` `-` `~` (unary) | `unary_expr` | Right (prefix) |
| 14 | `.` `[]` `()` `?` `as` `as?` | `postfix_expr` | Left |

### Parser Implementation Requirements

The Rust parser must implement each precedence level. Key locations:

| Grammar Production | Rust Location | Function |
|-------------------|---------------|----------|
| `mul_expr` | `ori_parse/src/grammar/expr/operators.rs` | `match_multiplicative_op()` |
| `add_expr` | Same file | `match_additive_op()` |
| `shift_expr` | Same file | `match_shift_op()` |
| `cmp_expr` | Same file | `match_comparison_op()` |
| `eq_expr` | Same file | `match_equality_op()` |
| `bit_and_expr` | Same file | `match_bitwise_and_op()` |
| `bit_xor_expr` | Same file | `match_bitwise_xor_op()` |
| `bit_or_expr` | Same file | `match_bitwise_or_op()` |
| `and_expr` | Same file | `match_logical_and_op()` |
| `or_expr` | Same file | `match_logical_or_op()` |
| `coalesce_expr` | Same file | `match_coalesce_op()` |

### Test Requirements

Each operator requires tests validating:

1. **Basic parsing** — The operator parses correctly
2. **Precedence** — Correct precedence relative to adjacent levels
3. **Associativity** — Left-to-right grouping for binary operators
4. **Edge cases** — Interaction with parentheses, unary operators

Example test structure:

```
tests/spec/operators/
├── precedence/
│   ├── mul_over_add.ori          # * binds tighter than +
│   ├── add_over_shift.ori        # + binds tighter than <<
│   ├── shift_over_range.ori      # << binds tighter than ..
│   └── ...
├── associativity/
│   ├── mul_left_assoc.ori        # a * b * c = (a * b) * c
│   └── ...
└── operators/
    ├── div_floor.ori             # 10 div 3 = 3
    └── ...
```

---

## Enhanced `sync-grammar` Skill

The existing `sync-grammar` skill syncs grammar.ebnf with spec files. We enhance it to also verify the Rust parser implementation.

### Checks to Perform

#### 1. Operator Coverage Check

For each operator in grammar.ebnf:
- Verify corresponding `TokenKind` exists in lexer
- Verify corresponding `BinaryOp`/`UnaryOp` variant exists
- Verify parser's match function includes the operator
- Verify type checker handles the operator
- Verify evaluator implements the operator

#### 2. Precedence Chain Verification

For the grammar's precedence chain (`binary_expr` → `coalesce_expr` → ... → `mul_expr` → `unary_expr`):
- Verify parser builds expressions with correct precedence
- Verify no precedence level is skipped

#### 3. Test Coverage Report

For each operator and precedence relationship:
- Report whether tests exist
- Identify gaps in coverage

### Output Format

```
Grammar Sync Report
==================

Operators:
  [✓] * (mul)     - lexer: ✓, parser: ✓, typeck: ✓, eval: ✓
  [✓] / (div)     - lexer: ✓, parser: ✓, typeck: ✓, eval: ✓
  [✓] % (mod)     - lexer: ✓, parser: ✓, typeck: ✓, eval: ✓
  [✗] div (floor) - lexer: ✓, parser: ✗, typeck: ✓, eval: ✓
      └─ Missing: match_multiplicative_op() in ori_parse/src/grammar/expr/operators.rs

Precedence:
  [✓] mul > add
  [✓] add > shift
  [✓] shift > range
  ...

Test Coverage:
  [✓] tests/spec/operators/div_floor.ori
  [✗] Missing: precedence test for div vs mul
```

---

## Implementation Plan

### Phase 1: Fix Known Issues

1. Add `div` case to `match_multiplicative_op()` in the parser
2. Add test for `div` operator parsing

### Phase 2: Create Test Infrastructure

1. Create `tests/spec/operators/` directory structure
2. Add precedence tests for each adjacent pair
3. Add associativity tests for each binary operator level

### Phase 3: Enhance `sync-grammar` Skill

1. Add parser verification logic
2. Add test coverage reporting
3. Document the sync process

---

## Affected Files

### Parser (to fix `div`)

```
compiler/ori_parse/src/grammar/expr/operators.rs
```

### Tests (to add)

```
tests/spec/operators/precedence/
tests/spec/operators/associativity/
tests/spec/operators/div_floor.ori
```

### Skill (to enhance)

```
.claude/commands/sync-grammar.md
```

---

## Design Rationale

### Why grammar.ebnf as Source of Truth?

1. **Human-readable** — Easier to review than Rust code
2. **Language-agnostic** — Could generate parsers for other implementations
3. **Formal** — EBNF is a standard notation
4. **Already exists** — We've invested in keeping it accurate

### Why Automated Verification?

Manual review is error-prone. The `div` discrepancy existed despite:
- The grammar being correct
- The lexer, type checker, and evaluator being correct
- Only the parser was missed

Automated checks catch these gaps.

### Why Test Coverage Tracking?

Grammar defines what's valid. Tests verify the implementation matches. Without coverage tracking, we don't know if our tests actually exercise the grammar.

---

## Alternatives Considered

### Generate Parser from Grammar

Instead of verifying the hand-written parser matches grammar, we could generate the parser from grammar.ebnf.

**Rejected because:**
- Significant implementation effort
- Hand-written parsers offer better error messages
- Verification is sufficient for catching drift

### Inline Precedence in Spec Files

Instead of extracting precedence from grammar.ebnf, embed it in spec files.

**Rejected because:**
- Duplicates information
- Grammar.ebnf already encodes precedence in production structure
- Single source of truth is better

---

## Summary

| Change | Purpose |
|--------|---------|
| Formalize precedence table | Canonical reference derived from grammar.ebnf |
| Fix `div` parser bug | Immediate discrepancy fix |
| Add operator tests | Ensure coverage |
| Enhance `sync-grammar` | Automated verification |

This proposal establishes grammar.ebnf as the authoritative syntax specification and provides tooling to ensure the implementation stays synchronized.
