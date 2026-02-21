---
plan: "dpr_parser-lexer-scalability_02212026"
title: "Design Pattern Review: Lexer/Parser Scalability for Complex Syntax"
status: draft
---

# Design Pattern Review: Lexer/Parser Scalability for Complex Syntax

## Ori Today

Ori's lexer is a two-phase pipeline: a raw scanner (`RawScanner` in `ori_lexer_core`) produces `(RawTag, len)` pairs at ~1 GiB/s, then a cooking layer (`TokenCooker` in `ori_lexer`) maps these to `TokenKind` values at ~240 MiB/s, resolving keywords, parsing literals, and interning strings. The raw scanner dispatches on the first byte (0-255 exhaustive match), and each operator character has a focused method that peeks ahead for compound forms: `equal()` checks for `=` or `>`, `less()` checks for `=` or `<`, `dot()` chains up to three dots. This architecture is clean, fast, and well-tested. The `RawTag` enum (`repr(u8)`) occupies exactly 1 byte; `RawToken` is 8 bytes. The cooker is a stateless 1:1 map for operators/delimiters and a stateful path for identifiers, literals, and templates.

The parser uses a Pratt (binding power) approach centered on a macro-generated static lookup table. The `define_operators!` macro in `compiler/ori_parse/src/grammar/expr/operators.rs` generates `OPER_TABLE[128]` (indexed by `TokenTag` discriminant), `op_from_u8()`, and a test-only operator count. Each entry is a 4-byte `OperInfo { left_bp, right_bp, op, token_count }`. The `infix_binding_power()` method on `Parser` does a single array lookup for O(1) dispatch, with special-case handling for `>` (which synthesizes `>=` and `>>` from adjacent tokens to support generic syntax like `Result<Result<T, E>, E>`). Assignment is handled outside this table, as a simple `if self.cursor.check(&TokenKind::Eq)` in `parse_expr_inner()`. Range operators (`..`/`..=`) are also outside the table due to their non-associative, optional-operand grammar.

The system works well for the current operator set (22 `BinaryOp` variants, 18 in the table, 4 special-cased). But it has structural gaps that compound assignment will expose. Assignment lives entirely outside the operator table, so compound assignment will require a parallel dispatch mechanism. The `>` token synthesis pattern is invisible to the table, making `>>=` (shift-right-assign) require yet another manual code path. Each new operator requires touching 4-6 files across 3 crates (`ori_lexer_core`, `ori_ir`, `ori_lexer`) with no centralized registry ensuring they stay in sync. There is no concept of "operator class" (this operator is an assignment form of that binary operator), so the relationship between `+=` and `+` must be encoded as ad-hoc parser logic rather than declarative data.

## Prior Art

### Go -- Operator Metadata Carriage

Go's scanner classifies operators into explicit categories: `_Operator`, `_AssignOp`, `_IncOp`, `_Assign`. When the scanner encounters `+`, it stores `Op=Add` and precedence metadata, then checks if the next character is `=` to emit `_AssignOp` instead of `_Operator`. The critical insight is that **operators are metadata, not types** -- a single `_AssignOp` token carries the underlying operator identity as a field, rather than having 13 separate `PlusEq`, `MinusEq`, etc. token kinds. This means adding a new compound assignment requires exactly one new `Op=X` case in the scanner; the parser's assignment-handling code is generic over all compound operators. The downside is that the token kind enum loses information -- you need to inspect the metadata to know which operator you have -- but for a language where compound assignment is pure syntactic sugar (like Ori), this trade-off is strongly favorable.

### Rust -- Two-Layer Reconstruction with AST Distinction

Rust's lexer (`rustc_lexer`) produces only single-character tokens: `Plus`, `Equals`, `Minus`, etc. The parser layer reconstructs compound operators from token pairs: `+` followed by `=` becomes an `AssignOp(AddAssign)`. This enables context-sensitive decisions -- `<` in generics vs. comparison -- without lexer complexity. Rust maintains a separate `AssignOpKind` enum (10 variants) distinct from `BinOpKind`, with explicit `From<AssignOpKind> for BinOpKind` conversion. The key lesson for Ori is that Rust already solves the `>` disambiguation problem that Ori has (generics vs. operators) and does it by deferring compound operator assembly to the parser. However, Rust's approach requires the parser to do token-pair matching for every compound operator, which adds parse-time overhead and manual code per operator.

### TypeScript -- Dense Token Enumeration with Re-scanning

TypeScript takes the opposite approach: every possible operator gets its own `SyntaxKind` variant, including three-character forms like `GreaterThanGreaterThanGreaterThanEqualsToken`. The lexer scans maximally (greedy), and when the parser discovers it was wrong (e.g., `>` was a generic closer, not a comparison), it calls `reScanGreaterToken()` to re-tokenize. This is the "enumerate everything" strategy: low parser complexity (each token IS its meaning), high token-kind count, and an explicit re-scanning escape hatch for disambiguation. The lesson for Ori is the re-scanning pattern -- when maximal munch guesses wrong, have a principled way to decompose a compound token back into its parts.

### Zig -- Explicit State Machine with No Ambiguity

Zig uses a state machine where each partial operator has its own state: `.plus` checks for `=` (to `.plus_equal`), `+` (to `.plus_plus`), `%` (to `.plus_percent`), or `|` (to `.plus_pipe`). Multi-character operators get intermediate states: `.plus_percent` can further see `=` to become `.plus_percent_equal`. This is the most explicit approach: every operator path is programmed as a state transition, making the scanner correct by construction. There is no ambiguity and no need for re-scanning. The cost is O(n) states for n operators, but the states are cheap (enum variants) and the transitions are simple (single-byte lookahead). For Ori, the relevant lesson is that Zig proves explicit per-operator scanning scales well -- Zig has more operators than most languages and its scanner is fast and correct.

## Proposed Best-of-Breed Design

### Core Idea

Combine Go's "operator metadata carriage" with Ori's existing two-phase architecture and Pratt table. Instead of adding 13 new `RawTag` variants for compound assignment, add a single `AssignOp` concept that carries the underlying binary operator as metadata. At the raw scanner level, each existing operator method gains one additional lookahead check (`= follows?`). At the `TokenKind` level, compound assignment operators are **not** separate enum variants -- instead, the parser recognizes the `base_op` + `Eq` token pair in the assignment position, informed by a new declarative `COMPOUND_ASSIGN_TABLE`. This keeps the raw scanner changes to a minimum (extending existing methods, not adding new dispatch paths), avoids token-kind explosion, preserves the Pratt table for binary operators unchanged, and centralizes the `compound_op -> binary_op` mapping in one place.

The design has two viable implementation paths, and this proposal recommends the first:

**Path A (Recommended): Lexer-level compound tokens with metadata.** Add `RawTag` variants for compound assignment operators (the discriminant range 32-61 has room). Each existing scanner method (`single()` for `+`, `*`, `%`, `^`; `minus_or_arrow()` for `-`; etc.) gains one `= follows?` check. The cooker maps these to dedicated `TokenKind` variants. The parser handles compound assignment in `parse_expr_inner()` alongside plain assignment, using a declarative mapping table. This path is explicit, cacheable (Salsa sees stable token kinds), and follows the Zig philosophy of no ambiguity.

**Path B (Alternative): Parser-level reconstruction from token pairs.** Keep the raw scanner unchanged. The parser, after parsing the left-hand side and seeing a binary operator token followed by `=`, recognizes the compound assignment. This follows Rust's approach but would require the parser to peek at two tokens and verify adjacency, adding complexity to `parse_expr_inner()`. It also means the `>>=` case requires triple-token synthesis (three adjacent `>`, `>`, `=`), which is fragile.

Path A is recommended because it aligns with Ori's existing pattern (the lexer already produces `==`, `!=`, `<=`, `&&`, `||`, `<<`, `..`, `..=` as compound tokens) and avoids parser-level token synthesis for the common case. The `>>=` edge case (3 characters) is handled by extending the existing `>` synthesis pattern in the parser, which already handles `>=` and `>>`.

### Key Design Choices

1. **Lexer-level compound tokens, not parser-level reconstruction** (inspired by Go and Zig, contra Rust). Ori's raw scanner already handles multi-character operators via lookahead. Extending each operator method with one `=` check is O(1) per operator, adds ~2 instructions to the hot path (load + compare), and keeps the parser simple. This preserves the ~1 GiB/s raw scan target -- the additional branch is highly predictable (compound assignment is rare in source code).

2. **Declarative compound-assignment-to-binary-op mapping table** (inspired by Go's metadata carriage). A single `const` array maps each compound assignment `TokenTag` to its corresponding `BinaryOp`. This replaces ad-hoc match arms scattered across the parser. Adding a future compound operator requires one entry in the scanner, one entry in the cooker, and one entry in this table -- three lines of code, not thirteen.

3. **Compound assignment handled alongside plain assignment in `parse_expr_inner()`** (follows Ori's existing pattern). Assignment is already a top-level check after binary Pratt parsing. Compound assignment extends this check: after parsing `left`, check for `=` (plain assign) OR any compound-assign token (compound assign with desugaring). This keeps the Pratt table unchanged and avoids polluting binary operator dispatch with assignment semantics.

4. **`>>=` handled via the existing `>` synthesis pattern** (inspired by Rust's parser-level reconstruction). The raw scanner emits three separate `>` tokens for `>>=` (or `>` `>` `=` if spaced). The parser's `infix_binding_power()` already synthesizes `>>` from adjacent `>` `>`. For `>>=`, the parser first synthesizes `>>` and then checks if the next token is `=` and adjacent -- if so, it is `>>=` (shift-right-assign). This is consistent with the existing `>=` synthesis and avoids adding a `GreaterGreaterEqual` raw tag that would complicate the generic-closing logic.

5. **No new AST node types** (follows the approved compound assignment proposal). Compound assignment desugars to `ExprKind::Assign { target, value: ExprKind::Binary { left: target, op, right: rhs } }` at parse time. The `&&=` and `||=` forms desugar through `ExprKind::Binary { op: BinaryOp::And/Or, ... }`, which already implements short-circuit evaluation. No IR, type checker, evaluator, or codegen changes needed.

6. **`RawTag` discriminant allocation uses the existing gap** (Ori-specific constraint). The operator range is 32-61, currently using 32-61 with discriminant 47 and 51 reserved. There is room for 13 new compound assignment tags if we use a new range. The proposal uses range 62-79 (currently empty between operators and delimiters) for compound assignment operators, keeping the existing operator range untouched.

### What Makes Ori's Approach Unique

Ori's two-phase lexer creates an opportunity that no reference compiler has. The raw scanner can produce compound tokens aggressively (maximal munch) because the cooker sits between the scanner and parser, acting as a normalization layer. If a future syntax change requires decomposing a compound token, the cooker can split it -- unlike Go (which has no cooker), Rust (which already uses single-char raw tokens), or TypeScript (which requires parser-level re-scanning).

The `OPER_TABLE[128]` indexed by `TokenTag` discriminant is also unique among the reference compilers. It gives O(1) operator lookup where Go uses precedence integers, Rust uses match chains, and TypeScript uses a precedence function. The 128-slot table has ~40 active entries, leaving ~90 slots for future operators without resizing. As long as `TokenTag` discriminants stay below 128, the table requires no structural changes.

Ori's Pratt parser with `token_count` field in `OperInfo` was designed with multi-token operators in mind (the field exists but is always 1 today). The `>>=` case will be the first real use of a 3-token compound operator, though it is handled via synthesis rather than the token_count field.

### Concrete Types & Interfaces

#### New `RawTag` Variants

```rust
// In ori_lexer_core/src/tag/mod.rs, new range 62-79 for compound assignment:

// Compound assignment operators (62-79)
/// `+=`
PlusEq = 62,
/// `-=`
MinusEq = 63,
/// `*=`
StarEq = 64,
/// `/=`
SlashEq = 65,
/// `%=`
PercentEq = 66,
/// `@=`
AtEq = 67,
/// `&=`
AmpersandEq = 68,
/// `|=`
PipeEq = 69,
/// `^=`
CaretEq = 70,
/// `<<=`
ShlEq = 71,
/// `&&=`
AmpersandAmpersandEq = 72,
/// `||=`
PipePipeEq = 73,
// Note: `>>=` is synthesized at parse time from `>` `>` `=`,
// not tokenized as a single raw token (same reason as `>=` and `>>`).
```

#### Scanner Method Extensions

```rust
// In ori_lexer_core/src/raw_scanner/mod.rs, extend existing methods:

// Before (current):
b'+' => self.single(start, RawTag::Plus),

// After:
b'+' => self.plus(start),

// New method:
fn plus(&mut self, start: u32) -> RawToken {
    self.cursor.advance(); // consume '+'
    if self.cursor.current() == b'=' {
        self.cursor.advance();
        RawToken { tag: RawTag::PlusEq, len: self.cursor.pos() - start }
    } else {
        RawToken { tag: RawTag::Plus, len: self.cursor.pos() - start }
    }
}

// Similarly for -, *, /, %, ^, @. The & and | methods already exist
// (ampersand() and pipe()) and need one additional branch:

fn ampersand(&mut self, start: u32) -> RawToken {
    self.cursor.advance(); // consume '&'
    match self.cursor.current() {
        b'&' => {
            self.cursor.advance();
            // Check for &&=
            if self.cursor.current() == b'=' {
                self.cursor.advance();
                RawToken { tag: RawTag::AmpersandAmpersandEq, len: self.cursor.pos() - start }
            } else {
                RawToken { tag: RawTag::AmpersandAmpersand, len: self.cursor.pos() - start }
            }
        }
        b'=' => {
            self.cursor.advance();
            RawToken { tag: RawTag::AmpersandEq, len: self.cursor.pos() - start }
        }
        _ => RawToken { tag: RawTag::Ampersand, len: self.cursor.pos() - start },
    }
}

// The `<` method extends similarly:
fn less(&mut self, start: u32) -> RawToken {
    self.cursor.advance(); // consume '<'
    match self.cursor.current() {
        b'=' => {
            self.cursor.advance();
            RawToken { tag: RawTag::LessEqual, len: self.cursor.pos() - start }
        }
        b'<' => {
            self.cursor.advance();
            // Check for <<=
            if self.cursor.current() == b'=' {
                self.cursor.advance();
                RawToken { tag: RawTag::ShlEq, len: self.cursor.pos() - start }
            } else {
                RawToken { tag: RawTag::Shl, len: self.cursor.pos() - start }
            }
        }
        _ => RawToken { tag: RawTag::Less, len: self.cursor.pos() - start },
    }
}
```

#### New `TokenTag` and `TokenKind` Variants

```rust
// In ori_ir/src/token/tag.rs, new range after operators:
// Compound assignment (130-142) — outside the 0-127 OPER_TABLE range
// because compound assignment is NOT a binary operator and should NOT
// be in the Pratt table.
//
// IMPORTANT: This means TokenTag::MAX_DISCRIMINANT must increase,
// and the POSTFIX_BITSET and OPER_TABLE sizing must be reconsidered.
//
// ALTERNATIVE (recommended): Use the 0-127 range. Compound assignment
// tokens will have OPER_TABLE entries with left_bp == 0 (not binary ops),
// which is the existing "not an operator" sentinel. This avoids
// expanding the table.

// Recommended: fit within 0-127 by using currently-unused discriminant values.
// TokenTag operator range is 100-120. We can extend to 100-127:
PlusEq = 121,      // (repurpose: Newline moves to a higher range)
// ... actually, Newline is at 121, Error at 122, Eof at 127.
// We need to reorganize. See Implementation Roadmap Phase 1.
```

The discriminant layout needs reorganization to fit compound assignment tokens. The current layout uses 100-120 for operators and 121-127 for special tokens. Two approaches:

**Option A: Expand the table to 256 entries.** Change `OPER_TABLE` from `[OperInfo; 128]` to `[OperInfo; 256]`. This costs 512 bytes of static memory (4 bytes x 128 extra slots) and removes the 128-discriminant constraint. `TokenTag` becomes `repr(u8)` with full 0-255 range.

**Option B: Reorganize discriminants.** Move special tokens (Newline=121, Error=122, Eof=127) to higher values (e.g., 250-255) and use 121-133 for compound assignment. The `OPER_TABLE` stays at 128 entries but compound assignment tokens are outside it (which is correct -- they are not binary operators for the Pratt parser).

**Recommendation: Option B.** The table stays small. Compound assignment tokens live at 121-133, outside the Pratt table but within u8 range. The only constraint is that the `POSTFIX_BITSET` (2 x u64 = 128 bits) would not cover them, which is fine because compound assignment tokens are not postfix operators. Actual implementation: give special tokens high discriminants (e.g., Newline=250, Error=251, Eof=255) and use 121-133 for compound assignment.

```rust
// In ori_ir/src/token/tag.rs (after reorganization):

// Compound assignment operators (121-133)
PlusEq = 121,       // +=
MinusEq = 122,      // -=
StarEq = 123,       // *=
SlashEq = 124,      // /=
PercentEq = 125,    // %=
AtEq = 126,         // @=
AmpEq = 127,        // &=
PipeEq = 128,       // |=     -- NOTE: exceeds 128, need larger table or different approach
CaretEq = 129,      // ^=
ShlEq = 130,        // <<=
// ShrEq is synthesized, not a token
AmpAmpEq = 131,     // &&=
PipePipeEq = 132,   // ||=
```

Given that we exceed 128 at `PipeEq`, the cleanest approach is **Option A: expand to 256**. This adds 512 bytes of static memory (trivial) and removes all discriminant pressure permanently. The `POSTFIX_BITSET` should also expand to 4 x u64 = 256 bits (32 bytes, trivial).

```rust
// Final recommendation: expand tables to 256

// OPER_TABLE becomes:
static OPER_TABLE: [OperInfo; 256] = { ... };

// POSTFIX_BITSET becomes:
const POSTFIX_BITSET: [u64; 4] = { ... };

// TokenTag discriminant constraint relaxes:
const _: () = assert!(TokenTag::MAX_DISCRIMINANT <= 255);
```

#### Compound Assignment Dispatch Table

```rust
// In ori_parse/src/grammar/expr/operators.rs (or a new sibling file):

/// Maps compound assignment token tags to their underlying BinaryOp.
///
/// Returns `None` for tags that are not compound assignment operators.
/// `>>=` is not in this table -- it is synthesized from `>` `>` `=` tokens.
#[inline]
pub(crate) fn compound_assign_op(tag: u8) -> Option<BinaryOp> {
    match tag {
        TokenKind::TAG_PLUS_EQ    => Some(BinaryOp::Add),
        TokenKind::TAG_MINUS_EQ   => Some(BinaryOp::Sub),
        TokenKind::TAG_STAR_EQ    => Some(BinaryOp::Mul),
        TokenKind::TAG_SLASH_EQ   => Some(BinaryOp::Div),
        TokenKind::TAG_PERCENT_EQ => Some(BinaryOp::Mod),
        TokenKind::TAG_AT_EQ      => Some(BinaryOp::MatMul),
        TokenKind::TAG_AMP_EQ     => Some(BinaryOp::BitAnd),
        TokenKind::TAG_PIPE_EQ    => Some(BinaryOp::BitOr),
        TokenKind::TAG_CARET_EQ   => Some(BinaryOp::BitXor),
        TokenKind::TAG_SHL_EQ     => Some(BinaryOp::Shl),
        TokenKind::TAG_AMPAMP_EQ  => Some(BinaryOp::And),
        TokenKind::TAG_PIPEPIPE_EQ => Some(BinaryOp::Or),
        _ => None,
    }
}
```

#### Parser Integration

```rust
// In ori_parse/src/grammar/expr/mod.rs, extend parse_expr_inner():

fn parse_expr_inner(&mut self) -> ParseOutcome<ExprId> {
    let left = chain!(self, self.parse_binary_pratt(0));

    // Check for plain assignment (= but not == or =>)
    if self.cursor.check(&TokenKind::Eq) {
        let left_span = self.arena.get_expr(left).span;
        self.cursor.advance();
        let right = require!(self, self.parse_expr(), "expression after `=`");
        let right_span = self.arena.get_expr(right).span;
        let span = left_span.merge(right_span);
        return ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
            ExprKind::Assign { target: left, value: right },
            span,
        )));
    }

    // Check for compound assignment (+=, -=, *=, etc.)
    let tag = self.cursor.current_tag();
    if let Some(op) = compound_assign_op(tag) {
        return self.parse_compound_assign(left, op);
    }

    // Check for >>= (synthesized from > > =)
    if self.cursor.is_shift_right_assign() {
        return self.parse_compound_assign(left, BinaryOp::Shr);
    }

    ParseOutcome::consumed_ok(left)
}

/// Parse compound assignment: `target op= rhs` desugars to
/// `target = target op rhs`.
fn parse_compound_assign(
    &mut self,
    target: ExprId,
    op: BinaryOp,
) -> ParseOutcome<ExprId> {
    let target_span = self.arena.get_expr(target).span;
    self.cursor.advance(); // consume the compound assignment token
    // (for >>=, the caller already advanced past >> and we advance past =)

    let rhs = require!(self, self.parse_expr(), "expression after compound assignment");
    let rhs_span = self.arena.get_expr(rhs).span;

    // Desugar: target op= rhs  -->  target = target op rhs
    let binary = self.arena.alloc_expr(Expr::new(
        ExprKind::Binary { op, left: target, right: rhs },
        target_span.merge(rhs_span),
    ));
    let span = target_span.merge(rhs_span);
    ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(
        ExprKind::Assign { target, value: binary },
        span,
    )))
}
```

#### Cursor Extension for `>>=` Detection

```rust
// In ori_parse/src/cursor/mod.rs:

/// Check if looking at `>` `>` `=` (all adjacent, no whitespace).
/// Used for detecting `>>=` shift-right-assign in expression context.
pub fn is_shift_right_assign(&self) -> bool {
    self.current_tag() == TokenKind::TAG_GT
        && self.pos + 2 < self.tags.len()
        && self.tags[self.pos + 1] == TokenKind::TAG_GT
        && self.tags[self.pos + 2] == TokenKind::TAG_EQ
        && self.next_is_adjacent()
        && self.is_adjacent(self.pos + 1, self.pos + 2)
}
```

## Immediate Application: Compound Assignment

The 13 compound assignment operators from the approved proposal map to the design as follows:

### Layer 1: Raw Scanner (`ori_lexer_core`)

Add 12 new `RawTag` variants (all except `>>=`):

| Variant | Discriminant | Scanner Method | Lookahead Change |
|---------|-------------|----------------|-----------------|
| `PlusEq` | 62 | `plus()` (new, replaces `single()`) | `+` then `=`? |
| `MinusEq` | 63 | `minus_or_arrow()` (extend) | `-` then `=`? before `>` check |
| `StarEq` | 64 | `star()` (new, replaces `single()`) | `*` then `=`? |
| `SlashEq` | 65 | `slash_or_comment()` (extend) | `/` then `=`? before `//` check |
| `PercentEq` | 66 | `percent()` (new, replaces `single()`) | `%` then `=`? |
| `AtEq` | 67 | `at()` (new, replaces `single()`) | `@` then `=`? |
| `AmpersandEq` | 68 | `ampersand()` (extend) | `&` then `=`? branch |
| `PipeEq` | 69 | `pipe()` (extend) | `\|` then `=`? branch |
| `CaretEq` | 70 | `caret()` (new, replaces `single()`) | `^` then `=`? |
| `ShlEq` | 71 | `less()` (extend) | `<<` then `=`? |
| `AmpersandAmpersandEq` | 72 | `ampersand()` (extend) | `&&` then `=`? |
| `PipePipeEq` | 73 | `pipe()` (extend) | `\|\|` then `=`? |

Each change adds exactly one `if self.cursor.current() == b'='` check to the existing method. Methods that currently use `self.single(start, RawTag::X)` become dedicated methods with the `=` check. The scanner code growth is approximately 12 x 5 lines = 60 lines.

**`>>=` is NOT a raw token.** It remains three separate `>` tokens. The parser synthesizes `>>=` the same way it already synthesizes `>=` and `>>` -- by checking adjacency. This preserves the critical property that `>>` in generic context (`Result<Result<T, E>, E>`) tokenizes as separate `>` tokens.

### Layer 2: Cooker (`ori_lexer`)

Add 12 direct-map entries in `TokenCooker::cook()`:

```rust
RawTag::PlusEq => TokenKind::PlusEq,
RawTag::MinusEq => TokenKind::MinusEq,
// ... etc.
```

### Layer 3: Token Types (`ori_ir`)

Add to `TokenTag` (12 new discriminants), `TokenKind` (12 new variants), `discriminant_index()` (12 new match arms), `display_name()` (12 new arms), `friendly_name_from_index()` (12 new arms), `TAG_*` constants (12 new constants). Also add `lexeme()` and `name()` entries to `RawTag`.

### Layer 4: Parser (`ori_parse`)

1. Add `compound_assign_op()` function mapping 12 token tags to `BinaryOp` variants.
2. Extend `parse_expr_inner()` with compound assignment check after plain assignment.
3. Add `parse_compound_assign()` method for desugaring.
4. Add `is_shift_right_assign()` to cursor for `>>=` detection.
5. Update `mistakes.rs`: remove compound assignment operators from the "common mistake" list (keep `??=`).

### Layer 5: No Changes Needed

- `ori_ir/src/ast/expr.rs`: No new `ExprKind` -- desugared to `Assign` + `Binary`.
- `ori_ir/src/ast/operators.rs`: No new `BinaryOp` variants (already has all needed ops).
- `ori_types`: Sees only the desugared form.
- `ori_eval`: Evaluates the desugared form.
- `ori_llvm`: Compiles the desugared form.

### Total Change Budget

| File | Lines Added | Lines Modified |
|------|------------|---------------|
| `ori_lexer_core/src/tag/mod.rs` | ~60 (variants + lexeme + name) | 0 |
| `ori_lexer_core/src/raw_scanner/mod.rs` | ~60 (method extensions) | ~10 (replace `single()` calls) |
| `ori_ir/src/token/tag.rs` | ~15 (discriminants) | ~5 (MAX_DISCRIMINANT) |
| `ori_ir/src/token/kind.rs` | ~80 (variants + match arms) | 0 |
| `ori_lexer/src/cooker/mod.rs` | ~12 (direct mappings) | 0 |
| `ori_parse/src/grammar/expr/mod.rs` | ~30 (compound assign logic) | ~5 (parse_expr_inner) |
| `ori_parse/src/grammar/expr/operators.rs` | ~20 (dispatch table) | 0 |
| `ori_parse/src/cursor/mod.rs` | ~10 (is_shift_right_assign) | 0 |
| `ori_parse/src/error/mistakes.rs` | 0 | ~5 (remove compound assign detection) |
| Tests | ~200+ | ~20 |
| **Total** | **~490** | **~45** |

## Future-Proofing: Next Syntax Additions

### Pipeline Operators (`|>`, `<|`)

If Ori adds pipeline operators, the design handles them naturally:

1. **Raw scanner**: `pipe()` gains one more branch (`|` then `>` becomes `PipeGt`). The method already has `||` branching, so this is one more arm.
2. **Cooker**: Direct map `RawTag::PipeGt => TokenKind::PipeGt`.
3. **Token types**: One new `TokenTag` discriminant, one new `TokenKind` variant.
4. **Pratt table**: One new entry in `define_operators!`: `TokenKind::TAG_PIPE_GT, Pipe, bp::PIPELINE, 1;` (assuming pipeline precedence is defined).

Total: ~15 lines of code across 4 files. No structural changes.

### New 3-Character Operators

The pattern established by `<<=` and `&&=` generalizes: the scanner method for the first character chains through the second and third characters. Each additional 3-character operator adds one nested `if` branch to the existing method. For example, if Ori added `<=>` (spaceship/three-way comparison):

```rust
fn less(&mut self, start: u32) -> RawToken {
    self.cursor.advance();
    match self.cursor.current() {
        b'=' => {
            self.cursor.advance();
            if self.cursor.current() == b'>' {
                self.cursor.advance();
                RawToken { tag: RawTag::Spaceship, len: self.cursor.pos() - start }
            } else {
                RawToken { tag: RawTag::LessEqual, len: self.cursor.pos() - start }
            }
        }
        b'<' => { /* existing Shl logic */ }
        _ => RawToken { tag: RawTag::Less, len: self.cursor.pos() - start },
    }
}
```

The key property: each 3-character operator adds depth to an existing method, not a new dispatch path. There is no state explosion because each method handles at most one first-character.

### Context-Sensitive Operator Disambiguation

The `>` token already demonstrates Ori's disambiguation strategy: the lexer emits the simplest token, and the parser uses adjacency checks to synthesize compounds. This pattern extends to any future context-sensitive operator. If a token has dual meaning depending on context (e.g., `<` in generics vs. comparison), the lexer emits the single-character form and the parser disambiguates.

The cooker layer provides an additional disambiguation point that reference compilers lack. If a raw token needs context-dependent cooking (e.g., a hypothetical `#` that means different things in different positions), the cooker can accept a mode flag without changing the scanner.

### User-Defined Operators

If Ori ever supports user-defined operators (unlikely given the design pillars, but worth considering), the architecture supports it through:

1. **Lexer**: User-defined operators would be identifiers, not new token kinds. No scanner changes.
2. **Parser**: The Pratt table would need dynamic entries, which is a fundamental change. However, the `compound_assign_op()` function and `OPER_TABLE` are already separated -- user-defined operators could live in a parallel dynamic table without affecting the static built-in table.
3. **Precedence**: User-defined operators would need explicit precedence annotations, parsed as metadata.

This is a major feature that would require its own proposal, but the static/dynamic table separation means it would not require rewriting the existing infrastructure.

### What Happens When Operator Count Doubles?

Today Ori has ~30 operator-related token kinds. After compound assignment, it has ~42. If the count doubles to ~84:

- **`RawTag`**: `repr(u8)` supports 256 variants. At 84 operators, we are at 33% capacity. No pressure.
- **`TokenTag`**: With the table expanded to 256, no pressure until ~200 total token kinds.
- **`OPER_TABLE`**: 256 entries x 4 bytes = 1 KB. Fits in L1 cache. The sparse entries (most are `NONE`) mean cache lines are not wasted -- the CPU only loads the lines containing actual operator entries.
- **Raw scanner**: Each new operator adds one branch to one existing method. The dispatch is on the first byte (256-way), and each method handles 2-4 continuations. At 84 operators, the average method handles ~3 continuations. This is still a single predicted branch per token.
- **Cooker**: Direct 1:1 map. Linear growth, constant time per token.
- **Pratt table**: Binary operators grow the `define_operators!` macro. The macro auto-generates `op_from_u8()` and `OPER_TABLE`, so the cost is one line per operator. The `op_from_u8()` function is a chain of `if` comparisons, which the compiler optimizes to a jump table.

**Conclusion**: The architecture scales linearly in code size and O(1) in runtime for operator count increases up to ~200, well beyond any realistic language design.

## Implementation Roadmap

### Phase 1: Foundation (Token Infrastructure)

- [ ] Reorganize `TokenTag` discriminants: move Newline/Error/Eof to high values (250+), freeing 121-149 for compound assignment and future use
- [ ] Expand `OPER_TABLE` from `[OperInfo; 128]` to `[OperInfo; 256]` (or keep at 128 and ensure compound assign tags are outside Pratt range)
- [ ] Expand `POSTFIX_BITSET` from `[u64; 2]` to `[u64; 4]` (256 bits)
- [ ] Update `TokenTag::MAX_DISCRIMINANT` assertion
- [ ] Add compile-time tests verifying discriminant layout invariants
- [ ] Verify all existing tests pass after reorganization

### Phase 2: Core (Compound Assignment Operators)

- [ ] Add 12 `RawTag` variants for compound assignment (range 62-73 in raw tag space)
- [ ] Extend 9 scanner methods with `=` lookahead (plus, minus, star, slash, percent, at, caret; extend ampersand, pipe, less for 3-char forms)
- [ ] Add `RawTag::lexeme()` and `RawTag::name()` entries for all 12 new tags
- [ ] Add 12 `TokenTag` discriminants and 12 `TokenKind` variants
- [ ] Add cooker mappings (12 direct-map entries)
- [ ] Add `TAG_*` constants, `discriminant_index()`, `display_name()`, `friendly_name_from_index()` arms
- [ ] Add `compound_assign_op()` dispatch function in parser
- [ ] Extend `parse_expr_inner()` with compound assignment check
- [ ] Add `parse_compound_assign()` desugaring method
- [ ] Add `is_shift_right_assign()` to cursor
- [ ] Update `mistakes.rs`: remove compound assignment from common-mistake detection (keep `??=`)
- [ ] Write raw scanner tests for all 12 new token forms
- [ ] Write cooker tests for all 12 mappings
- [ ] Write parser tests: basic compound assign, field access, index, nested, precedence of RHS
- [ ] Write parser tests: `>>=` synthesis from `>` `>` `=`
- [ ] Write spec tests: `tests/spec/operators/compound_assignment/`

### Phase 3: Polish

- [ ] Run `./test-all.sh`, `./clippy-all.sh`, `./fmt-all.sh`
- [ ] Run benchmarks to verify no regression: `cargo bench -p oric --bench lexer_core`, `cargo bench -p oric --bench parser`
- [ ] Verify `>>=` does not break generic syntax: `let x: Result<Result<int, str>, str>` still parses
- [ ] Verify `&&=` and `||=` short-circuit correctly via spec tests
- [ ] Update spec: `grammar.ebnf` with compound assignment grammar rule
- [ ] Update spec: `operator-rules.md` with compound assignment desugaring rules
- [ ] Update `.claude/rules/ori-syntax.md` with compound assignment syntax

## References

### Ori Codebase (files studied)

- `compiler/ori_lexer_core/src/tag/mod.rs` -- `RawTag` enum, discriminant layout, lexeme/name methods
- `compiler/ori_lexer_core/src/raw_scanner/mod.rs` -- Scanner dispatch, operator methods, lookahead patterns
- `compiler/ori_lexer/src/cooker/mod.rs` -- `TokenCooker::cook()`, raw-to-cooked mapping
- `compiler/ori_ir/src/token/tag.rs` -- `TokenTag` discriminant layout, MAX_DISCRIMINANT assertion
- `compiler/ori_ir/src/token/kind.rs` -- `TokenKind` enum, TAG_* constants, discriminant_index()
- `compiler/ori_ir/src/ast/expr.rs` -- `ExprKind` variants, `Assign` node
- `compiler/ori_ir/src/ast/operators.rs` -- `BinaryOp` enum, trait_method_name(), precedence()
- `compiler/ori_parse/src/grammar/expr/mod.rs` -- Pratt parser, parse_expr_inner(), assignment handling
- `compiler/ori_parse/src/grammar/expr/operators.rs` -- define_operators! macro, OPER_TABLE, OperInfo
- `compiler/ori_parse/src/grammar/expr/postfix.rs` -- POSTFIX_BITSET, is_postfix_tag()
- `compiler/ori_parse/src/cursor/mod.rs` -- is_shift_right(), is_greater_equal(), adjacency checks
- `compiler/ori_parse/src/error/mistakes.rs` -- Common mistake detection for compound assignment

### Reference Compilers

- **Go**: `cmd/compile/internal/syntax/scanner.go` -- `goto assignop` pattern, operator metadata carriage
- **Rust**: `compiler/rustc_lexer/src/lib.rs` -- Single-char raw tokens; `compiler/rustc_parse/src/parser/expr.rs` -- Parser-level compound operator reconstruction, `AssignOpKind` enum
- **TypeScript**: `src/compiler/scanner.ts` -- Dense `SyntaxKind` enum, `reScanGreaterToken()` re-scanning; `src/compiler/parser.ts` -- Precedence table
- **Zig**: `src/Tokenizer.zig` -- Explicit state machine, compound lookahead states

### Design Documents

- `docs/ori_lang/proposals/approved/compound-assignment-proposal.md` -- Approved proposal specifying 13 operators, desugaring semantics, mutability requirements
- `docs/ori_lang/0.1-alpha/spec/operator-rules.md` -- Operator typing and evaluation rules
- `docs/ori_lang/0.1-alpha/spec/grammar.ebnf` -- Formal grammar (to be updated)
