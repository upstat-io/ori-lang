---
title: "Parser Overview"
description: "Ori Compiler Design — Parser Overview"
order: 400
section: "Parser"
---

# Parser Overview

The Ori parser transforms a token stream into an AST. It uses recursive descent parsing with operator precedence handling.

## Location

```
compiler/ori_parse/src/
├── lib.rs                  # Parser struct and entry point
├── error.rs                # Parse error types
├── stack.rs                # Stack safety (stacker integration)
└── grammar/
    ├── mod.rs              # Grammar module organization
    ├── expr/               # Expression parsing (~1,681 lines total)
    │   ├── mod.rs              # Entry point, parse_binary_level! macro (~247 lines)
    │   ├── operators.rs        # Operator matching helpers (~103 lines)
    │   ├── patterns.rs         # function_seq/function_exp parsing (~472 lines)
    │   ├── postfix.rs          # Call, method call, field, index (~248 lines)
    │   └── primary.rs          # Primary expressions, literals (~611 lines)
    ├── item/               # Item parsing (split into submodules)
    │   ├── mod.rs              # Re-exports
    │   ├── function.rs         # Function/test parsing (~199 lines)
    │   ├── type_decl.rs        # Type declarations
    │   ├── trait_def.rs        # Trait definitions
    │   ├── impl_def.rs         # Impl blocks
    │   ├── use_def.rs          # Import statements
    │   ├── extend.rs           # Extension definitions
    │   ├── generics.rs         # Generic parameter parsing
    │   └── config.rs           # Config variable parsing
    ├── type.rs             # Type annotation parsing
    ├── pattern.rs          # Pattern parsing
    ├── stmt.rs             # Statement parsing
    └── attr.rs             # Attribute parsing
```

The parser is a separate crate with dependencies:
- `ori_ir` - for `Token`, `TokenKind`, `Span`, `ExprArena`, etc.
- `ori_diagnostic` - for `Diagnostic`, `ErrorCode`
- `stacker` - for stack overflow protection on deeply nested expressions

## Design Goals

1. **Clear grammar structure** - One file per grammar category
2. **Error recovery** - Parse as much as possible despite errors
3. **Arena allocation** - Build flat AST in ExprArena
4. **Comprehensive spans** - Track source locations for diagnostics

## Parser Structure

```rust
pub struct Parser<'a> {
    tokens: &'a TokenList,
    pos: usize,
    arena: ExprArena,
    errors: Vec<ParseError>,
    interner: &'a Interner,
}
```

## Entry Point

```rust
pub fn parse(db: &dyn Db, tokens: TokenList) -> ParseResult {
    let mut parser = Parser::new(&tokens, db.interner());
    let module = parser.parse_module();

    ParseResult {
        module,
        arena: parser.arena,
        errors: parser.errors,
    }
}
```

## Parsing Flow

```
TokenList
    │
    │ parse_module()
    ▼
Module { functions, types, tests, imports }
    │
    │ Each item calls:
    │   - parse_function()
    │   - parse_type_def()
    │   - parse_test()
    │   - parse_import()
    ▼
ExprArena (populated during parsing)
```

## Core Methods

### Token Access

```rust
impl Parser<'_> {
    fn current(&self) -> &TokenKind {
        self.tokens.get(self.pos).unwrap_or(&TokenKind::Eof)
    }

    fn advance(&mut self) -> &TokenKind {
        let tok = self.current();
        self.pos += 1;
        tok
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.current() == kind
    }

    fn expect(&mut self, kind: TokenKind) -> Result<(), ParseError> {
        if self.check(&kind) {
            self.advance();
            Ok(())
        } else {
            Err(self.error_expected(kind))
        }
    }
}
```

### Expression Parsing

Binary operator precedence levels are generated via the `parse_binary_level!` macro, which creates a precedence chain of parsing functions. Each level calls the next-higher precedence as its "next" parser:

```rust
/// Generate a binary operator parsing function.
/// Two forms:
///   parse_binary_level! { fn_name, next_fn, matcher_fn }     — for multi-op levels
///   parse_binary_level! { fn_name, next_fn, token: T, op: O } — for single-token levels
macro_rules! parse_binary_level { ... }

// 10 precedence levels generated:
parse_binary_level! { parse_logical_or, parse_logical_and, match_logical_or_op }
parse_binary_level! { parse_logical_and, parse_bitwise_or, match_logical_and_op }
// ... through to:
parse_binary_level! { parse_multiply, parse_coalesce, match_multiplicative_op }
```

This replaces 10 hand-written functions that differed only in which operator they matched and which "next" function they called.

### Arena Allocation

```rust
impl Parser<'_> {
    fn alloc(&mut self, kind: ExprKind) -> ExprId {
        self.arena.alloc(Expr {
            kind,
            span: self.current_span(),
        })
    }
}
```

## Expression Precedence

| Prec | Operators | Associativity |
|------|-----------|---------------|
| 1 | `\|\|` | Left |
| 2 | `&&` | Left |
| 3 | `\|` | Left |
| 4 | `^` | Left |
| 5 | `&` | Left |
| 6 | `==` `!=` | Left |
| 7 | `<` `>` `<=` `>=` | Left |
| 8 | `..` `..=` | Left |
| 9 | `<<` `>>` | Left |
| 10 | `+` `-` | Left |
| 11 | `*` `/` `%` | Left |
| 12 | Unary `-` `!` `~` | Right |
| 13 | `.` `[]` `()` `?` | Left |

## Error Handling

Errors are accumulated, not fatal:

```rust
impl Parser<'_> {
    fn error(&mut self, kind: ParseErrorKind) {
        self.errors.push(ParseError {
            kind,
            span: self.current_span(),
        });
    }

    fn synchronize(&mut self) {
        // Skip tokens until we find a synchronization point
        while !self.at_end() {
            match self.current() {
                TokenKind::Let |
                TokenKind::If |
                TokenKind::For |
                TokenKind::At => return,
                _ => { self.advance(); }
            }
        }
    }
}
```

## Salsa Integration

Parsing is a Salsa query:

```rust
#[salsa::tracked]
pub fn parsed(db: &dyn Db, file: SourceFile) -> ParseResult {
    let tokens = tokens(db, file);
    parse(db, tokens)
}
```

## Related Documents

- [Recursive Descent](recursive-descent.md) - Parsing approach
- [Error Recovery](error-recovery.md) - Handling syntax errors
- [Grammar Modules](grammar-modules.md) - Module organization
