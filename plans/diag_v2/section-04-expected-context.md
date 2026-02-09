---
section: "04"
title: Expected Context Encoding
status: not-started
goal: Track WHY a type is expected, enabling contextual "because..." type error messages
sections:
  - id: "04.1"
    title: Expected Enum Design
    status: not-started
  - id: "04.2"
    title: Category Enum
    status: not-started
  - id: "04.3"
    title: Type Checker Integration
    status: not-started
  - id: "04.4"
    title: Error Message Generation
    status: not-started
  - id: "04.5"
    title: Completion Checklist
    status: not-started
---

# Section 04: Expected Context Encoding

**Status:** Not Started
**Goal:** Enrich the type checker's error context with `Expected<T>` encoding that tracks *why* a type is expected. This is the **highest-impact pattern** from the reference compiler analysis — it transforms error messages from "expected X, found Y" to "expected X because of the annotation on line 3, found Y because `+` produces `float`."

**Reference compilers:**
- **Elm** `compiler/src/Type/Error.hs` — `Expected = NoExpectation | FromContext Region Category Type | FromAnnotation Name Int SubContext Type`; `Category = List | Number | If | Case | CallResult Name | Lambda | ...`
- **Roc** `crates/compiler/types/src/types.rs` — `Expected<T> = NoExpectation(T) | ForReason(Reason, T, Region) | FromAnnotation(Loc<Symbol>, usize, AnnotationSource, T)`
- **Rust** `compiler/rustc_hir_typeck/src/expectation.rs` — `Expectation::ExpectHasType(Ty)`, `ExpectCastableToType`, `ExpectRvalueLikeUnsized`

**Current state:** `ori_types` has `ErrorContext { kind: ContextKind, expected: Option<Expected>, notes: Vec<String> }` and `Expected { origin: ExpectedOrigin, ty: Idx }`. The `ExpectedOrigin` has 6 variants (`Annotation`, `ReturnType`, `IfCondition`, `OperatorResult`, `Implicit`, `Pattern`). This is a good start but needs enrichment:
1. `ExpectedOrigin` lacks source spans (can't point to the annotation that created the expectation)
2. No `Category` equivalent — can't explain *what kind of value* was inferred
3. Not propagated through all inference paths (many `unify` calls pass `None`)

---

## 04.1 Expected Enum Design

### Enriched Expected

```rust
// In ori_types/src/type_error/expected.rs (new file, extracted from check_error.rs)

/// Tracks WHY a particular type is expected at a given location.
///
/// This is the core context for producing Elm-quality error messages.
/// Instead of "expected int, found float", we can say:
/// "expected int because the function return type is int (line 3),
///  found float because `1.5` is a float literal".
///
/// Modeled after Elm's `Expected` and Roc's `Expected<T>`.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Expected {
    /// No specific expectation — the type was freely inferred.
    NoExpectation,

    /// Expected from a type annotation written by the user.
    ///
    /// `name` is the annotated binding (function, variable, field).
    /// `span` points to the annotation itself.
    /// `annotation_kind` distinguishes parameter, return, field, etc.
    FromAnnotation {
        name: Name,
        span: Span,
        annotation_kind: AnnotationKind,
    },

    /// Expected from surrounding context (not an explicit annotation).
    ///
    /// `reason` explains the contextual origin.
    /// `span` points to the source of the expectation.
    FromContext {
        reason: ExpectedReason,
        span: Span,
    },
}

/// What kind of annotation created the expectation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum AnnotationKind {
    /// A parameter type annotation: `fn foo(x: int)`
    Parameter,
    /// A return type annotation: `fn foo() -> int`
    ReturnType,
    /// A variable type annotation: `let x: int = ...`
    LetBinding,
    /// A field type in a struct/record: `type Foo = { x: int }`
    FieldType,
    /// A type alias: `type Alias = int`
    TypeAlias,
    /// A generic constraint: `fn foo(x: T) where T: Num`
    Constraint,
}

/// Contextual reason for an expectation (when not from an explicit annotation).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ExpectedReason {
    /// The if-condition must be `bool`.
    IfCondition,
    /// Both branches of an `if` must have the same type.
    IfBranches,
    /// All arms of a `match` must have the same type.
    MatchArms,
    /// Binary operator requires specific operand types.
    BinaryOp { op: Name },
    /// Unary operator requires a specific operand type.
    UnaryOp { op: Name },
    /// Function call: argument N must match parameter type.
    Argument { func_name: Name, param_index: u32 },
    /// Function call: return type flows to usage site.
    CallResult { func_name: Name },
    /// List literal: all elements must have the same type.
    ListElement { first_element_span: Span },
    /// Map literal: all keys/values must have the same type.
    MapElement,
    /// Tuple element at a specific position.
    TupleElement { index: u32 },
    /// Assignment: RHS must match LHS type.
    Assignment,
    /// Pattern: the scrutinee type determines the pattern type.
    Pattern,
    /// Range: start and end must have the same numeric type.
    RangeEndpoint,
    /// For loop: the iterable must be an iterator.
    ForIterable,
    /// The test body must return `void`.
    TestBody,
}
```

### Relationship to Existing Types

The existing `ExpectedOrigin` and `ErrorContext.expected` are replaced:

| V1 Type | V2 Replacement |
|---------|---------------|
| `ExpectedOrigin::Annotation` | `Expected::FromAnnotation { annotation_kind: * }` |
| `ExpectedOrigin::ReturnType` | `Expected::FromAnnotation { annotation_kind: ReturnType }` |
| `ExpectedOrigin::IfCondition` | `Expected::FromContext { reason: IfCondition }` |
| `ExpectedOrigin::OperatorResult` | `Expected::FromContext { reason: BinaryOp { op } }` |
| `ExpectedOrigin::Implicit` | `Expected::NoExpectation` |
| `ExpectedOrigin::Pattern` | `Expected::FromContext { reason: Pattern }` |

The key additions:
1. **Spans on every variant** — V1's `ExpectedOrigin` has no spans
2. **`AnnotationKind`** — distinguishes parameter vs return type vs let binding
3. **Richer reasons** — V1's `OperatorResult` becomes `BinaryOp { op }` with the specific operator

- [ ] Define `Expected` enum with three variants
- [ ] Define `AnnotationKind` enum (6 variants)
- [ ] Define `ExpectedReason` enum (15+ variants)
- [ ] Migration: replace `ExpectedOrigin` with `Expected`
- [ ] All types derive `Clone, Debug, Eq, PartialEq, Hash` (Salsa-compatible)
- [ ] Unit tests: construction, equality

---

## 04.2 Category Enum

### Value Category

Elm's `Category` type answers "what kind of value is this?" — the complement to `Expected` which answers "why was this type expected?":

```rust
/// Describes what kind of value produced a particular type.
///
/// Used in error messages: "this is a `float` because it's a float literal"
/// or "this is a `list(int)` because it's a list comprehension".
///
/// Modeled after Elm's `Category` type.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Category {
    /// A literal value (int, float, string, bool, char).
    Literal,
    /// A list literal or comprehension.
    List,
    /// A map literal.
    Map,
    /// A tuple literal.
    Tuple,
    /// A function definition or lambda.
    Lambda,
    /// The result of calling a named function.
    CallResult { func_name: Name },
    /// The result of an if-expression.
    If,
    /// The result of a match-expression.
    Match,
    /// The result of a for-comprehension.
    For,
    /// A field access on a struct/record.
    FieldAccess { field_name: Name },
    /// A binary operation result.
    BinaryOp { op: Name },
    /// A unary operation result.
    UnaryOp { op: Name },
    /// A block expression (value is the last expression).
    Block,
    /// An identifier reference.
    Identifier { name: Name },
    /// A type cast (`as` expression).
    Cast,
}
```

### Usage in Error Messages

The combination of `Expected` + `Category` enables messages like:

```
error[E2001]: type mismatch
  --> src/main.ori:10:5
   |
10 |     x + 1.5
   |     ^^^^^^^ this is `float` because `+` with a `float` operand produces `float`
   |
   = expected `int` because of the return type annotation on line 3
```

The rendering logic:
1. `Category` generates the "this is ... because ..." part
2. `Expected` generates the "expected ... because ..." part
3. Combined, they give full context for both sides of a mismatch

- [ ] Define `Category` enum (16 variants)
- [ ] All types derive `Clone, Debug, Eq, PartialEq, Hash`
- [ ] `describe()` method that generates human-readable text
- [ ] Unit tests: describe() for each variant

---

## 04.3 Type Checker Integration

### Propagating Expected Through Inference

The type checker must thread `Expected` through all inference paths. Currently, many `unify()` calls pass `None` for the expected context.

**Key integration points in `ori_types`:**

| Inference Site | Expected Value |
|---------------|---------------|
| Function parameter | `FromAnnotation { name: param, kind: Parameter }` |
| Function return | `FromAnnotation { name: func, kind: ReturnType }` |
| Let binding with annotation | `FromAnnotation { name: binding, kind: LetBinding }` |
| If condition | `FromContext { reason: IfCondition }` |
| If branch merge | `FromContext { reason: IfBranches }` |
| Match arm merge | `FromContext { reason: MatchArms }` |
| Binary operator | `FromContext { reason: BinaryOp { op } }` |
| Function argument | `FromContext { reason: Argument { func, index } }` |
| List element | `FromContext { reason: ListElement { first_span } }` |
| For iterable | `FromContext { reason: ForIterable }` |
| Assignment RHS | `FromContext { reason: Assignment }` |

**Migration strategy:**
1. Add `Expected` field to `TypeCheckError` (alongside existing `context: ErrorContext`)
2. Gradually replace `None` expected values in `unify()` calls with proper `Expected` values
3. Each replacement is independently testable — errors get better one at a time

### Enriching TypeCheckError

```rust
// Updated TypeCheckError
pub struct TypeCheckError {
    pub span: Span,
    pub kind: TypeErrorKind,
    pub context: ErrorContext,
    pub suggestions: Vec<Suggestion>,

    // NEW: structured expected/found context
    pub expected_ctx: Expected,     // WHY the expected type is expected
    pub found_category: Category,   // WHAT kind of value was found
}
```

The `expected_ctx` and `found_category` fields are populated at the `unify` call site, where both pieces of information are available. They flow through to the renderer without modification.

- [ ] Add `expected_ctx: Expected` field to `TypeCheckError`
- [ ] Add `found_category: Category` field to `TypeCheckError`
- [ ] Update `TypeCheckError` factory methods to accept `Expected` + `Category`
- [ ] Propagate `Expected` through top 10 highest-frequency `unify()` call sites
- [ ] Propagate `Category` through expression inference
- [ ] Tests: each integration point produces correct `Expected` value
- [ ] Tests: each expression kind produces correct `Category` value

---

## 04.4 Error Message Generation

### Rendering Expected Context

The `TypeErrorRenderer` uses `Expected` and `Category` to generate contextual messages:

```rust
impl TypeErrorRenderer<'_> {
    fn render_expected_context(&self, expected: &Expected) -> Option<String> {
        match expected {
            Expected::NoExpectation => None,
            Expected::FromAnnotation { name, annotation_kind, .. } => {
                let name_str = self.format_name(*name);
                Some(match annotation_kind {
                    AnnotationKind::Parameter =>
                        format!("expected because parameter `{name_str}` has this type"),
                    AnnotationKind::ReturnType =>
                        format!("expected because the return type of `{name_str}` is declared as this"),
                    AnnotationKind::LetBinding =>
                        format!("expected because `{name_str}` is annotated with this type"),
                    AnnotationKind::FieldType =>
                        format!("expected because field `{name_str}` has this type"),
                    _ => format!("expected from annotation on `{name_str}`"),
                })
            }
            Expected::FromContext { reason, .. } => {
                Some(match reason {
                    ExpectedReason::IfCondition =>
                        "expected `bool` because this is an `if` condition".to_string(),
                    ExpectedReason::IfBranches =>
                        "expected because both branches of `if` must have the same type".to_string(),
                    ExpectedReason::MatchArms =>
                        "expected because all `match` arms must have the same type".to_string(),
                    ExpectedReason::BinaryOp { op } =>
                        format!("expected because `{}` requires operands of the same type",
                            self.format_name(*op)),
                    ExpectedReason::Argument { func_name, param_index } =>
                        format!("expected because argument {} of `{}` has this type",
                            param_index + 1, self.format_name(*func_name)),
                    // ... other reasons
                    _ => "expected from context".to_string(),
                })
            }
        }
    }

    fn render_found_category(&self, category: &Category) -> Option<String> {
        match category {
            Category::Literal => Some("this is a literal value".to_string()),
            Category::CallResult { func_name } =>
                Some(format!("this is the result of calling `{}`",
                    self.format_name(*func_name))),
            Category::BinaryOp { op } =>
                Some(format!("this is the result of the `{}` operation",
                    self.format_name(*op))),
            Category::FieldAccess { field_name } =>
                Some(format!("this is the field `{}`",
                    self.format_name(*field_name))),
            _ => None, // Not all categories need explicit explanation
        }
    }
}
```

### Example Output

```
error[E2001]: type mismatch
  --> src/main.ori:10:5
   |
10 |     x + 1.5
   |     ^^^^^^^ expected `int`, found `float`
   |
   = note: expected `int` because parameter `x` has this type
    --> src/main.ori:3:10
     |
   3 | fn add(x: int) -> int =
     |           ^^^ type annotation here
   = note: found `float` because `1.5` is a float literal
   = help: try using `x as float + 1.5` to convert `x` to `float`
```

### ExplanationChain Integration

When an `Expected` has a span, the renderer generates an `ExplanationChain` (from Section 02):

```rust
fn build_expected_chain(&self, expected: &Expected) -> Option<ExplanationChain> {
    match expected {
        Expected::FromAnnotation { name, span, annotation_kind } => {
            Some(ExplanationChain::new(
                self.render_expected_context(expected).unwrap()
            ).with_span(*span))
        }
        Expected::FromContext { reason, span } => {
            Some(ExplanationChain::new(
                self.render_expected_context(expected).unwrap()
            ).with_span(*span))
        }
        Expected::NoExpectation => None,
    }
}
```

- [ ] Implement `render_expected_context()` for all `Expected` variants
- [ ] Implement `render_found_category()` for all `Category` variants
- [ ] Implement `build_expected_chain()` for ExplanationChain integration
- [ ] Update `TypeErrorRenderer::render()` to use new context
- [ ] Tests: error messages include "because..." context
- [ ] Tests: ExplanationChain is generated with correct spans
- [ ] End-to-end tests: type errors produce contextual messages

---

## 04.5 Completion Checklist

- [ ] `Expected` enum defined (3 variants, all with spans)
- [ ] `AnnotationKind` enum defined (6 variants)
- [ ] `ExpectedReason` enum defined (15+ variants)
- [ ] `Category` enum defined (16 variants)
- [ ] `TypeCheckError` extended with `expected_ctx` and `found_category`
- [ ] Top 10 `unify()` call sites propagate `Expected`
- [ ] Expression inference propagates `Category`
- [ ] `TypeErrorRenderer` generates contextual messages
- [ ] ExplanationChain integration
- [ ] Migration from `ExpectedOrigin` complete
- [ ] Tests: 20+ unit tests for Expected/Category
- [ ] Tests: 10+ integration tests for error messages
- [ ] `./test-all.sh` passes

**Exit Criteria:** Type mismatch errors include "because..." context explaining both *why* the expected type is expected and *what* produced the found type. The type checker propagates `Expected` and `Category` through all major inference paths.
