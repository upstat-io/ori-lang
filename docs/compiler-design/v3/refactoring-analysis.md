# V3 Compiler Refactoring Analysis & Plan

## Executive Summary

This document provides a comprehensive analysis of the v3 compiler architecture and a detailed refactoring plan to achieve maximum extensibility and maintainability following SOLID and DRY principles.

**Current State**: The v3 compiler has excellent foundation (Salsa queries, arena allocation, pattern trait system) but has extensibility gaps in expression handling, parser organization, and dependency injection.

**Goal**: A compiler where adding/modifying syntax, expressions, types, or behaviors requires minimal file changes, with tests validating the design spec (not working around broken code).

---

## Part 1: Current Architecture Analysis

### Strengths (Keep These)

| Component | Why It's Good | SOLID Principle |
|-----------|--------------|-----------------|
| **Salsa Query System** | Automatic caching, incremental recomputation, dependency tracking | Single Responsibility |
| **Pattern Trait System** | Adding patterns = new file + trait impl + registry entry | Open/Closed |
| **Arena Allocation** | Cache locality, no Box overhead, simple lifetime management | - |
| **String Interning** | O(1) name comparison, memory efficiency | - |
| **ExprId/TypeId** | Type-safe indices, no raw u32 confusion | - |
| **Context Objects** | TypeCheckContext, EvalContext decouple phases | Dependency Inversion |

### Weaknesses (Fix These)

| Component | Problem | Impact | SOLID Violation |
|-----------|---------|--------|-----------------|
| **Parser (1706 lines)** | Monolithic file handles all syntax | Adding syntax requires understanding entire parser | Single Responsibility |
| **Expression Handling** | Adding ExprKind requires changes in parser, typeck, evaluator (3+ files) | High coupling, error-prone | Open/Closed |
| **Type Checker** | Creates own `PatternRegistry::new()` | Can't inject mock patterns for testing | Dependency Inversion |
| **Operator Handling** | 800+ lines inline in evaluator | Duplicated logic, hard to test operators in isolation | Single Responsibility |
| **Type System** | Compound types fall back to fresh vars | Incomplete type checking | - |
| **No Visitor Pattern** | AST traversal duplicated across phases | Code duplication, inconsistent handling | DRY |
| **Method Dispatch** | Inline match on value type in evaluator | Adding methods requires evaluator changes | Open/Closed |

### Current File Distribution

```
compiler/sigilc-v3/src/
├── lib.rs               (64 lines)   - Public API ✓
├── db.rs                (~100 lines) - Salsa database ✓
├── input.rs             (~50 lines)  - Source file input ✓
├── query.rs             (~200 lines) - Salsa queries ✓
├── lexer.rs             (657 lines)  - Tokenization ⚠️
├── parser.rs            (1706 lines) - Parsing ❌ TOO LARGE
├── typeck.rs            (1349 lines) - Type checking ⚠️
├── types.rs             (1015 lines) - Type system ⚠️
├── diagnostic.rs        (467 lines)  - Error reporting ✓
├── ir/                  (10 files)   - IR types ✓
├── eval/                (7 files)    - Evaluator ⚠️
│   └── evaluator.rs     (2000+ lines)- Expression eval ❌ TOO LARGE
├── patterns/            (16 files)   - Pattern system ✓ EXCELLENT
└── test/                (4 files)    - Test runner ✓
```

---

## Part 2: Target Architecture

### Design Principles

1. **Expression Definitions** - Mirror the pattern system for expressions
2. **Operator Registry** - Centralized operator handling with trait implementations
3. **Type Registry** - Injectable type definitions for custom/compound types
4. **Method Registry** - Trait-based method dispatch
5. **Parser Modularization** - Separate parsers for declarations, expressions, patterns
6. **Visitor Pattern** - Generic AST traversal for all phases

### Target File Structure

```
compiler/sigilc-v3/src/
├── lib.rs                      - Public API
├── db.rs                       - Salsa database (injectable registries)
├── input.rs                    - Source file input
├── query.rs                    - Salsa queries
├── diagnostic.rs               - Error reporting
│
├── syntax/                     - Lexing and parsing
│   ├── mod.rs                  - Module exports
│   ├── lexer.rs                - Tokenization
│   ├── parser/                 - Modular parser
│   │   ├── mod.rs              - Parser orchestration
│   │   ├── declarations.rs     - Functions, types, traits, tests
│   │   ├── expressions.rs      - Expression parsing (uses expr registry)
│   │   ├── patterns.rs         - Match patterns
│   │   ├── types.rs            - Type annotations
│   │   └── recovery.rs         - Error recovery strategies
│   └── token.rs                - Token definitions
│
├── ir/                         - Intermediate representation
│   ├── mod.rs                  - IR types
│   ├── arena.rs                - Arena allocation
│   ├── ast.rs                  - AST node definitions
│   ├── expr_kinds.rs           - Expression kind enum (SINGLE SOURCE)
│   ├── visitor.rs              - Visitor trait + default implementations
│   └── ...
│
├── types/                      - Type system
│   ├── mod.rs                  - Type definitions
│   ├── registry.rs             - Type registry (injectable)
│   ├── inference.rs            - Type inference context
│   ├── unification.rs          - Type unification
│   └── primitives.rs           - Built-in type definitions
│
├── check/                      - Type checking
│   ├── mod.rs                  - Type checker orchestration
│   ├── checker.rs              - Main type checker
│   ├── expr_checker.rs         - Expression type checking (uses registry)
│   └── pattern_checker.rs      - Pattern type checking (uses registry)
│
├── eval/                       - Interpreter
│   ├── mod.rs                  - Evaluator orchestration
│   ├── evaluator.rs            - Core evaluator (smaller, delegates)
│   ├── expr_evaluator.rs       - Expression evaluation (uses registry)
│   └── builtins.rs             - Built-in functions
│
├── operators/                  - Operator system (NEW)
│   ├── mod.rs                  - Operator exports
│   ├── registry.rs             - Operator registry (injectable)
│   ├── binary.rs               - Binary operator definitions
│   ├── unary.rs                - Unary operator definitions
│   └── traits.rs               - Operator traits
│
├── methods/                    - Method system (NEW)
│   ├── mod.rs                  - Method exports
│   ├── registry.rs             - Method registry (injectable)
│   ├── string_methods.rs       - String methods
│   ├── list_methods.rs         - List methods
│   ├── option_methods.rs       - Option methods
│   ├── result_methods.rs       - Result methods
│   └── traits.rs               - Method traits
│
├── patterns/                   - Pattern system (EXISTING - GOOD)
│   └── ...
│
└── test/                       - Test runner
    └── ...
```

---

## Part 3: Core Refactoring Tasks

### Phase 1: Foundation (Prerequisites)

#### Task 1.1: Create Visitor Pattern for AST

**Why**: Every phase (type checking, evaluation, optimization) needs to traverse the AST. Currently, each phase duplicates traversal logic.

```rust
// ir/visitor.rs
pub trait ExprVisitor {
    type Output;
    type Error;

    fn visit_expr(&mut self, arena: &ExprArena, id: ExprId) -> Result<Self::Output, Self::Error>;

    // Default implementations for each ExprKind
    fn visit_int(&mut self, value: i64, span: Span) -> Result<Self::Output, Self::Error>;
    fn visit_binary(&mut self, op: BinaryOp, left: ExprId, right: ExprId) -> Result<Self::Output, Self::Error>;
    fn visit_call(&mut self, func: ExprId, args: CallArgRange) -> Result<Self::Output, Self::Error>;
    // ... etc
}

// Default walking implementation
pub fn walk_expr<V: ExprVisitor>(
    visitor: &mut V,
    arena: &ExprArena,
    id: ExprId
) -> Result<V::Output, V::Error> {
    let expr = arena.get_expr(id);
    match &expr.kind {
        ExprKind::Int(n) => visitor.visit_int(*n, expr.span),
        ExprKind::Binary { op, left, right } => {
            visitor.visit_binary(*op, *left, *right)
        }
        // ... handle all variants
    }
}
```

**Benefit**: Add new ExprKind → add one method to visitor trait → all phases get default handling.

#### Task 1.2: Create Expression Definition Trait

**Why**: Mirror the pattern system's extensibility for expressions.

```rust
// expressions/definition.rs
pub trait ExprDefinition: Send + Sync {
    /// The expression kind this definition handles.
    fn kind(&self) -> ExprKindTag;

    /// Type check this expression.
    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type;

    /// Evaluate this expression.
    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn ExprExecutor) -> EvalResult;

    /// Parse this expression (for custom syntax).
    /// Returns None to use default parsing.
    fn parse(&self, parser: &mut Parser) -> Option<Result<ExprId, ParseError>> {
        None
    }
}

// expressions/registry.rs
pub struct ExprRegistry {
    definitions: HashMap<ExprKindTag, Arc<dyn ExprDefinition>>,
}

impl ExprRegistry {
    pub fn new() -> Self {
        let mut registry = Self { definitions: HashMap::new() };
        // Register all built-in expressions
        registry.register(IntExpr);
        registry.register(BinaryExpr);
        registry.register(IfExpr);
        // ...
        registry
    }
}
```

**Benefit**: Adding a new expression type = new file + trait impl + registry entry (same as patterns).

#### Task 1.3: Create Operator Trait System

**Why**: Operators are currently handled with massive match statements in the evaluator.

```rust
// operators/traits.rs
pub trait BinaryOperator: Send + Sync {
    fn op(&self) -> BinaryOp;
    fn type_check(&self, left: &Type, right: &Type) -> Result<Type, TypeError>;
    fn evaluate(&self, left: Value, right: Value) -> EvalResult;
}

// operators/add.rs
pub struct AddOperator;

impl BinaryOperator for AddOperator {
    fn op(&self) -> BinaryOp { BinaryOp::Add }

    fn type_check(&self, left: &Type, right: &Type) -> Result<Type, TypeError> {
        match (left, right) {
            (Type::Int, Type::Int) => Ok(Type::Int),
            (Type::Float, Type::Float) => Ok(Type::Float),
            (Type::Str, Type::Str) => Ok(Type::Str), // String concat
            (Type::List(a), Type::List(b)) if a == b => Ok(Type::List(a.clone())),
            _ => Err(TypeError::BinaryMismatch(BinaryOp::Add, left.clone(), right.clone()))
        }
    }

    fn evaluate(&self, left: Value, right: Value) -> EvalResult {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Str(a), Value::Str(b)) => Ok(Value::Str(Rc::new(format!("{}{}", a, b)))),
            // ...
        }
    }
}

// operators/registry.rs
pub struct OperatorRegistry {
    binary: HashMap<BinaryOp, Arc<dyn BinaryOperator>>,
    unary: HashMap<UnaryOp, Arc<dyn UnaryOperator>>,
}
```

**Benefit**:
- Each operator isolated and testable
- Adding operators = new file + trait impl + registry entry
- Type checking and evaluation logic co-located

#### Task 1.4: Create Method Trait System

**Why**: Method dispatch is currently a giant match in the evaluator.

```rust
// methods/traits.rs
pub trait MethodDefinition: Send + Sync {
    fn name(&self) -> &'static str;
    fn receiver_type(&self) -> &Type;
    fn param_types(&self) -> &[Type];
    fn return_type(&self) -> Type;
    fn evaluate(&self, receiver: Value, args: Vec<Value>) -> EvalResult;
}

// methods/string_methods.rs
pub struct StringLenMethod;

impl MethodDefinition for StringLenMethod {
    fn name(&self) -> &'static str { "len" }
    fn receiver_type(&self) -> &Type { &Type::Str }
    fn param_types(&self) -> &[Type] { &[] }
    fn return_type(&self) -> Type { Type::Int }

    fn evaluate(&self, receiver: Value, _args: Vec<Value>) -> EvalResult {
        match receiver {
            Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
            _ => Err(EvalError::TypeMismatch("str", receiver.type_name()))
        }
    }
}

// methods/registry.rs
pub struct MethodRegistry {
    // (receiver_type, method_name) -> definition
    methods: HashMap<(TypeTag, Name), Arc<dyn MethodDefinition>>,
}
```

**Benefit**: Adding methods = new file + trait impl + registry entry.

### Phase 2: Dependency Injection

#### Task 2.1: Injectable Registries in Database

**Why**: Currently TypeChecker creates its own `PatternRegistry::new()`. Can't inject mocks for testing.

```rust
// db.rs
#[salsa::db]
pub trait Db: salsa::Database {
    fn interner(&self) -> &SharedInterner;

    // Injectable registries
    fn pattern_registry(&self) -> &PatternRegistry;
    fn operator_registry(&self) -> &OperatorRegistry;
    fn method_registry(&self) -> &MethodRegistry;
    fn expr_registry(&self) -> &ExprRegistry;
    fn type_registry(&self) -> &TypeRegistry;
}

#[salsa::db]
pub struct CompilerDb {
    storage: salsa::Storage<Self>,
    interner: SharedInterner,

    // Registries
    patterns: PatternRegistry,
    operators: OperatorRegistry,
    methods: MethodRegistry,
    exprs: ExprRegistry,
    types: TypeRegistry,
}

impl CompilerDb {
    /// Create with default registries (production).
    pub fn new() -> Self { ... }

    /// Create with custom registries (testing).
    pub fn with_registries(
        patterns: PatternRegistry,
        operators: OperatorRegistry,
        // ...
    ) -> Self { ... }
}
```

**Benefit**:
- Tests can inject mock registries
- Custom language variants possible (subset for education, etc.)
- Follows Dependency Inversion principle

#### Task 2.2: Update Type Checker to Use Injected Registry

```rust
// check/checker.rs
pub struct TypeChecker<'a> {
    db: &'a dyn Db,  // Has access to all registries
    arena: &'a ExprArena,
    ctx: InferenceContext,
    env: TypeEnv,
    // ... (no more self-created PatternRegistry)
}

impl<'a> TypeChecker<'a> {
    pub fn new(db: &'a dyn Db, arena: &'a ExprArena) -> Self {
        TypeChecker {
            db,
            arena,
            ctx: InferenceContext::new(),
            env: TypeEnv::new(),
        }
    }

    fn check_pattern(&mut self, pattern: &FunctionExp) -> Type {
        let registry = self.db.pattern_registry();
        if let Some(def) = registry.get(pattern.kind) {
            def.type_check(&mut self.create_type_context())
        } else {
            self.error("unknown pattern");
            Type::Error
        }
    }
}
```

### Phase 3: Parser Modularization

#### Task 3.1: Extract Declaration Parsing

```rust
// syntax/parser/declarations.rs
pub struct DeclarationParser<'a> {
    tokens: &'a TokenList,
    pos: &'a mut usize,
    arena: &'a mut ExprArena,
    expr_parser: &'a ExprParser<'a>,
}

impl<'a> DeclarationParser<'a> {
    pub fn parse_function(&mut self) -> Result<Function, ParseError> { ... }
    pub fn parse_test(&mut self) -> Result<TestDef, ParseError> { ... }
    pub fn parse_type_def(&mut self) -> Result<TypeDef, ParseError> { ... }
    pub fn parse_trait_def(&mut self) -> Result<TraitDef, ParseError> { ... }
    pub fn parse_impl_block(&mut self) -> Result<ImplBlock, ParseError> { ... }
}
```

#### Task 3.2: Extract Expression Parsing

```rust
// syntax/parser/expressions.rs
pub struct ExprParser<'a> {
    tokens: &'a TokenList,
    pos: &'a mut usize,
    arena: &'a mut ExprArena,
    interner: &'a StringInterner,
    registry: &'a ExprRegistry,  // For custom expression syntax
}

impl<'a> ExprParser<'a> {
    pub fn parse_expr(&mut self) -> Result<ExprId, ParseError> { ... }
    pub fn parse_primary(&mut self) -> Result<ExprId, ParseError> { ... }
    pub fn parse_binary(&mut self, min_prec: u8) -> Result<ExprId, ParseError> { ... }

    // Pattern expressions
    pub fn parse_function_seq(&mut self) -> Result<ExprId, ParseError> { ... }
    pub fn parse_function_exp(&mut self) -> Result<ExprId, ParseError> { ... }
}
```

#### Task 3.3: Extract Pattern Parsing

```rust
// syntax/parser/patterns.rs
pub struct PatternParser<'a> {
    tokens: &'a TokenList,
    pos: &'a mut usize,
    arena: &'a mut ExprArena,
}

impl<'a> PatternParser<'a> {
    pub fn parse_match_pattern(&mut self) -> Result<MatchPattern, ParseError> { ... }
    pub fn parse_binding_pattern(&mut self) -> Result<BindingPattern, ParseError> { ... }
}
```

**Result**: Parser goes from 1706 lines in one file to ~400 lines each in focused files.

### Phase 4: Type System Completion

#### Task 4.1: Type Registry for Compound Types

```rust
// types/registry.rs
pub trait TypeDefinition: Send + Sync {
    fn name(&self) -> &'static str;
    fn type_id(&self) -> TypeId;
    fn supertraits(&self) -> &[TraitId];
    fn methods(&self) -> &[MethodSignature];
}

pub struct TypeRegistry {
    types: HashMap<TypeId, Arc<dyn TypeDefinition>>,
    by_name: HashMap<Name, TypeId>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        let mut reg = Self::empty();
        // Register primitive types
        reg.register(IntType);
        reg.register(FloatType);
        reg.register(BoolType);
        reg.register(StrType);
        // Register compound types
        reg.register(ListType);
        reg.register(MapType);
        reg.register(OptionType);
        reg.register(ResultType);
        reg
    }
}
```

#### Task 4.2: Fix Compound Type Handling in Type Checker

```rust
// Current (broken):
fn type_id_to_type(&mut self, type_id: TypeId) -> Type {
    match type_id {
        TypeId::INT => Type::Int,
        // ...
        _ => self.ctx.fresh_var()  // BUG: ignores compound types!
    }
}

// Fixed:
fn type_id_to_type(&mut self, type_id: TypeId) -> Type {
    let registry = self.db.type_registry();
    match registry.get(type_id) {
        Some(def) => def.to_type(),
        None => {
            self.error("unknown type");
            Type::Error
        }
    }
}
```

---

## Part 4: Testing Strategy

### Principle: Tests Validate the Spec, Not the Code

Tests should be written against the language specification. If a test fails, the code is wrong (not the test).

### Test Organization

```
tests/
├── spec/                    - Specification conformance tests
│   ├── lexical/             - Lexer tests (from spec section 3)
│   ├── expressions/         - Expression tests (from spec section 9)
│   ├── patterns/            - Pattern tests (from spec section 10)
│   ├── types/               - Type system tests (from spec section 6-7)
│   └── README.md            - Test philosophy
│
├── unit/                    - Unit tests for compiler components
│   ├── operators/           - Each operator in isolation
│   ├── methods/             - Each method in isolation
│   ├── patterns/            - Each pattern in isolation
│   └── types/               - Type inference/unification
│
├── integration/             - End-to-end compiler tests
│   ├── compile_success/     - Programs that should compile
│   ├── compile_fail/        - Programs that should fail with specific errors
│   └── runtime/             - Programs that should produce specific output
│
└── regression/              - Bug fixes that should stay fixed
```

### Test Template

```rust
// tests/unit/operators/add_test.rs
//! Tests for the + operator.
//!
//! Spec Reference: docs/sigil_lang/0.1-alpha/spec/09-expressions.md § Binary Operators
//! Design Reference: docs/sigil_lang/0.1-alpha/design/02-syntax/02-expressions.md

use sigilc_v3::operators::AddOperator;

#[test]
fn test_int_addition() {
    // From spec: "int + int -> int"
    let op = AddOperator;
    let result = op.evaluate(Value::Int(2), Value::Int(3)).unwrap();
    assert_eq!(result, Value::Int(5));
}

#[test]
fn test_string_concatenation() {
    // From spec: "+ concatenates str operands"
    let op = AddOperator;
    let result = op.evaluate(
        Value::Str("hello".into()),
        Value::Str(" world".into())
    ).unwrap();
    assert_eq!(result, Value::Str("hello world".into()));
}

#[test]
fn test_type_mismatch() {
    // From spec: "int + str is a type error"
    let op = AddOperator;
    let result = op.type_check(&Type::Int, &Type::Str);
    assert!(result.is_err());
}
```

### Property-Based Tests

```rust
// tests/unit/operators/properties.rs
use proptest::prelude::*;

proptest! {
    #[test]
    fn add_commutative(a: i64, b: i64) {
        // a + b == b + a (for ints)
        let op = AddOperator;
        let result1 = op.evaluate(Value::Int(a), Value::Int(b)).unwrap();
        let result2 = op.evaluate(Value::Int(b), Value::Int(a)).unwrap();
        prop_assert_eq!(result1, result2);
    }

    #[test]
    fn add_identity(a: i64) {
        // a + 0 == a
        let op = AddOperator;
        let result = op.evaluate(Value::Int(a), Value::Int(0)).unwrap();
        prop_assert_eq!(result, Value::Int(a));
    }
}
```

### Test Macros for Spec Conformance

```rust
// tests/spec/macros.rs
macro_rules! spec_test {
    ($name:ident, $spec_section:literal, $code:literal, $expected:expr) => {
        #[test]
        fn $name() {
            // Compile and run the code
            let result = run_sigil_code($code);

            // Assert expected behavior
            assert_eq!(result, $expected,
                "Spec violation: {}\nCode: {}\nExpected: {:?}\nGot: {:?}",
                $spec_section, $code, $expected, result
            );
        }
    };
}

// Usage:
spec_test!(
    int_addition,
    "09-expressions.md § Binary Operators",
    "2 + 3",
    Ok(Value::Int(5))
);
```

---

## Part 5: Implementation Phases

### Phase 1: Foundation (Estimated: 2 weeks)

1. **Week 1: Visitor + Expression Traits**
   - Create `ir/visitor.rs` with `ExprVisitor` trait
   - Create `expressions/definition.rs` with `ExprDefinition` trait
   - Create `expressions/registry.rs` with `ExprRegistry`
   - Write tests for visitor pattern

2. **Week 2: Operator + Method Traits**
   - Create `operators/` module with trait and registry
   - Create `methods/` module with trait and registry
   - Implement all existing operators as trait impls
   - Write unit tests for each operator

### Phase 2: Dependency Injection (Estimated: 1 week)

1. **Update `db.rs`** to include all registries
2. **Update `TypeChecker`** to use `db.pattern_registry()`
3. **Update `Evaluator`** to use injected registries
4. **Add test helpers** for creating DBs with mock registries

### Phase 3: Parser Modularization (Estimated: 2 weeks)

1. **Week 1: Extract Components**
   - Create `syntax/parser/` directory structure
   - Extract declaration parsing
   - Extract expression parsing
   - Extract pattern parsing

2. **Week 2: Testing + Cleanup**
   - Write parser unit tests for each component
   - Remove old monolithic parser
   - Ensure all existing tests pass

### Phase 4: Type System Completion (Estimated: 1 week)

1. Create `types/registry.rs` with `TypeDefinition` trait
2. Implement all primitive types as definitions
3. Implement compound types (List, Option, Result, etc.)
4. Fix type checker to use registry for compound types
5. Write comprehensive type inference tests

### Phase 5: Test Suite Overhaul (Estimated: 1 week)

1. Reorganize tests into spec/unit/integration/regression
2. Create test macros for spec conformance
3. Add property-based tests for operators
4. Ensure 100% coverage of spec sections
5. Document test philosophy in README

---

## Part 6: Migration Strategy

### Approach: Parallel Development

1. **Create new modules alongside existing code**
   - `operators/` exists alongside inline operator code in evaluator
   - Both work during transition

2. **Migrate incrementally**
   - Move one operator at a time
   - Move one expression type at a time
   - Keep all tests passing

3. **Remove old code only after new code is tested**
   - Delete inline operator handling after all operators migrated
   - Delete monolithic parser after modular parser complete

### No Feature Flags Needed

Since this is a brand new language with no users:
- No backward compatibility concerns
- No migration path needed
- Full refactors allowed
- Tests define correctness (not existing behavior)

---

## Part 7: Success Criteria

### Adding a New Expression Type

**Before refactoring**: 5+ file changes (parser.rs, ast.rs, typeck.rs, evaluator.rs, tests)

**After refactoring**: 3 file changes
1. `ir/expr_kinds.rs` - Add enum variant
2. `expressions/new_expr.rs` - Implement `ExprDefinition`
3. `expressions/registry.rs` - Register the definition

### Adding a New Operator

**Before refactoring**: 3+ file changes (lexer.rs, evaluator.rs, typeck.rs)

**After refactoring**: 2 file changes
1. `operators/new_op.rs` - Implement `BinaryOperator`
2. `operators/registry.rs` - Register the operator

### Adding a New Pattern

**Current (already good)**: 2 file changes
1. `patterns/new_pattern.rs` - Implement `PatternDefinition`
2. `patterns/registry.rs` - Register the pattern

### Adding a New Method

**Before refactoring**: 1 large file change (evaluator.rs, inline match)

**After refactoring**: 2 file changes
1. `methods/type_methods.rs` - Implement `MethodDefinition`
2. `methods/registry.rs` - Register the method

### Changing Syntax

**Before refactoring**: Parser is 1706 lines, hard to locate relevant section

**After refactoring**: Parser split into focused 400-line files, easy to find

---

## Appendix A: Complete Registry Interfaces

```rust
// Complete trait definitions for all registries

pub trait ExprDefinition: Send + Sync {
    fn kind(&self) -> ExprKindTag;
    fn type_check(&self, ctx: &mut ExprTypeContext) -> Type;
    fn evaluate(&self, ctx: &ExprEvalContext) -> EvalResult;
}

pub trait BinaryOperator: Send + Sync {
    fn op(&self) -> BinaryOp;
    fn type_check(&self, left: &Type, right: &Type) -> Result<Type, TypeError>;
    fn evaluate(&self, left: Value, right: Value) -> EvalResult;
}

pub trait UnaryOperator: Send + Sync {
    fn op(&self) -> UnaryOp;
    fn type_check(&self, operand: &Type) -> Result<Type, TypeError>;
    fn evaluate(&self, operand: Value) -> EvalResult;
}

pub trait MethodDefinition: Send + Sync {
    fn name(&self) -> &'static str;
    fn receiver_type(&self) -> TypeTag;
    fn param_types(&self) -> &[Type];
    fn return_type(&self) -> Type;
    fn evaluate(&self, receiver: Value, args: Vec<Value>) -> EvalResult;
}

pub trait TypeDefinition: Send + Sync {
    fn id(&self) -> TypeId;
    fn name(&self) -> &'static str;
    fn to_type(&self) -> Type;
    fn supertraits(&self) -> &[TraitId];
}

// PatternDefinition already exists and is well-designed
```

## Appendix B: Example Expression Definition

```rust
// expressions/if_expr.rs
//!
//! If expression: `if cond then expr else expr`
//!
//! Spec Reference: docs/sigil_lang/0.1-alpha/spec/09-expressions.md § Conditionals

pub struct IfExprDefinition;

impl ExprDefinition for IfExprDefinition {
    fn kind(&self) -> ExprKindTag {
        ExprKindTag::If
    }

    fn type_check(&self, ctx: &mut ExprTypeContext) -> Type {
        let cond_type = ctx.check_child(ctx.if_cond());
        ctx.unify(&cond_type, &Type::Bool);

        let then_type = ctx.check_child(ctx.if_then());
        let else_type = ctx.check_child(ctx.if_else());
        ctx.unify(&then_type, &else_type);

        then_type
    }

    fn evaluate(&self, ctx: &ExprEvalContext) -> EvalResult {
        let cond = ctx.eval_child(ctx.if_cond())?;
        if cond.is_truthy() {
            ctx.eval_child(ctx.if_then())
        } else {
            ctx.eval_child(ctx.if_else())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_if_true_branch() {
        // Spec: "if true then A else B" evaluates to A
        let code = "if true then 1 else 2";
        assert_eq!(eval(code), Ok(Value::Int(1)));
    }

    #[test]
    fn test_if_false_branch() {
        // Spec: "if false then A else B" evaluates to B
        let code = "if false then 1 else 2";
        assert_eq!(eval(code), Ok(Value::Int(2)));
    }

    #[test]
    fn test_if_branches_must_match_types() {
        // Spec: then and else branches must have same type
        let code = "if true then 1 else \"string\"";
        assert!(type_check(code).is_err());
    }
}
```

---

## Summary

This refactoring plan transforms the v3 compiler from a moderately extensible codebase to a highly modular one where:

1. **Adding features** requires minimal file changes (typically 2-3 files)
2. **Each component** is independently testable
3. **Dependencies** are injectable for testing
4. **Tests** validate the spec, not workaround broken behavior
5. **No legacy concerns** - full refactors allowed

The pattern system is already well-designed and serves as the template for the other subsystems (expressions, operators, methods, types).
