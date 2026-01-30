---
title: "Implementation Overview"
description: "Ori Formatter Design — Implementation Approach"
order: 1
section: "Implementation"
---

# Implementation Overview

This document describes the implementation approach for the Ori formatter.

## Reference: Go's gofmt

The Ori formatter follows gofmt's philosophy: **zero-config, deterministic, implementation is the spec**.

Key techniques from gofmt that inform Ori's implementation:

| Technique | gofmt | Ori adaptation |
|-----------|-------|----------------|
| **Whitespace buffering** | Accumulates directives, flushes on content | Useful for comment interspersion |
| **Two-phase pipeline** | AST printer → tabwriter | Width calculator → formatter |
| **Idempotence** | Core guarantee | Core guarantee |

### Whitespace Buffering

gofmt doesn't emit whitespace immediately. Instead, it buffers formatting directives:

```
buffer = [indent, newline, space, ...]
```

Benefits:
- **Comment interspersion**: Can place comments relative to buffered whitespace
- **Deferred decisions**: Adjust whitespace before committing
- **No trailing whitespace**: Only flush when content is written

Ori may adopt this pattern for comment handling, though it's simpler since Ori prohibits inline comments.

## Architecture

The formatter operates on the parsed AST and produces formatted source text.

```
Source Text → Lexer → Parser → AST → Formatter → Formatted Text
```

### Key Components

| Component | Purpose |
|-----------|---------|
| `WidthCalculator` | Computes inline width of AST nodes |
| `Formatter` | Decides inline vs broken and emits output |
| `Emitter` | Produces output (string, file, etc.) |
| `CommentAttacher` | Associates comments with AST nodes |

## Width Calculation

### Bottom-Up Traversal

Calculate widths from leaves up. Cache results on AST nodes or in a side table.

```rust
fn calculate_width(node: &Expr, cache: &mut WidthCache) -> usize {
    if let Some(w) = cache.get(node.id()) {
        return w;
    }

    let width = match node {
        Expr::Literal(lit) => lit.text.len(),
        Expr::Identifier(name) => name.len(),
        Expr::Binary { left, op, right } => {
            calculate_width(left, cache)
                + 1 + op.len() + 1  // spaces around operator
                + calculate_width(right, cache)
        }
        Expr::Call { func, args } => {
            func.len()
                + 1  // opening paren
                + args.iter()
                    .map(|a| a.name.len() + 2 + calculate_width(&a.value, cache))
                    .sum::<usize>()
                + (args.len().saturating_sub(1)) * 2  // ", " separators
                + 1  // closing paren
        }
        // ... other cases
    };

    cache.insert(node.id(), width);
    width
}
```

### Width Constants

| Construct | Width Formula |
|-----------|---------------|
| Identifier | `name.len()` |
| Integer literal | `text.len()` |
| String literal | `text.len() + 2` (quotes) |
| Binary expr | `left + 1 + op + 1 + right` |
| Function call | `name + 1 + args_width + separators + 1` |
| Named argument | `name + 2 + value` (`: `) |

## Formatting Algorithm

### Top-Down Rendering

```rust
fn format(node: &Expr, ctx: &mut FormatContext) {
    let width = ctx.width_cache.get(node.id());

    if ctx.column + width <= 100 {
        emit_inline(node, ctx);
    } else {
        emit_broken(node, ctx);
    }
}

fn emit_inline(node: &Expr, ctx: &mut FormatContext) {
    match node {
        Expr::Binary { left, op, right } => {
            emit_inline(left, ctx);
            ctx.emit(" ");
            ctx.emit(op);
            ctx.emit(" ");
            emit_inline(right, ctx);
        }
        // ... other cases
    }
}

fn emit_broken(node: &Expr, ctx: &mut FormatContext) {
    match node {
        Expr::Binary { left, op, right } => {
            format(left, ctx);  // May be inline or broken
            ctx.emit_newline();
            ctx.emit_indent();
            ctx.emit(op);
            ctx.emit(" ");
            format(right, ctx);
        }
        // ... other cases
    }
}
```

### Context State

```rust
struct FormatContext {
    column: usize,           // Current column position
    indent_level: usize,     // Nesting depth (multiply by 4)
    width_cache: WidthCache,
    output: String,
}

impl FormatContext {
    fn emit(&mut self, text: &str) {
        self.output.push_str(text);
        self.column += text.len();
    }

    fn emit_newline(&mut self) {
        self.output.push('\n');
        self.column = 0;
    }

    fn emit_indent(&mut self) {
        let spaces = self.indent_level * 4;
        self.output.push_str(&" ".repeat(spaces));
        self.column = spaces;
    }

    fn with_indent<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.indent_level += 1;
        let result = f(self);
        self.indent_level -= 1;
        result
    }
}
```

## Always-Stacked Constructs

Some constructs bypass the width check and always use broken format:

```rust
fn format(node: &Expr, ctx: &mut FormatContext) {
    match node {
        Expr::Run { .. } | Expr::Try { .. } => {
            emit_stacked_run(node, ctx);  // Always stacked
        }
        Expr::Match { scrutinee, arms } => {
            emit_stacked_match(scrutinee, arms, ctx);  // Arms always stacked
        }
        _ => {
            // Normal width-based decision
            let width = ctx.width_cache.get(node.id());
            if ctx.column + width <= 100 {
                emit_inline(node, ctx);
            } else {
                emit_broken(node, ctx);
            }
        }
    }
}
```

## Comment Handling

### Comment Attachment

During parsing, comments are collected separately. Before formatting, attach comments to AST nodes:

```rust
struct CommentAttachment {
    leading: Vec<Comment>,   // Comments before the node
    trailing: Vec<Comment>,  // Comments after (same line - but Ori doesn't allow these)
}

fn attach_comments(ast: &Module, comments: &[Comment]) -> CommentMap {
    let mut map = CommentMap::new();

    for comment in comments {
        // Find the AST node that follows this comment
        let node = find_next_node(ast, comment.span.end);
        map.entry(node.id())
            .or_default()
            .leading
            .push(comment.clone());
    }

    map
}
```

### Emitting Comments

```rust
fn format_with_comments(node: &Expr, ctx: &mut FormatContext, comments: &CommentMap) {
    // Emit leading comments
    if let Some(attached) = comments.get(node.id()) {
        for comment in &attached.leading {
            emit_comment(comment, ctx);
            ctx.emit_newline();
            ctx.emit_indent();
        }
    }

    // Format the node itself
    format(node, ctx);
}

fn emit_comment(comment: &Comment, ctx: &mut FormatContext) {
    ctx.emit("// ");
    ctx.emit(&comment.text);
}
```

### Doc Comment Reordering

```rust
fn reorder_doc_comments(comments: &mut Vec<Comment>) {
    comments.sort_by_key(|c| doc_comment_order(&c.text));
}

fn doc_comment_order(text: &str) -> usize {
    if text.starts_with('#') { 1 }
    else if text.starts_with("@param") || text.starts_with("@field") { 2 }
    else if text.starts_with('!') { 3 }
    else if text.starts_with('>') { 4 }
    else { 0 }  // Non-doc comments come first
}
```

## List Wrapping Algorithm

For lists, the formatter fills as many items as fit per line:

```rust
fn emit_broken_list(items: &[Expr], ctx: &mut FormatContext) {
    ctx.emit("[");
    ctx.emit_newline();
    ctx.with_indent(|ctx| {
        ctx.emit_indent();

        for (i, item) in items.iter().enumerate() {
            let item_width = ctx.width_cache.get(item.id());

            // Check if item fits on current line
            if ctx.column + item_width + 1 > 100 && ctx.column > ctx.indent_level * 4 {
                // Doesn't fit - wrap to next line
                ctx.emit(",");
                ctx.emit_newline();
                ctx.emit_indent();
            } else if i > 0 {
                ctx.emit(", ");
            }

            emit_inline(item, ctx);
        }

        ctx.emit(",");  // Trailing comma
    });
    ctx.emit_newline();
    ctx.emit_indent();
    ctx.emit("]");
}
```

## Error Handling

### Partial Formatting

If the AST contains parse errors, format what's valid:

```rust
fn format_module(module: &Module) -> FormatResult {
    let mut output = String::new();
    let mut errors = Vec::new();

    for item in &module.items {
        match item {
            Item::Valid(decl) => {
                format_decl(decl, &mut output);
            }
            Item::Error(span) => {
                // Preserve original text for error region
                output.push_str(&module.source[span.start..span.end]);
                errors.push(FormatError::ParseError(*span));
            }
        }
    }

    FormatResult { output, errors }
}
```

## Performance

### Caching

Width calculations are cached to avoid recomputation:

```rust
struct WidthCache {
    cache: HashMap<ExprId, usize>,
}
```

### Streaming Output

For large files, use a streaming emitter instead of building a string:

```rust
trait Emitter {
    fn emit(&mut self, text: &str);
    fn emit_newline(&mut self);
}

struct StringEmitter {
    buffer: String,
}

struct FileEmitter {
    writer: BufWriter<File>,
}
```

### Parallelization

Format multiple files in parallel:

```rust
fn format_directory(path: &Path) -> Vec<FormatResult> {
    let files: Vec<_> = glob(&path.join("**/*.ori")).collect();

    files.par_iter()
        .map(|file| format_file(file))
        .collect()
}
```

## Tooling Integration

### Crate Structure

The formatter is implemented as two crates:

| Crate | Location | Purpose |
|-------|----------|---------|
| `ori_fmt` | `compiler/ori_fmt/` | Core formatting logic |
| `ori_lsp` | `compiler/ori_lsp/` | Language Server Protocol implementation |

```
compiler/ori_fmt/     ← formatting algorithms, width calculation
        │
        ▼ (dependency)
compiler/ori_lsp/     ← LSP server, editor protocol
        │
        ▼ (compile to WASM)
playground/wasm/      ← browser integration
```

### LSP Server

The LSP server (`ori_lsp`) provides editor features via the Language Server Protocol:

| LSP Method | Feature | Description |
|------------|---------|-------------|
| `textDocument/formatting` | Format | Format entire document |
| `textDocument/publishDiagnostics` | Squigglies | Error/warning underlines |
| `textDocument/hover` | Hover | Type info and documentation |

Future capabilities:
- `textDocument/completion` — code completion
- `textDocument/definition` — go to definition
- `textDocument/references` — find all references

### Playground Integration

The Ori Playground uses the existing WASM infrastructure at `playground/wasm/`.

**Format-on-Run**: Following Go and Gleam playground conventions, code is automatically formatted when the user clicks Run. No separate format button.

**Architecture**:

```
┌─────────────────────────────────────────┐
│              Browser                    │
│  ┌──────────────┐    ┌──────────────┐   │
│  │    Monaco    │◄──►│  ori_lsp     │   │
│  │    Editor    │    │  (WASM)      │   │
│  └──────────────┘    └──────────────┘   │
│         │                   │           │
│         ▼                   ▼           │
│  ┌──────────────────────────────────┐   │
│  │         ori_eval (WASM)          │   │
│  │  (existing playground runtime)   │   │
│  └──────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

The LSP server compiles to WASM and runs in-browser, providing:
- Real-time diagnostics (red squigglies for errors)
- Hover information (types, documentation)
- Formatting (triggered on Run)

This architecture serves as an early sandbox for LSP features before the VS Code extension.

### Editor Integration

The same `ori_lsp` binary serves desktop editors:

| Editor | Integration |
|--------|-------------|
| VS Code | Extension spawns `ori_lsp` process |
| Neovim | Native LSP client connects to `ori_lsp` |
| Other | Any LSP-compatible editor |

Single implementation, multiple clients — the LSP handles:
- Formatting requests
- Diagnostic publishing
- Hover information
- (Future) Completions, definitions, references

## Testing

### Round-Trip Testing

Verify idempotence:

```rust
#[test]
fn test_idempotence() {
    let input = "...";
    let first = format(input);
    let second = format(&first);
    assert_eq!(first, second);
}
```

### Golden File Testing

Compare against expected output:

```rust
#[test]
fn test_function_formatting() {
    let input = include_str!("fixtures/input/functions.ori");
    let expected = include_str!("fixtures/expected/functions.ori");
    assert_eq!(format(input), expected);
}
```

### Property-Based Testing

Verify semantic preservation:

```rust
#[test]
fn test_semantic_preservation() {
    // Parse original
    let original_ast = parse(input);

    // Format and re-parse
    let formatted = format(input);
    let formatted_ast = parse(&formatted);

    // ASTs should be equivalent (ignoring spans)
    assert_eq!(
        strip_spans(&original_ast),
        strip_spans(&formatted_ast)
    );
}
```
