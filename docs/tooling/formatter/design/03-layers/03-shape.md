---
title: "Layer 3: Shape Tracking"
description: "Ori Formatter Design — Width Tracking for Fit Decisions"
order: 4
section: "Layers"
---

# Layer 3: Shape Tracking

The shape layer tracks available width through recursive formatting. Inspired by rustfmt's shape tracking, it enables independent breaking decisions for nested constructs.

## Architecture

```
Shape { width, indent, offset }
       │
       ├── consume(n) ──▶ reduce width, increase offset
       ├── indent(n) ──▶ increase indent, reduce width
       ├── next_line() ──▶ reset to indent, recalculate width
       └── fits(w) ──▶ check if content fits
```

## Key Types

### Shape

The central struct tracking formatting state:

```rust
pub struct Shape {
    /// Characters remaining on current line.
    pub width: usize,

    /// Current indentation level (in spaces).
    pub indent: usize,

    /// Position on current line (from start of line).
    pub offset: usize,
}
```

The three fields capture distinct information:
- **width**: How many more characters fit on this line
- **indent**: Where the next line starts (after newline + indent)
- **offset**: Current horizontal position (for alignment calculations)

## Core Operations

### consume(n)

Reduce available width after emitting content:

```rust
pub fn consume(self, n: usize) -> Self {
    Shape {
        width: self.width.saturating_sub(n),
        offset: self.offset + n,
        ..self
    }
}
```

Example:
```rust
let shape = Shape::new(100);  // width=100, offset=0
let shape = shape.consume(8); // "let x = " - width=92, offset=8
```

### indent(n)

Add indentation for nested block:

```rust
pub fn indent(self, spaces: usize) -> Self {
    Shape {
        indent: self.indent + spaces,
        width: self.width.saturating_sub(spaces),
        ..self
    }
}
```

Example:
```rust
let shape = Shape::new(100);   // indent=0, width=100
let indented = shape.indent(4); // indent=4, width=96
```

### dedent(n)

Remove indentation (reverse of `indent`):

```rust
pub fn dedent(self, spaces: usize) -> Self {
    Shape {
        indent: self.indent.saturating_sub(spaces),
        width: self.width + spaces,
        ..self
    }
}
```

Example:
```rust
let shape = Shape::new(100).indent(8); // indent=8, width=92
let back = shape.dedent(4);            // indent=4, width=96
```

### next_line(max_width)

Reset to start of next line:

```rust
pub fn next_line(self, max_width: usize) -> Self {
    Shape {
        width: max_width.saturating_sub(self.indent),
        offset: self.indent,
        indent: self.indent,
    }
}
```

Example:
```rust
let shape = Shape::new(100).indent(8);
// At end of long line...
let next = shape.next_line(100); // width=92, offset=8
```

### fits(content_width)

Check if content fits in remaining width:

```rust
pub fn fits(&self, content_width: usize) -> bool {
    content_width <= self.width
}
```

## Independent Breaking

The key design principle (Spec lines 93-95):

> "Nested constructs break independently based on their own width"

This means:
- A function call inside a broken container can stay inline if it fits
- Each nested construct gets a fresh width calculation from current indent
- Parent breaking doesn't force child breaking

### for_nested()

Creates shape for nested construct with fresh width from current indent:

```rust
pub fn for_nested(&self, config: &FormatConfig) -> Shape {
    Shape {
        width: config.max_width.saturating_sub(self.indent),
        indent: self.indent,
        offset: self.indent,
    }
}
```

Example:
```ori
// Even though outer run is broken, inner fits inline:
let result = run(
    process(items.map(x -> x * 2)),  // This call fits, stays inline
    validate(result),                 // So does this
)
```

Without independent breaking, the inner calls would also break, creating unnecessary vertical sprawl.

## Usage Patterns

### Basic Format Decision

```rust
let shape = Shape::from_config(&config);

// Measure content width
let width = self.width_calc.width(expr_id);

// Decide format mode
if shape.fits(width) {
    self.emit_inline(expr_id);
} else {
    self.emit_broken(expr_id);
}
```

### Recursive Formatting

```rust
fn format_binary(&mut self, op: &BinaryOp, left: ExprId, right: ExprId, shape: Shape) {
    // Format left operand
    self.format_expr(left, shape);

    // Emit operator, consuming its width
    let shape = shape.consume(3); // " + "
    self.emit_op(op);

    // Format right operand in remaining space
    self.format_expr(right, shape);
}
```

### Broken Container

```rust
fn format_list_broken(&mut self, items: &[ExprId], shape: Shape) {
    self.emit("[");
    self.emit_newline();

    let item_shape = shape.indent(4);
    for item in items {
        self.emit_indent();
        // Each item gets fresh width from new line
        self.format_expr(item, item_shape.next_line(config.max_width));
        self.emit(",");
        self.emit_newline();
    }

    self.emit("]");
}
```

## Helper Methods

### Block and Continuation

```rust
impl Shape {
    /// Get shape for indented block body
    pub fn for_block(&self, config: &FormatConfig) -> Self {
        self.indent(config.indent_size).next_line(config.max_width)
    }

    /// Get shape for continuation line (same indent)
    pub fn for_continuation(&self, config: &FormatConfig) -> Self {
        self.next_line(config.max_width)
    }

    /// Get shape after emitting prefix
    pub fn after(&self, prefix: &str) -> Self {
        self.consume(prefix.len())
    }
}
```

### State Checks

```rust
impl Shape {
    /// Remaining characters on line
    pub fn remaining(&self) -> usize {
        self.width
    }

    /// Should we break? (content doesn't fit)
    pub fn should_break(&self, content_width: usize) -> bool {
        !self.fits(content_width)
    }

    /// Are we at line start?
    pub fn at_line_start(&self) -> bool {
        self.offset == self.indent
    }
}
```

## Integration with Width Calculator

The width calculator produces inline widths. Shape determines if those widths fit:

```rust
impl Formatter {
    fn format(&mut self, expr_id: ExprId, shape: Shape) {
        let width = self.width_calc.width(expr_id);

        if width == ALWAYS_STACKED {
            // Special constructs (run, try, match)
            self.emit_stacked(expr_id);
        } else if shape.fits(width) {
            // Fits inline
            self.emit_inline(expr_id);
        } else {
            // Break according to rules
            self.emit_broken(expr_id);
        }
    }
}
```

## Default Configuration

```rust
impl Default for Shape {
    fn default() -> Self {
        Shape {
            width: 100,  // Default max width from spec
            indent: 0,
            offset: 0,
        }
    }
}
```

## Design Notes

### Why Three Fields?

- **width alone** wouldn't track indentation for new lines
- **indent alone** wouldn't track current horizontal position
- **offset** enables alignment (though rarely used in Ori's simple formatting)

### Saturating Operations

All operations use `saturating_sub` to prevent overflow:

```rust
// Safe even if consumed more than available
width: self.width.saturating_sub(n)
```

This handles edge cases gracefully rather than panicking.

### Immutable Pattern

Shape operations return new shapes (functional style):

```rust
#[must_use = "consume returns a new Shape"]
pub fn consume(self, n: usize) -> Self
```

This prevents bugs from modifying shared state during recursive formatting.

## Spec Reference

This layer implements:
- Lines 14, 19: Max width (100 chars)
- Line 18: Indent size (4 spaces)
- Lines 93-95: Independent breaking for nested constructs
