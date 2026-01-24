# Grammar Modules

The Sigil parser organizes grammar rules into separate modules for maintainability.

## Module Structure

```
compiler/sigilc/src/parser/
├── mod.rs              # Parser struct, entry point
├── error.rs            # Error types
└── grammar/
    ├── mod.rs          # Re-exports
    ├── expr.rs         # Expression parsing
    ├── item.rs         # Top-level items
    ├── type.rs         # Type annotations
    ├── pattern.rs      # Destructuring patterns
    ├── stmt.rs         # Statement-like constructs
    └── attr.rs         # Attributes
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
parse_pattern_call()  // map(.over: items, ...)

// Structures
parse_list()          // [1, 2, 3]
parse_map()           // {"key": value}
parse_struct()        // Point { x: 0, y: 0 }

// Lambdas
parse_lambda()        // x -> x + 1
```

### item.rs (~446 lines)

Handles top-level declarations:

```rust
// Functions
parse_function()      // @name (params) -> Type = body
parse_params()        // (a: int, b: str)

// Types
parse_type_def()      // type Name = ...
parse_struct_def()    // type Point = { x: int, y: int }
parse_enum_def()      // type Option<T> = Some(T) | None

// Traits
parse_trait()         // trait Name { ... }
parse_impl()          // impl Trait for Type { ... }

// Tests
parse_test()          // @test_name tests @target () -> void = ...

// Imports
parse_import()        // use './math' { add, subtract }
parse_config()        // $timeout = 30s
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

| File | Target | Maximum | Current |
|------|--------|---------|---------|
| expr.rs | 800 | 1,500 | ~1,337 |
| item.rs | 400 | 600 | ~446 |
| type.rs | 200 | 400 | ~200 |
| pattern.rs | 200 | 400 | ~200 |
| stmt.rs | 100 | 200 | ~100 |
| attr.rs | 100 | 200 | ~100 |

If a file exceeds limits, split into sub-modules:
- `expr.rs` → `expr/binary.rs`, `expr/control.rs`, `expr/call.rs`
