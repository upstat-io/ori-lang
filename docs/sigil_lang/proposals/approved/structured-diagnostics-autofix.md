# Proposal: Structured Diagnostics and Auto-Fix

**Status:** Approved
**Author:** Claude
**Created:** 2026-01-23
**Approved:** 2026-01-25
**Affects:** `compiler/sigilc/src/diagnostic.rs`, `main.rs`, CLI interface

## Summary

Extend the compiler's diagnostic system to output structured, machine-readable diagnostics with actionable fix suggestions that can be automatically applied. This enables:

1. AI agents to programmatically consume and act on compiler errors
2. IDE integrations via JSON output
3. `sigil check --fix` to automatically resolve fixable errors
4. Better human-readable output with source snippets

## Motivation

Sigil is designed as an "AI-first" language. The strict formatting rules (vertical stacking, named arguments, mandatory trailing commas) mean the compiler often knows the *exact* fix for an error, not just a description of the problem.

Current state:
```
  12..45: expected `target:`, found `.target_value:`
```

Proposed state (human):
```
error[E1010]: unknown pattern argument
  --> src/main.si:12:5
   |
12 |     .target_value: 5,
   |     ^^^^^^^^^^^^^ unknown argument
   |
   = note: valid arguments are: `over:`, `where:`, `.map:`
   = help: did you mean `target:`?

   fix available: replace `.target_value:` with `target:`
```

Proposed state (JSON for AI/tooling):
```json
{
  "diagnostics": [{
    "code": "E1010",
    "severity": "error",
    "message": "unknown pattern argument",
    "file": "src/main.si",
    "span": { "start": 12, "end": 45, "line": 12, "column": 5 },
    "labels": [
      { "span": { "line": 12, "column": 5, "len": 13 }, "message": "unknown argument", "primary": true }
    ],
    "notes": ["valid arguments are: `over:`, `where:`, `.map:`"],
    "fixes": [{
      "message": "replace `.target_value:` with `target:`",
      "edits": [{ "span": { "start": 12, "end": 25 }, "replacement": "target:" }],
      "applicability": "maybe_incorrect"
    }]
  }],
  "summary": { "errors": 1, "warnings": 0, "fixable": 1 }
}
```

## Design

### Fix and Edit Types

```rust
/// A suggested fix for a diagnostic
pub struct Fix {
    /// Human-readable description of the fix
    pub message: String,
    /// The edits to apply (usually one, but refactors may have multiple)
    pub edits: Vec<Edit>,
    /// Whether this fix can be automatically applied
    pub applicability: Applicability,
}

/// A single text edit
pub struct Edit {
    /// The span to replace (can be empty span for insertions)
    pub span: Span,
    /// The replacement text (can be empty for deletions)
    pub replacement: String,
}

/// Applicability level for auto-fix (modeled after Rust's Applicability)
pub enum Applicability {
    /// The fix is definitely correct and maintains exact semantics.
    /// Safe to apply automatically without review.
    MachineApplicable,

    /// The fix is likely correct but uncertain.
    /// Will produce valid Sigil code, but may not match user intent.
    MaybeIncorrect,

    /// The fix contains placeholders that require user input.
    /// Cannot be auto-applied.
    HasPlaceholders,

    /// Applicability unknown or suggestion is informational only.
    Unspecified,
}
```

### CLI Flags

```
sigil check <file>              # Human-readable output (improved)
sigil check <file> --json       # JSON output for tooling/AI agents
sigil check <file> --fix        # Apply MachineApplicable fixes only
sigil check <file> --fix --dry  # Show what --fix would change
sigil check <file> --fix=all    # Apply MachineApplicable + MaybeIncorrect
```

**Fix levels:**
- `--fix` (default): Only `MachineApplicable` - guaranteed safe
- `--fix=all`: Also `MaybeIncorrect` - likely correct, review diff

### Fix Categories by Applicability

#### MachineApplicable (auto-apply with `--fix`)

| Error | Fix | Why Safe |
|-------|-----|----------|
| Trailing comma missing | Add `,` | Sigil requires trailing commas |
| Wrong indentation | Fix to 4 spaces | Sigil enforces indentation |
| Inline comment | Move to own line | Sigil forbids inline comments |
| Extra blank lines | Remove | Sigil forbids consecutive blanks |
| Typo in pattern arg | `.targt:` → `target:` | Only one valid spelling |

#### MaybeIncorrect (suggest but don't auto-apply by default)

| Error | Fix | Why Uncertain |
|-------|-----|---------------|
| `int` ↔ `float` mismatch | Add `int(x)` or `float(x)` | User might want different conversion |
| Missing `Some()` wrapper | Wrap in `Some(x)` | Value might intentionally be wrong type |
| Unknown identifier | "Did you mean `similar_name`?" | Multiple similar names possible |

#### HasPlaceholders (show suggestion, requires user input)

| Error | Fix | Placeholder |
|-------|-----|-------------|
| Missing required pattern arg | Add `.arg: ???` | User must provide value |
| Missing function body | Add `= ???` | User must implement |
| Missing type annotation | Add `: ???` | User must specify type |

## Implementation Plan

### Phase 1: Core Types
- Add `Fix`, `Edit`, `Applicability` types to `diagnostic.rs`
- Update `Diagnostic` to use `Vec<Fix>` instead of `Vec<String>`
- Add `SourceLoc` and span-to-location conversion

### Phase 2: Upgrade Existing Suggestions
- Convert type error suggestions to structured fixes
- Convert pattern validation suggestions to structured fixes
- Convert parser error suggestions to structured fixes
- Assign appropriate `Applicability` level to each fix type

### Phase 3: JSON Output
- Add serde dependencies
- Implement `Serialize` for diagnostic types
- Add `--json` flag to CLI
- Create `DiagnosticOutput` wrapper with summary

### Phase 4: Improved Human Output
- Build line index for source files
- Implement snippet extraction with context lines
- Implement diagnostic renderer with colors and arrows

### Phase 5: Auto-Fix
- Implement `apply_fixes()` function
- Handle overlapping edits (reject or merge)
- Add `--fix` and `--fix --dry` flags

### Phase 6: Extended Fixes
- Add typo detection (Levenshtein distance) for identifiers
- Add formatting fixes (indentation, commas, blank lines)
- Add import suggestions for unknown types
- Add "did you mean" for pattern names

## AI Agent Integration

With structured output, an AI agent workflow becomes:

```
1. Agent runs: sigil check file.si --json
2. Agent receives structured diagnostics
3. Agent analyzes:
   - 3 errors, all MachineApplicable
   - Decision: apply fixes automatically
4. Agent runs: sigil check file.si --fix
5. Agent verifies: sigil check file.si --json returns no errors
6. Agent reports: "Fixed 3 errors automatically"
```

## References

- Rust compiler diagnostics: `~/lang_repos/rust/compiler/rustc_errors/`
- Go analysis framework: `~/lang_repos/golang/src/cmd/vendor/golang.org/x/tools/go/analysis/`
- TypeScript code fixes: `~/lang_repos/typescript/src/services/codefixes/`
