---
section: "03"
title: Shape Tracking
status: not-started
goal: Rustfmt-style shape tracking for width-based breaking
sections:
  - id: "03.1"
    title: Shape Struct
    status: not-started
  - id: "03.2"
    title: FormatterConfig
    status: not-started
  - id: "03.3"
    title: Shape Operations
    status: not-started
  - id: "03.4"
    title: Independent Breaking
    status: not-started
---

# Section 03: Shape Tracking

**Status:** 📋 Planned
**Goal:** Track available width as we descend into nested structures

> **Spec Reference:** Lines 14, 19 (max width), Lines 93-95 (independent breaking)

---

## 03.1 Shape Struct

Core type for tracking available formatting space.

- [ ] **Create** `ori_fmt/src/shape.rs`
- [ ] **Implement** `Shape` struct

```rust
/// Available formatting space
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// Characters remaining on current line
    pub width: usize,

    /// Current indentation level (in spaces)
    pub indent: usize,

    /// Position on first line (for alignment)
    pub offset: usize,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            width: 100,  // Default max width
            indent: 0,
            offset: 0,
        }
    }
}
```

- [ ] **Tests**: Shape creation and default values

---

## 03.2 FormatterConfig

Configuration for the formatter.

- [ ] **Implement** `FormatterConfig` struct

```rust
pub struct FormatterConfig {
    /// Maximum line width (default: 100, Spec line 19)
    pub max_width: usize,

    /// Indentation size in spaces (default: 4, Spec line 18)
    pub indent_size: usize,

    /// Whether to preserve trailing commas (or normalize)
    pub trailing_commas: TrailingCommas,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailingCommas {
    /// Always add trailing commas in multi-line
    Always,
    /// Never add trailing commas
    Never,
    /// Preserve user's choice
    Preserve,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        FormatterConfig {
            max_width: 100,
            indent_size: 4,
            trailing_commas: TrailingCommas::Always,
        }
    }
}
```

- [ ] **Tests**: Config creation and defaults

---

## 03.3 Shape Operations

Methods for manipulating shape during formatting.

- [ ] **Implement** Shape methods

```rust
impl Shape {
    /// Create a new shape with given max width
    pub fn new(max_width: usize) -> Self {
        Shape {
            width: max_width,
            indent: 0,
            offset: 0,
        }
    }

    /// Create shape from config
    pub fn from_config(config: &FormatterConfig) -> Self {
        Shape::new(config.max_width)
    }

    /// Reduce width by n characters (for content already emitted)
    pub fn consume(self, n: usize) -> Self {
        Shape {
            width: self.width.saturating_sub(n),
            offset: self.offset + n,
            ..self
        }
    }

    /// Add indentation for nested block
    pub fn indent(self, spaces: usize) -> Self {
        Shape {
            indent: self.indent + spaces,
            width: self.width.saturating_sub(spaces),
            ..self
        }
    }

    /// Remove indentation (dedent)
    pub fn dedent(self, spaces: usize) -> Self {
        Shape {
            indent: self.indent.saturating_sub(spaces),
            width: self.width + spaces,
            ..self
        }
    }

    /// Check if content fits in remaining width
    pub fn fits(&self, content_width: usize) -> bool {
        content_width <= self.width
    }

    /// Check if string fits
    pub fn fits_str(&self, s: &str) -> bool {
        self.fits(s.len())
    }

    /// Get shape for next line (reset to indent)
    pub fn next_line(self, max_width: usize) -> Self {
        Shape {
            width: max_width.saturating_sub(self.indent),
            offset: self.indent,
            indent: self.indent,
        }
    }

    /// Get remaining width
    pub fn remaining(&self) -> usize {
        self.width
    }
}
```

- [ ] **Tests**: Each operation with edge cases (overflow, underflow)

---

## 03.4 Independent Breaking

Nested constructs break independently based on their own width.

- [ ] **Implement** `for_nested()` method

```rust
impl Shape {
    /// Create shape for nested construct (Spec lines 93-95)
    /// "Nested constructs break independently based on their own width"
    pub fn for_nested(&self, config: &FormatterConfig) -> Shape {
        // Nested gets fresh width calculation from current position
        Shape {
            width: config.max_width.saturating_sub(self.indent),
            indent: self.indent,
            offset: self.indent,
        }
    }
}
```

- [ ] **Document** independent breaking semantics

```
Example: A function call that fits on one line stays inline even if
it's inside a larger construct that needs to break.

// The inner call fits, so it stays inline
let result = run(
    process(items.map(x -> x * 2)),  // This line fits, stays inline
    validate(result),
)
```

- [ ] **Tests**: Nested shapes get independent width checks

---

## 03.5 Integration Helpers

Helper methods for common formatting scenarios.

- [ ] **Implement** integration helpers

```rust
impl Shape {
    /// Get shape for function body (indented block)
    pub fn for_block(&self, config: &FormatterConfig) -> Self {
        self.indent(config.indent_size)
            .next_line(config.max_width)
    }

    /// Get shape for continuation (same indent, fresh line)
    pub fn for_continuation(&self, config: &FormatterConfig) -> Self {
        self.next_line(config.max_width)
    }

    /// Get shape after emitting a prefix string
    pub fn after(&self, prefix: &str) -> Self {
        self.consume(prefix.len())
    }

    /// Check if we should break (content doesn't fit)
    pub fn should_break(&self, content_width: usize) -> bool {
        !self.fits(content_width)
    }
}
```

- [ ] **Tests**: Integration helpers in formatting scenarios

---

## 03.6 Completion Checklist

- [ ] `Shape` struct with width, indent, offset
- [ ] `FormatterConfig` with max_width, indent_size
- [ ] All shape operations implemented
- [ ] Independent breaking for nested constructs
- [ ] Integration helpers for common scenarios
- [ ] Comprehensive unit tests

**Exit Criteria:** Shape tracking flows correctly through recursive formatting; nested constructs can check width independently without affecting parent's state.
