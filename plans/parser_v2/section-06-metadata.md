---
section: "06"
title: Formatting Metadata
status: not-started
goal: Preserve non-semantic information for lossless formatting and IDE support
sections:
  - id: "06.1"
    title: ModuleExtra Structure
    status: not-started
  - id: "06.2"
    title: Comment Collection
    status: not-started
  - id: "06.3"
    title: SpaceBefore/SpaceAfter Pattern
    status: not-started
  - id: "06.4"
    title: Detached Doc Comment Warnings
    status: not-started
---

# Section 06: Formatting Metadata

**Status:** 📋 Planned
**Goal:** Lossless roundtrip formatting and full IDE metadata support
**Source:** Gleam (`compiler-core/src/parse/extra.rs`), Roc (`crates/compiler/parse/src/ast.rs`)

---

## Background

Traditional parsers discard "trivia" (comments, whitespace). This creates problems:

1. **Formatter loses information** — Can't preserve user's intentional formatting
2. **IDE features incomplete** — Comments not available for hover/docs
3. **Refactoring destroys layout** — Extract function loses surrounding comments

Solution: Collect all non-semantic information in `ModuleExtra` alongside the AST.

---

## 06.1 ModuleExtra Structure

**Goal:** Comprehensive metadata collection for formatters and IDEs

### Tasks

- [ ] Design `ModuleExtra` struct
  ```rust
  #[derive(Debug, Default)]
  pub struct ModuleExtra {
      /// Module-level comments (//! or equivalent)
      pub module_comments: Vec<CommentSpan>,

      /// Documentation comments (/// or equivalent)
      pub doc_comments: Vec<DocComment>,

      /// Regular comments (// or equivalent)
      pub comments: Vec<CommentSpan>,

      /// Blank line positions (for formatting preservation)
      pub blank_lines: Vec<u32>,

      /// Newline positions (for line counting)
      pub newlines: Vec<u32>,

      /// Trailing comma positions (for style preservation)
      pub trailing_commas: Vec<u32>,
  }
  ```

- [ ] Define comment types
  ```rust
  #[derive(Debug, Clone)]
  pub struct CommentSpan {
      pub span: Span,
      pub kind: CommentKind,
  }

  #[derive(Debug, Clone, Copy)]
  pub enum CommentKind {
      Line,        // //
      Block,       // /* */
      DocLine,     // ///
      DocBlock,    // /** */
      ModuleLine,  // //!
      ModuleBlock, // /*! */
  }

  #[derive(Debug, Clone)]
  pub struct DocComment {
      pub span: Span,
      pub content: String,
      pub attached_to: Option<NodeIdx>,
  }
  ```

- [ ] Implement collection during parsing
  ```rust
  impl Parser<'_> {
      fn skip_trivia(&mut self) {
          loop {
              match self.current_kind() {
                  TokenKind::Comment(kind) => {
                      self.extra.comments.push(CommentSpan {
                          span: self.current_span(),
                          kind,
                      });
                      self.advance();
                  }
                  TokenKind::Newline => {
                      self.extra.newlines.push(self.current_position());
                      self.advance();
                  }
                  TokenKind::BlankLine => {
                      self.extra.blank_lines.push(self.current_position());
                      self.advance();
                  }
                  _ => break,
              }
          }
      }
  }
  ```

- [ ] Return `ModuleExtra` with parse result
  ```rust
  pub struct ParsedModule {
      pub ast: Module,
      pub extra: ModuleExtra,
      pub errors: Vec<ParseError>,
  }
  ```

---

## 06.2 Comment Collection

**Goal:** Collect and categorize all comments for IDE and formatter use

### Tasks

- [ ] Implement comment collection in lexer
  ```rust
  impl Lexer<'_> {
      fn scan_comment(&mut self) -> Token {
          let start = self.position();

          if self.peek() == '/' {
              // Line comment
              self.advance(); // second /

              let kind = if self.peek() == '/' {
                  self.advance();
                  CommentKind::DocLine
              } else if self.peek() == '!' {
                  self.advance();
                  CommentKind::ModuleLine
              } else {
                  CommentKind::Line
              };

              // Consume until newline
              while !self.is_at_end() && self.peek() != '\n' {
                  self.advance();
              }

              Token::new(TokenKind::Comment(kind), Span::new(start, self.position()))
          } else if self.peek() == '*' {
              // Block comment (handle nesting)
              self.scan_block_comment(start)
          } else {
              // Just a slash
              Token::new(TokenKind::Slash, Span::new(start, self.position()))
          }
      }
  }
  ```

- [ ] Track comment positions relative to declarations
  ```rust
  impl ModuleExtra {
      /// Get comments immediately before a position
      pub fn comments_before(&self, pos: u32) -> Vec<&CommentSpan> {
          self.comments
              .iter()
              .filter(|c| c.span.end <= pos && c.span.end + 2 >= pos)
              .collect()
      }

      /// Get doc comments that should attach to a declaration
      pub fn doc_comments_for(&self, decl_start: u32) -> Vec<&DocComment> {
          self.doc_comments
              .iter()
              .filter(|d| {
                  d.span.end <= decl_start &&
                  !self.has_blank_line_between(d.span.end, decl_start)
              })
              .collect()
      }

      fn has_blank_line_between(&self, start: u32, end: u32) -> bool {
          self.blank_lines.iter().any(|&pos| pos > start && pos < end)
      }
  }
  ```

- [ ] Attach doc comments to declarations during parsing
  ```rust
  impl Parser<'_> {
      fn parse_function(&mut self) -> ParseResult<Function> {
          // Collect pending doc comments
          let docs = self.take_pending_doc_comments();

          self.expect(&TokenKind::Fn)?;
          let name = self.parse_ident()?;
          // ...

          Ok(Function {
              docs,
              name,
              // ...
          })
      }

      fn take_pending_doc_comments(&mut self) -> Vec<DocComment> {
          let until = self.current_position();
          self.extra.doc_comments
              .drain_filter(|d| d.span.end <= until && d.attached_to.is_none())
              .collect()
      }
  }
  ```

---

## 06.3 SpaceBefore/SpaceAfter Pattern

**Goal:** Preserve exact whitespace around AST nodes (from Roc)

### Tasks

- [ ] Design `Spaced` wrapper type
  ```rust
  /// Wraps a value with optional surrounding whitespace/comments
  #[derive(Debug, Clone)]
  pub enum Spaced<'a, T> {
      /// Just the item, no surrounding trivia
      Item(T),

      /// Trivia before the item
      SpaceBefore(&'a Spaced<'a, T>, &'a [CommentOrNewline<'a>]),

      /// Trivia after the item
      SpaceAfter(&'a Spaced<'a, T>, &'a [CommentOrNewline<'a>]),
  }

  #[derive(Debug, Clone)]
  pub enum CommentOrNewline<'a> {
      Comment(&'a str),
      DocComment(&'a str),
      Newline,
      BlankLine,
  }
  ```

- [ ] Implement accessor methods
  ```rust
  impl<'a, T> Spaced<'a, T> {
      /// Get the inner value, ignoring trivia
      pub fn value(&self) -> &T {
          match self {
              Spaced::Item(t) => t,
              Spaced::SpaceBefore(inner, _) => inner.value(),
              Spaced::SpaceAfter(inner, _) => inner.value(),
          }
      }

      /// Get all trivia before the item
      pub fn space_before(&self) -> Vec<&CommentOrNewline<'a>> {
          match self {
              Spaced::Item(_) => vec![],
              Spaced::SpaceBefore(inner, space) => {
                  let mut result = inner.space_before();
                  result.extend(space.iter());
                  result
              }
              Spaced::SpaceAfter(inner, _) => inner.space_before(),
          }
      }

      /// Get all trivia after the item
      pub fn space_after(&self) -> Vec<&CommentOrNewline<'a>> {
          match self {
              Spaced::Item(_) => vec![],
              Spaced::SpaceBefore(inner, _) => inner.space_after(),
              Spaced::SpaceAfter(inner, space) => {
                  let mut result = inner.space_after();
                  result.extend(space.iter());
                  result
              }
          }
      }
  }
  ```

- [ ] Use in AST where trivia matters
  ```rust
  pub struct FunctionDef<'a> {
      pub name: Spaced<'a, Name>,
      pub params: Vec<Spaced<'a, Param>>,
      pub body: Spaced<'a, NodeIdx>,
  }
  ```

- [ ] Alternative: Simpler attached trivia model
  ```rust
  // If full Spaced is too complex, use simpler attachment
  pub struct WithTrivia<T> {
      pub leading: Vec<Trivia>,
      pub value: T,
      pub trailing: Vec<Trivia>,
  }

  #[derive(Debug, Clone)]
  pub enum Trivia {
      Whitespace(u32),  // count of spaces
      Newline,
      Comment(String),
  }
  ```

---

## 06.4 Detached Doc Comment Warnings

**Goal:** Warn when doc comments don't attach to any declaration

### Tasks

- [ ] Track unattached doc comments
  ```rust
  impl Parser<'_> {
      fn check_detached_doc_comments(&mut self) {
          for doc in &self.extra.doc_comments {
              if doc.attached_to.is_none() {
                  self.warnings.push(ParseWarning::DetachedDocComment {
                      span: doc.span,
                      hint: "This doc comment isn't attached to any declaration. \
                             Did you mean to put it directly above a function or type?",
                  });
              }
          }
      }
  }
  ```

- [ ] Detect common causes
  ```rust
  fn diagnose_detached_doc(&self, doc: &DocComment) -> Option<String> {
      let next_token_pos = self.next_non_trivia_position(doc.span.end);

      // Check if there's a blank line between doc and next item
      if self.extra.has_blank_line_between(doc.span.end, next_token_pos) {
          return Some(
              "There's a blank line between this doc comment and the next \
               declaration. Remove the blank line to attach the comment.".into()
          );
      }

      // Check if there's a regular comment interrupting
      if self.extra.has_comment_between(doc.span.end, next_token_pos) {
          return Some(
              "A regular comment is interrupting this doc comment. \
               Doc comments must be immediately before the declaration.".into()
          );
      }

      None
  }
  ```

- [ ] Add to warning output
  ```rust
  pub enum ParseWarning {
      DetachedDocComment {
          span: Span,
          hint: &'static str,
      },
      // ... other warnings
  }
  ```

---

## 06.5 Completion Checklist

- [ ] `ModuleExtra` collects all trivia
- [ ] Comments categorized by type
- [ ] Doc comments attached to declarations
- [ ] Blank lines tracked for formatting
- [ ] Trailing commas tracked for style
- [ ] Detached doc comment warnings implemented
- [ ] Formatter can do lossless roundtrip

**Exit Criteria:**
- `parse(source) -> format(ast, extra) == source` for well-formatted files
- IDE hover shows doc comments correctly
- Formatter preserves intentional blank lines
- Warnings help users fix doc comment placement
