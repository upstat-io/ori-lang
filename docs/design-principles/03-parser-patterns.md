# Parser Patterns & Techniques

Quick-reference guide to parser design, recursive descent, and expression parsing.

---

## Parser Architecture Choices

### Recursive Descent (Recommended)
- Hand-written parser, one function per grammar rule
- Full control over error messages and recovery
- Easy to understand and debug
- Used by: Rust, Go, TypeScript, GCC, Clang
- Handles LL(k) grammars naturally

### Parser Generators
- Tools like ANTLR, Bison, yacc
- Define grammar in DSL, generate parser code
- Good for prototyping, harder to customize errors
- Often LALR(1) or LL(*) based

### Parser Combinators
- Build parser from composable functions
- Popular in functional languages (Haskell Parsec, nom in Rust)
- Elegant but can be slow without optimization
- Good for DSLs and config files

---

## Grammar Notation

### BNF (Backus-Naur Form)
```bnf
<expr> ::= <term> | <expr> "+" <term>
<term> ::= <factor> | <term> "*" <factor>
<factor> ::= <number> | "(" <expr> ")"
```

### EBNF (Extended BNF)
```ebnf
expr = term { ("+" | "-") term } ;
term = factor { ("*" | "/") factor } ;
factor = number | "(" expr ")" ;
```
- `{ }` = zero or more
- `[ ]` = optional
- `( )` = grouping
- `|` = alternative

### PEG (Parsing Expression Grammar)
```peg
expr <- term (('+' / '-') term)*
term <- factor (('*' / '/') factor)*
factor <- number / '(' expr ')'
```
- Ordered choice (first match wins)
- No ambiguity by design
- `*` = zero or more, `+` = one or more, `?` = optional

---

## Recursive Descent Structure

### Parser State
```rust
struct Parser {
    tokens: Vec<Token>,
    current: usize,        // current token index
    // or alternatively:
    lexer: Lexer,          // on-demand lexing
    current_token: Token,
    peek_token: Token,
}
```

### Core Methods
```rust
impl Parser {
    // Lookahead
    fn peek(&self) -> &Token;
    fn peek_next(&self) -> &Token;  // for 2-token lookahead
    fn check(&self, kind: TokenKind) -> bool;

    // Consume
    fn advance(&mut self) -> Token;
    fn expect(&mut self, kind: TokenKind) -> Result<Token, Error>;
    fn consume(&mut self, kind: TokenKind, msg: &str) -> Result<(), Error>;

    // Match conditionally
    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) { self.advance(); true } else { false }
    }

    // Position
    fn is_at_end(&self) -> bool;
    fn previous(&self) -> &Token;
}
```

### Go Pattern: `got` and `want`
```go
// got checks if current token matches, advances if so
func (p *parser) got(tok token) bool {
    if p.tok == tok {
        p.next()
        return true
    }
    return false
}

// want requires a token, reports error if not found
func (p *parser) want(tok token) {
    if !p.got(tok) {
        p.syntaxError("expected " + tokstring(tok))
        p.advance()
    }
}
```

---

## Grammar Rule to Function

### Rule Pattern
```
rule = A B C | D E ;
```

Translates to:
```rust
fn parse_rule(&mut self) -> Result<Node, Error> {
    if self.check(A) {
        // parse A B C variant
        let a = self.parse_a()?;
        let b = self.parse_b()?;
        let c = self.parse_c()?;
        Ok(Node::ABC(a, b, c))
    } else if self.check(D) {
        // parse D E variant
        let d = self.parse_d()?;
        let e = self.parse_e()?;
        Ok(Node::DE(d, e))
    } else {
        Err(self.error("expected A or D"))
    }
}
```

### Repetition Pattern
```
list = item { "," item } ;
```

Translates to:
```rust
fn parse_list(&mut self) -> Result<Vec<Item>, Error> {
    let mut items = vec![self.parse_item()?];
    while self.match_token(Comma) {
        items.push(self.parse_item()?);
    }
    Ok(items)
}
```

### Optional Pattern
```
optional_type = [ ":" type ] ;
```

Translates to:
```rust
fn parse_optional_type(&mut self) -> Result<Option<Type>, Error> {
    if self.match_token(Colon) {
        Ok(Some(self.parse_type()?))
    } else {
        Ok(None)
    }
}
```

---

## Expression Parsing

### The Precedence Problem
```
1 + 2 * 3   // Should be 1 + (2 * 3), not (1 + 2) * 3
a - b - c   // Should be (a - b) - c (left-associative)
a = b = c   // Should be a = (b = c) (right-associative)
```

### Precedence Climbing (Classic)
One function per precedence level:
```rust
fn parse_expr(&mut self) -> Expr {
    self.parse_assignment()
}

fn parse_assignment(&mut self) -> Expr {
    let left = self.parse_or();
    if self.match_token(Eq) {
        let right = self.parse_assignment();  // Right-associative
        Expr::Assign(left, right)
    } else {
        left
    }
}

fn parse_or(&mut self) -> Expr {
    let mut left = self.parse_and();
    while self.match_token(Or) {
        let right = self.parse_and();
        left = Expr::Binary(Or, left, right);
    }
    left
}

fn parse_and(&mut self) -> Expr {
    let mut left = self.parse_equality();
    while self.match_token(And) {
        let right = self.parse_equality();
        left = Expr::Binary(And, left, right);
    }
    left
}

// ... continue for each level ...

fn parse_unary(&mut self) -> Expr {
    if self.match_token(Minus) || self.match_token(Not) {
        let op = self.previous();
        let right = self.parse_unary();  // Recursive for prefix
        Expr::Unary(op, right)
    } else {
        self.parse_primary()
    }
}

fn parse_primary(&mut self) -> Expr {
    // Literals, identifiers, grouping
}
```

### Pratt Parsing (Recommended)

#### Binding Power Concept
- Every operator has left and right binding power
- Left-associative: right power = left power + 1
- Right-associative: right power = left power

#### Core Algorithm
```rust
fn parse_expr(&mut self, min_bp: u8) -> Expr {
    // 1. Parse prefix (atom or prefix operator)
    let mut left = self.parse_prefix();

    // 2. Loop: parse infix/postfix while binding power allows
    loop {
        let op = match self.peek() {
            Some(tok) if is_infix(tok) => tok,
            _ => break,
        };

        let (l_bp, r_bp) = infix_binding_power(op);
        if l_bp < min_bp {
            break;  // Operator binds less tightly
        }

        self.advance();  // consume operator
        let right = self.parse_expr(r_bp);  // recurse with right bp
        left = Expr::Binary(op, left, right);
    }

    left
}

fn parse_prefix(&mut self) -> Expr {
    let tok = self.advance();
    match tok {
        Token::Number(n) => Expr::Lit(n),
        Token::Ident(s) => Expr::Var(s),
        Token::Minus => {
            let ((), r_bp) = prefix_binding_power(tok);
            let right = self.parse_expr(r_bp);
            Expr::Unary(Minus, right)
        }
        Token::LParen => {
            let inner = self.parse_expr(0);
            self.expect(RParen);
            inner
        }
        _ => panic!("unexpected token"),
    }
}
```

#### Binding Power Table
```rust
fn infix_binding_power(op: &Token) -> (u8, u8) {
    match op {
        // Assignment: right-associative
        Eq => (2, 1),

        // Logical or
        Or => (3, 4),

        // Logical and
        And => (5, 6),

        // Equality
        EqEq | NotEq => (7, 8),

        // Comparison
        Lt | Gt | LtEq | GtEq => (9, 10),

        // Additive
        Plus | Minus => (11, 12),

        // Multiplicative
        Star | Slash | Percent => (13, 14),

        // Exponentiation (right-associative)
        Caret => (16, 15),

        _ => panic!("not an infix operator"),
    }
}

fn prefix_binding_power(op: &Token) -> ((), u8) {
    match op {
        Minus | Not => ((), 17),  // High precedence
        _ => panic!("not a prefix operator"),
    }
}

fn postfix_binding_power(op: &Token) -> (u8, ()) {
    match op {
        LParen => (19, ()),  // Function call
        LBracket => (19, ()), // Index
        Dot => (19, ()),     // Member access
        _ => panic!("not a postfix operator"),
    }
}
```

#### Pratt with Postfix
```rust
fn parse_expr(&mut self, min_bp: u8) -> Expr {
    let mut left = self.parse_prefix();

    loop {
        // Try postfix first (highest precedence)
        if let Some(op) = self.peek_postfix() {
            let (l_bp, ()) = postfix_binding_power(op);
            if l_bp < min_bp { break; }
            left = self.parse_postfix(left);
            continue;
        }

        // Then infix
        if let Some(op) = self.peek_infix() {
            let (l_bp, r_bp) = infix_binding_power(op);
            if l_bp < min_bp { break; }
            self.advance();
            let right = self.parse_expr(r_bp);
            left = Expr::Binary(op, left, right);
            continue;
        }

        break;
    }

    left
}
```

---

## AST Design Patterns

### Node Base
```rust
struct Span {
    start: u32,
    end: u32,
}

trait AstNode {
    fn span(&self) -> Span;
}
```

### Expression AST
```rust
enum Expr {
    // Literals
    IntLit(i64),
    FloatLit(f64),
    StringLit(String),
    BoolLit(bool),

    // Names
    Ident(String),

    // Operations
    Binary { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    Unary { op: UnaryOp, operand: Box<Expr> },

    // Access
    Index { base: Box<Expr>, index: Box<Expr> },
    Field { base: Box<Expr>, field: String },
    Call { callee: Box<Expr>, args: Vec<Expr> },

    // Control
    If { cond: Box<Expr>, then: Box<Expr>, else_: Option<Box<Expr>> },
    Block(Vec<Stmt>),

    // Grouping (for error messages)
    Paren(Box<Expr>),
}
```

### Statement AST
```rust
enum Stmt {
    Let { name: String, ty: Option<Type>, init: Option<Expr> },
    Expr(Expr),
    Return(Option<Expr>),
    While { cond: Expr, body: Block },
    For { var: String, iter: Expr, body: Block },
    Block(Vec<Stmt>),
}
```

### With Spans
```rust
struct Spanned<T> {
    node: T,
    span: Span,
}

type SpannedExpr = Spanned<Expr>;
```

### Builder Pattern
```rust
impl Parser {
    fn mk_binary(&self, op: BinOp, left: Expr, right: Expr) -> Expr {
        let span = left.span().to(right.span());
        Expr::Binary { op, left: Box::new(left), right: Box::new(right), span }
    }
}
```

---

## Operator Precedence & Associativity

### Common Precedence Levels (Low to High)
| Level | Operators | Associativity |
|-------|-----------|---------------|
| 1 | `=`, `+=`, `-=` | Right |
| 2 | `?:` (ternary) | Right |
| 3 | `\|\|` | Left |
| 4 | `&&` | Left |
| 5 | `\|` (bitwise) | Left |
| 6 | `^` (xor) | Left |
| 7 | `&` (bitwise) | Left |
| 8 | `==`, `!=` | Left |
| 9 | `<`, `>`, `<=`, `>=` | Left |
| 10 | `<<`, `>>` | Left |
| 11 | `+`, `-` | Left |
| 12 | `*`, `/`, `%` | Left |
| 13 | `**` (power) | Right |
| 14 | `!`, `-` (unary), `&`, `*` | Prefix |
| 15 | `.`, `()`, `[]` | Postfix |

### Implementing Associativity
- **Left-associative**: In Pratt, right_bp = left_bp + 1
- **Right-associative**: In Pratt, right_bp = left_bp (or left_bp - 1)
- **Non-associative**: Error if same operator appears consecutively

---

## Context-Sensitive Parsing

### Keywords as Identifiers
```rust
fn parse_ident(&mut self) -> Result<String, Error> {
    match &self.peek().kind {
        TokenKind::Ident(s) => {
            self.advance();
            Ok(s.clone())
        }
        // Allow contextual keywords as identifiers
        TokenKind::Keyword(kw) if is_contextual_keyword(kw) => {
            self.advance();
            Ok(kw.to_string())
        }
        _ => Err(self.error("expected identifier")),
    }
}
```

### Ambiguity Resolution (Go Example)
```go
// Is `{` a composite literal or block statement?
// Go tracks expression nesting level
if p.xnest >= 0 {
    // Inside expression context, `{` starts composite literal
    complit_ok = true
}
// At statement level, `{` starts block
```

### Lookahead for Disambiguation
```rust
// Is this `<` a comparison or generic parameter?
fn parse_expr_or_type(&mut self) -> Either<Expr, Type> {
    // Look ahead to see what follows `<`
    if self.peek() == Lt && self.lookahead_is_type_params() {
        Either::Right(self.parse_type())
    } else {
        Either::Left(self.parse_expr())
    }
}
```

---

## Error Recovery Strategies

### Panic Mode
1. On error, set "panic" flag
2. Skip tokens until synchronization point
3. Synchronization points: semicolons, keywords (`fn`, `if`, `}`)
4. Clear panic flag, continue parsing

```rust
fn synchronize(&mut self) {
    self.advance();
    while !self.is_at_end() {
        if self.previous().kind == Semicolon {
            return;
        }
        match self.peek().kind {
            Fn | Let | If | While | Return => return,
            _ => self.advance(),
        }
    }
}
```

### Follow Set Recovery (Go Pattern)
```go
// stopset: tokens that start statements (good sync points)
const stopset uint64 = 1<<_Break | 1<<_Const | 1<<_Continue |
    1<<_For | 1<<_If | 1<<_Return | 1<<_Switch | 1<<_Type | 1<<_Var

func (p *parser) advance(followlist ...token) {
    // Skip until we find a follow token or stopset token
    for !contains(followset|stopset, p.tok) {
        p.next()
    }
}
```

### Error Productions
Add grammar rules that match common errors:
```rust
fn parse_if(&mut self) -> Result<Stmt, Error> {
    self.expect(If)?;

    // Error production: missing parens around condition
    if self.check(LParen) {
        self.advance();  // consume (
        let cond = self.parse_expr()?;
        if !self.match_token(RParen) {
            self.error("missing ')' after condition");
        }
    } else {
        // Expected: no parens (Rust/Go style)
        let cond = self.parse_expr()?;
    }
    // ...
}
```

### Error Accumulation
```rust
struct Parser {
    errors: Vec<ParseError>,
    had_error: bool,
}

impl Parser {
    fn error(&mut self, msg: &str) {
        self.errors.push(ParseError {
            message: msg.to_string(),
            span: self.current_span(),
        });
        self.had_error = true;
    }

    fn parse(&mut self) -> (Option<Ast>, Vec<ParseError>) {
        let ast = self.parse_program();
        (if self.had_error { None } else { Some(ast) }, self.errors)
    }
}
```

---

## Left Recursion Elimination

### Problem
```
expr = expr "+" term | term ;  // Left-recursive!
```

Recursive descent can't handle this - infinite recursion.

### Solution: Convert to Iteration
```
expr = term { "+" term } ;  // Right-recursive/iterative
```

### Pratt Parsing Advantage
- Naturally handles left-recursion through iteration
- The `while` loop replaces recursion for left-associative operators

---

## Lookahead Management

### LL(1) - One Token Lookahead
- Most grammars can be parsed with single lookahead
- `peek()` to see next token
- Sufficient for most statements and expressions

### LL(2) - Two Token Lookahead
Needed for:
- `a.b` vs `a.0` (field vs tuple index)
- `..` vs `.` (range vs member access)
- `<T>` vs `<` (generics vs comparison)

```rust
fn peek_next(&self) -> &Token {
    self.tokens.get(self.current + 1)
        .unwrap_or(&Token::Eof)
}
```

### Unlimited Lookahead
- Save position, try parsing, restore on failure
- "Backtracking" or "speculative parsing"
- Use sparingly - expensive

```rust
fn try_parse<T>(&mut self, f: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
    let checkpoint = self.current;
    match f(self) {
        Some(result) => Some(result),
        None => {
            self.current = checkpoint;  // Restore
            None
        }
    }
}
```

---

## Performance Tips

### Avoid Excessive Allocation
- Use arenas for AST nodes
- Intern identifiers
- Box large variants, not small ones

### Minimize Lookahead
- Design grammar for LL(1) where possible
- Context can reduce lookahead needs

### Pre-allocate Token Vector
```rust
fn tokenize_all(lexer: &mut Lexer) -> Vec<Token> {
    let mut tokens = Vec::with_capacity(estimated_size);
    while let Some(tok) = lexer.next() {
        tokens.push(tok);
    }
    tokens
}
```

### On-Demand Lexing
```rust
// Alternative: lex one token at a time
struct Parser {
    lexer: Lexer,
    current: Token,
    peeked: Option<Token>,
}

fn advance(&mut self) -> Token {
    let current = mem::replace(&mut self.current,
        self.peeked.take().unwrap_or_else(|| self.lexer.next()));
    current
}
```

---

## Real-World Examples

### Rust (`rustc_parse/src/parser/`)
- Recursive descent with Pratt for expressions
- `parse_expr_assoc_with(min_prec, ...)` - Pratt core
- Rich error recovery with suggestions
- Modular: separate files for expr, stmt, item, ty, pat

### Go (`cmd/compile/internal/syntax/parser.go`)
- Clean recursive descent
- `xnest` for expression context tracking
- Automatic semicolon handling
- Grammar annotations in comments

### TypeScript
- Hand-written recursive descent
- Extensive lookahead for complex grammar
- Incremental reparsing for IDE performance

---

## Parser Checklist

### Infrastructure
- [ ] Token stream or on-demand lexing
- [ ] Peek/advance/expect methods
- [ ] Position tracking for errors
- [ ] Error recovery strategy

### Grammar Coverage
- [ ] Expressions with correct precedence
- [ ] Statements (let, if, while, return, etc.)
- [ ] Declarations (fn, struct, enum, etc.)
- [ ] Types
- [ ] Patterns (if applicable)

### Expression Parsing
- [ ] All binary operators with precedence
- [ ] Unary prefix operators
- [ ] Postfix operators (call, index, field)
- [ ] Grouping with parentheses
- [ ] Associativity correct

### Error Handling
- [ ] Clear error messages with locations
- [ ] Error recovery (don't stop at first error)
- [ ] Synchronization points defined
- [ ] Common mistakes handled gracefully

---

## Key References
- Crafting Interpreters (Parsing): https://craftinginterpreters.com/parsing-expressions.html
- Pratt Parsing: https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html
- Go parser: `src/cmd/compile/internal/syntax/parser.go`
- Rust parser: `compiler/rustc_parse/src/parser/expr.rs`
