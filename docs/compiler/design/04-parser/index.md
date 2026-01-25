# Parser Overview

The Sigil parser transforms a token stream into an AST. It uses recursive descent parsing with operator precedence handling.

## Location

```
compiler/sigil_parse/src/
├── lib.rs              # Parser struct and entry point
├── error.rs            # Parse error types
├── stack.rs            # Stack safety (stacker integration)
└── grammar/
    ├── mod.rs          # Grammar module organization
    ├── expr/           # Expression parsing (~1,436 lines total)
    │   ├── mod.rs          # Entry point, binary precedence chain (~271 lines)
    │   ├── operators.rs    # Operator matching helpers (~95 lines)
    │   ├── patterns.rs     # function_seq/function_exp parsing (~444 lines)
    │   ├── postfix.rs      # Call, method call, field, index (~167 lines)
    │   └── primary.rs      # Primary expressions, literals (~459 lines)
    ├── item.rs         # Function/type/test parsing (~446 lines)
    ├── type.rs         # Type annotation parsing
    ├── pattern.rs      # Pattern parsing
    ├── stmt.rs         # Statement parsing
    └── attr.rs         # Attribute parsing
```

The parser is a separate crate with dependencies:
- `sigil_ir` - for `Token`, `TokenKind`, `Span`, `ExprArena`, etc.
- `sigil_diagnostic` - for `Diagnostic`, `ErrorCode`
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

```rust
impl Parser<'_> {
    fn parse_expr(&mut self) -> ExprId {
        self.parse_expr_precedence(0)
    }

    fn parse_expr_precedence(&mut self, min_prec: u8) -> ExprId {
        let mut left = self.parse_unary();

        while let Some((op, prec)) = self.binary_op() {
            if prec < min_prec {
                break;
            }
            self.advance();
            let right = self.parse_expr_precedence(prec + 1);
            left = self.arena.alloc(Expr::binary(left, op, right));
        }

        left
    }
}
```

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
