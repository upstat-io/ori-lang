---
plan: "dpr_parser_02212026"
title: "Design Pattern Review: Parser Architecture for Growth"
status: draft
---

# Design Pattern Review: Parser Architecture for Growth

## Ori Today

Ori's parser is a recursive descent + Pratt hybrid built on a cursor-driven token stream: `TokenList` flows through `Cursor` (stateful navigation with tag-based O(1) checks) into `Parser` (which delegates to grammar modules) and produces an `ExprArena` (flat arena-allocated AST with `ExprId` handles). The Pratt loop in `grammar/expr/mod.rs::parse_binary_pratt()` uses a static `OPER_TABLE[128]` indexed by token discriminant tag, giving O(1) binding power lookup for 18 binary operators. The four-way `ParseOutcome<T>` (ConsumedOk / EmptyOk / ConsumedErr / EmptyErr) from `outcome/mod.rs` enables Elm-style automatic backtracking: `one_of!` tries alternatives on EmptyErr, stops on ConsumedErr. Context-sensitive parsing uses a `ParseContext` u16 bitfield (9 flags: `NO_STRUCT_LIT`, `IN_LOOP`, `PIPE_IS_SEPARATOR`, etc.) scoped via `with_context()` / `without_context()` RAII-style closures. Error recovery uses bitset-based `TokenSet` (u128) for O(1) membership and `synchronize()` to skip to recovery points.

What works well: the Pratt parser reduces the precedence chain from 12+ nested calls to ~4 per expression. `ParseOutcome` eliminates most explicit backtracking. The tag-based cursor (`current_tag()` returns a `u8` discriminant) enables O(1) dispatch in both the operator table and the primary expression fast path. Arena allocation (`ExprArena::with_capacity(source_len / 20)`) keeps memory compact and cache-friendly. Pre-interned `KnownNames` avoid interner lock contention for contextual keywords. The postfix loop uses a compile-time `POSTFIX_BITSET` (two `u64`s) for O(1) early exit.

What's missing or strained: `primary.rs` at 1650 lines is the parser's biggest bottleneck for growth -- every new expression form adds another branch to `parse_primary()`, and the file already carries `#[expect(clippy::too_many_lines)]`. The operator table (`OPER_TABLE`, `op_from_u8()`, `BinaryOp` enum) requires manual 3-way sync with no compile-time validation -- adding an operator means updating three locations and hoping they match. Block statement parsing (`collect_block_stmts()` in `blocks.rs`) is shared between `parse_block_expr_body()` and `parse_try_block()`, but `parse_match_expr_body()` in `patterns.rs` reinvents parts of it. The `ErrorContext` enum has 30 variants and `in_error_context()` is used at ~15 call sites, but many sub-parsers that would benefit from context wrapping (postfix operations, call argument parsing, binary operators) don't use it. There is no compile-time or test-time validation that the parser, type checker, and evaluator all handle the same set of `ExprKind` variants -- drift is caught only when a new expression hits a `_ => unreachable!()` arm in production. The language currently has ~53 `ExprKind` variants. At the current growth rate (unsafe blocks, contracts, channels added in the last month), it will exceed 70 within a few months and push well past the boundary where Zig's monolithic approach (3750 lines for ~180 node types) starts to buckle.

## Prior Art

### Rust -- Submodule Decomposition with Restrictions Bitflags

Rust's parser (`rustc_parse/src/parser/`, 24K LOC) splits into 14 submodules: `expr.rs` (4315L), `item.rs` (3423L), `pat.rs`, `ty.rs`, `stmt.rs`, `path.rs`, `attr.rs`, `generics.rs`, and `diagnostics.rs` (3146L for error recovery alone). The key scaling insight is a `Restrictions` bitflags struct (u8, 6 flags) that gates parsing behavior: `STMT_EXPR` stops parsing at struct-like tokens in statement position, `NO_STRUCT_LITERAL` prevents struct literals in conditions, `CONST_EXPR` restricts to compile-time evaluable forms. Expression parsing dispatches via `parse_expr_res(restrictions, attrs)` which scopes restrictions and pre-collected attributes together. This pattern has scaled from Rust 1.0 (~120 syntax forms) to today (~250+) without redesigning the core loop. Rust's `SnapshotParser` pattern (capture full parser state including diagnostic state, attempt parse, revert on failure) is heavy but enables sophisticated error recovery -- trying multiple parse paths and choosing the one that produced the best error. This is overkill for Ori's 53 forms but becomes necessary around 150+ when ambiguities multiply.

### Zig -- Compile-Time Operator Table with SoA AST

Zig's parser (`Parse.zig`, 3750L) is a single monolithic file but its operator handling is clean: `operTable` is a compile-time `directEnumArrayDefault` indexed by `Token.Tag`, mapping every token tag to `OperInfo { prec, tag }` with a sentinel default of `{ .prec = -1 }`. The `parseExprPrecedence(min_prec)` function is a textbook Pratt loop that reads `operTable[@intFromEnum(tok_tag)]` for O(1) dispatch. The novel contribution is `banned_prec` -- a flag that bans chained comparisons (`a < b < c` is illegal) by checking if the current operator is at banned precedence. Zig's AST uses Struct-of-Arrays (SoA) layout where each node field lives in a separate contiguous array, enabling SIMD-friendly iteration and compact memory. All 244 `Node.Tag` variants live in one enum with deterministic discriminant values, and the monolithic file keeps everything inlineable. This works because Zig's syntax is deliberately simple; the single-file approach would not scale to Ori's more complex expression forms (template literals, capability provision, function expressions).

### Gleam -- Stack-Based Operator Parsing with ExpressionUnitContext

Gleam's parser (`parse.rs`, 5081L) uses a two-stack simple precedence algorithm instead of Pratt recursion: `parse_expression_inner()` maintains `opstack` (operators) and `estack` (expression units), accumulating via `handle_op()` which reduces the stacks when precedence allows. The decoupling of operator precedence from call stack depth means Gleam never risks stack overflow on deeply nested operator chains. But the key takeaway for Ori is `ExpressionUnitContext` -- a context object passed to `parse_expression_unit()` that tracks whether the parser is inside a case clause, list, etc. This avoids backtracking entirely: the parser commits at token boundaries because the context tells it what's valid. Gleam also has 150+ `ParseErrorType` variants with specific context for each error, giving precise diagnostics without a separate diagnostics module. The downside: Gleam's error types are ad-hoc strings in many places, making them hard to test programmatically.

### TypeScript -- Deferred Disambiguation

TypeScript's parser (`parser.ts`, 15K LOC) embodies a philosophy: parse liberally, error in the type checker. `parseBinaryExpression(minPrecedence)` looks up `getBinaryOperatorPrecedence(tokenKind)` for Pratt-style dispatch, but the interesting pattern is how ambiguity is handled -- the parser builds a permissive AST and lets later phases reject invalid combinations. For example, `<T>` in expression position could be a generic call or a JSX element; the parser picks one interpretation and the type checker validates. This avoids complex lookahead in the parser at the cost of duplicated validation in later phases. For Ori, this pattern is relevant for capability expressions (`with Capability = Provider in body`) and future macro syntax where parse-time disambiguation would require unbounded lookahead. The key lesson: don't solve every ambiguity in the parser if a later phase can resolve it more cheaply.

## Proposed Best-of-Breed Design

### Core Idea

Ori should evolve its parser along three axes, each borrowed from a different compiler: (1) **category-based module splitting** from Rust, applied to `primary.rs` and `patterns.rs` now and to `lib.rs` when it exceeds 500L of parse logic; (2) **compile-time validated operator registration** inspired by Zig's `operTable` but adapted to Rust's macro system and Ori's `OPER_TABLE` + `BinaryOp` sync; (3) **systematic error context propagation** extending Ori's existing `in_error_context()` / `ErrorContext` infrastructure to cover all committed parse paths, following Gleam's pattern of context-aware error messages.

The Pratt loop, `ParseOutcome`, arena allocation, and tag-based cursor are all working well and should not change. The proposal focuses exclusively on the scaling bottlenecks: file organization, registration sync, and error quality. The parser's current architecture is sound at 53 expression forms; the goal is to keep it sound at 100+ without requiring a redesign.

### Key Design Choices

1. **Split `primary.rs` into category submodules** (Rust: `expr.rs` split into submodules at ~200 forms). Extract `parse_primary()` into a dispatcher that calls into submodule functions. Five categories emerge naturally from the existing code:
   - `primary/literals.rs`: `parse_literal_primary()`, `parse_template_literal()` (~100L)
   - `primary/identifiers.rs`: `parse_ident_primary()`, `parse_variant_primary()`, `parse_misc_primary()`, `match_channel_kind()` (~180L)
   - `primary/control.rs`: `parse_if_expr()`, `parse_for_loop()`, `parse_loop_expr()`, `parse_control_flow_primary()`, `parse_unsafe_expr()`, `parse_with_capability()` (~400L)
   - `primary/bindings.rs`: `parse_let_expr()`, `parse_binding_pattern()`, `exprs_to_params()`, `is_typed_lambda_params()` (~250L)
   - `primary/collections.rs`: `parse_parenthesized()`, `parse_list_literal()`, `parse_block_or_map()`, `parse_map_literal_body()` (~350L)
   - `primary/mod.rs`: The fast-path tag dispatch and `one_of!` fallback (~80L)

   This brings every file under 500L and makes it clear where new syntax goes. The fast-path `match` in `parse_primary()` stays in `mod.rs` as a thin dispatcher.

2. **Macro-generated operator table with compile-time sync** (Zig: `directEnumArrayDefault`). Replace the manual 3-way sync (`OPER_TABLE` entries, `op_from_u8()` match arms, `BinaryOp` enum) with a declarative macro:

   ```rust
   define_operators! {
       DoubleQuestion => Coalesce,  bp::COALESCE, 1;
       PipePipe       => Or,        bp::OR,       1;
       AmpAmp         => And,       bp::AND,      1;
       // ...
   }
   ```

   The macro generates: the `OPER_TABLE` static, the `op_from_u8()` function, and a compile-time assertion that every `BinaryOp` variant appears exactly once. If a developer adds a `BinaryOp::Power` variant but forgets the table entry, the assertion fails at compile time.

3. **Exhaustive `ErrorContext` wrapping on all committed paths** (Gleam: context-aware errors). Currently only ~15 of ~40 committed parse paths use `in_error_context()`. Wrap all remaining committed paths (postfix operations, call arguments, type parsing, attribute parsing). This means every `ConsumedErr` carries context like "while parsing a method call" or "while parsing call arguments". The existing `ErrorContext` enum already has variants for most of these; the work is mechanical wrapping.

4. **Cross-phase `ExprKind` exhaustiveness test** (Ori's own registration sync pattern from iterator dispatch). Add a test in `oric/tests/` that:
   - Collects all `ExprKind` variants via an `ExprKind::all_variants()` test-only method
   - Verifies that `ori_types`'s expression type-checking has a branch for each variant
   - Verifies that `ori_eval`'s expression evaluation has a branch for each variant
   - Verifies that `ori_llvm`'s codegen has a branch for each variant (or an explicit `unimplemented!()` with a tracking issue)

   This catches drift at test time rather than in production. Follows the same pattern as `CollectionMethod::all_iterator_variants()` and `TYPECK_BUILTIN_METHODS`.

5. **`ParseContext` flag for pipe-separator contexts** (Rust: `Restrictions` bitflags for context gating). Ori already has `PIPE_IS_SEPARATOR` in `ParseContext`. Generalize this pattern: when adding new context-sensitive tokens (e.g., `by` as range step vs identifier, `in` as for-loop keyword vs identifier), add flags to `ParseContext` rather than ad-hoc `if` chains in the Pratt loop. The u16 bitfield has 7 unused bits, enough for the foreseeable future.

6. **Deferred disambiguation for capability and macro syntax** (TypeScript: parse permissively, validate later). For `with Capability = Provider in body`, the current parser commits early via `is_with_capability_syntax()` (3-token lookahead). As capability syntax grows more complex (nested provisions, computed providers), shift validation to the type checker rather than extending parser lookahead. Parse a generic `with ... in ...` form and let the type checker reject invalid capability expressions.

7. **Shared block statement parsing via trait or helper** (extracted from Gleam's `ExpressionUnitContext`). `collect_block_stmts()` is currently shared between `parse_block_expr_body()` and `parse_try_block()`. Match expressions should reuse this too (their arm bodies are block-like). Parameterize `collect_block_stmts()` on a `BlockConfig` struct that specifies terminator, separator policy, and whether `let` bindings are allowed.

### What Makes Ori's Approach Unique

Ori's `ParseOutcome<T>` is more principled than any reference compiler's error handling. Rust uses `PResult<T>` (plain `Result`) and relies on diagnostic stashing for error recovery. Zig uses `warnMsg()` and continues. TypeScript silently skips. Gleam uses `Result<Option<T>>` which conflates "not present" with "present but errored." Ori's four-way split -- ConsumedOk/EmptyOk/ConsumedErr/EmptyErr -- encodes both progress and success independently, enabling the `one_of!` macro to make correct backtracking decisions without any explicit state management. This is Ori's core parser innovation and it should be preserved and extended, not replaced.

The combination of `ParseOutcome` with the tag-based `POSTFIX_BITSET` and `OPER_TABLE` creates a unique fast-path architecture: the common case (identifier, literal, simple binary op, method call) touches only O(1) bitset checks and array lookups, while the uncommon case (backtracking, error recovery, context-sensitive keywords) falls through to the full `one_of!` + snapshot machinery. No reference compiler achieves this combination -- Zig has fast dispatch but no backtracking; Rust has snapshot recovery but no fast-path bitsets; Gleam has stack-based operators but tag-based dispatch is absent.

Ori's expression-based nature (no `return` keyword, last expression is block value) means block parsing is more nuanced than in statement-based languages. The `collect_block_stmts()` function must track whether the last expression has a trailing semicolon to determine if it's a statement or the block's value. This interplay between statements and the final expression is unique to Ori among the reference compilers (Rust has it but with an explicit `return` escape hatch). The `BlockConfig` parameterization proposed in choice 7 should preserve this semantics precisely.

### Concrete Types & Interfaces

**Operator registration macro:**

```rust
/// Declarative operator table: one line per operator, generates all
/// registration artifacts and a compile-time exhaustiveness check.
macro_rules! define_operators {
    ($(
        $tag_const:ident => $op:ident, $bp:expr, $token_count:expr;
    )*) => {
        // Generate OPER_TABLE static
        static OPER_TABLE: [OperInfo; 128] = {
            let mut table = [OperInfo::NONE; 128];
            let mut _idx: u8 = 0;
            $(
                table[TokenKind::$tag_const as usize] =
                    OperInfo::new($bp.0, $bp.1, _idx, $token_count);
                _idx += 1;
            )*
            table
        };

        // Generate op_from_u8
        #[inline]
        fn op_from_u8(op: u8) -> BinaryOp {
            let mut _idx: u8 = 0;
            $(
                if op == { let v = _idx; _idx += 1; v } {
                    return BinaryOp::$op;
                }
            )*
            unreachable!("invalid op index: {op}")
        }

        // Compile-time count assertion (optional: match BinaryOp exhaustively)
        #[cfg(test)]
        const _OPERATOR_COUNT: usize = {
            let mut count = 0u32;
            $( let _ = BinaryOp::$op; count += 1; )*
            count as usize
        };
    };
}

// Usage:
define_operators! {
    TAG_DOUBLE_QUESTION => Coalesce,  bp::COALESCE,       1;
    TAG_PIPEPIPE        => Or,        bp::OR,             1;
    TAG_AMPAMP          => And,       bp::AND,            1;
    TAG_PIPE            => BitOr,     bp::BIT_OR,         1;
    TAG_CARET           => BitXor,    bp::BIT_XOR,        1;
    TAG_AMP             => BitAnd,    bp::BIT_AND,        1;
    TAG_EQEQ            => Eq,        bp::EQUALITY,       1;
    TAG_NOTEQ           => NotEq,     bp::EQUALITY,       1;
    TAG_LT              => Lt,        bp::COMPARISON,     1;
    TAG_LTEQ            => LtEq,      bp::COMPARISON,     1;
    TAG_GT              => Gt,        bp::COMPARISON,     1;
    TAG_SHL             => Shl,       bp::SHIFT,          1;
    TAG_PLUS            => Add,       bp::ADDITIVE,       1;
    TAG_MINUS           => Sub,       bp::ADDITIVE,       1;
    TAG_STAR            => Mul,       bp::MULTIPLICATIVE,  1;
    TAG_SLASH           => Div,       bp::MULTIPLICATIVE,  1;
    TAG_PERCENT         => Mod,       bp::MULTIPLICATIVE,  1;
    TAG_DIV             => FloorDiv,  bp::MULTIPLICATIVE,  1;
}
```

**Primary expression category dispatch (post-split `primary/mod.rs`):**

```rust
// primary/mod.rs -- thin dispatcher only
impl Parser<'_> {
    pub(crate) fn parse_primary(&mut self) -> ParseOutcome<ExprId> {
        // Context-sensitive keywords requiring multi-token lookahead
        // (these stay here because they need cursor state before dispatching)
        if let Some(outcome) = self.try_context_sensitive_primary() {
            return outcome;
        }

        // Fast path: tag-based direct dispatch
        match self.cursor.current_tag() {
            TokenKind::TAG_INT | TokenKind::TAG_FLOAT | TokenKind::TAG_STRING
            | TokenKind::TAG_CHAR | TokenKind::TAG_TRUE | TokenKind::TAG_FALSE
            | TokenKind::TAG_DURATION | TokenKind::TAG_SIZE
                => self.parse_literal_primary(),

            TokenKind::TAG_IDENT | TokenKind::TAG_SUSPEND | TokenKind::TAG_EXTERN
                => self.parse_ident_primary(),

            TokenKind::TAG_TEMPLATE_FULL | TokenKind::TAG_TEMPLATE_HEAD
                => self.parse_template_literal(),

            TokenKind::TAG_LPAREN => self.parse_parenthesized(),
            TokenKind::TAG_LBRACKET => self.parse_list_literal(),
            TokenKind::TAG_LBRACE => self.parse_block_or_map(),

            TokenKind::TAG_IF => self.parse_if_expr(),
            TokenKind::TAG_LET => self.parse_let_expr(),
            TokenKind::TAG_LOOP => self.parse_loop_expr(),
            TokenKind::TAG_UNSAFE => self.parse_unsafe_expr(),

            TokenKind::TAG_SOME | TokenKind::TAG_NONE
            | TokenKind::TAG_OK | TokenKind::TAG_ERR
                => self.parse_variant_primary(),

            TokenKind::TAG_BREAK | TokenKind::TAG_CONTINUE | TokenKind::TAG_RETURN
                => self.parse_control_flow_primary(),

            TokenKind::TAG_DOLLAR | TokenKind::TAG_HASH
                => self.parse_misc_primary(),

            TokenKind::TAG_ERROR => self.consume_error_token(),

            _ => self.parse_primary_fallback(),
        }
    }

    /// Fallback: one_of! for soft-keyword cases not covered by fast path.
    fn parse_primary_fallback(&mut self) -> ParseOutcome<ExprId> {
        one_of!(self,
            self.parse_literal_primary(),
            self.parse_ident_primary(),
            self.parse_variant_primary(),
            self.parse_misc_primary(),
            self.parse_parenthesized(),
            self.parse_list_literal(),
            self.parse_block_or_map(),
            self.parse_if_expr(),
            self.parse_let_expr(),
            self.parse_loop_expr(),
            self.parse_for_loop(),
            self.parse_control_flow_primary(),
            self.parse_template_literal(),
        )
    }
}
```

**Block configuration struct:**

```rust
/// Configuration for block-like statement sequences.
///
/// Parameterizes `collect_block_stmts()` for reuse across block
/// expressions, try blocks, and match arm bodies.
pub(crate) struct BlockConfig<'a> {
    /// Human-readable name for error messages (e.g., "block", "try block").
    pub name: &'a str,
    /// Token that terminates the block (usually `RBrace`).
    pub terminator: TokenKind,
    /// Whether `let` bindings are allowed (false in some expression contexts).
    pub allow_let: bool,
}

impl Default for BlockConfig<'_> {
    fn default() -> Self {
        Self {
            name: "block",
            terminator: TokenKind::RBrace,
            allow_let: true,
        }
    }
}

impl BlockConfig<'_> {
    pub fn try_block() -> Self {
        Self {
            name: "try block",
            ..Self::default()
        }
    }
}
```

**Cross-phase exhaustiveness test:**

```rust
// oric/tests/consistency.rs (or extend existing consistency tests)

/// Verify that every ExprKind variant is handled by the type checker,
/// evaluator, and LLVM codegen (or explicitly marked unimplemented).
#[test]
fn expr_kind_exhaustiveness_across_phases() {
    // ExprKind::all_variant_names() returns &[&str] of variant names.
    // Each downstream phase provides a similar list of handled variants.
    let all = ExprKind::all_variant_names();
    let typeck_handled = ori_types::HANDLED_EXPR_KINDS;
    let eval_handled = ori_eval::HANDLED_EXPR_KINDS;

    for variant in all {
        assert!(
            typeck_handled.contains(variant),
            "ExprKind::{variant} is not handled in ori_types"
        );
        assert!(
            eval_handled.contains(variant),
            "ExprKind::{variant} is not handled in ori_eval"
        );
    }
}
```

## Implementation Roadmap

### Phase 1: Foundation (Structural) -- Do Now

These address the 1650-line `primary.rs` and other files already over the 500L limit. No behavioral changes; purely organizational.

- [ ] Create `grammar/expr/primary/` directory and split `primary.rs` into `mod.rs` + 5 category files (literals, identifiers, control, bindings, collections). Keep the fast-path tag dispatch in `mod.rs`. Total: ~80L dispatcher + 5 files each under 400L.
- [ ] Extract `try_context_sensitive_primary()` helper from the if-chain at the top of `parse_primary()` into `primary/mod.rs`. This isolates the lookahead-dependent cases (run, try, match, for, with, channel, function_exp) from the tag-dispatch cases.
- [ ] Move `parse_binding_pattern()` from `primary.rs` to `primary/bindings.rs` along with `exprs_to_params()`, `is_typed_lambda_params()`, and `parse_optional_label()`.
- [ ] Audit `patterns.rs` (1042L): extract match pattern parsing (`parse_match_pattern()`, variant/struct/list/tuple pattern sub-parsers) into `patterns/match_patterns.rs`, leaving `parse_try()`, `parse_match_expr()`, and `parse_function_exp()` in `patterns/mod.rs`.

### Phase 2: Core (Extensibility & Error Quality) -- Do Soon

These improve the parser's ability to absorb new syntax without drift.

- [ ] Implement `define_operators!` macro in `operators.rs`. Replace the hand-written `OPER_TABLE`, `op_from_u8()`, and individual constant assignments with a single macro invocation. Add a `#[cfg(test)]` assertion that the count of macro entries matches `BinaryOp::variant_count()`.
- [ ] Add `in_error_context()` wrapping to all committed parse paths currently missing it: `apply_postfix_ops()` (each branch), `parse_call_args()`, `parse_binary_pratt()` (operator error case), `parse_range_continuation()`, `parse_unary()`, `parse_type()`. Target: every `ConsumedErr` carries an `ErrorContext`.
- [ ] Add `ExprKind::all_variant_names()` test-only method to `ori_ir`. Add cross-phase consistency test in `oric/tests/consistency.rs` that verifies every `ExprKind` variant is handled (or explicitly listed as unimplemented) in `ori_types`, `ori_eval`, and `ori_llvm`.
- [ ] Parameterize `collect_block_stmts()` with `BlockConfig` struct. Migrate `parse_block_expr_body()` and `parse_try_block()` to use `BlockConfig`. Verify match arm bodies can reuse the same path.

### Phase 3: Polish (Performance & Validation) -- Do When Needed

These are investments that pay off at higher syntax counts (~80-100+ forms).

- [ ] Add a `#[cfg(test)]` assertion in `operators.rs` that every `BinaryOp` variant appears in the operator table, and every table entry maps to a valid `BinaryOp`. This catches the case where a new operator is added to the enum but not the table (or vice versa).
- [ ] Instrument `in_error_context()` with `tracing::debug!` to log parse context transitions. This enables `ORI_LOG=ori_parse=debug` to show a context trail like: `FunctionDef > Expression > IfExpression > BinaryOp`.
- [ ] Evaluate snapshot-based error recovery (Rust's `SnapshotParser` pattern) when Ori exceeds ~100 expression forms. Current `synchronize()` skip-to-recovery-point is sufficient for 53 forms but produces poor errors for nested constructs. The signal to invest: when users report "expected `}` but found `)`" errors that are unhelpful because the real problem is 10 tokens earlier.
- [ ] Evaluate Zig's SoA AST layout when benchmarks show cache pressure in the type checker or evaluator walking the `ExprArena`. Current AoS (each `Expr` is a struct with kind + span) is fine for arena-allocated nodes but SoA (separate arrays for kinds, spans, and child pointers) could improve iteration performance. The signal to invest: when profile-guided benchmarks show >10% time in cache misses during AST walks.
- [ ] Consider Gleam-style stack-based operator parsing if deeply nested expressions cause stack overflows in the Pratt loop. The `ensure_sufficient_stack()` guard in `parse_expr()` currently handles this, but if it fires frequently in production code (check via `tracing::warn!`), a non-recursive operator stack would eliminate the issue entirely.

## References

### Ori Parser (current implementation)
- `compiler/ori_parse/src/grammar/expr/mod.rs` -- Pratt parser loop, binding power constants
- `compiler/ori_parse/src/grammar/expr/operators.rs` -- `OPER_TABLE`, `op_from_u8()`, `OperInfo`
- `compiler/ori_parse/src/grammar/expr/primary.rs` -- Primary expression dispatch (1650L)
- `compiler/ori_parse/src/grammar/expr/postfix.rs` -- `POSTFIX_BITSET`, postfix loop
- `compiler/ori_parse/src/grammar/expr/patterns.rs` -- try, match, for, function_exp
- `compiler/ori_parse/src/grammar/expr/blocks.rs` -- `collect_block_stmts()` shared logic
- `compiler/ori_parse/src/outcome/mod.rs` -- `ParseOutcome`, `one_of!`, `chain!`, `committed!`, `require!`
- `compiler/ori_parse/src/context/mod.rs` -- `ParseContext` bitfield (u16, 9 flags)
- `compiler/ori_parse/src/recovery/mod.rs` -- `TokenSet` (u128 bitset), `synchronize()`
- `compiler/ori_parse/src/snapshot/mod.rs` -- `ParserSnapshot` for speculative parsing
- `compiler/ori_parse/src/error/context.rs` -- `ErrorContext` enum (30 variants)
- `compiler/ori_parse/src/error/kind.rs` -- `ParseErrorKind` structured error types
- `compiler/ori_parse/src/lib.rs` -- `Parser` struct, `with_context()`, `in_error_context()`
- `compiler/ori_parse/src/series/mod.rs` -- `SeriesConfig` combinator for delimited lists

### Reference compilers
- `~/projects/reference_repos/lang_repos/rust/compiler/rustc_parse/src/parser/mod.rs` -- `Restrictions` bitflags (u8), parser state, submodule organization
- `~/projects/reference_repos/lang_repos/rust/compiler/rustc_parse/src/parser/expr.rs` -- `parse_expr_res()`, precedence handling, attribute wrapping
- `~/projects/reference_repos/lang_repos/rust/compiler/rustc_parse/src/parser/diagnostics.rs` -- `SnapshotParser`, error stashing, recovery strategies
- `~/projects/reference_repos/lang_repos/zig/lib/std/zig/Parse.zig` -- `operTable` (line 1554), `parseExprPrecedence()`, `banned_prec`, SoA AST layout
- `~/projects/reference_repos/lang_repos/gleam/compiler-core/src/parse.rs` -- `parse_expression_inner()` two-stack algorithm (line 418), `ExpressionUnitContext`, `opstack`/`estack`
- `~/projects/reference_repos/lang_repos/typescript/src/compiler/parser.ts` -- `parseBinaryExpression()`, `getBinaryOperatorPrecedence()`, deferred disambiguation
