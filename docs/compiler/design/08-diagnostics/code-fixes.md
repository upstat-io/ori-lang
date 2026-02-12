---
title: "Code Fixes"
description: "Ori Compiler Design — Code Fixes"
order: 801
section: "Diagnostics"
---

# Code Fixes

Code fixes are automatic repair suggestions that can be applied to resolve errors.

## Location

```
compiler/ori_diagnostic/src/diagnostic.rs  # Suggestion, Substitution, Applicability types
compiler/ori_diagnostic/src/fixes/         # Fix registry and helpers
```

## Suggestion Structure

Code fixes are represented as `Suggestion` with `Substitution` components:

```rust
/// A structured suggestion with substitutions and applicability.
pub struct Suggestion {
    /// Human-readable message describing the fix.
    pub message: String,

    /// The text substitutions to make.
    pub substitutions: Vec<Substitution>,

    /// How confident we are in this suggestion.
    pub applicability: Applicability,
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

    /// Add another substitution to this suggestion.
    pub fn with_substitution(self, span: Span, snippet: impl Into<String>) -> Self;
}
```

## Creating Fixes

### Using Diagnostic Builder

The easiest way to add fixes is through the `Diagnostic` builder:

```rust
// Machine-applicable fix (safe to auto-apply)
Diagnostic::error(ErrorCode::E1001)
    .with_message("missing semicolon")
    .with_fix("add semicolon", span, ";")

// Maybe-incorrect fix (needs human review)
Diagnostic::error(ErrorCode::E2001)
    .with_message("type mismatch")
    .with_maybe_fix("convert to int", span, "int(x)")
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
