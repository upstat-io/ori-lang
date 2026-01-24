# Proposal: function_seq and function_exp Pattern Distinction

**Status:** Approved
**Author:** Eric
**Created:** 2026-01-22
**Approved:** 2026-01-22

---

## Summary

Two changes:

### 1. Pattern Categorization

Formalize the distinction between two categories of built-in patterns:

- **function_seq** — Contains a sequence of expressions (`run`, `try`, `match`)
- **function_exp** — Contains named expressions (`map`, `filter`, `fold`, `parallel`, etc.)

These are distinct constructs, not function calls. The compiler should treat them as separate AST node types with different parsing, validation, and codegen paths.

### 2. Remove Positional Arguments from Function Calls

User-defined function calls must use named arguments for functions with multiple parameters:

```sigil
// Before (positional allowed)
add(1, 2)
assert_eq(result, expected)

// After (named required for multi-param)
add(.a: 1, .b: 2)
assert_eq(actual: result, expected: expected)

// Single-param functions remain simple
print("hello")
len(items)
```

Only **function_seq** may contain positional/sequential expressions. Function calls and function_exp require named syntax.

---

## Motivation

### Clarity of Semantics

Currently, all patterns are loosely grouped together. But they have fundamentally different internal structures:

```sigil
// function_seq: sequence of expressions, order is meaning
run(
    let x = step1(),
    let y = step2(x),
    x + y,
)

// function_exp: named expressions, names are meaning
fold(
    over: items,
    init: 0,
    op: +,
)
```

These are not "function calls with different argument styles." They are different constructs entirely.

### No "Positional vs Named" Ambiguity

The current mental model creates confusion:
- "Patterns use named arguments"
- "But `run` and `try` use positional..."
- "Is that allowed? Is it inconsistent?"

The new model eliminates this:
- **function_seq**: has a sequence (not parameters) — positional expressions allowed
- **function_exp**: has named expressions (not parameters) — `.name:` required
- **function call**: has arguments — named required for multi-param functions

Three distinct constructs. Clear rules for each.

### Better Error Messages

With explicit categorization, the compiler can give precise errors:

```
// function_exp error
error: `map` missing required property `over:`
  --> src/main.si:10:5
   |
10 |     map(
11 |         transform: x -> x * 2,
12 |     )
   |     ^ missing `over:`
   |
   = help: add the required property:
     map(
         over: <collection>,
         transform: x -> x * 2,
     )

// function_seq error
error: `try` body must end with Result or Option expression
  --> src/main.si:15:5
   |
15 |     try(
16 |         let x = fetch(),
17 |         x + 1,
   |         ^^^^^ expected Result<_, _> or Option<_>
18 |     )
   |
   = help: wrap the final expression:
     try(
         let x = fetch(),
         Ok(x + 1),
     )

// function call error (positional not allowed)
error: function `add` requires named arguments
  --> src/main.si:20:5
   |
20 |     add(1, 2)
   |     ^^^^^^^^^ positional arguments not allowed
   |
   = help: use named arguments:
     add(
         .a: 1,
         .b: 2,
     )
```

### Why Remove Positional Arguments?

#### For AI

1. **Line-oriented edits** — Each argument is a separate line. Add, remove, or modify without touching other lines.

2. **No signature lookup** — AI doesn't need to trace callers or read docs to understand parameter order.

3. **Reduced context** — Property names convey meaning immediately.

#### For Humans

1. **Zero ambiguity** — `assert_eq(actual: x, expected: y)` is unambiguous. `assert_eq(x, y)` requires knowing the signature.

2. **Self-documenting** — Code explains itself at the call site.

3. **Faster scanning** — Vertical layout with `.name:` prefixes creates a scannable structure.

#### The Rule

| Construct | Positional Allowed? |
|-----------|-------------------|
| function_seq (`run`, `try`, `match`) | Yes — they contain sequences |
| function_exp (`map`, `fold`, etc.) | No — `.name:` required |
| Function call (1 param) | Yes — no ambiguity |
| Function call (2+ params) | No — `.name:` required |

### Cleaner Compiler Architecture

Separate AST nodes enable:
- Different parsing logic
- Different type checking rules
- Different codegen strategies
- Simpler pattern matching in compiler code

---

## Design

### AST Changes

#### Before

```rust
enum Expr {
    // Patterns lumped together or scattered
    Pattern { name: String, args: Vec<PatternArg> },
    // or mixed into other variants...
}
```

#### After

```rust
enum Expr {
    // User function calls (named args required for multi-param)
    Call {
        callee: Ident,
        args: Vec<NamedArg>,  // Always named (single-param may omit name)
    },

    // function_seq: sequential expression constructs
    FunctionSeq(FunctionSeq),

    // function_exp: named expression constructs
    FunctionExp(FunctionExp),

    // ...other variants
}

struct NamedArg {
    name: Option<Ident>,  // None only for single-param calls
    value: Box<Expr>,
    span: Span,
}

enum FunctionSeq {
    Run {
        bindings: Vec<Binding>,
        result: Box<Expr>,
    },
    Try {
        bindings: Vec<Binding>,
        result: Box<Expr>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
}

struct FunctionExp {
    kind: FunctionExpKind,
    props: Vec<NamedExpr>,
    span: Span,
}

enum FunctionExpKind {
    Map,
    Filter,
    Fold,
    Collect,
    Find,
    Recurse,
    Parallel,
    Timeout,
    Retry,
    Cache,
    Validate,
    With,
}

struct NamedExpr {
    name: Ident,      // e.g., "over", "transform"
    value: Box<Expr>,
    span: Span,
}

struct Binding {
    mutable: bool,
    name: Ident,
    type_ann: Option<Type>,
    value: Box<Expr>,
    span: Span,
}
```

### Lexer Changes

No changes required. The keywords `run`, `try`, `match`, `map`, `filter`, etc. are already recognized.

### Parser Changes

#### function_seq Parsing

```rust
fn parse_function_seq(&mut self) -> Result<FunctionSeq, ParseError> {
    match self.current().kind {
        TokenKind::Run => self.parse_run(),
        TokenKind::Try => self.parse_try(),
        TokenKind::Match => self.parse_match(),
        _ => Err(self.unexpected_token()),
    }
}

fn parse_run(&mut self) -> Result<FunctionSeq, ParseError> {
    self.expect(TokenKind::Run)?;
    self.expect(TokenKind::LParen)?;

    let mut bindings = vec![];

    // Parse bindings: let x = expr,
    while self.check(TokenKind::Let) {
        bindings.push(self.parse_binding()?);
        self.expect(TokenKind::Comma)?;
    }

    // Parse final expression
    let result = self.parse_expr()?;
    self.consume_trailing_comma();

    self.expect(TokenKind::RParen)?;

    Ok(FunctionSeq::Run { bindings, result: Box::new(result) })
}
```

#### function_exp Parsing

```rust
fn parse_function_exp(&mut self) -> Result<FunctionExp, ParseError> {
    let kind = self.parse_function_exp_kind()?;
    self.expect(TokenKind::LParen)?;

    let mut props = vec![];

    while !self.check(TokenKind::RParen) {
        // Expect .name: expr
        self.expect(TokenKind::Dot)?;
        let name = self.parse_ident()?;
        self.expect(TokenKind::Colon)?;
        let value = self.parse_expr()?;

        props.push(NamedExpr {
            name,
            value: Box::new(value),
            span: self.span(),
        });

        if !self.check(TokenKind::RParen) {
            self.expect(TokenKind::Comma)?;
        }
    }

    self.expect(TokenKind::RParen)?;

    Ok(FunctionExp { kind, props, span: self.span() })
}

fn parse_function_exp_kind(&mut self) -> Result<FunctionExpKind, ParseError> {
    match self.current().kind {
        TokenKind::Map => { self.advance(); Ok(FunctionExpKind::Map) }
        TokenKind::Filter => { self.advance(); Ok(FunctionExpKind::Filter) }
        TokenKind::Fold => { self.advance(); Ok(FunctionExpKind::Fold) }
        TokenKind::Collect => { self.advance(); Ok(FunctionExpKind::Collect) }
        TokenKind::Find => { self.advance(); Ok(FunctionExpKind::Find) }
        TokenKind::Recurse => { self.advance(); Ok(FunctionExpKind::Recurse) }
        TokenKind::Parallel => { self.advance(); Ok(FunctionExpKind::Parallel) }
        TokenKind::Timeout => { self.advance(); Ok(FunctionExpKind::Timeout) }
        TokenKind::Retry => { self.advance(); Ok(FunctionExpKind::Retry) }
        TokenKind::Cache => { self.advance(); Ok(FunctionExpKind::Cache) }
        TokenKind::Validate => { self.advance(); Ok(FunctionExpKind::Validate) }
        TokenKind::With => { self.advance(); Ok(FunctionExpKind::With) }
        _ => Err(self.unexpected_token()),
    }
}
```

#### Function Call Parsing (Named Args Required)

```rust
fn parse_call(&mut self, callee: Ident) -> Result<Expr, ParseError> {
    self.expect(TokenKind::LParen)?;

    let mut args = vec![];

    while !self.check(TokenKind::RParen) {
        let arg = if self.check(TokenKind::Dot) {
            // Named argument: .name: expr
            self.advance(); // consume .
            let name = self.parse_ident()?;
            self.expect(TokenKind::Colon)?;
            let value = self.parse_expr()?;
            NamedArg { name: Some(name), value: Box::new(value), span: self.span() }
        } else {
            // Positional argument (only allowed for single-param)
            let value = self.parse_expr()?;
            NamedArg { name: None, value: Box::new(value), span: self.span() }
        };

        args.push(arg);

        if !self.check(TokenKind::RParen) {
            self.expect(TokenKind::Comma)?;
        }
    }

    self.expect(TokenKind::RParen)?;

    // Validate: if more than one arg, all must be named
    if args.len() > 1 && args.iter().any(|a| a.name.is_none()) {
        return Err(ParseError::PositionalArgsNotAllowed {
            callee: callee.clone(),
            span: self.span(),
            help: "functions with multiple parameters require named arguments".into(),
        });
    }

    Ok(Expr::Call { callee, args })
}
```

### Type Checker Changes

#### function_seq Validation

```rust
fn check_function_seq(&mut self, seq: &FunctionSeq) -> Result<Type, TypeError> {
    match seq {
        FunctionSeq::Run { bindings, result } => {
            // Check each binding in order, adding to scope
            for binding in bindings {
                let ty = self.check_expr(&binding.value)?;
                self.scope.insert(binding.name.clone(), ty);
            }
            // Result type is type of final expression
            self.check_expr(result)
        }

        FunctionSeq::Try { bindings, result } => {
            // Similar to run, but handle ? operator
            for binding in bindings {
                let ty = self.check_expr(&binding.value)?;
                // If expr uses ?, unwrap the Result/Option
                let unwrapped = self.unwrap_propagated_type(ty)?;
                self.scope.insert(binding.name.clone(), unwrapped);
            }
            // Result must be Result<T, E> or Option<T>
            let result_ty = self.check_expr(result)?;
            self.expect_result_or_option(result_ty)
        }

        FunctionSeq::Match { scrutinee, arms } => {
            let scrutinee_ty = self.check_expr(scrutinee)?;
            self.check_match_exhaustiveness(scrutinee_ty, arms)?;
            // All arms must have same type
            self.check_match_arms(arms)
        }
    }
}
```

#### function_exp Validation

```rust
fn check_function_exp(&mut self, exp: &FunctionExp) -> Result<Type, TypeError> {
    // Get required and optional properties for this pattern
    let schema = self.get_function_exp_schema(exp.kind);

    // Check all required properties are present
    for required in &schema.required {
        if !exp.props.iter().any(|p| p.name.as_str() == *required) {
            return Err(TypeError::MissingProperty {
                pattern: exp.kind,
                property: required.to_string(),
                span: exp.span,
            });
        }
    }

    // Check no unknown properties
    for prop in &exp.props {
        if !schema.all_properties().contains(&prop.name.as_str()) {
            return Err(TypeError::UnknownProperty {
                pattern: exp.kind,
                property: prop.name.clone(),
                span: prop.span,
            });
        }
    }

    // Type-check each property
    for prop in &exp.props {
        let expected = schema.property_type(&prop.name);
        let actual = self.check_expr(&prop.value)?;
        self.unify(expected, actual, prop.span)?;
    }

    // Compute result type
    schema.result_type(&exp.props, self)
}

fn get_function_exp_schema(&self, kind: FunctionExpKind) -> FunctionExpSchema {
    match kind {
        FunctionExpKind::Map => FunctionExpSchema {
            required: vec!["over", "transform"],
            optional: vec![],
            // over: [T], transform: T -> U  =>  [U]
        },
        FunctionExpKind::Filter => FunctionExpSchema {
            required: vec!["over", "predicate"],
            optional: vec![],
            // over: [T], predicate: T -> bool  =>  [T]
        },
        FunctionExpKind::Fold => FunctionExpSchema {
            required: vec!["over", "init", "op"],
            optional: vec![],
            // over: [T], init: U, op: (U, T) -> U  =>  U
        },
        // ... etc for each pattern
    }
}
```

#### Function Call Validation

```rust
fn check_call(&mut self, callee: &Ident, args: &[NamedArg]) -> Result<Type, TypeError> {
    let func_ty = self.lookup_function(callee)?;
    let params = func_ty.params();

    // Check arity
    if args.len() != params.len() {
        return Err(TypeError::ArityMismatch {
            expected: params.len(),
            found: args.len(),
            span: callee.span,
        });
    }

    // For single-param, positional is allowed (name can be None)
    // For multi-param, all names must be present (checked in parser)

    // Match named args to params
    for arg in args {
        let param = if let Some(name) = &arg.name {
            // Find param by name
            params.iter()
                .find(|p| p.name == *name)
                .ok_or_else(|| TypeError::UnknownParameter {
                    function: callee.clone(),
                    parameter: name.clone(),
                    span: arg.span,
                })?
        } else {
            // Single positional arg -> first param
            &params[0]
        };

        let actual = self.check_expr(&arg.value)?;
        self.unify(param.ty.clone(), actual, arg.span)?;
    }

    Ok(func_ty.return_type())
}
```

### Codegen Changes

#### function_seq Codegen

```rust
fn codegen_function_seq(&mut self, seq: &FunctionSeq) -> CExpr {
    match seq {
        FunctionSeq::Run { bindings, result } => {
            // Generate a block with variable declarations
            let mut stmts = vec![];
            for binding in bindings {
                stmts.push(self.codegen_let(binding));
            }
            stmts.push(CStmt::Return(self.codegen_expr(result)));
            CExpr::Block(stmts)
        }

        FunctionSeq::Try { bindings, result } => {
            // Generate early-return checks after each binding
            let mut stmts = vec![];
            for binding in bindings {
                stmts.push(self.codegen_let(binding));
                // If binding used ?, generate: if (is_err(x)) return x;
                if binding.has_propagation {
                    stmts.push(self.codegen_err_check(&binding.name));
                }
            }
            stmts.push(CStmt::Return(self.codegen_expr(result)));
            CExpr::Block(stmts)
        }

        FunctionSeq::Match { scrutinee, arms } => {
            // Generate switch or if-else chain
            self.codegen_match(scrutinee, arms)
        }
    }
}
```

#### function_exp Codegen

```rust
fn codegen_function_exp(&mut self, exp: &FunctionExp) -> CExpr {
    match exp.kind {
        FunctionExpKind::Map => {
            let over = self.get_prop(&exp.props, "over");
            let transform = self.get_prop(&exp.props, "transform");
            self.codegen_map_loop(over, transform)
        }

        FunctionExpKind::Filter => {
            let over = self.get_prop(&exp.props, "over");
            let predicate = self.get_prop(&exp.props, "predicate");
            self.codegen_filter_loop(over, predicate)
        }

        FunctionExpKind::Fold => {
            let over = self.get_prop(&exp.props, "over");
            let init = self.get_prop(&exp.props, "init");
            let op = self.get_prop(&exp.props, "op");
            self.codegen_fold_loop(over, init, op)
        }

        FunctionExpKind::Parallel => {
            // Generate thread spawning / async code
            self.codegen_parallel(&exp.props)
        }

        // ... etc
    }
}
```

### Formatter Changes

The formatter already handles these differently, but should be explicit:

```rust
fn format_expr(&mut self, expr: &Expr) {
    match expr {
        Expr::FunctionSeq(seq) => self.format_function_seq(seq),
        Expr::FunctionExp(exp) => self.format_function_exp(exp),
        Expr::Call { callee, args } => self.format_call(callee, args),
        // ...
    }
}

fn format_function_seq(&mut self, seq: &FunctionSeq) {
    // Sequence expressions inline within the parens
    match seq {
        FunctionSeq::Run { bindings, result } => {
            self.write("run(");
            self.indent();
            for binding in bindings {
                self.newline();
                self.format_binding(binding);
                self.write(",");
            }
            self.newline();
            self.format_expr(result);
            self.write(",");
            self.dedent();
            self.newline();
            self.write(")");
        }
        // ...
    }
}

fn format_function_exp(&mut self, exp: &FunctionExp) {
    // Named expressions always stacked, one per line
    self.write(&exp.kind.to_string());
    self.write("(");
    self.indent();
    for prop in &exp.props {
        self.newline();
        self.write(".");
        self.write(&prop.name);
        self.write(": ");
        self.format_expr(&prop.value);
        self.write(",");
    }
    self.dedent();
    self.newline();
    self.write(")");
}

fn format_call(&mut self, callee: &Ident, args: &[NamedArg]) {
    self.write(&callee.name);
    self.write("(");

    if args.len() == 1 && args[0].name.is_none() {
        // Single positional arg: inline
        self.format_expr(&args[0].value);
    } else {
        // Named args: stacked, one per line
        self.indent();
        for arg in args {
            self.newline();
            self.write(".");
            self.write(&arg.name.as_ref().unwrap().name);
            self.write(": ");
            self.format_expr(&arg.value);
            self.write(",");
        }
        self.dedent();
        self.newline();
    }

    self.write(")");
}
```

### LSP Changes

```rust
fn get_completions(&self, position: Position) -> Vec<Completion> {
    let context = self.get_context(position);

    match context {
        // Inside function_exp, suggest properties
        Context::InsideFunctionExp { kind, existing_props } => {
            let schema = get_function_exp_schema(kind);
            schema.all_properties()
                .filter(|p| !existing_props.contains(p))
                .map(|p| Completion {
                    label: format!(".{}:", p),
                    kind: CompletionKind::Property,
                    detail: schema.property_doc(p),
                })
                .collect()
        }

        // Inside function_seq, suggest based on context
        Context::InsideFunctionSeq { kind } => {
            match kind {
                SeqKind::Run | SeqKind::Try => vec![
                    Completion { label: "let ".into(), kind: CompletionKind::Keyword, .. }
                ],
                SeqKind::Match => vec![
                    Completion { label: "_ -> ".into(), kind: CompletionKind::Snippet, .. }
                ],
            }
        }

        // At expression position, suggest both
        Context::Expression => {
            let mut completions = vec![];
            completions.extend(self.function_seq_completions());
            completions.extend(self.function_exp_completions());
            completions
        }

        _ => vec![],
    }
}
```

---

## Grammar Summary

```ebnf
pattern_expr       = function_seq | function_exp .

function_seq       = run_expr | try_expr | match_expr .
run_expr           = "run" "(" seq_body ")" .
try_expr           = "try" "(" seq_body ")" .
match_expr         = "match" "(" expr "," match_arms ")" .
seq_body           = { binding "," } expr [ "," ] .

function_exp       = exp_name "(" named_exp_list ")" .
exp_name           = "map" | "filter" | "fold" | "collect" | "find"
                   | "recurse" | "parallel" | "timeout" | "retry"
                   | "cache" | "validate" | "with" .
named_exp_list     = named_exp { "," named_exp } [ "," ] .
named_exp          = "." identifier ":" expr .

call_expr          = identifier "(" call_args ")" .
call_args          = call_arg { "," call_arg } [ "," ] .
call_arg           = expr                           (* single-param only *)
                   | "." identifier ":" expr .      (* named, required for multi-param *)
```

---

## Migration

This includes both internal refactoring and a syntax change for function calls.

### Syntax Change: Named Arguments Required

Functions with multiple parameters now require named arguments:

```sigil
// Before
add(1, 2)
assert_eq(result, 42)
compare(a, b)

// After
add(.a: 1, .b: 2)
assert_eq(actual: result, expected: 42)
compare(left: a, right: b)
```

Single-parameter functions remain unchanged:

```sigil
print("hello")      // OK - single param
len(items)          // OK - single param
str(42)             // OK - single param
```

### Migration Tool

`sigil fmt --migrate` will automatically convert positional calls to named:

```bash
$ sigil fmt --migrate src/
Migrated 47 function calls to named arguments:
  - src/main.si: 12 calls
  - src/utils.si: 35 calls
```

The tool uses function signatures to determine parameter names.

### Compiler Changes

1. Add new AST types (`FunctionSeq`, `FunctionExp`)
2. Update parser to produce new AST
3. Update type checker to handle new AST
4. Update codegen to handle new AST
5. Update formatter to handle new AST
6. Update LSP to handle new AST

### Testing

- All existing tests should continue to pass
- Add unit tests for each `FunctionSeq` variant parsing
- Add unit tests for each `FunctionExp` variant parsing
- Add error message tests for missing/unknown properties
- Add formatter tests for both categories

---

## Summary

| Category | Constructs | Internal Structure | Positional Allowed? |
|----------|------------|-------------------|---------------------|
| **function_seq** | `run`, `try`, `match` | Sequence of expressions | Yes (it's a sequence) |
| **function_exp** | `map`, `filter`, `fold`, etc. | Named expressions | No (`.name:` required) |
| **function call** (1 param) | User functions | Single argument | Yes |
| **function call** (2+ params) | User functions | Named arguments | No (`.name:` required) |

This formalization:
1. Eliminates "positional vs named" confusion
2. Enables precise error messages
3. Simplifies compiler architecture
4. Aligns implementation with language semantics
5. Makes all multi-argument calls self-documenting
6. Enables safe line-oriented editing for AI
