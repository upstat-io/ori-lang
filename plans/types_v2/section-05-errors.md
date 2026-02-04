---
section: "05"
title: Context-Aware Errors
status: not-started
goal: Elm-quality error messages with rich context tracking
sections:
  - id: "05.1"
    title: Expected Type Tracking
    status: not-started
  - id: "05.2"
    title: ContextKind Enum
    status: not-started
  - id: "05.3"
    title: TypeProblem Identification
    status: not-started
  - id: "05.4"
    title: Type Diffing
    status: not-started
  - id: "05.5"
    title: Suggestion Generation
    status: not-started
  - id: "05.6"
    title: TypeCheckError Structure
    status: not-started
---

# Section 05: Context-Aware Errors

**Status:** Not Started
**Goal:** Elm-quality error messages with rich context tracking
**Source:** Elm (`Reporting/Error/Type.hs`), Gleam (`parse/error.rs`)

---

## Background

### Current Problems

1. Errors lose context — "expected int, found str" without WHERE
2. No tracking of WHY we expected a certain type
3. Generic errors don't identify specific problems
4. No actionable suggestions

### Elm's Approach

Every type expectation carries its origin, enabling messages like:
```
I ran into a type mismatch when checking the 2nd argument of `add`:

    add(1, "two")
           ^^^^^

I was expecting an `int`, but found a `str`.
```

---

## 05.1 Expected Type Tracking

**Goal:** Track why we expect each type

### Design

```rust
/// A type expectation with its origin context.
#[derive(Clone, Debug)]
pub struct Expected {
    /// The expected type.
    pub ty: Idx,
    /// Why we expect this type.
    pub origin: ExpectedOrigin,
}

#[derive(Clone, Debug)]
pub enum ExpectedOrigin {
    /// No specific expectation (inference determines).
    NoExpectation,

    /// From a type annotation in source.
    Annotation {
        name: Name,
        span: Span,
    },

    /// From surrounding context.
    Context {
        span: Span,
        kind: ContextKind,
    },

    /// From a previous element in a sequence.
    PreviousInSequence {
        previous_span: Span,
        current_index: usize,
        sequence_kind: SequenceKind,
    },
}

#[derive(Copy, Clone, Debug)]
pub enum SequenceKind {
    ListLiteral,
    MatchArms,
    IfBranches,
    TupleElements,
}
```

### Tasks

- [ ] Create `ori_types/src/error/expected.rs`
- [ ] Define `Expected` struct
- [ ] Define `ExpectedOrigin` enum with all variants
- [ ] Define `SequenceKind` enum

---

## 05.2 ContextKind Enum

**Goal:** Classify all contexts where types are expected

### Design

```rust
/// The kind of context that created a type expectation.
#[derive(Clone, Debug)]
pub enum ContextKind {
    // === Literals ===
    ListElement { index: usize },
    MapKey,
    MapValue,
    TupleElement { index: usize },
    SetElement,

    // === Control Flow ===
    IfCondition,
    IfThenBranch,
    IfElseBranch { branch_index: usize },
    MatchScrutinee,
    MatchArm { arm_index: usize },
    MatchArmPattern { arm_index: usize },
    MatchArmGuard { arm_index: usize },
    LoopCondition,
    LoopBody,

    // === Functions ===
    FunctionArgument {
        func_name: Option<Name>,
        arg_index: usize,
        param_name: Option<Name>,
    },
    FunctionReturn { func_name: Option<Name> },
    LambdaBody,
    LambdaParameter { index: usize },
    LambdaReturn,

    // === Operators ===
    BinaryOpLeft { op: BinaryOp },
    BinaryOpRight { op: BinaryOp },
    UnaryOpOperand { op: UnaryOp },
    PipelineInput,
    PipelineOutput,
    ComparisonLeft,
    ComparisonRight,

    // === Records/Structs ===
    FieldAccess { field_name: Name },
    FieldAssignment { field_name: Name },
    StructField { struct_name: Name, field_name: Name },
    RecordUpdate { field_name: Name },
    StructConstruction { struct_name: Name },

    // === Patterns ===
    PatternBinding { pattern_kind: &'static str },
    PatternMatch { pattern_kind: &'static str },
    Destructure,
    RangeStart,
    RangeEnd,

    // === Special ===
    CapabilityRequirement { capability: Name },
    PreCheck,
    PostCheck,
    TestBody,
    TestAssertion,
    Assignment,
    IndexOperation,
}
```

### Tasks

- [ ] Create `ori_types/src/error/context.rs`
- [ ] Define `ContextKind` with all 30+ variants
- [ ] Add `ContextKind::describe(&self) -> String` method
- [ ] Add tests for context descriptions

---

## 05.3 TypeProblem Identification

**Goal:** Identify specific problems, not just "mismatch"

### Design

```rust
/// A specific problem identified by comparing two types.
#[derive(Clone, Debug)]
pub enum TypeProblem {
    // === Numeric Problems ===
    IntFloat,
    NumberToString,
    StringToNumber,

    // === Collection Problems ===
    ExpectedList { found: &'static str },
    ListElementMismatch,
    ExpectedOption,
    NeedsUnwrap,
    WrongCollectionType { expected: &'static str, found: &'static str },

    // === Function Problems ===
    WrongArity { expected: usize, found: usize },
    ArgumentMismatch { arg_index: usize, expected: Idx, found: Idx },
    ReturnMismatch,
    NotCallable,
    MissingArguments { missing: Vec<Name> },
    ExtraArguments { count: usize },

    // === Record/Struct Problems ===
    MissingField { field_name: Name, available: Vec<Name> },
    ExtraField { field_name: Name },
    FieldTypeMismatch { field_name: Name },
    FieldTypo { attempted: Name, suggestion: Name, distance: usize },
    WrongRecordType { expected: Name, found: Name },

    // === Type Variable Problems ===
    RigidMismatch { rigid_name: Name },
    InfiniteType,
    EscapingVariable { var_name: Option<Name> },

    // === Capability Problems ===
    MissingCapability { required: Name },
    CapabilityConflict { provided: Name, required: Name },

    // === Generic Fallback ===
    TypeMismatch {
        expected_category: &'static str,
        found_category: &'static str,
    },
}
```

### Tasks

- [ ] Create `ori_types/src/error/problem.rs`
- [ ] Define `TypeProblem` with all 25+ variants
- [ ] Add `TypeProblem::severity(&self) -> Severity` method
- [ ] Add `TypeProblem::suggestion(&self) -> Option<String>` method

---

## 05.4 Type Diffing

**Goal:** Compare types and identify specific problems

### Design

```rust
impl Pool {
    /// Compare two types and identify specific problems.
    pub fn diff_types(&self, expected: Idx, found: Idx) -> Vec<TypeProblem> {
        let mut problems = Vec::new();

        let exp_tag = self.tag(expected);
        let found_tag = self.tag(found);

        match (exp_tag, found_tag) {
            // Int vs Float
            (Tag::Int, Tag::Float) | (Tag::Float, Tag::Int) => {
                problems.push(TypeProblem::IntFloat);
            }

            // String vs Number
            (Tag::Str, Tag::Int | Tag::Float) => {
                problems.push(TypeProblem::NumberToString);
            }
            (Tag::Int | Tag::Float, Tag::Str) => {
                problems.push(TypeProblem::StringToNumber);
            }

            // List mismatches
            (Tag::List, other) if other != Tag::List => {
                problems.push(TypeProblem::ExpectedList {
                    found: self.tag_name(other),
                });
            }

            // Option mismatches - check if needs unwrap
            (Tag::Option, other) if other != Tag::Option => {
                let inner = Idx(self.data(expected));
                if self.types_structurally_equal(inner, found) {
                    problems.push(TypeProblem::NeedsUnwrap);
                } else {
                    problems.push(TypeProblem::ExpectedOption);
                }
            }

            // Function arity
            (Tag::Function, Tag::Function) => {
                let exp_params = self.function_params(expected);
                let found_params = self.function_params(found);
                if exp_params.len() != found_params.len() {
                    problems.push(TypeProblem::WrongArity {
                        expected: exp_params.len(),
                        found: found_params.len(),
                    });
                }
            }

            // Not a function
            (Tag::Function, _) => {
                problems.push(TypeProblem::NotCallable);
            }

            // Struct field mismatches
            (Tag::Struct, Tag::Struct) => {
                self.diff_struct_fields(expected, found, &mut problems);
            }

            _ => {
                problems.push(TypeProblem::TypeMismatch {
                    expected_category: self.type_category(expected),
                    found_category: self.type_category(found),
                });
            }
        }

        problems
    }
}
```

### Tasks

- [ ] Create `ori_types/src/error/diff.rs`
- [ ] Implement `diff_types()` for all tag combinations
- [ ] Implement `diff_struct_fields()` for structural comparison
- [ ] Add edit distance for typo detection
- [ ] Add tests for each diff case

---

## 05.5 Suggestion Generation

**Goal:** Generate actionable suggestions for each problem

### Design

```rust
#[derive(Clone, Debug)]
pub struct Suggestion {
    pub message: String,
    pub replacement: Option<Replacement>,
    pub priority: u8,
}

#[derive(Clone, Debug)]
pub struct Replacement {
    pub span: Span,
    pub new_text: String,
}

impl TypeProblem {
    pub fn suggestions(&self, pool: &Pool) -> Vec<Suggestion> {
        match self {
            TypeProblem::IntFloat => vec![
                Suggestion {
                    message: "Use `to_float()` to convert int to float".to_string(),
                    replacement: None,
                    priority: 1,
                },
                Suggestion {
                    message: "Use `to_int()` to convert float to int (truncates)".to_string(),
                    replacement: None,
                    priority: 2,
                },
            ],

            TypeProblem::NeedsUnwrap => vec![
                Suggestion {
                    message: "Use `?` to propagate none".to_string(),
                    replacement: None,
                    priority: 1,
                },
                Suggestion {
                    message: "Use `match` to handle both cases".to_string(),
                    replacement: None,
                    priority: 2,
                },
            ],

            TypeProblem::FieldTypo { attempted, suggestion, .. } => vec![
                Suggestion {
                    message: format!(
                        "Did you mean `{}`?",
                        pool.strings.resolve(*suggestion)
                    ),
                    replacement: None, // Could add replacement with span
                    priority: 0,
                },
            ],

            TypeProblem::MissingField { field_name, available } => vec![
                Suggestion {
                    message: format!(
                        "Add the missing field `{}`",
                        pool.strings.resolve(*field_name)
                    ),
                    replacement: None,
                    priority: 1,
                },
            ],

            // ... other problems
            _ => vec![],
        }
    }
}
```

### Tasks

- [ ] Create `ori_types/src/error/suggest.rs`
- [ ] Define `Suggestion` and `Replacement` types
- [ ] Implement suggestions for all problem types
- [ ] Add priority-based sorting

---

## 05.6 TypeCheckError Structure

**Goal:** Define the comprehensive error type

### Design

```rust
/// A type checking error with full context.
#[derive(Clone, Debug)]
pub struct TypeCheckError {
    pub span: Span,
    pub code: ErrorCode,
    pub kind: TypeErrorKind,
    pub context: ErrorContext,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Clone, Debug)]
pub enum TypeErrorKind {
    Mismatch {
        expected: Idx,
        found: Idx,
        problems: Vec<TypeProblem>,
    },

    UnknownIdent {
        name: Name,
        similar: Vec<Name>,
    },

    UndefinedField {
        ty: Idx,
        field: Name,
        available: Vec<Name>,
    },

    ArityMismatch {
        expected: usize,
        found: usize,
        kind: ArityKind,
    },

    MissingCapability {
        required: Name,
        available: Vec<Name>,
    },

    InfiniteType {
        var_name: Option<Name>,
    },

    AmbiguousType {
        var_id: u32,
        context: String,
    },

    // ... more variants as needed
}

#[derive(Clone, Debug)]
pub struct ErrorContext {
    pub checking: ContextKind,
    pub expected_because: ExpectedOrigin,
    pub notes: Vec<String>,
}
```

### Tasks

- [ ] Create `ori_types/src/error/mod.rs`
- [ ] Define `TypeCheckError` with all fields
- [ ] Define `TypeErrorKind` with all variants
- [ ] Define `ErrorContext` for rich context
- [ ] Implement `to_diagnostic()` for rendering

---

## 05.7 Error Message Formatting

**Goal:** Generate user-friendly error messages

### Design

```rust
impl TypeCheckError {
    pub fn to_diagnostic(&self, pool: &Pool) -> Diagnostic {
        let mut diag = Diagnostic::error()
            .with_code(self.code)
            .with_span(self.span);

        let message = self.format_message(pool);
        diag = diag.with_message(message);

        // Add context note
        if let Some(context_note) = self.format_context(pool) {
            diag = diag.with_note(context_note);
        }

        // Add suggestions
        for suggestion in &self.suggestions {
            diag = diag.with_suggestion(&suggestion.message);
        }

        diag
    }

    fn format_message(&self, pool: &Pool) -> String {
        match &self.kind {
            TypeErrorKind::Mismatch { expected, found, problems } => {
                let mut msg = format!(
                    "expected `{}`, found `{}`",
                    pool.format_type(*expected),
                    pool.format_type(*found),
                );

                for problem in problems {
                    if let Some(hint) = problem.hint() {
                        msg.push_str("\n\n");
                        msg.push_str(&hint);
                    }
                }

                msg
            }
            // ... other kinds
            _ => "Type error".to_string(),
        }
    }

    fn format_context(&self, pool: &Pool) -> Option<String> {
        Some(self.context.checking.describe(pool))
    }
}
```

### Tasks

- [ ] Create `ori_types/src/error/format.rs`
- [ ] Implement `to_diagnostic()` for all error kinds
- [ ] Implement context formatting
- [ ] Add tests for error message quality

---

## 05.8 Completion Checklist

- [ ] `Expected` and `ExpectedOrigin` complete
- [ ] `ContextKind` with 30+ variants
- [ ] `TypeProblem` with 25+ specific problems
- [ ] Type diffing working for all combinations
- [ ] Suggestions generated for each problem
- [ ] `TypeCheckError` with full context
- [ ] Error messages rendering correctly
- [ ] Quality comparable to Elm errors (subjective)

**Exit Criteria:** Error messages include rich context ("in argument 2 of call to `add`"), identify specific problems ("int vs float"), and provide actionable suggestions ("use `to_float()` to convert").
