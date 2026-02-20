---
section: "06"
title: Production Code Fixes
status: not-started
goal: Implement 20+ concrete CodeFix instances covering the most common fixable errors
sections:
  - id: "06.1"
    title: Fix Infrastructure Audit
    status: not-started
  - id: "06.2"
    title: MachineApplicable Fixes
    status: not-started
  - id: "06.3"
    title: MaybeIncorrect Fixes
    status: not-started
  - id: "06.4"
    title: HasPlaceholders Fixes
    status: not-started
  - id: "06.5"
    title: Fix Application Engine
    status: not-started
  - id: "06.6"
    title: Completion Checklist
    status: not-started
---

# Section 06: Production Code Fixes

**Status:** Not Started
**Goal:** Implement concrete `CodeFix` instances for the most common fixable errors, fulfilling the structured-diagnostics-autofix proposal (Steps 5-6). The existing `CodeFix` trait and `FixRegistry` are in place but empty — this section populates them.

**Reference compilers:**
- **Rust** `compiler/rustc_errors/src/diagnostic.rs` — `Applicability::MachineApplicable` suggestions emitted directly from type checker
- **TypeScript** `src/services/codefixes/` — 50+ `CodeFixProvider` implementations, one per error code, registered in `codeFixProvider.ts`
- **Gleam** `compiler-core/src/error.rs` — Suggestions embedded per error variant, not a separate registry

**Current state:** `ori_diagnostic/src/fixes/{mod.rs, registry.rs}` define:
- `TextEdit { span, new_text }` with `replace()`, `insert()`, `delete()` constructors
- `CodeAction { title, edits, is_preferred }`
- `CodeFix` trait with `error_codes()`, `get_fixes(ctx)`, `id()`
- `FixRegistry` with `register()`, `get_fixes()`, `has_fixes_for()`
- **Zero implementations** — the registry is empty.

**Approved proposal:** `docs/ori_lang/proposals/approved/structured-diagnostics-autofix.md` defines fix categories by applicability. This section implements that proposal.

---

## 06.1 Fix Infrastructure Audit

Before implementing fixes, verify the infrastructure is sufficient:

### FixContext Enhancement

The current `FixContext` provides `primary_span()` and `text_at(span)`. For many fixes, we also need:

```rust
pub struct FixContext<'a> {
    pub diagnostic: &'a Diagnostic,
    pub source: &'a str,
    pub line_table: &'a LineOffsetTable, // For line/column queries
}

impl<'a> FixContext<'a> {
    /// Get the primary span of the diagnostic.
    pub fn primary_span(&self) -> Option<Span> { /* ... */ }

    /// Get the source text at a span.
    pub fn text_at(&self, span: Span) -> &str { /* ... */ }

    /// Get the line number for a byte offset.
    pub fn line_at(&self, offset: u32) -> u32 { /* ... */ }

    /// Get the full line text containing a byte offset.
    pub fn line_text_at(&self, offset: u32) -> &str { /* ... */ }

    /// Get the indentation of the line containing a byte offset.
    pub fn indentation_at(&self, offset: u32) -> &str { /* ... */ }
}
```

### Registry Initialization

Create a `default_fixes()` function that registers all built-in fixes:

```rust
/// Create a FixRegistry populated with all built-in code fixes.
pub fn default_fixes() -> FixRegistry {
    let mut registry = FixRegistry::new();
    // MachineApplicable
    registry.register(FixTrailingComma);
    registry.register(FixSemicolon);
    registry.register(FixTripleEquals);
    registry.register(FixIncrementDecrement);
    registry.register(FixSingleQuoteString);
    // MaybeIncorrect
    registry.register(FixTypoCast);
    registry.register(FixDidYouMean);
    // HasPlaceholders
    registry.register(FixMissingPatternArg);
    registry
}
```

- [ ] Audit `FixContext` — add `line_table`, `line_at()`, `line_text_at()`, `indentation_at()`
- [ ] Create `default_fixes()` initialization function
- [ ] Wire `default_fixes()` into the compilation driver
- [ ] Tests: `FixContext` methods work correctly

---

## 06.2 MachineApplicable Fixes

These fixes are **guaranteed safe** and can be auto-applied with `ori check --fix`.

### Fix: Trailing Comma (E1001)

```rust
struct FixTrailingComma;

impl CodeFix for FixTrailingComma {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[ErrorCode::E1001] // Expected comma
    }

    fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction> {
        // Insert comma at the expected position
        vec![CodeAction::new(
            "add trailing comma",
            vec![TextEdit::insert(ctx.primary_span().unwrap().end, ",")],
        ).preferred()]
    }
}
```

### Fix: `===` → `==` (E0008)

```rust
struct FixTripleEquals;

impl CodeFix for FixTripleEquals {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[ErrorCode::E0008] // Triple equals
    }

    fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction> {
        vec![CodeAction::new(
            "replace `===` with `==`",
            vec![TextEdit::replace(ctx.primary_span().unwrap(), "==")],
        ).preferred()]
    }
}
```

### Fix: `++`/`--` → Assign (E0010/E0011)

```rust
struct FixIncrementDecrement;

impl CodeFix for FixIncrementDecrement {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[ErrorCode::E0010, ErrorCode::E0011] // ++ and --
    }

    fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction> {
        let span = ctx.primary_span().unwrap();
        let text = ctx.text_at(span);
        let (replacement, desc) = if text.contains("++") {
            ("+= 1", "replace `++` with `+= 1`")
        } else {
            ("-= 1", "replace `--` with `-= 1`")
        };

        // Need to figure out the variable name — for now, suggest the pattern
        vec![CodeAction::new(desc, vec![
            TextEdit::replace(span, &format!("x {replacement}")),
        ])]
    }
}
```

### Fix: Single Quotes → Double Quotes (E0009)

```rust
struct FixSingleQuoteString;

impl CodeFix for FixSingleQuoteString {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[ErrorCode::E0009]
    }

    fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction> {
        let span = ctx.primary_span().unwrap();
        let text = ctx.text_at(span);
        // Replace surrounding ' with "
        if text.starts_with('\'') && text.ends_with('\'') && text.len() >= 2 {
            let inner = &text[1..text.len()-1];
            vec![CodeAction::new(
                "use double quotes for strings",
                vec![TextEdit::replace(span, &format!("\"{inner}\""))],
            ).preferred()]
        } else {
            vec![]
        }
    }
}
```

### Full MachineApplicable Fix List

| Fix | Error Code | Description |
|-----|-----------|-------------|
| `FixTrailingComma` | E1001 | Add missing trailing comma |
| `FixTripleEquals` | E0008 | Replace `===` with `==` |
| `FixSingleQuoteString` | E0009 | Replace `'...'` with `"..."` |
| `FixIncrementDecrement` | E0010/E0011 | Replace `++`/`--` with `+= 1`/`-= 1` |
| `FixReservedKeyword` | E0015 | Suggest alternative to reserved future keyword |

- [ ] Implement `FixTrailingComma`
- [ ] Implement `FixTripleEquals`
- [ ] Implement `FixSingleQuoteString`
- [ ] Implement `FixIncrementDecrement`
- [ ] Implement `FixReservedKeyword`
- [ ] Tests: each fix produces correct `TextEdit` operations
- [ ] Tests: preferred flag is set correctly

---

## 06.3 MaybeIncorrect Fixes

These fixes are **likely correct** but require human verification. Applied with `ori check --fix=all`.

### Fix: Did You Mean? (Multiple Error Codes)

Uses Section 01's suggest module:

```rust
struct FixDidYouMean;

impl CodeFix for FixDidYouMean {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[
            ErrorCode::E2002, // Unknown identifier (type checker)
            ErrorCode::E1010, // Unknown pattern argument
        ]
    }

    fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction> {
        // Extract suggested names from the diagnostic's notes
        // (populated by the suggest module in Section 01)
        let suggestions = extract_suggestions_from_notes(ctx.diagnostic);
        suggestions.into_iter().map(|suggested| {
            CodeAction::new(
                format!("replace with `{suggested}`"),
                vec![TextEdit::replace(ctx.primary_span().unwrap(), &suggested)],
            )
        }).collect()
    }
}
```

### Fix: Type Cast (E2001)

When types are compatible via cast (int ↔ float, int ↔ str):

```rust
struct FixTypeCast;

impl CodeFix for FixTypeCast {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[ErrorCode::E2001] // Type mismatch
    }

    fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction> {
        // Parse expected/found types from the diagnostic
        // Suggest `x as expected_type` if cast is valid
        // ...
        vec![]  // Populated based on type pair analysis
    }
}
```

### Fix: Option Wrapping (E2001)

When `T` is found but `Option(T)` is expected:

```rust
struct FixOptionWrap;

impl CodeFix for FixOptionWrap {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[ErrorCode::E2001]
    }

    fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction> {
        // If expected is Option(T) and found is T:
        // Suggest wrapping in Some()
        // ...
        vec![]
    }
}
```

### Full MaybeIncorrect Fix List

| Fix | Error Code | Description |
|-----|-----------|-------------|
| `FixDidYouMean` | E2002, E1010 | Replace with edit-distance suggestion |
| `FixTypeCast` | E2001 | Add `as target_type` cast |
| `FixOptionWrap` | E2001 | Wrap value in `Some()` |
| `FixResultWrap` | E2001 | Wrap value in `Ok()` |

- [ ] Implement `FixDidYouMean` (depends on Section 01)
- [ ] Implement `FixTypeCast`
- [ ] Implement `FixOptionWrap`
- [ ] Implement `FixResultWrap`
- [ ] Tests: each fix produces correct edits with `MaybeIncorrect` applicability

---

## 06.4 HasPlaceholders Fixes

These fixes require user input to complete. Shown as suggestions but never auto-applied.

### Fix: Missing Pattern Argument (E1010)

```rust
struct FixMissingPatternArg;

impl CodeFix for FixMissingPatternArg {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[ErrorCode::E1009] // Missing required pattern argument
    }

    fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction> {
        // Insert `arg_name: ???` at the right position
        // ??? is the placeholder the user must fill in
        vec![]
    }
}
```

### Full HasPlaceholders Fix List

| Fix | Error Code | Description |
|-----|-----------|-------------|
| `FixMissingPatternArg` | E1009 | Add `arg: ???` placeholder |
| `FixMissingFunctionBody` | E1006 | Add `= ???` body placeholder |
| `FixMissingTypeAnnotation` | E2007 | Add `: ???` type annotation |

- [ ] Implement `FixMissingPatternArg`
- [ ] Implement `FixMissingFunctionBody`
- [ ] Implement `FixMissingTypeAnnotation`
- [ ] Tests: placeholder edits contain `???` marker

---

## 06.5 Fix Application Engine

### Apply Function

The `apply_suggestions` function from the structured-diagnostics-autofix proposal:

```rust
/// Apply code fixes to source text.
///
/// Processes TextEdits in reverse order (highest offset first) to
/// avoid offset invalidation. Rejects overlapping edits.
///
/// # Arguments
/// * `source` — Original source text
/// * `edits` — TextEdit operations to apply (can be from multiple CodeActions)
///
/// # Returns
/// The modified source text, or an error if edits overlap.
pub fn apply_edits(source: &str, edits: &[TextEdit]) -> Result<String, ApplyError>
```

### Overlap Detection

```rust
/// Check if any edits in the list overlap.
pub fn check_overlaps(edits: &[TextEdit]) -> Result<(), Vec<(usize, usize)>>
```

Edits are sorted by span start, then checked pairwise for overlap. Overlapping edits are rejected (not merged) — this is the safe approach used by Rust and TypeScript.

### CLI Integration

Wire into `ori check --fix`:

```rust
// In oric CLI handler
if args.fix {
    let applicable_level = if args.fix_all {
        Applicability::MaybeIncorrect
    } else {
        Applicability::MachineApplicable
    };

    let fixes = registry.get_fixes(&ctx)
        .into_iter()
        .filter(|a| a.applicability() <= applicable_level)
        .collect();

    let edits = fixes.iter().flat_map(|a| &a.edits).collect();
    let new_source = apply_edits(&source, &edits)?;
    fs::write(&path, new_source)?;
}
```

- [ ] Implement `apply_edits(source, edits) -> Result<String, ApplyError>`
- [ ] Implement `check_overlaps(edits)`
- [ ] Wire `--fix` and `--fix=all` CLI flags
- [ ] Implement `--fix --dry` (show diff without applying)
- [ ] Tests: non-overlapping edits applied correctly
- [ ] Tests: overlapping edits rejected
- [ ] Tests: reverse-order application preserves offsets
- [ ] Tests: CLI integration (end-to-end)

---

## 06.6 Completion Checklist

- [ ] `FixContext` enhanced with line table and helper methods
- [ ] `default_fixes()` function creates populated registry
- [ ] 6+ MachineApplicable fixes implemented
- [ ] 4+ MaybeIncorrect fixes implemented
- [ ] 3+ HasPlaceholders fixes implemented
- [ ] `apply_edits()` function with overlap detection
- [ ] `--fix` and `--fix=all` CLI flags
- [ ] `--fix --dry` preview mode
- [ ] Tests: 30+ unit tests across all fix types
- [ ] Tests: 5+ end-to-end tests
- [ ] `./test-all.sh` passes

**Exit Criteria:** The `FixRegistry` contains 13+ concrete fix implementations. `ori check --fix` can auto-apply safe fixes. All fix types have applicability levels and are correctly categorized. The apply engine handles edit ordering and overlap detection.
