---
section: "08"
title: Parser Integration & Migration
status: not-started
goal: "Migrate the compiler from V1 (logos) to V2 (hand-written) lexer via a feature-flagged transition, preserving all parser behavior"
sections:
  - id: "08.1"
    title: Existing Parser Infrastructure
    status: not-started
  - id: "08.2"
    title: Tag Constant Migration
    status: not-started
  - id: "08.3"
    title: Greater-Than Token Handling
    status: not-started
  - id: "08.4"
    title: TokenFlags Exposure
    status: not-started
  - id: "08.5"
    title: Template Literal Integration
    status: not-started
  - id: "08.6"
    title: Migration Strategy
    status: not-started
  - id: "08.7"
    title: Tests
    status: not-started
---

# Section 08: Parser Integration & Migration

**Status:** :clipboard: Planned
**Goal:** Migrate the compiler from V1 (logos) to V2 (hand-written) lexer via a feature-flagged transition, preserving all parser behavior. The parser cursor and `TokenList` are unchanged -- only the function that produces the `TokenList` changes.

> **REFERENCE**: TypeScript's re-scanning mechanism for context-sensitive tokens; Ori's existing tag-based cursor dispatch.
>
> **CONVENTIONS**: Follows `plans/v2-conventions.md` SS6 (Phase Output), SS7 (Shared Types in `ori_ir`), SS2 (Tag/Discriminant Enums), SS4 (Flag Types).

---

## Design Rationale

The parser cursor (`compiler/ori_parse/src/cursor.rs`) and `TokenList` (`ori_ir/src/token.rs`) are **not changing**. The V2 lexer produces the same `TokenList` output as V1. The migration work is:

1. **Document existing infrastructure**: The parser has significant tag-based infrastructure that the V2 lexer must integrate with seamlessly
2. **Feature-flagged switchover**: V2 `lex()` runs alongside V1 behind a feature flag for verification
3. **Equivalence verification**: Exhaustive comparison that V1 and V2 produce identical `TokenList` for all test files
4. **`>` token behavior**: Verify the parser's adjacent-`>` synthesis for generics/shifts works with V2 output
5. **TokenFlags exposure**: Provide `NEWLINE_BEFORE` and `IS_DOC` flags to the parser cursor for line significance and doc comment handling
6. **Template literal parsing**: Parser handles TemplateHead/Middle/Tail token sequences for string interpolation
7. **Logos removal**: Once verified, remove the logos-based code and dependency

### Migration Constraint

The parser has extensive test coverage and is performance-critical. Migration must be done incrementally with verification at each step. The golden rule: **`./test-all.sh` must pass at every commit**.

---

## 08.1 Existing Parser Infrastructure

The parser already has significant tag-based infrastructure that the lexer V2 must integrate with seamlessly. These are not workarounds -- they are the **established contract** between lexer output and parser consumption. Understanding these is prerequisite to integration.

### Tag-Based Components

| Parser Component | Location | What It Does | Lexer V2 Must Provide |
|-----------------|----------|-------------|----------------------|
| `Cursor::current_tag() -> u8` | `cursor.rs` | Reads tag from `tags: &[u8]` slice (extracted via `tokens.tags()`) | `TokenList.tags()` method returning `&[u8]` slice |
| `OPER_TABLE[128]` | `operators.rs` | Static Pratt parser binding power lookup indexed by tag | Tag values < 128 for all operator tokens |
| `POSTFIX_BITSET` | `postfix.rs` | Two-u64 bitset for O(1) postfix token membership | Stable tag values so bitset indices are correct |
| `parse_primary()` fast path | `primary.rs` | Direct tag match before `one_of!` macro (~95% of common cases) | `TAG_*` constants or equivalent discriminant values |
| `match_unary_op()` | `operators.rs` | Tag-based unary operator detection (`-`, `!`, `~`) | Consistent tag values for unary operators |
| `match_function_exp_kind()` | `operators.rs` | Tag-based keyword detection for patterns (`recurse`, `parallel`, etc.) | Consistent tag values for pattern keywords |
| Branchless `advance()` | `cursor.rs` | No bounds check, relies on EOF sentinel at end of storage | EOF token always present at end of `TokenList` |
| `check_type_keyword()` | `cursor.rs` | Range-based tag check: `TAG_INT_TYPE..=TAG_NEVER_TYPE` plus `TAG_VOID` | Type keyword tags (42-48) must remain contiguous |
| `parse_type()` tag dispatch | `ty.rs` | Tag-based dispatch for primitive type keywords | Same contiguous range requirement as `check_type_keyword()` |
| `#[cold]` split `expect()` | `cursor.rs` | Error path isolated from hot path for LLVM inlining | Compatible error types |

### Key Constraint: `#[repr(u8)]` Layout Compatibility

The `Cursor` reads the tag array as a `&[u8]` slice. Currently, `TokenKind` uses `discriminant_index()` to map variants to u8 tags. Lexer V2 can either:

1. Continue using `TokenKind` with `discriminant_index()` (current approach)
2. Introduce a separate `TokenTag` enum that's `#[repr(u8)]` with direct discriminant values

**Current implementation (ori_ir/src/token.rs):**

```rust
// TokenKind uses discriminant_index() for tag extraction
impl TokenKind {
    pub const TAG_GT: u8 = 96;
    pub const TAG_EQ: u8 = 90;
    pub const TAG_LPAREN: u8 = 70;
    // ... etc

    #[inline]
    pub const fn discriminant_index(&self) -> u8 {
        match self {
            Self::Gt => 96,
            Self::Eq => 90,
            // ... etc
        }
    }
}

// The cursor reads tags as raw u8 values:
impl Cursor<'_> {
    #[inline]
    pub fn current_tag(&self) -> u8 {
        self.tags[self.pos]  // tags extracted via tokens.tags()
    }
}

// This means tag values are the direct u8 indices into OPER_TABLE and POSTFIX_BITSET.
// Changing a tag value silently breaks all table-based dispatch.
```

**Cursor fields (compiler/ori_parse/src/cursor.rs):**

```rust
pub struct Cursor<'a> {
    tokens: &'a TokenList,
    tags: &'a [u8],           // Parallel to tokens, for fast dispatch
    interner: &'a StringInterner,
    pos: usize,
}
```

### Parser Hot Path Techniques (Already Proven)

These techniques from the parser optimization work (February 2026, +12-16% throughput) inform the lexer V2 design. They are proven patterns, not speculative:

| Technique | Parser Result | Lexer V2 Application |
|-----------|--------------|---------------------|
| `#[cold]` + `#[inline(never)]` on error paths | Prevented `format!()` from poisoning LLVM inlining | Apply to all `LexError` construction |
| Branchless advance with EOF sentinel | Eliminated bounds check on every `advance()` | Sentinel-terminated source buffer (section 01) enables the same pattern |
| Static lookup tables (`OPER_TABLE[128]`) | Replaced 20-arm match with one memory read | Character classification table in scanner (section 05) |
| Bitset membership (`POSTFIX_BITSET`) | O(1) token set check via two `u64`s | Character class sets, keyword-start chars |
| Tag-based dispatch | Skip snapshot/restore for 95% of cases | Direct state machine dispatch on first byte |

### Verification Tasks

- [ ] Verify `Cursor::new(tokens, interner)` accepts V2 `TokenList` identically (it takes `&TokenList`, which is unchanged)
- [ ] Verify hot-path methods produce identical results with V2 tokens:
  - `current_tag()` -- reads from `tags` slice
  - `current()` -- reads from `tokens` slice
  - `current_span()` -- reads span from `Token`
- [ ] Verify `skip_newlines()` works (depends on tag value for `Newline` being identical)
- [ ] Verify `check_type_keyword()` works (depends on `TAG_INT_TYPE..=TAG_NEVER_TYPE` being contiguous at 42-48, plus `TAG_VOID` at 32)
- [ ] Verify `parse_type()` tag dispatch in `ty.rs` aligns with `check_type_keyword()` range
- [ ] Verify `advance()`, `expect()`, `check()`, `check_tag()` all work unchanged

---

## 08.2 Tag Constant Migration

- [ ] Ensure V2 token tag values align with the existing `TAG_*` constants:
  ```rust
  // Current constants (ori_ir/src/token.rs lines 210-312)
  impl TokenKind {
      pub const TAG_IDENT: u8 = 6;
      pub const TAG_NEWLINE: u8 = 111;
      pub const TAG_EOF: u8 = 112;
      pub const TAG_LET: u8 = 19;
      pub const TAG_GT: u8 = 96;   // Used for > splitting
      pub const TAG_EQ: u8 = 90;   // Used for >= synthesis
      pub const TAG_LPAREN: u8 = 70;
      pub const TAG_DOT: u8 = 79;
      pub const TAG_LBRACKET: u8 = 74;
      pub const TAG_LBRACE: u8 = 72;
      pub const TAG_QUESTION: u8 = 86;
      pub const TAG_AS: u8 = 37;
      pub const TAG_ARROW: u8 = 83;
      // ... etc (116 total variants, indices 0-115)
  }
  ```
- [ ] Verify that `TokenSet` (u128 bitset) still works with the new tag values:
  - `TokenSet::contains(&kind)` uses `1u128 << kind.discriminant_index()` -- this requires discriminant indices < 128
  - If `TokenTag` has > 128 variants, split into two `u128` or use a different bitset
  - Current `TokenKind` has 116 variants, so this should still fit
- [ ] Update `TokenKind::discriminant_index()` if the discriminant mapping changes
- [ ] Document the tag numbering contract: tags must be stable across versions for incremental compilation
- [ ] Verify `OPER_TABLE[128]` entries correspond to correct tag values:
  ```rust
  // Actual OPER_TABLE (compiler/ori_parse/src/grammar/expr/operators.rs lines 92-120)
  static OPER_TABLE: [OperInfo; 128] = {
      table[TokenKind::TAG_DOUBLE_QUESTION as usize] = OperInfo::new(bp::COALESCE.0, bp::COALESCE.1, 0, 1);
      table[TokenKind::TAG_PIPEPIPE as usize] = OperInfo::new(bp::OR.0, bp::OR.1, 1, 1);
      table[TokenKind::TAG_GT as usize] = OperInfo::new(bp::COMPARISON.0, bp::COMPARISON.1, 10, 1);
      // ... etc (18 operators mapped)
  };
  ```
- [ ] Verify `POSTFIX_BITSET` bit positions correspond to correct tag values:
  ```rust
  // Actual POSTFIX_BITSET (compiler/ori_parse/src/grammar/expr/postfix.rs lines 13-31)
  const POSTFIX_BITSET: [u64; 2] = {
      let tags: [u8; 7] = [
          TokenKind::TAG_LPAREN,   // 70
          TokenKind::TAG_DOT,      // 79
          TokenKind::TAG_LBRACKET, // 74
          TokenKind::TAG_LBRACE,   // 72
          TokenKind::TAG_QUESTION, // 86
          TokenKind::TAG_AS,       // 37
          TokenKind::TAG_ARROW,    // 83
      ];
      // ... bitset construction
  };
  ```

---

## 08.3 Greater-Than Token Handling

- [ ] Preserve the existing `>` splitting behavior:
  - The lexer always emits `>` as a single token (for generics like `Result<Option<T>>`)
  - The parser synthesizes `>>` and `>=` by checking adjacent `>` token spans:
    ```rust
    // Actual implementation (compiler/ori_parse/src/cursor.rs lines 189-203)
    pub fn is_shift_right(&self) -> bool {
        self.current_tag() == TokenKind::TAG_GT
            && self.pos + 1 < self.tags.len()
            && self.tags[self.pos + 1] == TokenKind::TAG_GT
            && self.current_and_next_adjacent()  // adjacent, no whitespace
    }

    pub fn is_greater_equal(&self) -> bool {
        self.current_tag() == TokenKind::TAG_GT
            && self.pos + 1 < self.tags.len()
            && self.tags[self.pos + 1] == TokenKind::TAG_EQ
            && self.current_and_next_adjacent()  // adjacent
    }
    ```
- [ ] The adjacency check uses span data from `Token`, which is unchanged:
  - V2 must produce spans where `>` at position N has `Span { start: N, end: N+1 }`
  - Adjacent `>` tokens must have contiguous spans (no gap)
- [ ] Test: `Result<Option<T>>` parses correctly (two `>` tokens close nested generics)
- [ ] Test: `a >> b` is recognized as a shift operation (two adjacent `>` tokens)
- [ ] Test: `a >= b` is recognized as greater-equal (adjacent `>` + `=` tokens)
- [ ] Test: `a > b` with whitespace is NOT a shift/greater-equal

> **Note on re-scanning:** Template literals use stack-based mode switching in the scanner (section 02), not parser-driven re-scanning. The `>` synthesis is handled entirely within the parser using span adjacency, not re-scanning. No `Rescannable` trait is needed.

---

## 08.4 TokenFlags Exposure

**STATUS: TO BE ADDED.** The V2 lexer will compute `TokenFlags` (v2-conventions SS4) during token production. The parser cursor will need access to specific flags for line significance and doc comment handling.

**Current state:** The existing `TokenList` in `ori_ir/src/token.rs` does NOT expose TokenFlags. The current parser detects newlines by checking for `TokenKind::Newline` tokens in the stream.

### Cursor Flag Methods (TO BE ADDED)

```rust
impl Cursor<'_> {
    /// True if the current token was preceded by a newline.
    /// Used for implicit line continuation detection.
    #[inline]
    pub fn has_newline_before(&self) -> bool {
        self.flags[self.pos].contains(TokenFlags::NEWLINE_BEFORE)
    }

    /// True if the current token is the first non-trivia token on its line.
    /// Used for layout-sensitive constructs.
    #[inline]
    pub fn at_line_start(&self) -> bool {
        self.flags[self.pos].contains(TokenFlags::LINE_START)
    }

    /// True if the current token is a doc comment token (IS_DOC flag).
    /// Used for doc comment classification and attachment.
    #[inline]
    pub fn is_doc_token(&self) -> bool {
        self.flags[self.pos].contains(TokenFlags::IS_DOC)
    }
}
```

### TokenFlags Reference (v2-conventions SS4)

```rust
bitflags::bitflags! {
    pub struct TokenFlags: u8 {
        // Whitespace flags (bits 0-3)
        const SPACE_BEFORE   = 1 << 0;
        const NEWLINE_BEFORE = 1 << 1;
        const TRIVIA_BEFORE  = 1 << 2;
        const ADJACENT       = 1 << 3;

        // Position flags (bits 4-5)
        const LINE_START     = 1 << 4;
        const CONTEXTUAL_KW  = 1 << 5;

        // Status flags (bits 6-7)
        const HAS_ERROR      = 1 << 6;
        const IS_DOC         = 1 << 7;
    }
}
```

### Tasks

- [ ] Add `TokenFlags` storage to `TokenList` (parallel array or SoA column)
- [ ] Add `flags()` accessor method to `TokenList` returning `&[TokenFlags]`
- [ ] Add `flags: &[TokenFlags]` field to `Cursor` (extracted via `tokens.flags()`)
- [ ] Implement `has_newline_before()`, `at_line_start()`, `is_doc_token()` on `Cursor`
- [ ] Verify `NEWLINE_BEFORE` is set correctly for implicit line continuation rules
- [ ] Verify `IS_DOC` is set on doc comment tokens (classified by spec markers `*`, `!`, `>`)
- [ ] Verify `LINE_START` is set correctly for the first non-trivia token on each line

**Alternative:** The current approach of emitting explicit `TokenKind::Newline` tokens works and may be kept. TokenFlags are an optimization to avoid allocating tokens for whitespace.

---

## 08.5 Template Literal Integration

**STATUS: TO BE ADDED.** Template literals will use a multi-token strategy (inspired by TypeScript). The lexer's stack-based mode switching (section 02) will produce these token sequences, which the parser must handle.

**Current state:** Template literal tokens (`TemplateHead`, `TemplateMiddle`, `TemplateTail`, `TemplateComplete`) do NOT exist in the current `TokenKind` enum (ori_ir/src/token.rs). The grammar (spec/grammar.ebnf lines 101-106) defines template literals, but they are not yet implemented in the lexer or parser.

### Token Sequence

For a template literal like `` `hello {name}, you are {age} years old` ``:

```
TemplateHead("hello ")        -- from ` to first {
  Expression(name)            -- parsed by expression parser
TemplateMiddle(", you are ")  -- from } to next {
  Expression(age)             -- parsed by expression parser
TemplateTail(" years old")    -- from } to closing `
```

For a template with no interpolation, e.g. `` `hello world` ``:

```
TemplateComplete("hello world")  -- single token, no interpolation
```

### Parser Integration (Illustrative - TO BE IMPLEMENTED)

The following is a sketch of how template parsing would work. Actual implementation will need to integrate with the existing parser API (`ParseOutcome`, `ExprId`, etc.).

```rust
impl Parser<'_> {
    fn parse_template_literal(&mut self) -> ParseOutcome<ExprId> {
        match self.cursor.current_tag() {
            TAG_TEMPLATE_COMPLETE => {
                // Simple case: no interpolation
                // Extract string content from token (interned Name)
                let text = /* extract from token */;
                self.cursor.advance();
                // Allocate template expression in arena
                // Ok(arena.alloc_expr(Expr::TemplateLiteral { parts: ... }))
            }
            TAG_TEMPLATE_HEAD => {
                // Build up template parts: alternating text and expressions
                // let mut parts = Vec::new();
                // parts.push(/* text from head token */);
                // self.cursor.advance();

                // loop {
                //     // Parse the interpolated expression using existing parse_expr()
                //     let expr_id = chain!(self, self.parse_expr());
                //     // parts.push(/* expr_id */);
                //
                //     match self.cursor.current_tag() {
                //         TAG_TEMPLATE_MIDDLE => {
                //             // parts.push(/* text from middle token */);
                //             self.cursor.advance();
                //         }
                //         TAG_TEMPLATE_TAIL => {
                //             // parts.push(/* text from tail token */);
                //             self.cursor.advance();
                //             break;
                //         }
                //         _ => {
                //             return ParseOutcome::consumed_err(/* error */);
                //         }
                //     }
                // }
                //
                // ParseOutcome::consumed_ok(/* allocate in arena */)
            }
            _ => unreachable!("parse_template_literal called with non-template token"),
        }
    }
}
```

**Note:** This is illustrative pseudo-code. The actual implementation must use:
- `ParseOutcome<ExprId>` return type (not `Result`)
- `chain!()` macro for error propagation
- `self.parse_expr()` for interpolated expressions (existing method)
- `self.arena.alloc_expr()` for AST allocation
- Proper span tracking via `self.current_span()` and `self.previous_span()`
```

### Format Spec Handling

Template format specs (grammar lines 106-119) are captured as raw text by the lexer within the template token. The parser does not need to lex format specs separately -- they are parsed by a dedicated format spec parser operating on the raw text extracted from the token:

```
`{value:>10.2f}`
       ^^^^^^^^^  -- format spec captured as raw text in the TemplateMiddle/Tail token
```

### Tasks

- [ ] Add `TemplateHead(Name)`, `TemplateMiddle(Name)`, `TemplateTail(Name)`, `TemplateComplete(Name)` variants to `TokenKind` enum
- [ ] Add corresponding `TAG_TEMPLATE_HEAD`, `TAG_TEMPLATE_MIDDLE`, `TAG_TEMPLATE_TAIL`, `TAG_TEMPLATE_COMPLETE` constants
- [ ] Update `discriminant_index()` to map new variants to tag values
- [ ] Implement `parse_template_literal()` in the expression parser
- [ ] Wire template token tags into `parse_primary()` dispatch
- [ ] Handle nested template literals (template within interpolation)
- [ ] Handle empty interpolation: `` `{}` `` (TemplateHead + TemplateTail, no expression)
- [ ] Handle adjacent interpolation: `` `{a}{b}` `` (TemplateHead + TemplateMiddle + TemplateTail)
- [ ] Test: Template with format spec `` `{x:>10.2f}` `` parses correctly
- [ ] Test: Nested template `` `outer {`inner {x}`}` `` parses correctly
- [ ] Test: Unterminated template produces good error message

**Note:** Template literals are specified in the grammar but not yet implemented. This is new functionality, not a migration task.

---

## 08.6 Migration Strategy

> **CONVENTIONS**: Feature-flagged migration follows the incremental approach from v2-conventions SS6 (Phase Output) -- new output type is introduced alongside old, verified equivalent, then old is removed.

- [ ] **Phase 1: Dual-mode**: Add V2 lexer alongside V1 behind a feature flag
  - `#[cfg(feature = "lexer_v2")]` switches between V1 and V2 in the `lex()` function
  - Both produce the same `TokenList` type
  - Run full test suite with both paths
- [ ] **Phase 2: Verify equivalence**: Exhaustive comparison of V1 and V2 output
  - For every `.ori` file in `tests/spec/`, lexing with V1 and V2 produces identical `TokenList`
  - Automated comparison script
  - Special attention to: tag values, span positions, identifier interning, keyword recognition
- [ ] **Phase 3: Switch default**: Make V2 the default, V1 behind `lexer_v1` feature flag
  - Run benchmarks to confirm performance improvement (target: >= 1.5x throughput)
  - Run full test suite
  - Verify Salsa early cutoff still works (section 09)
- [ ] **Phase 4: Remove V1**: Delete logos-based code, remove `logos` dependency
  - Clean up `raw_token.rs`, `convert.rs`
  - Update `Cargo.toml`
  - Run `./test-all.sh` one final time

---

## 08.7 Tests

- [ ] **Full pipeline tests**: `./test-all.sh` passes with V2 lexer
- [ ] **Parser cursor tests**: All existing cursor tests pass without modification
- [ ] **Tag dispatch tests**: `TokenSet` operations work correctly with new tag values
- [ ] **OPER_TABLE tests**: Binding power lookups return correct values for V2 tag values
- [ ] **POSTFIX_BITSET tests**: Postfix membership checks return correct values for V2 tag values
- [ ] **Greater-than synthesis tests**: All generic type parsing and shift operator tests pass
- [ ] **TokenFlags tests**: `has_newline_before()`, `at_line_start()`, `is_doc_token()` return correct values
- [ ] **Template literal tests**: All template token sequences parse correctly (head/middle/tail, complete, nested, format specs)
- [ ] **Incremental migration tests**: Feature flag switching between V1 and V2 produces identical parse results
- [ ] **Performance tests**: Parser throughput with V2 lexer is >= V1

---

## 08.8 Completion Checklist

- [ ] Existing parser infrastructure documented and understood
- [ ] Tag constants aligned between lexer and parser
- [ ] `OPER_TABLE` and `POSTFIX_BITSET` verified with V2 tag values
- [ ] Greater-than token synthesis works
- [ ] `TokenFlags` exposed to parser cursor (`NEWLINE_BEFORE`, `LINE_START`, `IS_DOC`)
- [ ] Template literal parsing implemented (head/middle/tail/complete)
- [ ] Dual-mode feature flag works
- [ ] V1/V2 equivalence verified
- [ ] `./test-all.sh` passes with V2 as default
- [ ] V1 code removed, `logos` dependency dropped

**Exit Criteria:** The parser consumes V2 `TokenList` without any behavioral changes. `TokenFlags` are accessible via the cursor. Template literals parse correctly. `./test-all.sh` passes. `logos` dependency is removed. Parser throughput is equal to or better than baseline.
