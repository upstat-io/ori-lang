---
section: "02"
title: Enhanced Diagnostic Types
status: not-started
goal: Add ExplanationChain and RelatedInformation to the Diagnostic struct for richer error context
sections:
  - id: "02.1"
    title: ExplanationChain
    status: not-started
  - id: "02.2"
    title: RelatedInformation
    status: not-started
  - id: "02.3"
    title: Diagnostic Struct Updates
    status: not-started
  - id: "02.4"
    title: Rendering Updates
    status: not-started
  - id: "02.5"
    title: Completion Checklist
    status: not-started
---

# Section 02: Enhanced Diagnostic Types

**Status:** Not Started
**Goal:** Extend the core `Diagnostic` struct with two new concepts: `ExplanationChain` (nested "because..." reasoning) and `RelatedInformation` (cross-file context with source snippets). These are small, backward-compatible additions that enable significantly richer error messages.

**Reference compilers:**
- **TypeScript** `src/compiler/types.ts` — `DiagnosticMessageChain { messageText, category, code, next?: DiagnosticMessageChain[] }` for nested "because..." chains
- **TypeScript** `src/compiler/types.ts` — `DiagnosticRelatedInformation { location, messageText }` for cross-file context
- **Rust** `compiler/rustc_errors/src/diagnostic.rs` — `children: Vec<SubDiagnostic>` with sub-notes and sub-helps

**Current state:** `Diagnostic` has flat `notes: Vec<String>` and `labels: Vec<Label>`. Labels can reference other files via `SourceInfo`, but there is no structured chain of reasoning or typed related-information concept.

---

## 02.1 ExplanationChain

### Motivation

Many type errors have causal chains that are currently flattened into opaque messages:

```
// Current (flat):
error[E2001]: type mismatch
  --> src/main.ori:10:5
   |
10 |     x + 1.5
   |     ^ expected `int`, found `float`
   = note: `x` was inferred as `int` from the annotation on line 3
```

```
// With chain (structured):
error[E2001]: type mismatch
  --> src/main.ori:10:5
   |
10 |     x + 1.5
   |     ^ expected `int`, found `float`
   |
   = because: `x` has type `int`
     = because: `x` is bound to the parameter on line 3
       = because: the function signature declares `x: int`
```

### Design

```rust
/// A chain of explanations for why an error occurred.
///
/// Each link in the chain explains one step of reasoning, forming a
/// "because..." tree. Chains are rendered with increasing indentation.
///
/// Modeled after TypeScript's `DiagnosticMessageChain`, but with spans
/// for each link (TypeScript only has spans on the root).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ExplanationChain {
    /// The explanation message for this link.
    pub message: String,

    /// Optional source location for this explanation.
    pub span: Option<Span>,

    /// Optional source info for cross-file explanations.
    pub source_info: Option<SourceInfo>,

    /// Child explanations (the "because..." sub-chain).
    /// Usually 0 or 1 children. Multiple children represent
    /// alternative reasoning paths (e.g., "either because A or because B").
    pub children: Vec<ExplanationChain>,
}

impl ExplanationChain {
    /// Create a new explanation with a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            source_info: None,
            children: Vec::new(),
        }
    }

    /// Attach a source location to this explanation.
    #[must_use]
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Attach cross-file source info.
    #[must_use]
    pub fn with_source(mut self, info: SourceInfo) -> Self {
        self.source_info = Some(info);
        self
    }

    /// Add a child "because..." explanation.
    #[must_use]
    pub fn because(mut self, child: ExplanationChain) -> Self {
        self.children.push(child);
        self
    }

    /// Convenience: add a simple text "because..." without a span.
    #[must_use]
    pub fn because_of(self, message: impl Into<String>) -> Self {
        self.because(ExplanationChain::new(message))
    }

    /// Total depth of the chain (for rendering decisions).
    pub fn depth(&self) -> usize {
        1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
    }
}
```

### Rendering

Chains render with indented "because:" prefixes:

```
   = because: `x` has type `int`
     = because: `x` is bound to the parameter on line 3
       = because: the function signature declares `x: int`
```

If a link has a span, it renders with a source snippet:

```
   = because: the function signature declares `x: int`
    --> src/lib.ori:3:15
     |
   3 | fn add(x: int, y: int) -> int =
     |           ^^^ declared here
```

Depth is capped at 4 levels for readability. Deeper chains are truncated with "... (N more reasons)".

- [ ] Define `ExplanationChain` struct in `ori_diagnostic`
- [ ] Builder methods: `new()`, `with_span()`, `with_source()`, `because()`, `because_of()`
- [ ] `depth()` utility method
- [ ] Unit tests: construction, depth, rendering format

---

## 02.2 RelatedInformation

### Motivation

When an error involves multiple files or distant locations, flat labels become confusing. `RelatedInformation` provides typed, structured cross-location context:

```
error[E2003]: duplicate definition of `Config`
  --> src/main.ori:5:1
   |
 5 | type Config = { ... }
   | ^^^^^^ defined here
   |
  related: previously defined here
    --> src/lib.ori:12:1
     |
  12 | type Config = { ... }
     | ^^^^^^ first definition
```

### Design

```rust
/// A piece of related information attached to a diagnostic.
///
/// Unlike labels (which annotate the primary source), related information
/// references other locations that provide context for understanding the error.
///
/// Modeled after TypeScript's `DiagnosticRelatedInformation` and LSP's
/// `DiagnosticRelatedInformation`.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RelatedInformation {
    /// Description of how this location relates to the error.
    pub message: String,

    /// The source location of the related information.
    pub span: Span,

    /// Source info if the related location is in a different file.
    pub source_info: Option<SourceInfo>,
}

impl RelatedInformation {
    /// Create related information with a message and span.
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            source_info: None,
        }
    }

    /// Create cross-file related information.
    pub fn cross_file(
        message: impl Into<String>,
        span: Span,
        source_info: SourceInfo,
    ) -> Self {
        Self {
            message: message.into(),
            span,
            source_info: Some(source_info),
        }
    }
}
```

### Relationship to Existing Labels

`Label` and `RelatedInformation` serve different purposes:

| Feature | `Label` | `RelatedInformation` |
|---------|---------|---------------------|
| Purpose | Annotate the error's source snippet | Reference related locations |
| Rendering | Inline underlines `^^^` | Separate "related:" blocks |
| Multiplicity | Multiple per snippet | Multiple per diagnostic |
| Cross-file | Via `SourceInfo` | Via `SourceInfo` |
| LSP mapping | `Diagnostic.range` | `DiagnosticRelatedInformation` |

Labels are rendered *within* the primary source snippet. Related information is rendered *after* the snippet as separate blocks.

- [ ] Define `RelatedInformation` struct in `ori_diagnostic`
- [ ] Constructors: `new()`, `cross_file()`
- [ ] Unit tests: construction, cross-file

---

## 02.3 Diagnostic Struct Updates

### New Fields

Add two new fields to `Diagnostic`, both defaulting to empty:

```rust
pub struct Diagnostic {
    // ... existing fields unchanged ...
    pub code: ErrorCode,
    pub severity: Severity,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub suggestions: Vec<String>,
    pub structured_suggestions: Vec<Suggestion>,

    // NEW in V2:
    /// Chain of explanations for why this error occurred.
    /// Empty means no chain (backward compatible with V1).
    pub explanation: Vec<ExplanationChain>,

    /// Related information from other locations.
    /// Empty means no related info (backward compatible).
    pub related: Vec<RelatedInformation>,
}
```

### Builder Methods

```rust
impl Diagnostic {
    // ... existing methods unchanged ...

    /// Add an explanation chain ("because..." reasoning).
    #[must_use]
    pub fn with_explanation(mut self, chain: ExplanationChain) -> Self {
        self.explanation.push(chain);
        self
    }

    /// Add related information from another location.
    #[must_use]
    pub fn with_related(mut self, info: RelatedInformation) -> Self {
        self.related.push(info);
        self
    }

    /// Convenience: add related information from the same file.
    #[must_use]
    pub fn with_related_span(
        mut self,
        message: impl Into<String>,
        span: Span,
    ) -> Self {
        self.related.push(RelatedInformation::new(message, span));
        self
    }
}
```

### Backward Compatibility

- All constructors initialize `explanation` and `related` to empty `Vec`.
- Existing `into_diagnostic()` implementations are unchanged.
- Display impl renders chains and related info only when non-empty.
- JSON emitter emits `"explanation"` and `"related"` fields only when non-empty.
- SARIF emitter maps `related` to `relatedLocations` (already partially supported via cross-file labels).

- [ ] Add `explanation: Vec<ExplanationChain>` field to `Diagnostic`
- [ ] Add `related: Vec<RelatedInformation>` field to `Diagnostic`
- [ ] Add builder methods: `with_explanation()`, `with_related()`, `with_related_span()`
- [ ] Update all constructors to initialize new fields to empty
- [ ] Verify backward compatibility: `./test-all.sh` passes with no changes to existing code

---

## 02.4 Rendering Updates

### Terminal Emitter

Render explanation chains after notes, before suggestions:

```
error[E2001]: type mismatch
  --> src/main.ori:10:5
   |
10 |     x + 1.5
   |     ^ expected `int`, found `float`
   |
   = note: binary `+` requires operands of the same type
   = because: `x` has type `int`
     = because: declared as parameter `x: int`
       --> src/main.ori:3:15
        |
      3 | fn add(x: int, y: int) -> int =
        |           ^^^ declared here
   = help: try `x as float + 1.5` or `x + 1 as int`
```

Render related information as separate blocks:

```
  related: first defined here
    --> src/lib.ori:12:1
     |
  12 | type Config = { ... }
     | ^^^^^^
```

### JSON Emitter

Add new fields to JSON output:

```json
{
  "explanation": [
    {
      "message": "`x` has type `int`",
      "span": { "start": 30, "end": 33 },
      "children": [
        {
          "message": "declared as parameter `x: int`",
          "span": { "start": 15, "end": 21 }
        }
      ]
    }
  ],
  "related": [
    {
      "message": "first defined here",
      "span": { "start": 120, "end": 135 },
      "file": "src/lib.ori"
    }
  ]
}
```

### SARIF Emitter

Map `related` to SARIF `relatedLocations` (already partially supported). Map `explanation` to SARIF `message.markdown` with nested bullet points.

- [ ] Update terminal emitter to render `ExplanationChain` with indentation
- [ ] Update terminal emitter to render `RelatedInformation` as separate blocks
- [ ] Update JSON emitter to include `explanation` and `related` fields
- [ ] Update SARIF emitter to map to `relatedLocations` and `message.markdown`
- [ ] Cap chain depth at 4 levels in terminal rendering
- [ ] Tests: terminal output format for chains
- [ ] Tests: terminal output format for related info
- [ ] Tests: JSON output includes new fields

---

## 02.5 Completion Checklist

- [ ] `ExplanationChain` struct defined with builder API
- [ ] `RelatedInformation` struct defined
- [ ] `Diagnostic` struct extended with new fields
- [ ] All constructors updated (backward compatible)
- [ ] Terminal emitter renders chains and related info
- [ ] JSON emitter includes new fields when non-empty
- [ ] SARIF emitter maps new fields appropriately
- [ ] Unit tests for all new types
- [ ] Integration tests for rendering
- [ ] `./test-all.sh` passes

**Exit Criteria:** The `Diagnostic` struct supports structured explanation chains and cross-location related information. All three emitters render the new fields correctly. No existing tests are broken.
