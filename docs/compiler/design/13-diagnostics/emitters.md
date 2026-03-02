---
title: "Emitters"
description: "Ori Compiler Design — Diagnostic Output Formats"
order: 1303
section: "Diagnostics"
---

# Emitters

## What Are Diagnostic Emitters?

A compiler diagnostic is a structured data object: a severity level, an error code, a message, a set of labeled source spans, optional notes, and optional fix suggestions. The question of how to render that object — what it looks like when a user sees it — is entirely separate from the question of what it contains.

That separation is the rendering problem. Three approaches dominate compiler design:

**Direct printing** embeds rendering code inside the diagnostic object itself. Each diagnostic knows how to print itself. This is simple to implement and adequate for small compilers, but it couples data representation to presentation. Adding a JSON output format means touching every diagnostic type. Testing rendering requires constructing full diagnostic objects.

**Visitor pattern** decouples rendering by passing a renderer into the diagnostic, which then calls back into the renderer for each field. This is flexible but verbose: every diagnostic type must implement the visitor protocol, and the protocol tends to grow over time as new fields are added.

**Trait-based polymorphism** defines a rendering interface as a trait. Concrete implementations — one per output format — handle all rendering for that format. The diagnostic type itself is format-agnostic; emitter implementations are diagnostic-agnostic. This is Ori's approach.

The three output targets serve distinct audiences:

| Format | Primary Consumers |
|--------|------------------|
| Terminal | Humans running `ori check` in a shell |
| JSON | IDE extensions, CI scripts, programmatic tooling |
| SARIF | Static analysis platforms — GitHub Advanced Security, Azure DevOps, VS Code SARIF Viewer |

Each format has different structural requirements. Terminal output must be readable at a glance and fit within 80–100 columns. JSON must be machine-parseable and self-describing. SARIF must conform to an OASIS standard schema and support features like rule catalogs and fix descriptions.

---

## The DiagnosticEmitter Trait

The trait is defined in `compiler/ori_diagnostic/src/emitter/mod.rs`:

```rust
/// Trait for emitting diagnostics in various formats.
pub trait DiagnosticEmitter {
    /// Emit a single diagnostic.
    fn emit(&mut self, diagnostic: &Diagnostic);

    /// Emit multiple diagnostics.
    fn emit_all(&mut self, diagnostics: &[Diagnostic]) {
        for diag in diagnostics {
            self.emit(diag);
        }
    }

    /// Flush any buffered output.
    fn flush(&mut self);

    /// Emit a summary of errors and warnings.
    fn emit_summary(&mut self, error_count: usize, warning_count: usize);
}
```

`emit` is the core method. It takes a shared reference to one `Diagnostic` and produces output — to a terminal, a buffer, or an in-memory accumulator, depending on the implementation. The `&mut self` receiver is necessary because most emitters maintain state (write position, item count, accumulated results).

`emit_all` has a default implementation that loops over a slice. Emitters may override it if batch processing enables optimizations, but in practice the default is sufficient.

`flush` exists because not all emitters write output immediately. The **terminal emitter** writes each diagnostic as it arrives and uses `flush` to flush the underlying writer's I/O buffers. The **SARIF emitter** accumulates results internally and writes nothing until `finish()` is called — `flush` merely flushes the writer after the document is complete. Without `flush`, buffered output might be lost at process exit.

`emit_summary` is separate from `flush` because different formats want to express totals differently. The terminal emitter writes a human-readable line — `"error: aborting due to 3 previous errors; 1 warning emitted"` — with appropriate pluralization and color. The JSON and SARIF emitters ignore the summary entirely: the data is self-describing, and callers can count items themselves. Keeping `emit_summary` as a trait method allows each emitter to handle it appropriately without an `if format == terminal` branch in the caller.

The module also provides two shared helpers:

```rust
/// Returns a trailing comma for JSON/SARIF list serialization.
pub(crate) fn trailing_comma(index: usize, total: usize) -> &'static str;

/// Escape a string for JSON output.
pub(crate) fn escape_json(s: &str) -> String;
```

`escape_json` handles `"`, `\`, `\n`, `\r`, `\t`, and arbitrary control characters via `\uXXXX`. It is used by both the JSON and SARIF emitters, which both produce JSON-encoded text.

---

## Terminal Emitter

The **terminal emitter** produces human-readable output for shell users. It is defined in `compiler/ori_diagnostic/src/emitter/terminal/mod.rs`.

### Construction

```rust
pub struct TerminalEmitter<'src, W: Write> {
    writer: W,
    colors: bool,
    source: Option<&'src str>,
    file_path: Option<String>,
    line_table: Option<LineOffsetTable>,
}
```

The generic parameter `W: Write` allows any writer — `io::Stdout`, `io::Stderr`, or `Vec<u8>` for tests. The `'src` lifetime borrows the source text without cloning it: one full source allocation is eliminated per compile.

Construction uses a builder pattern:

```rust
// Construct targeting stderr with automatic color detection
let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
let mut emitter = TerminalEmitter::with_color_mode(io::stderr(), ColorMode::Auto, is_tty)
    .with_source(file.text(&db).as_str())
    .with_file_path(path);
```

`with_source` does more than store a reference — it eagerly builds a `LineOffsetTable` from the source. The table is an array of byte offsets marking the start of each line, built in O(n) time once. Subsequent span-to-line-column lookups use binary search: O(log L) instead of O(n), where L is the line count. For files with many diagnostics, this difference is material.

`with_file_path` stores the path string displayed in location headers (`--> src/main.ori:10:5`). Without it, the header shows `<unknown>`.

Three factory methods cover the common cases: `with_color_mode(writer, mode, is_tty)` for any writer, `stdout(mode, is_tty)`, and `stderr(mode, is_tty)`.

### ColorMode

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorMode {
    #[default]
    Auto,    // Enable colors iff the output is a TTY
    Always,  // Always enable colors
    Never,   // Never enable colors
}

impl ColorMode {
    pub fn should_use_colors(self, is_tty: bool) -> bool {
        match self {
            ColorMode::Auto => is_tty,
            ColorMode::Always => true,
            ColorMode::Never => false,
        }
    }
}
```

`Auto` is the default. The `is_tty` flag is passed in from the CLI layer, which calls `std::io::IsTerminal::is_terminal()` on the target stream. This separation keeps the emitter free of I/O detection: the emitter itself is agnostic to whether it is writing to a pipe, a file, or a terminal.

### Color Scheme

Colors are defined as module-level constants in a private `colors` submodule — not as an external dependency:

```rust
mod colors {
    pub const ERROR: &str    = "\x1b[1;31m"; // Bold red
    pub const WARNING: &str  = "\x1b[1;33m"; // Bold yellow
    pub const NOTE: &str     = "\x1b[1;36m"; // Bold cyan
    pub const HELP: &str     = "\x1b[1;32m"; // Bold green
    pub const BOLD: &str     = "\x1b[1m";
    pub const SECONDARY: &str = "\x1b[1;34m"; // Bold blue
    pub const RESET: &str    = "\x1b[0m";
}
```

Raw ANSI escape sequences are used rather than a color library such as `colored` or `termcolor`. This eliminates a dependency, keeps the emitter self-contained, and gives precise control over the exact sequences emitted.

### Rendering Pipeline

When `emit` is called with source text available, it follows this sequence:

**1. Header.** The severity (colored by level) followed by the error code in brackets and the message:

```
error[E2001]: type mismatch: expected `int`, found `str`
```

**2. Label separation.** Labels are partitioned into same-file and cross-file groups. Cross-file labels carry a `SourceInfo` struct with a path and content string for their file.

**3. Gutter width.** The maximum line number across all same-file labels determines how many digits are needed to display line numbers. A 10-line file uses a 2-character gutter; a 999-line file uses a 3-character gutter. All gutters are right-aligned to this width.

**4. Location header.** The first primary label (or the first label if none is primary) provides the canonical file, line, and column for the `-->` arrow:

```
 --> src/main.ori:10:15
```

**5. Same-file labels.** Labels are sorted by `span.start`. Single-line labels are grouped by line number. For each group:
- An empty gutter line separates non-consecutive groups
- The source line is printed with its line number
- Each label's underline is printed below the source: `^` characters for primary spans, `-` characters for secondary spans, in red and blue respectively
- If multiple labels fall on the same line, their underlines are emitted in column order, leftmost first

**6. Multi-line spans.** Labels whose start and end lines differ use a different rendering: a `/` marker on the opening line, `|` continuation markers on intermediate lines, and an underline on the closing line. When a span crosses more than four lines, intermediate lines are elided with `| ...` rather than printing all of them.

**7. Cross-file labels.** Each cross-file label is rendered with a `::: path:line:col` header and a snippet from `SourceInfo.content`. A temporary `LineOffsetTable` is built from the cross-file content for accurate column computation:

```
  ::: src/lib.ori:25:1
   |
25 | @get_name () -> str
   | ------------------- return type defined here
```

**8. Notes and suggestions.** Notes are rendered as `= note: ...` lines. Plain text suggestions are rendered as `= help: ...` lines. Structured suggestions (with spans and applicability) also render their message on a `= help: ...` line — the span substitutions are reserved for `ori fix`.

**Fallback path.** Without source text, the emitter falls back to byte-offset output: `-->  Span { start: 10, end: 15 }`. This is less readable but ensures every diagnostic produces some output regardless of whether source is available.

### Terminal Output Example

```
error[E2001]: type mismatch: expected `int`, found `str`
  --> src/main.ori:10:14
   |
10 |     let x: int = "hello"
   |            ---   ^^^^^^^ expected `int`, found `str`
   |            |
   |            expected due to this annotation
   |
  ::: src/lib.ori:25:1
   |
25 | @get_name () -> str
   | ------------------- return type defined here
   |
   = note: int and str are incompatible
   = help: consider using `int()` to convert
```

When colors are enabled, `error` is bold red, `[E2001]` is bold, gutter pipes and `-->` are bold blue, `^` underlines are bold red, and `-` underlines are bold blue.

### Summary

```rust
fn emit_summary(&mut self, error_count: usize, warning_count: usize) {
    // "error: aborting due to 3 previous errors; 1 warning emitted"
    // "error: aborting due to previous error"
    // "warning: 3 warnings emitted"
}
```

Pluralization is handled by the `plural_s` helper: `"s"` for counts other than one, `""` for exactly one. The word "previous" appears before "error"/"errors" to match rustc's phrasing, which users of other Rust-compiled tooling already recognize.

---

## JSON Emitter

The **JSON emitter** produces machine-readable output for tooling. It is defined in `compiler/ori_diagnostic/src/emitter/json/mod.rs`.

### Construction

```rust
pub struct JsonEmitter<W: Write> {
    writer: W,
    first: bool,   // Tracks whether a comma separator is needed
}

impl<W: Write> JsonEmitter<W> {
    pub fn new(writer: W) -> Self;
    pub fn begin(&mut self);   // Writes the opening `[`
    pub fn end(&mut self);     // Writes the closing `]`
}
```

The emitter streams — each `emit` call writes one JSON object to the writer immediately, without buffering all diagnostics first. `begin()` and `end()` delimit the surrounding array. The `first` flag tracks whether to emit a leading comma: the first object gets no comma; subsequent objects get one.

### Output Schema

Each diagnostic is serialized as a JSON object with this structure:

```json
{
  "code": "E2001",
  "severity": "Error",
  "message": "type mismatch: expected `int`, found `str`",
  "labels": [
    {
      "start": 150,
      "end": 157,
      "message": "expected `int`, found `str`",
      "primary": true,
      "cross_file": false
    },
    {
      "start": 0,
      "end": 19,
      "message": "return type defined here",
      "primary": false,
      "cross_file": true,
      "file": "src/lib.ori"
    }
  ],
  "notes": ["int and str are incompatible"],
  "suggestions": ["consider using `int()` to convert"],
  "structured_suggestions": [
    {
      "message": "convert using `int()`"
    }
  ]
}
```

Spans are emitted as raw byte offsets (`start`, `end`). Line and column numbers are not included in JSON output — tooling that needs line/column can compute them from the byte offsets and the source file, which the caller already has.

Cross-file labels include a `"file"` field with the path from `SourceInfo.path` and `"cross_file": true`. Same-file labels emit `"cross_file": false` with no `"file"` field.

**Design limitation.** The `structured_suggestions` array currently emits only the `message` field. The `substitutions` array (with span and snippet pairs for auto-apply) is omitted — that data is present in the `Suggestion` type but not yet serialized. Full substitution output is planned for LSP integration via `ori fix`.

### No serde Dependency

JSON is written manually using `write!` macros and the shared `escape_json` and `trailing_comma` helpers. This choice eliminates the `serde` and `serde_json` dependencies from `ori_diagnostic`. For the flat, well-known structure of diagnostic output, manual serialization is straightforward and produces no meaningful maintenance burden.

---

## SARIF Emitter

The **SARIF emitter** produces output in **Static Analysis Results Interchange Format**, an OASIS standard (version 2.1.0) for exchanging static analysis tool results. SARIF is supported by GitHub Advanced Security (code scanning), Azure DevOps, and the VS Code SARIF Viewer extension. It is defined in `compiler/ori_diagnostic/src/emitter/sarif/mod.rs`.

### SARIF Structure

A SARIF document contains one or more **runs**. Each run describes:

- A **tool** with a **driver** — the compiler name, version, and a catalog of **rules** (one per error code observed)
- A list of **results** — one per diagnostic

Each result has:

- `ruleId` — the error code string (e.g., `"E2001"`)
- `level` — severity: `"error"`, `"warning"`, or `"note"`
- `message` — the diagnostic message text
- `locations` — primary source locations
- `relatedLocations` — secondary labels, cross-file references
- `fixes` — structured fix suggestions (planned)

### Construction

```rust
pub struct SarifEmitter<'src, W: Write> {
    writer: W,
    tool_name: String,
    tool_version: String,
    artifact_uri: Option<String>,
    source: Option<&'src str>,
    line_table: Option<LineOffsetTable>,
    results: Vec<SarifResult>,
}

impl<'src, W: Write> SarifEmitter<'src, W> {
    pub fn new(writer: W, tool_name: impl Into<String>, tool_version: impl Into<String>) -> Self;
    pub fn with_artifact(self, uri: impl Into<String>) -> Self;
    pub fn with_source(self, source: &'src str) -> Self;
    pub fn finish(&mut self);
}
```

Like `TerminalEmitter`, `SarifEmitter` takes a `'src` lifetime borrow of the source text and builds a `LineOffsetTable` on `with_source`. The `with_artifact` method sets the default artifact URI — the file path used in `physicalLocation.artifactLocation.uri` for all same-file labels.

`finish()` writes the complete SARIF document. This is intentionally separate from `flush()`: `finish()` serializes the buffered results and writes the full document, while `flush()` only flushes the underlying writer's I/O buffer.

### Buffering

Unlike the JSON emitter, SARIF **buffers** all results before writing. This is required by the format: the rule catalog in `tool.driver.rules` must list every rule referenced by any result in the run. That means all results must be known before the document can be written — either emit everything first and scan for unique codes, or buffer results and emit rules at document-close time.

Ori buffers `SarifResult` objects (an internal struct, not the full diagnostic) and collects unique rule IDs using a `BTreeSet<&str>`. `BTreeSet` gives deterministic alphabetical ordering of rules without an explicit sort pass.

### Label Mapping

Primary labels map to `locations[]`. Secondary labels map to `relatedLocations[]`. Cross-file labels use the path from `SourceInfo.path` as the per-location artifact URI, overriding the default artifact URI set by `with_artifact`.

For cross-file labels, a temporary `LineOffsetTable` is built from `SourceInfo.content` to compute accurate line and column numbers. This mirrors the terminal emitter's approach.

Severity mapping:

| `Severity` | SARIF `level` |
|------------|--------------|
| `Error` | `"error"` |
| `Warning` | `"warning"` |
| `Note` | `"note"` |
| `Help` | `"note"` |

SARIF's `"none"` level (informational messages with no severity implication) is not used; `Help` maps to `"note"` as the closest equivalent.

### SARIF Output Example

```json
{
  "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
  "version": "2.1.0",
  "runs": [{
    "tool": {
      "driver": {
        "name": "oric",
        "version": "2026.03.01.1-Alpha",
        "rules": [
          { "id": "E2001" }
        ]
      }
    },
    "results": [
      {
        "ruleId": "E2001",
        "level": "error",
        "message": { "text": "type mismatch: expected `int`, found `str`" },
        "locations": [
          {
            "physicalLocation": {
              "artifactLocation": { "uri": "src/main.ori" },
              "region": {
                "startLine": 10,
                "startColumn": 14,
                "endLine": 10,
                "endColumn": 21
              }
            }
          }
        ],
        "relatedLocations": [
          {
            "id": 0,
            "physicalLocation": {
              "artifactLocation": { "uri": "src/lib.ori" },
              "region": {
                "startLine": 25,
                "startColumn": 1,
                "endLine": 25,
                "endColumn": 20
              }
            },
            "message": { "text": "return type defined here" }
          }
        ]
      }
    ]
  }]
}
```

---

## Choosing an Emitter

Emitters are instantiated directly at the CLI layer — there is no factory function yet. The pattern used throughout `oric` commands is:

```rust
let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
let mut emitter = TerminalEmitter::with_color_mode(io::stderr(), ColorMode::Auto, is_tty)
    .with_source(source_text)
    .with_file_path(path);
```

For JSON output, the caller constructs a `JsonEmitter`, calls `begin()`, loops over `emit()` calls, then calls `end()` and `flush()`. For SARIF, the caller constructs a `SarifEmitter`, calls `emit()` for each diagnostic, then calls `finish()`.

A `ColorMode::Never` terminal emitter, without colors, provides "plain" text output suitable for piped consumption by tools that want readable text without ANSI codes.

CLI integration follows the convention of other compilers:

```bash
# Terminal output (default, colors when TTY)
ori check src/main.ori

# Machine-readable JSON
ori check --format=json src/main.ori

# SARIF for CI integration
ori check --format=sarif src/main.ori > results.sarif
```

---

## Prior Art

**rustc** — The reference implementation for terminal diagnostic rendering. `HumanEmitter` (formerly `EmitterWriter`) handles multi-line spans, overlapping labels, and the Rust-style gutter format that Ori's terminal emitter closely follows. `JsonEmitter` emits one JSON object per diagnostic to stdout. `DiagCtxt` routes diagnostics to the registered emitter. rustc added SARIF support later, via a separate consumer.

**Clang** — Uses a `DiagnosticConsumer` class hierarchy. `TextDiagnosticPrinter` handles terminal output with caret-based underlines; `SerializedDiagnosticConsumer` writes a binary diagnostic format for Xcode integration. Clang added SARIF output as an additional consumer. The multi-consumer architecture (one diagnostic fan-out to multiple consumers simultaneously) differs from Ori's single-emitter model.

**TypeScript** — `diagnosticMessages.json` is the canonical catalog of all diagnostics, from which code is generated. `formatDiagnosticsWithColorAndContext` handles terminal rendering. TypeScript's language server protocol integration provides JSON-like structured diagnostics to IDEs via the LSP wire format rather than a separate JSON emitter.

**Elm** — Uses a `Doc`-based pretty printing system for terminal output. All output is always colored; there is no color mode selection. Elm has no JSON or SARIF output — the compiler is designed around a single human-readable output format, which is part of what enables its famously readable error messages. The document algebra makes complex multi-column layouts straightforward.

**GHC** — Uses `SDoc` (structured document) with `PprStyle` for different output modes: user-facing output, dump output, and debug output. The separation between document construction and rendering enables GHC's various output backends without changing the diagnostic construction sites.

---

## Design Tradeoffs

**Trait-based polymorphism vs enum dispatch.** The `DiagnosticEmitter` trait allows new emitter types to be added without modifying existing code. The cost is a vtable dispatch per `emit` call. An enum dispatch (`match format { Terminal => ..., Json => ..., Sarif => ... }`) would be marginally faster but requires modifying a central match arm to add a new format. For diagnostic output — which is not in a hot path — the trait approach is the right choice.

**Borrowing source text (`'src` lifetime) vs cloning.** The `'src` lifetime on `TerminalEmitter` and `SarifEmitter` allows the emitter to borrow source text rather than own a copy. This eliminates one full source file allocation per compile session. The tradeoff is a more complex type signature and the requirement that the source text outlive the emitter — which is always true in practice, since the emitter is created, used, and dropped within a single function scope.

**No serde for JSON.** The JSON emitter writes JSON manually. The diagnostic structure is flat and well-defined; manual serialization is ten lines of `write!` calls per field. Adding `serde` would pull in a significant compilation-time dependency across all crates that use `ori_diagnostic`. For the limited and stable structure of diagnostic JSON, manual serialization is preferable.

**SARIF buffering vs streaming.** SARIF requires the rule catalog to appear before results in the document, but rules can only be enumerated after all results are known. This forces buffering. An alternative — two-pass output — would require writing to a seekable output, which is not generally available (pipes are not seekable). Buffering `SarifResult` structs (small, no source text) is the practical solution.

**Raw ANSI escapes vs a color library.** Using `\x1b[1;31m` directly avoids any dependency. The color constants are defined once in `mod colors` and referenced by name. The tradeoff: platform-specific behavior (Windows console API vs ANSI) is not handled automatically. Ori targets Unix-first; Windows users running in terminals that support ANSI (Windows Terminal, VS Code integrated terminal, WSL) are covered by the raw sequences.

**Separate `emit_summary` vs implicit in `flush`.** Merging summary into `flush` would simplify the trait by one method, but it would require every `flush` call to know the error and warning counts — information that `flush` does not currently receive. The separate method keeps `flush` a pure I/O operation and gives callers explicit control over when and whether to emit a summary.

---

## Related Documents

- [Diagnostics Overview](index.md) — System architecture, `DiagnosticQueue`, `ErrorGuaranteed`
- [Problem Types](problem-types.md) — Phase-specific problem enums and their conversion to `Diagnostic`
- [Code Fixes](code-fixes.md) — `Suggestion`, `Substitution`, `Applicability`, and the `ori fix` pipeline
