---
title: "Pratt Parser"
description: "Ori Compiler Design — Pratt Parser for Operator Precedence"
order: 403
section: "Parser"
---

# Pratt Parser

The Ori parser uses a Pratt parser (also known as "top-down operator precedence" or "precedence climbing") for binary expressions. A single loop with a binding power table handles all operator precedence levels, replacing the traditional recursive descent chain where each precedence level requires its own function.

## Binding Power Model

Each binary operator has a left and right binding power. The Pratt loop compares the left binding power against a minimum threshold (`min_bp`) to decide whether to parse the operator at the current level:

```rust
fn parse_binary_pratt(&mut self, min_bp: u8) -> Result<ExprId, ParseError> {
    let mut left = self.parse_unary()?;

    loop {
        self.skip_newlines();

        // Range operators require special handling (non-associative, optional end)
        if !parsed_range && min_bp <= bp::RANGE
            && matches!(self.current_kind(), TokenKind::DotDot | TokenKind::DotDotEq)
        {
            left = self.parse_range_continuation(left)?;
            parsed_range = true;
            continue;
        }

        // Standard binary operators
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

### How Associativity Works

Associativity is encoded entirely in the binding power gap between left and right:

- **Left-associative**: `(even, odd)` — right power is higher, so the right operand binds more tightly, causing left-to-right grouping
- **Right-associative**: `(odd, even)` — left power is higher, so the same operator can recurse on the right

```
// Left-associative (e.g., +): bp = (21, 22)
a + b + c  →  (a + b) + c
// min_bp starts at 0, parses a, sees +, recurses with min_bp=22
// Inner call parses b, sees + with l_bp=21 < min_bp=22, stops
// Outer loop picks up second +

// Right-associative (e.g., ??): bp = (2, 1)
a ?? b ?? c  →  a ?? (b ?? c)
// min_bp starts at 0, parses a, sees ??, recurses with min_bp=1
// Inner call parses b, sees ?? with l_bp=2 >= min_bp=1, recurses again
```

## Binding Power Table

Constants are defined in `grammar/expr/mod.rs`:

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
    pub const MULTIPLICATIVE: (u8, u8) = (23, 24); // * / % div
}
```

Range operators use a single binding power constant rather than a pair because they are non-associative — `1..10..20` is a parse error. A `parsed_range` flag prevents chaining.

## Entry Points

Different contexts need different subsets of operators:

| Function | `min_bp` | Use Case |
|----------|----------|----------|
| `parse_expr()` | 0 + assignment | Full expressions including `=` |
| `parse_non_assign_expr()` | 0 | Expressions without top-level assignment (guard clauses) |
| `parse_non_comparison_expr()` | `bp::RANGE` (17) | Expressions where `<`/`>` are delimiters (const generic defaults) |

## Compound Operator Synthesis

The lexer produces individual `>` tokens so that nested generics like `Result<Result<T, E>, E>` parse correctly. In expression context, the Pratt parser's `infix_binding_power()` combines adjacent `>` tokens:

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
        // ...
    }
}
```

The `token_count` return value (2 for compound operators) tells the Pratt loop how many tokens to advance. The `Cursor` methods `is_shift_right()` and `is_greater_equal()` check span adjacency — the tokens must have no whitespace between them.

## Unary Operators

Unary operators are parsed before entering the Pratt loop:

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

        let operand = self.parse_unary()?;  // Right-recursive for chaining
        // ...alloc Unary node
    } else {
        self.parse_postfix()
    }
}
```

The constant folding optimization for negative literals handles `i64::MIN` correctly using `wrapping_neg`, since `-(-9223372036854775808)` cannot be represented as a positive literal.

## Range Expressions

Range operators require special handling in the Pratt loop because they have optional operands and a `by` step clause:

```
Grammar: left ( ".." | "..=" ) [ end ] [ "by" step ]
```

- Open-ended ranges: `0..` (no end)
- Stepped ranges: `0..100 by 2`
- End and step are parsed at shift precedence level (`bp::SHIFT.0`)

## Postfix Operators

Postfix operators are not part of the Pratt loop — they are handled by `apply_postfix_ops()` which runs after each primary/unary parse:

| Syntax | Kind | Parsing |
|--------|------|---------|
| `f(args)` | Function call | `parse_call_args()` |
| `f(a: 1, b: 2)` | Named call | Detected via `is_named_arg_start()` |
| `obj.method(args)` | Method call | Dot + ident + lparen |
| `obj.field` | Field access | Dot + ident |
| `arr[i]` | Index | Bracket-delimited |
| `expr?` | Try operator | Single token |
| `expr.await` | Await | Dot + keyword |
| `expr as Type` | Cast | `as` keyword |
| `expr as? Type` | Safe cast | `as?` keyword pair |

## Stack Safety

Deeply nested expressions (e.g., `(((((...)))))`  or long chains) can overflow the stack in recursive descent. The parser wraps `parse_expr` in `ori_stack::ensure_sufficient_stack()`, which uses platform-appropriate stack probing (via `stacker` on native, no-op on WASM).

## Design Rationale

The Pratt parser approach was chosen over alternatives:

| Approach | Pros | Cons |
|----------|------|------|
| **Recursive descent chain** (original) | Simple, one function per level | 12+ function calls per primary expression; `parse_binary_level!` macro for DRY |
| **Pratt parser** (current) | Single loop, table-driven, ~4 calls per expression | Range operators need special handling |
| **Parser generator** (e.g., LALR) | Formal grammar verification | Poor error messages, difficult error recovery, external dependency |

The Pratt parser reduces function call overhead from ~30 calls per simple expression to ~4, while keeping the code readable and the error messages precise.
