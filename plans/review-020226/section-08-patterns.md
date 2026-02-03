---
section: "08"
title: Extractable Patterns
status: not-started
priority: high
goal: Consolidate repetitive patterns into shared abstractions
files:
  - compiler/ori_ir/src/token.rs
  - compiler/ori_eval/src/operators.rs
  - compiler/ori_parse/src/incremental.rs
  - compiler/ori_patterns/src/value/mod.rs
  - compiler/ori_fmt/src/spacing/rules.rs
---

# Section 08: Extractable Patterns

**Status:** 📋 Planned
**Priority:** HIGH — Repetitive code is maintenance burden and bug magnet
**Goal:** Extract common patterns into shared abstractions or code generation

---

## 08.1 Token Display (115+ match arms)

Location: `compiler/ori_ir/src/token.rs:420-536`

### Problem

`display_name()` has 115+ match arms mapping token variants to strings.
Every new token requires adding an arm.

### Solution: Const Lookup Table

- [ ] Create lookup table
  ```rust
  impl TokenKind {
      /// Display name for error messages.
      pub fn display_name(self) -> &'static str {
          // Use discriminant as index into const array
          const NAMES: &[&str] = &[
              "integer",      // TokenKind::Int = 0
              "float",        // TokenKind::Float = 1
              "string",       // TokenKind::String = 2
              // ... all variants in order
          ];
          NAMES[self as usize]
      }
  }
  ```

- [ ] Alternative: Derive macro
  ```rust
  #[derive(TokenDisplay)]
  #[token(display = "integer")]
  Int(i64),
  ```

- [ ] Apply same pattern to `discriminant_index()` if not already table-driven

---

## 08.2 Binary Operator Pattern (15 functions)

Location: `compiler/ori_eval/src/operators.rs`

### Problem

15 nearly-identical functions:
- `eval_int_binary`
- `eval_float_binary`
- `eval_string_binary`
- `eval_list_binary`
- `eval_char_binary`
- `eval_tuple_binary`
- `eval_option_binary`
- `eval_result_binary`
- `eval_duration_binary`
- `eval_size_binary`
- ... etc

All follow same pattern: match op, apply operation, wrap result.

### Solution: Trait-Based Dispatch

- [ ] Create `BinaryOperator` trait
  ```rust
  pub trait BinaryOperator: Sized {
      fn apply_add(self, other: Self) -> Result<Value, EvalError>;
      fn apply_sub(self, other: Self) -> Result<Value, EvalError>;
      fn apply_mul(self, other: Self) -> Result<Value, EvalError>;
      fn apply_div(self, other: Self) -> Result<Value, EvalError>;
      fn apply_eq(self, other: Self) -> bool;
      fn apply_cmp(self, other: Self) -> Ordering;
      // ... other ops
  }
  ```

- [ ] Implement for each type
  ```rust
  impl BinaryOperator for i64 {
      fn apply_add(self, other: Self) -> Result<Value, EvalError> {
          self.checked_add(other)
              .map(Value::Int)
              .ok_or_else(|| EvalError::overflow())
      }
      // ...
  }
  ```

- [ ] Single dispatch function
  ```rust
  pub fn eval_binary(left: Value, op: BinaryOp, right: Value) -> Result<Value, EvalError> {
      match (left, right) {
          (Value::Int(a), Value::Int(b)) => a.apply_op(op, b),
          (Value::Float(a), Value::Float(b)) => a.apply_op(op, b),
          // ...
      }
  }
  ```

---

## 08.3 AST Copy Methods (29 methods, 800 lines)

Location: `compiler/ori_parse/src/incremental.rs:548-1348`

### Problem

29 `copy_*` methods all follow the same pattern:
1. Get old value from old arena
2. Recursively copy children
3. Adjust spans
4. Allocate in new arena

### Solution: Visitor Pattern

- [ ] Create `AstVisitor` trait
  ```rust
  pub trait AstVisitor {
      type Output;

      fn visit_expr(&mut self, expr: &Expr) -> Self::Output;
      fn visit_stmt(&mut self, stmt: &Stmt) -> Self::Output;
      fn visit_pattern(&mut self, pattern: &MatchPattern) -> Self::Output;
      // ... other node types
  }
  ```

- [ ] Implement `CopyVisitor`
  ```rust
  impl AstVisitor for AstCopier<'_> {
      type Output = ExprId; // or appropriate ID type

      fn visit_expr(&mut self, expr: &Expr) -> ExprId {
          let kind = match &expr.kind {
              ExprKind::Int(n) => ExprKind::Int(*n),
              ExprKind::Binary { left, op, right } => ExprKind::Binary {
                  left: self.visit_expr_id(*left),
                  op: *op,
                  right: self.visit_expr_id(*right),
              },
              // ... all variants, but structure is uniform
          };
          self.new_arena.alloc(Expr {
              kind,
              span: self.adjust_span(expr.span),
          })
      }
  }
  ```

- [ ] Alternative: Derive macro for deep copy with span adjustment

---

## 08.4 Value Debug/Display (40+ arms each)

Location: `compiler/ori_patterns/src/value/mod.rs:659-893`

### Problem

Debug and Display impls have 40+ nearly identical match arms:
```rust
Value::Int(n) => write!(f, "Int({n})"),
Value::Float(n) => write!(f, "Float({n})"),
// ... 40 more
```

### Solution: Derive Macro

- [ ] Use standard derive where possible
  ```rust
  #[derive(Debug)]  // If default Debug output is acceptable
  pub enum Value { ... }
  ```

- [ ] Or create custom derive
  ```rust
  #[derive(ValueDebug, ValueDisplay)]
  pub enum Value {
      #[display("{0}")]
      Int(i64),
      #[display("{0}")]
      Float(f64),
      // ...
  }
  ```

- [ ] Apply to Hash and PartialEq if also manual

---

## 08.5 Spacing Rules (100+ rules)

Location: `compiler/ori_fmt/src/spacing/rules.rs`

### Problem

100+ static `SpaceRule` entries with repetitive structure.

### Solution: DSL or Table-Driven

- [ ] Create declarative format
  ```rust
  spacing_rules! {
      // Delimiters - no space inside
      (LParen, _) => NoSpace @ 20,
      (_, RParen) => NoSpace @ 20,
      (LBracket, _) => NoSpace @ 20,
      (_, RBracket) => NoSpace @ 20,

      // Operators - space around
      (_, Plus | Minus | Star | Slash) => Space @ 10,
      (Plus | Minus | Star | Slash, _) => Space @ 10,

      // Keywords
      (If | For | While, _) => Space @ 15,
  }
  ```

- [ ] Generate `Vec<SpaceRule>` at compile time

---

## 08.6 Declaration Collection (9 similar blocks)

Location: `compiler/ori_parse/src/incremental.rs:54-132`

### Problem

9 nearly identical blocks:
```rust
for (i, func) in module.functions.iter().enumerate() {
    decls.push(DeclRef { kind: DeclKind::Function, index: i, span: func.span });
}
for (i, ty) in module.types.iter().enumerate() {
    decls.push(DeclRef { kind: DeclKind::Type, index: i, span: ty.span });
}
// ... 7 more
```

### Solution: Macro or Helper

- [ ] Create helper function
  ```rust
  fn collect_items<T: HasSpan>(
      items: &[T],
      kind: DeclKind,
      decls: &mut Vec<DeclRef>,
  ) {
      for (i, item) in items.iter().enumerate() {
          decls.push(DeclRef { kind, index: i, span: item.span() });
      }
  }
  ```

- [ ] Or use macro
  ```rust
  macro_rules! collect_decls {
      ($module:expr, $decls:expr, $($field:ident => $kind:ident),+ $(,)?) => {
          $(
              for (i, item) in $module.$field.iter().enumerate() {
                  $decls.push(DeclRef { kind: DeclKind::$kind, index: i, span: item.span });
              }
          )+
      };
  }

  collect_decls!(module, decls,
      functions => Function,
      types => Type,
      traits => Trait,
      // ...
  );
  ```

---

## 08.7 Builtin Methods (250+ entries)

Location: `compiler/ori_ir/src/builtin_methods.rs`

### Current State

Well-organized static array. Approaching maintainability threshold.

### Solution: Consider Code Generation

- [ ] Evaluate if schema-based generation would help
- [ ] If maintained manually, ensure good documentation
- [ ] Lower priority — current approach is acceptable

---

## 08.8 Verification

- [ ] No duplicate code patterns >3 occurrences
- [ ] Repetitive match arms use helper functions or macros
- [ ] `./clippy-all` passes
- [ ] `./test-all` passes

---

## 08.N Completion Checklist

- [ ] Token display uses lookup table or derive
- [ ] Binary operators use trait-based dispatch
- [ ] AST copy uses visitor pattern
- [ ] Value Debug/Display use derive
- [ ] Spacing rules use DSL
- [ ] Declaration collection uses helper/macro
- [ ] `./test-all` passes

**Exit Criteria:** No repetitive code patterns; shared abstractions for common operations
