---
section: "02"
title: Container Packing
status: not-started
goal: Gleam-style container packing decisions
sections:
  - id: "02.1"
    title: Packing Enum
    status: not-started
  - id: "02.2"
    title: Construct Kind
    status: not-started
  - id: "02.3"
    title: Determine Packing
    status: not-started
  - id: "02.4"
    title: Simple vs Complex Detection
    status: not-started
  - id: "02.5"
    title: Separator Handling
    status: not-started
---

# Section 02: Container Packing

**Status:** 📋 Planned
**Goal:** Decision tree for how to format lists, args, fields, and other containers

> **Spec Reference:** Lines 58-92 (Width-based and always-stacked rules)

---

## 02.1 Packing Enum

Define the packing strategies.

- [ ] **Create** `ori_fmt/src/packing/mod.rs`
- [ ] **Implement** `Packing` enum

```rust
/// How to pack items in a container
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Packing {
    /// Try single line; if doesn't fit, one item per line
    FitOrOnePerLine,

    /// Try single line; if doesn't fit, pack multiple per line
    FitOrPackMultiple,

    /// Always one item per line (trailing comma present, or rule says so)
    AlwaysOnePerLine,

    /// Always stacked with specific formatting (run, try, match, etc.)
    AlwaysStacked,
}
```

- [ ] **Tests**: Packing enum equality and debug output

---

## 02.2 Construct Kind

Enumerate all container types that need packing decisions.

- [ ] **Implement** `ConstructKind` enum

```rust
pub enum ConstructKind {
    // === ALWAYS STACKED (Spec lines 78-90) ===
    RunTopLevel,
    Try,
    Match,
    Recurse,
    Parallel,
    Spawn,
    Nursery,

    // === WIDTH-BASED: One per line when broken (Spec lines 64-74) ===
    FunctionParams,
    FunctionArgs,
    GenericParams,
    WhereConstraints,
    Capabilities,
    StructFieldsDef,
    StructFieldsLiteral,
    SumVariants,
    MapEntries,
    TupleElements,
    ImportItems,

    // === WIDTH-BASED: Multiple per line for simple items (Spec line 75) ===
    ListSimple,

    // === WIDTH-BASED: One per line for complex items (Spec line 76) ===
    ListComplex,

    // === CONTEXT-DEPENDENT ===
    RunNested,  // Width-based (Spec line 91)
}
```

- [ ] **Tests**: All construct kinds are distinct

---

## 02.3 Determine Packing

Main decision function for container formatting.

- [ ] **Implement** `determine_packing()` function

```rust
pub fn determine_packing(
    construct: ConstructKind,
    has_trailing_comma: bool,
    has_comments: bool,
    has_empty_lines: bool,
    _item_count: usize,
) -> Packing {
    use ConstructKind::*;
    use Packing::*;

    // Always-stacked constructs (from spec)
    if matches!(construct,
        RunTopLevel | Try | Match | Recurse | Parallel | Spawn | Nursery
    ) {
        return AlwaysStacked;
    }

    // Empty lines between items → preserve vertical spacing
    if has_empty_lines {
        return AlwaysOnePerLine;
    }

    // Trailing comma signals user intent to break
    if has_trailing_comma {
        return AlwaysOnePerLine;
    }

    // Comments force breaking
    if has_comments {
        return AlwaysOnePerLine;
    }

    // Simple lists can pack multiple per line
    if matches!(construct, ListSimple) {
        return FitOrPackMultiple;
    }

    // Default: try inline, else one per line
    FitOrOnePerLine
}
```

- [ ] **Tests**: Each packing path with relevant inputs

---

## 02.4 Simple vs Complex Detection

Determine if list items are simple (can pack multiple) or complex (one per line).

- [ ] **Implement** `is_simple_item()` function

```rust
/// Spec lines 225-242: Simple = literals, identifiers; Complex = structs, calls, nested
pub fn is_simple_item(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Literal { .. } => true,
        ExprKind::Ident { .. } => true,
        _ => false,
    }
}
```

- [ ] **Implement** `list_packing()` helper

```rust
pub fn list_packing(items: &[ExprId], arena: &ExprArena) -> Packing {
    if items.iter().all(|id| is_simple_item(arena.get_expr(*id))) {
        Packing::FitOrPackMultiple
    } else {
        Packing::FitOrOnePerLine
    }
}
```

- [ ] **Tests**: Lists with all simple, all complex, mixed items

---

## 02.5 Separator Handling

Define separators for different packing modes.

- [ ] **Implement** `Separator` enum

```rust
/// What separator to use between items
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Separator {
    /// ", " inline, ",\n" when broken
    Comma,
    /// " " inline, "\n" when broken
    Space,
    /// Always newline
    Newline,
}
```

- [ ] **Implement** `separator_for()` function

```rust
pub fn separator_for(construct: ConstructKind, packing: Packing) -> Separator {
    use ConstructKind::*;
    use Packing::*;
    use Separator::*;

    match (construct, packing) {
        // Always-stacked uses commas with newlines
        (RunTopLevel | Try | Match | Recurse | Parallel | Spawn | Nursery, _) => Comma,

        // Most constructs use commas
        (FunctionParams | FunctionArgs | GenericParams | StructFieldsDef |
         StructFieldsLiteral | MapEntries | TupleElements | ImportItems |
         ListSimple | ListComplex | RunNested, _) => Comma,

        // Where constraints and sum variants may use space
        (WhereConstraints, AlwaysOnePerLine) => Comma,
        (WhereConstraints, _) => Comma,

        // Sum variants use | (handled specially)
        (SumVariants, _) => Space,  // | is part of syntax, not separator

        // Capabilities use comma
        (Capabilities, _) => Comma,
    }
}
```

- [ ] **Tests**: Separator selection for various construct/packing combinations

---

## 02.6 Completion Checklist

- [ ] `Packing` enum with 4 variants
- [ ] `ConstructKind` enum with ~18 construct types
- [ ] `determine_packing()` handles all cases
- [ ] Simple/complex item detection working
- [ ] Separator selection for all constructs
- [ ] Unit tests for each decision path

**Exit Criteria:** Container formatting decisions are centralized; adding support for new constructs requires only adding a `ConstructKind` variant and updating `determine_packing()`.
