---
title: "Grammar Modules"
description: "Ori Compiler Design — Grammar Modules"
order: 402
section: "Parser"
---

# Grammar Modules

The Ori parser organizes grammar rules into separate modules for maintainability. The authoritative grammar is in [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf). Each production maps to parsing functions in the modules below.

## Module Structure

```
compiler/ori_parse/src/
├── lib.rs                  # Parser struct, entry point
├── error.rs                # Error types
└── grammar/
    ├── mod.rs              # Re-exports
    ├── expr/               # Expression parsing (split into submodules)
    │   ├── mod.rs              # Entry point, Pratt parser for binary operators
    │   ├── operators.rs        # Binding power table, operator matching
    │   ├── primary.rs          # Literals, identifiers, lambdas, let bindings
    │   ├── postfix.rs          # Call, method call, field, index, await, try, cast
    │   └── patterns.rs         # block, try, match, for, function_exp
    ├── item/               # Top-level items (split into submodules)
    │   ├── mod.rs              # Re-exports
    │   ├── use_def.rs          # Import/use statements
    │   ├── config.rs           # Config variable parsing
    │   ├── function.rs         # Function and test definitions
    │   ├── trait_def.rs        # Trait definitions
    │   ├── impl_def.rs         # Impl blocks, def impl blocks
    │   ├── type_decl.rs        # Type declarations (struct, sum, newtype)
    │   ├── extend.rs           # Extend blocks
    │   └── generics.rs         # Generic params, bounds, where clauses
    ├── ty.rs               # Type annotations
    └── attr.rs             # Attributes
```

## Module Responsibilities

### expr/ — Expressions

#### mod.rs — Pratt Parser Entry

The core expression parsing entry point. Handles binary operators via a Pratt parser with a binding power table (see [Pratt Parser](pratt-parser.md)):

```rust
parse_expr()                 // Full expression (includes assignment)
parse_non_assign_expr()      // No top-level = (guard clauses)
parse_non_comparison_expr()  // No < > (const generic defaults)
parse_binary_pratt(min_bp)   // Pratt loop: single-loop precedence climbing
parse_unary()                // - ! ~ with constant folding for negation
parse_range_continuation()   // left .. end by step
```

#### operators.rs — Binding Power Table

Maps `TokenKind` to `(left_bp, right_bp, BinaryOp, token_count)`:

```rust
infix_binding_power()        // Binary operator → binding power + token count
match_unary_op()             // - ! ~ → UnaryOp
match_function_exp_kind()    // Keyword → FunctionExpKind (recurse, parallel, spawn, ...)
```

Compound operators (`>=`, `>>`) return `token_count = 2` since they consume two `>` tokens.

#### primary.rs — Literals and Atoms

```rust
// Literals
parse_literal()              // 42, "hello", true, 3.14, 'c', 5s, 1MB
parse_string()               // String literal with escape handling

// Identifiers and special names
parse_ident()                // x, my_var
parse_self_ref()             // self
parse_function_ref()         // @name (function references)
parse_hash_length()          // # (collection length in index context)

// Variant constructors
parse_ok_expr()              // Ok(value)
parse_err_expr()             // Err(value)
parse_some_expr()            // Some(value)
parse_none()                 // None

// Bindings
parse_let()                  // let x = value, let mut x = value
parse_binding_pattern()      // Destructuring: (a, b), { x, y }, [head, ..rest]

// Lambdas
parse_lambda()               // x -> x + 1, (a, b) -> a + b
```

#### postfix.rs — Postfix Operations

```rust
apply_postfix_ops(base)      // Loop applying postfix operators to base expression

// Individual operations:
parse_call_args()            // f(a, b, c)
parse_named_call_args()      // f(a: 1, b: 2)
parse_method_call()          // obj.method(args)
parse_field_access()         // obj.field
parse_index()                // arr[i]
parse_try()                  // expr?
parse_await()                // expr.await
parse_cast()                 // expr as Type, expr as? Type
```

#### patterns.rs — Control Flow and Pattern Expressions

```rust
// Sequential execution
parse_block()                // { expr \n expr \n result }
parse_try_expr()             // try { expr? \n Ok(value) }

// Pattern matching
parse_match()                // match value { pattern -> body \n ... }
parse_match_pattern()        // Literal, binding, struct, list, variant patterns
parse_variant_inner_patterns() // Comma-separated patterns inside variants

// Loops
parse_for()                  // for x in items do body / yield transform
parse_loop()                 // loop { body }

// Function expressions (compiler patterns)
parse_function_exp()         // recurse(...), parallel(...), spawn(...), with(...)
```

### item/ — Top-Level Declarations

#### function.rs — Functions and Tests

```rust
parse_function_or_test_with_outcome()  // Dispatches @ to function or test
parse_function()                       // @name (params) -> Type = body
parse_test()                           // @test_name tests @target (args) -> void = body
parse_params()                         // (a: int, b: str = "default")
parse_return_type()                    // -> Type
parse_capabilities()                   // uses Http, FileSystem
```

#### type_decl.rs — Type Declarations

```rust
parse_type_decl()            // type Name = ...
parse_struct_body()          // { x: int, y: int }
parse_sum_or_newtype()       // Some(T) | None
```

#### trait_def.rs — Trait Definitions

```rust
parse_trait()                // trait Name { ... }
parse_trait_item()           // Method signatures, default methods
                             // Associated types: type Item, type Output = Self
```

#### impl_def.rs — Implementations

```rust
parse_impl()                 // impl Trait for Type { ... }
parse_def_impl()             // def impl Trait { ... }
parse_impl_method()          // @method (self) -> Type = body
```

#### use_def.rs — Imports

```rust
parse_use_inner()            // use "./math" { add, subtract }
                             // use std.net.http as http
                             // pub use "./internal" { helper }
```

#### extend.rs — Extension Methods

```rust
parse_extend()               // extend [T] { @map ... }
```

#### generics.rs — Generic Parameters

```rust
parse_generics()             // <T, U: Bound>, <T = Self>, <$N: int = 10>
parse_bounds()               // Eq + Clone + Printable
parse_where_clauses()        // where T: Clone, U: Default
parse_uses_clause()          // uses Http, FileSystem
```

#### config.rs — Config Variables

```rust
parse_const()                // $TIMEOUT = 30s
```

### ty.rs — Type Annotations

```rust
parse_type()                 // int, str, bool, void, never

// Compound types
parse_list_type()            // [int]
parse_map_type()             // {str: int}
parse_tuple_type()           // (int, str)

// Named types
parse_named_type()           // Result<T, E>, Option<T>
parse_generic_args()         // <T, U>

// Function types
parse_function_type()        // (int, int) -> int

// Special types
parse_self_type()            // Self
parse_associated_type()      // T.Assoc
parse_infer_type()           // _ (infer)
```

### attr.rs — Attributes

```rust
parse_attributes()           // All attributes before a declaration
parse_derive()               // #derive(Eq, Clone)
parse_test_attr()            // #test
parse_skip()                 // #skip("reason")
parse_compile_fail()         // #compile_fail("error")
```

`ParsedAttrs` collects all parsed attributes into a struct:

```rust
pub struct ParsedAttrs {
    pub derives: Vec<Name>,
    pub test_target: Option<TestTarget>,
    pub skip_reason: Option<String>,
    pub compile_fail: Option<String>,
    // ...
}
```

## Cross-Module Dependencies

```
         lib.rs (entry: parse_module)
            │
            ▼
     ┌──────┴──────┐
     │             │
  item/         expr/
     │             │
     ├─────────────┤
     │             │
   ty.rs     patterns.rs
     │             │
     └──────┬──────┘
            │
         attr.rs
```

- `lib.rs` calls `item/` for top-level declarations
- `item/` calls `expr/` for function bodies
- `item/` calls `ty.rs` for type annotations
- `expr/` calls `patterns.rs` for match arms and binding patterns
- All modules call `attr.rs` for attributes

## Naming Conventions

### Parse Functions

```rust
parse_X()                    // Parse X, return Result<X, ParseError>
parse_X_with_outcome()       // Parse X, return ParseOutcome<X>
```

### Cursor Methods

```rust
check(&kind)                 // Test current token without consuming
check_ident()                // Test if current is identifier
peek_next_kind()             // Look ahead one token
advance()                    // Consume and return current token
expect(&kind)                // Consume specific token or error
expect_ident()               // Consume identifier or error
skip_newlines()              // Skip newline tokens
```

### Context Methods

```rust
with_context(flags, f)       // Add context flags, run f, restore
without_context(flags, f)    // Remove context flags, run f, restore
allows_struct_lit()           // Check NO_STRUCT_LIT flag
in_error_context(ctx, f)     // Wrap errors with "while parsing X"
```

## Series Combinator

The `SeriesConfig` type provides a reusable combinator for parsing delimiter-separated lists (inspired by Gleam's `series_of()`):

```rust
pub struct SeriesConfig {
    pub separator: TokenKind,        // Usually Comma
    pub terminator: TokenKind,       // e.g., RParen, RBracket
    pub trailing: TrailingSeparator, // Allowed, Forbidden, Required
    pub skip_newlines: bool,
    pub min_count: usize,
    pub max_count: Option<usize>,
}

pub enum TrailingSeparator {
    Allowed,    // Trailing separator accepted
    Forbidden,  // Error on trailing separator
    Required,   // Must have separator between items
}
```

Convenience methods:

| Method | Delimiters | Usage |
|--------|------------|-------|
| `paren_series()` | `(` `)` | Function arguments, tuples |
| `bracket_series()` | `[` `]` | List literals, indexing |
| `brace_series()` | `{` `}` | Struct literals, blocks |
| `angle_series()` | `<` `>` | Generic parameters |

## Compound Operator Synthesis

The lexer produces individual `>` tokens. The parser synthesizes `>>` and `>=` from adjacent tokens in expression context. See [Token Design](../03-lexer/token-design.md#lexer-parser-token-boundary) and [Pratt Parser](pratt-parser.md#compound-operator-synthesis).

## Soft Keywords

Several Ori keywords are context-sensitive ("soft keywords"). The `Cursor::soft_keyword_to_name()` method maps tokens that are keywords in some contexts to identifiers in others. For example, `print` is a soft keyword — it is treated as a keyword when followed by `(`, but as an identifier otherwise. The `match_function_exp_kind()` method in `operators.rs` similarly gates keywords like `recurse`, `parallel`, `spawn`, and `with` on the presence of a following `(`. Note that `match`, `try`, and `loop` use block syntax (`match expr { ... }`, `try { ... }`, `loop { ... }`) rather than parenthesized function-call syntax.
