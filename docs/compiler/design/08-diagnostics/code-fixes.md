# Code Fixes

Code fixes are automatic repair suggestions that can be applied to resolve errors.

## Location

```
compiler/oric/src/diagnostic/fixes/mod.rs (~258 lines)
```

## CodeFix Structure

```rust
pub struct CodeFix {
    /// Description shown to user
    pub message: String,

    /// Text edits to apply
    pub edits: Vec<TextEdit>,

    /// How safe is this fix?
    pub applicability: Applicability,
}

pub struct TextEdit {
    /// Range to replace
    pub span: Span,

    /// Replacement text
    pub new_text: String,
}

pub enum Applicability {
    /// Safe to apply automatically (e.g., typo fix)
    MachineApplicable,

    /// Might change semantics (e.g., type conversion)
    MaybeIncorrect,

    /// Contains placeholders user must fill in
    HasPlaceholders,

    /// Just a suggestion, likely needs thought
    Unspecified,
}
```

## Creating Fixes

### Typo Correction

```rust
fn fix_typo(span: Span, wrong: &str, correct: &str) -> CodeFix {
    CodeFix {
        message: format!("change `{}` to `{}`", wrong, correct),
        edits: vec![TextEdit {
            span,
            new_text: correct.to_string(),
        }],
        applicability: Applicability::MachineApplicable,
    }
}
```

### Type Conversion

```rust
fn fix_type_conversion(span: Span, from: &Type, to: &Type) -> Option<CodeFix> {
    let conversion = match (from, to) {
        (Type::String, Type::Int) => "int",
        (Type::Int, Type::String) => "str",
        (Type::Int, Type::Float) => "float",
        _ => return None,
    };

    Some(CodeFix {
        message: format!("convert using `{}()`", conversion),
        edits: vec![
            // Wrap expression: expr -> int(expr)
            TextEdit { span: Span::new(span.start, span.start), new_text: format!("{}(", conversion) },
            TextEdit { span: Span::new(span.end, span.end), new_text: ")".into() },
        ],
        applicability: Applicability::MaybeIncorrect,
    })
}
```

### Missing Import

```rust
fn fix_missing_import(module: &str, item: &str, insert_pos: u32) -> CodeFix {
    CodeFix {
        message: format!("add import from '{}'", module),
        edits: vec![TextEdit {
            span: Span::new(insert_pos, insert_pos),
            new_text: format!("use '{}' {{ {} }}\n", module, item),
        }],
        applicability: Applicability::MachineApplicable,
    }
}
```

### Add Missing Field

```rust
fn fix_missing_field(struct_span: Span, field: &str, field_type: &Type) -> CodeFix {
    CodeFix {
        message: format!("add missing field `{}`", field),
        edits: vec![TextEdit {
            // Insert before closing brace
            span: Span::new(struct_span.end - 1, struct_span.end - 1),
            new_text: format!(", {}: /* TODO */", field),
        }],
        applicability: Applicability::HasPlaceholders,
    }
}
```

## Fix Suggestions per Error

### E2001: Type Mismatch

```rust
fn suggest_for_type_mismatch(expected: &Type, found: &Type, span: Span) -> Vec<CodeFix> {
    let mut fixes = Vec::new();

    // Suggest conversion if possible
    if let Some(fix) = fix_type_conversion(span, found, expected) {
        fixes.push(fix);
    }

    // Suggest changing annotation
    if let Some(annotation_span) = find_type_annotation(span) {
        fixes.push(CodeFix {
            message: format!("change type annotation to `{}`", found.display()),
            edits: vec![TextEdit {
                span: annotation_span,
                new_text: found.display().to_string(),
            }],
            applicability: Applicability::MaybeIncorrect,
        });
    }

    fixes
}
```

### E2002: Undefined Variable

```rust
fn suggest_for_undefined_var(name: Name, similar: &[Name], span: Span) -> Vec<CodeFix> {
    similar.iter().map(|&s| {
        fix_typo(span, &name.to_string(), &s.to_string())
    }).collect()
}
```

### E2003: Missing Capability

```rust
fn suggest_for_missing_capability(cap: Capability, func_span: Span) -> Vec<CodeFix> {
    vec![CodeFix {
        message: format!("add `uses {}` to function signature", cap),
        edits: vec![TextEdit {
            span: find_capability_insert_point(func_span),
            new_text: format!(" uses {}", cap),
        }],
        applicability: Applicability::MachineApplicable,
    }]
}
```

## Applying Fixes

### Single Fix

```rust
pub fn apply_fix(source: &str, fix: &CodeFix) -> String {
    let mut result = source.to_string();

    // Apply edits in reverse order to preserve spans
    let mut edits = fix.edits.clone();
    edits.sort_by(|a, b| b.span.start.cmp(&a.span.start));

    for edit in edits {
        result.replace_range(
            edit.span.start as usize..edit.span.end as usize,
            &edit.new_text,
        );
    }

    result
}
```

### Multiple Fixes

```rust
pub fn apply_all_fixes(source: &str, fixes: &[CodeFix]) -> String {
    // Collect all edits
    let mut all_edits: Vec<_> = fixes
        .iter()
        .filter(|f| f.applicability == Applicability::MachineApplicable)
        .flat_map(|f| f.edits.iter())
        .cloned()
        .collect();

    // Sort reverse by position
    all_edits.sort_by(|a, b| b.span.start.cmp(&a.span.start));

    // Check for overlaps
    for window in all_edits.windows(2) {
        if window[0].span.start < window[1].span.end {
            // Overlapping edits - apply one at a time
            return apply_fixes_one_by_one(source, fixes);
        }
    }

    // Apply all at once
    let mut result = source.to_string();
    for edit in all_edits {
        result.replace_range(
            edit.span.start as usize..edit.span.end as usize,
            &edit.new_text,
        );
    }
    result
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
