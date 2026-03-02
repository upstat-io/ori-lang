---
title: "Code Fixes"
description: "Ori Compiler Design — Automated Code Fix System"
order: 1302
section: "Diagnostics"
---

# Code Fixes

A compiler that only tells you what went wrong is useful. A compiler that also tells you how to fix it is transformative. This chapter describes how Ori's automated code fix system is designed, how fixes attach to diagnostics, and how they flow to IDEs and the `ori fix` command-line tool.

## What Are Automated Code Fixes?

Compiler error messages evolved through several generations. The first generation reported the location and a bare error code. The second added human-readable prose. The third — where modern compilers now operate — added **suggestions**: structured repair instructions that a tool can apply automatically.

The progression has three tiers:

**Plain text suggestions** are human-readable strings like `did you mean \`print\`?`. They appear in terminal output as help text. An IDE can show them but cannot apply them mechanically — they contain no span information, no replacement text. They guide the programmer's hands; they do not move them.

**Structured suggestions** carry a span (where to change) and a snippet (what to replace it with). These are machine-applicable. The `ori fix` command can read them and rewrite the source file without human intervention. An LSP server can present them as code actions — one click in the editor applies the fix. The programmer still approves, but the edit itself is generated.

**Multi-substitution fixes** extend structured suggestions to cases where a single logical fix requires edits at multiple locations. Wrapping an expression `x` in `int(x)` requires inserting `int(` before the expression and `)` after it — two substitutions, one semantic operation. These compose via the `with_substitution` chaining API.

### The Confidence Spectrum

Not every fix the compiler can propose is equally safe to apply. Ori uses four **applicability** levels:

| Level | Meaning | Auto-apply? |
|---|---|---|
| `MachineApplicable` | Definitely correct | Yes |
| `MaybeIncorrect` | Probably correct, may change semantics | Human review |
| `HasPlaceholders` | Contains `<type>` or `/* TODO */` markers | User must complete |
| `Unspecified` | Informational only | No |

`MachineApplicable` is the goal. A missing semicolon, a typo in an identifier, a missing closing delimiter — these are safe. `MaybeIncorrect` covers type conversions (the compiler cannot always know whether a conversion is semantically right, only that it is type-correct). `HasPlaceholders` is used when the fix structure is known but part of the content is not — for example, adding a missing struct field with a placeholder value.

### Why This Matters

IDEs expose machine-applicable suggestions as code actions — the light-bulb menu that appears next to an error. Batch tooling (`ori fix --apply`) can rewrite entire codebases. Structured suggestions also aid learning: seeing the exact diff between broken and fixed code teaches idioms faster than prose alone.

## Ori's Code Fix Architecture

The system has two independent levels, chosen based on the origin of the fix knowledge.

**Inline suggestions on `Diagnostic`** — the diagnostic creator attaches the fix at the point the error is detected. This handles the common case: the code that identifies the error usually has the context to suggest a repair. A type mismatch error is raised in the type checker; the type checker knows both the expected type and the found type and can immediately propose a conversion. Inline suggestions use `.with_suggestion()` (text only), `.with_fix()` (machine-applicable), `.with_maybe_fix()` (uncertain), and `.with_structured_suggestion()` (full control).

**`CodeFix` trait + `FixRegistry`** — independent fix providers that inspect completed diagnostics and generate `CodeAction` objects. This layer is designed for cross-cutting fixes: cases where a single provider knows how to repair an entire class of errors regardless of which subsystem emitted them. It is also the natural integration point for LSP, where fixes must be served on demand when the editor requests code actions for a given cursor position.

The design principle: inline suggestions handle the common case cheaply. `FixRegistry` handles the cross-cutting case correctly. Neither replaces the other.

**Current status**: both levels are fully implemented. Inline suggestions are used throughout the compiler. The `FixRegistry` framework is complete — `CodeFix` trait, `FixRegistry` struct, `CodeAction`, `TextEdit`, `FixContext` — but zero production fix providers are registered. The registry will be populated when LSP is built.

## Suggestion and Substitution Types

The core types live in `compiler/ori_diagnostic/src/diagnostic/mod.rs`.

### `Applicability`

```rust
pub enum Applicability {
    /// Definitely correct — safe to auto-apply.
    MachineApplicable,
    /// Probably correct, may change semantics.
    MaybeIncorrect,
    /// Contains placeholders the user must complete.
    HasPlaceholders,
    /// Unknown confidence — informational only.
    #[default]
    Unspecified,
}
```

`Applicability::is_machine_applicable()` is the single predicate used by `ori fix` and the LSP to gate automatic application.

### `Substitution`

```rust
pub struct Substitution {
    /// The byte-offset span to replace.
    pub span: Span,
    /// The replacement text.
    pub snippet: String,
}
```

A substitution is a span-replacement pair. The span uses byte offsets into the source file, matching the span representation used everywhere else in the compiler. An empty span (`start == end`) represents a pure insertion.

### `Suggestion`

```rust
pub struct Suggestion {
    /// Human-readable description of the fix.
    pub message: String,
    /// Substitutions to apply (empty for text-only suggestions).
    pub substitutions: Vec<Substitution>,
    /// Confidence level.
    pub applicability: Applicability,
    /// Priority for display ordering (0 = most likely, 3 = unlikely).
    pub priority: u8,
}
```

The `priority` field controls ordering when multiple suggestions apply to the same error. Priority 0 is reserved for high-confidence suggestions like exact typo matches; priority 3 is for speculative suggestions the programmer may want to see but should not act on without thought.

### Factory Methods

```rust
impl Suggestion {
    // Span-bearing constructors
    pub fn machine_applicable(message, span, snippet) -> Self;
    pub fn maybe_incorrect(message, span, snippet) -> Self;
    pub fn has_placeholders(message, span, snippet) -> Self;

    // Text-only constructors
    pub fn text(message, priority) -> Self;
    pub fn did_you_mean(suggestion) -> Self;           // priority 0
    pub fn wrap_in(wrapper, example) -> Self;          // priority 1

    // Text with replacement (MaybeIncorrect, no explicit applicability)
    pub fn text_with_replacement(message, priority, span, new_text) -> Self;

    // Multi-span chaining
    #[must_use]
    pub fn with_substitution(self, span, snippet) -> Self;

    // Predicate
    pub fn is_text_only(&self) -> bool;
}
```

`did_you_mean` and `wrap_in` are text-only shortcuts. They produce human-readable hints with no span data. They appear in terminal output but are not machine-applicable. `text_with_replacement` is a middle ground — it carries a span but sets `MaybeIncorrect` applicability, appropriate for suggestions where the replacement is known but its correctness is not guaranteed.

## The Diagnostic Builder Integration

`Diagnostic` exposes a fluent builder API. The fix-related methods map directly to the two `suggestions` fields on `Diagnostic`:

```rust
pub struct Diagnostic {
    // ...
    /// Plain text suggestions (human-readable, no substitution).
    pub suggestions: Vec<String>,
    /// Structured suggestions with spans (for `ori fix` and LSP).
    pub structured_suggestions: Vec<Suggestion>,
}
```

The builder methods:

```rust
impl Diagnostic {
    /// Plain text — goes to `suggestions`. Terminal-only, not machine-applicable.
    pub fn with_suggestion(self, suggestion: impl Into<String>) -> Self;

    /// Full control — goes to `structured_suggestions`.
    pub fn with_structured_suggestion(self, suggestion: Suggestion) -> Self;

    /// Convenience: machine-applicable single-span fix.
    pub fn with_fix(self, message, span, snippet) -> Self;

    /// Convenience: maybe-incorrect single-span fix.
    pub fn with_maybe_fix(self, message, span, snippet) -> Self;
}
```

### Worked Examples

**Typo correction** — the simplest case. The fix is a single span replacement, definitely correct, machine-applicable:

```rust
Diagnostic::error(ErrorCode::E2002)
    .with_message("unknown identifier `pritn`")
    .with_fix("change `pritn` to `print`", ident_span, "print")
```

**Type conversion** — wrapping requires two substitutions. The fix might change semantics, so `MaybeIncorrect`:

```rust
Diagnostic::error(ErrorCode::E2001)
    .with_message("expected `int`, found `str`")
    .with_structured_suggestion(
        Suggestion::maybe_incorrect("convert using `int()`", expr_span.start_span(), "int(")
            .with_substitution(expr_span.end_span(), ")")
    )
```

Note `expr_span.start_span()` and `expr_span.end_span()` are zero-width spans at the start and end of the expression — pure insertions.

**Missing import** — inserting a `use` statement at the file top. The fix is structural and correct if the module exists:

```rust
Diagnostic::error(ErrorCode::E2002)
    .with_message("unknown identifier `sqrt`")
    .with_fix(
        "add `use std.math { sqrt }`",
        Span::new(import_insert_offset, import_insert_offset),
        "use std.math { sqrt }\n",
    )
```

**Plain text hint** — when no span is available, fall back to prose:

```rust
Diagnostic::error(ErrorCode::E2003)
    .with_message("function uses `Http` capability without declaring it")
    .with_suggestion("add `uses Http` to the function signature")
```

## The CodeFix Trait and FixRegistry

These types live in `compiler/ori_diagnostic/src/fixes/`.

### `CodeFix` Trait

```rust
pub trait CodeFix: Send + Sync {
    /// Error codes this fix handles.
    fn error_codes(&self) -> &'static [ErrorCode];

    /// Generate fix actions for a specific diagnostic.
    ///
    /// Returns empty if the fix does not apply to this particular diagnostic
    /// even if the error code matches — error codes are necessary but not
    /// sufficient for applicability.
    fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction>;

    /// Unique identifier for debugging and testing.
    /// Defaults to `std::any::type_name::<Self>()`.
    fn id(&self) -> &'static str { std::any::type_name::<Self>() }
}
```

`CodeFix` is `Send + Sync` because the `FixRegistry` stores fixes in `Arc<dyn CodeFix>` and the registry will be shared across threads in the LSP server. Fixes must be stateless or use interior mutability correctly.

### `FixRegistry`

```rust
pub struct FixRegistry {
    /// All registered fixes — stored once.
    fixes: Vec<Arc<dyn CodeFix>>,
    /// Maps error code → indices into `fixes`. O(1) lookup.
    by_code: HashMap<ErrorCode, Vec<usize>>,
}

impl FixRegistry {
    pub fn new() -> Self;
    pub fn register<F: CodeFix + 'static>(&mut self, fix: F);
    pub fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction>;
    pub fn has_fixes_for(&self, code: ErrorCode) -> bool;
    pub fn fix_count(&self) -> usize;
    pub fn mapping_count(&self) -> usize;
}
```

Registration stores the fix once in `fixes` and records its index in `by_code` for every error code it declares. Lookup is O(1) by error code — no linear scan over all registered fixes. A fix that handles multiple error codes is stored once and indexed under each.

### `FixContext`

```rust
pub struct FixContext<'a> {
    pub diagnostic: &'a Diagnostic,
    pub source: &'a str,
}

impl<'a> FixContext<'a> {
    pub fn primary_span(&self) -> Option<Span>;
    pub fn text_at(&self, span: Span) -> &str;
}
```

`text_at` lets a fix provider read the source text at any span — for example, to extract the identifier text for a typo-correction suggestion.

### `TextEdit` and `CodeAction`

```rust
pub struct TextEdit {
    pub span: Span,
    pub new_text: String,
}

impl TextEdit {
    pub fn replace(span, new_text) -> Self;
    pub fn insert(at: u32, text) -> Self;  // empty span at `at`
    pub fn delete(span) -> Self;           // empty new_text

    pub fn is_insert(&self) -> bool;
    pub fn is_delete(&self) -> bool;
    pub fn is_replace(&self) -> bool;
}

pub struct CodeAction {
    pub title: String,
    pub edits: Vec<TextEdit>,
    pub is_preferred: bool,
}

impl CodeAction {
    pub fn new(title, edits) -> Self;
    #[must_use] pub fn preferred(self) -> Self;
    #[must_use] pub fn with_edit(self, edit) -> Self;
}
```

`CodeAction` is the LSP-facing type. One diagnostic can produce multiple actions — each gets a title, a set of text edits, and an `is_preferred` flag that marks the most likely correct fix. The LSP maps `is_preferred` to the `isPreferred` field in the `CodeAction` response, which IDEs use to highlight the primary fix.

## Fix Categories

Below are the canonical fix patterns organized by what they repair.

### Typo Correction

Edit-distance matching produces a ranked list of candidates. The closest match is machine-applicable; more distant candidates are demoted to lower priority.

```rust
fn fix_typo(span: Span, wrong: &str, correct: &str) -> Suggestion {
    Suggestion::machine_applicable(
        format!("change `{}` to `{}`", wrong, correct),
        span,
        correct,
    )
}
```

For the `CodeFix` layer, this becomes a `TextEdit::replace` inside a `CodeAction`.

### Type Conversion

Wrapping changes semantics — `int("42")` parses a string, which is not the same operation as discarding the string. This warrants `MaybeIncorrect`:

```rust
fn fix_type_conversion(
    expr_span: Span,
    to_type: &str,
) -> Suggestion {
    Suggestion::maybe_incorrect(
        format!("convert using `{}()`", to_type),
        Span::new(expr_span.start, expr_span.start), // insert before
        format!("{}(", to_type),
    )
    .with_substitution(
        Span::new(expr_span.end, expr_span.end), // insert after
        ")",
    )
}
```

### Missing Import

A missing identifier that matches a known module item can be fixed by inserting a `use` declaration. This is machine-applicable when the module path is unambiguous:

```rust
fn fix_missing_import(
    module: &str,
    item: &str,
    insert_offset: u32,
) -> Suggestion {
    Suggestion::machine_applicable(
        format!("add `use {} {{ {} }}`", module, item),
        Span::new(insert_offset, insert_offset),
        format!("use {} {{ {} }}\n", module, item),
    )
}
```

### Missing Struct Field

When a struct literal omits a required field, the fix inserts it with a placeholder:

```rust
fn fix_missing_field(insert_before_brace: u32, field: &str) -> Suggestion {
    Suggestion::has_placeholders(
        format!("add missing field `{}`", field),
        Span::new(insert_before_brace, insert_before_brace),
        format!(", {}: /* TODO */", field),
    )
}
```

`HasPlaceholders` prevents auto-application — the programmer must supply the value.

### Missing Capability Declaration

When a function calls code that requires a capability but the function does not declare it, the fix inserts `uses Cap` into the function signature:

```rust
fn fix_missing_capability(
    uses_insert_span: Span,
    capability: &str,
) -> Suggestion {
    Suggestion::machine_applicable(
        format!("add `uses {}` to the function signature", capability),
        uses_insert_span,
        format!(" uses {}", capability),
    )
}
```

This is machine-applicable: adding a capability declaration does not change semantics, it only widens the function's effect type.

### Missing Semicolon

A missing statement terminator is always machine-applicable:

```rust
fn fix_missing_semicolon(insert_after: u32) -> Suggestion {
    Suggestion::machine_applicable(
        "add semicolon",
        Span::new(insert_after, insert_after),
        ";",
    )
}
```

## LSP Integration

When the LSP server receives a `textDocument/codeAction` request, it:

1. Retrieves the `Diagnostic` for the cursor position from its cache.
2. Calls `FixRegistry::get_fixes(&FixContext { diagnostic, source })`.
3. Translates each `CodeAction` into the LSP wire format.
4. Also reads `diagnostic.structured_suggestions` for inline fixes attached at diagnostic creation time.

The LSP wire format for a code action:

```json
{
  "title": "change `pritn` to `print`",
  "kind": "quickfix",
  "diagnostics": [{ "code": "E2002" }],
  "edit": {
    "changes": {
      "file:///src/main.ori": [
        {
          "range": {
            "start": { "line": 5, "character": 4 },
            "end":   { "line": 5, "character": 9 }
          },
          "newText": "print"
        }
      ]
    }
  },
  "isPreferred": true
}
```

The `isPreferred` flag maps from `CodeAction::is_preferred`. IDEs use it to auto-select a fix without user interaction when the context is unambiguous (e.g., "fix all in file" operations). Only one action per diagnostic should be marked preferred.

Multi-span fixes produce multiple entries in `changes` for the same file — one per `TextEdit`.

## Prior Art

**rustc and Clippy** — The most direct influence. rustc introduced the `Applicability` enum (`MachineApplicable`, `MaybeIncorrect`, `HasPlaceholders`, `Unspecified`) and the `rustfix` tool that batch-applies machine-applicable suggestions across a project. Clippy's fix providers follow the same trait pattern — `LateLintPass` implementations emit structured suggestions with explicit applicability. Ori's `Suggestion` type mirrors rustc's design almost exactly.

**TypeScript language service** — TypeScript decomposes fixes into `codeFixProvider` modules, each registered for specific diagnostic codes via `errorCodes: number[]`. The `getCodeActions(context)` method maps directly to Ori's `CodeFix::get_fixes`. TypeScript's providers are stateless singletons looked up by error code — the same pattern as `FixRegistry`.

**Clang** — Introduced fix-it hints in 2009, embedded inline in diagnostic output. The `-fixit` flag applies them automatically. Clang's hints are single-span replacements only; Ori extends this to multi-span via `with_substitution` chaining.

**Elm** — Known for exceptional error messages, Elm uses edit-distance suggestions (`did you mean X?`) throughout. Elm's suggestions are always text-only — no machine-applicable substitutions. This simplicity keeps the error format clean but limits IDE integration. Ori's `Suggestion::did_you_mean` is the text-only equivalent; `Suggestion::machine_applicable` is the structured upgrade.

**Roslyn (C#)** — `CodeFixProvider` with `RegisterCodeFixesAsync`, `FixableDiagnosticIds`, and `SyntaxAnnotation` for tracking nodes through rewrites. Roslyn's equivalent of `is_preferred` is `EquivalenceKey` combined with `IsPreferred` on code actions. The key difference: Roslyn operates on syntax trees, Ori operates on source spans. Span-based edits are simpler but require careful ordering when multiple edits touch overlapping regions.

## Design Tradeoffs

**Inline suggestions vs. external fix providers** — Inline fixes are simpler and collocated with the error detection logic. External providers decouple fixes from the emitting phase, enabling fixes that aggregate knowledge from multiple phases. Ori uses both: inline for the common case, `FixRegistry` for LSP cross-cutting concerns.

**`MachineApplicable` confidence** — The risk of marking a fix `MachineApplicable` incorrectly is that `ori fix --apply` silently makes a change the programmer did not intend. The conservative policy: when in doubt, prefer `MaybeIncorrect`. Typo corrections, missing delimiters, and missing semicolons are safe. Type conversions, imports, and structural changes are not.

**Multi-span substitutions** — The `with_substitution` chaining API enables wrapping and structural repairs in a single suggestion. The complexity cost is that applying multi-span fixes must handle overlapping spans correctly (last-wins or error). The current implementation leaves overlap resolution to the consumer.

**Framework-first vs. demand-driven** — `FixRegistry` and `CodeFix` exist before any production fix providers. The tradeoff: upfront design cost for zero immediate benefit, but the API is stable when LSP is built and no retrofitting is required. The alternative — bolt fixes on later — risks a messy API surface as different subsystems add fixes ad hoc.

**Priority ordering (0–3)** — Priority is a hint, not a guarantee. Emitters present fixes sorted by priority ascending, but all fixes for a given error are shown. This is different from Roslyn's `isPreferred`, which is binary. The four-level scale allows finer ranking within a diagnostic's fix list without hiding alternatives.

## Related Documents

- [Diagnostics Overview](index.md) — `Diagnostic` structure, error codes, `DiagnosticQueue`
- [Problem Types](problem-types.md) — Phase-specific error types and `into_diagnostic()`
- [Emitters](emitters.md) — Terminal, JSON, and SARIF output including suggestion rendering
