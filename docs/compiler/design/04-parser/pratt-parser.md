---
title: "Pratt Parser"
description: "Ori Compiler Design — Pratt Parsing for Operator Precedence"
order: 403
section: "Parser"
---

# Pratt Parsing

## What Is Operator Precedence Parsing?

Every programming language with infix operators faces a fundamental parsing problem: how does `a + b * c` become a tree? The answer depends on **operator precedence** — the convention that multiplication binds tighter than addition, so the expression parses as `a + (b * c)` rather than `(a + b) * c`. A language with 16 precedence levels needs some mechanism to encode this hierarchy and apply it during parsing.

This is a problem that only arises with infix operators. Prefix notation (`+ a (* b c)`) and postfix notation (`a b c * +`) are unambiguous by construction — the operator's position uniquely determines its operands. Infix notation is ambiguous without precedence and associativity rules, and handling these rules efficiently is one of the central challenges of expression parsing.

### The Recursive Descent Approach

The textbook solution for recursive descent parsers is a chain of mutually recursive functions, one per precedence level:

```text
parse_expression   → parse_assignment
parse_assignment   → parse_or
parse_or           → parse_and
parse_and          → parse_equality
parse_equality     → parse_comparison
parse_comparison   → parse_range
parse_range        → parse_shift
parse_shift        → parse_additive
parse_additive     → parse_multiplicative
parse_multiplicative → parse_unary
parse_unary        → parse_postfix
parse_postfix      → parse_primary
```

Each function parses its level's operators, then calls the next-higher-precedence function for operands. `parse_additive()` looks for `+` and `-`, calling `parse_multiplicative()` for each operand. `parse_multiplicative()` looks for `*`, `/`, and `%`, calling `parse_unary()` for each operand.

This works, but it has a cost: parsing a simple expression like `42` requires descending through every level of the chain — 12+ function calls just to reach `parse_primary()` and discover there are no operators at all. For expression-heavy languages, this overhead is measurable.

### Pratt's Insight

In 1973, Vaughan Pratt published ["Top Down Operator Precedence"](https://dl.acm.org/doi/10.1145/512927.512931), which offered a different way to think about the problem. Instead of encoding precedence in the call stack (one function per level), Pratt encoded it in data: each operator carries a **binding power** number, and a single loop uses these numbers to decide grouping.

The core observation is that operator precedence is really about competition between adjacent operators for a shared operand. In `a + b * c`, the `+` and `*` both want `b` as an operand. The higher-precedence operator (`*`) wins, so `b` groups with `*`. Pratt formalized this with left and right binding powers: an operator captures an operand on its right if its right binding power exceeds the minimum threshold set by the operator on its left.

This idea lay dormant for decades — most compiler textbooks continued teaching the recursive descent chain — until a series of influential blog posts revived interest. Bob Nystrom's ["Pratt Parsers: Expression Parsing Made Easy"](https://journal.stuffwithstuff.com/2011/03/19/pratt-parsers-expression-parsing-made-easy/) (2011) and Matklad's ["Simple but Powerful Pratt Parsing"](https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html) (2020) brought Pratt parsing into mainstream adoption for hand-written parsers.

### Related Algorithms

Several algorithms solve the same problem as Pratt parsing:

**Precedence climbing** (Martin Richards, 1979) is essentially the same algorithm described differently — a single function with a minimum precedence parameter. The distinction between Pratt parsing and precedence climbing is largely one of presentation. Pratt's formulation uses "null denotation" (prefix) and "left denotation" (infix) functions per token type; precedence climbing uses a loop with a minimum precedence parameter. Ori's implementation is closer to precedence climbing in structure but uses Pratt's binding power terminology.

**Shunting-yard** (Edsger Dijkstra, 1961) uses an explicit operator stack to convert infix to postfix notation. It handles the same class of grammars but produces a different output (postfix stream or tree) and is typically used in calculators rather than compiler parsers. The explicit stack makes it harder to integrate with recursive descent for non-expression constructs.

## How Pratt Parsing Works

### The Algorithm

The Pratt loop is remarkably simple. It takes a minimum binding power parameter and loops:

1. Parse a prefix expression (literal, identifier, unary operator, parenthesized expression)
2. Check the next token — is it an infix operator?
3. If the operator's left binding power is less than the minimum, stop — the operator belongs to a caller
4. Otherwise, consume the operator and recursively parse the right operand with the operator's right binding power as the new minimum
5. Build a binary node from the left operand, operator, and right operand
6. Loop back to step 2

```rust
fn parse_binary_pratt(&mut self, min_bp: u8) -> Result<ExprId, ParseError> {
    let mut left = self.parse_unary()?;

    loop {
        self.skip_newlines();

        // Range operators: non-associative, optional end, special handling
        if !parsed_range && min_bp <= bp::RANGE
            && matches!(self.current_kind(), TokenKind::DotDot | TokenKind::DotDotEq)
        {
            left = self.parse_range_continuation(left)?;
            parsed_range = true;
            continue;
        }

        // Standard binary operators via table lookup
        if let Some((l_bp, r_bp, op, token_count)) = self.infix_binding_power() {
            if l_bp < min_bp {
                break;
            }
            for _ in 0..token_count {
                self.advance();
            }
            let right = self.parse_binary_pratt(r_bp)?;
            left = self.arena.alloc_expr(/* Binary { op, left, right } */);
        } else {
            break;
        }
    }

    Ok(left)
}
```

### Encoding Associativity in Binding Powers

The elegant part of Pratt parsing is how associativity emerges from the binding power gap between left and right. No special-case code is needed — the numbers do the work:

**Left-associative** operators use `(even, odd)` — the right binding power is higher than the left. This means the recursive call has a higher minimum, so the same operator at the next position will fail the `l_bp < min_bp` check and stop. The left operand groups first:

```text
Left-associative +: bp = (21, 22)

Parsing: a + b + c    (min_bp starts at 0)

1. Parse a
2. See + with l_bp=21 ≥ min_bp=0 → consume, recurse with min_bp=22
   3. Parse b
   4. See + with l_bp=21 < min_bp=22 → STOP, return b
5. Build (a + b)
6. See + with l_bp=21 ≥ min_bp=0 → consume, recurse with min_bp=22
   7. Parse c, no more operators → return c
8. Build ((a + b) + c)
```

**Right-associative** operators use `(odd, even)` — the right binding power is lower than the left. The recursive call has a lower minimum, so the same operator at the next position will pass the check and recurse further. The right operand groups first:

```text
Right-associative ??: bp = (2, 1)

Parsing: a ?? b ?? c    (min_bp starts at 0)

1. Parse a
2. See ?? with l_bp=2 ≥ min_bp=0 → consume, recurse with min_bp=1
   3. Parse b
   4. See ?? with l_bp=2 ≥ min_bp=1 → consume, recurse with min_bp=1
      5. Parse c, no more operators → return c
   6. Build (b ?? c)
7. Build (a ?? (b ?? c))
```

**Non-associative** operators (like range `..`) use a flag to prevent chaining entirely — `1..10..20` is a parse error.

### The Binding Power Table

Ori's operators are assigned binding powers in pairs, ordered from lowest to highest precedence:

```rust
pub(super) mod bp {
    pub const COALESCE: (u8, u8) = (2, 1);      // ?? (right-assoc)
    pub const OR: (u8, u8) = (3, 4);             // ||
    pub const AND: (u8, u8) = (5, 6);            // &&
    pub const BIT_OR: (u8, u8) = (7, 8);         // |
    pub const BIT_XOR: (u8, u8) = (9, 10);       // ^
    pub const BIT_AND: (u8, u8) = (11, 12);      // &
    pub const EQUALITY: (u8, u8) = (13, 14);     // == !=
    pub const COMPARISON: (u8, u8) = (15, 16);   // < > <= >=
    pub const RANGE: u8 = 17;                     // .. ..= (non-assoc, special)
    pub const SHIFT: (u8, u8) = (19, 20);        // << >>
    pub const ADDITIVE: (u8, u8) = (21, 22);     // + -
    pub const MULTIPLICATIVE: (u8, u8) = (23, 24); // * / % div @
}
```

Range operators use a single constant rather than a pair because they are non-associative — a `parsed_range` flag prevents chaining.

### Static OPER_TABLE: O(1) Operator Lookup

The `infix_binding_power()` function uses a static `OPER_TABLE`: a 128-entry array indexed by token discriminant tag (`TokenTag` as `u8`). Each entry stores the left and right binding powers, operator variant index, and token count. Non-operator tags have `left_bp == 0` as a sentinel.

This provides O(1) operator lookup — a single array index instead of a `match` chain — on the hottest path in the Pratt loop. The table is populated at compile time by a `define_operators!` macro that maps token tags to their binding powers and operator variants, preventing drift between the table and the `bp` constants.

The `Gt` entry maps to `BinaryOp::Gt` by default; compound `>=` and `>>` are handled by adjacency checks at the call site (see [Compound Operator Synthesis](#compound-operator-synthesis) below).

## Entry Points

Different syntactic contexts need different subsets of operators. The Pratt parser handles this with different initial `min_bp` values:

| Function | Minimum | Use Case |
|----------|---------|----------|
| `parse_expr()` | 0 + assignment | Full expressions including `=` |
| `parse_non_assign_expr()` | 0 | Expressions without top-level assignment (guard clauses) |
| `parse_non_comparison_expr()` | `bp::RANGE` (17) | Expressions where `<`/`>` are delimiters (const generic defaults) |

`parse_non_comparison_expr()` is necessary for contexts like `type Foo<$N: int = 10>`, where `>` closes the generic parameter list rather than acting as a comparison operator. By starting with `min_bp = bp::RANGE`, all comparison-level and lower-precedence operators are excluded from parsing, and `>` is left for the caller to interpret as a delimiter.

## Compound Operator Synthesis

The lexer produces individual `>` tokens so that nested generics like `Result<Result<T, E>, E>` parse without special lexer modes. In expression context, the Pratt parser reconstructs multi-character operators from adjacent tokens:

```rust
fn infix_binding_power(&self) -> Option<(u8, u8, BinaryOp, usize)> {
    match self.current_kind() {
        TokenKind::Gt => {
            if self.is_greater_equal() {
                Some((bp::COMPARISON.0, bp::COMPARISON.1, BinaryOp::GtEq, 2))
            } else if self.is_shift_right() {
                Some((bp::SHIFT.0, bp::SHIFT.1, BinaryOp::Shr, 2))
            } else {
                Some((bp::COMPARISON.0, bp::COMPARISON.1, BinaryOp::Gt, 1))
            }
        }
        // ...other operators via OPER_TABLE...
    }
}
```

The `token_count` return value (2 for compound operators) tells the Pratt loop how many tokens to advance past. The `Cursor` methods `is_shift_right()` and `is_greater_equal()` check span adjacency — the two `>` tokens (or `>` and `=`) must have no whitespace between them. `> >` with a space is two separate greater-than comparisons; `>>` without a space is a right shift.

This is the same strategy used by [Rust](https://github.com/rust-lang/rust) and [Swift](https://github.com/swiftlang/swift). The alternative — having the lexer emit `>>` and `>=` as distinct tokens and then splitting them in type contexts — requires the lexer to know about parsing context, which violates phase separation.

## Unary Operators

Unary operators are parsed before entering the Pratt loop, in `parse_unary()`. This is standard — unary operators have the highest precedence (they bind tighter than any binary operator), so they are handled at the base of the expression recursion:

```rust
fn parse_unary(&mut self) -> Result<ExprId, ParseError> {
    if let Some(op) = self.match_unary_op() {
        let op_span = self.current_span();
        self.advance();

        // Constant folding: -42 → Int(-42) instead of Unary(Neg, Int(42))
        if matches!(op, UnaryOp::Neg) {
            if let TokenKind::Int(n) = self.current_kind() {
                let negated = n.wrapping_neg();
                // ...fold to ExprKind::Int(negated)
            }
        }

        let operand = self.parse_unary()?;  // Right-recursive for chaining: --x
        // ...alloc Unary node
    } else {
        self.parse_postfix()
    }
}
```

The constant folding for negative literals is worth noting. Without it, `-42` would be represented as `Unary(Neg, Int(42))` — an extra AST node that every downstream phase must handle. By folding at parse time, `-42` becomes `Int(-42)`, the same representation as any other integer literal. The `wrapping_neg()` call handles `i64::MIN` correctly, since `-(-9223372036854775808)` cannot be represented as a positive literal that is then negated.

Unary operators are right-recursive (`parse_unary()` calls itself), so `--x` parses as `-(-(x))` and `!!flag` parses as `!(!flag)`.

## Range Expressions

Range operators (`..`, `..=`) require special handling in the Pratt loop for two reasons:

1. **Non-associative** — `1..10..20` is a parse error, not a nested range. A `parsed_range` flag prevents chaining.
2. **Optional operands** — the end is optional (`0..` for open-ended ranges), and an optional `by` step clause follows (`0..100 by 2`).

```text
Grammar: left ( ".." | "..=" ) [ end ] [ "by" step ]
```

End and step expressions are parsed at shift precedence level (`bp::SHIFT.0 = 19`), ensuring that lower-precedence operators like `==` or `&&` are not consumed as part of the range.

## Postfix Operators

Postfix operators are not part of the Pratt loop. They are handled by `apply_postfix_ops()`, which runs in a tight loop after each primary/unary parse, consuming postfix operations as long as they continue:

| Syntax | Kind | Parsing |
|--------|------|---------|
| `f(args)` | Function call | `parse_call_args()` |
| `f(a: 1, b: 2)` | Named call | Detected via `is_named_arg_start()` |
| `obj.method(args)` | Method call | Dot + ident + lparen |
| `obj.field` | Field access | Dot + ident |
| `obj.0` | Tuple access | Dot + int literal |
| `arr[i]` | Index | Bracket-delimited |
| `expr?` | Try operator | Single token |
| `expr as Type` | Cast | `as` keyword |
| `expr as? Type` | Safe cast | `as?` keyword pair |

Postfix operators bind tighter than any binary or unary operator — `a.b + c` is `(a.b) + c`, never `a.(b + c)`. This is why they are parsed outside the Pratt loop, at the base of the expression recursion.

## Stack Safety

Deeply nested expressions (e.g., `(((((...)))))`  or long chains of binary operators) can overflow the stack in recursive descent. The parser wraps `parse_expr` in `ori_stack::ensure_sufficient_stack()`, which uses the [`stacker`](https://crates.io/crates/stacker) crate for platform-appropriate stack probing on native targets (no-op on WASM).

This is the same approach used by [Rust's compiler](https://github.com/rust-lang/rust), which also uses `stacker` to handle arbitrarily nested expressions without limiting nesting depth artificially.

## Design Tradeoffs

### Why Pratt Over Recursive Descent Chain

Ori originally used a recursive descent chain with a `parse_binary_level!` macro for DRY. The Pratt parser replaced it for three reasons:

1. **Performance** — ~30 function calls per simple expression reduced to ~4. The static lookup table avoids both the call overhead and the branch prediction misses of a 12-level dispatch chain.
2. **Readability** — a single loop with a data table is easier to understand than 12 nearly-identical functions that differ only in which operators they match and which function they call next.
3. **Extensibility** — adding a new operator or precedence level means adding one entry to the binding power table, not inserting a new function into the chain and updating all the call relationships.

The tradeoff is that range operators and compound assignment need special handling outside the table — their semantics don't fit the pure "left, operator, right" model. This is a small cost compared to the simplification of the common case.

### Why Not Shunting-Yard

Dijkstra's shunting-yard algorithm uses an explicit operator stack, which makes it harder to integrate with the recursive descent parser that handles declarations, statements, and non-expression constructs. The Pratt parser is a natural fit for a recursive descent parser because it uses the call stack for nesting — the same mechanism the rest of the parser uses.

## Prior Art

| Implementation | Approach | Notable Detail |
|---------------|----------|----------------|
| [Pratt (1973)](https://dl.acm.org/doi/10.1145/512927.512931) | Original paper: "null denotation" (prefix) and "left denotation" (infix) per token type | Formalized binding power as the core abstraction for operator precedence |
| [Go](https://github.com/golang/go) | Pratt-style expression parsing in `go/parser` | Simple binding power table; Go's deliberately small operator set makes the table tiny |
| [Rust](https://github.com/rust-lang/rust) | Precedence climbing in `rustc_parse` with `AssocOp` enum and `Fixity` | `>` splitting for generics; turbofish `::<>` disambiguation adds complexity |
| [Zig](https://github.com/ziglang/zig) | Flat precedence table with explicit state machine | Expression parsing uses a flat array indexed by operator, similar to Ori's `OPER_TABLE` |
| [rust-analyzer](https://github.com/rust-lang/rust-analyzer) | Pratt parser by Matklad, who wrote the influential ["Simple but Powerful Pratt Parsing"](https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html) blog post | Demonstrates Pratt parsing in a real-world IDE parser with error recovery |
| [Clang](https://github.com/llvm/llvm-project) | Precedence climbing for C/C++ expressions | Handles C's complex operator set (ternary `?:`, comma, assignment) in a single loop |
| [Crafting Interpreters](https://craftinginterpreters.com/compiling-expressions.html) | Pratt parser tutorial by Bob Nystrom | The most widely-read modern introduction to Pratt parsing, responsible for much of its current popularity |
