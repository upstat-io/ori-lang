---
section: "06"
title: Type Inference Engine
status: in-progress
goal: Expression-level Hindley-Milner type inference
sections:
  - id: "06.1"
    title: InferEngine Structure
    status: complete
  - id: "06.2"
    title: Literal Inference
    status: complete
  - id: "06.3"
    title: Identifier Lookup
    status: complete
  - id: "06.4"
    title: Function Call Inference
    status: complete
  - id: "06.5"
    title: Operator Inference
    status: complete
  - id: "06.6"
    title: Control Flow
    status: complete
    note: break value tracking implemented via loop_break_types
  - id: "06.7"
    title: Lambda Inference
    status: complete
  - id: "06.8"
    title: Pattern Expression Inference
    status: complete
  - id: "06.9"
    title: Unit Tests
    status: complete
---

# Section 06: Type Inference Engine

**Status:** In Progress (~90% complete)
**Goal:** Expression-level Hindley-Milner type inference with bidirectional checking
**Source:** All analyzed compilers, current Ori implementation

## Implementation Status (2026-02-04)

Created `compiler/ori_types/src/infer/expr.rs` with expression dispatch for all 50+ `ExprKind` variants:

**Complete:**
- All literal inference (Int, Float, Bool, Str, Char, Byte, Unit)
- Identifier lookup with environment binding
- Binary operators (arithmetic, comparison, boolean, range)
- Unary operators (Not, Neg, BitNot, Try)
- Collection literals (List, Tuple, Map, Set, Range)
- If/else expression with branch unification
- Lambda/closure inference with scope management
- Function calls with arity checking
- Block expressions with statement processing
- **Match expressions** with full pattern checking (2026-02-04)
- **For loops** with iterator element type extraction (2026-02-04)
- **Loop (infinite)** inference (2026-02-04)

**Stub implementations (marked TODO):**
- Method calls and field access (needs Section 07 Registries)
- Named calls and method call named
- Pattern expressions (FunctionSeq, FunctionExp)
- ParsedType → Idx conversion

---

## 06.1 InferEngine Structure

**Goal:** Define the main inference engine

### Design

```rust
/// The type inference engine.
pub struct InferEngine<'a> {
    /// The type pool.
    pool: &'a mut Pool,
    /// The unification engine.
    unify: UnifyEngine<'a>,
    /// Type environment (name -> type scheme).
    env: TypeEnv,
    /// Expression arena reference.
    arena: &'a ExprArena,
    /// Inferred types for expressions.
    expr_types: FxHashMap<ExprId, Idx>,
    /// Error context stack.
    context_stack: Vec<ContextKind>,
    /// Accumulated errors.
    errors: Vec<TypeCheckError>,
}

impl<'a> InferEngine<'a> {
    pub fn new(
        pool: &'a mut Pool,
        arena: &'a ExprArena,
    ) -> Self {
        Self {
            unify: UnifyEngine::new(pool),
            pool,
            env: TypeEnv::new(),
            arena,
            expr_types: FxHashMap::default(),
            context_stack: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Infer the type of an expression.
    pub fn infer(&mut self, expr_id: ExprId) -> Idx {
        let expr = &self.arena[expr_id];
        let ty = self.infer_inner(expr_id, expr);
        self.expr_types.insert(expr_id, ty);
        ty
    }

    /// Check an expression against an expected type.
    pub fn check(&mut self, expr_id: ExprId, expected: Expected) -> Idx {
        let inferred = self.infer(expr_id);
        if let Err(e) = self.unify.unify(inferred, expected.ty) {
            self.report_mismatch(expr_id, expected, inferred, e);
        }
        inferred
    }
}
```

### Tasks

- [x] Create `ori_types/src/infer/mod.rs` (location changed from ori_typeck)
- [x] Define `InferEngine` struct
- [x] Implement `infer()` and `check()` entry points
- [x] Add context stack management
- [x] Add error accumulation
- [x] Create `infer_expr()` dispatch function for all ExprKind variants

---

## 06.2 Literal Inference

**Goal:** Infer types of literal expressions

### Design

```rust
impl<'a> InferEngine<'a> {
    fn infer_inner(&mut self, expr_id: ExprId, expr: &Expr) -> Idx {
        match &expr.kind {
            ExprKind::Int(_) => Idx::INT,
            ExprKind::Float(_) => Idx::FLOAT,
            ExprKind::Bool(_) => Idx::BOOL,
            ExprKind::Str(_) => Idx::STR,
            ExprKind::Char(_) => Idx::CHAR,
            ExprKind::Unit => Idx::UNIT,

            ExprKind::List { elements } => self.infer_list(expr_id, elements),
            ExprKind::Tuple { elements } => self.infer_tuple(expr_id, elements),
            ExprKind::Map { entries } => self.infer_map(expr_id, entries),

            // ... other expression kinds
            _ => self.infer_complex(expr_id, expr),
        }
    }

    fn infer_list(&mut self, expr_id: ExprId, elements: &[ExprId]) -> Idx {
        if elements.is_empty() {
            let elem_ty = self.unify.fresh_var();
            return self.pool.list(elem_ty);
        }

        // Infer first element
        let first_ty = self.infer(elements[0]);

        // Check remaining elements against first
        for (i, &elem_id) in elements[1..].iter().enumerate() {
            let expected = Expected {
                ty: first_ty,
                origin: ExpectedOrigin::PreviousInSequence {
                    previous_span: self.arena[elements[0]].span,
                    current_index: i + 1,
                    sequence_kind: SequenceKind::ListLiteral,
                },
            };
            self.check(elem_id, expected);
        }

        self.pool.list(first_ty)
    }

    fn infer_tuple(&mut self, expr_id: ExprId, elements: &[ExprId]) -> Idx {
        let elem_types: Vec<_> = elements.iter()
            .map(|&e| self.infer(e))
            .collect();
        self.pool.tuple(&elem_types)
    }
}
```

### Tasks

- [x] Implement literal inference for all primitive types
- [x] Implement `infer_list()` with element unification
- [x] Implement `infer_tuple()` with element collection
- [x] Implement `infer_map()` with key/value inference
- [x] Add tests for all literal types ✅ (2026-02-04)

---

## 06.3 Identifier Lookup

**Goal:** Look up identifiers and instantiate type schemes

### Design

```rust
impl<'a> InferEngine<'a> {
    fn infer_ident(&mut self, expr_id: ExprId, name: Name) -> Idx {
        match self.env.lookup(name) {
            Some(scheme) => {
                // Instantiate the type scheme with fresh variables
                self.unify.instantiate(scheme)
            }
            None => {
                // Unknown identifier - report error
                let similar = self.env.find_similar(name);
                self.errors.push(TypeCheckError {
                    span: self.arena[expr_id].span,
                    code: ErrorCode::E2001,
                    kind: TypeErrorKind::UnknownIdent { name, similar },
                    context: self.current_context(),
                    suggestions: vec![],
                });
                Idx::ERROR
            }
        }
    }
}
```

### Tasks

- [x] Implement identifier lookup with scheme instantiation
- [x] Add similar name detection for typo suggestions ✅ (2026-02-05, Levenshtein distance in TypeEnv::find_similar)
- [ ] Handle qualified identifiers (module.name)
- [x] Add tests for identifier resolution ✅ (2026-02-04)

---

## 06.4 Function Call Inference

**Goal:** Infer types of function calls

### Design

```rust
impl<'a> InferEngine<'a> {
    fn infer_call(
        &mut self,
        expr_id: ExprId,
        func_expr: ExprId,
        args: &[ExprId],
    ) -> Idx {
        let func_ty = self.infer(func_expr);
        let func_ty = self.unify.resolve(func_ty);

        // Check if it's a function
        if self.pool.tag(func_ty) != Tag::Function {
            if func_ty != Idx::ERROR {
                self.errors.push(TypeCheckError {
                    span: self.arena[func_expr].span,
                    code: ErrorCode::E2010,
                    kind: TypeErrorKind::NotCallable { ty: func_ty },
                    context: self.current_context(),
                    suggestions: vec![],
                });
            }
            return Idx::ERROR;
        }

        let params = self.pool.function_params(func_ty);
        let ret = self.pool.function_return(func_ty);

        // Check arity
        if args.len() != params.len() {
            self.errors.push(TypeCheckError {
                span: self.arena[expr_id].span,
                code: ErrorCode::E2011,
                kind: TypeErrorKind::ArityMismatch {
                    expected: params.len(),
                    found: args.len(),
                    kind: ArityKind::Function,
                },
                context: self.current_context(),
                suggestions: vec![],
            });
            return Idx::ERROR;
        }

        // Check each argument
        let func_name = self.get_func_name(func_expr);
        for (i, (&arg_id, &param_ty)) in args.iter().zip(params.iter()).enumerate() {
            let expected = Expected {
                ty: param_ty,
                origin: ExpectedOrigin::Context {
                    span: self.arena[arg_id].span,
                    kind: ContextKind::FunctionArgument {
                        func_name,
                        arg_index: i,
                        param_name: None, // Could extract from function def
                    },
                },
            };
            self.check(arg_id, expected);
        }

        ret
    }
}
```

### Tasks

- [x] Implement `infer_call()` with arity checking
- [x] Add argument type checking with context
- [x] Handle method calls (receiver.method(args)) ✅ (2026-02-05, implemented in infer/expr.rs:3016-3095)
- [x] Handle generic function instantiation ✅ (2026-02-05, works via scheme instantiation)
- [x] Add tests for various call scenarios ✅ (2026-02-04)

---

## 06.5 Operator Inference

**Goal:** Infer types of operator expressions

### Design

```rust
impl<'a> InferEngine<'a> {
    fn infer_binary(
        &mut self,
        expr_id: ExprId,
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
    ) -> Idx {
        match op {
            // Arithmetic: int -> int -> int, float -> float -> float
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                let left_ty = self.infer(left);
                let expected = Expected {
                    ty: left_ty,
                    origin: ExpectedOrigin::Context {
                        span: self.arena[right].span,
                        kind: ContextKind::BinaryOpRight { op },
                    },
                };
                self.check(right, expected);

                // Result is same as operands
                left_ty
            }

            // Comparison: T -> T -> bool
            BinaryOp::Eq | BinaryOp::Ne |
            BinaryOp::Lt | BinaryOp::Le |
            BinaryOp::Gt | BinaryOp::Ge => {
                let left_ty = self.infer(left);
                let expected = Expected {
                    ty: left_ty,
                    origin: ExpectedOrigin::Context {
                        span: self.arena[right].span,
                        kind: ContextKind::ComparisonRight,
                    },
                };
                self.check(right, expected);

                Idx::BOOL
            }

            // Boolean: bool -> bool -> bool
            BinaryOp::And | BinaryOp::Or => {
                let bool_expected = Expected {
                    ty: Idx::BOOL,
                    origin: ExpectedOrigin::Context {
                        span: self.arena[left].span,
                        kind: ContextKind::BinaryOpLeft { op },
                    },
                };
                self.check(left, bool_expected.clone());
                self.check(right, Expected {
                    ty: Idx::BOOL,
                    origin: ExpectedOrigin::Context {
                        span: self.arena[right].span,
                        kind: ContextKind::BinaryOpRight { op },
                    },
                });

                Idx::BOOL
            }

            // String concat: str -> str -> str
            BinaryOp::Concat => {
                let str_expected = Expected {
                    ty: Idx::STR,
                    origin: ExpectedOrigin::Context {
                        span: self.arena[left].span,
                        kind: ContextKind::BinaryOpLeft { op },
                    },
                };
                self.check(left, str_expected);
                self.check(right, Expected {
                    ty: Idx::STR,
                    origin: ExpectedOrigin::Context {
                        span: self.arena[right].span,
                        kind: ContextKind::BinaryOpRight { op },
                    },
                });

                Idx::STR
            }

            // ... other operators
        }
    }
}
```

### Tasks

- [x] Implement `infer_binary()` for all operators
- [x] Implement `infer_unary()` for unary operators
- [ ] Handle operator overloading if applicable
- [x] Add tests for all operator types ✅ (2026-02-04)

---

## 06.6 Control Flow

**Goal:** Infer types of control flow expressions

### Design

```rust
impl<'a> InferEngine<'a> {
    fn infer_if(
        &mut self,
        expr_id: ExprId,
        cond: ExprId,
        then_branch: ExprId,
        else_branch: Option<ExprId>,
    ) -> Idx {
        // Condition must be bool
        let cond_expected = Expected {
            ty: Idx::BOOL,
            origin: ExpectedOrigin::Context {
                span: self.arena[cond].span,
                kind: ContextKind::IfCondition,
            },
        };
        self.check(cond, cond_expected);

        // Infer then branch
        let then_ty = self.infer(then_branch);

        match else_branch {
            Some(else_id) => {
                // Else must match then
                let else_expected = Expected {
                    ty: then_ty,
                    origin: ExpectedOrigin::PreviousInSequence {
                        previous_span: self.arena[then_branch].span,
                        current_index: 1,
                        sequence_kind: SequenceKind::IfBranches,
                    },
                };
                self.check(else_id, else_expected);
                then_ty
            }
            None => {
                // No else: then must be unit
                if then_ty != Idx::UNIT {
                    // Warning or error about missing else
                }
                Idx::UNIT
            }
        }
    }

    fn infer_match(
        &mut self,
        expr_id: ExprId,
        scrutinee: ExprId,
        arms: &[MatchArm],
    ) -> Idx {
        let scrutinee_ty = self.infer(scrutinee);

        if arms.is_empty() {
            return Idx::NEVER; // Empty match never returns
        }

        // Check all patterns against scrutinee
        for (i, arm) in arms.iter().enumerate() {
            self.check_pattern(arm.pattern, scrutinee_ty);
        }

        // Infer first arm body
        let first_ty = self.infer(arms[0].body);

        // Check remaining arm bodies match first
        for (i, arm) in arms[1..].iter().enumerate() {
            let expected = Expected {
                ty: first_ty,
                origin: ExpectedOrigin::PreviousInSequence {
                    previous_span: self.arena[arms[0].body].span,
                    current_index: i + 1,
                    sequence_kind: SequenceKind::MatchArms,
                },
            };
            self.check(arm.body, expected);
        }

        first_ty
    }
}
```

### Tasks

- [x] Implement `infer_if()` with branch unification
- [x] Implement `infer_match()` with pattern checking ✅ (2026-02-04)
- [x] Implement loop inference (for, while) ✅ (2026-02-04)
- [x] Handle never type propagation (break value tracking) ✅ (2026-02-05, loop_break_types stack in InferEngine)
- [x] Add tests for control flow ✅ (2026-02-04)

---

## 06.7 Lambda Inference

**Goal:** Infer types of lambda expressions

### Design

```rust
impl<'a> InferEngine<'a> {
    fn infer_lambda(
        &mut self,
        expr_id: ExprId,
        params: &[LambdaParam],
        body: ExprId,
        ret_annotation: Option<ParsedType>,
    ) -> Idx {
        // Enter new scope for lambda
        self.unify.enter_scope();
        self.env.push_scope();

        // Create types for parameters
        let mut param_types = Vec::with_capacity(params.len());
        for param in params {
            let param_ty = match &param.ty_annotation {
                Some(parsed) => self.resolve_parsed_type(parsed),
                None => self.unify.fresh_var(),
            };
            self.env.bind(param.name, param_ty);
            param_types.push(param_ty);
        }

        // Infer body
        let body_ty = match ret_annotation {
            Some(parsed) => {
                let expected_ret = self.resolve_parsed_type(&parsed);
                let expected = Expected {
                    ty: expected_ret,
                    origin: ExpectedOrigin::Annotation {
                        name: Name::LAMBDA,
                        span: self.arena[body].span,
                    },
                };
                self.check(body, expected)
            }
            None => self.infer(body),
        };

        // Exit scope
        self.env.pop_scope();
        let _generalizable = self.unify.exit_scope();

        // Create function type
        self.pool.function(&param_types, body_ty)
    }
}
```

### Tasks

- [x] Implement `infer_lambda()` with scope management
- [ ] Handle parameter type annotations (needs ParsedType → Idx)
- [ ] Handle return type annotations (needs ParsedType → Idx)
- [ ] Implement closure variable capture
- [x] Add tests for various lambda forms ✅ (2026-02-04)

---

## 06.8 Pattern Expression Inference

**Goal:** Infer types of Ori's pattern expressions

### Design

```rust
impl<'a> InferEngine<'a> {
    fn infer_pattern_expr(
        &mut self,
        expr_id: ExprId,
        kind: &PatternExprKind,
    ) -> Idx {
        match kind {
            PatternExprKind::Run { pattern, input } => {
                // pattern : Pattern<In, Out>
                // input : In
                // result : Out
                let pattern_ty = self.infer(*pattern);
                let input_ty = self.infer(*input);

                // Extract In and Out from Pattern<In, Out>
                let (in_ty, out_ty) = self.extract_pattern_types(pattern_ty)?;

                // Check input matches In
                let expected = Expected {
                    ty: in_ty,
                    origin: ExpectedOrigin::Context {
                        span: self.arena[*input].span,
                        kind: ContextKind::PatternBinding { pattern_kind: "run" },
                    },
                };
                self.check(*input, expected);

                out_ty
            }

            PatternExprKind::Try { pattern, input } => {
                // pattern : Pattern<In, Out>
                // input : In
                // result : Option<Out>
                let pattern_ty = self.infer(*pattern);
                let input_ty = self.infer(*input);

                let (in_ty, out_ty) = self.extract_pattern_types(pattern_ty)?;
                self.check(*input, Expected { ty: in_ty, ... });

                self.pool.option(out_ty)
            }

            // ... other pattern expressions
        }
    }
}
```

### Tasks

- [x] Implement `FunctionSeq` inference (run, try, match, for) ✅ (2026-02-04)
- [x] Implement `FunctionExp` inference (print, panic, todo, catch, etc.) ✅ (2026-02-04)
- [x] Scope management for sequential bindings ✅ (2026-02-04)
- [x] Auto-unwrap Result/Option in try blocks ✅ (2026-02-04)
- [x] Binding pattern support (Name, Tuple, List, Struct, Wildcard) ✅ (2026-02-04)
- [x] Add tests for pattern expressions (8 new tests) ✅ (2026-02-04)
- [ ] Scoped bindings for `recurse` pattern (`self` binding) - needs Section 07

### Implementation Notes (2026-02-04)

Implemented in `ori_types/src/infer/expr.rs`:

**FunctionSeq variants:**
- `Run`: Sequential bindings with scope management, returns result type
- `Try`: Auto-unwraps Result/Option, wraps result in Result
- `Match`: Delegates to existing `infer_match`
- `ForPattern`: Iterator with optional map, pattern matching, default

**FunctionExp variants:**
- `Print` → unit
- `Panic`, `Todo`, `Unreachable` → never
- `Catch` → unifies try/catch branch types
- `Timeout` → Option<T>
- `Recurse`, `Parallel`, `Spawn`, `Cache`, `With` → basic implementations

**Binding patterns:**
- `bind_pattern()` helper extracts types from tuples, lists, structs
- Struct field lookup deferred to Section 07 (Registries)

---

## 06.9 Completion Checklist

- [x] `InferEngine` structure complete
- [x] All literal types inferred correctly
- [x] Identifier lookup with instantiation working
- [x] Function call inference with arity checking
- [x] All operators typed correctly
- [x] Control flow (if/match/loops) working ✅ (2026-02-04)
- [x] Lambda inference with closures ✅ (2026-02-04)
- [x] Pattern expressions integrated ✅ (2026-02-04)
- [x] All existing tests passing (213 tests) ✅ (2026-02-04)
- [x] Comprehensive unit tests for expression inference ✅ (2026-02-04, 71 tests)
- [x] `ParsedType` → `Idx` conversion ✅ (2026-02-04)
- [x] Type annotation checking for let bindings ✅ (2026-02-04)
- [x] Type annotation checking for lambdas ✅ (2026-02-04)
- [x] Cast expression inference ✅ (2026-02-04)
- [x] 11 new tests for ParsedType resolution ✅ (2026-02-04, 243 total)

**Exit Criteria:** The inference engine can type check all Ori expressions with correct HM inference, producing rich error messages when types don't match.

**Section 06 Status:** In Progress (~95%)

**Remaining for 100%:**
1. Qualified identifier resolution (`module.name`) (06.3)
2. Closure variable capture tracking (06.7)
3. Scoped bindings for `recurse` pattern (06.8)

---

## Next Steps

1. ~~Implement `ParsedType` → `Idx` conversion for type annotations~~ ✅ Done (2026-02-04)
2. Implement method call and field access inference (needs Section 07 Registries)
3. ~~Add unit tests for match/loop inference~~ ✅ Done (2026-02-04)
4. ~~Implement pattern expressions (FunctionSeq, FunctionExp)~~ ✅ Done (2026-02-04)
