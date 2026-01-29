# Proposal: Structured Diagnostics and Auto-Fix

**Status:** Approved
**Author:** Claude
**Created:** 2026-01-23
**Draft:** 2026-01-25
**Approved:** 2026-01-28
**Affects:** `compiler/ori_diagnostic/`, `compiler/oric/`, CLI interface

## Summary

Extend the compiler's diagnostic system to output structured, machine-readable diagnostics with actionable fix suggestions that can be automatically applied. This enables:

1. AI agents to programmatically consume and act on compiler errors
2. IDE integrations via JSON output
3. `ori check --fix` to automatically resolve fixable errors
4. Better human-readable output with source snippets

## Motivation

Ori is designed as an "AI-first" language. The strict formatting rules (vertical stacking, named arguments, mandatory trailing commas) mean the compiler often knows the *exact* fix for an error, not just a description of the problem.

Current state:
```
  12..45: expected `target:`, found `.target_value:`
```

Proposed state (human):
```
error[E1010]: unknown pattern argument
  --> src/main.ori:12:5
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
    "severity": "Error",
    "message": "unknown pattern argument",
    "file": "src/main.ori",
    "span": {
      "start": 12,
      "end": 45,
      "start_loc": { "line": 12, "column": 5 },
      "end_loc": { "line": 12, "column": 18 }
    },
    "labels": [
      {
        "span": { "start": 12, "end": 25, "start_loc": { "line": 12, "column": 5 }, "end_loc": { "line": 12, "column": 18 } },
        "message": "unknown argument",
        "primary": true
      }
    ],
    "notes": ["valid arguments are: `over:`, `where:`, `.map:`"],
    "suggestions": ["did you mean `target:`?"],
    "structured_suggestions": [{
      "message": "replace `.target_value:` with `target:`",
      "substitutions": [{ "span": { "start": 12, "end": 25 }, "snippet": "target:" }],
      "applicability": "MaybeIncorrect"
    }]
  }],
  "summary": { "errors": 1, "warnings": 0, "fixable": 1 }
}
```

## Design

### Existing Infrastructure

The core diagnostic types are already implemented in `ori_diagnostic/src/diagnostic.rs`:

```rust
/// Applicability level for code suggestions.
pub enum Applicability {
    /// The suggestion is definitely correct and can be auto-applied.
    MachineApplicable,
    /// The suggestion might be correct but requires human verification.
    MaybeIncorrect,
    /// The suggestion contains placeholders that need user input.
    HasPlaceholders,
    /// Applicability unknown or suggestion is informational only.
    Unspecified,
}

/// A text substitution for a code fix.
pub struct Substitution {
    pub span: Span,
    pub snippet: String,
}

/// A structured suggestion with substitutions and applicability.
pub struct Suggestion {
    pub message: String,
    pub substitutions: Vec<Substitution>,
    pub applicability: Applicability,
}

/// A rich diagnostic with all context needed for great error messages.
pub struct Diagnostic {
    pub code: ErrorCode,
    pub severity: Severity,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub suggestions: Vec<String>,
    pub structured_suggestions: Vec<Suggestion>,  // Already present!
}
```

The `Diagnostic` type already has a `structured_suggestions` field and builder methods like `.with_fix()` and `.with_maybe_fix()`.

### New Types

The following type needs to be added for line/column information:

```rust
/// Source location with line and column information.
pub struct SourceLoc {
    /// 1-based line number
    pub line: u32,
    /// 1-based column (Unicode codepoints from line start)
    pub column: u32,
}
```

### JSON Output Enhancement

The existing JSON emitter (`ori_diagnostic/src/emitter/json.rs`) needs to be enhanced to include:

1. **File path** — Currently missing; must be added at the diagnostic level
2. **Line/column locations** — Add `start_loc` and `end_loc` to spans
3. **Structured suggestions** — Currently omitted from JSON output
4. **Summary object** — Add error/warning/fixable counts at the end

### CLI Flags

```
ori check <file>              # Human-readable output (improved)
ori check <file> --json       # JSON output for tooling/AI agents
ori check <file> --fix        # Apply MachineApplicable fixes only
ori check <file> --fix --dry  # Show what --fix would change
ori check <file> --fix=all    # Apply MachineApplicable + MaybeIncorrect
```

**Fix levels:**
- `--fix` (default): Only `MachineApplicable` — guaranteed safe
- `--fix=all`: Also `MaybeIncorrect` — likely correct, review diff

### Fix Categories by Applicability

#### MachineApplicable (auto-apply with `--fix`)

| Error | Fix | Why Safe |
|-------|-----|----------|
| Trailing comma missing | Add `,` | Ori requires trailing commas |
| Wrong indentation | Fix to 4 spaces | Ori enforces indentation |
| Inline comment | Move to own line | Ori forbids inline comments |
| Extra blank lines | Remove | Ori forbids consecutive blanks |
| Typo in pattern arg | `.targt:` → `target:` | Only one valid spelling |

#### MaybeIncorrect (suggest but don't auto-apply by default)

| Error | Fix | Why Uncertain |
|-------|-----|---------------|
| `int` ↔ `float` mismatch | Add `x as int` or `x as float` | User might want different conversion |
| Missing `Some()` wrapper | Wrap in `Some(x)` | Value might intentionally be wrong type |
| Unknown identifier | "Did you mean `similar_name`?" | Multiple similar names possible |

#### HasPlaceholders (show suggestion, requires user input)

| Error | Fix | Placeholder |
|-------|-----|-------------|
| Missing required pattern arg | Add `arg: ???` | User must provide value |
| Missing function body | Add `= ???` | User must implement |
| Missing type annotation | Add `: ???` | User must specify type |

## Implementation Plan

### Step 1: SourceLoc Type
- Add `SourceLoc` struct to `ori_diagnostic`
- Add span-to-location conversion utility using source text
- Build line index for efficient lookups

### Step 2: JSON Output Enhancement
- Add file path to diagnostic JSON output
- Add `start_loc`/`end_loc` to span serialization
- Add `structured_suggestions` to JSON output
- Add summary object with counts

### Step 3: Improved Human Output
- Implement Rust-style snippet extraction with context lines
- Implement diagnostic renderer with colors and arrows
- Show "fix available" indicator for fixable diagnostics

### Step 4: Auto-Fix Infrastructure
- Implement `apply_suggestions()` function
- Handle overlapping substitutions (reject or merge)
- Add `--fix` and `--fix --dry` flags to CLI

### Step 5: Upgrade Existing Diagnostics
- Convert type error suggestions to structured suggestions
- Convert pattern validation suggestions to structured suggestions
- Convert parser error suggestions to structured suggestions
- Assign appropriate `Applicability` level to each

### Step 6: Extended Fixes
- Add typo detection (Levenshtein distance) for identifiers
- Add formatting fixes (indentation, commas, blank lines)
- Add import suggestions for unknown types
- Add "did you mean" for pattern names

## AI Agent Integration

With structured output, an AI agent workflow becomes:

```
1. Agent runs: ori check file.ori --json
2. Agent receives structured diagnostics
3. Agent analyzes:
   - 3 errors, all MachineApplicable
   - Decision: apply fixes automatically
4. Agent runs: ori check file.ori --fix
5. Agent verifies: ori check file.ori --json returns no errors
6. Agent reports: "Fixed 3 errors automatically"
```

## References

- Rust compiler diagnostics: `~/lang_repos/rust/compiler/rustc_errors/`
- Go analysis framework: `~/lang_repos/golang/src/cmd/vendor/golang.org/x/tools/go/analysis/`
- TypeScript code fixes: `~/lang_repos/typescript/src/services/codefixes/`
