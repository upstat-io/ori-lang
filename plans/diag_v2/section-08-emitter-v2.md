---
section: "08"
title: Terminal Emitter V2
status: not-started
goal: Upgrade terminal rendering to support ori_doc trees, type diffs, explanation chains, and related information
sections:
  - id: "08.1"
    title: Rich Message Rendering
    status: not-started
  - id: "08.2"
    title: Type Diff Display
    status: not-started
  - id: "08.3"
    title: Chain and Related Rendering
    status: not-started
  - id: "08.4"
    title: Enhanced Summary
    status: not-started
  - id: "08.5"
    title: Completion Checklist
    status: not-started
---

# Section 08: Terminal Emitter V2

**Status:** Not Started
**Goal:** Upgrade the terminal emitter to render all V2 diagnostic features: `ori_doc` trees with semantic annotations, structural type diffs, explanation chains, and related information blocks. The emitter should produce output quality comparable to Rust's error messages but with Elm's conversational tone.

**Reference compilers:**
- **Rust** `compiler/rustc_errors/src/emitter.rs` — Rich terminal output with multi-line snippets, suggestion rendering, `--color` control
- **Elm** `compiler/src/Reporting/Render/Code.hs` — Conversational error rendering with aligned source snippets and colored type highlights
- **Roc** `crates/reporting/src/report.rs` — `Palette` pattern rendering `RocDoc` trees to ANSI terminal

**Current state:** `ori_diagnostic/src/emitter/terminal.rs` (~600 lines) renders Rust-style diagnostics with:
- ANSI color codes (red=error, yellow=warning, cyan=note, green=help)
- Source snippets with `^` underlines for primary and `-` for secondary spans
- Cross-file labels with `::: path` notation
- Line numbers in left gutter

What's missing: rendering for `Doc` trees with annotations, type diff alignment, explanation chain indentation, and related information blocks.

---

## 08.1 Rich Message Rendering

### Doc Tree Rendering

When a diagnostic has a `rich_message: Option<Doc>`, the emitter renders it with the `AnsiPalette` instead of the plain `message` string:

```rust
impl<W: Write> TerminalEmitter<W> {
    fn emit_message(&mut self, diagnostic: &Diagnostic) -> io::Result<()> {
        // Render severity + error code header
        self.emit_header(diagnostic)?;

        // Choose rich or plain message
        if let Some(ref doc) = diagnostic.rich_message {
            if self.color_enabled {
                let palette = AnsiPalette { color_enabled: true };
                write!(self.writer, "{}", doc.render(&palette))?;
            } else {
                write!(self.writer, "{}", doc.to_plain_text())?;
            }
        } else {
            write!(self.writer, "{}", diagnostic.message)?;
        }

        writeln!(self.writer)?;
        Ok(())
    }
}
```

### ANSI Color Mapping

The `AnsiPalette` maps each `Annotation` to ANSI escape sequences:

```rust
impl Palette for AnsiPalette {
    fn begin_annotation(&self, ann: Annotation, out: &mut String) {
        if !self.color_enabled { return; }
        match ann {
            Annotation::TypeName     => out.push_str("\x1b[1;36m"), // Bold cyan
            Annotation::TypeVariable => out.push_str("\x1b[3;36m"), // Italic cyan
            Annotation::Keyword      => out.push_str("\x1b[1m"),    // Bold
            Annotation::Ident        => out.push_str("\x1b[1;37m"), // Bold white
            Annotation::Emphasis     => out.push_str("\x1b[1m"),    // Bold
            Annotation::Operator     => out.push_str("\x1b[33m"),   // Yellow
            Annotation::Literal      => out.push_str("\x1b[32m"),   // Green
            Annotation::Module       => out.push_str("\x1b[4m"),    // Underline
            Annotation::Suggestion   => out.push_str("\x1b[1;32m"), // Bold green
            Annotation::Expected     => out.push_str("\x1b[1;36m"), // Bold cyan
            Annotation::Found        => out.push_str("\x1b[1;31m"), // Bold red
            Annotation::DiffAdd      => out.push_str("\x1b[42m"),   // Green background
            Annotation::DiffRemove   => out.push_str("\x1b[41m"),   // Red background
            Annotation::Url          => out.push_str("\x1b[4;34m"), // Underline blue
            Annotation::ErrorCode    => out.push_str("\x1b[1;33m"), // Bold yellow
        }
    }

    fn end_annotation(&self, _ann: Annotation, out: &mut String) {
        if !self.color_enabled { return; }
        out.push_str("\x1b[0m"); // Reset
    }
}
```

### Color Accessibility

Support `NO_COLOR` environment variable (de-facto standard) and `--color=never` CLI flag. The existing `ColorMode` enum (Auto/Always/Never) already handles this — just ensure `NO_COLOR` is checked during `Auto` detection.

- [ ] Implement `emit_message()` with rich/plain fallback
- [ ] Implement `AnsiPalette` with all annotation mappings
- [ ] Support `NO_COLOR` environment variable
- [ ] Tests: rich message renders with ANSI codes when color enabled
- [ ] Tests: rich message falls back to plain text when color disabled
- [ ] Tests: `NO_COLOR` environment variable respected

---

## 08.2 Type Diff Display

### Aligned Diff Rendering

When a diagnostic contains a type diff (via `rich_message` with `DiffAdd`/`DiffRemove` annotations), render an aligned comparison:

```
   expected: fn(int, str, bool)  -> Result(Data, Error)
   found:    fn(int, str, float) -> Result(Data, Error)
                          ^^^^^
```

The alignment algorithm:
1. Render both expected and found doc trees to plain text (for length computation)
2. Find the maximum width of each aligned segment
3. Pad shorter segments with spaces for alignment
4. Render with annotations (color highlighting on divergent parts)
5. Add `^` underline on the found line under divergent parts

```rust
fn emit_type_diff(
    &mut self,
    expected_doc: &Doc,
    found_doc: &Doc,
) -> io::Result<()> {
    let palette = &self.palette();

    // Render to annotated strings
    let expected_str = expected_doc.render(palette);
    let found_str = found_doc.render(palette);

    // Compute alignment (using plain text for width)
    let expected_plain = expected_doc.to_plain_text();
    let found_plain = found_doc.to_plain_text();

    let expected_width = expected_plain.len();
    let found_width = found_plain.len();
    let max_width = expected_width.max(found_width);

    // Render with alignment
    writeln!(self.writer, "   expected: {}", expected_str)?;
    writeln!(self.writer, "   found:    {}", found_str)?;

    // Underline divergent parts
    let underline = compute_diff_underline(&expected_plain, &found_plain);
    if !underline.is_empty() {
        writeln!(self.writer, "             {}", underline)?;
    }

    Ok(())
}

/// Compute a `^` underline string highlighting character positions
/// where expected and found differ.
fn compute_diff_underline(expected: &str, found: &str) -> String {
    let mut underline = String::new();
    let max_len = expected.len().max(found.len());

    let e_chars: Vec<char> = expected.chars().collect();
    let f_chars: Vec<char> = found.chars().collect();

    let mut in_diff = false;
    for i in 0..max_len {
        let e = e_chars.get(i);
        let f = f_chars.get(i);
        if e != f {
            underline.push('^');
            in_diff = true;
        } else if in_diff {
            underline.push(' ');
        } else {
            underline.push(' ');
        }
    }

    underline.trim_end().to_string()
}
```

- [ ] Implement `emit_type_diff()` with alignment
- [ ] Implement `compute_diff_underline()`
- [ ] Handle multi-line type representations
- [ ] Tests: aligned diff output for various type pairs
- [ ] Tests: underline correctly marks divergent positions

---

## 08.3 Chain and Related Rendering

### ExplanationChain Rendering

Render chains with indented "because:" prefixes after the note/label section:

```rust
fn emit_explanation_chain(
    &mut self,
    chain: &ExplanationChain,
    depth: u16,
) -> io::Result<()> {
    if depth > 4 {
        writeln!(self.writer, "{}  = ... ({} more reasons)",
            " ".repeat(depth as usize * 2),
            chain.depth() - 1)?;
        return Ok(());
    }

    let indent = " ".repeat(depth as usize * 2);
    let prefix = if depth == 0 { "   = because: " } else { &format!("{indent}  = because: ") };

    write!(self.writer, "{}{}", prefix, chain.message)?;
    writeln!(self.writer)?;

    // If this link has a span and we have source, show the snippet
    if let Some(span) = chain.span {
        self.emit_chain_snippet(span, chain.source_info.as_ref(), depth)?;
    }

    // Recurse into children
    for child in &chain.children {
        self.emit_explanation_chain(child, depth + 1)?;
    }

    Ok(())
}
```

### RelatedInformation Rendering

Render as separate "related:" blocks after the main diagnostic:

```rust
fn emit_related_information(
    &mut self,
    related: &[RelatedInformation],
) -> io::Result<()> {
    for info in related {
        writeln!(self.writer)?;
        self.emit_color("  related: ", Color::Cyan)?;
        writeln!(self.writer, "{}", info.message)?;

        if let Some(ref source_info) = info.source_info {
            // Cross-file: show file path and snippet
            writeln!(self.writer, "    --> {}", source_info.path)?;
            if !source_info.content.is_empty() {
                self.emit_snippet_from_source(
                    &source_info.content,
                    info.span,
                )?;
            }
        } else {
            // Same file: show snippet from current source
            self.emit_snippet(info.span)?;
        }
    }
    Ok(())
}
```

### Rendering Order

The complete rendering order for a diagnostic:

```
1. Header:    error[E2001]: type mismatch
2. Location:  --> file:line:col
3. Snippet:   source code with underlines
4. Notes:     = note: ...
5. Chains:    = because: ...
6. Related:   related: ...
7. Suggest:   = help: ...
8. Fix:       fix available: ...
```

- [ ] Implement `emit_explanation_chain()` with depth limiting
- [ ] Implement `emit_chain_snippet()` for chain links with spans
- [ ] Implement `emit_related_information()` with cross-file support
- [ ] Define rendering order (notes → chains → related → suggestions)
- [ ] Tests: chain rendering at various depths
- [ ] Tests: related info rendering (same-file and cross-file)
- [ ] Tests: depth cap at 4 with "... (N more)" truncation

---

## 08.4 Enhanced Summary

### Fix-Aware Summary

Update the summary line to include fixable error counts:

```
error: 3 errors, 1 warning (2 fixable with `ori check --fix`)
```

```rust
fn emit_summary(
    &mut self,
    error_count: usize,
    warning_count: usize,
    fixable_count: usize,
) -> io::Result<()> {
    // Build summary parts
    let mut parts = Vec::new();
    if error_count > 0 {
        parts.push(format!("{error_count} error{}", if error_count == 1 { "" } else { "s" }));
    }
    if warning_count > 0 {
        parts.push(format!("{warning_count} warning{}", if warning_count == 1 { "" } else { "s" }));
    }

    self.emit_color("error", Color::Red)?;
    write!(self.writer, ": {}", parts.join(", "))?;

    if fixable_count > 0 {
        write!(self.writer, " ({fixable_count} fixable with ")?;
        self.emit_color("`ori check --fix`", Color::Green)?;
        write!(self.writer, ")")?;
    }

    writeln!(self.writer)?;
    Ok(())
}
```

### Error Code Explanation Hint

For the first error in output, show a hint about `ori explain`:

```
   = for more information about this error, try `ori explain E2001`
```

Only show this hint for the first error to avoid noise.

- [ ] Update `emit_summary()` with fixable count
- [ ] Add `ori explain` hint for first error
- [ ] Tests: summary with various error/warning/fixable counts
- [ ] Tests: explain hint appears only for first error

---

## 08.5 Completion Checklist

- [ ] `AnsiPalette` implemented with all 15 annotation mappings
- [ ] Rich message rendering with Doc tree support
- [ ] Type diff alignment with underline markers
- [ ] Explanation chain rendering with depth limiting
- [ ] Related information rendering (same-file + cross-file)
- [ ] Enhanced summary with fixable count
- [ ] `ori explain` hint
- [ ] `NO_COLOR` environment variable support
- [ ] Rendering order: header → location → snippet → notes → chains → related → help → fix
- [ ] Tests: 20+ rendering tests covering all new features
- [ ] Tests: color disabled produces clean plain text
- [ ] `./test-all.sh` passes

**Exit Criteria:** The terminal emitter renders all V2 diagnostic features. Type mismatches show aligned diffs with highlighted differences. Explanation chains provide indented "because..." reasoning. Related information references cross-file locations. The summary reports fixable error counts.
