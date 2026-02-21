---
title: "Code Fixes"
description: "Ori Compiler Design — Code Fixes"
order: 801
section: "Diagnostics"
---

# Code Fixes

Code fixes are automatic repair suggestions that can be applied to resolve errors. The framework (`CodeFix` trait, `FixRegistry`) is fully designed and implemented, but **no production fixes are currently registered**. The examples below illustrate how fixes are structured for future implementation.

## Location

```
compiler/ori_diagnostic/src/diagnostic.rs  # Suggestion, Substitution, Applicability types
compiler/ori_diagnostic/src/fixes/         # Fix registry and helpers
```

## Suggestion Structure

Code fixes are represented as `Suggestion` with `Substitution` components:

```rust
/// A structured suggestion with substitutions and applicability.
///
/// Supports two forms:
/// - **Text-only**: A human-readable message with no code substitutions.
///   Created via `text()`, `did_you_mean()`, `wrap_in()`.
/// - **Span-bearing**: A message with exact code substitutions for `ori fix`.
///   Created via `new()`, `machine_applicable()`, `maybe_incorrect()`, etc.
pub struct Suggestion {
    /// Human-readable message describing the fix.
    pub message: String,

    /// The text substitutions to make (empty for text-only suggestions).
    pub substitutions: Vec<Substitution>,

    /// How confident we are in this suggestion.
    pub applicability: Applicability,

    /// Priority (lower = more likely to be relevant).
    /// 0 = most likely, 1 = likely, 2 = possible, 3 = unlikely.
    pub priority: u8,
}

/// A text substitution for a code fix.
pub struct Substitution {
    /// The span to replace.
    pub span: Span,

    /// Replacement text.
    pub snippet: String,
}

pub enum Applicability {
    /// Safe to apply automatically (e.g., typo fix)
    MachineApplicable,

    /// Might change semantics (e.g., type conversion)
    MaybeIncorrect,

    /// Contains placeholders user must fill in
    HasPlaceholders,

    /// Just a suggestion, likely needs thought
    #[default]
    Unspecified,
}
```

### Factory Methods

```rust
impl Suggestion {
    /// Create a machine-applicable suggestion (safe to auto-apply).
    pub fn machine_applicable(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self;

    /// Create a suggestion that might be incorrect.
    pub fn maybe_incorrect(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self;

    /// Create a suggestion with placeholders.
    pub fn has_placeholders(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self;

    /// Create a text-only suggestion (no code substitution).
    pub fn text(message: impl Into<String>, priority: u8) -> Self;

    /// Create a text-only suggestion with a single code replacement.
    pub fn text_with_replacement(
        message: impl Into<String>,
        priority: u8,
        span: Span,
        new_text: impl Into<String>,
    ) -> Self;

    /// Create a "did you mean" suggestion (priority 0).
    pub fn did_you_mean(suggestion: impl Into<String>) -> Self;

    /// Create a suggestion to wrap in something (priority 1).
    pub fn wrap_in(wrapper: &str, example: &str) -> Self;

    /// Add another substitution to this suggestion.
    pub fn with_substitution(self, span: Span, snippet: impl Into<String>) -> Self;

    /// Check if this is a text-only suggestion (no code substitutions).
    pub fn is_text_only(&self) -> bool;
}
```

## Creating Fixes

### Using Diagnostic Builder

The easiest way to add fixes is through the `Diagnostic` builder:

```rust
impl Diagnostic {
    /// Add a plain text suggestion (human-readable, no code substitution).
    pub fn with_suggestion(self, suggestion: impl Into<String>) -> Self;

    /// Add a structured suggestion with applicability information (for `ori fix`).
    pub fn with_structured_suggestion(self, suggestion: Suggestion) -> Self;

    /// Add a machine-applicable fix (safe to auto-apply).
    pub fn with_fix(self, message: impl Into<String>, span: Span, snippet: impl Into<String>) -> Self;

    /// Add a suggestion that might be incorrect.
    pub fn with_maybe_fix(self, message: impl Into<String>, span: Span, snippet: impl Into<String>) -> Self;
}
```

Examples:

```rust
// Machine-applicable fix (safe to auto-apply)
Diagnostic::error(ErrorCode::E1001)
    .with_message("missing semicolon")
    .with_fix("add semicolon", span, ";")

// Maybe-incorrect fix (needs human review)
Diagnostic::error(ErrorCode::E2001)
    .with_message("type mismatch")
    .with_maybe_fix("convert to int", span, "int(x)")

// Plain text suggestion
Diagnostic::error(ErrorCode::E2003)
    .with_message("unknown identifier `pritn`")
    .with_suggestion("did you mean `print`?")

// Structured suggestion with explicit applicability
Diagnostic::error(ErrorCode::E2003)
    .with_message("unknown identifier `pritn`")
    .with_structured_suggestion(Suggestion::did_you_mean("print"))
```

### Typo Correction

```rust
fn fix_typo(span: Span, wrong: &str, correct: &str) -> Suggestion {
    Suggestion::machine_applicable(
        format!("change `{}` to `{}`", wrong, correct),
        span,
        correct,
    )
}
```

### Type Conversion

```rust
fn fix_type_conversion(span: Span, from: &Type, to: &Type) -> Option<Suggestion> {
    let conversion = match (from, to) {
        (Type::String, Type::Int) => "int",
        (Type::Int, Type::String) => "str",
        (Type::Int, Type::Float) => "float",
        _ => return None,
    };

    // Wrap expression: expr -> int(expr)
    Some(Suggestion::maybe_incorrect(
        format!("convert using `{}()`", conversion),
        Span::new(span.start, span.start),
        format!("{}(", conversion),
    ).with_substitution(
        Span::new(span.end, span.end),
        ")",
    ))
}
```

### Missing Import

```rust
fn fix_missing_import(module: &str, item: &str, insert_pos: u32) -> Suggestion {
    Suggestion::machine_applicable(
        format!("add import from '{}'", module),
        Span::new(insert_pos, insert_pos),
        format!("use '{}' {{ {} }}\n", module, item),
    )
}
```

### Add Missing Field

```rust
fn fix_missing_field(struct_span: Span, field: &str) -> Suggestion {
    Suggestion::has_placeholders(
        format!("add missing field `{}`", field),
        // Insert before closing brace
        Span::new(struct_span.end - 1, struct_span.end - 1),
        format!(", {}: /* TODO */", field),
    )
}
```

## Fix Suggestions per Error

### E2001: Type Mismatch

```rust
fn suggest_for_type_mismatch(expected: &Type, found: &Type, span: Span) -> Vec<Suggestion> {
    let mut fixes = Vec::new();

    // Suggest conversion if possible
    if let Some(fix) = fix_type_conversion(span, found, expected) {
        fixes.push(fix);
    }

    // Suggest changing annotation
    if let Some(annotation_span) = find_type_annotation(span) {
        fixes.push(Suggestion::maybe_incorrect(
            format!("change type annotation to `{}`", found.display()),
            annotation_span,
            found.display().to_string(),
        ));
    }

    fixes
}
```

### E2002: Undefined Variable

```rust
fn suggest_for_undefined_var(name: Name, similar: &[Name], span: Span) -> Vec<Suggestion> {
    similar.iter().map(|&s| {
        fix_typo(span, &name.to_string(), &s.to_string())
    }).collect()
}
```

### E2003: Missing Capability

```rust
fn suggest_for_missing_capability(cap: Capability, func_span: Span) -> Vec<Suggestion> {
    vec![Suggestion::machine_applicable(
        format!("add `uses {}` to function signature", cap),
        find_capability_insert_point(func_span),
        format!(" uses {}", cap),
    )]
}
```

## IDE Integration

Fixes are exposed in LSP `codeAction` responses:

```json
{
  "title": "change `pritn` to `print`",
  "kind": "quickfix",
  "diagnostics": [{ "code": "E2002" }],
  "edit": {
    "changes": {
      "file:///src/main.ori": [
        {
          "range": { "start": { "line": 5, "character": 4 }, "end": { "line": 5, "character": 9 } },
          "newText": "print"
        }
      ]
    }
  },
  "isPreferred": true
}
```

## Best Practices

1. **Prefer MachineApplicable** - Users trust automatic fixes
2. **Use HasPlaceholders** for incomplete fixes - Don't leave broken code
3. **Provide multiple options** - Let user choose
4. **Order by likelihood** - Put best fix first
5. **Explain the fix** - Clear message helps user learn
