---
section: "06"
title: Testing & Validation
status: not-started
goal: Comprehensive testing at each layer and integration level
sections:
  - id: "06.1"
    title: Layer 1 Tests
    status: not-started
  - id: "06.2"
    title: Layer 2 Tests
    status: not-started
  - id: "06.3"
    title: Layer 3 Tests
    status: not-started
  - id: "06.4"
    title: Layer 4 Tests
    status: not-started
  - id: "06.5"
    title: Golden Tests
    status: not-started
  - id: "06.6"
    title: Property Tests
    status: not-started
  - id: "06.7"
    title: Spec Compliance
    status: not-started
---

# Section 06: Testing & Validation

**Status:** 📋 Planned
**Goal:** Comprehensive testing ensuring each layer works correctly and integrates properly

---

## 06.1 Layer 1 Tests (Token Spacing)

- [ ] **Create** `ori_fmt/src/spacing/tests.rs`
- [ ] **Test** each spacing rule individually

```rust
#[test]
fn test_space_around_binary_ops() {
    assert_eq!(spacing_between(Plus, Ident), SpaceAction::Space);
    assert_eq!(spacing_between(Ident, Plus), SpaceAction::Space);
}

#[test]
fn test_no_space_inside_parens() {
    assert_eq!(spacing_between(LParen, Ident), SpaceAction::None);
    assert_eq!(spacing_between(Ident, RParen), SpaceAction::None);
}

#[test]
fn test_space_inside_struct_braces() {
    let ctx = FormattingContext::struct_literal();
    assert_eq!(spacing_between_with_ctx(LBrace, Ident, &ctx), SpaceAction::Space);
}
```

- [ ] **Test** context function correctness
- [ ] **Test** RulesMap O(1) lookup

---

## 06.2 Layer 2 Tests (Container Packing)

- [ ] **Create** `ori_fmt/src/packing/tests.rs`
- [ ] **Test** packing determination for each construct

```rust
#[test]
fn test_always_stacked_constructs() {
    assert_eq!(
        determine_packing(ConstructKind::RunTopLevel, false, false, false, 3),
        Packing::AlwaysStacked
    );
    assert_eq!(
        determine_packing(ConstructKind::Try, false, false, false, 2),
        Packing::AlwaysStacked
    );
}

#[test]
fn test_trailing_comma_forces_multiline() {
    assert_eq!(
        determine_packing(ConstructKind::FunctionArgs, true, false, false, 3),
        Packing::AlwaysOnePerLine
    );
}

#[test]
fn test_simple_list_can_pack() {
    assert_eq!(
        determine_packing(ConstructKind::ListSimple, false, false, false, 10),
        Packing::FitOrPackMultiple
    );
}
```

- [ ] **Test** simple vs complex item detection
- [ ] **Test** separator selection

---

## 06.3 Layer 3 Tests (Shape Tracking)

- [ ] **Create** `ori_fmt/src/shape/tests.rs`
- [ ] **Test** shape operations

```rust
#[test]
fn test_shape_consume() {
    let shape = Shape::new(100);
    let after = shape.consume(10);
    assert_eq!(after.width, 90);
    assert_eq!(after.offset, 10);
}

#[test]
fn test_shape_indent() {
    let shape = Shape::new(100);
    let indented = shape.indent(4);
    assert_eq!(indented.indent, 4);
    assert_eq!(indented.width, 96);
}

#[test]
fn test_shape_fits() {
    let shape = Shape::new(100).consume(90);
    assert!(shape.fits(10));
    assert!(!shape.fits(11));
}

#[test]
fn test_nested_shape_independence() {
    let config = FormatterConfig::default();
    let shape = Shape::new(100).indent(20).consume(30);
    let nested = shape.for_nested(&config);
    // Nested gets fresh width from indent, not from consumed position
    assert_eq!(nested.width, 80); // 100 - 20 indent
}
```

- [ ] **Test** edge cases (overflow, underflow)

---

## 06.4 Layer 4 Tests (Breaking Rules)

- [ ] **Create** `ori_fmt/src/rules/tests.rs`
- [ ] **Test** each breaking rule

### MethodChainRule Tests
```rust
#[test]
fn test_method_chain_fits_inline() {
    let code = "items.map(x -> x)";
    assert_formats_inline(code, "items.map(x -> x)");
}

#[test]
fn test_method_chain_breaks_all() {
    let code = "items.map(x -> transform(x)).filter(x -> x > 0).take(n: 10)";
    assert_formats_to(code, r#"
items
    .map(x -> transform(x))
    .filter(x -> x > 0)
    .take(n: 10)
"#);
}
```

### ShortBodyRule Tests
```rust
#[test]
fn test_short_body_stays_with_yield() {
    let code = "for user in users yield user";
    assert_formats_to(code, "for user in users yield user");
}

#[test]
fn test_long_body_breaks() {
    let code = "for user in users yield user.transform().validate().save()";
    assert_formats_to(code, r#"
for user in users yield
    user.transform().validate().save()
"#);
}
```

### BooleanBreakRule Tests
```rust
#[test]
fn test_two_or_clauses_inline() {
    let code = "a || b";
    assert_formats_inline(code, "a || b");
}

#[test]
fn test_three_or_clauses_break() {
    let code = "a || b || c";
    assert_formats_to(code, r#"
a
    || b
    || c
"#);
}
```

- [ ] **Test** ChainedElseIfRule
- [ ] **Test** NestedForRule
- [ ] **Test** ParenthesesRule
- [ ] **Test** RunRule
- [ ] **Test** LoopRule

---

## 06.5 Golden Tests

Maintain existing golden tests and add new ones for layered architecture.

- [ ] **Verify** all existing golden tests pass
  - `tests/fmt/declarations/*.golden`
  - `tests/fmt/expressions/*.golden`
  - `tests/fmt/patterns/*.golden`
  - `tests/fmt/collections/*.golden`
  - `tests/fmt/comments/*.golden`
  - `tests/fmt/edge-cases/*.golden`

- [ ] **Add** golden tests for each breaking rule
  - `tests/fmt/rules/method_chain.golden`
  - `tests/fmt/rules/short_body.golden`
  - `tests/fmt/rules/boolean_break.golden`
  - `tests/fmt/rules/chained_else_if.golden`
  - `tests/fmt/rules/nested_for.golden`
  - `tests/fmt/rules/parentheses.golden`
  - `tests/fmt/rules/run_rule.golden`
  - `tests/fmt/rules/loop_rule.golden`

- [ ] **Add** target example as golden test

```rust
// tests/fmt/complex/target_example.ori
// This should format to the target example in 00-overview.md
```

---

## 06.6 Property Tests

Property-based testing for formatting invariants.

- [ ] **Create** `ori_fmt/src/formatter/proptest.rs`
- [ ] **Implement** property tests

```rust
use proptest::prelude::*;

proptest! {
    /// Formatting is idempotent: format(format(x)) == format(x)
    #[test]
    fn test_idempotent(code in arbitrary_ori_code()) {
        let once = format_code(&code);
        let twice = format_code(&once);
        assert_eq!(once, twice);
    }

    /// Formatted code parses successfully
    #[test]
    fn test_preserves_parsability(code in arbitrary_ori_code()) {
        let formatted = format_code(&code);
        assert!(parses_successfully(&formatted));
    }

    /// Formatting preserves semantics (AST equality)
    #[test]
    fn test_preserves_semantics(code in arbitrary_ori_code()) {
        let formatted = format_code(&code);
        let original_ast = parse(&code);
        let formatted_ast = parse(&formatted);
        assert_eq!(original_ast, formatted_ast);
    }
}
```

- [ ] **Generate** arbitrary Ori code for property tests

---

## 06.7 Spec Compliance

Verify all spec rules are implemented.

- [ ] **Create** spec compliance checklist

```markdown
## Spec Lines Verified

### Spacing (Lines 25-47)
- [ ] Line 25-30: Binary operators ✓
- [ ] Line 31-35: Delimiters ✓
- [ ] Line 36-41: Keywords ✓
- [ ] Line 42-47: Context-dependent ✓

### Packing (Lines 58-92)
- [ ] Line 58-63: General width-based ✓
- [ ] Line 64-77: Construct-specific ✓
- [ ] Line 78-90: Always-stacked ✓
- [ ] Line 91-92: Run context ✓

... (continue for all spec sections)
```

- [ ] **Cross-reference** each spec line with test

---

## 06.8 Regression Testing

- [ ] **Run** formatter on entire codebase
  - `./fmt-all --check` must pass
  - No formatting changes to existing code (unless intentional)

- [ ] **Compare** output with previous version
  - Store baseline formatted output
  - Diff against new formatter

- [ ] **Performance** regression testing
  - Large file benchmark (10k lines)
  - Many files benchmark (100 files)

---

## 06.9 Completion Checklist

- [ ] Unit tests for each layer (Sections 06.1-06.4)
- [ ] All existing golden tests pass (Section 06.5)
- [ ] New golden tests for breaking rules (Section 06.5)
- [ ] Property tests for invariants (Section 06.6)
- [ ] Spec compliance verified (Section 06.7)
- [ ] No regressions (Section 06.8)

**Exit Criteria:** Test suite provides confidence that the layered architecture maintains all existing behavior while being more maintainable.
