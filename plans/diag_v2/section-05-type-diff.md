---
section: "05"
title: Structural Type Diffing
status: not-started
goal: Recursive structural type comparison that highlights WHERE two types diverge
sections:
  - id: "05.1"
    title: Diff Algorithm
    status: not-started
  - id: "05.2"
    title: Diff Result Types
    status: not-started
  - id: "05.3"
    title: Doc Integration
    status: not-started
  - id: "05.4"
    title: Rendering
    status: not-started
  - id: "05.5"
    title: Completion Checklist
    status: not-started
---

# Section 05: Structural Type Diffing

**Status:** Not Started
**Goal:** Implement recursive structural type comparison that highlights exactly WHERE two types diverge, producing annotated document trees for rendering. This transforms "expected `fn(int, str, bool) -> Result(Data, Error)`, found `fn(int, str, float) -> Result(Data, Error)`" into a visual diff that highlights only `bool` vs `float`.

**Reference compilers:**
- **Roc** `crates/reporting/src/error/type.rs` — `to_diff(alloc, pool, expected, found)` recurses into type structure, extracts `Problem` during rendering, produces `Diff { left, right, status }` where `left`/`right` are `RocDoc` trees with `Annotation::Expected`/`Annotation::Found` on divergent parts
- **Elm** `compiler/src/Reporting/Error/Type.hs` — `toComparison` recursively diffs types, producing aligned doc trees; handles "same prefix, different suffix" patterns
- **Rust** `compiler/rustc_infer/src/infer/error_reporting/` — Type diff with highlighting, but less structured than Roc

**Current state:** Type mismatches use `format_type(idx)` to render full types as strings. No structural comparison — the user must visually diff the "expected" and "found" types themselves. For complex types like `fn(A, B, C, D) -> E`, finding the one differing parameter is tedious.

---

## 05.1 Diff Algorithm

### Recursive Structural Diff

The diff algorithm walks two types in parallel, identifying where they diverge:

```rust
// In ori_types/src/type_diff.rs (new file)

/// Compare two types structurally and produce a diff result.
///
/// Walks both types in parallel. Where they match, produces `Same` nodes.
/// Where they diverge, produces `Different` nodes with the expected and
/// found sub-types annotated.
///
/// # Arguments
/// * `pool` — Type pool for resolving type indices
/// * `expected` — The expected type (from annotation or context)
/// * `found` — The actual inferred type
///
/// # Returns
/// A `TypeDiff` tree that can be rendered to highlighted output.
pub fn diff_types(pool: &Pool, expected: Idx, found: Idx) -> TypeDiff
```

### Core Algorithm

```rust
fn diff_inner(pool: &Pool, expected: Idx, found: Idx) -> TypeDiff {
    // 1. Resolve aliases/newtypes
    let expected = pool.resolve(expected);
    let found = pool.resolve(found);

    // 2. If structurally identical, return Same
    if pool.types_equal(expected, found) {
        return TypeDiff::Same(expected);
    }

    // 3. If same top-level constructor, recurse into sub-components
    let e_tag = pool.tag(expected);
    let f_tag = pool.tag(found);

    if e_tag == f_tag {
        match e_tag {
            Tag::Function => diff_function(pool, expected, found),
            Tag::List => diff_list(pool, expected, found),
            Tag::Map => diff_map(pool, expected, found),
            Tag::Set => diff_set(pool, expected, found),
            Tag::Option => diff_option(pool, expected, found),
            Tag::Result => diff_result(pool, expected, found),
            Tag::Tuple => diff_tuple(pool, expected, found),
            Tag::Struct => diff_struct(pool, expected, found),
            Tag::Enum => diff_enum(pool, expected, found),
            _ => TypeDiff::Different { expected, found },
        }
    } else {
        // 4. Different constructors — leaf difference
        TypeDiff::Different { expected, found }
    }
}

fn diff_function(pool: &Pool, expected: Idx, found: Idx) -> TypeDiff {
    let e_params = pool.function_params(expected);
    let f_params = pool.function_params(found);
    let e_ret = pool.function_return(expected);
    let f_ret = pool.function_return(found);

    // Diff parameters positionally
    let param_diffs: Vec<TypeDiff> = e_params.iter().zip(f_params.iter())
        .map(|(&e, &f)| diff_inner(pool, e, f))
        .collect();

    // Handle arity mismatch
    let extra_expected = if e_params.len() > f_params.len() {
        e_params[f_params.len()..].iter()
            .map(|&p| TypeDiff::Missing { ty: p, side: Side::Found })
            .collect()
    } else { vec![] };

    let extra_found = if f_params.len() > e_params.len() {
        f_params[e_params.len()..].iter()
            .map(|&p| TypeDiff::Missing { ty: p, side: Side::Expected })
            .collect()
    } else { vec![] };

    // Diff return type
    let ret_diff = diff_inner(pool, e_ret, f_ret);

    TypeDiff::Function {
        params: param_diffs,
        extra_expected,
        extra_found,
        ret: Box::new(ret_diff),
    }
}
```

**Design decisions:**
- **Resolve aliases first** — diff operates on resolved types, not surface syntax
- **Same-constructor recursion** — only recurse when top-level constructors match
- **Positional parameter matching** — function parameters are diffed by position, not by name
- **Arity mismatch handling** — extra parameters are marked as `Missing` on the appropriate side

- [ ] Implement `diff_types(pool, expected, found) -> TypeDiff`
- [ ] Implement `diff_inner` recursive core
- [ ] Implement `diff_function` for function type diffs
- [ ] Implement `diff_list`, `diff_map`, `diff_option`, `diff_result`
- [ ] Implement `diff_tuple` for tuple diffs (positional)
- [ ] Implement `diff_struct` for struct diffs (by field name)
- [ ] Handle arity mismatches in function and tuple diffs
- [ ] Unit tests: identical types → `Same`
- [ ] Unit tests: leaf differences → `Different`
- [ ] Unit tests: nested differences (e.g., `fn(int) -> float` vs `fn(int) -> int`)
- [ ] Unit tests: arity mismatches

---

## 05.2 Diff Result Types

### TypeDiff Enum

```rust
/// The result of structurally comparing two types.
///
/// Each variant represents a different kind of relationship between
/// the expected and found types at that position in the type tree.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum TypeDiff {
    /// Types are structurally identical at this position.
    Same(Idx),

    /// Types differ at this leaf position.
    Different {
        expected: Idx,
        found: Idx,
    },

    /// A sub-component is missing on one side (arity mismatch).
    Missing {
        ty: Idx,
        side: Side,
    },

    /// Function types with per-component diffs.
    Function {
        params: Vec<TypeDiff>,
        extra_expected: Vec<TypeDiff>,
        extra_found: Vec<TypeDiff>,
        ret: Box<TypeDiff>,
    },

    /// Container types (list, set, option) with element diff.
    Container {
        constructor: ContainerKind,
        element: Box<TypeDiff>,
    },

    /// Map type with key and value diffs.
    MapDiff {
        key: Box<TypeDiff>,
        value: Box<TypeDiff>,
    },

    /// Result type with ok and err diffs.
    ResultDiff {
        ok: Box<TypeDiff>,
        err: Box<TypeDiff>,
    },

    /// Tuple type with per-element diffs.
    TupleDiff {
        elements: Vec<TypeDiff>,
    },

    /// Struct type with per-field diffs.
    StructDiff {
        matching: Vec<(Name, TypeDiff)>,
        only_expected: Vec<(Name, Idx)>,
        only_found: Vec<(Name, Idx)>,
    },
}

/// Which side is missing a component.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Side {
    Expected,
    Found,
}

/// Kind of single-element container.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ContainerKind {
    List,
    Set,
    Option,
}
```

### Utility Methods

```rust
impl TypeDiff {
    /// Check if the diff is trivially "same" (no differences).
    pub fn is_same(&self) -> bool {
        matches!(self, TypeDiff::Same(_))
    }

    /// Check if there are any differences in this tree.
    pub fn has_differences(&self) -> bool {
        !self.is_same()
    }

    /// Count the number of leaf differences.
    pub fn difference_count(&self) -> usize { /* recursive count */ }

    /// Check if the diff is "simple" (single leaf difference).
    /// Simple diffs can use a compact rendering.
    pub fn is_simple(&self) -> bool {
        self.difference_count() == 1
    }
}
```

- [ ] Define `TypeDiff` enum with all variants
- [ ] Define `Side` and `ContainerKind` enums
- [ ] Utility methods: `is_same()`, `has_differences()`, `difference_count()`, `is_simple()`
- [ ] All types derive `Clone, Debug, Eq, PartialEq, Hash`
- [ ] Unit tests: utility methods

---

## 05.3 Doc Integration

### Converting TypeDiff to Doc

The diff result is converted to a `Doc` tree (from Section 03) with `DiffAdd`/`DiffRemove` annotations on divergent parts:

```rust
// In ori_types/src/type_diff.rs or a new type_diff_render.rs

/// Render a type diff as a pair of Doc trees (expected side, found side).
///
/// Matching parts render with `Annotation::TypeName` (neutral).
/// Divergent parts render with `Annotation::Expected` / `Annotation::Found`
/// and additionally `Annotation::DiffRemove` / `Annotation::DiffAdd`.
pub fn render_diff(
    diff: &TypeDiff,
    pool: &Pool,
    interner: &StringInterner,
) -> DiffDocs {
    match diff {
        TypeDiff::Same(idx) => {
            let text = format_type(pool, interner, *idx);
            let doc = Doc::type_name(text);
            DiffDocs { expected: doc.clone(), found: doc }
        }
        TypeDiff::Different { expected, found } => {
            let e_text = format_type(pool, interner, *expected);
            let f_text = format_type(pool, interner, *found);
            DiffDocs {
                expected: Doc::expected(Doc::type_name(e_text)),
                found: Doc::found(Doc::type_name(f_text)),
            }
        }
        TypeDiff::Function { params, ret, .. } => {
            // Build "fn(p1, p2, p3) -> ret" with per-component annotations
            render_function_diff(params, ret, pool, interner)
        }
        // ... other variants
    }
}

/// A pair of documents for the expected and found sides of a diff.
pub struct DiffDocs {
    pub expected: Doc,
    pub found: Doc,
}
```

### Example Rendering

For `fn(int, str, bool) -> Result(Data, Error)` vs `fn(int, str, float) -> Result(Data, Error)`:

```
Expected: fn(int, str, [bool]) -> Result(Data, Error)
Found:    fn(int, str, [float]) -> Result(Data, Error)
                       ^^^^^          (highlighted in red/green)
```

The `[bool]` and `[float]` parts are wrapped in `DiffRemove`/`DiffAdd` annotations, while the matching `fn(int, str, ` and `) -> Result(Data, Error)` parts are neutral `TypeName`.

For simple differences (single leaf), a compact format is used:

```
expected `bool`, found `float` (in parameter 3 of `fn(int, str, _) -> Result(Data, Error)`)
```

- [ ] Implement `render_diff(diff, pool, interner) -> DiffDocs`
- [ ] Implement per-variant rendering (function, container, struct, etc.)
- [ ] Compact format for simple (single-leaf) differences
- [ ] Expanded format for complex (multi-leaf) differences
- [ ] Tests: simple diff rendering
- [ ] Tests: complex diff rendering with aligned output

---

## 05.4 Rendering

### Terminal Rendering of Type Diffs

When a type diff is available, the terminal emitter renders an aligned comparison:

```
error[E2001]: type mismatch
  --> src/main.ori:10:5
   |
10 |     process(data)
   |     ^^^^^^^^^^^^^ type mismatch in return value
   |
   expected: fn(int, str, bool)  -> Result(Data, Error)
   found:    fn(int, str, float) -> Result(Data, Error)
                          ^^^^^
   |
   = note: the third parameter differs: expected `bool`, found `float`
```

### Integration with TypeErrorRenderer

```rust
// In oric/src/reporting/type_errors.rs

impl TypeErrorRenderer<'_> {
    fn render_type_mismatch(
        &self,
        expected: Idx,
        found: Idx,
        span: Span,
        // ... other context
    ) -> Diagnostic {
        let diff = diff_types(self.pool, expected, found);

        if diff.is_simple() {
            // Simple case: use compact inline format
            self.render_simple_mismatch(expected, found, span)
        } else {
            // Complex case: use aligned diff format
            let diff_docs = render_diff(&diff, self.pool, self.interner);
            Diagnostic::error(ErrorCode::E2001)
                .with_message("type mismatch")
                .with_label(span, "type mismatch here")
                .with_rich_message(
                    Doc::text("expected: ")
                        .append(diff_docs.expected)
                        .append(Doc::line())
                        .append(Doc::text("found:    "))
                        .append(diff_docs.found)
                )
                .with_diff_notes(&diff, self.pool, self.interner)
        }
    }
}
```

### Diff Notes

For each leaf difference, generate a note explaining the specific divergence:

```rust
fn with_diff_notes(
    mut self,
    diff: &TypeDiff,
    pool: &Pool,
    interner: &StringInterner,
) -> Self {
    let differences = collect_leaf_differences(diff);
    for (path, expected, found) in differences {
        self = self.with_note(format!(
            "{}: expected `{}`, found `{}`",
            path.describe(pool, interner),
            format_type(pool, interner, expected),
            format_type(pool, interner, found),
        ));
    }
    self
}
```

- [ ] Terminal emitter renders aligned type diffs
- [ ] `TypeErrorRenderer` uses diff for complex mismatches
- [ ] Per-difference notes generated
- [ ] Compact format for simple mismatches
- [ ] Tests: terminal output for simple and complex diffs
- [ ] Tests: note generation for leaf differences

---

## 05.5 Completion Checklist

- [ ] `ori_types/src/type_diff.rs` module created
- [ ] `diff_types()` recursive algorithm
- [ ] All type constructors handled (function, list, map, tuple, struct, etc.)
- [ ] `TypeDiff` enum with all variants
- [ ] `render_diff()` produces `DiffDocs` with semantic annotations
- [ ] Compact format for single-leaf differences
- [ ] Expanded format with alignment for multi-leaf differences
- [ ] Terminal emitter renders diff output
- [ ] `TypeErrorRenderer` integrated
- [ ] Tests: 25+ unit tests for diff algorithm
- [ ] Tests: 10+ rendering tests
- [ ] Tests: 5+ end-to-end tests in `tests/spec/`
- [ ] `./test-all.sh` passes

**Exit Criteria:** Type mismatch errors for complex types (functions, nested generics, structs) highlight exactly which sub-component differs, using aligned visual diff output. Simple mismatches (leaf types) use the existing compact format.
