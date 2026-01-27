---
title: "Recursive Descent Parsing"
description: "Ori Compiler Design — Recursive Descent Parsing"
order: 403
section: "Parser"
---

# Recursive Descent Parsing

The Ori parser uses recursive descent, a top-down parsing technique where each grammar rule becomes a function.

## What is Recursive Descent?

In recursive descent:
- Each non-terminal in the grammar has a corresponding parse function
- Functions call each other according to grammar rules
- Lookahead determines which production to use

```
Grammar rule:        Parse function:
expr -> term '+' expr   fn parse_expr() {
                            let left = parse_term();
                            expect('+');
                            let right = parse_expr();
                        }
```

## Grammar Structure

### Module Level

```
module -> item*
item -> function | type_def | test | import | config
```

```rust
fn parse_module(&mut self) -> Module {
    let mut functions = Vec::new();
    let mut types = Vec::new();
    let mut tests = Vec::new();
    let mut imports = Vec::new();

    while !self.at_end() {
        match self.current() {
            TokenKind::At => {
                // Could be function or test
                if self.peek_test() {
                    tests.push(self.parse_test());
                } else {
                    functions.push(self.parse_function());
                }
            }
            TokenKind::Type => types.push(self.parse_type_def()),
            TokenKind::Use => imports.push(self.parse_import()),
            TokenKind::Dollar => configs.push(self.parse_config()),
            _ => self.error_unexpected(),
        }
    }

    Module { functions, types, tests, imports }
}
```

### Function Level

```
function -> attributes? 'pub'? '@' IDENT generics? params return_type? capabilities? '=' expr
```

```rust
fn parse_function(&mut self) -> Function {
    let attrs = self.parse_attributes();
    let is_pub = self.eat(TokenKind::Pub);

    self.expect(TokenKind::At);
    let name = self.parse_ident();
    let generics = self.parse_optional_generics();
    let params = self.parse_params();
    let ret_type = self.parse_optional_return_type();
    let capabilities = self.parse_optional_capabilities();

    self.expect(TokenKind::Eq);
    let body = self.parse_expr();

    Function {
        attrs, is_pub, name, generics,
        params, ret_type, capabilities, body
    }
}
```

### Expression Level

```
expr -> if_expr | for_expr | match_expr | binary_expr
binary_expr -> unary_expr (binary_op unary_expr)*
unary_expr -> '-' unary_expr | '!' unary_expr | primary_expr
primary_expr -> literal | ident | call | '(' expr ')' | ...
```

```rust
fn parse_expr(&mut self) -> ExprId {
    match self.current() {
        TokenKind::If => self.parse_if(),
        TokenKind::For => self.parse_for(),
        TokenKind::Match => self.parse_match(),
        _ => self.parse_binary_expr(),
    }
}

fn parse_binary_expr(&mut self) -> ExprId {
    self.parse_precedence(0)
}

fn parse_unary(&mut self) -> ExprId {
    match self.current() {
        TokenKind::Minus => {
            self.advance();
            let operand = self.parse_unary();
            self.alloc(ExprKind::Unary { op: UnaryOp::Neg, operand })
        }
        TokenKind::Bang => {
            self.advance();
            let operand = self.parse_unary();
            self.alloc(ExprKind::Unary { op: UnaryOp::Not, operand })
        }
        _ => self.parse_postfix(),
    }
}
```

## Operator Precedence Parsing

For binary expressions, we use Pratt parsing (precedence climbing):

```rust
fn parse_precedence(&mut self, min_prec: u8) -> ExprId {
    let mut left = self.parse_unary();

    loop {
        let (op, prec, assoc) = match self.binary_op_info() {
            Some(info) => info,
            None => break,
        };

        if prec < min_prec {
            break;
        }

        self.advance();

        // Right associativity: use same prec, left: use prec + 1
        let next_prec = if assoc == Assoc::Right { prec } else { prec + 1 };
        let right = self.parse_precedence(next_prec);

        left = self.alloc(ExprKind::Binary { left, op, right });
    }

    left
}
```

## Lookahead

Sometimes we need to look ahead to decide:

```rust
fn parse_item(&mut self) {
    match self.current() {
        TokenKind::At => {
            // @ could be function or test
            // Look ahead to see if "tests" keyword follows
            if self.lookahead_is_test() {
                self.parse_test()
            } else {
                self.parse_function()
            }
        }
        // ...
    }
}

fn lookahead_is_test(&self) -> bool {
    // @test_name tests @target ...
    // Look for "tests" keyword after @ident
    let mut pos = self.pos + 2;  // Skip @ and ident
    matches!(self.tokens.get(pos), Some(TokenKind::Tests))
}
```

## Benefits of Recursive Descent

1. **Easy to understand** - Grammar maps directly to code
2. **Good error messages** - Know context when errors occur
3. **Flexible** - Can handle context-sensitive syntax
4. **No external tools** - Pure Rust implementation

## Limitations

1. **Left recursion** - Cannot handle `A -> A b` directly
2. **Backtracking** - May need lookahead for ambiguity
3. **Repetition** - Grammar rules become nested loops

## Handling Left Recursion

Left recursion is transformed:

```
// Original (left recursive - won't work):
expr -> expr '+' term | term

// Transformed (iterative):
expr -> term ('+' term)*
```

```rust
// Transformed version
fn parse_add_expr(&mut self) -> ExprId {
    let mut left = self.parse_term();

    while self.check(&TokenKind::Plus) {
        self.advance();
        let right = self.parse_term();
        left = self.alloc(ExprKind::Binary {
            left, op: BinaryOp::Add, right
        });
    }

    left
}
```

## Ori-Specific Parsing

### Named Arguments

```rust
// name: expr
fn parse_named_arg(&mut self) -> NamedArg {
    self.expect(TokenKind::Dot);
    let name = self.parse_ident();
    self.expect(TokenKind::Colon);
    let value = self.parse_expr();

    NamedArg { name, value }
}
```

### Pattern Expressions

```rust
// map(over: items, transform: fn)
fn parse_pattern_call(&mut self, name: Name) -> ExprId {
    self.expect(TokenKind::LParen);

    let mut args = Vec::new();
    while !self.check(&TokenKind::RParen) {
        args.push(self.parse_named_arg());
        if !self.check(&TokenKind::RParen) {
            self.expect(TokenKind::Comma);
        }
    }

    self.expect(TokenKind::RParen);
    self.alloc(ExprKind::Pattern { name, args })
}
```

### Function Definitions

```rust
// @name (params) -> Type = body
fn parse_function(&mut self) -> Function {
    self.expect(TokenKind::At);
    let name = self.parse_ident();

    self.expect(TokenKind::LParen);
    let params = self.parse_params();
    self.expect(TokenKind::RParen);

    let ret_type = if self.check(&TokenKind::Arrow) {
        self.advance();
        Some(self.parse_type())
    } else {
        None
    };

    self.expect(TokenKind::Eq);
    let body = self.parse_expr();

    Function { name, params, ret_type, body }
}
```
