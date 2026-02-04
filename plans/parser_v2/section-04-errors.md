---
section: "04"
title: Structured Errors
status: not-started
goal: Build Elm-quality error messages with actionable suggestions
sections:
  - id: "04.1"
    title: ParseErrorDetails Structure
    status: not-started
  - id: "04.2"
    title: Empathetic Message Templates
    status: not-started
  - id: "04.3"
    title: Common Mistake Detection
    status: not-started
  - id: "04.4"
    title: Cross-file Error Labels
    status: not-started
---

# Section 04: Structured Errors

**Status:** 📋 Planned
**Goal:** Gold-standard error messages (Elm quality) with actionable suggestions
**Source:** Elm (`compiler/src/Reporting/`), Gleam (`compiler-core/src/parse/error.rs`)

---

## Background

Elm and Gleam are renowned for exceptional error messages:

```
-- UNEXPECTED TOKEN ------------------------------------ src/Main.elm

I ran into something unexpected when parsing this if-expression:

    if count > 0 then
        "positive"
    else
        count
        ^^^^^

I was expecting an expression, but I found `count` which looks like the
start of a new statement.

Hint: Maybe you wanted to write `else if count == 0 then` to check
another condition?
```

Key characteristics:
1. **Empathetic language** — "I ran into..." not "Error:"
2. **Visual context** — Shows relevant code with pointer
3. **Explanation** — What was expected, what was found
4. **Educational** — Why this is wrong
5. **Actionable** — Specific suggestion to fix

---

## 04.1 ParseErrorDetails Structure

**Goal:** Define comprehensive error detail type

### Tasks

- [ ] Design `ParseErrorDetails` struct
  ```rust
  #[derive(Clone, Debug)]
  pub struct ParseErrorDetails {
      /// Error title (e.g., "UNEXPECTED TOKEN")
      pub title: &'static str,

      /// Main explanation text
      pub text: String,

      /// Inline label at error location
      pub label_text: String,

      /// Additional labels (for related locations)
      pub extra_labels: Vec<ExtraLabel>,

      /// Actionable suggestion
      pub hint: Option<String>,

      /// Code suggestion (for auto-fix)
      pub suggestion: Option<CodeSuggestion>,

      /// Structured error code
      pub error_code: ErrorCode,
  }
  ```

- [ ] Design `ExtraLabel` for cross-references
  ```rust
  #[derive(Clone, Debug)]
  pub struct ExtraLabel {
      /// Source location (may be in different file)
      pub span: Span,
      /// Optional: different source if cross-file
      pub src_info: Option<SourceInfo>,
      /// Label text
      pub text: String,
  }
  ```

- [ ] Design `CodeSuggestion` for auto-fixes
  ```rust
  #[derive(Clone, Debug)]
  pub struct CodeSuggestion {
      /// What to replace
      pub span: Span,
      /// Replacement text
      pub replacement: String,
      /// Description of the fix
      pub message: String,
      /// Confidence level
      pub applicability: Applicability,
  }

  #[derive(Clone, Copy, Debug)]
  pub enum Applicability {
      /// Safe to apply automatically
      MachineApplicable,
      /// May need human review
      MaybeIncorrect,
      /// Just a hint, don't auto-apply
      HasPlaceholders,
  }
  ```

- [ ] Implement `details()` method on error types
  ```rust
  impl ParseErrorKind {
      pub fn details(&self, source: &str, context: &ParseContext) -> ParseErrorDetails {
          match self {
              // 50+ specific error variants
          }
      }
  }
  ```

---

## 04.2 Empathetic Message Templates

**Goal:** Human-friendly error phrasing

### Tasks

- [ ] Create message template system
  ```rust
  mod error_templates {
      pub const UNEXPECTED_TOKEN: &str =
          "I ran into something unexpected while parsing {context}:";

      pub const EXPECTED_EXPRESSION: &str =
          "I was expecting an expression here, but I found {found}.";

      pub const UNCLOSED_DELIMITER: &str =
          "I found an unclosed {open}. I think it was opened here:";

      // ... more templates
  }
  ```

- [ ] Implement error message builder
  ```rust
  impl ParseErrorKind {
      pub fn details(&self, source: &str, ctx: &ParseContext) -> ParseErrorDetails {
          match self {
              Self::UnexpectedToken { found, expected } => {
                  let found_name = found.friendly_name();
                  let expected_list = expected.friendly_list();

                  ParseErrorDetails {
                      title: "UNEXPECTED TOKEN",
                      text: format!(
                          "I was expecting {}, but I found {}.",
                          expected_list,
                          found_name,
                      ),
                      label_text: format!("unexpected {}", found_name),
                      hint: self.suggest_fix(found, expected, ctx),
                      ..Default::default()
                  }
              }

              Self::UnclosedDelimiter { open, open_span, expected_close } => {
                  ParseErrorDetails {
                      title: "UNCLOSED DELIMITER",
                      text: format!(
                          "I found an unclosed `{}`. I expected a matching `{}`.",
                          open.symbol(),
                          expected_close.symbol(),
                      ),
                      label_text: "expected closing delimiter here".into(),
                      extra_labels: vec![
                          ExtraLabel {
                              span: *open_span,
                              text: format!("the `{}` was opened here", open.symbol()),
                              src_info: None,
                          }
                      ],
                      hint: Some(format!("Add a `{}` to close this {}",
                          expected_close.symbol(),
                          open.name(),
                      )),
                      ..Default::default()
                  }
              }

              // ... 50+ more variants
          }
      }
  }
  ```

- [ ] Add formatting helpers
  ```rust
  impl TokenTypeSet {
      /// Generate human-readable list: "`,`, `)`, or `}`"
      pub fn friendly_list(&self) -> String {
          let names: Vec<_> = self.iter()
              .map(|t| format!("`{}`", t.symbol()))
              .collect();

          match names.len() {
              0 => "nothing".into(),
              1 => names[0].clone(),
              2 => format!("{} or {}", names[0], names[1]),
              _ => {
                  let (last, rest) = names.split_last().unwrap();
                  format!("{}, or {}", rest.join(", "), last)
              }
          }
      }
  }
  ```

---

## 04.3 Common Mistake Detection

**Goal:** Recognize patterns of frequent errors and provide targeted help

### Tasks

- [ ] Identify common mistakes in Ori
  - [ ] Using `;` (Ori doesn't use semicolons)
  - [ ] Using `return` (Ori has no return keyword)
  - [ ] Using `mut` (variables are mutable by default)
  - [ ] Using `(` for grouping (use `{` in Ori)
  - [ ] Using `class` (Ori uses `type` + `trait`)
  - [ ] Using `===` instead of `==`
  - [ ] Using `<>` instead of `!=`

- [ ] Implement detection and suggestions
  ```rust
  impl ParseErrorKind {
      fn check_common_mistakes(
          found: &Token,
          expected: &TokenTypeSet,
          context: &ParseContext,
      ) -> Option<String> {
          match found.kind {
              TokenKind::Semicolon => Some(
                  "Ori doesn't use semicolons. Remove the `;` and the \
                   expression's value will flow to the next line.".into()
              ),

              TokenKind::Return => Some(
                  "Ori has no `return` keyword. The last expression in a \
                   block is automatically its value. Just write the \
                   expression directly.".into()
              ),

              TokenKind::Mut => Some(
                  "In Ori, variables are mutable by default. Use `$name` \
                   (with a `$` prefix) to create an immutable binding.".into()
              ),

              TokenKind::Class => Some(
                  "Ori doesn't have classes. Use `type` for data structures \
                   and `trait` for shared behavior.".into()
              ),

              _ => None,
          }
      }
  }
  ```

- [ ] Add pattern-based suggestions
  ```rust
  fn suggest_from_pattern(
      source: &str,
      error_span: Span,
      context: &ParseContext,
  ) -> Option<CodeSuggestion> {
      let text = &source[error_span.start..error_span.end];

      // Detect `i++` pattern
      if text.ends_with("++") {
          return Some(CodeSuggestion {
              span: error_span,
              replacement: format!("{} = {} + 1",
                  &text[..text.len()-2],
                  &text[..text.len()-2],
              ),
              message: "Ori doesn't have `++`. Use assignment instead.".into(),
              applicability: Applicability::MaybeIncorrect,
          });
      }

      // Detect `===` pattern
      if text == "===" {
          return Some(CodeSuggestion {
              span: error_span,
              replacement: "==".into(),
              message: "Ori uses `==` for equality (there's no `===`).".into(),
              applicability: Applicability::MachineApplicable,
          });
      }

      None
  }
  ```

- [ ] Add educational context for language differences
  ```rust
  fn educational_note(context: &ParseContext) -> Option<String> {
      match context {
          ParseContext::IfExpression => Some(
              "Note: In Ori, `if` is an expression that returns a value, \
               not a statement. Both branches must have the same type.".into()
          ),

          ParseContext::MatchExpression => Some(
              "Note: Ori requires match expressions to be exhaustive. \
               All possible cases must be handled.".into()
          ),

          _ => None,
      }
  }
  ```

---

## 04.4 Cross-file Error Labels

**Goal:** Show related code from other files when relevant

### Tasks

- [ ] Extend `ExtraLabel` with source info
  ```rust
  #[derive(Clone, Debug)]
  pub struct SourceInfo {
      pub path: PathBuf,
      pub content: String,  // Or reference to cached content
  }
  ```

- [ ] Implement cross-file label rendering
  ```rust
  impl ParseErrorDetails {
      pub fn render(&self, primary_source: &str) -> String {
          let mut output = String::new();

          // Primary error
          writeln!(output, "-- {} --", self.title);
          writeln!(output);
          writeln!(output, "{}", self.text);
          writeln!(output);

          // Primary location
          render_code_snippet(&mut output, primary_source, self.primary_span, &self.label_text);

          // Extra labels
          for label in &self.extra_labels {
              writeln!(output);
              let source = label.src_info
                  .as_ref()
                  .map(|s| s.content.as_str())
                  .unwrap_or(primary_source);
              render_code_snippet(&mut output, source, label.span, &label.text);
          }

          // Hint
          if let Some(hint) = &self.hint {
              writeln!(output);
              writeln!(output, "Hint: {}", hint);
          }

          output
      }
  }
  ```

- [ ] Use case: Import errors referencing definition site
  ```rust
  // Error in src/main.ori points to definition in src/lib.ori
  ExtraLabel {
      span: definition_span,
      src_info: Some(SourceInfo {
          path: "src/lib.ori".into(),
          content: lib_source.clone(),
      }),
      text: "the function is defined here".into(),
  }
  ```

---

## 04.5 Completion Checklist

- [ ] `ParseErrorDetails` structure implemented
- [ ] All error variants have detailed messages
- [ ] Empathetic language used throughout
- [ ] Common mistakes detected and explained
- [ ] Cross-file labels working
- [ ] Error rendering produces beautiful output

**Exit Criteria:**
- Error messages match Elm/Gleam quality
- All common mistakes have targeted suggestions
- Code suggestions are accurate and helpful
- Users report improved error experience
