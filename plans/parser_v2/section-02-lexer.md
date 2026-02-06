---
section: "02"
title: Lexer Modernization
status: in-progress
goal: Align lexer with parser/type system architecture; implement approved proposals
sections:
  - id: "02.1"
    title: Perfect Hash Keywords
    status: satisfied-by-logos
  - id: "02.2"
    title: Compile-time Collision Detection
    status: satisfied-by-logos
  - id: "02.3"
    title: Precedence Metadata in Tokens
    status: satisfied-by-parser
  - id: "02.4"
    title: Adjacent Token Optimization
    status: satisfied-by-parser
  - id: "02.5"
    title: Simplified Attributes (Remove HashBracket)
    status: not-started
  - id: "02.6"
    title: Decimal Duration and Size Literals
    status: not-started
  - id: "02.7"
    title: Simplified Doc Comments
    status: not-started
  - id: "02.8"
    title: Template String Interpolation
    status: not-started
  - id: "02.9"
    title: TokenList SoA Migration
    status: not-started
  - id: "02.10"
    title: TokenKind Cleanup (GtEq/Shr Audit)
    status: not-started
---

# Section 02: Lexer Modernization

**Status:** In Progress
**Goal:** Align lexer architecture with parser (SoA ExprArena, ParseOutcome) and type system (Pool SoA), implement approved proposals
**Source:** Go compiler (02.1-02.2), parser architecture review (02.3-02.4), approved proposals (02.5-02.8), system alignment (02.9-02.10)

---

## Design Principle: Phase Separation

The Ori compiler enforces strict phase boundaries (CLAUDE.md: "No phase bleeding: parser ≠ type-check, lexer ≠ parse"):

```
Lexer (context-free)     Parser (context-sensitive)       Type Checker (semantic)
─────────────────────    ───────────────────────────      ──────────────────────
Pure function over        Pure function over tokens        Pure function over AST
source text               (ParseOutcome, ExprArena SoA)   (Pool SoA, Idx)
                   │                               │
                   │   TokenList (flat, no context) │   Module + ExprArena
                   └───────────────────────────────┘
```

The lexer must remain a **context-free, pure function**. Parser concerns (precedence, generic disambiguation) belong in the parser. This enables Salsa caching and incremental recomputation.

---

## Implementation Status

### 02.1-02.2: Already Satisfied by Logos

**Discovery (2026-02-04):** Investigation revealed that Ori's lexer already uses the `logos` crate, which provides **optimal keyword recognition** through a different but equally efficient mechanism:

| Approach | Go's Perfect Hash | Logos State Machine |
|----------|------------------|---------------------|
| Technique | Hash table lookup | DFA-based matching |
| Complexity | O(1) hash + compare | O(n) but optimized DFA |
| Memory | 64-entry table | Compiled state machine |
| Collisions | Must be avoided | Not applicable |
| Maintenance | Manual table updates | Automatic from `#[token]` |

**Why Logos is Equivalent or Better:**
1. **No manual hash function** — Keywords are declared via `#[token("fn")]` attributes
2. **Compile-time generation** — State machine is generated at compile time
3. **Zero runtime overhead** — No hash computation, direct state transitions
4. **Automatic optimization** — Logos merges common prefixes into efficient DFA
5. **Type-safe** — Keywords map directly to enum variants

**Conclusion:** No changes needed. Logos already provides O(1)-equivalent keyword recognition with better maintainability than a manual perfect hash table.

---

### 02.3: Satisfied by Parser (Pratt Binding Power Table)

**Discovery (2026-02-06):** Cross-system review determined that embedding precedence/associativity in tokens would violate phase separation and conflict with the parser's existing Pratt parser design.

**Why this belongs in the parser, not the lexer:**

1. **Phase bleeding** — Precedence is a parser-level semantic concern. The lexer's job is to produce a flat, context-free token stream. Moving parser semantics into the lexer creates coupling between phases and violates `CLAUDE.md: "No phase bleeding: parser ≠ type-check, lexer ≠ parse"`.

2. **Already solved** — The parser's Pratt parser (`ori_parse/src/grammar/expr/operators.rs`) has a `const` binding power table with 13 precedence levels. `parse_binary_pratt(min_bp)` looks up binding power via `infix_binding_power()` — a single match that compiles to a jump table. This is already optimal.

3. **Token size bloat** — Token is 24 bytes (`static_assert_size!(Token, 24)` in `token.rs`). Adding `precedence: u8` + `associativity: Assoc` would push it to 32 bytes (33% increase). Every token in every file gets bigger, but only ~15% of tokens are operators.

4. **Salsa incompatibility** — Token derives `Clone, Eq, PartialEq, Hash` for Salsa caching. Adding dead fields (`precedence: 0` for non-operators) that participate in hashing wastes cycles and changes hash landscapes for no benefit.

5. **Two sources of truth** — The parser would have both its binding power table AND the token's embedded precedence. When precedence changes, both must be updated — a maintenance hazard.

**Conclusion:** No changes needed. The Pratt parser's binding power table is the correct, single-responsibility solution. It's `const`, inlined, and already as fast as a direct field read.

---

### 02.4: Satisfied by Parser (Cursor Compound Synthesis)

**Discovery (2026-02-06):** Cross-system review confirmed that the parser's existing compound operator synthesis is the correct and battle-tested approach. Adding `LexContext` to the lexer would break context-free purity.

**Why this belongs in the parser, not the lexer:**

1. **Context-free lexing enables Salsa** — `fn lex(source, interner) -> TokenList` is a pure function. Introducing `LexContext::in_generic_params` makes the lexer stateful, breaking Salsa's deterministic caching or requiring context threading.

2. **Already implemented** — `cursor.rs` provides:
   - `is_shift_right()` — checks `Gt` + adjacent `Gt` (line 175)
   - `is_greater_equal()` — checks `Gt` + adjacent `Eq` (line 183)
   - `consume_compound()` — consumes two adjacent tokens, returns merged span (line 192)
   - `spans_adjacent()` — O(1) adjacency check via span comparison (line 164)

3. **Industry consensus** — Rust, Go, TypeScript, and C++ all handle `>>` disambiguation at the parser level, not the lexer. The lexer emits individual `>` tokens; the parser synthesizes compounds when appropriate.

4. **Compound operators already handled** — The following are already lexed correctly:
   - `->`, `=>`, `..`, `..=`, `::` — lexed as single tokens by logos
   - `>>`, `>=` — synthesized by parser from adjacent `>` tokens
   - `<<`, `<=` — lexed as single tokens (no ambiguity with generics)

**Conclusion:** No changes needed. The parser's `Cursor` compound synthesis is the canonical solution. Document this pattern in design docs.

---

## New Tasks: Approved Proposals

The following approved proposals require lexer changes. These are ordered by implementation dependency.

---

## 02.5 Simplified Attributes (Remove HashBracket)

**Proposal:** `docs/ori_lang/proposals/approved/simplified-attributes-proposal.md`
**Status:** Not started

Simplify attribute syntax from `#[name(...)]` to `#name(...)`.

### Changes

- [ ] Remove `HashBracket` token variant from `TokenKind`
- [ ] Remove `HashBracket` from `RawToken` in `raw_token.rs`
- [ ] Update `discriminant_index()` mapping (renumber or leave gap)
- [ ] Update `friendly_name_from_index()` mapping
- [ ] Update `TOKEN_KIND_COUNT` (116 → 115)
- [ ] Update `TokenSet` bitset if indices shift
- [ ] Update parser attribute parsing to expect `Hash` + `Ident` instead of `HashBracket`
- [ ] Update all tests referencing `HashBracket`

### Compatibility Notes

- **Parser**: `ori_parse/src/grammar/item/attr.rs` parses attributes — must update to new syntax
- **TokenSet**: If discriminant indices shift, all predefined `TokenSet` constants need updating
- **Incremental**: `TokenCapture` ranges remain valid (token indices, not kinds)

---

## 02.6 Decimal Duration and Size Literals

**Proposal:** `docs/ori_lang/proposals/approved/decimal-duration-size-literals-proposal.md`
**Status:** Not started

Allow decimal syntax as compile-time sugar: `1.5s` → 1,500,000,000 nanoseconds.

### Changes

- [ ] Remove `FloatDurationError` token variant from `TokenKind`
- [ ] Remove `FloatSizeError` token variant from `TokenKind`
- [ ] Update `TOKEN_KIND_COUNT` (accounting for removed variants)
- [ ] Update `discriminant_index()` and `friendly_name_from_index()`
- [ ] Add decimal duration regex patterns to `raw_token.rs`
  ```rust
  // Match patterns like 1.5s, 0.5ms, 2.0h
  #[regex(r"[0-9]+\.[0-9]+ns", priority = 3)]
  DecimalDurationNs,
  // ... for each unit
  ```
- [ ] Add decimal size regex patterns to `raw_token.rs`
- [ ] Implement compile-time conversion in `convert.rs`
  - Multiply to base units (nanoseconds for duration, bytes for size)
  - Validate result is whole number (error if not)
- [ ] Update `DurationUnit::to_nanos()` — currently takes `u64`, may need to handle the computed value
- [ ] Update `SizeUnit::to_bytes()` — SI units (1000, not 1024) already correct
- [ ] Add validation error for non-whole results (e.g., `1.5ns` → error)

### Compatibility Notes

- **Token size**: No change — `Duration(u64, DurationUnit)` stores the computed base-unit value
- **Type system**: No change — type checker sees `Duration` token with integer value as before
- **Salsa**: Token Hash/Eq unchanged — same `Duration(u64, DurationUnit)` representation
- **Parser**: No change — parser handles `Duration`/`Size` tokens identically

---

## 02.7 Simplified Doc Comments

**Proposal:** `docs/ori_lang/proposals/approved/simplified-doc-comments-proposal.md`
**Status:** Not started

Simplify doc comment markers: remove `#` for descriptions, replace `@param`/`@field` with `* name:`.

### Changes

- [ ] Update `CommentKind` enum in `ori_ir`:
  - Remove `DocParam` and `DocField` (if separate)
  - Add `DocMember` (unified for params and fields)
  - Keep `DocDescription` (now: unmarked comments before declarations)
- [ ] Update `classify_and_normalize_comment()` in `comments.rs`:
  - `// text` → `DocDescription` (was: `Regular` unless `#`-prefixed)
  - `// * name: desc` → `DocMember` (was: `// @param name` → `DocParam`)
  - `// ! text` → `DocWarning` (unchanged)
  - `// > text` → `DocExample` (unchanged)
- [ ] Update `ModuleExtra::doc_comments_for()` to use new classification
- [ ] Update tests in `comments.rs` and `lib.rs`

### Compatibility Notes

- **Parser**: Uses `CommentKind` for doc attachment — field name changes only
- **Formatter**: Uses `CommentKind` for output — must update classification logic
- **Type system**: Does not use comments — no impact

---

## 02.8 Template String Interpolation

**Proposal:** `docs/ori_lang/proposals/approved/string-interpolation-proposal.md`
**Status:** Not started
**Complexity:** High — requires sub-lexer state machine

Add backtick-delimited template strings with `{expr}` interpolation.

### Architecture

Template strings require a **nested lexer** or state machine because `{expr}` segments contain arbitrary Ori expressions that must be lexed recursively:

```ori
`Hello, {user.name}! You have {count + 1} messages.`
```

The lexer must produce a sequence of tokens that the parser can reconstruct:

```
TemplateLiteralStart    // `Hello,
TemplateExprStart       // {
Ident(user)             // user
Dot                     // .
Ident(name)             // name
TemplateExprEnd         // }
TemplateLiteralMiddle   // ! You have
TemplateExprStart       // {
Ident(count)            // count
Plus                    // +
Int(1)                  // 1
TemplateExprEnd         // }
TemplateLiteralEnd      // messages.`
```

### New Token Variants

- [ ] Add `TemplateLiteralStart(Name)` — opening backtick + text before first `{`
- [ ] Add `TemplateLiteralMiddle(Name)` — text between `}` and next `{`
- [ ] Add `TemplateLiteralEnd(Name)` — text after last `}` + closing backtick
- [ ] Add `TemplateLiteralFull(Name)` — backtick string with no interpolation
- [ ] Add `TemplateExprStart` — `{` inside template
- [ ] Add `TemplateExprEnd` — `}` inside template
- [ ] Update `TOKEN_KIND_COUNT` and `discriminant_index()`

### Lexer Changes

- [ ] Add template string state to logos or implement as post-processing
- [ ] Handle nested braces: `{map[key]}` — must track brace depth
- [ ] Handle escape sequences: `` \` `` for literal backtick, `{{`/`}}` for literal braces
- [ ] Handle multi-line template strings
- [ ] Handle format specifiers: `{value:.2}`, `{name:<10}`

### Compatibility Notes

- **Token size**: `TemplateLiteralStart(Name)` fits in 16-byte `TokenKind` (Name is u32)
- **TokenSet**: Must accommodate 6 new token variants — current 128-bit bitset has room (116 + 6 = 122 < 128)
- **Parser**: Needs new grammar rules for template expressions — significant parser work
- **Type system**: Template expressions must type-check each `{expr}` segment — needs `Printable` trait resolution
- **Salsa**: New tokens derive same traits — no issue

### Implementation Strategy

This is the most complex lexer change. Consider:
1. **Logos limitation**: Logos is a DFA-based lexer — it cannot handle recursive/nested structures natively. Template strings may require a **two-pass** approach: logos tokenizes the template as a raw string, then a post-processing pass splits it into segments.
2. **Brace depth tracking**: Need a counter to handle `{map[{key}]}` — logos cannot do this.
3. **Reference**: See Rust's `rustc_lexer` for how they handle raw strings, and TypeScript's template literal lexing.

---

## New Tasks: System Alignment

---

## 02.9 TokenList SoA Migration

**Status:** Not started
**Goal:** Align TokenList with ExprArena (parser) and Pool (type system) SoA patterns

### Current State

The design doc (`03-lexer/index.md`) describes TokenList as SoA:
```rust
// Design doc says:
pub struct TokenList {
    tokens: Vec<TokenKind>,
    spans: Vec<Span>,
}
```

But the actual implementation (`ori_ir/src/token.rs`) is AoS:
```rust
// Reality:
pub struct TokenList {
    tokens: Vec<Token>,  // Token = { kind: TokenKind, span: Span }
}
```

### Why SoA

The parser's hot path scans token **kinds** far more often than spans:
- `cursor.check()` — discriminant comparison on kind only
- `cursor.check_ident()` — pattern match on kind only
- `cursor.skip_newlines()` — kind comparison only
- `cursor.is_at_end()` — kind comparison only

SoA layout (`Vec<TokenKind>` + `Vec<Span>`) would improve cache locality for these operations because kinds are packed contiguously without span data interleaved.

Both the parser's `ExprArena` and the type system's `Pool` use SoA for the same reason.

### Changes

- [ ] Split `TokenList` storage:
  ```rust
  pub struct TokenList {
      kinds: Vec<TokenKind>,   // 16 bytes each, contiguous
      spans: Vec<Span>,        // 8 bytes each, contiguous
  }
  ```
- [ ] Update `TokenList` API:
  - `get(index)` → returns `TokenRef { kind: &TokenKind, span: Span }` or separate accessors
  - `get_kind(index)` → `&TokenKind` (hot path)
  - `get_span(index)` → `Span` (cold path during error reporting)
  - `push(kind, span)` instead of `push(Token)`
- [ ] Update `Cursor` to use split accessors:
  - `current_kind()` → `&self.tokens.kinds[self.pos]` (direct, no struct indirection)
  - `current_span()` → `self.tokens.spans[self.pos]`
- [ ] Update `TokenCapture::span()` to use `spans` array directly
- [ ] Update `TokenList::get_range()` — may need to return a view type instead of `&[Token]`
- [ ] Update `static_assert_size!` — remove Token size assert, add TokenKind and Span asserts
- [ ] Update Salsa Hash/Eq impls for new layout
- [ ] Update all call sites across crates

### Compatibility Notes

- **Cursor**: Main consumer — needs `current_kind()` and `current_span()` to use separate arrays
- **TokenCapture**: `get_range()` currently returns `&[Token]` — must change to an iterator or view
- **Incremental parsing**: `SyntaxCursor` and `AstCopier` use `TokenList` — update span access
- **Formatter**: Uses `TokenList` for position info — update to use `get_span()`
- **Salsa**: `TokenList` derives `Clone, Eq, PartialEq, Hash` — SoA layout hashes the same content

### Risk

**Medium** — This changes a widely-used type but the API surface is small. All access goes through `Cursor` or `TokenList` methods, so the change is contained.

---

## 02.10 TokenKind Cleanup (GtEq/Shr Audit)

**Status:** Not started
**Goal:** Clarify whether `GtEq` and `Shr` token variants are used or dead

### Context

The design docs state that `>=` and `>>` are **never lexed as single tokens** — the lexer emits individual `>` tokens, and the parser synthesizes compound operators from adjacent tokens. However, `TokenKind::GtEq` (index 97) and `TokenKind::Shr` (index 98) exist as variants.

### Tasks

- [ ] Audit: Does the lexer (`raw_token.rs`) ever produce `GtEq` or `Shr`?
- [ ] Audit: Does the parser ever construct or match on `GtEq` or `Shr`?
- [ ] Audit: Does the evaluator, type checker, or codegen use `GtEq` or `Shr`?
- [ ] Decision: If unused, remove variants and update:
  - `discriminant_index()` — renumber or leave gaps
  - `friendly_name_from_index()` — update mapping
  - `TOKEN_KIND_COUNT` — decrement
  - `display_name()` — remove arms
  - Binding power table — ensure `is_shift_right()` path is the only `>>` path
- [ ] Decision: If used (e.g., by evaluator after parser synthesis), document this as intentional

### Compatibility Notes

- **TokenSet**: Removing variants frees bits in the 128-bit bitset
- **Parser**: Uses `is_shift_right()` / `is_greater_equal()` for synthesis — may internally create these tokens
- **Evaluator/Codegen**: May match on `GtEq`/`Shr` after parser creates them

---

## 02.11 Completion Checklist

- [x] Perfect hash function → satisfied by logos DFA
- [x] Compile-time collision detection → satisfied by logos
- [x] All keywords recognized correctly → verified by logos
- [x] Precedence handling → satisfied by parser's Pratt binding power table
- [x] Adjacent token handling → satisfied by parser's Cursor compound synthesis
- [ ] `HashBracket` removed (02.5)
- [ ] Decimal duration/size literals (02.6)
- [ ] Simplified doc comments (02.7)
- [ ] Template string interpolation (02.8)
- [ ] TokenList SoA migration (02.9)
- [ ] GtEq/Shr audit (02.10)

**Exit Criteria:**
- All approved proposals implemented in lexer
- TokenList aligned with parser/type system SoA patterns
- No dead token variants
- All lexer, parser, type checker, and spec tests pass
- `./test-all.sh` passes
