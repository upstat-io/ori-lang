---
title: "Grammar Modules"
description: "Ori Compiler Design — Grammar Modules"
order: 402
section: "Parser"
---

# Grammar Modules

The Ori parser organizes grammar rules into separate modules for maintainability. The authoritative grammar is in [Parser Overview § Formal Grammar](index.md#formal-grammar). Each production maps to parsing functions in the modules below.

## Module Structure

```
compiler/ori_parse/src/
├── lib.rs                  # Parser struct, entry point
├── error.rs                # Error types
└── grammar/
    ├── mod.rs              # Re-exports
    ├── expr/               # Expression parsing (split into submodules)
    │   ├── mod.rs              # Entry point, binary operators
    │   ├── operators.rs        # Operator matching helpers
    │   ├── primary.rs          # Literals, identifiers, lambdas
    │   ├── postfix.rs          # Call, method call, field, index
    │   └── patterns.rs         # run, try, match, for, function_exp
    ├── item/               # Top-level items (split into submodules)
    │   ├── mod.rs              # Re-exports
    │   ├── use_def.rs          # Import/use statements
    │   ├── config.rs           # Config variable parsing
    │   ├── function.rs         # Function and test definitions
    │   ├── trait_def.rs        # Trait definitions
    │   ├── impl_def.rs         # Impl blocks
    │   ├── type_decl.rs        # Type declarations (struct, enum, newtype)
    │   ├── extend.rs           # Extend blocks
    │   └── generics.rs         # Generic params, bounds, where clauses
    ├── ty.rs               # Type annotations
    └── attr.rs             # Attributes
```

## Module Responsibilities

### expr.rs (~1,337 lines)

Handles all expression parsing:

```rust
// Literals
parse_literal()       // 42, "hello", true

// Operators
parse_binary_expr()   // a + b, x && y
parse_unary_expr()    // -x, !cond

// Control flow
parse_if()            // if cond then x else y
parse_for()           // for x in items do/yield
parse_loop()          // loop(...) with break/continue
parse_match()         // match(value, ...)

// Calls
parse_call()          // func(args)
parse_method_call()   // obj.method(args)
parse_pattern_call()  // map(over: items, ...)

// Structures
parse_list()          // [1, 2, 3]
parse_map()           // {"key": value}
parse_struct()        // Point { x: 0, y: 0 }

// Lambdas
parse_lambda()        // x -> x + 1
```

### item/ (~1,112 lines total, split into 8 modules)

Handles top-level declarations:

```rust
// use_def.rs - Imports
parse_use_inner()     // use "./math" { add, subtract }
                      // use std.net.http as http (module alias)
                      // pub use "./internal" { helper } (re-exports)

// config.rs - Config variables
parse_config()        // $timeout = 30s

// function.rs - Functions and tests
parse_function_or_test_with_attrs()  // @name (params) -> Type = body
parse_params()        // (a: int, b: str)

// trait_def.rs - Traits
parse_trait()         // trait Name { ... }
parse_trait_item()    // Method signatures, default methods, assoc types

// impl_def.rs - Implementations
parse_impl()          // impl Trait for Type { ... }
parse_impl_method()   // @method (self) -> Type = body

// type_decl.rs - Type declarations
parse_type_decl()     // type Name = ...
parse_struct_body()   // { x: int, y: int }
parse_sum_or_newtype()// Some(T) | None

// extend.rs - Extension methods
parse_extend()        // extend [T] { @map... }

// generics.rs - Generic parameters
parse_generics()      // <T, U: Bound>, <T = Self>, <T: Clone = int>
parse_bounds()        // Eq + Clone + Printable
parse_where_clauses() // where T: Clone, U: Default
parse_uses_clause()   // uses Http, FileSystem
```

### type.rs

Handles type annotations:

```rust
// Simple types
parse_type()          // int, str, bool

// Compound types
parse_list_type()     // [int]
parse_map_type()      // {str: int}
parse_tuple_type()    // (int, str)
parse_option_type()   // Option<T>
parse_result_type()   // Result<T, E>

// Function types
parse_function_type() // (int, int) -> int

// Generics
parse_generic_args()  // <T, U>
parse_type_bounds()   // T: Eq + Clone
```

### pattern.rs (Match Patterns)

Handles match arm patterns in `expr/patterns.rs`:

```rust
// Literal patterns
parse_literal_pattern()  // 42, "hello"

// Binding patterns
parse_binding()          // x, _

// Struct patterns
parse_struct_pattern()   // { x, y }

// List patterns
parse_list_pattern()     // [head, ..tail]

// Variant patterns (single and multi-field)
parse_variant_pattern()       // Some(x), None, Click(x, y)
parse_variant_inner_patterns() // Helper for comma-separated patterns

// Guards
parse_guard()            // x.match(x > 0)
```

#### Multi-Field Variant Patterns

Variant patterns support multiple fields via `parse_variant_inner_patterns()`:

```rust
// Grammar: type_path [ "(" pattern { "," pattern } ")" ]
fn parse_variant_inner_patterns(&mut self) -> Result<Vec<MatchPattern>, ParseError> {
    let mut patterns = Vec::new();
    if self.check(&TokenKind::RParen) {
        return Ok(patterns);  // Unit variant: None, Quit
    }
    patterns.push(self.parse_match_pattern()?);
    while self.check(&TokenKind::Comma) {
        self.advance();
        if self.check(&TokenKind::RParen) { break; }  // Trailing comma
        patterns.push(self.parse_match_pattern()?);
    }
    Ok(patterns)
}
```

Examples:
- Unit variant: `None` → `inner: []`
- Single-field: `Some(x)` → `inner: [Binding("x")]`
- Multi-field: `Click(x, y)` → `inner: [Binding("x"), Binding("y")]`
- Nested: `Event(Click(x, _))` → `inner: [Variant { name: "Click", inner: [...] }]`

### Binding Patterns (in primary.rs)

Handles let binding patterns in `expr/primary.rs`:

```rust
// parse_binding_pattern() handles:
parse_binding_pattern()  // Entry point

// Name binding
// let x = value
BindingPattern::Name(name)

// Wildcard
// let _ = value
BindingPattern::Wildcard

// Tuple destructuring
// let (a, b) = pair
BindingPattern::Tuple(patterns)

// Struct destructuring
// let { x, y } = point
// let { x: px, y: py } = point  (rename)
// let { position: { x, y } } = entity  (nested)
BindingPattern::Struct { fields }

// List destructuring
// let [a, b, c] = items
// let [head, ..rest] = items
BindingPattern::List { elements, rest }
```

### stmt.rs

Handles statement-like constructs:

```rust
// Let bindings
parse_let()              // let x = value
parse_let_mut()          // let mut x = value

// Sequences (in run/try)
parse_sequence()         // expr, expr, result
```

### attr.rs

Handles attributes:

```rust
// Simple attributes
parse_attribute()        // #[skip("reason")]

// Derive attributes
parse_derive()           // #[derive(Eq, Clone)]

// Test attributes
parse_test_attr()        // #[compile_fail("error")]
```

## Cross-Module Dependencies

```
         mod.rs (entry)
            |
            v
     ┌──────┴──────┐
     |             |
  item.rs      expr.rs
     |             |
     ├─────────────┤
     |             |
  type.rs    pattern.rs
     |             |
     └──────┬──────┘
            |
         attr.rs
```

- `mod.rs` calls `item.rs` for top-level parsing
- `item.rs` calls `expr.rs` for function bodies
- `item.rs` calls `type.rs` for type annotations
- `expr.rs` calls `pattern.rs` for match arms
- All modules can call `attr.rs` for attributes

## Naming Conventions

### Function Names

```rust
// parse_X - parse and return X
fn parse_expr(&mut self) -> ExprId
fn parse_function(&mut self) -> Function

// try_parse_X - parse X or return None
fn try_parse_named_arg(&mut self) -> Option<NamedArg>

// expect_X - must find X or error
fn expect_type(&mut self) -> Type

// parse_X_list - parse comma-separated X
fn parse_param_list(&mut self) -> Vec<Param>
```

### Helper Functions

```rust
// check_X - test without consuming
fn check_keyword(&self, kw: &str) -> bool

// eat_X - consume if present
fn eat_comma(&mut self) -> bool

// skip_X - skip over X (for recovery)
fn skip_to_close_brace(&mut self)
```

## Adding New Grammar

### 1. Choose Module

- New expression → `expr.rs`
- New declaration → `item.rs`
- New type syntax → `type.rs`

### 2. Add Parse Function

```rust
// In expr.rs
fn parse_new_feature(&mut self) -> ExprId {
    // Parse the new syntax
    self.expect(TokenKind::NewKeyword);
    let value = self.parse_expr();
    self.alloc(ExprKind::NewFeature { value })
}
```

### 3. Wire Into Grammar

```rust
// In parse_primary or appropriate caller
fn parse_primary(&mut self) -> ExprId {
    match self.current() {
        TokenKind::NewKeyword => self.parse_new_feature(),
        // ...existing cases...
    }
}
```

### 4. Add Tests

```rust
#[test]
fn test_parse_new_feature() {
    let result = parse("new_keyword 42");
    assert_matches!(result.module.expressions[0], Expr::NewFeature { .. });
}
```

## File Size Guidelines

| Module | Target | Maximum | Current |
|--------|--------|---------|---------|
| expr/ (total) | 800 | 1,500 | ~1,100 |
| item/ (total) | 800 | 1,500 | ~1,112 |
| ty.rs | 200 | 400 | ~400 |
| attr.rs | 200 | 400 | ~400 |

Both `expr/` and `item/` are split into sub-modules for maintainability:
- `expr/` → `mod.rs`, `operators.rs`, `primary.rs`, `postfix.rs`, `patterns.rs`
- `item/` → `mod.rs`, `use_def.rs`, `config.rs`, `function.rs`, `trait_def.rs`, `impl_def.rs`, `type_decl.rs`, `extend.rs`, `generics.rs`

## Compound Operator Synthesis

The lexer produces individual `>` tokens. The parser synthesizes `>>` and `>=` from adjacent tokens in expression context. See [Token Design](../03-lexer/token-design.md#lexer-parser-token-boundary).

### Cursor Methods

The `Cursor` type (`cursor.rs`) detects adjacent tokens:

```rust
impl Cursor<'_> {
    fn spans_adjacent(&self, span1: Span, span2: Span) -> bool {
        span1.end == span2.start
    }

    fn is_shift_right(&self) -> bool {  // >>
        self.check(&TokenKind::Gt)
            && matches!(self.peek_next_kind(), TokenKind::Gt)
            && self.current_and_next_adjacent()
    }

    fn is_greater_equal(&self) -> bool {  // >=
        self.check(&TokenKind::Gt)
            && matches!(self.peek_next_kind(), TokenKind::Eq)
            && self.current_and_next_adjacent()
    }
}
```

### Operator Matching

Matcher functions in `operators.rs` return `(BinaryOp, usize)` — the operator and token count to consume:

```rust
fn match_comparison_op(&self) -> Option<(BinaryOp, usize)> {
    match self.current_kind() {
        TokenKind::Lt => Some((BinaryOp::Lt, 1)),
        TokenKind::LtEq => Some((BinaryOp::LtEq, 1)),
        TokenKind::Gt => {
            if self.is_greater_equal() {
                Some((BinaryOp::GtEq, 2))  // >= consumes 2 tokens
            } else {
                Some((BinaryOp::Gt, 1))
            }
        }
        _ => None,
    }
}

fn match_shift_op(&self) -> Option<(BinaryOp, usize)> {
    match self.current_kind() {
        TokenKind::Shl => Some((BinaryOp::Shl, 1)),
        TokenKind::Gt if self.is_shift_right() => Some((BinaryOp::Shr, 2)),  // >> consumes 2
        _ => None,
    }
}
```

### Binary Level Macro

The `parse_binary_level!` macro uses the token count:

```rust
while let Some((op, token_count)) = self.$matcher() {
    for _ in 0..token_count { self.advance(); }
    let right = self.$next()?;
    // ... build binary expression ...
}
```

### Type Parser

The type parser uses single `>` tokens to close generic parameter lists:

```rust
fn parse_optional_generic_args_full(&mut self) -> Vec<ParsedType> {
    self.advance(); // <
    // ... parse type arguments ...
    if self.check(&TokenKind::Gt) {
        self.advance(); // > (single token)
    }
    args
}
```

Nested generics use multiple `>` tokens: `Result<Result<int, str>, str>` has two `>` tokens.
