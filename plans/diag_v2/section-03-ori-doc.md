---
section: "03"
title: Composable Document System (ori_doc)
status: not-started
goal: A composable document tree for building diagnostic messages, decoupled from rendering
sections:
  - id: "03.1"
    title: Document Tree Types
    status: not-started
  - id: "03.2"
    title: Semantic Annotations
    status: not-started
  - id: "03.3"
    title: Builder API
    status: not-started
  - id: "03.4"
    title: Palette Rendering
    status: not-started
  - id: "03.5"
    title: Integration with Diagnostic
    status: not-started
  - id: "03.6"
    title: Completion Checklist
    status: not-started
---

# Section 03: Composable Document System (ori_doc)

**Status:** Not Started
**Goal:** Create a composable document tree system for building diagnostic messages. The document tree separates *what* to say (semantic content) from *how* to say it (rendering), enabling rich type-aware error messages that render correctly to terminal, JSON, HTML, and plain text.

**Reference compilers:**
- **Roc** `crates/reporting/src/report.rs` — `RocDocAllocator` wrapping `ven_pretty::BoxAllocator`; semantic `Annotation` enum (`Module`, `TypeVariable`, `Keyword`, `Emphasized`, etc.); `Palette` pattern for multi-format rendering
- **Elm** `compiler/src/Reporting/Doc.hs` — Wadler-Lindig pretty-printer combinators; `Doc` type with `Text`, `Color`, `Append`, `Indent`, `Line`; used throughout all error reporting
- **Gleam** `compiler-core/src/pretty.rs` — `codespan_reporting::Diagnostic` rendering with `Printer { uid, names, printed_types }` for deterministic type variable naming

**Current state:** Error messages are built as `String` via `format!()`. This makes it impossible to:
1. Highlight specific parts of an error message (e.g., the type name in red)
2. Produce different formatting for different output targets
3. Build complex messages composably (each error arm does its own `format!()`)
4. Include structured markup for IDE consumption

---

## 03.1 Document Tree Types

### Core Doc Enum

The document tree is a simple recursive enum. Unlike Roc's approach (which uses an external `ven_pretty` crate with arena allocation), we use a self-contained `Box`-based tree. This is acceptable because:
1. Document trees are built only on the error path (cold code)
2. Trees are small (typically <50 nodes for even complex errors)
3. No need for the full Wadler-Lindig line-breaking algorithm (terminal width is handled by the emitter, not the document)

```rust
// In ori_diagnostic/src/doc.rs

/// A composable document tree for building diagnostic messages.
///
/// Document trees separate semantic content from rendering. Each node
/// describes *what* to display; the renderer (Palette) decides *how*.
///
/// # Example
/// ```
/// use ori_diagnostic::doc::Doc;
///
/// let doc = Doc::text("expected ")
///     .append(Doc::type_name("int"))
///     .append(Doc::text(", found "))
///     .append(Doc::type_name("float"));
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Doc {
    /// Plain text (no styling).
    Text(String),

    /// Annotated content — a semantic tag wrapping a sub-document.
    Annotated(Annotation, Box<Doc>),

    /// Concatenation of two documents.
    Concat(Box<Doc>, Box<Doc>),

    /// A sequence of documents (more efficient than nested Concat).
    Sequence(Vec<Doc>),

    /// A line break.
    Line,

    /// Indented content (increases indent by N spaces).
    Indent(u16, Box<Doc>),

    /// Empty document (identity for Concat).
    Empty,
}
```

### Memory Considerations

Each `Doc` node is small:
- `Text`: 24 bytes (String)
- `Annotated`: 8 + 1 (Annotation u8 + padding) + 8 (Box) = ~17 bytes
- `Concat`: 16 bytes (two Box)
- `Sequence`: 24 bytes (Vec)
- `Line`: 0 bytes (unit variant)
- `Indent`: 2 + 8 = 10 bytes
- `Empty`: 0 bytes

With discriminant, each node is ~32 bytes max. A complex error message with 30 nodes uses ~1 KB — negligible on the error path.

- [ ] Define `Doc` enum with all variants
- [ ] Size assertions: `assert!(size_of::<Doc>() <= 32)`
- [ ] Unit tests: construction, equality

---

## 03.2 Semantic Annotations

### Annotation Enum

Annotations describe *what* a piece of text represents semantically. The renderer maps these to colors, styles, or markup based on the output target.

```rust
/// Semantic annotation for a piece of document content.
///
/// Annotations describe the *meaning* of text, not its appearance.
/// The Palette maps annotations to concrete styles per output target.
///
/// Follows Roc's `Annotation` enum pattern, adapted for Ori's needs.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum Annotation {
    /// A type name (e.g., `int`, `Option(str)`, `fn(int) -> bool`).
    TypeName = 0,

    /// A type variable (e.g., `a`, `T`).
    TypeVariable = 1,

    /// A keyword in an error message (e.g., `if`, `match`, `fn`).
    Keyword = 2,

    /// An identifier (variable or function name).
    Ident = 3,

    /// Emphasized text (the "important part" of a message).
    Emphasis = 4,

    /// An operator symbol (e.g., `+`, `==`, `|>`).
    Operator = 5,

    /// A literal value (e.g., `42`, `"hello"`, `true`).
    Literal = 6,

    /// A module or file path.
    Module = 7,

    /// Suggested replacement text (in suggestions/fixes).
    Suggestion = 8,

    /// The "expected" type in a mismatch (renders differently from "found").
    Expected = 9,

    /// The "found" type in a mismatch.
    Found = 10,

    /// A diff addition (type diff — what was added/different).
    DiffAdd = 11,

    /// A diff removal (type diff — what was expected but missing).
    DiffRemove = 12,

    /// A URL or link.
    Url = 13,

    /// An error code (e.g., `E2001`).
    ErrorCode = 14,
}
```

### Default Palette Mapping

| Annotation | Terminal (ANSI) | JSON | Plain Text |
|-----------|-----------------|------|------------|
| `TypeName` | Bold cyan | `"type"` tag | backtick-wrapped |
| `TypeVariable` | Italic cyan | `"typevar"` tag | backtick-wrapped |
| `Keyword` | Bold | `"keyword"` tag | backtick-wrapped |
| `Ident` | Bold white | `"ident"` tag | backtick-wrapped |
| `Emphasis` | Bold | `"emphasis"` tag | unchanged |
| `Operator` | Yellow | `"operator"` tag | backtick-wrapped |
| `Literal` | Green | `"literal"` tag | unchanged |
| `Module` | Underline | `"module"` tag | unchanged |
| `Suggestion` | Bold green | `"suggestion"` tag | unchanged |
| `Expected` | Bold cyan | `"expected"` tag | backtick-wrapped |
| `Found` | Bold red | `"found"` tag | backtick-wrapped |
| `DiffAdd` | Green background | `"diff_add"` tag | `+` prefix |
| `DiffRemove` | Red background | `"diff_remove"` tag | `-` prefix |
| `Url` | Underline blue | `"url"` tag | unchanged |
| `ErrorCode` | Bold yellow | `"error_code"` tag | unchanged |

- [ ] Define `Annotation` enum with all variants
- [ ] Size assertion: `assert!(size_of::<Annotation>() == 1)`
- [ ] Unit tests: all variants round-trip through Debug

---

## 03.3 Builder API

### Fluent Construction

```rust
impl Doc {
    // === Leaf constructors ===

    /// Create a plain text document.
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    /// Create an annotated type name: `int`, `Option(str)`.
    pub fn type_name(s: impl Into<String>) -> Self {
        Self::Annotated(Annotation::TypeName, Box::new(Self::text(s)))
    }

    /// Create an annotated type variable: `a`, `T`.
    pub fn type_var(s: impl Into<String>) -> Self {
        Self::Annotated(Annotation::TypeVariable, Box::new(Self::text(s)))
    }

    /// Create an annotated keyword: `if`, `match`.
    pub fn keyword(s: impl Into<String>) -> Self {
        Self::Annotated(Annotation::Keyword, Box::new(Self::text(s)))
    }

    /// Create an annotated identifier: `x`, `my_function`.
    pub fn ident(s: impl Into<String>) -> Self {
        Self::Annotated(Annotation::Ident, Box::new(Self::text(s)))
    }

    /// Create emphasized text.
    pub fn emphasis(s: impl Into<String>) -> Self {
        Self::Annotated(Annotation::Emphasis, Box::new(Self::text(s)))
    }

    /// Create an annotated literal value.
    pub fn literal(s: impl Into<String>) -> Self {
        Self::Annotated(Annotation::Literal, Box::new(Self::text(s)))
    }

    /// Create an "expected" type annotation (for type mismatches).
    pub fn expected(inner: Doc) -> Self {
        Self::Annotated(Annotation::Expected, Box::new(inner))
    }

    /// Create a "found" type annotation (for type mismatches).
    pub fn found(inner: Doc) -> Self {
        Self::Annotated(Annotation::Found, Box::new(inner))
    }

    // === Composition ===

    /// Append another document after this one.
    #[must_use]
    pub fn append(self, other: Doc) -> Self {
        match (self, other) {
            (Doc::Empty, other) => other,
            (this, Doc::Empty) => this,
            (this, other) => Doc::Concat(Box::new(this), Box::new(other)),
        }
    }

    /// Create a sequence from multiple documents.
    pub fn seq(docs: Vec<Doc>) -> Self {
        Doc::Sequence(docs)
    }

    /// Join documents with a separator.
    pub fn join(docs: Vec<Doc>, sep: Doc) -> Self {
        let mut result = Vec::with_capacity(docs.len() * 2);
        for (i, doc) in docs.into_iter().enumerate() {
            if i > 0 {
                result.push(sep.clone());
            }
            result.push(doc);
        }
        Doc::Sequence(result)
    }

    /// Indent the document by `n` spaces.
    #[must_use]
    pub fn indent(self, n: u16) -> Self {
        Doc::Indent(n, Box::new(self))
    }

    /// A line break.
    pub fn line() -> Self {
        Doc::Line
    }

    // === Rendering ===

    /// Render to plain text (no styling, annotations stripped).
    pub fn to_plain_text(&self) -> String { /* ... */ }

    /// Render to a string using a Palette.
    pub fn render(&self, palette: &Palette) -> String { /* ... */ }
}
```

### Display Impl

`Display` renders as plain text (annotations stripped, for backward compatibility in `format!()` contexts):

```rust
impl fmt::Display for Doc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_plain_text())
    }
}
```

This means a `Doc` can be used as a drop-in replacement for `String` in `format!()` contexts, preserving backward compatibility while enabling rich rendering when a Palette is available.

- [ ] All leaf constructors implemented
- [ ] Composition methods: `append()`, `seq()`, `join()`, `indent()`, `line()`
- [ ] `to_plain_text()` rendering
- [ ] `Display` impl (plain text)
- [ ] Unit tests: construction, plain text rendering, Display
- [ ] Unit tests: `append` with `Empty` optimization

---

## 03.4 Palette Rendering

### Palette Trait

```rust
/// Rendering strategy for document trees.
///
/// A Palette maps semantic annotations to concrete formatting. Different
/// palettes produce different output: ANSI terminal, HTML, JSON, plain text.
///
/// Follows Roc's Palette pattern where the same document tree renders
/// differently based on the target.
pub trait Palette {
    /// Start an annotation (e.g., emit ANSI escape code).
    fn begin_annotation(&self, annotation: Annotation, out: &mut String);

    /// End an annotation (e.g., emit ANSI reset code).
    fn end_annotation(&self, annotation: Annotation, out: &mut String);
}
```

### Built-in Palettes

```rust
/// ANSI terminal palette with color support.
pub struct AnsiPalette {
    pub color_enabled: bool,
}

/// Plain text palette (annotations become backtick-wrapping or nothing).
pub struct PlainPalette;

/// HTML palette (annotations become <span class="...">).
pub struct HtmlPalette;
```

### Rendering Implementation

```rust
impl Doc {
    /// Render the document using a palette.
    pub fn render(&self, palette: &dyn Palette) -> String {
        let mut out = String::new();
        self.render_into(palette, &mut out, 0);
        out
    }

    fn render_into(&self, palette: &dyn Palette, out: &mut String, indent: u16) {
        match self {
            Doc::Text(s) => out.push_str(s),
            Doc::Annotated(ann, inner) => {
                palette.begin_annotation(*ann, out);
                inner.render_into(palette, out, indent);
                palette.end_annotation(*ann, out);
            }
            Doc::Concat(a, b) => {
                a.render_into(palette, out, indent);
                b.render_into(palette, out, indent);
            }
            Doc::Sequence(docs) => {
                for doc in docs {
                    doc.render_into(palette, out, indent);
                }
            }
            Doc::Line => {
                out.push('\n');
                for _ in 0..indent {
                    out.push(' ');
                }
            }
            Doc::Indent(n, inner) => {
                inner.render_into(palette, out, indent + n);
            }
            Doc::Empty => {}
        }
    }
}
```

- [ ] Define `Palette` trait
- [ ] Implement `AnsiPalette` (ANSI color codes for each annotation)
- [ ] Implement `PlainPalette` (backtick wrapping, no colors)
- [ ] Implement `HtmlPalette` (span elements with CSS classes)
- [ ] `render()` and `render_into()` methods on `Doc`
- [ ] Tests: ANSI output for each annotation type
- [ ] Tests: Plain text output preserves content
- [ ] Tests: HTML output generates correct markup

---

## 03.5 Integration with Diagnostic

### Gradual Adoption

The `Doc` system integrates gradually. Existing `String`-based messages continue to work. New diagnostic producers can return `Doc` trees for richer output.

**Phase 1: Message-level adoption**

Add an optional `rich_message` field to `Diagnostic`:

```rust
pub struct Diagnostic {
    // ... existing fields ...
    pub message: String, // Backward-compatible plain text

    // NEW: optional rich message (overrides `message` when rendered with a Palette)
    pub rich_message: Option<Doc>,
}

impl Diagnostic {
    /// Set a rich message that overrides the plain text message when
    /// rendered with a Palette-aware emitter.
    #[must_use]
    pub fn with_rich_message(mut self, doc: Doc) -> Self {
        self.rich_message = Some(doc);
        self
    }
}
```

**Phase 2: Per-error-code adoption**

Convert individual error renderers to produce `Doc` trees:

```rust
// Before (flat string):
Diagnostic::error(ErrorCode::E2001)
    .with_message(format!("expected `{}`, found `{}`", expected, found))

// After (rich Doc):
Diagnostic::error(ErrorCode::E2001)
    .with_message(format!("expected `{}`, found `{}`", expected, found))
    .with_rich_message(
        Doc::text("expected ")
            .append(Doc::expected(Doc::type_name(&expected)))
            .append(Doc::text(", found "))
            .append(Doc::found(Doc::type_name(&found)))
    )
```

The plain `message` field is always populated (for backward compatibility, JSON output, and non-Palette consumers). The `rich_message` is an enhancement layer.

**Phase 3: Emitter integration**

Update `TerminalEmitter` to use `rich_message` when available:

```rust
// In TerminalEmitter::emit():
if let Some(ref doc) = diagnostic.rich_message {
    let palette = AnsiPalette { color_enabled: self.color_enabled };
    write!(self.writer, "{}", doc.render(&palette))?;
} else {
    write!(self.writer, "{}", diagnostic.message)?;
}
```

- [ ] Add `rich_message: Option<Doc>` field to `Diagnostic`
- [ ] Add `.with_rich_message()` builder method
- [ ] Update `TerminalEmitter` to prefer `rich_message` when available
- [ ] Update `JsonEmitter` to include structured annotation data when `rich_message` present
- [ ] Convert 3-5 high-impact error renderers to use `Doc` (type mismatch, unknown ident, undefined field)
- [ ] Tests: backward compatibility (no rich_message = unchanged behavior)
- [ ] Tests: terminal rendering with rich_message

---

## 03.6 Completion Checklist

- [ ] `ori_diagnostic/src/doc.rs` module created
- [ ] `Doc` enum with all variants
- [ ] `Annotation` enum with 15 semantic annotations
- [ ] Builder API: leaf constructors + composition methods
- [ ] `Palette` trait defined
- [ ] `AnsiPalette`, `PlainPalette`, `HtmlPalette` implementations
- [ ] `to_plain_text()` and `render()` methods
- [ ] `Display` impl for backward compatibility
- [ ] `Diagnostic.rich_message` field added
- [ ] `TerminalEmitter` integration
- [ ] 3-5 error renderers converted to `Doc`
- [ ] Tests: 20+ unit tests for Doc construction, rendering, annotations
- [ ] `./test-all.sh` passes

**Exit Criteria:** A working composable document system that can produce annotated error messages. At least 3 error types render with semantic annotations (type names highlighted, expected/found distinguished). All existing tests pass unchanged.
