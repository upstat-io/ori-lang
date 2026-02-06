---
section: "08"
title: Parser Integration
status: not-started
goal: Seamless integration with Parser V2
sections:
  - id: "08.1"
    title: Trivia Preservation
    status: not-started
  - id: "08.2"
    title: Comment Classification
    status: not-started
  - id: "08.3"
    title: Incremental Lexing Support
    status: not-started
  - id: "08.4"
    title: Whitespace-Sensitive Parsing
    status: not-started
---

# Section 08: Parser Integration

**Status:** 📋 Planned
**Goal:** Seamless integration with Parser V2
**Source:** Gleam (`ModuleExtra`), TypeScript, Roc

> **Conventions:** Follows `plans/v2-conventions.md` §6 (Phase Output), §7 (Shared Types in ori_ir)

---

## Background

The lexer must integrate smoothly with Parser V2 (see `plans/parser_v2/`):

| Parser V2 Requirement | Lexer V2 Solution |
|----------------------|-------------------|
| Trivia preservation (Section 6) | `ModuleExtra` structure |
| Expected tokens (Section 3) | `TokenSet` from `TokenTag` |
| Incremental parsing (Section 5) | Range re-lexing API |
| Error context (Section 4) | Rich `LexError` types |

### Existing Parser Infrastructure (2026-02-06)

The parser already has significant tag-based infrastructure that the lexer V2 `TokenStorage` must integrate with seamlessly. These are not workarounds — they're the **established contract** between lexer output and parser consumption:

| Parser Component | Location | What It Does | Lexer V2 Must Provide |
|-----------------|----------|-------------|----------------------|
| `Cursor::current_tag() -> u8` | `cursor.rs` | Reads tag from `tags: &[u8]` slice | `TokenStorage.tags` as a contiguous `&[u8]` or `&[TokenTag]` slice |
| `OPER_TABLE[128]` | `operators.rs` | Static binding power lookup indexed by tag | Tag values < 128 for all operator tokens |
| `POSTFIX_BITSET` | `postfix.rs` | Two-u64 bitset for postfix token membership | Stable tag values so bitset indices don't change |
| `parse_primary()` fast path | `primary.rs` | Direct tag match before `one_of!` macro | `TAG_*` constants or equivalent discriminant values |
| `match_unary_op()` | `operators.rs` | Tag-based unary operator detection | Consistent tag values for `-`, `!`, `~` |
| `match_function_exp_kind()` | `operators.rs` | Tag-based keyword detection for patterns | Consistent tag values for `recurse`, `parallel`, etc. |
| Branchless `advance()` | `cursor.rs` | No bounds check, relies on EOF sentinel | EOF token always present at end of storage |
| `#[cold]` split `expect()` | `cursor.rs` | Error path isolated for LLVM inlining | Compatible error types |

**Key constraint:** The `Cursor` reads the tag array as a `&[u8]` slice. The lexer V2's `TokenStorage.tags` must be layout-compatible — either `Vec<u8>` directly, or `Vec<TokenTag>` where `TokenTag` is `#[repr(u8)]` so it can be safely transmuted to `&[u8]` for the cursor.

---

## 08.1 Trivia Preservation

**Goal:** Preserve comments, whitespace, and blank lines for formatter

### Tasks

- [ ] Use `ModuleExtra` from `ori_ir` (conventions §7 — shared types live in ori_ir)

  > **Note:** `ModuleExtra` is defined in `ori_ir::metadata` (not here).
  > The lexer populates it; the parser may extend it (e.g., trailing commas).
  > See `plans/v2-conventions.md` §7: all types that cross phase boundaries are defined once in `ori_ir`.

  The existing `ori_ir::metadata::ModuleExtra` structure includes:
  - `comments: Vec<Comment>` — all comments in the module
  - `blank_lines: Vec<u32>` — positions of blank lines
  - `newlines: Vec<u32>` — positions of newlines
  - `trailing_commas: Vec<Span>` — trailing commas (populated by parser)

  Comment classification (`CommentKind::Line`, `Block`, `DocLine`, `DocBlock`, `ModuleLine`, `ModuleBlock`) is also defined in `ori_ir`.

- [ ] Collect trivia during lexing
  ```rust
  pub struct TriviaCollector {
      comments: Vec<Comment>,
      blank_lines: Vec<u32>,
      newlines: Vec<u32>,
      last_line_end: u32,
      consecutive_newlines: u32,
  }

  impl TriviaCollector {
      /// Returns `ori_ir::ModuleExtra` — the shared metadata type (conventions §7)
      pub fn on_newline(&mut self, pos: u32) {
          self.newlines.push(pos);
          self.consecutive_newlines += 1;

          // Blank line = two consecutive newlines
          if self.consecutive_newlines >= 2 {
              self.blank_lines.push(pos);
          }

          self.last_line_end = pos;
      }

      pub fn on_non_trivia(&mut self) {
          self.consecutive_newlines = 0;
      }

      pub fn on_comment(&mut self, kind: CommentKind, span: Span, content: String) {
          self.comments.push(Comment { kind, span, content });
      }

      /// Produce `ori_ir::ModuleExtra` (conventions §6, §7)
      pub fn finish(self) -> ori_ir::ModuleExtra {
          ori_ir::ModuleExtra {
              comments: self.comments,
              blank_lines: self.blank_lines,
              newlines: self.newlines,
              trailing_commas: Vec::new(), // Filled by parser
          }
      }
  }
  ```

- [ ] Integrate with lexer API — return `LexOutput` (conventions §6)
  ```rust
  /// Phase output for the lexer (conventions §6 — Phase Output Shape).
  /// Immutable after creation. Next phase borrows read-only.
  pub struct LexOutput {
      pub tokens: TokenStorage,
      pub errors: Vec<LexError>,
      pub metadata: ori_ir::ModuleExtra,  // Shared type (conventions §7)
  }

  /// Lex with trivia collection (for formatter)
  pub fn lex_with_trivia(
      source: &str,
      interner: &StringInterner,
  ) -> LexOutput {
      let mut tokenizer = Tokenizer::new(source);
      let mut storage = TokenStorage::with_capacity(source.len());
      let mut trivia = TriviaCollector::new();

      loop {
          let token = tokenizer.next_token();

          match token.tag {
              RawTag::Eof => break,

              RawTag::Newline => {
                  trivia.on_newline(token.start);
                  // Don't add to token storage
              }

              RawTag::LineComment | RawTag::BlockComment => {
                  let kind = match token.tag {
                      RawTag::LineComment => CommentKind::Line,
                      RawTag::BlockComment => CommentKind::Block,
                      _ => unreachable!(),
                  };
                  let span = Span::new(token.start, token.start + token.len);
                  let content = source[span.start as usize..span.end as usize].to_string();
                  trivia.on_comment(kind, span, content);
              }

              RawTag::DocComment => {
                  let span = Span::new(token.start, token.start + token.len);
                  let content = source[span.start as usize..span.end as usize].to_string();
                  trivia.on_comment(CommentKind::DocLine, span, content);
              }

              _ => {
                  trivia.on_non_trivia();
                  storage.push_token(token, source, interner);
              }
          }
      }

      let (_, errors) = tokenizer.finish();
      LexOutput {
          tokens: storage,
          errors,
          metadata: trivia.finish(),
      }
  }
  ```

---

## 08.2 Comment Classification

**Goal:** Classify comments for documentation and formatting

### Tasks

- [ ] Detect doc comments during lexing
  ```rust
  fn handle_slash(&mut self, tag: &mut RawTag) -> State {
      match self.current() {
          b'/' => {
              self.advance();
              if self.current() == b'/' {
                  // /// doc comment
                  self.advance();
                  *tag = RawTag::DocComment;
                  return State::LineComment;
              } else if self.current() == b'!' {
                  // //! module doc
                  self.advance();
                  *tag = RawTag::ModuleDocComment;
                  return State::LineComment;
              }
              *tag = RawTag::LineComment;
              State::LineComment
          }

          b'*' => {
              self.advance();
              if self.current() == b'*' && self.peek() != b'/' {
                  // /** doc comment */
                  self.advance();
                  *tag = RawTag::DocBlockComment;
                  return State::BlockComment;
              } else if self.current() == b'!' {
                  // /*! module doc */
                  self.advance();
                  *tag = RawTag::ModuleDocBlockComment;
                  return State::BlockComment;
              }
              *tag = RawTag::BlockComment;
              State::BlockComment
          }

          _ => {
              *tag = RawTag::Slash;
              State::Done
          }
      }
  }
  ```

- [ ] Associate doc comments with declarations
  ```rust
  /// Attach doc comments to their following declarations
  pub fn attach_doc_comments(
      tokens: &TokenStorage,
      extra: &mut ModuleExtra,
  ) -> Vec<AttachedDocComment> {
      let mut attached = Vec::new();
      let mut pending_docs: Vec<&Comment> = Vec::new();

      for comment in &extra.comments {
          if matches!(comment.kind, CommentKind::DocLine | CommentKind::DocBlock) {
              pending_docs.push(comment);
          }
      }

      // Match with following non-comment tokens
      // (More sophisticated matching in parser)

      attached
  }

  pub struct AttachedDocComment {
      pub comment_span: Span,
      pub declaration_span: Span,
      pub content: String,
  }
  ```

- [ ] Warn on detached doc comments (Gleam pattern)
  ```rust
  /// Check for doc comments not attached to declarations
  pub fn check_detached_docs(
      extra: &ModuleExtra,
      declarations: &[DeclSpan],
  ) -> Vec<Warning> {
      let mut warnings = Vec::new();

      for comment in &extra.comments {
          if !matches!(comment.kind, CommentKind::DocLine | CommentKind::DocBlock) {
              continue;
          }

          // Find next declaration
          let next_decl = declarations.iter()
              .find(|d| d.start > comment.span.end);

          match next_decl {
              Some(decl) if has_blank_line_between(comment.span.end, decl.start, extra) => {
                  warnings.push(Warning::DetachedDocComment {
                      comment: comment.span,
                      hint: "Doc comments should be directly above their declaration",
                  });
              }
              None => {
                  warnings.push(Warning::DetachedDocComment {
                      comment: comment.span,
                      hint: "This doc comment is not attached to any declaration",
                  });
              }
              _ => {}
          }
      }

      warnings
  }
  ```

---

## 08.3 Incremental Lexing Support

**Goal:** Enable efficient re-lexing for IDE scenarios

### Tasks

- [ ] Implement range tokenization
  ```rust
  /// Tokenize a specific range of source
  pub fn tokenize_range(
      source: &str,
      start: u32,
      end: u32,
  ) -> impl Iterator<Item = RawToken> + '_ {
      let range = &source[start as usize..end as usize];
      Tokenizer::new(range.as_bytes())
          .map(move |mut tok| {
              // Adjust positions to be relative to full source
              tok.start += start;
              tok
          })
  }
  ```

- [ ] Define reusability predicates for tokens
  ```rust
  impl TokenTag {
      /// Can this token be reused after an edit?
      /// Returns false for tokens that might span the edit point
      pub fn is_atomic(self) -> bool {
          match self {
              // These can always be reused
              TokenTag::Ident
              | TokenTag::Int
              | TokenTag::Float
              | TokenTag::LParen
              | TokenTag::RParen
              // ... simple tokens
              => true,

              // These might span edit points
              TokenTag::String
              | TokenTag::RawString
              | TokenTag::BlockComment
              | TokenTag::DocBlockComment
              => false,

              // Keywords are atomic
              _ if self.is_keyword() => true,

              _ => true,
          }
      }
  }
  ```

- [ ] Implement LazyTokens pattern (from Rust)
  ```rust
  /// Lazy token reconstruction for incremental parsing
  pub struct LazyTokens<'a> {
      source: &'a str,
      storage: &'a TokenStorage,
  }

  impl<'a> LazyTokens<'a> {
      /// Get token text, re-lexing if needed
      pub fn token_text(&self, index: usize) -> &'a str {
          let tag = self.storage.tag(index);
          let start = self.storage.start(index) as usize;

          if let Some(lexeme) = tag.lexeme() {
              // Fixed token - return static string
              return lexeme;
          }

          // Variable token - need to find end
          let end = self.storage.token_end(index) as usize;
          &self.source[start..end]
      }

      /// Re-lex a range of tokens
      pub fn relex_range(&self, start_idx: usize, end_idx: usize) -> Vec<RawToken> {
          let start_pos = self.storage.start(start_idx);
          let end_pos = self.storage.token_end(end_idx);

          tokenize_range(self.source, start_pos, end_pos).collect()
      }
  }
  ```

- [ ] Integrate with Parser V2's SyntaxCursor
  ```rust
  /// Bridge between lexer and parser for incremental updates
  pub struct TokenBridge<'a> {
      old_tokens: &'a TokenStorage,
      new_tokens: TokenStorage,
      edit_range: TextRange,
      position_delta: i32,
  }

  impl<'a> TokenBridge<'a> {
      /// Check if old token can be reused
      pub fn can_reuse(&self, old_index: usize) -> bool {
          let start = self.old_tokens.start(old_index);
          let end = self.old_tokens.token_end(old_index);

          // Token is before edit - reuse with position adjustment
          if end < self.edit_range.start {
              return true;
          }

          // Token is after edit - reuse with position adjustment
          if start > self.edit_range.end {
              return true;
          }

          // Token overlaps edit - must re-lex
          false
      }
  }
  ```

---

## 08.4 Whitespace-Sensitive Parsing

**Goal:** Support space-aware syntax disambiguation

### Tasks

- [ ] Expose whitespace info to parser
  ```rust
  impl Cursor<'_> {
      /// Check if current token is adjacent to previous (no space)
      pub fn is_adjacent(&self) -> bool {
          self.storage.flags(self.pos).contains(TokenFlags::ADJACENT)
      }

      /// Check if current token has preceding space
      pub fn has_space_before(&self) -> bool {
          self.storage.flags(self.pos).contains(TokenFlags::SPACE_BEFORE)
      }

      /// Check if current token has preceding newline
      pub fn has_newline_before(&self) -> bool {
          self.storage.flags(self.pos).contains(TokenFlags::NEWLINE_BEFORE)
      }

      /// Check if at start of line
      pub fn at_line_start(&self) -> bool {
          self.storage.flags(self.pos).contains(TokenFlags::LINE_START)
      }
  }
  ```

- [ ] Implement space-sensitive disambiguation
  ```rust
  impl Parser<'_> {
      /// Parse function call vs grouped expression
      /// `foo(x)` = call, `foo (x)` = function ref + grouped
      fn parse_postfix(&mut self, lhs: Expr) -> ParseResult<Expr> {
          match self.cursor.current_tag() {
              TokenTag::LParen if self.cursor.is_adjacent() => {
                  // No space before ( = function call
                  self.parse_call(lhs)
              }
              TokenTag::LParen => {
                  // Space before ( = not a call
                  Ok(lhs)
              }
              TokenTag::Dot if self.cursor.is_adjacent() => {
                  // foo.bar = field access
                  self.parse_field_access(lhs)
              }
              TokenTag::Dot => {
                  // foo . bar = might be different (operator?)
                  self.parse_method_operator(lhs)
              }
              _ => Ok(lhs),
          }
      }

      /// Parse generic closing vs shift-right
      fn parse_generic_args(&mut self) -> ParseResult<Vec<Type>> {
          self.expect(TokenTag::Lt)?;
          let args = self.parse_type_list()?;

          // Handle >> as two > tokens
          if self.cursor.current_tag() == TokenTag::Gt
              && self.cursor.peek_tag() == TokenTag::Gt
              && self.cursor.is_adjacent()
          {
              // This is >> - only consume first >
              self.cursor.advance();
              return Ok(args);
          }

          self.expect(TokenTag::Gt)?;
          Ok(args)
      }
  }
  ```

- [ ] Document all space-sensitive constructs
  ```rust
  /// # Whitespace-Sensitive Syntax
  ///
  /// Ori uses whitespace to disambiguate certain constructs:
  ///
  /// | Syntax | With Space | Without Space |
  /// |--------|------------|---------------|
  /// | `foo(x)` | N/A | Function call |
  /// | `foo (x)` | Grouped expr | N/A |
  /// | `foo.bar` | N/A | Field access |
  /// | `a - b` | Subtraction | Subtraction |
  /// | `a -b` | N/A | Negative b? |
  /// | `List<T>` | N/A | Generic |
  /// | `a < b` | Comparison | Comparison |
  ```

---

## 08.5 Completion Checklist

- [ ] `ModuleExtra` structure defined
- [ ] Trivia collector implemented
- [ ] Comment classification working
- [ ] Doc comment attachment
- [ ] Detached doc comment warnings
- [ ] Range tokenization API
- [ ] Token reusability predicates
- [ ] LazyTokens implemented
- [ ] Whitespace flags exposed
- [ ] Space-sensitive parsing documented
- [ ] Integration tests with Parser V2

**Exit Criteria:**
- Formatter can preserve all whitespace/comments
- Incremental lexing works correctly
- Parser can use whitespace info for disambiguation
- All Parser V2 requirements met
