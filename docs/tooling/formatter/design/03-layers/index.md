---
title: "5-Layer Architecture"
description: "Ori Formatter Design — Modular Architecture"
order: 1
section: "Layers"
---

# 5-Layer Architecture

The Ori formatter uses a layered architecture inspired by modern formatters (rustfmt, Gleam, TypeScript). Each layer has a single responsibility and well-defined interfaces.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│  Layer 5: Orchestration (formatter/)                            │
│  • Coordinates all layers                                       │
│  • Renders inline/broken/stacked based on width                 │
│  • Main Formatter struct and public API                         │
├─────────────────────────────────────────────────────────────────┤
│  Layer 4: Breaking Rules (rules/)                               │
│  • 8 Ori-specific rules for special constructs                  │
│  • Method chains, short bodies, boolean breaks, etc.            │
├─────────────────────────────────────────────────────────────────┤
│  Layer 3: Shape Tracking (shape/)                               │
│  • Width tracking through recursion                             │
│  • Independent breaking for nested constructs                   │
├─────────────────────────────────────────────────────────────────┤
│  Layer 2: Container Packing (packing/)                          │
│  • When to inline vs break containers                           │
│  • Packing strategies: FitOrOnePerLine, FitOrPackMultiple, etc. │
├─────────────────────────────────────────────────────────────────┤
│  Layer 1: Token Spacing (spacing/)                              │
│  • O(1) declarative spacing rules                               │
│  • TokenCategory → SpaceAction lookup                           │
└─────────────────────────────────────────────────────────────────┘
```

## Layer Responsibilities

| Layer | Module | Responsibility | Key Types |
|-------|--------|----------------|-----------|
| 1 | `spacing/` | Token spacing (space, none, newline) | `SpaceAction`, `TokenCategory`, `RulesMap` |
| 2 | `packing/` | Container packing decisions | `Packing`, `ConstructKind`, `Separator` |
| 3 | `shape/` | Width tracking through recursion | `Shape` |
| 4 | `rules/` | Ori-specific breaking rules | 8 rule structs |
| 5 | `formatter/` | Orchestration and rendering | `Formatter` |

## Data Flow

```
Source → Parse → AST
                  │
                  ▼
         ┌──────────────────┐
         │ Width Calculator │ (bottom-up traversal, uses shape/)
         └────────┬─────────┘
                  │
                  ▼
         ┌──────────────────┐
         │    Formatter     │ (top-down rendering)
         │                  │
         │  ┌─────────────┐ │
         │  │ spacing/    │─┼─→ token spacing
         │  │ packing/    │─┼─→ container decisions
         │  │ rules/      │─┼─→ special construct handling
         │  └─────────────┘ │
         └────────┬─────────┘
                  │
                  ▼
           Formatted Output
```

## Design Principles

### 1. Separation of Concerns

Each layer handles exactly one aspect of formatting:

- **Spacing**: Only token-to-token spacing decisions
- **Packing**: Only inline vs break decisions for containers
- **Shape**: Only width tracking and fit checks
- **Rules**: Only special-case breaking logic
- **Orchestration**: Only coordination and rendering

### 2. Information Flow

Information flows down the layers:
- Orchestration queries rules for special cases
- Rules query shape for fit checks
- Shape queries packing for strategy
- All layers may use spacing for token output

No layer calls upward, preventing circular dependencies.

### 3. Declarative Where Possible

- **Layer 1 (Spacing)**: Pure declarative rules (token pair → action)
- **Layer 2 (Packing)**: Declarative strategies (construct → packing)
- **Layers 3-5**: Imperative but with clear decision boundaries

## Adding or Modifying Rules

### Token Spacing (Layer 1)

Add new spacing rules in `spacing/rules.rs`:

```rust
// Example: Add spacing after new keyword
(TokenCategory::MyKeyword, TokenCategory::Ident) => SpaceAction::Space,
```

### Packing Strategies (Layer 2)

Add new constructs in `packing/construct.rs`:

```rust
// Add new construct kind
pub enum ConstructKind {
    // ...existing...
    MyNewConstruct,
}
```

### Special Breaking Rules (Layer 4)

Create a new rule module:

1. Create `rules/my_rule.rs`
2. Define the rule struct with decision logic
3. Export from `rules/mod.rs`
4. Integrate with orchestration

See [Breaking Rules](04-rules.md) for details on each rule.

## Layer Documentation

- [Layer 1: Token Spacing](01-spacing.md) — O(1) declarative token spacing
- [Layer 2: Container Packing](02-packing.md) — Container packing decisions
- [Layer 3: Shape Tracking](03-shape.md) — Width tracking for fit decisions
- [Layer 4: Breaking Rules](04-rules.md) — Ori-specific breaking rules

## Module Structure

```
ori_fmt/
├── spacing/           # Layer 1: Token spacing
│   ├── action.rs      # SpaceAction enum
│   ├── category.rs    # TokenCategory grouping
│   ├── matcher.rs     # TokenMatcher patterns
│   ├── rules.rs       # Declarative rules
│   ├── lookup.rs      # O(1) RulesMap
│   └── tests.rs
├── packing/           # Layer 2: Container packing
│   ├── strategy.rs    # Packing enum, determine_packing()
│   ├── construct.rs   # ConstructKind
│   ├── separator.rs   # Separator enum
│   ├── simple.rs      # Simple item detection
│   └── tests.rs
├── shape/             # Layer 3: Shape tracking
│   ├── core.rs        # Shape struct
│   └── tests.rs
├── rules/             # Layer 4: Breaking rules
│   ├── method_chain.rs
│   ├── short_body.rs
│   ├── boolean_break.rs
│   ├── chained_else_if.rs
│   ├── nested_for.rs
│   ├── parentheses.rs
│   ├── run_rule.rs
│   ├── loop_rule.rs
│   └── tests.rs
├── formatter/         # Layer 5: Orchestration
│   ├── mod.rs         # Main Formatter struct
│   ├── inline.rs      # Single-line rendering
│   ├── broken.rs      # Multi-line rendering
│   ├── stacked.rs     # Always-stacked constructs
│   ├── helpers.rs     # Collection helpers
│   ├── patterns.rs    # Pattern rendering
│   ├── literals.rs    # Literal rendering
│   └── tests.rs
├── declarations/      # Module-level formatting
├── comments/          # Comment preservation
├── context/           # Formatting context
├── emitter/           # Output abstraction
├── width/             # Width calculation
└── incremental/       # Incremental formatting
```

## Relationship to Spec

The formatting spec (`docs/ori_lang/0.1-alpha/spec/16-formatting.md`) defines *what* the canonical format is. This layer architecture explains *how* the implementation achieves that format.

| Spec Section | Implementation Layer |
|--------------|---------------------|
| Spacing table (lines 25-47) | Layer 1 (spacing/) |
| Width-based rules (lines 58-92) | Layer 2 (packing/), Layer 3 (shape/) |
| Construct-specific rules | Layer 4 (rules/) |
| Overall formatting | Layer 5 (formatter/) |
