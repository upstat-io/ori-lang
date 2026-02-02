---
section: "05"
title: Formatter Orchestration
status: not-started
goal: Main formatter orchestrating all layers
sections:
  - id: "05.1"
    title: Formatter Struct
    status: not-started
  - id: "05.2"
    title: Try-Inline Pattern
    status: not-started
  - id: "05.3"
    title: Broken Formatting
    status: not-started
  - id: "05.4"
    title: Module Structure
    status: not-started
  - id: "05.5"
    title: Trailing Commas
    status: not-started
  - id: "05.6"
    title: Blank Lines
    status: not-started
---

# Section 05: Formatter Orchestration

**Status:** 📋 Planned
**Goal:** Main formatter that orchestrates all layers (spacing, packing, shape, breaking rules)

> **Spec Reference:** Lines 17-21 (general rules), Lines 49-56 (blank lines)

---

## 05.1 Formatter Struct

Core formatter type that holds state and delegates to layers.

- [ ] **Refactor** `ori_fmt/src/formatter/mod.rs`
- [ ] **Implement** `Formatter` struct

```rust
pub struct Formatter<'a> {
    // Input
    arena: &'a ExprArena,
    config: FormatterConfig,

    // Layers
    rules_map: &'a RulesMap,

    // State
    shape: Shape,
    indent_stack: Vec<usize>,

    // Output
    emitter: Emitter,
}

impl<'a> Formatter<'a> {
    pub fn new(
        arena: &'a ExprArena,
        config: FormatterConfig,
        rules_map: &'a RulesMap,
    ) -> Self {
        Formatter {
            arena,
            config,
            rules_map,
            shape: Shape::from_config(&config),
            indent_stack: Vec::new(),
            emitter: Emitter::new(),
        }
    }

    pub fn format_module(&mut self, module: &Module) -> String {
        self.format_imports(&module.imports);
        self.format_constants(&module.constants);
        self.format_declarations(&module.declarations);
        self.emitter.finish()
    }
}
```

- [ ] **Tests**: Formatter creation and basic usage

---

## 05.2 Try-Inline Pattern

Core formatting strategy: try inline first, fall back to broken.

- [ ] **Implement** try-inline pattern

```rust
impl<'a> Formatter<'a> {
    /// Main entry point for formatting an expression
    pub fn format_expr(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);

        // Try inline first
        if let Some(inline) = self.try_inline(expr_id) {
            if self.shape.fits_str(&inline) {
                self.emitter.emit(&inline);
                return;
            }
        }

        // Fall back to broken format
        self.format_broken(expr_id);
    }

    /// Try to render expression on single line
    fn try_inline(&self, expr_id: ExprId) -> Option<String> {
        let expr = self.arena.get_expr(expr_id);

        // Check if always-stacked (never inline)
        if self.is_always_stacked(expr) {
            return None;
        }

        // Try to render inline
        let mut inline_formatter = self.clone_for_inline();
        inline_formatter.emit_inline(expr_id);
        Some(inline_formatter.emitter.finish())
    }

    fn is_always_stacked(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Call { func, .. } => {
                if let Some(name) = self.get_builtin_name(func) {
                    let is_stacked_builtin = matches!(
                        name, "run" | "try" | "match" | "recurse" |
                              "parallel" | "spawn" | "nursery"
                    );
                    is_stacked_builtin && RunRule::is_top_level(&self.context)
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}
```

- [ ] **Tests**: Expressions that fit inline, expressions that break

---

## 05.3 Broken Formatting

Dispatch to appropriate breaking rule based on expression type.

- [ ] **Implement** broken formatting dispatch

```rust
impl<'a> Formatter<'a> {
    fn format_broken(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            // Delegate to breaking rules
            ExprKind::For { .. } => {
                NestedForRule::format(expr.as_for(), self.shape, &mut self.emitter, self.arena, &self.config);
            }
            ExprKind::MethodCall { .. } => {
                let chain = self.collect_method_chain(expr_id);
                MethodChainRule::format(&chain, self.shape, &mut self.emitter, self.arena);
            }
            ExprKind::If { .. } => {
                ChainedElseIfRule::format(expr.as_if(), self.shape, &mut self.emitter, self.arena);
            }
            ExprKind::Binary { op: BinaryOp::Or, .. } => {
                let clauses = self.collect_or_clauses(expr_id);
                if BooleanBreakRule::should_break_at_or(expr) {
                    BooleanBreakRule::format(&clauses, self.shape, &mut self.emitter, self.arena);
                } else {
                    self.format_binary_default(expr_id);
                }
            }
            ExprKind::Call { func, args, .. } => {
                if self.is_run_call(func) {
                    self.format_run(args);
                } else if self.is_loop_call(func) {
                    LoopRule::format(expr.as_loop(), self.shape, &mut self.emitter, self.arena);
                } else {
                    self.format_call_default(expr_id);
                }
            }
            // Other expressions use default broken format
            _ => self.format_default_broken(expr_id),
        }
    }
}
```

- [ ] **Tests**: Each expression type dispatches correctly

---

## 05.4 Module Structure

Format module-level structure with proper blank lines.

- [ ] **Implement** module formatting

```rust
impl<'a> Formatter<'a> {
    /// Format module with proper section ordering and blank lines
    pub fn format_module(&mut self, module: &Module) {
        // Imports first
        self.format_imports(&module.imports);
        if !module.imports.is_empty() {
            self.emitter.blank_line();
        }

        // Constants
        self.format_constants(&module.constants);
        if !module.constants.is_empty() {
            self.emitter.blank_line();
        }

        // Declarations with blank lines between
        for (i, decl) in module.declarations.iter().enumerate() {
            if i > 0 {
                self.emitter.blank_line();
            }
            self.format_declaration(decl);
        }
    }

    /// Format imports: stdlib first, relative second, sorted alphabetically
    fn format_imports(&mut self, imports: &[Import]) {
        let (stdlib, relative): (Vec<_>, Vec<_>) =
            imports.iter().partition(|i| i.is_stdlib());

        let mut stdlib: Vec<_> = stdlib;
        let mut relative: Vec<_> = relative;

        stdlib.sort_by(|a, b| a.path.cmp(&b.path));
        relative.sort_by(|a, b| a.path.cmp(&b.path));

        for import in &stdlib {
            self.format_import(import);
            self.emitter.newline();
        }

        if !stdlib.is_empty() && !relative.is_empty() {
            self.emitter.blank_line();
        }

        for import in &relative {
            self.format_import(import);
            self.emitter.newline();
        }
    }
}
```

- [ ] **Tests**: Module with various section combinations

> **Spec Reference:** Lines 848-877 (Import ordering)

---

## 05.5 Trailing Commas

Handle trailing comma normalization per spec.

- [ ] **Implement** trailing comma handling

```rust
impl<'a> Formatter<'a> {
    /// Emit trailing comma based on config and line mode
    /// Spec line 20: "Trailing commas required in multi-line, forbidden in single-line"
    fn emit_trailing_comma(&mut self, is_multiline: bool) {
        match self.config.trailing_commas {
            TrailingCommas::Always if is_multiline => {
                self.emitter.emit(",");
            }
            TrailingCommas::Preserve => {
                // Keep whatever was in source (handled by caller)
            }
            TrailingCommas::Never | TrailingCommas::Always => {
                // Never for single-line, or Always but single-line
            }
        }
    }

    /// Format list of items with proper trailing comma
    fn format_items_with_trailing<F>(
        &mut self,
        items: &[ExprId],
        is_multiline: bool,
        format_item: F,
    )
    where
        F: Fn(&mut Self, ExprId),
    {
        for (i, item) in items.iter().enumerate() {
            format_item(self, *item);
            if i < items.len() - 1 {
                self.emitter.emit(",");
                if is_multiline {
                    self.emitter.newline();
                } else {
                    self.emitter.emit(" ");
                }
            } else {
                // Last item: trailing comma if multiline
                self.emit_trailing_comma(is_multiline);
            }
        }
    }
}
```

- [ ] **Tests**: Single-line (no trailing), multi-line (with trailing)

---

## 05.6 Blank Lines

Normalize blank lines per spec.

- [ ] **Implement** blank line normalization

```rust
impl<'a> Formatter<'a> {
    /// Format impl/trait methods with blank lines between
    /// Spec lines 360-382: "One blank line between methods (except single-method)"
    fn format_impl_methods(&mut self, methods: &[Method]) {
        if methods.len() == 1 {
            self.format_method(&methods[0]);
        } else {
            for (i, method) in methods.iter().enumerate() {
                if i > 0 {
                    self.emitter.blank_line();
                }
                self.format_method(method);
            }
        }
    }
}

impl Emitter {
    /// Emit a blank line (normalizing consecutive blank lines)
    /// Spec line 21: "No consecutive, leading, or trailing blank lines"
    pub fn blank_line(&mut self) {
        // Only add blank line if last line wasn't blank
        if !self.last_was_blank {
            self.output.push('\n');
            self.last_was_blank = true;
        }
    }

    /// Post-process to remove leading/trailing blank lines
    pub fn normalize_blank_lines(&mut self) {
        // Remove leading blank lines
        let trimmed = self.output.trim_start_matches('\n');
        let leading_removed = self.output.len() - trimmed.len();
        self.output.drain(..leading_removed);

        // Remove trailing blank lines (but keep final newline)
        while self.output.ends_with("\n\n") {
            self.output.pop();
        }
    }
}
```

- [ ] **Tests**: Consecutive blank lines normalized, leading/trailing removed

---

## 05.7 Emitter

Low-level output emitter with indentation tracking.

- [ ] **Refactor** `ori_fmt/src/emitter.rs`
- [ ] **Implement** clean emitter interface

```rust
pub struct Emitter {
    output: String,
    current_indent: usize,
    indent_size: usize,
    at_line_start: bool,
    last_was_blank: bool,
}

impl Emitter {
    pub fn new() -> Self {
        Emitter {
            output: String::new(),
            current_indent: 0,
            indent_size: 4,
            at_line_start: true,
            last_was_blank: false,
        }
    }

    pub fn emit(&mut self, s: &str) {
        if self.at_line_start && !s.is_empty() {
            self.emit_indent();
            self.at_line_start = false;
        }
        self.output.push_str(s);
        self.last_was_blank = false;
    }

    pub fn newline(&mut self) {
        self.output.push('\n');
        self.at_line_start = true;
        self.last_was_blank = false;
    }

    pub fn indent(&mut self) {
        self.current_indent += self.indent_size;
    }

    pub fn dedent(&mut self) {
        self.current_indent = self.current_indent.saturating_sub(self.indent_size);
    }

    fn emit_indent(&mut self) {
        for _ in 0..self.current_indent {
            self.output.push(' ');
        }
    }

    pub fn finish(mut self) -> String {
        self.normalize_blank_lines();
        self.output
    }
}
```

- [ ] **Tests**: Emitter indentation, newlines, normalization

---

## 05.8 Completion Checklist

- [ ] `Formatter` struct with all layers integrated
- [ ] Try-inline pattern implemented
- [ ] Broken formatting dispatches to rules
- [ ] Module structure with proper sections
- [ ] Trailing comma normalization
- [ ] Blank line normalization
- [ ] Emitter with clean interface
- [ ] Unit tests for each component
- [ ] Integration tests for full formatting

**Exit Criteria:** The formatter orchestrates all 5 layers cleanly; formatting decisions are delegated to appropriate layers rather than hardcoded in the main loop.
