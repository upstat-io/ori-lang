---
title: "Grammar Modules"
description: "Ori Compiler Design — Grammar Modules"
order: 402
section: "Parser"
---

# Grammar Modules

The Ori parser organizes grammar rules into separate modules for maintainability.

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
parse_use()           // use './math' { add, subtract }

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
parse_generics()      // <T, U: Bound>
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

### pattern.rs

Handles destructuring patterns:

```rust
// Literal patterns
parse_literal_pattern()  // 42, "hello"

// Binding patterns
parse_binding()          // x, _

// Struct patterns
parse_struct_pattern()   // { x, y }

// List patterns
parse_list_pattern()     // [head, ..tail]

// Variant patterns
parse_variant_pattern()  // Some(x), None

// Guards
parse_guard()            // x.match(x > 0)
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

Both `expr/` and `item/` have been split into sub-modules for maintainability:
- `expr/` → `mod.rs`, `operators.rs`, `primary.rs`, `postfix.rs`, `patterns.rs`
- `item/` → `mod.rs`, `use_def.rs`, `config.rs`, `function.rs`, `trait_def.rs`, `impl_def.rs`, `type_decl.rs`, `extend.rs`, `generics.rs`
