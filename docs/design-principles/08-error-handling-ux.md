# Error Handling & User Experience

Quick-reference guide to compiler error message design and developer experience.

---

## Error Message Anatomy

### Core Components
```
error[E0308]: mismatched types
  --> src/mainrs:4:18
   |
3  |     let x: i32 = "hello";
   |            ---   ^^^^^^^ expected `i32`, found `&str`
   |            |
   |            expected due to this
   |
   = note: expected type `i32`
              found type `&str`
help: consider parsing the string
   |
3  |     let x: i32 = "hello".parse().unwrap();
   |                         +++++++++++++++++
```

### Component Breakdown
| Component | Purpose |
|-----------|---------|
| Level | `error`, `warning`, `note`, `help` |
| Code | E0308 - links to extended explanation |
| Message | Brief description of the problem |
| Location | File, line, column |
| Snippet | Source code context |
| Primary span | Underlines the exact problem (`^^^^^`) |
| Secondary span | Additional context (`---`) |
| Labels | Explain what spans mean |
| Note | Extra context or information |
| Help | Actionable suggestion |

---

## Error Message Principles

### IDE-First Design
- Design for IDE panels first, CLI second
- Short messages work in constrained panels
- Longer explanations available via `--explain`
- Error location = where the red squiggly goes

### Conciseness
- Single, clear sentence when possible
- ~80% of the time, developers know the fix immediately
- Verbose details belong in extended documentation
- Don't repeat information visible in the code

### User-Centric Tone
- Use "we" (first person plural) - "we found an error"
- Elm uses "I" for more personal feel
- Use present tense: "we see" not "we found"
- Never blame the user
- Avoid intimidating language

### Vocabulary
- Use developer language, not compiler-speak
- Say "invalid" not "illegal"
- Wrap identifiers in backticks: `foo`
- Reference "the compiler" not implementation details
- Use proper curly quotes (U+201C/U+201D)

---

## Span-Based Error Reporting

### Span Structure
```rust
struct Span {
    start: u32,  // Byte offset
    end: u32,    // Exclusive
}

struct Diagnostic {
    level: Level,
    message: String,
    code: Option<ErrorCode>,
    primary_span: Span,
    secondary_spans: Vec<(Span, String)>,  // Span + label
    notes: Vec<String>,
    suggestions: Vec<Suggestion>,
}
```

### Span Selection
- Point to the **smallest relevant location**
- Primary span = the actual error site
- Secondary spans = context that explains why
- If refactoring, point to all locations needing change

### Line Offset Calculation
```rust
struct SourceFile {
    content: String,
    line_starts: Vec<u32>,  // Byte offset of each line start
}

impl SourceFile {
    fn line_col(&self, byte_offset: u32) -> (u32, u32) {
        let line = self.line_starts
            .partition_point(|&start| start <= byte_offset) - 1;
        let col = byte_offset - self.line_starts[line];
        (line as u32 + 1, col + 1)  // 1-indexed
    }
}
```

---

## Error Recovery Strategies

### Parser Recovery
1. Report error at current position
2. Skip tokens until synchronization point
3. Continue parsing from sync point
4. Accumulate multiple errors

```rust
fn parse_stmt(&mut self) -> Result<Stmt, ()> {
    match self.parse_stmt_inner() {
        Ok(stmt) => Ok(stmt),
        Err(e) => {
            self.report_error(e);
            self.synchronize();  // Skip to safe point
            Err(())
        }
    }
}

fn synchronize(&mut self) {
    while !self.is_at_end() {
        // Statement boundaries
        if self.previous().kind == Semicolon {
            return;
        }
        // Statement-starting keywords
        match self.peek().kind {
            Fn | Let | If | While | Return => return,
            _ => self.advance(),
        }
    }
}
```

### Type Checker Recovery
- Use `Error` type that unifies with anything
- Prevents cascading type errors
- Continue checking to find more errors

```rust
fn check_expr(&mut self, expr: &Expr) -> Type {
    match self.check_expr_inner(expr) {
        Ok(ty) => ty,
        Err(e) => {
            self.report_error(e);
            Type::Error  // Allows checking to continue
        }
    }
}
```

---

## Suggestion Generation

### Applicability Levels
```rust
enum Applicability {
    MachineApplicable,  // Safe for auto-fix
    HasPlaceholders,    // Contains <type> etc.
    MaybeIncorrect,     // Might not be right fix
    Unspecified,        // Unknown confidence
}

struct Suggestion {
    message: String,
    span: Span,
    replacement: String,
    applicability: Applicability,
}
```

### Suggestion Phrasing
**Good:**
- "there is a struct with a similar name: `Foo`"
- "consider adding a semicolon here"
- "help: add `mut` to make it mutable"

**Bad:**
- "did you mean `Foo`?" (don't ask questions)
- "maybe you forgot a semicolon?" (uncertain)

### Common Suggestions
- Missing semicolon: Insert `;`
- Typo in name: Suggest similar names
- Type mismatch: Suggest conversion
- Missing import: Suggest `use` statement
- Unused variable: Suggest `_` prefix
- Mutable borrow: Suggest `mut`

---

## Multi-Error Handling

### Error Accumulation
```rust
struct Diagnostics {
    errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
    error_count: usize,
}

impl Diagnostics {
    fn error(&mut self, msg: impl Into<String>, span: Span) {
        self.errors.push(Diagnostic {
            level: Level::Error,
            message: msg.into(),
            primary_span: span,
            ..Default::default()
        });
        self.error_count += 1;
    }

    fn has_errors(&self) -> bool {
        self.error_count > 0
    }

    fn emit_all(&self) {
        for diag in &self.errors {
            eprintln!("{}", self.format(diag));
        }
        for diag in &self.warnings {
            eprintln!("{}", self.format(diag));
        }
    }
}
```

### Error Limits
```rust
const MAX_ERRORS: usize = 20;

fn report_error(&mut self, err: Error) {
    if self.error_count >= MAX_ERRORS {
        if self.error_count == MAX_ERRORS {
            eprintln!("... additional errors omitted");
        }
        return;
    }
    self.emit_error(err);
    self.error_count += 1;
}
```

### Deduplication
```rust
// Avoid reporting same error multiple times
fn report_if_new(&mut self, err: Error) {
    let key = (err.span, err.kind);
    if !self.reported.contains(&key) {
        self.reported.insert(key);
        self.report_error(err);
    }
}
```

---

## Error Formatting

### Terminal Output
```rust
fn format_diagnostic(&self, diag: &Diagnostic, source: &SourceFile) -> String {
    let mut out = String::new();

    // Header
    let (line, col) = source.line_col(diag.primary_span.start);
    writeln!(out, "{}[{}]: {}",
        diag.level.color(),
        diag.code.unwrap_or(""),
        diag.message
    );
    writeln!(out, "  --> {}:{}:{}", source.name, line, col);

    // Code snippet with annotations
    writeln!(out, "   |");
    for (line_num, line_content) in self.snippet_lines(diag, source) {
        writeln!(out, "{:3} | {}", line_num, line_content);
        if let Some(underline) = self.underline_for_line(line_num, diag) {
            writeln!(out, "    | {}", underline);
        }
    }
    writeln!(out, "   |");

    // Notes and suggestions
    for note in &diag.notes {
        writeln!(out, "   = note: {}", note);
    }
    for suggestion in &diag.suggestions {
        writeln!(out, "help: {}", suggestion.message);
        // Show suggested code change
    }

    out
}
```

### Color Coding
| Level | Color | ANSI Code |
|-------|-------|-----------|
| error | Red | `\x1b[1;31m` |
| warning | Yellow | `\x1b[1;33m` |
| note | Blue | `\x1b[1;34m` |
| help | Cyan | `\x1b[1;36m` |
| code | White/Bold | `\x1b[1;37m` |

### Non-Color Fallback
- Test messages without color
- Use ASCII markers: `^^^^^`, `-----`
- Prefix lines clearly: `error:`, `note:`

---

## "Did You Mean?" Suggestions

### Edit Distance (Levenshtein)
```rust
fn levenshtein(a: &str, b: &str) -> usize {
    let mut dp = vec![vec![0; b.len() + 1]; a.len() + 1];

    for i in 0..=a.len() { dp[i][0] = i; }
    for j in 0..=b.len() { dp[0][j] = j; }

    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            dp[i + 1][j + 1] = min(
                dp[i][j + 1] + 1,      // deletion
                dp[i + 1][j] + 1,      // insertion
                dp[i][j] + cost,       // substitution
            );
        }
    }

    dp[a.len()][b.len()]
}
```

### Finding Similar Names
```rust
fn suggest_similar(unknown: &str, candidates: &[String]) -> Option<String> {
    candidates
        .iter()
        .map(|c| (c, levenshtein(unknown, c)))
        .filter(|(_, dist)| *dist <= 3)  // Threshold
        .min_by_key(|(_, dist)| *dist)
        .map(|(name, _)| name.clone())
}
```

---

## Error Codes & Documentation

### Error Code System
```rust
// E0001, E0002, etc.
enum ErrorCode {
    E0001,  // Parse error
    E0308,  // Type mismatch
    E0425,  // Undefined name
    // ...
}

impl ErrorCode {
    fn explanation(&self) -> &'static str {
        match self {
            E0308 => include_str!("errors/E0308.md"),
            // ...
        }
    }
}
```

### Extended Explanations
```markdown
# E0308: Mismatched types

This error occurs when the compiler expected one type but found another.

## Example

```rust
let x: i32 = "hello";  // Error!
```

The variable `x` is declared as `i32`, but a string literal was provided.

## Common causes

1. Wrong type annotation
2. Function returns wrong type
3. Missing type conversion

## How to fix

- Change the type annotation to match the value
- Convert the value to the expected type
- Use a different function that returns the expected type
```

---

## IDE Integration (LSP)

### Diagnostic Structure
```json
{
  "uri": "file:///path/to/file.rs",
  "diagnostics": [{
    "range": {
      "start": {"line": 3, "character": 12},
      "end": {"line": 3, "character": 19}
    },
    "severity": 1,
    "code": "E0308",
    "source": "rustc",
    "message": "mismatched types",
    "relatedInformation": [{
      "location": {...},
      "message": "expected due to this"
    }]
  }]
}
```

### Code Actions (Quick Fixes)
```json
{
  "title": "Add missing semicolon",
  "kind": "quickfix",
  "diagnostics": [...],
  "edit": {
    "changes": {
      "file:///path/to/file.rs": [{
        "range": {...},
        "newText": ";"
      }]
    }
  }
}
```

---

## Testing Error Messages

### Snapshot Testing
```rust
#[test]
fn test_type_mismatch_error() {
    let source = r#"
        let x: int = "hello"
    "#;

    let errors = compile_and_collect_errors(source);

    insta::assert_snapshot!(format_errors(&errors));
}
```

### Expected Patterns
```rust
// In test file:
let x: int = "hello";
//           ^^^^^^^ error: expected `int`, found `str`

// Test assertion:
assert!(errors[0].message.contains("expected `int`"));
assert!(errors[0].span == span_of("\"hello\""));
```

---

## Error UX Checklist

### Message Quality
- [ ] Clear, single-sentence primary message
- [ ] Points to smallest relevant location
- [ ] Uses developer terminology
- [ ] Present tense, "we" voice
- [ ] No intimidating language

### Context & Help
- [ ] Shows relevant code snippet
- [ ] Labels explain primary and secondary spans
- [ ] Notes provide additional context
- [ ] Suggestions are actionable
- [ ] Similar names suggested when appropriate

### Technical
- [ ] Errors accumulate (don't stop at first)
- [ ] Recovery prevents cascading errors
- [ ] Error codes link to docs
- [ ] Works without color
- [ ] LSP-compatible format

---

## Key References
- Rust Diagnostics Guide: https://rustc-dev-guide.rust-lang.org/diagnostics.html
- Rust RFC 1644 (Error Format): https://rust-lang.github.io/rfcs/1644-default-and-expanded-rustc-errors.html
- Writing Good Compiler Error Messages: https://calebmer.com/2019/07/01/writing-good-compiler-error-messages.html
- Elm Error Messages: https://elm-lang.org/news/compiler-errors-for-humans
