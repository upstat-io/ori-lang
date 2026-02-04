---
section: "03"
title: Enhanced Progress System
status: not-started
goal: Extend progress tracking with context capture for Elm-quality error messages
sections:
  - id: "03.1"
    title: ParseOutcome with Context
    status: not-started
  - id: "03.2"
    title: Automatic Backtracking Macros
    status: not-started
  - id: "03.3"
    title: Expected Token Accumulation
    status: not-started
  - id: "03.4"
    title: Context Wrapping Utilities
    status: not-started
---

# Section 03: Enhanced Progress System

**Status:** 📋 Planned
**Goal:** Elm/Roc-style progress tracking with rich error context
**Source:** Elm (`compiler/src/Parse/Primitives.hs`), Roc (`crates/compiler/parse/src/parser.rs`)

---

## Background

Ori already has excellent progress tracking:
```rust
pub enum Progress {
    Made,  // Consumed input
    None,  // No input consumed
}
```

This section enhances it with:
1. **Context capture** — Know WHAT was being parsed when error occurred
2. **Automatic backtracking** — `one_of!` macro like Roc
3. **Expected token accumulation** — List ALL tokens that could have worked (from Rust)
4. **Context wrapping** — `in_context()` like Elm

---

## 03.1 ParseOutcome with Context

**Goal:** Extend progress to carry parsing context for better errors

### Tasks

- [ ] Design `ParseOutcome` enum
  ```rust
  pub enum ParseOutcome<T, E> {
      /// Consumed input and succeeded
      ConsumedOk {
          value: T,
          state: State,
      },

      /// No input consumed, succeeded (for optional parsers)
      EmptyOk {
          value: T,
          state: State,
      },

      /// Consumed input then failed (hard error, no backtracking)
      ConsumedErr {
          error: E,
          context: ParseContext,
          consumed_span: Span,
      },

      /// No input consumed, failed (can try alternatives)
      EmptyErr {
          expected: TokenTypeSet,
          position: Position,
      },
  }
  ```

- [ ] Add helper methods
  ```rust
  impl<T, E> ParseOutcome<T, E> {
      pub fn is_ok(&self) -> bool { ... }
      pub fn made_progress(&self) -> bool { ... }
      pub fn map<U>(self, f: impl FnOnce(T) -> U) -> ParseOutcome<U, E> { ... }
      pub fn map_err<F>(self, f: impl FnOnce(E) -> F) -> ParseOutcome<T, F> { ... }
  }
  ```

- [ ] Implement `From` conversions
  ```rust
  impl<T, E> From<ParseOutcome<T, E>> for Result<T, ParseError> { ... }
  impl<T, E> From<ParseResult<T>> for ParseOutcome<T, E> { ... }
  ```

- [ ] Migration: Update core parsing functions to return `ParseOutcome`

### Design Notes

The key insight from Elm is the **four-way distinction**:

| Progress | Result | Meaning |
|----------|--------|---------|
| Consumed | Ok | Committed to this parse path |
| Empty | Ok | Optional content not present |
| Consumed | Err | Real error (no backtracking) |
| Empty | Err | Try next alternative |

---

## 03.2 Automatic Backtracking Macros

**Goal:** `one_of!` macro for clean alternative parsing

### Tasks

- [ ] Design `one_of!` macro
  ```rust
  macro_rules! one_of {
      ($self:expr, $($parser:expr),+ $(,)?) => {{
          let original = $self.snapshot();
          $(
              match $parser {
                  ParseOutcome::ConsumedOk { value, state } => {
                      return ParseOutcome::ConsumedOk { value, state };
                  }
                  ParseOutcome::EmptyOk { value, state } => {
                      return ParseOutcome::EmptyOk { value, state };
                  }
                  ParseOutcome::ConsumedErr { error, context, consumed_span } => {
                      // Hard error: propagate immediately
                      return ParseOutcome::ConsumedErr { error, context, consumed_span };
                  }
                  ParseOutcome::EmptyErr { expected, .. } => {
                      // Soft error: try next, accumulate expected
                      $self.expected_tokens.union_with(&expected);
                      $self.restore(original.clone());
                  }
              }
          )+
          // All alternatives failed without consuming
          ParseOutcome::EmptyErr {
              expected: $self.expected_tokens.clone(),
              position: $self.current_position(),
          }
      }};
  }
  ```

- [ ] Add `try_parse!` for optional parsing
  ```rust
  macro_rules! try_parse {
      ($self:expr, $parser:expr) => {
          match $parser {
              ParseOutcome::ConsumedOk { value, state } => Some(value),
              ParseOutcome::EmptyOk { value, state } => Some(value),
              ParseOutcome::ConsumedErr { .. } => return /* propagate */,
              ParseOutcome::EmptyErr { .. } => None,
          }
      };
  }
  ```

- [ ] Add `require!` for mandatory parsing
  ```rust
  macro_rules! require {
      ($self:expr, $parser:expr, $context:expr) => {
          match $parser {
              ParseOutcome::ConsumedOk { value, state } => value,
              ParseOutcome::EmptyOk { value, state } => value,
              err => return $self.wrap_error(err, $context),
          }
      };
  }
  ```

- [ ] Update parser to use macros
  ```rust
  // Before
  fn parse_expr(&mut self) -> ParseResult<Expr> {
      if let Ok(lit) = self.parse_literal() { return Ok(lit); }
      if let Ok(id) = self.parse_ident() { return Ok(id); }
      // ...
  }

  // After
  fn parse_expr(&mut self) -> ParseOutcome<Expr, EExpr> {
      one_of!(self,
          self.parse_literal(),
          self.parse_ident(),
          self.parse_if_expr(),
          self.parse_match_expr(),
      )
  }
  ```

---

## 03.3 Expected Token Accumulation

**Goal:** Collect ALL expected tokens for comprehensive error messages

### Tasks

- [ ] Review existing `TokenTypeSet` (128-bit bitset)
  - [ ] Location: `compiler/ori_parse/src/recovery.rs`
  - [ ] Verify it can accumulate across alternatives

- [ ] Add accumulation during parsing
  ```rust
  impl Parser<'_> {
      fn check(&mut self, expected: ExpTokenPair) -> bool {
          let matched = self.current_kind() == expected.kind;
          if !matched {
              self.expected_tokens.insert(expected.token_type);
          }
          matched
      }
  }
  ```

- [ ] Implement friendly token type names
  ```rust
  impl TokenType {
      pub fn friendly_name(&self) -> &'static str {
          match self {
              TokenType::Identifier => "an identifier",
              TokenType::IntLiteral => "an integer",
              TokenType::StringLiteral => "a string",
              TokenType::Keyword(kw) => kw.friendly_name(),
              TokenType::Punctuation(p) => p.symbol(),
              // ...
          }
      }
  }
  ```

- [ ] Generate error messages from accumulated set
  ```rust
  fn expected_one_of_not_found(&self) -> ParseError {
      let expected: Vec<_> = self.expected_tokens.iter()
          .map(|t| t.friendly_name())
          .collect();
      expected.sort();
      expected.dedup();

      ParseError::unexpected_token(
          found: self.current_token().clone(),
          expected: expected,
          position: self.current_position(),
      )
  }
  ```

### Example Output

```
error: Unexpected token
  --> src/main.ori:10:5
   |
10 |     foo bar
   |         ^^^ found identifier `bar`
   |
   = expected one of: `,`, `)`, `+`, `-`, `*`, `/`, `==`
```

---

## 03.4 Context Wrapping Utilities

**Goal:** Elm-style `in_context` for rich error context

### Tasks

- [ ] Design `in_context` function
  ```rust
  pub fn in_context<T, E, F>(
      &mut self,
      context: ParseContext,
      parser: F,
  ) -> ParseOutcome<T, E>
  where
      F: FnOnce(&mut Self) -> ParseOutcome<T, E>,
  {
      let start_pos = self.current_position();
      match parser(self) {
          ok @ ParseOutcome::ConsumedOk { .. } => ok,
          ok @ ParseOutcome::EmptyOk { .. } => ok,
          ParseOutcome::ConsumedErr { error, context: _, consumed_span } => {
              // Wrap with our context
              ParseOutcome::ConsumedErr {
                  error,
                  context,
                  consumed_span,
              }
          }
          ParseOutcome::EmptyErr { expected, position } => {
              ParseOutcome::EmptyErr { expected, position }
          }
      }
  }
  ```

- [ ] Define `ParseContext` enum
  ```rust
  #[derive(Clone, Debug)]
  pub enum ParseContext {
      Expression,
      Statement,
      Pattern,
      Type,
      FunctionDef,
      IfExpression,
      MatchExpression,
      LetBinding,
      ListLiteral,
      MapLiteral,
      // ... more contexts
  }

  impl ParseContext {
      pub fn description(&self) -> &'static str {
          match self {
              Self::Expression => "an expression",
              Self::IfExpression => "an if expression",
              Self::MatchExpression => "a match expression",
              // ...
          }
      }
  }
  ```

- [ ] Add `specialize_err` for error type conversion
  ```rust
  pub fn specialize_err<T, E1, E2, F, G>(
      &mut self,
      converter: G,
      parser: F,
  ) -> ParseOutcome<T, E2>
  where
      F: FnOnce(&mut Self) -> ParseOutcome<T, E1>,
      G: FnOnce(E1) -> E2,
  {
      parser(self).map_err(converter)
  }
  ```

- [ ] Use in parser
  ```rust
  fn parse_if_expr(&mut self) -> ParseOutcome<Expr, EExpr> {
      self.in_context(ParseContext::IfExpression, |p| {
          p.expect(&TokenKind::If)?;
          let cond = p.specialize_err(EExpr::IfCondition, |p| p.parse_expr())?;
          p.expect(&TokenKind::Then)?;
          let then_branch = p.specialize_err(EExpr::IfThen, |p| p.parse_expr())?;
          p.expect(&TokenKind::Else)?;
          let else_branch = p.specialize_err(EExpr::IfElse, |p| p.parse_expr())?;
          // ...
      })
  }
  ```

---

## 03.5 Completion Checklist

- [ ] `ParseOutcome` type implemented
- [ ] `one_of!`, `try_parse!`, `require!` macros working
- [ ] Expected token accumulation functional
- [ ] `in_context` and `specialize_err` implemented
- [ ] All parser tests pass with new types
- [ ] Error messages show full expected set

**Exit Criteria:**
- Error messages list all valid alternatives
- Context is preserved through parsing chain
- Automatic backtracking works correctly
- No performance regression
