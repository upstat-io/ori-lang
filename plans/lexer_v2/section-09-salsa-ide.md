---
section: "09"
title: Salsa & IDE Integration
status: not-started
goal: "Integrate V2 lexer with Salsa incremental queries, formatter metadata extraction, and IDE support"
sections:
  - id: "09.1"
    title: Salsa Query Integration
    status: not-started
  - id: "09.2"
    title: LexOutput Phase Output
    status: not-started
  - id: "09.3"
    title: TriviaCollector
    status: not-started
  - id: "09.4"
    title: Doc Comment Classification
    status: not-started
  - id: "09.5"
    title: Tests
    status: not-started
---

# Section 09: Salsa & IDE Integration

**Status:** :clipboard: Planned
**Goal:** Integrate the V2 lexer with Ori's Salsa-based incremental compilation pipeline, preserving early cutoff semantics. Provide structured trivia collection and doc comment classification for the formatter and IDE.

## :warning: Alignment Issues Found (2026-02-06)

This section was reviewed against the actual codebase and spec. Several discrepancies were identified:

### 1. Doc Comment Markers (Grammar vs Implementation)
- **Grammar** (lines 43-47): Defines markers `*`, `!`, `>` for `member_doc`, `warning_doc`, `example_doc`
- **Implementation** (`ori_ir/src/comment.rs`): Uses `#`, `@param`, `@field`, `!`, `>`
- **Issue**: Grammar does not define `#` (description) or `@param`/`@field` markers
- **Action Required**: Either update grammar or change implementation to match

### 2. CommentKind Enum Names
- **Plan**: Used `DocMember`, `DocWarning`, `DocExample`, `DocDescription`
- **Actual**: `DocDescription`, `DocParam`, `DocField`, `DocWarning`, `DocExample`
- **Fixed**: Updated plan to reflect actual enum variants

### 3. LexOutput Structure
- **Plan**: Showed `LexOutput { tokens, errors, metadata }`
- **Actual**: `LexOutput { tokens, comments, blank_lines, newlines }` (no `errors` field, no `metadata` wrapper)
- **Fixed**: Added "CURRENT" vs "V2 TARGET" sections to show both

### 4. Capacity Heuristic
- **Plan**: `source.len() / 6 + 1`
- **Actual**: `source.len() / 4 + 1`
- **Fixed**: Documented actual value, kept V2 target at /6

### 5. TokenList Storage Pattern
- **Plan**: Assumed fully SoA (struct-of-arrays) with separate `tags`, `starts`, `kinds` vectors
- **Actual**: Uses `Vec<Token>` (AoS) PLUS a parallel `tags: Vec<u8>` for O(1) tag dispatch. NOT pure SoA, but already has tag-based dispatch.
- **Fixed**: Corrected plan to reflect actual hybrid storage (AoS tokens + parallel tags)

### 6. TriviaCollector
- **Plan**: Assumed separate `TriviaCollector` struct
- **Actual**: Inline trivia tracking in `lex_with_comments()` with `last_significant_was_newline` flag
- **Fixed**: Documented both approaches, marked as decision point

### 7. Blank Line Logic
- **Plan**: Comments don't reset `consecutive_newlines` (blank line with comment is still blank)
- **Actual**: Comments DO reset the flag (comments are content, not trivia)
- **Fixed**: Updated to match actual behavior, marked as decision point

### 8. Salsa Traits on LexOutput
- **Plan**: Assumed `LexOutput` derives `Clone, Eq, PartialEq, Hash, Debug`
- **Actual**: Only derives `Clone, Default` (not used in Salsa queries)
- **Fixed**: Noted that current `LexOutput` is NOT Salsa-compatible

### 9. Query Chain Completeness
- **Plan**: Described chain as `tokens() -> parse() -> check()`
- **Actual**: Full chain is `tokens() -> parsed() -> typed()` and `tokens() -> parsed() -> evaluated()`
  - Query names: `tokens`, `parsed`, `typed`, `evaluated` (not `parse`, `check`)
  - `evaluated()` also depends on `parsed()` but NOT on `typed()` (calls type checker directly)
  - Additional queries: `line_count`, `non_empty_line_count`, `first_line` (trivial, depend on `file.text()`)
- **Fixed**: Documented actual query names and full dependency graph

### 10. Span::NONE Does Not Exist
- **Plan**: Used `Span::NONE` in `check_detached_doc_comments()`
- **Actual**: The codebase has `Span::DUMMY` (start=0, end=0) but no `Span::NONE`
- **Fixed**: Replaced with `u32::MAX` sentinel value

### 11. Formatter Dependency on DocParam/DocField
- **Plan**: Proposed merging `DocParam`/`DocField` into `DocMember` for V2
- **Actual**: Formatter (`ori_fmt/src/comments.rs`) has dedicated `@param`/`@field` reordering logic:
  - `take_comments_before_function()` reorders `DocParam` comments to match parameter order
  - `take_comments_before_type()` reorders `DocField` comments to match field order
  - `extract_param_name()`, `extract_field_name()` parse `@param`/`@field` content
- **Impact**: Merging to `DocMember` requires updating formatter to parse `* name: desc` format
- **Fixed**: Added formatter dependency as blocker

> **REFERENCE**: Ori's existing Salsa `tokens()` query in `compiler/oric/src/query/mod.rs`; Gleam's `ModuleExtra` for formatter metadata; Gleam's detached doc comment warnings; Roc's space-preserving AST.
>
> **CONVENTIONS**: Follows `plans/v2-conventions.md` SS6 (Phase Output), SS7 (Shared Types in `ori_ir`), SS8 (Salsa Compatibility).

---

## Design Rationale

### Salsa Integration

The lexer feeds into Ori's Salsa query system:

```rust
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    let text = file.text(db);
    ori_lexer::lex(text, db.interner())
}
```

Salsa provides **early cutoff**: if the source text changes but the resulting `TokenList` hashes identically, all downstream queries (parsing, type checking, etc.) skip recomputation. This requires `TokenList` to implement `Eq` and `Hash`.

The V2 `TokenList` must:
1. Implement `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug` (Salsa requirements, v2-conventions SS8)
2. Produce deterministic output (no randomness, no IO)
3. Hash efficiently (hash the tag array first for fast early cutoff)

### Phase Output: `LexOutput`

The V2 lexer introduces `LexOutput` as its phase output type (v2-conventions SS6). This replaces the current approach where `lex()` returns just `TokenList`. The full-metadata variant `lex_with_comments()` returns the complete `LexOutput`:

```rust
/// Phase output for the lexer (v2-conventions SS6 -- Phase Output Shape).
/// Immutable after creation. Next phase borrows read-only.
///
/// **CURRENT IMPLEMENTATION** (ori_lexer/src/lib.rs):
pub struct LexOutput {
    pub tokens: TokenList,           // Primary output (existing type)
    pub comments: CommentList,       // Comments captured during lexing
    pub blank_lines: Vec<u32>,       // Byte positions of blank lines
    pub newlines: Vec<u32>,          // Byte positions of all newlines
}

/// **V2 TARGET** (aligned with v2-conventions SS6):
pub struct LexOutputV2 {
    pub tokens: TokenList,           // Primary output
    pub errors: Vec<LexError>,       // Accumulated errors (v2-conventions SS5)
    pub metadata: ModuleExtra,       // Non-semantic info (contains comments, blank_lines, newlines)
}
```

`LexOutput` must derive `Clone, Eq, PartialEq, Hash, Debug` for Salsa compatibility (v2-conventions SS8).

### Formatter Metadata

The formatter needs:
- Comment positions and content (for preserving comments in formatted output)
- Blank line positions (for preserving intentional blank lines)
- Newline positions (for layout analysis)
- Doc comment classification per spec markers (for documentation tools)

---

## 09.1 Salsa Query Integration

- [ ] Ensure V2 `TokenList` derives/implements required Salsa traits (v2-conventions SS8):
  ```rust
  // CURRENT STRUCTURE (ori_ir/src/token.rs):
  // Hybrid AoS + parallel tags (NOT pure SoA):
  #[derive(Clone, Default)]  // Manual Eq/PartialEq/Hash impls
  pub struct TokenList {
      tokens: Vec<Token>,   // AoS: Token { kind: TokenKind, span: Span }
      tags: Vec<u8>,        // Parallel discriminant tags for O(1) dispatch
  }
  // Manual PartialEq/Eq: compares only `tokens` (tags derived from tokens)
  // Manual Hash: hashes only `tokens` (tags are redundant)
  // Manual Debug: shows count only
  ```
  **Already has Salsa-required traits**: `Clone` (derive), `Eq`/`PartialEq`/`Hash` (manual), `Debug` (manual).
- [ ] **CURRENT**: `TokenList` uses `Vec<Token>` (AoS) PLUS a parallel `tags: Vec<u8>` for O(1) tag-based dispatch. Tags store `token.kind.discriminant_index()` at insertion time. This is already a hybrid storage pattern.
- [ ] **V2 TARGET**: If switching to full SoA (separate `tags`, `starts`, `kinds` vectors), optimize `Hash` implementation for early cutoff:
  - Hash the `tags` array first (smallest, most likely to differ on source change)
  - Then hash `kinds` (captures identifier names and literal values)
  - Skip hashing `starts` (positions change on every edit but don't affect semantics)
  - This means two files with the same tokens at different positions hash the same -- this is intentional for early cutoff
  - **Alternative**: If position-sensitive hashing is needed, hash everything
  - **NOTE**: Current Hash impl hashes full `Token` (including spans), so position changes DO invalidate cache
- [ ] **DECISION**: Current `TokenList` uses hybrid AoS+tags. Switch to full SoA for V2, or keep hybrid?
- [ ] Update the `tokens()` Salsa query to call V2 `lex()`:
  ```rust
  // CURRENT IMPLEMENTATION (compiler/oric/src/query/mod.rs):
  #[salsa::tracked]
  pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
      tracing::debug!(path = %file.path(db).display(), "lexing");
      let text = file.text(db);
      ori_lexer::lex(text, db.interner())  // ✓ Already calls current lex()
  }
  // V2: No change needed - signature is stable
  ```
- [ ] Add a `tokens_with_metadata()` Salsa query for the formatter/IDE path:
  ```rust
  // NOT YET IMPLEMENTED - proposed for V2
  #[salsa::tracked]
  pub fn tokens_with_metadata(db: &dyn Db, file: SourceFile) -> LexOutput {
      let text = file.text(db);
      ori_lexer::lex_with_comments(text, db.interner())
  }
  ```
  **Note**: Current `LexOutput` does NOT derive Salsa traits. Need to either:
  1. Add derives to current `LexOutput` (breaks if it contains non-Salsa types)
  2. Create `LexOutputV2` with proper derives for V2
  3. Return `(TokenList, ModuleExtra)` tuple instead
- [ ] Ensure `LexOutput` derives required Salsa traits (v2-conventions SS8):
  ```rust
  // CURRENT (does NOT derive Salsa traits - not used in queries):
  #[derive(Clone, Default)]
  pub struct LexOutput {
      pub tokens: TokenList,
      pub comments: CommentList,
      pub blank_lines: Vec<u32>,
      pub newlines: Vec<u32>,
  }

  // V2 TARGET (for Salsa queries):
  #[derive(Clone, Debug, PartialEq, Eq, Hash)]
  pub struct LexOutputV2 {
      pub tokens: TokenList,
      pub errors: Vec<LexError>,
      pub metadata: ModuleExtra,
  }
  ```
- [ ] Verify early cutoff works: edit whitespace in a file -> downstream queries should NOT recompute
  - **CAVEAT**: Current `TokenList` hashes include `Span` (positions). Adding whitespace at the start shifts all positions, changing the hash. True early cutoff for whitespace-only edits requires position-independent hashing (V2 full SoA target).
- [ ] Verify semantic change triggers recomputation: rename an identifier -> downstream queries recompute
- [ ] Verify comment-only edit behavior: edit a comment -> `tokens()` query should cut off (comments stripped), `tokens_with_metadata()` should recompute

---

## 09.2 LexOutput Phase Output

`LexOutput` is the structured phase output (v2-conventions SS6). Currently, `lex()` returns just `TokenList` and `lex_with_comments()` returns `LexOutput` (a struct with `tokens`, `comments`, `blank_lines`, `newlines`). The V2 target is `LexOutputV2` with `tokens`, `errors`, and `metadata: ModuleExtra`.

### API

```rust
/// Phase output for the lexer (v2-conventions SS6).
/// Immutable after creation. Next phase borrows read-only.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LexOutput {
    pub tokens: TokenList,           // Primary output
    pub errors: Vec<LexError>,       // Accumulated errors (v2-conventions SS5)
    pub metadata: ModuleExtra,       // Non-semantic trivia (v2-conventions SS7)
}

/// Fast path: lex without metadata collection.
/// Returns only TokenList; comments and trivia are skipped.
/// **CURRENT IMPLEMENTATION** (ori_lexer/src/lib.rs):
pub fn lex(source: &str, interner: &StringInterner) -> TokenList {
    let mut result = TokenList::with_capacity(source.len() / 4 + 1);  // Current: /4, not /6
    let mut logos = RawToken::lexer(source);

    while let Some(token_result) = logos.next() {
        let span = Span::try_from_range(logos.span()).unwrap_or_else(|_| {
            Span::new(u32::MAX.saturating_sub(1), u32::MAX)
        });
        let slice = logos.slice();

        match token_result {
            Ok(raw) => {
                match raw {
                    RawToken::LineComment | RawToken::LineContinuation => {}
                    RawToken::Newline => {
                        result.push(Token::new(TokenKind::Newline, span));
                    }
                    _ => {
                        let kind = convert_token(raw, slice, interner);
                        result.push(Token::new(kind, span));
                    }
                }
            }
            Err(()) => {
                result.push(Token::new(TokenKind::Error, span));
            }
        }
    }

    // NOTE: Simplified for plan readability. Actual implementation handles
    // files > u32::MAX by emitting an error token before EOF.
    let eof_pos = u32::try_from(source.len()).unwrap_or(u32::MAX);
    result.push(Token::new(TokenKind::Eof, Span::point(eof_pos)));
    result
}

/// Full path: lex with trivia collection.
/// Returns complete LexOutput with tokens, comments, blank lines, and newlines.
/// **CURRENT IMPLEMENTATION** (ori_lexer/src/lib.rs):
pub fn lex_with_comments(source: &str, interner: &StringInterner) -> LexOutput {
    let mut output = LexOutput::with_capacity(source.len());
    let mut logos = RawToken::lexer(source);
    let mut last_significant_was_newline = false;

    while let Some(token_result) = logos.next() {
        let span = Span::try_from_range(logos.span()).unwrap_or_else(|_| {
            Span::new(u32::MAX.saturating_sub(1), u32::MAX)
        });
        let slice = logos.slice();

        if let Ok(raw) = token_result {
            match raw {
                RawToken::LineComment => {
                    let content_str = if slice.len() > 2 { &slice[2..] } else { "" };
                    let (kind, normalized) = classify_and_normalize_comment(content_str);
                    let content = interner.intern(&normalized);
                    output.comments.push(Comment::new(content, span, kind));
                    last_significant_was_newline = false;
                }
                RawToken::LineContinuation => {
                    // No-op: does not reset blank line flag, does not emit token
                }
                RawToken::Newline => {
                    output.newlines.push(span.start);
                    if last_significant_was_newline {
                        output.blank_lines.push(span.start);
                    }
                    output.tokens.push(Token::new(TokenKind::Newline, span));
                    last_significant_was_newline = true;
                }
                _ => {
                    last_significant_was_newline = false;
                    let kind = convert_token(raw, slice, interner);
                    output.tokens.push(Token::new(kind, span));
                }
            }
        } else {
            last_significant_was_newline = false;
            output.tokens.push(Token::new(TokenKind::Error, span));
        }
    }

    let eof_pos = u32::try_from(source.len()).unwrap_or(u32::MAX);
    output.tokens.push(Token::new(TokenKind::Eof, Span::point(eof_pos)));
    output
}

/// **V2 PROPOSED** (with TriviaCollector pattern):
/// This is the PLANNED implementation, not the current one.
pub fn lex_with_comments_v2(source: &str, interner: &StringInterner) -> LexOutputV2 {
    let buffer = SourceBuffer::new(source);
    let mut scanner = RawScanner::new(buffer.cursor());
    let mut tokens = TokenList::with_capacity(source.len() / 6 + 1);  // V2 target: /6
    let mut trivia = TriviaCollector::new();
    let mut errors = Vec::new();
    let cooker = TokenCooker::new(source.as_bytes(), interner);
    let mut offset = 0u32;

    loop {
        let raw = scanner.next();
        match raw.tag {
            RawTag::Whitespace => {
                offset += raw.len;
                continue;
            }
            RawTag::LineComment => {
                let span = Span::new(offset, offset + raw.len);
                let text = &source[offset as usize..(offset + raw.len) as usize];
                trivia.on_comment(text, span, interner);
                offset += raw.len;
                continue;
            }
            RawTag::Newline => {
                trivia.on_newline(offset);
                tokens.push(Token::new(TokenKind::Newline, Span::point(offset)));
                offset += raw.len;
                continue;
            }
            RawTag::Eof => {
                tokens.push(Token::new(TokenKind::Eof, Span::point(offset)));
                break;
            }
            _ => {
                trivia.on_non_trivia();
                let kind = cooker.cook(raw, offset);
                tokens.push(Token::new(kind, Span::new(offset, offset + raw.len)));
                offset += raw.len;
            }
        }
    }

    LexOutputV2 {
        tokens,
        errors,
        metadata: trivia.finish(),
    }
}
```

### Rules (v2-conventions SS6)

1. `LexOutput` is **immutable after creation** -- no mutable references leak out
2. Next phase **borrows read-only** -- no cloning phase outputs
3. Errors are **accumulated, not fatal** -- lexing continues past errors for IDE support
4. Metadata is **non-semantic** -- removing it does not change program behavior

---

## 09.3 TriviaCollector

**STATUS**: The current implementation (ori_lexer v1) does NOT use a separate `TriviaCollector` struct. It implements trivia collection inline within `lex_with_comments()` using a `last_significant_was_newline` flag.

**V2 PROPOSAL**: Extract trivia collection into a dedicated `TriviaCollector` for better separation of concerns and testability.

### Current Implementation (Inline)

The current `lex_with_comments()` in `ori_lexer/src/lib.rs` tracks trivia inline:

```rust
let mut output = LexOutput::with_capacity(source.len());
let mut last_significant_was_newline = false;

// During lexing:
match raw {
    RawToken::LineComment => {
        // Classify and store comment
        output.comments.push(Comment::new(content, span, kind));
        last_significant_was_newline = false;  // Comments are content
    }
    RawToken::Newline => {
        output.newlines.push(span.start);
        if last_significant_was_newline {
            output.blank_lines.push(span.start);  // Blank line detected
        }
        last_significant_was_newline = true;
    }
    _ => {
        last_significant_was_newline = false;  // Non-trivia token
    }
}
```

### V2 Proposed Implementation (TriviaCollector)

```rust
/// Collects trivia (comments, blank lines, newlines) during lexing.
/// Produces `ori_ir::ModuleExtra` (v2-conventions SS7 -- shared types in ori_ir).
pub struct TriviaCollector {
    comments: Vec<Comment>,
    blank_lines: Vec<u32>,
    newlines: Vec<u32>,
    consecutive_newlines: u32,
}

impl TriviaCollector {
    pub fn new() -> Self {
        Self {
            comments: Vec::new(),
            blank_lines: Vec::new(),
            newlines: Vec::new(),
            consecutive_newlines: 0,
        }
    }

    /// Record a newline at the given position.
    /// Tracks consecutive newlines for blank line detection.
    pub fn on_newline(&mut self, pos: u32) {
        self.newlines.push(pos);
        self.consecutive_newlines += 1;

        // Blank line = two consecutive newlines (with only whitespace between)
        if self.consecutive_newlines >= 2 {
            self.blank_lines.push(pos);
        }
    }

    /// Record a non-trivia token. Resets the consecutive newline counter.
    pub fn on_non_trivia(&mut self) {
        self.consecutive_newlines = 0;
    }

    /// Record a comment. Classifies it and stores in the comment list.
    /// NOTE: Current implementation sets consecutive_newlines = 0 (comments are content).
    /// This differs from the comment in the original plan.
    pub fn on_comment(&mut self, text: &str, span: Span, interner: &StringInterner) {
        let (kind, normalized) = classify_and_normalize_comment(text);
        let content = interner.intern(&normalized);
        self.comments.push(Comment::new(content, span, kind));
        // Comments ARE content, so reset blank line detection
        self.consecutive_newlines = 0;
    }

    /// Produce the final `ori_ir::ModuleExtra` (v2-conventions SS6, SS7).
    pub fn finish(self) -> ModuleExtra {
        ModuleExtra {
            comments: CommentList::from_vec(self.comments),
            blank_lines: self.blank_lines,
            newlines: self.newlines,
            trailing_commas: Vec::new(),  // Populated by parser
        }
    }
}
```

### Tasks

- [ ] **Decision Required**: Keep inline implementation or extract to `TriviaCollector`?
- [ ] If extracting, implement `TriviaCollector` in `ori_lexer`
- [ ] Wire `TriviaCollector` into V2 `lex_with_comments()` (see 09.2)
- [ ] Ensure `ModuleExtra` is the shared type from `ori_ir` (v2-conventions SS7) ✓ Already done
  - `ModuleExtra` derives: `Clone, Eq, PartialEq, Default` with manual `Hash` and `Debug` impls
  - Contains: `comments: CommentList`, `blank_lines: Vec<u32>`, `newlines: Vec<u32>`, `trailing_commas: Vec<u32>`
  - Has helper methods: `has_blank_line_between()`, `doc_comments_for()`, `unattached_doc_comments()`
- [ ] **Blank line behavior**: Current implementation treats comments as content (resets counter). V2 plan suggested comments DON'T reset. Which is correct?
- [ ] Verify newline positions are accurate (match existing behavior)
- [ ] Update `parse_with_metadata` to accept V2 `LexOutput`:
  ```rust
  // CURRENT: parse_with_metadata already exists in ori_parse/src/lib.rs:
  // pub fn parse_with_metadata(tokens: &TokenList, metadata: ModuleExtra, interner: &StringInterner) -> ParseOutput
  // ParseOutput already has: module, arena, errors, warnings, metadata: ModuleExtra
  // ParseOutput derives: Clone, Eq, PartialEq, Hash, Debug (Salsa-compatible)
  //
  // V2 usage (no signature change needed):
  let lex_output = lex_with_comments_v2(source, &interner);
  let parse_output = parse_with_metadata(&lex_output.tokens, lex_output.metadata, &interner);
  ```

---

## 09.4 Doc Comment Classification

Doc comments in Ori are classified by spec-defined markers (grammar lines 43-47). The lexer classifies these during trivia collection and sets the `IS_DOC` flag (v2-conventions SS4) on tokens that carry doc semantics.

### Spec Doc Markers

Per grammar lines 42-47, Ori uses single-line comments with markers:

| Marker | Grammar Production | Classification | Example |
|--------|-------------------|---------------|---------|
| `#` | Not in grammar, implemented | Description | `// #Description text` |
| `@param` | Not in grammar, implemented | Parameter doc | `// @param x description` |
| `@field` | Not in grammar, implemented | Field doc | `// @field name description` |
| `*` | `member_doc` | Member documentation | `// * field: description` |
| `!` | `warning_doc` | Warning documentation | `// ! warning text` |
| `>` | `example_doc` | Example documentation | `// > example code` |

**Note**: Current implementation (`ori_ir/src/comment.rs`) uses `DocDescription`, `DocParam`, `DocField`, `DocWarning`, `DocExample`. The grammar shows `*`, `!`, `>` markers but does not include `#`, `@param`, or `@field`. There is a discrepancy between spec and implementation that needs resolution.

### Classification Logic

**Current implementation** (in `ori_lexer/src/comments.rs`):

```rust
/// Classify a comment based on its content.
/// Current implementation uses `#`, `@param`, `@field`, `!`, `>` markers.
/// Grammar (lines 43-47) specifies `*`, `!`, `>` markers only.
fn classify_comment(text: &str) -> CommentKind {
    // Implementation in ori_lexer/src/comments.rs uses:
    // - `// #...` → DocDescription
    // - `// @param ...` → DocParam
    // - `// @field ...` → DocField
    // - `// !...` → DocWarning (matches grammar)
    // - `// >...` → DocExample (matches grammar)
    // - Everything else → Regular

    // See ori_lexer/src/comments.rs for actual implementation
}
```

**Grammar-conformant implementation** (lines 43-47):

```rust
/// Classify a comment per grammar spec (lines 43-47).
/// NOTE: V2 proposal merges DocParam/DocField into a single DocMember variant.
/// Current codebase has: Regular, DocDescription, DocParam, DocField, DocWarning, DocExample.
/// V2 target has: Regular, DocDescription, DocMember, DocWarning, DocExample.
fn classify_comment_spec(text: &str) -> CommentKind {
    let trimmed = text.strip_prefix(' ').unwrap_or(text);
    match trimmed.as_bytes().first() {
        Some(b'*') if trimmed.len() > 1 && trimmed.as_bytes()[1] == b' ' => {
            CommentKind::DocMember    // // * identifier: description (V2 new variant)
            // NOTE: DocMember does NOT exist in current codebase.
            // Current code has DocParam and DocField instead.
            // V2 will add DocMember and remove DocParam/DocField.
        }
        Some(b'!') if trimmed.len() > 1 && trimmed.as_bytes()[1] == b' ' => {
            CommentKind::DocWarning   // // ! warning text
        }
        Some(b'>') if trimmed.len() > 1 && trimmed.as_bytes()[1] == b' ' => {
            CommentKind::DocExample   // // > example code
        }
        _ => CommentKind::Regular,
    }
}
```

**Note**: The spec does not define a "description" marker. Grammar production `doc_comment` (line 43) allows optional markers but doesn't specify how unmarked doc comments are distinguished from regular comments.

### Detached Doc Comment Warnings

A doc comment that is not attached to a declaration should produce a warning, following the Gleam pattern. Two cases:

1. **Doc comment not followed by a declaration** -- the doc comment is at the end of the file or followed only by other comments/blank lines
2. **Doc comment separated from its declaration by a blank line** -- a blank line between doc comment and declaration severs the attachment

```rust
/// Check for doc comments not attached to declarations.
/// Produces warnings for detached doc comments (Gleam pattern).
///
/// This runs after parsing, when declaration spans are known.
///
/// NOTE: ModuleExtra already has `unattached_doc_comments(&self, declaration_starts: &[u32])`
/// which provides similar functionality. This V2 version adds structured warnings.
///
/// Current codebase uses CommentKind::is_doc() which covers:
///   DocDescription, DocParam, DocField, DocWarning, DocExample
/// V2 will use: DocDescription, DocMember, DocWarning, DocExample
/// (DocMember replaces DocParam/DocField per approved proposal)
#[cold]
pub fn check_detached_doc_comments(
    metadata: &ModuleExtra,
    declaration_starts: &[u32],  // Matches existing API signature (not &[Span])
) -> Vec<LexWarning> {
    let mut warnings = Vec::new();

    for comment in &metadata.comments {
        if !comment.kind.is_doc() {
            continue;
        }

        // Find the next declaration after this doc comment
        let next_decl = declaration_starts.iter().find(|&&start| start > comment.span.end);

        match next_decl {
            Some(&decl_start) => {
                // Check if there is a blank line between doc comment and declaration
                let has_blank = metadata.has_blank_line_between(comment.span.end, decl_start);
                if has_blank {
                    warnings.push(LexWarning::DetachedDocComment {
                        comment_span: comment.span,
                        declaration_start: decl_start,
                        reason: "blank line separates doc comment from declaration",
                    });
                }
            }
            None => {
                warnings.push(LexWarning::DetachedDocComment {
                    comment_span: comment.span,
                    declaration_start: u32::MAX,  // sentinel for "no declaration"
                    reason: "doc comment is not followed by any declaration",
                });
            }
        }
    }

    warnings
}
```

### Tasks

- [ ] Implement `classify_comment()` per spec grammar lines 43-47
- [ ] Set `IS_DOC` flag (v2-conventions SS4) on tokens that carry doc semantics
- [ ] Implement `check_detached_doc_comments()` (runs after parsing)
- [ ] Wire detached doc comment check into the compiler pipeline
- [ ] Test: `// * identifier: description` classified as `DocMember` (V2 new variant, grammar line 45; replaces current `DocParam`/`DocField`)
- [ ] Test: `// ! warning text` classified as `DocWarning` (grammar line 46)
- [ ] Test: `// > example code` classified as `DocExample` (grammar line 47)
- [ ] Test: `// regular comment` classified as `Regular`
- [ ] Test: Unmarked comment before declaration classified as `DocDescription`
- [ ] Test: Legacy `#`/`@param`/`@field` markers produce deprecation warning (migration support)
- [ ] Test: Detached doc comment (no following declaration) produces warning
- [ ] Test: Doc comment separated by blank line from declaration produces warning
- [ ] Test: Doc comment directly above declaration produces no warning

---

## 09.5 Tests

- [ ] **Salsa integration tests**:
  - Lex a file -> modify whitespace -> re-lex -> verify early cutoff (downstream not recomputed)
    - **NOTE**: Only works if whitespace change doesn't shift token positions (e.g., trailing whitespace). Currently, Hash includes spans, so position-shifting edits invalidate cache.
  - Lex a file -> modify identifier -> re-lex -> verify recomputation triggered
  - Lex a file -> modify comment -> re-lex -> verify `TokenList` unchanged (comments are stripped in `lex()`)
  - Lex a file -> modify comment -> re-lex -> verify `LexOutput` metadata changed (via `lex_with_comments()`)
- [ ] **LexOutput tests**:
  - `LexOutput` derives Salsa-required traits (`Clone, Eq, PartialEq, Hash, Debug`)
  - `LexOutput.tokens` matches `lex()` output for identical input
  - `LexOutput.errors` contains accumulated errors (not panic)
  - `LexOutput.metadata` contains accurate trivia
- [ ] **TriviaCollector tests**:
  - Comment extraction matches existing behavior for all comment types
  - Blank line detection matches existing behavior
  - Newline position tracking is accurate
  - Consecutive newlines correctly detected
- [ ] **Doc comment classification tests**:
  - All four spec markers (`*`, `!`, `>`, none) classified correctly
  - Edge cases: empty doc comment, doc marker without space, multiple markers
  - Detached doc comment warnings fire correctly
  - Attached doc comments produce no warnings
- [ ] **Formatter round-trip tests**:
  - Format a file -> reformat -> output is stable (idempotent)
  - Comments are preserved in formatted output
  - Blank lines are preserved
- [ ] **Full pipeline test**: `./test-all.sh` passes with V2 lexer in all modes

---

## 09.6 Completion Checklist

### Prerequisites (Spec/Implementation Alignment)
- [x] ~~BLOCKER~~: Doc comment marker mismatch — **RESOLVED** by approved proposal
  `proposals/approved/simplified-doc-comments-proposal.md`
  - Grammar markers `*`, `!`, `>` are correct (proposal approved 2026-01-30)
  - Implementation's `#`, `@param`, `@field` are legacy — V2 lexer implements new markers
  - `DocParam`/`DocField` merge into `DocMember` (`* name: description`)
  - Unmarked comments before declarations are `DocDescription` (no `#` needed)
  - V2 lexer should support legacy markers with deprecation warnings (migration path)
- [ ] **BLOCKER**: Decide on storage pattern for V2 TokenList
  - Keep current hybrid (AoS `Vec<Token>` + parallel `tags: Vec<u8>`) or switch to full SoA?
  - Current hybrid already enables O(1) tag dispatch; full SoA would also optimize Hash for early cutoff
  - Current Hash impl hashes full tokens (including spans), so whitespace-only edits that shift positions DO trigger recomputation

### Core Implementation
- [ ] `LexOutput` phase output type implemented (v2-conventions SS6)
  - Current `LexOutput` exists but lacks `errors` field and Salsa derives
  - Need `LexOutputV2` or modify existing struct
- [ ] `LexOutput` derives `Clone, Eq, PartialEq, Hash, Debug` (v2-conventions SS8)
  - Current: Only `Clone, Default`
  - Blocks: Using `LexOutput` in Salsa queries
- [ ] V2 `TokenList` implements Salsa-required traits ✓ (already has them)
- [ ] `tokens()` Salsa query uses V2 lexer ✓ (already implemented)
- [ ] `tokens_with_metadata()` Salsa query returns `LexOutput`
  - Blocked by: Salsa trait derives on `LexOutput`
- [ ] Early cutoff verified
  - Works now with `TokenList`, need to verify with `LexOutput`

### Trivia Collection
- [ ] **Decision**: Keep inline trivia collection or extract `TriviaCollector`?
- [ ] **Decision**: Blank line behavior with comments (reset counter or not?)
- [ ] If extracting: Implement `TriviaCollector` pattern
- [ ] `TriviaCollector` (or inline code) produces correct `ModuleExtra` (v2-conventions SS7)
  - Current: Direct field population ✓
  - V2: Via `TriviaCollector::finish()` if extracted

### Doc Comments
- [ ] Doc comment classification per approved proposal (`simplified-doc-comments-proposal.md`)
  - Markers: `*` (member), `!` (warning), `>` (example), none (description)
  - `CommentKind` V2 target: `Regular`, `DocDescription`, `DocMember`, `DocWarning`, `DocExample`
  - `CommentKind` current: `Regular`, `DocDescription`, `DocParam`, `DocField`, `DocWarning`, `DocExample` (V2 merges `DocParam`/`DocField` into `DocMember`)
  - Legacy `#`/`@param`/`@field` recognized with deprecation warnings
- [ ] Detached doc comment warnings implemented (Gleam pattern)
  - Depends on: Finalized doc comment classification

### Integration & Testing
- [ ] Formatter works correctly with V2 metadata
- [ ] `./test-all.sh` passes
- [ ] Update capacity heuristic from /4 to /6 (if needed for V2)

**Exit Criteria:** The V2 lexer integrates seamlessly with Salsa queries via `LexOutput`. Early cutoff works correctly. Trivia collection (inline or `TriviaCollector`) produces accurate `ModuleExtra`. Doc comments are classified per approved proposal (`*`, `!`, `>`, none). Detached doc comments produce warnings. Full test suite passes.

**Blockers:**
1. ~~Grammar vs implementation mismatch for doc comment markers~~ — **RESOLVED** (see `simplified-doc-comments-proposal.md`)
2. LexOutput Salsa compatibility (needs derives; current `LexOutput` only has `Clone, Default`)
3. Storage pattern decision (keep hybrid AoS+tags or switch to full SoA?)
4. `DocMember` variant must be added to `CommentKind` (replacing `DocParam`/`DocField`) before V2 classification logic can be implemented
5. Formatter (`ori_fmt/src/comments.rs`) depends on `DocParam`/`DocField` for `@param`/`@field` reordering -- must be updated when migrating to `DocMember`
