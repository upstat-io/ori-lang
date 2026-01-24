# Proposal: Structured Diagnostics and Auto-Fix

**Status:** Draft
**Author:** Claude
**Created:** 2026-01-23
**Affects:** `compiler/sigilc-v3/src/diagnostic.rs`, `main.rs`, CLI interface

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

## Current Implementation

### Diagnostic Struct (diagnostic.rs:195-209)

```rust
pub struct Diagnostic {
    pub code: ErrorCode,
    pub severity: Severity,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub suggestions: Vec<String>,  // Currently just strings
}
```

### Limitations

1. **Suggestions are strings** - "use `int(x)` to convert" tells humans what to do but isn't actionable by tools
2. **No line/column info** - Only byte offsets in `Span`, not source positions
3. **CLI doesn't use rich data** - `main.rs` just prints `span: message`
4. **No JSON output** - Can't be consumed by IDEs or AI agents
5. **No auto-fix** - Even when the compiler knows the fix, users must apply it manually

## Proposed Changes

### 1. Add `Fix` and `Edit` Types

```rust
/// A suggested fix for a diagnostic
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fix {
    /// Human-readable description of the fix
    pub message: String,
    /// The edits to apply (usually one, but refactors may have multiple)
    pub edits: Vec<Edit>,
    /// Whether this fix can be automatically applied
    pub applicability: Applicability,
}

/// A single text edit
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edit {
    /// The span to replace (can be empty span for insertions)
    pub span: Span,
    /// The replacement text (can be empty for deletions)
    pub replacement: String,
}

/// Applicability level for auto-fix (modeled after Rust's Applicability)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Applicability {
    /// The fix is definitely correct and maintains exact semantics.
    /// Safe to apply automatically without review.
    /// Examples: formatting fixes, missing trailing comma, typo in known keyword
    MachineApplicable,

    /// The fix is likely correct but uncertain.
    /// Will produce valid Sigil code, but may not match user intent.
    /// Examples: type conversion suggestions, "did you mean X?"
    MaybeIncorrect,

    /// The fix contains placeholders that require user input.
    /// Cannot be auto-applied - user must fill in `???` or similar.
    /// Examples: missing function argument, new variable name needed
    HasPlaceholders,

    /// Applicability unknown or suggestion is informational only.
    /// Show to user but never auto-apply.
    Unspecified,
}
```

### 2. Update Diagnostic Struct

```rust
pub struct Diagnostic {
    pub code: ErrorCode,
    pub severity: Severity,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub fixes: Vec<Fix>,  // Changed from suggestions: Vec<String>
}
```

### 3. Add Source Position Tracking

```rust
/// Rich source location with line/column info
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceLoc {
    pub file: PathBuf,
    pub span: Span,
    pub line: u32,      // 1-indexed
    pub column: u32,    // 1-indexed, in characters (not bytes)
    pub line_end: u32,  // For multi-line spans
    pub column_end: u32,
}

impl Span {
    /// Convert byte span to source location using line index
    pub fn to_source_loc(&self, source: &str, file: &Path) -> SourceLoc {
        // Compute line/column from byte offsets
    }
}
```

### 4. Add JSON Serialization

```rust
// In Cargo.toml
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

// In diagnostic.rs
#[derive(Serialize)]
pub struct DiagnosticOutput {
    pub diagnostics: Vec<JsonDiagnostic>,
    pub summary: Summary,
}

#[derive(Serialize)]
pub struct Summary {
    pub errors: usize,
    pub warnings: usize,
    pub fixable: usize,
}
```

### 5. CLI Flags

```
sigil check <file>              # Human-readable output (improved)
sigil check <file> --json       # JSON output for tooling/AI agents
sigil check <file> --fix        # Apply MachineApplicable fixes only
sigil check <file> --fix --dry  # Show what --fix would change
sigil check <file> --fix=all    # Apply MachineApplicable + MaybeIncorrect
sigil check <file> --fix=unsafe # Apply all non-placeholder fixes (use with caution)
```

**Fix levels:**
- `--fix` (default): Only `MachineApplicable` - guaranteed safe
- `--fix=all`: Also `MaybeIncorrect` - likely correct, review diff
- `--fix=unsafe`: Everything except `HasPlaceholders` - may break code

### 6. Improved Human Output

Render diagnostics like Rust's compiler:

```
error[E2001]: type mismatch
  --> src/math.si:15:12
   |
14 |     let x = get_value()
   |             ----------- this returns `Option<int>`
15 |     x + 1
   |     ^^^^^ expected `int`, found `Option<int>`
   |
   = help: use `.unwrap_or(default: 0)` to provide a default
   = help: or use `match` to handle the `None` case

   fix: add `.unwrap_or(default: 0)` [maybe-incorrect]
```

## Fix Categories by Applicability

### MachineApplicable (auto-apply with `--fix`)

These fixes are 100% correct and preserve semantics. Applied automatically.

| Error | Fix | Why Safe |
|-------|-----|----------|
| Trailing comma missing | Add `,` | Sigil requires trailing commas |
| Wrong indentation | Fix to 4 spaces | Sigil enforces indentation |
| Inline comment | Move to own line | Sigil forbids inline comments |
| Extra blank lines | Remove | Sigil forbids consecutive blanks |
| Typo in pattern arg | `.targt:` → `target:` | Only one valid spelling |
| Named arg not stacked | Reformat vertically | Sigil requires vertical stacking |

### MaybeIncorrect (suggest but don't auto-apply by default)

These fixes produce valid code but may not match user intent.

| Error | Fix | Why Uncertain |
|-------|-----|---------------|
| `int` ↔ `float` mismatch | Add `int(x)` or `float(x)` | User might want different conversion |
| Missing `Some()` wrapper | Wrap in `Some(x)` | Value might intentionally be wrong type |
| Missing list wrapper | Wrap in `[x]` | User might want different structure |
| Unknown identifier | "Did you mean `similar_name`?" | Multiple similar names possible |
| Type mismatch in return | Suggest conversion | Logic error vs type error unclear |

### HasPlaceholders (show suggestion, requires user input)

These fixes include `???` or similar that the user must fill in.

| Error | Fix | Placeholder |
|-------|-----|-------------|
| Missing required pattern arg | Add `.arg: ???` | User must provide value |
| Missing function body | Add `= ???` | User must implement |
| Missing type annotation | Add `: ???` | User must specify type |
| Incomplete match | Add `_ -> ???` | User must handle case |

### Unspecified (informational only)

These explain the problem but don't suggest specific fixes.

| Error | Note |
|-------|------|
| Logic errors | "This condition is always false" |
| Unreachable code | "Code after `panic()` never executes" |
| Unused binding | "Consider removing or using `_x`" |
| Architecture issues | "Consider extracting to separate function" |

## Implementation Plan

### Phase 1: Core Types

1. Add `Fix`, `Edit`, `Applicability` types to `diagnostic.rs`
2. Update `Diagnostic` to use `Vec<Fix>` instead of `Vec<String>`
3. Add `SourceLoc` and span-to-location conversion
4. Update all existing `with_suggestion()` calls to use new types

**Files:** `diagnostic.rs`, `ir/span.rs`

### Phase 2: Upgrade Existing Suggestions

1. Convert type error suggestions (`types.rs`) to structured fixes
2. Convert pattern validation suggestions to structured fixes
3. Convert parser error suggestions to structured fixes
4. Assign appropriate `Applicability` level to each fix type

**Files:** `types.rs`, `diagnostic.rs` helpers, `parser/mod.rs`

### Phase 3: JSON Output (1 day)

1. Add serde dependencies
2. Implement `Serialize` for diagnostic types
3. Add `--json` flag to CLI
4. Create `DiagnosticOutput` wrapper with summary

**Files:** `Cargo.toml`, `diagnostic.rs`, `main.rs`

### Phase 4: Improved Human Output (2-3 days)

1. Build line index for source files
2. Implement snippet extraction with context lines
3. Implement diagnostic renderer with colors and arrows
4. Add secondary label rendering

**Files:** `main.rs` or new `render.rs` module

### Phase 5: Auto-Fix (2-3 days)

1. Implement `apply_fixes()` function
2. Handle overlapping edits (reject or merge)
3. Add `--fix` and `--fix --dry` flags
4. Add `--fix=all` for lower-confidence fixes

**Files:** new `fix.rs` module, `main.rs`

### Phase 6: Extended Fixes (ongoing)

1. Add typo detection (Levenshtein distance) for identifiers
2. Add formatting fixes (indentation, commas, blank lines)
3. Add import suggestions for unknown types
4. Add "did you mean" for pattern names

## Example: Full Diagnostic Lifecycle

### Input (broken code)

```sigil
@add_numbers (a: int, b: int) -> int = run(
    let result = a + b
    result
)

@test_add tests @add_numbers () -> void = run(
    assert_eq(actual: add_numbers(a: 1, b: 2), expected: 3),
    assert_eq(actual: add_numbers(a: -1 b: 1), expected: 0),  // missing comma
)
```

### Diagnostic (internal)

```rust
Diagnostic {
    code: ErrorCode::E1001,
    severity: Severity::Error,
    message: "expected `,` or `)` after argument",
    labels: vec![
        Label::primary(span(156, 156), "expected `,` here"),
        Label::secondary(span(147, 155), "after this argument"),
    ],
    notes: vec![],
    fixes: vec![Fix {
        message: "add missing comma",
        edits: vec![Edit { span: span(156, 156), replacement: ",".into() }],
        applicability: Applicability::MachineApplicable,
    }],
}
```

### Human Output

```
error[E1001]: expected `,` or `)` after argument
  --> src/math.si:8:35
   |
 8 |     assert_eq(actual: add_numbers(a: -1 b: 1), expected: 0),
   |                                    ------- ^ expected `,` here
   |                                    |
   |                                    after this argument
   |
   fix available [machine-applicable]: add missing comma
```

### JSON Output

```json
{
  "diagnostics": [{
    "code": "E1001",
    "severity": "error",
    "message": "expected `,` or `)` after argument",
    "file": "src/math.si",
    "span": { "line": 8, "column": 35 },
    "labels": [
      { "span": { "line": 8, "column": 35, "len": 1 }, "message": "expected `,` here", "primary": true },
      { "span": { "line": 8, "column": 28, "len": 7 }, "message": "after this argument", "primary": false }
    ],
    "fixes": [{
      "message": "add missing comma",
      "edits": [{ "start": 156, "end": 156, "replacement": "," }],
      "applicability": "machine_applicable"
    }]
  }],
  "summary": { "errors": 1, "warnings": 0, "fixable": 1 }
}
```

### Auto-Fix Result

```bash
$ sigil check src/math.si --fix
Fixed 1 error:
  src/math.si:8:35 - added missing comma

$ sigil check src/math.si
No errors found.
```

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

For non-MachineApplicable fixes:
```
1. Agent receives: 1 error, MaybeIncorrect applicability
2. Agent reads the suggestion and source context
3. Agent applies its own judgment:
   - If confident in fix → apply manually or with --fix=all
   - If uncertain → ask user
4. Agent either applies fix or explains the issue
```

For HasPlaceholders fixes:
```
1. Agent receives: fix with `???` placeholder
2. Agent understands this requires reasoning about intent
3. Agent fills in placeholder based on context
4. Agent applies completed fix manually
```

## Backwards Compatibility

- Default output format unchanged (but improved)
- `--json` is opt-in
- `--fix` is opt-in
- No breaking changes to existing workflows

## Open Questions (Resolved)

Based on analysis of Rust, Go, and TypeScript:

1. **Fix conflicts**: How to handle overlapping fixes?
   - **Answer: Use Go's three-way merge approach**
   - Treat each fix as independent change to baseline
   - Attempt merge; if conflict, discard conflicting fix
   - User can re-run to apply remaining fixes
   - Never corrupt files with partial writes

2. **Multi-file fixes**: Should fixes span multiple files?
   - **Answer: Start single-file, design for multi-file**
   - TypeScript's `FileTextChanges[]` supports multiple files
   - Rust's multipart suggestions are single-file
   - Go explicitly requires all edits in same file
   - **Phase 1:** Single-file only
   - **Phase 2:** Multi-file for rename refactoring

3. **Undo mechanism**: Should `--fix` create backups?
   - **Answer: Rely on git, no backups**
   - Go doesn't create backups
   - Rust/TypeScript don't create backups
   - Modern workflows assume version control
   - Atomic writes prevent corruption

4. **IDE protocol**: Should we support LSP directly?
   - **Answer: JSON first, LSP later**
   - TypeScript's JSON is LSP-compatible
   - JSON output enables AI agents (primary use case)
   - LSP can be built on top of JSON layer
   - **Phase 1:** `--json` flag
   - **Phase 2:** LSP server (optional)

5. **Registration pattern**: How should fixes be organized?
   - **Answer: Self-registering modules (TypeScript pattern)**
   - Each fix in its own file
   - Registers at module load time
   - No central registry to maintain
   - Easy to add new fixes

## Reference: How Rust Does It

Analysis of `~/lang_repos/rust/compiler/rustc_errors/` reveals a mature, well-tested approach.

### Rust's Applicability Enum

**Location:** `compiler/rustc_lint_defs/src/lib.rs`

```rust
pub enum Applicability {
    /// The suggestion is definitely what the user intended, or maintains the
    /// exact meaning of the code. This suggestion should be automatically applied.
    MachineApplicable,

    /// The suggestion may be what the user intended, but it is uncertain.
    /// The suggestion should result in valid Rust code if it is applied.
    MaybeIncorrect,

    /// The suggestion contains placeholders like `(...)` or `{ /* fields */ }`.
    /// The suggestion cannot be applied automatically because it will not result
    /// in valid Rust code. The user will need to fill in the placeholders.
    HasPlaceholders,

    /// The applicability of the suggestion is unknown.
    Unspecified,
}
```

**Key insight:** Rust distinguishes between "uncertain but valid code" (`MaybeIncorrect`) and "has placeholders" (`HasPlaceholders`). Both are not auto-applied, but for different reasons.

### Rust's Suggestion Structure

**Location:** `compiler/rustc_errors/src/lib.rs`

```rust
pub struct CodeSuggestion {
    pub substitutions: Vec<Substitution>,  // Multiple alternatives
    pub msg: DiagMessage,
    pub style: SuggestionStyle,            // How to render
    pub applicability: Applicability,
}

pub struct Substitution {
    pub parts: Vec<SubstitutionPart>,      // Multipart edits
}

pub struct SubstitutionPart {
    pub span: Span,
    pub snippet: String,
}
```

**Key patterns:**
1. **Multiple substitutions** - Offer alternative fixes for the same error
2. **Multipart suggestions** - One fix can edit multiple locations
3. **Style control** - `HideCodeInline`, `ShowCode`, `CompletelyHidden` (tool-only)

### Rust's Diagnostic API

```rust
// Single span, single suggestion
err.span_suggestion(span, "use this instead", "replacement", Applicability::MachineApplicable);

// Single span, multiple alternatives
err.span_suggestions(span, "try one of these", vec!["option1", "option2"], Applicability::MaybeIncorrect);

// Multiple spans, one suggestion
err.multipart_suggestion("rename everywhere", vec![
    (span1, "new_name"),
    (span2, "new_name"),
], Applicability::MachineApplicable);

// Tool-only (hidden from CLI, for rustfix)
err.tool_only_multipart_suggestion("internal fix", edits, Applicability::MachineApplicable);
```

### Rust's JSON Output

**Location:** `compiler/rustc_errors/src/json.rs`

```rust
struct DiagnosticSpan {
    file_name: String,
    byte_start: u32,
    byte_end: u32,
    line_start: usize,
    line_end: usize,
    column_start: usize,
    column_end: usize,
    is_primary: bool,
    text: Vec<DiagnosticSpanLine>,
    label: Option<String>,
    suggested_replacement: Option<String>,       // THE FIX
    suggestion_applicability: Option<Applicability>,  // THE CONFIDENCE
}
```

**rustfix integration:** External tool reads JSON, filters by `suggestion_applicability == MachineApplicable`, applies `suggested_replacement` at the byte ranges.

### What Sigil Should Adopt from Rust

| Rust Feature | Sigil Adaptation |
|--------------|------------------|
| `Applicability` enum | Use `Confidence` with similar semantics |
| `HasPlaceholders` level | Add placeholder support (e.g., `???` in fixes) |
| Multiple substitutions | Support alternative fixes |
| Multipart suggestions | Support multi-edit fixes |
| `tool_only_*` methods | Add `style: Hidden` for tool-only fixes |
| JSON span format | Include both byte offsets AND line/column |
| Deduplication | Hash diagnostics to prevent duplicates |

### Sigil-Specific Additions

Rust's system is designed for a complex language with macros, lifetimes, and trait resolution. Sigil's simpler, stricter rules allow:

1. **Higher confidence baseline** - Sigil's strict formatting means more fixes are `Certain`
2. **Pattern-specific fixes** - Pattern validation knows exactly what args are valid
3. **No macro complexity** - No need to filter suggestions from macro expansions
4. **AI-first output** - JSON output designed for agent consumption, not just IDE integration

## Reference: How Go Does It

Analysis of `~/lang_repos/golang/` reveals a different philosophy: separation between compiler errors and analysis-driven fixes.

### Go's Diagnostic Structure

**Location:** `src/cmd/vendor/golang.org/x/tools/go/analysis/diagnostic.go`

```go
type Diagnostic struct {
    Pos            token.Pos
    End            token.Pos          // optional
    Category       string             // optional categorization
    Message        string
    URL            string             // documentation link
    SuggestedFixes []SuggestedFix     // THE FIXES
    Related        []RelatedInformation
}

type SuggestedFix struct {
    Message   string       // "Fix the foo problem"
    TextEdits []TextEdit
}

type TextEdit struct {
    Pos     token.Pos
    End     token.Pos
    NewText []byte
}
```

### Go's Two-Tool Philosophy

Go separates "safe" fixes from "review required" warnings:

| Tool | Purpose | Applicability |
|------|---------|---------------|
| `go fix` | Automated code modernization | **Always safe** - no user review needed |
| `go vet` | Static analysis warnings | **May need review** - not auto-applied |

**`go fix` philosophy:** Only includes fixes that are "unambiguously safe". If there's any doubt, it's not in `go fix`.

### Go's Three-Way Merge for Fixes

**Location:** `src/internal/analysis/driverutil/fix.go`

Go treats fixes as **independent changes to a baseline file**, merged like git:

```
Fix1: [original, original, changed]
Fix2: [original, changed, original]
                ↓
Merged: [original, changed, changed]  // if no conflict
```

**Conflict handling:**
- Conflicting fixes are **discarded** (not partially applied)
- User can re-run tool to apply remaining fixes
- No partial file writes - atomic success or failure

### Go's Analyzer Framework

```go
type Analyzer struct {
    Name             string
    Doc              string
    URL              string                    // Documentation link
    Run              func(*Pass) (any, error)
    Requires         []*Analyzer               // Dependencies
    FactTypes        []Fact                    // Cross-package facts
}

type Pass struct {
    Report func(Diagnostic)    // Report a diagnostic with fixes
    // ... type info, AST, etc.
}
```

**Key insight:** Analyzers are composable plugins. Each analyzer is self-contained and declares its dependencies.

### What Sigil Should Adopt from Go

| Go Feature | Sigil Adaptation |
|------------|------------------|
| Two-tool philosophy | `sigil fix` (safe) vs `sigil check` (warnings) |
| Three-way merge | Handle overlapping fixes gracefully |
| Analyzer plugins | Future: pluggable lint rules |
| Documentation URLs | Error codes link to docs |
| Related information | Secondary spans for context |
| Atomic file writes | No partial corruption on failure |

## Reference: How TypeScript Does It

Analysis of `~/lang_repos/typescript/src/services/` reveals a registration-based system designed for IDE integration.

### TypeScript's Diagnostic Structure

**Location:** `src/compiler/types.ts`

```typescript
interface Diagnostic {
    category: DiagnosticCategory;  // Warning, Error, Suggestion, Message
    code: number;                   // Numeric error code
    file: SourceFile | undefined;
    start: number | undefined;
    length: number | undefined;
    messageText: string | DiagnosticMessageChain;
    relatedInformation?: DiagnosticRelatedInformation[];
}

enum DiagnosticCategory {
    Warning = 0,
    Error = 1,
    Suggestion = 2,
    Message = 3,
}
```

### TypeScript's Code Fix Registration

**Location:** `src/services/codeFixProvider.ts`

TypeScript uses a **registration pattern** - fixes register themselves at module load:

```typescript
// Global registries
const errorCodeToFixes = createMultiMap<string, CodeFixRegistration>();
const fixIdToRegistration = new Map<string, CodeFixRegistration>();

// Each fix file registers itself
registerCodeFix({
    errorCodes: [Diagnostics.Cannot_find_name_0.code],
    getCodeActions: (context) => {
        // Return fixes or undefined if not applicable
    },
    fixIds: ["fixMissingImport"],
    getAllCodeActions: (context) => {
        // For "Fix All" functionality
    }
});
```

**Key insight:** 73 code fixes, each in its own file, self-registering. No central registry to maintain.

### TypeScript's Implicit Applicability

TypeScript doesn't explicitly model confidence levels. Instead:

```typescript
getCodeActions(context): CodeFixAction[] | undefined {
    // Return undefined = not applicable
    // Return [] = not applicable
    // Return [action] = applicable

    const info = getInfo(context.sourceFile, context.span.start);
    if (!info) return undefined;  // Early return = not applicable

    // Multiple options returned as array
    return [
        createCodeFixAction("option1", ...),
        createCodeFixAction("option2", ...),
    ];
}
```

**Confidence is implicit:**
- Applicable = returns action(s)
- Not applicable = returns undefined
- Multiple options = user chooses

### TypeScript's Fix-All Pattern

```typescript
registerCodeFix({
    errorCodes: [...],
    getCodeActions(context) {
        return [createCodeFixAction(
            fixName,
            changes,
            Diagnostics.Fix_description,
            "fixId",                              // Enables fix-all
            Diagnostics.Fix_all_description       // "Fix all X in file"
        )];
    },
    fixIds: ["fixId"],
    getAllCodeActions(context) {
        const seen = new Set<string>();           // Deduplication
        return codeFixAll(context, errorCodes, (changes, diag) => {
            const info = getInfo(diag.file, diag.start);
            if (info && addToSeen(seen, info.id)) {
                doChange(changes, info);
            }
        });
    }
});
```

### TypeScript's ChangeTracker

**Location:** `src/services/textChanges.ts`

```typescript
const changes = textChanges.ChangeTracker.with(context, tracker => {
    tracker.delete(sourceFile, node);
    tracker.replaceNode(sourceFile, oldNode, newNode);
    tracker.insertText(sourceFile, pos, "const ");
});
// Returns: FileTextChanges[]
```

### What Sigil Should Adopt from TypeScript

| TypeScript Feature | Sigil Adaptation |
|--------------------|------------------|
| Registration pattern | Self-registering fix modules |
| Error code → fix multimap | Fast lookup by error code |
| Fix-all with deduplication | `--fix` applies to all matching errors |
| ChangeTracker pattern | Builder API for constructing edits |
| Multiple options per error | Return alternatives, user/agent chooses |
| Implicit applicability | Simple: returns fix or doesn't |

## Synthesis: Best Practices Across Languages

| Aspect | Rust | Go | TypeScript | **Sigil Recommendation** |
|--------|------|----|-----------|-----------------------|
| **Confidence levels** | 4 explicit levels | Implicit (tool choice) | Implicit (return/don't) | **4 explicit levels** (Rust model) |
| **Fix structure** | Multipart edits | TextEdit list | ChangeTracker | **Multipart edits** |
| **Registration** | Inline in compiler | Analyzer plugins | Self-registering modules | **Self-registering** (TypeScript) |
| **Conflict handling** | N/A (single file) | Three-way merge | N/A | **Three-way merge** (Go model) |
| **Fix-all** | Via rustfix tool | Via analyzer | Built-in with dedup | **Built-in with dedup** |
| **Output format** | JSON with spans | JSON with positions | LSP protocol | **JSON + LSP-compatible** |

## References

- Rust compiler diagnostics: https://doc.rust-lang.org/rustc/
- Rust source: `~/lang_repos/rust/compiler/rustc_errors/`
- Go analysis framework: https://pkg.go.dev/golang.org/x/tools/go/analysis
- Go source: `~/lang_repos/golang/src/cmd/vendor/golang.org/x/tools/go/analysis/`
- TypeScript code fixes: https://github.com/microsoft/TypeScript/wiki/Writing-a-Language-Service-Plugin
- TypeScript source: `~/lang_repos/typescript/src/services/codefixes/`
- rust-analyzer fix actions: https://rust-analyzer.github.io/
- ESLint --fix: https://eslint.org/docs/user-guide/command-line-interface#fixing-problems
