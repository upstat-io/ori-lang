---
section: "04"
title: Structured Errors
status: partial
goal: Build Elm-quality error messages with actionable suggestions
sections:
  - id: "04.1"
    title: ParseErrorDetails Structure
    status: complete
  - id: "04.2"
    title: Empathetic Message Templates
    status: complete
  - id: "04.3"
    title: Common Mistake Detection
    status: complete
  - id: "04.4"
    title: Cross-file Error Labels
    status: not-started
---

# Section 04: Structured Errors

**Status:** 🔶 Partial (04.1, 04.2, 04.3 complete)
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

**Status:** ✅ Complete (2026-02-04)
**Goal:** Define comprehensive error detail type

### Implementation Summary

The following was implemented in `compiler/ori_parse/src/error.rs`:

#### ParseErrorDetails Struct
```rust
#[derive(Clone, Debug)]
pub struct ParseErrorDetails {
    pub title: &'static str,       // "UNEXPECTED TOKEN"
    pub text: String,              // Empathetic explanation
    pub label_text: String,        // Inline label at error
    pub extra_labels: Vec<ExtraLabel>,
    pub hint: Option<String>,      // Actionable suggestion
    pub suggestion: Option<CodeSuggestion>,
    pub error_code: ErrorCode,
}
```

#### ExtraLabel for Cross-References
```rust
pub struct ExtraLabel {
    pub span: Span,
    pub src_info: Option<SourceInfo>,  // For cross-file labels
    pub text: String,
}

impl ExtraLabel {
    fn same_file(span: Span, text: impl Into<String>) -> Self { ... }
    fn cross_file(span: Span, path: impl Into<String>, content: impl Into<String>, text: impl Into<String>) -> Self { ... }
}
```

#### CodeSuggestion for Auto-Fixes
```rust
pub struct CodeSuggestion {
    pub span: Span,
    pub replacement: String,
    pub message: String,
    pub applicability: Applicability,
}

#[derive(Default)]
pub enum Applicability {
    MachineApplicable,  // Safe to auto-apply
    #[default]
    MaybeIncorrect,     // May need review
    HasPlaceholders,    // Don't auto-apply
}

impl CodeSuggestion {
    fn machine_applicable(span, replacement, message) -> Self { ... }
    fn maybe_incorrect(span, replacement, message) -> Self { ... }
    fn with_placeholders(span, replacement, message) -> Self { ... }
}
```

#### ParseErrorKind::details() Method
```rust
impl ParseErrorKind {
    pub fn details(&self, error_span: Span) -> ParseErrorDetails {
        match self {
            Self::UnexpectedToken { .. } => { ... }
            Self::UnclosedDelimiter { .. } => {
                // Includes extra_label for "opened here"
                // Includes suggestion to add closing delimiter
            }
            // ... all 13 error variants
        }
    }
}
```

### Tasks

- [x] Design `ParseErrorDetails` struct
- [x] Design `ExtraLabel` for cross-references (with `same_file`/`cross_file` constructors)
- [x] Design `CodeSuggestion` for auto-fixes (with applicability levels)
- [x] Implement `details()` method on `ParseErrorKind` (all 13 variants)
- [x] Add comprehensive tests (19 new tests for details, suggestions, labels)

---

## 04.2 Empathetic Message Templates

**Status:** ✅ Complete (2026-02-04)
**Goal:** Human-friendly error phrasing

### Implementation Summary

The following was implemented in `compiler/ori_parse/src/error.rs`:

#### Error Titles
```rust
impl ParseErrorKind {
    pub fn title(&self) -> &'static str {
        match self {
            Self::UnexpectedToken { .. } => "UNEXPECTED TOKEN",
            Self::UnclosedDelimiter { .. } => "UNCLOSED DELIMITER",
            Self::ExpectedExpression { .. } => "EXPECTED EXPRESSION",
            Self::ExpectedType { .. } => "EXPECTED TYPE",
            Self::ExpectedPattern { .. } => "EXPECTED PATTERN",
            Self::InvalidPattern { .. } => "INVALID PATTERN",
            Self::InvalidEscape { .. } => "INVALID ESCAPE",
            Self::UnterminatedString => "UNTERMINATED STRING",
            Self::UnexpectedEof { .. } => "UNEXPECTED END OF FILE",
            Self::Custom { .. } => "PARSE ERROR",
        }
    }
}
```

#### Empathetic Messages
```rust
pub fn empathetic_message(&self) -> String {
    match self {
        Self::UnexpectedToken { found, expected, context } => {
            let ctx_phrase = context.map(|c| format!(" while parsing {c}")).unwrap_or_default();
            format!(
                "I ran into something unexpected{ctx_phrase}.\n\n\
                 I was expecting {expected}, but I found `{}`.",
                found.display_name()
            )
        }
        Self::UnclosedDelimiter { open, .. } => {
            let name = delimiter_name(open);
            format!(
                "I found an unclosed {name}.\n\n\
                 Every opening `{}` needs a matching closing `{}`.",
                open.display_name(),
                matching_close(open).display_name()
            )
        }
        // ... more variants
    }
}
```

#### Helper Functions
```rust
fn delimiter_name(open: &TokenKind) -> &'static str {
    match open {
        TokenKind::LParen => "parenthesis",
        TokenKind::LBracket => "bracket",
        TokenKind::LBrace => "brace",
        TokenKind::Lt => "angle bracket",
        _ => "delimiter",
    }
}
```

### Tasks

- [x] Create message template system
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

**Status:** ✅ Complete (2026-02-04)
**Goal:** Recognize patterns of frequent errors and provide targeted help

### Implementation Summary

The following was implemented in `compiler/ori_parse/src/error.rs`:

#### Enhanced hint() Method
```rust
pub fn hint(&self) -> Option<&'static str> {
    match self {
        // Semicolons
        Self::UnexpectedToken { found: TokenKind::Semicolon, .. } =>
            Some("Ori doesn't use semicolons. Remove the `;`..."),

        // Return keyword
        Self::UnexpectedToken { found: TokenKind::Return, .. } |
        Self::UnsupportedKeyword { keyword: TokenKind::Return, .. } =>
            Some("Ori has no `return` keyword..."),

        // Mutability
        Self::UnexpectedToken { found: TokenKind::Mut, .. } |
        Self::UnsupportedKeyword { keyword: TokenKind::Mut, .. } =>
            Some("In Ori, variables are mutable by default..."),

        // Trailing operators (*, /, +, -)
        Self::TrailingOperator { operator, .. } => Some("... needs a value on both sides"),

        // Empty blocks
        Self::ExpectedExpression { found: TokenKind::RBrace, .. } =>
            Some("Blocks must end with an expression. Try adding `void`..."),

        _ => None,
    }
}
```

#### Educational Notes
```rust
pub fn educational_note(&self) -> Option<&'static str> {
    match self {
        Self::ExpectedExpression { position, .. } => match position {
            ExprPosition::Conditional => Some("In Ori, `if` is an expression..."),
            ExprPosition::MatchArm => Some("Match arms must return values..."),
            _ => None,
        },
        Self::InvalidPattern { context, .. } => match context {
            PatternContext::Match => Some("Match patterns include: literals, bindings..."),
            PatternContext::Let => Some("Let bindings support destructuring..."),
            _ => None,
        },
        Self::UnclosedDelimiter { open, .. } => match open {
            TokenKind::LBrace => Some("Braces define blocks and record literals..."),
            _ => None,
        },
        _ => None,
    }
}
```

#### Source-Based Mistake Detection
```rust
pub fn detect_common_mistake(source_text: &str) -> Option<(&'static str, &'static str)> {
    match source_text {
        "===" => Some(("triple equals", "Ori uses `==`...")),
        "!==" => Some(("strict not-equals", "Ori uses `!=`...")),
        "++" => Some(("increment operator", "Use `x = x + 1`...")),
        "--" => Some(("decrement operator", "Use `x = x - 1`...")),
        "<>" => Some(("not-equals", "Ori uses `!=`...")),
        "+=" | "-=" | "*=" | "/=" => Some(("compound assignment", "Use `x = x + y`...")),
        _ => check_common_keyword_mistake(source_text),
    }
}
```

#### Keyword Detection (50+ patterns)
```rust
fn check_common_keyword_mistake(text: &str) -> Option<(&'static str, &'static str)> {
    match text {
        // OOP keywords
        "class" => Some(("class keyword", "Use `type` and `trait`...")),
        "interface" => Some(("interface keyword", "Use `trait`...")),
        // Control flow
        "switch" => Some(("switch keyword", "Use `match`...")),
        "elif" | "elsif" | "elseif" => Some(("elif keyword", "Use `else if`...")),
        // Functions
        "function" | "func" | "fn" => Some(("function keyword", "Use `@name`...")),
        // Variables
        "var" | "const" => Some(("var/const keyword", "Use `let`...")),
        // Null
        "null" | "nil" | "NULL" => Some(("null keyword", "Use `None`...")),
        // Types
        "String" | "string" => Some(("string type", "Use `str`...")),
        // ... 40+ more patterns
        _ => None,
    }
}
```

#### Integration with ParseError
```rust
impl ParseError {
    pub fn from_error_token(span: Span, source_text: &str) -> Self {
        if let Some((description, help)) = detect_common_mistake(source_text) {
            ParseError {
                message: format!("unrecognized {description}: `{source_text}`"),
                help: vec![help.to_string()],
                ..
            }
        } else {
            ParseError { message: "unrecognized token", .. }
        }
    }
}
```

### Tasks

- [x] Identify common mistakes in Ori
  - [x] Using `;` (Ori doesn't use semicolons)
  - [x] Using `return` (Ori has no return keyword)
  - [x] Using `mut` (variables are mutable by default)
  - [x] Using `class` (Ori uses `type` + `trait`)
  - [x] Using `===` instead of `==`
  - [x] Using `<>` instead of `!=`
  - [x] Using `++`/`--` operators
  - [x] Using compound assignments (`+=`, `-=`, etc.)

- [x] Implement detection and suggestions
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

- [x] Add pattern-based suggestions
  - Implemented via `detect_common_mistake()` for source-based patterns
  - Detects `===`, `!==`, `++`, `--`, `<>`, compound assignments

- [x] Add educational context for language differences
  - Implemented via `educational_note()` method on `ParseErrorKind`
  - Provides context for conditionals, match arms, patterns, delimiters

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
