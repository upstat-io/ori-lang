---
paths:
  - "**parse**"
  - "**lex**"
---

# Parser & Lexer Formal Ruleset

This document defines the **laws** of the parser and lexer — the boundary between source text and structured AST. The spec (`docs/ori_lang/v2026/spec/grammar.ebnf`) defines **what** the language's syntax is; this document defines **how** the parser implements that syntax faithfully. If the code violates a rule stated here, the code has a bug.

**Relationship to other rulesets**: The parser produces an AST consumed by the type checker (`ori_types`), which feeds into evaluation (`ori_eval`) and ARC analysis (`aims-rules.md`), which feeds into codegen (`codegen-rules.md`). The parser is the first phase — its correctness is upstream of every other phase.

**Relationship to compiler.md and impl-hygiene.md**: Those files are *operational* guides (how to structure code, test, trace). This document is *normative* (what the parser must guarantee). When they conflict, this document is authoritative for parser-specific rules.

**Scope**: This ruleset covers the lexer (`ori_lexer`), parser (`ori_parse`), and their shared boundary. AST node definitions live in `ori_ir` and are outside this ruleset's scope.

**Target-only rules**: Rules marked **(target-only)** describe the COMPLETE target system per the spec. The implementation may not have shipped them yet. The spec is authoritative; code divergences are bugs to file, not spec inaccuracies. These annotations prevent reviewers from re-flagging known implementation gaps as spec issues.

---

## Notation

- **SHALL** = mandatory requirement (violation = implementation bug)
- **SHOULD** = recommended practice (violation = design smell, may be justified)
- Rules are numbered `CATEGORY-N`. Categories: `CU` (cursor/token consumption), `CF` (context flag), `PO` (parse outcome / progress), `ER` (error recovery), `PR` (Pratt / expression parsing), `LB` (lexer-parser boundary), `SE` (series combinator), `AR` (arena allocation), `DI` (diagnostics), `SN` (snapshot / speculative parsing), `DD` (declaration dispatch), `TR` (tracing), `KW` (context-sensitive keywords)
- Cross-references: `impl-hygiene.md` rules prefixed with `HYG:`, compiler.md rules prefixed with `COMP:`

---

## §1 Cursor & Token Consumption

The cursor module is the parser's sole interface to the token stream. All token access and consumption flows through the cursor.

Source: `ori_parse/src/cursor/mod.rs`.

### CU-1 — Core Primitives

All parsing ultimately composes from these foundational operations:

| Primitive | Signature | Behavior |
|-----------|-----------|----------|
| `check(kind)` | `fn check(&self, kind: &TokenKind) -> bool` | Lookahead — test current token kind without consuming. O(1) via tag array. |
| `advance()` | `fn advance(&mut self) -> &Token` | Unconditional consume — caller MUST have verified the token kind first. |
| `expect(kind)` | `fn expect(&mut self, kind: &TokenKind) -> Result<&Token, ParseError>` | Committed consume — advance if match, else error. |

Note: there is no generic `eat(kind)` cursor primitive. Optional/speculative consumption (e.g., `eat_optional_semicolon()`) is implemented as parser-level helpers in `dispatch.rs` and grammar modules, composing from `check()` + `advance()`.

### CU-2 — Derived Cursor Operations

The cursor provides higher-level operations composed from the core primitives. These are part of the parser's public cursor contract, not violations of it:

| Category | Methods | Purpose |
|----------|---------|---------|
| Tag access | `current_tag()`, `current_kind()` | Raw discriminant access (u8 tag vs full TokenKind) |
| Lookahead | `peek_kind_at(n)`, `peek_next_kind()` | Multi-token lookahead |
| Adjacency | `next_is_adjacent()`, `is_shift_right()`, `is_greater_equal()`, `is_shift_right_assign()` | Compound operator detection (PR-4) |
| Named checks | `next_is_lparen()`, `next_is_colon()`, `is_named_arg_start()` | Production disambiguation |
| Compound consume | `consume_compound()`, `consume_triple()` | Multi-token operator consumption |
| Whitespace | `skip_newlines()` | Newline-insensitive contexts |
| Identity | `check_ident()`, `expect_ident()` | Identifier-specific operations |

All derived operations SHALL be implemented in the cursor module. Grammar code SHALL NOT perform raw position arithmetic or direct token array access.

Rationale: Centralizing all token access in the cursor module makes the parser's progress semantics auditable. The cursor is the SSOT for token stream position.

---

## §2 Context Flags

The parser uses a bitset of context flags to handle syntactic ambiguity without grammar transformation. Context flags control which productions are available at a given parse point.

Source: `ori_parse/src/context/mod.rs`. Representation: `ParseContext(u16)`.

### CF-1 — Context Flag Catalog

The following flags SHALL be the complete set. Adding a new flag requires updating this catalog. Bit `1 << 3` is unused/reserved.

| Flag | Bit | Status | Purpose | Affected Productions |
|------|-----|--------|---------|---------------------|
| `IN_PATTERN` | `1 << 0` | Defined | Parsing a pattern (match arms, let bindings) | Identifier vs binding, literal interpretation |
| `IN_TYPE` | `1 << 1` | Defined | Parsing a type annotation | `>` closes generic instead of comparison |
| `NO_STRUCT_LIT` | `1 << 2` | Active | Struct literals forbidden | `if` conditions — prevents `{ }` ambiguity with blocks (also applies to future `while` conditions when `while` ships) |
| *(reserved)* | `1 << 3` | Unused | Reserved for future use | — |
| `IN_LOOP` | `1 << 4` | Active | Inside a loop body | Enables `break` and `continue` as valid expressions |
| `ALLOW_YIELD` | `1 << 5` | Defined | Yield expressions allowed | `for...yield` constructs |
| `IN_FUNCTION` | `1 << 6` | Defined | Inside a function body | Set by function/impl body parsers but not yet consulted by grammar productions (infrastructure for future use) |
| `IN_INDEX` | `1 << 7` | Active | Inside `[...]` index expression | Makes `#` valid as the length-of-collection symbol |
| `PIPE_IS_SEPARATOR` | `1 << 8` | Active | `\|` is a message separator, not bitwise OR | `pre(condition \| "message")` and `post()` contracts |

**Status legend**: *Active* = consulted by production grammar code. *Defined* = exists in `ParseContext` but not yet consulted by grammar code (infrastructure for future language features). All defined flags are part of the `ParseContext` API contract and SHALL NOT be removed without a deprecation cycle.

### CF-2 — Scoped Context Modification

Grammar code SHALL modify context flags only through the scoped combinators `with_context(flag, closure)` and `without_context(flag, closure)`. Direct mutation of `self.context` from grammar code is forbidden.

Exception: snapshot restore (`Parser::restore()`) directly reassigns `self.context` to the saved `ParseContext` value as part of speculative rollback. This is the sole legitimate direct mutation — it restores a previously-valid state rather than introducing new context.

Rationale: Scoped modification guarantees that context is restored after parsing a sub-expression, preventing flag leakage across productions. Snapshot restore is the dual mechanism for speculative paths.

### CF-3 — NO_STRUCT_LIT in Conditions

`NO_STRUCT_LIT` SHALL be set when parsing the condition of `if` (shipped) and any future control-flow construct where a struct literal inside the condition would complicate parsing.

Ori's actual `if` syntax uses the `then` keyword (`if cond then expr else expr`) — `parse_if_expr_body()` explicitly calls `cursor.expect(&TokenKind::Then)` after the condition. So the ambiguity between struct literal and block body does not actually bite in the shipped syntax. The `NO_STRUCT_LIT` flag is retained as a consistency / forward-compatibility measure: disallowing struct literals in condition position keeps the grammar predictable and future-safe (e.g., for `while` conditions that may not have a `then` delimiter).

Note: `while` (target-only) is spec'd to use the same mechanism. The parser does not currently have a `while` production — when it ships, it will follow the same `NO_STRUCT_LIT` discipline as `if`.

Spec: `grammar.ebnf` — `if_expr` (shipped, uses `then`), `while_expr` (target-only).

### CF-4 — PIPE_IS_SEPARATOR in Contracts

`PIPE_IS_SEPARATOR` SHALL be set when parsing `pre()` and `post()` contract expressions. In contract context, `|` introduces a message string (`pre(x > 0 | "x must be positive")`) rather than acting as bitwise OR. The flag is scoped to the contract body only. The Pratt table's `infix_binding_power()` (PR-1) checks this flag and suppresses `|` as an operator when set.

---

## §3 Parse Outcome (Progress Tracking)

The parser uses a four-way result type that encodes both progress (consumed vs empty) and success (ok vs error). This is the Elm/Roc-inspired design that enables automatic backtracking without explicit lookahead.

Source: `ori_parse/src/outcome/mod.rs`.

### PO-1 — Four-Way Outcome

`ParseOutcome<T>` SHALL have exactly four variants:

| Variant | Progress | Result | Meaning |
|---------|----------|--------|---------|
| `ConsumedOk { value: T }` | Consumed | Ok | Committed to path, succeeded |
| `EmptyOk { value: T }` | Empty | Ok | Optional content absent, succeeded |
| `ConsumedErr { error, consumed_span }` | Consumed | Err | Hard error — committed, cannot backtrack |
| `EmptyErr { expected: TokenSet, position }` | Empty | Err | Soft error — try next alternative |

### PO-2 — Backtracking Semantics

The combination of progress and result SHALL determine backtracking strategy:

- **`ConsumedErr`**: the parser has consumed tokens and committed to a production. This is a **hard error** — report it, synchronize (ER-1), and continue. Do NOT try alternative productions.
- **`EmptyErr`**: the parser has not consumed any tokens. This is a **soft error** — restore position and try the next alternative in the `one_of!` list. The `expected` field accumulates valid token sets across alternatives for diagnostic purposes.

Rationale: This is the key insight from Elm's parser — progress information coupled with the result type makes backtracking decisions automatic and correct by construction. No explicit lookahead count is needed.

### PO-3 — Composition Macros

Five macros SHALL implement the outcome composition patterns:

| Macro | Purpose | Input Type | On Soft Error | On Hard Error |
|-------|---------|-----------|---------------|---------------|
| `one_of!(self, alt1, alt2, ...)` | Try alternatives in order | `ParseOutcome` | Restore position, try next | Return immediately |
| `try_outcome!(self, expr)` | Optional element | `ParseOutcome` | Restore, return `None` | Propagate |
| `require!(self, expr, context)` | Mandatory element after commitment | `ParseOutcome` | Convert to `ConsumedErr` with context | Propagate |
| `chain!(self, expr)` | Sequential composition — extract value, propagate errors | `ParseOutcome` | Propagate `EmptyErr` | Propagate `ConsumedErr` |
| `committed!(expr)` | Bridge `Result<T, ParseError>` after commitment | `Result` | N/A (Result, not Outcome) | Convert to `ConsumedErr` |

`chain!` and `committed!` serve analogous roles (extract-or-propagate) but differ in input type: `chain!` takes `ParseOutcome` from grammar functions, `committed!` takes `Result` from cursor functions (`expect()`, `series()`).

### PO-4 — one_of! Exhaustive Alternatives

`one_of!` SHALL try each alternative in declaration order. For each alternative:
1. Save parser snapshot
2. Evaluate the alternative
3. If `ConsumedOk` or `EmptyOk` → return immediately (first match wins)
4. If `ConsumedErr` → return immediately (committed error, no backtracking)
5. If `EmptyErr` → restore snapshot, accumulate `expected` tokens, try next
6. If all alternatives produce `EmptyErr` → return merged `EmptyErr` with union of all `expected` sets

Rationale: First-match semantics with accumulated expected sets provides both unambiguous parsing and high-quality diagnostics ("expected one of: ..., ..., or ...").

### PO-5 — Return Type Selection

Parse functions SHALL use the appropriate return type for their role:

| Return Type | When To Use |
|-------------|-------------|
| `ParseOutcome<T>` | Entry points, alternative-bearing productions |
| `Result<T, ParseError>` | Internal helpers after commitment point |
| `Option<T>` | Lightweight type parsing, optional elements |
| `ParsedAttrs + &mut Vec<ParseError>` | Attribute parsing (always produces a value) |

---

## §4 Error Recovery

The parser uses bitset-based synchronization to recover from errors and continue parsing, maximizing the number of diagnostics per compilation pass.

Source: `ori_parse/src/recovery/mod.rs`.

### ER-1 — TokenSet Representation

`TokenSet` SHALL use a `[u128; 2]` bitset (256 bits) for O(1) membership testing. Each `TokenKind` discriminant index maps to a single bit. All set operations (membership, union, intersection) are O(1) bitwise operations.

Rationale: The 256-bit bitset covers all possible `u8` discriminant values. O(1) membership testing is critical because recovery checks run on every error — linear scanning would dominate error-path performance.

### ER-2 — Pre-Defined Recovery Sets

The following recovery sets SHALL be defined as compile-time constants:

| Set | Members | Purpose |
|-----|---------|---------|
| `STMT_BOUNDARY` | `@`, `use`, `type`, `trait`, `impl`, `def`, `pub`, `let`, `$`, `extend`, `extern`, `extension`, `EOF` | Skip to next top-level declaration after error |
| `FUNCTION_BOUNDARY` | `@`, `EOF` | Skip to next function definition after error inside a function |

Additional recovery sets MAY be defined as needed for specific productions.

### ER-3 — Synchronization Protocol

`synchronize(cursor, recovery_set)` SHALL advance the cursor until either:
1. The current token is in `recovery_set` → return `true` (recovery point found)
2. EOF is reached → return `false` (no recovery possible)

The synchronization function SHALL NOT consume the recovery token itself — it leaves the cursor positioned AT the recovery token for the caller to resume parsing.

### ER-4 — Recovery Strategy

Parser error recovery primarily accumulates diagnostics and synchronizes token flow. The parser does NOT generally synthesize error AST nodes — `ExprKind::Error` is currently produced only when the lexer emits a `TokenKind::Error` token (e.g., for malformed literals). General parse errors are collected in the deferred error list; the parser synchronizes to a recovery boundary and continues.

Recovery monotonicity (HYG: Error Recovery Monotonicity): recovery in the parser SHALL NOT create work for later phases. Error expressions propagate silently through type checking (via `TyError` poison type) without generating cascading diagnostics.

---

## §5 Pratt Parsing (Expression Parsing)

Expressions are parsed using a Pratt parser (precedence climbing) with a static binding power table. This is the core expression parsing algorithm.

Source: `ori_parse/src/grammar/expr/mod.rs`, `ori_parse/src/grammar/expr/operators.rs`.

### PR-1 — Binding Power Table

Binary operators SHALL be dispatched via a static lookup table (`OPER_TABLE`) indexed by token discriminant tag. Table entry format: `(left_bp, right_bp, op_variant, token_count)`. A `left_bp` of 0 means "not an operator."

The binding power values SHALL match the spec's precedence table (`operator-rules.md`). Currently shipped operators:

| Precedence | Operators | Left BP | Right BP | Associativity |
|------------|-----------|---------|----------|---------------|
| 15 (lowest shipped) | `??` | 2 | 1 | Right |
| 14 | `\|\|` | 3 | 4 | Left |
| 13 | `&&` | 5 | 6 | Left |
| 12 | `\|` (bitwise) | 7 | 8 | Left |
| 11 | `^` | 9 | 10 | Left |
| 10 | `&` (bitwise) | 11 | 12 | Left |
| 9 | `==` `!=` | 13 | 14 | Left |
| 8 | `<` `>` `<=` `>=` | 15 | 16 | Left |
| 7 | `..` `..=` | 17 | — | Non-associative (special) |
| 6 | `<<` `>>` | 19 | 20 | Left |
| 5 | `+` `-` | 21 | 22 | Left |
| 4 (highest shipped) | `*` `/` `%` `div` | 23 | 24 | Left |

**Not yet shipped (target-only)**: `**` (power, precedence 2, right-associative), `@` (matrix multiply, precedence 4, left-associative), `|>` (pipe, precedence 16, lowest). The spec defines these; the parser does not yet lex or parse them. `@` exists only as the `@=` compound assignment operator. `MatMul` is a `BinaryOp` variant but is listed as a special case (not wired as infix).

Left-associative operators have `left_bp < right_bp`. Right-associative operators have `left_bp > right_bp`. The gap at binding power 18 separates the non-associative range operator from shift operators.

Spec: `docs/ori_lang/v2026/spec/operator-rules.md` §Precedence Table.

### PR-2 — Associativity Encoding

Associativity SHALL be encoded in the binding power pair:
- **Left-associative**: `left_bp < right_bp` (e.g., `+` has `(21, 22)`) — the left operand binds tighter, so `a + b + c` parses as `(a + b) + c`.
- **Right-associative**: `left_bp > right_bp` (e.g., `??` has `(2, 1)`) — the right operand binds tighter, so `a ?? b ?? c` parses as `a ?? (b ?? c)`.
- **Non-associative**: range operators (`..`, `..=`) have a single binding power value (17) and are parsed with special-case logic that rejects chaining (`a..b..c` is an error).

### PR-3 — Pratt Loop Structure

The expression parser SHALL follow this structure:
1. Parse a primary expression (literal, identifier, parenthesized, prefix unary, block, etc.)
2. Apply postfix operators (`.field`, `.method()`, `[index]`, `(args)`, `?`, `as`, `as?`)
3. Enter the infix loop: while the current token's `left_bp >= min_bp`, consume the operator, parse the right-hand side recursively with the operator's `right_bp` as the new `min_bp`, and build a `BinaryOp` node. (The loop terminates on `left_bp < min_bp`.)
4. Return the accumulated expression

### PR-4 — Compound Operator Synthesis

Operators composed of multiple tokens SHALL be synthesized by the parser at the expression level, not by the lexer:

| Token Sequence | Synthesized Operator | Condition |
|----------------|---------------------|-----------|
| `>` + `>` | `>>` (shift right) | Adjacent, no whitespace |
| `>` + `=` | `>=` (greater-equal) | Adjacent, no whitespace |
| `>` + `>` + `=` | `>>=` (shift-right-assign) | Adjacent, no whitespace; checked BEFORE `>>` |

The `>>=` check SHALL take priority over `>>` to prevent greedy consumption of `>>` when `>>=` was intended.

Rationale: This interacts with LB-1 — `>` is always a single token so that nested generics (`Result<Result<T, E>, E>`) parse correctly. The parser synthesizes multi-token operators only in expression context.

### PR-5 — Compound Assignment

Compound assignment operators (`+=`, `-=`, `*=`, `/=`, `%=`, `@=`, `&=`, `|=`, `^=`, `<<=`, `&&=`, `||=`) SHALL be parsed at the top level of the expression parser, ABOVE the Pratt loop. Compound assignment is a statement-level construct with the lowest precedence — it is NOT part of the Pratt binding power table.

The `>>=` compound assignment is synthesized from three adjacent tokens (`>` `>` `=`) and handled separately from the single-token compound assignment operators.

### PR-6 — Unary Operator Parsing

Expression parsing has three layers in order: **Pratt infix loop → unary layer → primary + postfix**. Prefix unary operators (`-`, `!`, `~`) SHALL be handled by `parse_unary()`, a distinct layer entered from `parse_binary_pratt()` before falling through to `parse_call()` (which handles primary + postfix). This places unary parsing ABOVE primary/postfix in precedence, which is the correct Pratt-parser layering for prefix operators.

Postfix `?` (try operator) SHALL be parsed in the postfix chain (`parse_call()`), NOT as a unary operator — it produces `ExprKind::Try`, not a `UnaryOp` node.

Source: `ori_parse/src/grammar/expr/mod.rs` — `parse_unary()`, `parse_call()`; postfix handling in `postfix.rs`.

### PR-7 — Operator Table Exhaustiveness

The operator table SHALL be validated by a compile-time-enforced exhaustive test (`table_covers_all_non_special_operators`) that asserts: table entries + special-case count = total `BinaryOp` variants. Adding a new `BinaryOp` variant without updating the table is a compilation failure.

Rationale: The static table is a registration sync point — its exhaustiveness is enforced per `impl-hygiene.md` §Registration Sync Points.

---

## §6 Lexer-Parser Boundary

The lexer and parser are separate phases with a clean boundary. The lexer produces a flat token stream; the parser builds structured AST from it.

### LB-1 — Single-Token `>`

The lexer SHALL always emit `>` as a single token — never `>>` or `>=`. The parser synthesizes multi-character operators from adjacent single tokens in expression context (PR-4).

Rationale: This design enables parsing nested generics like `Result<Result<T, E>, E>` where two adjacent `>` tokens close two generic parameter lists. If the lexer emitted `>>`, the parser could not distinguish "close two generics" from "shift right." This is the same approach used by Rust, C++, and Java.

### LB-2 — Token Representation

Tokens are represented as `Token { kind: TokenKind, span: Span }` in `ori_ir/src/token/mod.rs`. The `TokenList` in `ori_ir/src/token/list.rs` stores tokens alongside parallel arrays:
- `tokens: Vec<Token>` — full token data (kind + span)
- `tags: Vec<u8>` — parallel discriminant tags for O(1) lookahead (LB-3)
- `flags: Vec<TokenFlags>` — per-token flags (e.g., adjacency to previous token)

Source text is borrowed from the Salsa-managed source string via `Span`. Token-level string interning for identifiers happens at lex time via the `Name` interner. Tokens do NOT carry string copies.

### LB-3 — Tag-Parallel Array

The cursor SHALL use the parallel `u8` tag array for O(1) discriminant comparison in all hot-path lookahead operations (`check()`, `current_tag()`, `infix_binding_power()`). The full `TokenKind` enum is touched only when the actual kind value is needed (e.g., extracting literal values).

Rationale: The tag array is ~1/16th the size of the token array (1 byte vs 16+ bytes per token), fitting entirely in L1 cache for typical source files. This is the parser's primary performance optimization — every `check()` call in the hot parse loop benefits.

### LB-4 — String Interning at Lex Time

All identifiers SHALL be interned into `Name` values at lex time. The parser receives interned `Name` handles, never raw strings. Identifier comparison is O(1) index equality, not O(n) string comparison.

Rationale: Per `impl-hygiene.md` §Interning Discipline: "Identifiers: always `Name` (interned at lex time). Never compare identifier `String`s directly."

### LB-5 — Lexer Phase Purity

The lexer SHALL perform scanning with minimal local state (nesting depth, mode stack) and produce tokens. The lexer SHALL NOT perform name resolution, semantic validation, or any operation that requires knowledge beyond the current token and its local context.

Rationale: Per `compiler.md` §Phase-Specific Purity.

### LB-6 — Parser Phase Purity

The parser SHALL build AST from tokens. The parser owns: syntax, declaration-shape validation, attribute placement/applicability checks, and parse-time warnings (e.g., `UnknownCallingConvention` emitted by parser). The parser SHALL NOT perform name resolution, type checking, or deeper semantic analysis. Contextual keyword resolution (e.g., `with` as capability provision vs `with` as function_exp) is syntactic disambiguation, not semantic analysis.

Note: detached doc comment warnings (`LexProblem::DetachedDocComment`) are emitted by the LEXER in the standard pipeline (`oric/src/commands/mod.rs` processes `lex_output.warnings` before parsing). The parser provides `ParseOutput::check_detached_doc_comments()` as an opt-in alternative, but the standard `parse()` and Salsa `parsed()` query do not invoke it.

Rationale: Per `compiler.md` §Phase-Specific Purity. Note: the parser performs more than pure tree assembly — it validates declaration shapes, import ordering, and attribute applicability as part of its syntax-level responsibility.

---

## §7 Context-Sensitive Keywords

### KW-1 — function_exp Keywords

The `match_function_exp_kind()` dispatcher recognizes function_exp forms when the current token is followed by `(`. The implementation splits the eleven names into two categories based on whether the LEXER treats them as soft or reserved:

**Lexer-soft keywords** (6): `cache`, `catch`, `parallel`, `recurse`, `spawn`, `timeout`. The lexer does NOT produce the keyword token kind unless followed (after horizontal whitespace) by `(`. Without the `(`, they are lexed as ordinary identifiers.

Source: `ori_lexer/src/keywords/mod.rs` — contextual promotion logic.

**Reserved-but-reusable keywords** (5): `with`, `print`, `panic`, `todo`, `unreachable`. These are always lexed as their keyword token kind. However, the parser treats a subset of them as reusable as identifiers via `soft_keyword_str()` (`ori_parse/src/cursor/identifiers.rs`) — specifically `print`, `panic`, and `with` can appear in identifier positions (bindings, call names, pattern names) via `expect_ident()` and `parse_binding_pattern()`. `todo` and `unreachable` are always reserved.

Source: `ori_parse/src/grammar/expr/operators.rs` — `match_function_exp_kind()`; `ori_parse/src/cursor/identifiers.rs` — `soft_keyword_str()`; `ori_parse/src/grammar/expr/primary/literals.rs`, `bindings.rs` — reuse sites.

Rationale: Lexer-soft keywords balance expressiveness (identifier status preserved at lex time) with parser convenience (common idioms like `timeout(...)` read naturally). Reserved-but-reusable keywords are always lexed as keywords but accepted in identifier positions by the cursor's identifier-or-soft-keyword APIs, giving users flexibility without ambiguity.

### KW-2 — `with` Disambiguation

`with` has three parse paths selected by lookahead:
- **Capability provision** (`with Http = RealHttp { } in expr`): selected when `with` is followed by `Ident = `. This is checked via `is_with_capability_syntax()` before any function_exp dispatch.
- **function_exp `with(...)`**: selected when `with` is followed by `(` AND the capability-provision pattern does not match (the `(` token after `with` is not an `Ident`, so it cannot match the capability pattern).
- **Identifier**: when `with` appears in identifier position (see KW-1 reserved-but-reusable note), it is accepted by the cursor's identifier APIs.

The three paths are mutually exclusive: the first matching lookahead wins.

Source: `ori_parse/src/grammar/expr/operators.rs` — `match_function_exp_kind()`; `ori_parse/src/cursor/mod.rs` — `is_with_capability_syntax()` (the capability-pattern lookahead lives on `Cursor`).

---

## §8 Series Combinator

The series combinator unifies the common pattern of parsing delimiter-separated lists (function parameters, struct fields, generic arguments, etc.).

Source: `ori_parse/src/series/mod.rs`.

### SE-1 — Series Configuration

`SeriesConfig` SHALL specify:
- `separator` — the token between items (typically `Comma`)
- `terminator` — the token that ends the series (e.g., `RParen`, `RBracket`, `RBrace`)
- `trailing` — trailing separator policy (`Allowed`, `Forbidden`, or `Required`)
- `skip_newlines` — whether to skip newline tokens between items
- `min_count` / `max_count` — cardinality constraints

### SE-2 — Trailing Separator Policy

| Policy | Behavior |
|--------|----------|
| `Allowed` | Trailing separator permitted; break when terminator found after separator |
| `Forbidden` | Error if separator appears before terminator |
| `Required` | Distinct enum value, not currently differentiated in `series_core()` — behaves identically to `Allowed` at runtime. Reserved for future semantics distinguishing "separator required between items" from "trailing separator permitted." |

Spec: `grammar.ebnf` — most Ori syntax uses `Allowed` trailing commas in multi-line contexts.

`Forbidden` is the only policy with special runtime handling today. `Required` exists in the enum surface but no shipped call site uses it.

### SE-3 — Series as Single Source

All delimiter-separated list parsing SHOULD use the series combinator. Ad hoc `while !check(terminator) { parse(); expect(comma); }` loops that duplicate the series logic are `LEAK:algorithmic-duplication` per `impl-hygiene.md` §Algorithmic DRY.

---

## §9 Arena Allocation

The parser allocates all AST nodes in a flat arena for cache-friendly traversal and O(1) node creation.

Source: `ori_parse/src/lib.rs`, `ori_ir/src/arena/mod.rs`.

### AR-1 — Arena-Based AST

All expressions SHALL be allocated via `ExprArena::alloc_expr()`, returning an opaque `ExprId` handle. The parser SHALL NOT use `Box<Expr>` or any pointer-based tree structure.

Implementation: `ExprArena` is a **struct-of-arrays** of parallel `Vec`s (one per AST component: kinds, spans, auxiliary data), not a classical bump-pointer arena. `alloc_expr()` pushes into the parallel vectors and returns an `ExprId` that indexes into them.

Rationale: Arena-indexed allocation provides: (1) cache-friendly sequential memory layout for traversal over a single component (kinds, spans) without touching unrelated fields, (2) O(1) amortized allocation via `Vec::push`, (3) zero-cost deallocation (drop the arena), (4) opaque handle interface that decouples AST consumers from representation. Per `compiler.md` §Memory: "Arena + ID (`ExprArena`+`ExprId`), not `Box<Expr>`."

### AR-2 — Capacity Pre-Allocation

Arena capacity is derived in two steps:

1. **Parser-side estimate** (`ori_parse/src/lib.rs` — `Parser::new()`): `estimated_source_len = tokens.len() * 5` — a rough "source bytes" estimate (~5 bytes per token on average).
2. **Arena-side conversion** (`ori_ir/src/arena/mod.rs` — `ExprArena::with_capacity()`): interprets its argument as source bytes and applies its own heuristic (~1 expression per 20 bytes of source) to derive the initial arena capacity.

The composition: parser estimates source size from token count, arena converts source size to expression-count capacity. Neither step directly multiplies tokens by a proxy ratio.

### AR-3 — ExprId as Opaque Handle

`ExprId` SHALL be an opaque newtype over a `u32` index. The inner field SHALL be private outside the defining module. Construction via `ExprArena::alloc_expr()` only — never raw `u32` conversion in parser code.

Rationale: Per `impl-hygiene.md` §Type Discipline: "Newtypes for all IDs: `ExprId`, `TypeId`, `TokenIndex` — not raw `u32`."

---

## §10 Snapshot / Speculative Parsing

The parser supports speculative parsing via position snapshots for disambiguation.

Source: `ori_parse/src/snapshot/`.

### SN-1 — Snapshot Scope

`ParserSnapshot` SHALL capture the minimal state needed for rollback: cursor position (`usize`) and parse context flags (`ParseContext`). Total size: ~10 bytes. Snapshots are used by speculative parsing and `one_of!` (PO-4) for automatic backtracking on `EmptyErr`.

**Arena state is NOT captured.** If a speculative path allocates AST nodes (e.g., `parse_type()` during speculative disambiguation in collections parsing), those allocations persist even after snapshot restore. This is by design: keeping snapshots lightweight (10 bytes vs arena-state tracking) is a performance choice. Persisted speculative allocations are harmless — they remain in the arena but are unreferenced by the final AST.

### SN-2 — Snapshot Lifetime

Snapshots SHALL be short-lived — created before trying an alternative, consumed (restored or discarded) immediately after the alternative resolves. Long-lived snapshots that span multiple production rules indicate a design problem (the grammar may need refactoring to reduce ambiguity).

### SN-3 — Speculative Parsing Hierarchy

The parser provides four levels of speculation, from lightest to heaviest:

| Level | Method | What It Does | When To Use |
|-------|--------|-------------|-------------|
| Simple lookahead | `check()`, `next_is_*()` | Peek 1-2 tokens, no state change | Token-kind decisions |
| `look_ahead(predicate)` | Run a closure that may advance cursor, auto-restore | Complex multi-token patterns | >2 token lookahead, newline skipping |
| `try_parse(parser_fn)` | Attempt a full parse, auto-restore on failure | Full production attempt | Decision requires parse success, not just tokens |
| `snapshot()` / `restore()` | Manual state save/restore | Multiple alternative restorations | Complex decision trees with branching |

Simple lookahead SHOULD be preferred over heavier mechanisms when sufficient.

---

## §11 Diagnostics

The parser produces structured diagnostics with source locations, context, and suggestions.

Source: `ori_parse/src/error/`.

### DI-1 — Structured Error Construction

Parse errors SHALL be structured `ParseError` values carrying:
- `code: ErrorCode` — stable identifier for deduplication and testing
- `message: String` — human-readable description
- `span: Span` — source location (u32 byte offset pair)
- `context: Option<String>` — "while parsing X" annotation
- `help: Vec<String>` — actionable suggestions
- `severity: DiagnosticSeverity` — hard (report always) vs soft (may suppress after hard error)

### DI-2 — Error Code Stability

Parser error codes live in the **`E1xxx`** range per the diagnostic namespace allocation in `ori_diagnostic/src/error_code/mod.rs` (`E0xxx` = lexer errors, `E1xxx` = parser errors, `E2xxx` = type errors, `E3xxx` = pattern errors). Once assigned, a code SHALL NOT be reused or change meaning. Tests SHOULD assert on error codes, not exact message text.

Rationale: Per `impl-hygiene.md` §Error Handling: "Error codes are stable API: once assigned, never reuse or change meaning." Per-phase namespace allocation prevents cross-phase collision.

### DI-3 — Construction Paths

Two construction paths exist:

| Path | Description |
|------|-------------|
| `ParseError::new(code, message, span)` | Simple construction — code, message, span provided directly. Used by the majority of call sites for straightforward errors. |
| `ParseError::from_kind(&kind, span)` | Rich structured construction — derives code and message from a `ParseErrorKind` enum variant. Preferred when the error carries structured context. |

Both paths are active. The `new()` path is NOT deprecated — it is appropriate for simple errors. The `from_kind()` path is preferred when an error variant carries structured data (field names, token kinds, expected sets) that would otherwise require ad hoc message formatting.

### DI-4 — Parse Warnings

The parser produces warnings separately from errors via `ParseWarning` variants in `ori_parse/src/error/warning.rs`. Warnings are collected in `deferred_warnings: Vec<ParseWarning>`.

Inline warnings (e.g., `UnknownCallingConvention` in `grammar/item/extern_def.rs`) are pushed to `deferred_warnings` during parsing. Some parser warnings (e.g., `ParseOutput::check_detached_doc_comments()`) are opt-in post-parse checks, not invoked by the standard `parse()` / Salsa `parsed()` pipeline — the equivalent LEXER-emitted `LexProblem::DetachedDocComment` covers detached doc warnings in the production pipeline.

### DI-5 — Common Mistake Detection

Foreign-language keyword detection (e.g., `fn`, `func`, `function` → "use `@` for function declarations") is performed by `dispatch.rs` via the `foreign_keywords` module when an unknown identifier appears in declaration position. This is a parser-phase check applied at the top-level dispatch point.

Note: `def` is a valid Ori keyword (used in `def impl` for default trait implementations) and is NOT detected as a foreign-keyword mistake.

Lexer-level mistake detection (e.g., triple-equals `===`, single-quote strings, increment operators) is a LEXER responsibility and produces errors in the `E0xxx` range. The `ori_parse/src/error/mistakes.rs` module provides `ParseError::from_error_token()` to convert `TokenKind::Error` tokens into parser-side diagnostics; this path is rarely used because lex errors are already surfaced by the lexer.

Source: `ori_parse/src/dispatch.rs` (foreign-keyword dispatch), `ori_parse/src/foreign_keywords/mod.rs` (keyword table), `ori_lexer/` (lex-level mistake detection — `E0xxx` codes).

### DI-6 — Error Context Wrapping

Errors SHOULD carry context annotations ("while parsing function body", "while parsing match arm") using the `ErrorContext` mechanism. Two context-attachment paths exist with DIFFERENT semantics:

- **`ParseOutcome::with_error_context()`** and **`Parser::in_error_context_result()`** use **first-context-wins** semantics: if an error already has a context set, subsequent attempts are ignored (`error.context.is_none()` guard). Used for nested grammar scopes.
- **`ParseError::with_context()`** is an unconditional mutator that OVERWRITES any existing context. Used at specific call sites (e.g., `parse_contracts()`) where the inner context is less useful than the outer one.

Grammar code SHOULD prefer the first-context-wins wrappers for normal propagation. The unconditional mutator is reserved for deliberate override cases and SHOULD be documented inline when used.

### DI-7 — Error Accumulation

The parser SHALL accumulate all errors in a deferred error list rather than bailing on the first error. Error recovery (§4) enables the parser to continue past errors and report multiple diagnostics per pass.

Rationale: Per `impl-hygiene.md` §Error Handling: "Accumulate, don't bail: each phase collects all errors in one pass."

---

## §12 Declaration Dispatch

Top-level declarations are routed from the module parser to specific declaration parsers.

Source: `ori_parse/src/dispatch.rs`, `ori_parse/src/module_parse.rs`.

### DD-1 — Two-Stage Module Parsing

`parse_module()` SHALL route top-level forms through two distinct stages:

**Stage 1 — Imports** (`parse_imports()` in `module_parse.rs`): handles all leading import forms and must come first. Processes:

| Leading Token(s) | Form |
|-------------------|------|
| `use` / `pub use` | Import statement |
| `extension` / `pub extension` | Extension import |

**Stage 2 — Declaration Dispatch** (`dispatch_declaration()` in `dispatch.rs`): handles actual declarations after imports complete. Preprocessing in `module_parse.rs` parses any leading attributes (`#...`) and consumes any leading `pub` token BEFORE calling `dispatch_declaration()`. `dispatch_declaration()` itself dispatches on the tokens below:

| Leading Token(s) | Declaration |
|-------------------|-------------|
| `@` | Function or test declaration |
| `type` | Type declaration (struct, enum, newtype) |
| `trait` | Trait definition |
| `impl` | Impl block |
| `def impl` | Default impl (requires `impl` after `def`) |
| `let $` or bare `$` | Module-level constant |
| `extend` | Extension block |
| `extern` | FFI declaration block |

The two-stage structure enforces import ordering (imports SHALL precede declarations). Attributes and `pub` visibility are handled in `module_parse.rs` preprocessing; attribute applicability (declaration-kind checks) is enforced in `dispatch.rs` during stage 2.

### DD-2 — Semicolon Rule

Declarations follow the Ori semicolon rule: if the declaration body ends with `}`, no semicolon is needed. Two distinct helpers in `dispatch.rs` handle the other cases:

- **`eat_optional_semicolon()`**: used for imports, module-level constants, extension imports, and similar declarations where the terminating `;` is accepted but not required.
- **`eat_optional_item_semicolon()`**: used for expression-bodied items (functions, tests, methods, and type declarations). If the body is a non-block expression and `;` is missing, the parser emits error `E1016`. Call sites include `grammar/item/function/mod.rs`, `grammar/item/impl_def/mod.rs`, `grammar/item/trait_def.rs`, and `grammar/item/type_decl.rs`.

The two helpers encode the spec's intent: block-bodied items need no terminator, non-block expression-bodied items require one, and some module-level declarations accept either form.

Spec: `grammar.ebnf` — semicolon rules per declaration kind.

### DD-3 — Attribute Attachment

Attributes (`#derive(...)`, `#skip(...)`, `#compile_fail(...)`, `#target(...)`, `#cfg(...)`, `#repr(...)`) SHALL be parsed first and attached to the immediately following declaration. An attribute without a following declaration is an error.

Source: `ori_parse/src/grammar/attr/`.

---

## §13 Tracing

### TR-1 — Parse Tracing

Parser tracing SHALL use the `ori_parse` target. Diagnostic levels:
- `trace` — per-expression parsing events (hot path, very verbose)
- `debug` — production entry/exit, significant parsing decisions

Salsa parse query tracing uses target `oric` at `debug` level.

### TR-2 — Phase Dump

`ORI_DUMP_AFTER_PARSE=1` SHALL dump the complete AST to stderr after parsing completes. This is a zero-cost diagnostic — no overhead when disabled.

---

## §14 Key Files

| File / Directory | Responsibility |
|-----------------|---------------|
| `ori_lexer/src/lib.rs` | Lexer entry point, string interning |
| `ori_lexer/src/driver.rs` | Main tokenization loop |
| `ori_lexer/src/cooker/` | Token cooking (keywords, literals, escape sequences) |
| `ori_lexer/src/trivial/` | Simple single/multi-character token scanning |
| `ori_ir/src/token/mod.rs` | `Token { kind, span }` definition |
| `ori_ir/src/token/list.rs` | `TokenList` with parallel tag/flag arrays |
| `ori_ir/src/token/kind.rs` | `TokenKind` enum (all token discriminants) |
| `ori_ir/src/arena/mod.rs` | `ExprArena` — flat arena for AST nodes |
| `ori_parse/src/lib.rs` | Parser entry point, `Parser` struct, public API |
| `ori_parse/src/cursor/` | Token stream navigation, tag-parallel array, derived operations |
| `ori_parse/src/context/` | `ParseContext` flags |
| `ori_parse/src/outcome/` | `ParseOutcome<T>` four-way result, composition macros |
| `ori_parse/src/recovery/` | `TokenSet` bitset, `synchronize()`, recovery sets |
| `ori_parse/src/grammar/expr/` | Expression parsing (Pratt loop, operators, blocks, postfix) |
| `ori_parse/src/grammar/expr/operators.rs` | Binding power table, compound ops, keyword disambiguation |
| `ori_parse/src/grammar/item/` | Declaration parsing (functions, traits, impls, types) |
| `ori_parse/src/grammar/attr/` | Attribute parsing |
| `ori_parse/src/grammar/ty/` | Type expression parsing |
| `ori_parse/src/grammar/expr/patterns/` | Pattern parsing (within expressions) |
| `ori_parse/src/series/` | Series combinator for delimiter-separated lists |
| `ori_parse/src/error/` | Parse error types, error kinds, mistake detection |
| `ori_parse/src/error/warning.rs` | Parse warnings (separate from errors) |
| `ori_parse/src/snapshot/` | Speculative parsing support |
| `ori_parse/src/parser_capture.rs` | Token range capture for formatters/macros |
| `ori_parse/src/dispatch.rs` | Top-level declaration routing and validation |
| `ori_parse/src/module_parse.rs` | Module-level parsing orchestration |
| `ori_parse/src/incremental/` | Incremental reparsing support |
| `docs/ori_lang/v2026/spec/grammar.ebnf` | Authoritative grammar (EBNF) |
| `docs/ori_lang/v2026/spec/operator-rules.md` | Operator semantics and precedence |
