---
title: "Incremental Parsing"
description: "Ori Compiler Design — Incremental Parsing for IDE Support"
order: 404
section: "Parser"
---

# Incremental Parsing

The Ori parser supports incremental reuse of unchanged declarations from a previous parse. When a user edits a file in an IDE, only the declarations that overlap with the edited region are re-parsed — the rest are copied from the old AST with adjusted spans.

## Motivation

In IDE scenarios, most edits affect a single function body. Re-parsing the entire file on every keystroke wastes work. Incremental parsing identifies unchanged declarations and copies them from the old AST, adjusting spans to account for text insertions/deletions.

## Architecture

```
TextChange { start, old_len, new_len }
    │
    ▼
ChangeMarker { affected_start, affected_end, delta }
    │
    ├── SyntaxCursor: navigates old AST by span position
    │       │
    │       ▼
    │   DeclRef { kind, index, span } — reference to old declaration
    │
    └── AstCopier: deep-copies declarations with span adjustment
            │
            ▼
        New Module + new ExprArena (mixed old-copied + fresh-parsed)
```

## Components

### TextChange

Describes a single text edit:

```rust
pub struct TextChange {
    pub start: u32,    // Byte offset of edit start
    pub old_len: u32,  // Bytes removed
    pub new_len: u32,  // Bytes inserted
}
```

### ChangeMarker

Computed from `TextChange`, extends the affected region for lookahead safety:

```rust
pub struct ChangeMarker {
    pub affected_start: u32,
    pub affected_end: u32,
    pub delta: i64,  // new_len - old_len (positive = insertion)
}
```

The affected region is extended backwards to the end of the previous token, ensuring that multi-token lookahead patterns are not disrupted by the boundary.

### DeclRef

A lightweight reference to a declaration in the old AST:

```rust
pub struct DeclRef {
    pub kind: DeclKind,
    pub index: usize,
    pub span: Span,
}

pub enum DeclKind {
    Import, Const, Function, Test, Type, Trait, Impl, DefImpl, Extend,
}
```

### SyntaxCursor

Navigates the old module's declarations, sorted by span position:

```rust
pub struct SyntaxCursor<'a> {
    module: &'a Module,
    arena: &'a ExprArena,
    declarations: Vec<DeclRef>,  // sorted by span.start
    marker: ChangeMarker,
    position: usize,
}
```

`find_at(pos)` locates a declaration at the given position. The caller then checks whether the declaration intersects the change region.

### AstCopier

Deep-copies a declaration from the old arena into the new arena, adjusting all spans by the change delta:

```rust
pub struct AstCopier<'a> {
    old_arena: &'a ExprArena,
    marker: ChangeMarker,
}
```

Copy methods exist for each declaration type:

| Method | Input | Output |
|--------|-------|--------|
| `copy_function()` | `&Function` | `Function` with remapped `ExprId`s |
| `copy_test()` | `&TestDef` | `TestDef` with remapped `ExprId`s |
| `copy_type_decl()` | `&TypeDecl` | `TypeDecl` with remapped `ExprId`s |
| `copy_trait()` | `&TraitDef` | `TraitDef` with remapped `ExprId`s |
| `copy_impl()` | `&ImplDef` | `ImplDef` with remapped `ExprId`s |
| `copy_def_impl()` | `&DefImplDef` | `DefImplDef` with remapped `ExprId`s |
| `copy_extend()` | `&ExtendDef` | `ExtendDef` with remapped `ExprId`s |
| `copy_const()` | `&ConstDef` | `ConstDef` with remapped `ExprId`s |

Each copy recursively allocates new `ExprId`s in the destination arena, so the old and new arenas remain independent.

## Algorithm

### 1. Collect Declarations

`collect_declarations(module)` produces a sorted `Vec<DeclRef>` from the old module, ordered by `span.start`.

### 2. Parse with Reuse

`parse_module_incremental()` processes the token stream:

```
for each position in token stream:
    if SyntaxCursor finds a declaration at this position:
        if declaration is OUTSIDE the change region:
            → COPY via AstCopier (adjust spans, remap ExprIds)
            → Skip tokens past the declaration span
        else:
            → RE-PARSE fresh from the token stream
    else:
        → RE-PARSE fresh from the token stream
```

Imports are always re-parsed because they affect module resolution globally.

### 3. Span Adjustment

Declarations **after** the change region have their spans shifted by the change delta:

```
Original:   [func_a 0..50]  [EDITED 50..80]  [func_b 80..120]
After edit (delta = +10):
            [func_a 0..50]  [EDITED 50..90]  [func_b 90..130]
```

`func_a` is before the change — copied without adjustment.
`func_b` is after the change — all spans shifted by +10.
The edited region is re-parsed from tokens.

## Statistics

`IncrementalStats` tracks reuse efficiency:

```rust
pub struct IncrementalStats {
    pub reused_count: usize,
    pub reparsed_count: usize,
}

impl IncrementalStats {
    pub fn reuse_rate(&self) -> f64;
}
```

`CursorStats` tracks lookup performance:

```rust
pub struct CursorStats {
    pub lookups: usize,
    pub skipped: usize,
    pub intersected: usize,
}
```

## Limitations

- **Imports are always re-parsed** — They affect global resolution and are typically few in number.
- **Single-edit model** — The current design handles one `TextChange` per incremental parse. Multiple concurrent edits require coalescing into a single change.
- **Metadata not merged** — Incremental parsing does not yet merge `ModuleExtra` (comments, blank lines). For full metadata support, a separate lex-with-comments pass is needed.
- **Arena independence** — Copied declarations get new `ExprId`s in the new arena. The old arena and module are not modified.

## Usage

```rust
// Initial parse
let output = ori_parse::parse(&tokens, &interner);

// After user edits the file
let change = TextChange { start: 42, old_len: 5, new_len: 8 };
let new_tokens = ori_lexer::lex(new_source, &interner);
let new_output = ori_parse::parse_incremental(
    &new_tokens,
    &interner,
    &output,
    change,
);
```

## Design Rationale

This approach trades implementation complexity for parsing speed in IDE scenarios. The key trade-off:

| Approach | Speed | Complexity | Correctness |
|----------|-------|------------|-------------|
| Full re-parse | O(n) always | Simple | Trivially correct |
| Declaration-level reuse (current) | O(k) where k = changed decls | Moderate | Correct by span isolation |
| Token-level reuse (e.g., tree-sitter) | O(log n) | Very high | Requires error-tolerant grammar |

Declaration-level reuse offers a good balance: most IDE edits touch 1-2 declarations, so the majority of the file is reused. The implementation complexity is manageable because declarations are natural isolation boundaries — a change inside a function body cannot affect the parse of sibling functions.
