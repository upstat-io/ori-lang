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

---

## Background

The lexer must integrate smoothly with Parser V2 (see `plans/parser_v2/`):

| Parser V2 Requirement | Lexer V2 Solution |
|----------------------|-------------------|
| Trivia preservation (Section 6) | `ModuleExtra` structure |
| Expected tokens (Section 3) | `TokenSet` from `Tag` |
| Incremental parsing (Section 5) | Range re-lexing API |
| Error context (Section 4) | Rich `LexError` types |

---

## 08.1 Trivia Preservation

**Goal:** Preserve comments, whitespace, and blank lines for formatter

### Tasks

- [ ] Define `ModuleExtra` structure (Gleam pattern)
  ```rust
  /// Non-semantic information preserved for formatting
  #[derive(Clone, Debug, Default)]
  pub struct ModuleExtra {
      /// All comments in the module
      pub comments: Vec<Comment>,
      /// Positions of blank lines (empty lines)
      pub blank_lines: Vec<u32>,
      /// Positions of newlines
      pub newlines: Vec<u32>,
      /// Trailing commas in lists
      pub trailing_commas: Vec<Span>,
  }

  #[derive(Clone, Debug)]
  pub struct Comment {
      pub kind: CommentKind,
      pub span: Span,
      pub content: String,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum CommentKind {
      /// `// comment`
      Line,
      /// `/* comment */`
      Block,
      /// `/// doc comment`
      DocLine,
      /// `/** doc comment */`
      DocBlock,
      /// `//! module doc`
      ModuleLine,
      /// `/*! module doc */`
      ModuleBlock,
  }
  ```

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

      pub fn finish(self) -> ModuleExtra {
          ModuleExtra {
              comments: self.comments,
              blank_lines: self.blank_lines,
              newlines: self.newlines,
              trailing_commas: Vec::new(), // Filled by parser
          }
      }
  }
  ```

- [ ] Integrate with lexer API
  ```rust
  /// Lex with trivia collection (for formatter)
  pub fn lex_with_trivia(
      source: &str,
      interner: &StringInterner,
  ) -> (TokenStorage, ModuleExtra) {
      let mut tokenizer = Tokenizer::new(source);
      let mut storage = TokenStorage::with_source_size(source.len());
      let mut trivia = TriviaCollector::new();

      loop {
          let token = tokenizer.next_token();

          match token.tag {
              Tag::Eof => break,

              Tag::Newline => {
                  trivia.on_newline(token.start);
                  // Don't add to token storage
              }

              Tag::LineComment | Tag::BlockComment => {
                  let kind = match token.tag {
                      Tag::LineComment => CommentKind::Line,
                      Tag::BlockComment => CommentKind::Block,
                      _ => unreachable!(),
                  };
                  let span = Span::new(token.start, token.start + token.len);
                  let content = source[span.start as usize..span.end as usize].to_string();
                  trivia.on_comment(kind, span, content);
              }

              Tag::DocComment => {
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

      (storage, trivia.finish())
  }
  ```

---

## 08.2 Comment Classification

**Goal:** Classify comments for documentation and formatting

### Tasks

- [ ] Detect doc comments during lexing
  ```rust
  fn handle_slash(&mut self, tag: &mut Tag) -> State {
      match self.current() {
          b'/' => {
              self.advance();
              if self.current() == b'/' {
                  // /// doc comment
                  self.advance();
                  *tag = Tag::DocComment;
                  return State::LineComment;
              } else if self.current() == b'!' {
                  // //! module doc
                  self.advance();
                  *tag = Tag::ModuleDocComment;
                  return State::LineComment;
              }
              *tag = Tag::LineComment;
              State::LineComment
          }

          b'*' => {
              self.advance();
              if self.current() == b'*' && self.peek() != b'/' {
                  // /** doc comment */
                  self.advance();
                  *tag = Tag::DocBlockComment;
                  return State::BlockComment;
              } else if self.current() == b'!' {
                  // /*! module doc */
                  self.advance();
                  *tag = Tag::ModuleDocBlockComment;
                  return State::BlockComment;
              }
              *tag = Tag::BlockComment;
              State::BlockComment
          }

          _ => {
              *tag = Tag::Slash;
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
  impl Tag {
      /// Can this token be reused after an edit?
      /// Returns false for tokens that might span the edit point
      pub fn is_atomic(self) -> bool {
          match self {
              // These can always be reused
              Tag::Ident
              | Tag::Int
              | Tag::Float
              | Tag::LParen
              | Tag::RParen
              // ... simple tokens
              => true,

              // These might span edit points
              Tag::String
              | Tag::RawString
              | Tag::BlockComment
              | Tag::DocBlockComment
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
              Tag::LParen if self.cursor.is_adjacent() => {
                  // No space before ( = function call
                  self.parse_call(lhs)
              }
              Tag::LParen => {
                  // Space before ( = not a call
                  Ok(lhs)
              }
              Tag::Dot if self.cursor.is_adjacent() => {
                  // foo.bar = field access
                  self.parse_field_access(lhs)
              }
              Tag::Dot => {
                  // foo . bar = might be different (operator?)
                  self.parse_method_operator(lhs)
              }
              _ => Ok(lhs),
          }
      }

      /// Parse generic closing vs shift-right
      fn parse_generic_args(&mut self) -> ParseResult<Vec<Type>> {
          self.expect(Tag::Lt)?;
          let args = self.parse_type_list()?;

          // Handle >> as two > tokens
          if self.cursor.current_tag() == Tag::Gt
              && self.cursor.peek_tag() == Tag::Gt
              && self.cursor.is_adjacent()
          {
              // This is >> - only consume first >
              self.cursor.advance();
              return Ok(args);
          }

          self.expect(Tag::Gt)?;
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
