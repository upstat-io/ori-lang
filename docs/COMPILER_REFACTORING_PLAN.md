# Sigil Compiler Refactoring Plan

A comprehensive plan for making the Sigil compiler modular, extensible, and maintainable as new language features are added.

## Executive Summary

The current compiler has a solid foundation but exhibits several architectural issues that will impede iterative development:

1. **Tight coupling** between patterns, evaluation, and type checking
2. **Inconsistent error handling** (ad-hoc strings vs structured diagnostics)
3. **Unused symbol infrastructure** that should be the backbone of scoping
4. **Growing monolithic contexts** that violate Single Responsibility
5. **Closed extension points** for passes and patterns
6. **Duplicated pattern logic** across 4+ locations

This plan addresses these issues across 5 phases, establishing a clean architecture that can support the planned features: traits, generics, async, capabilities, and more.

---

## Current State Analysis

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                           lib.rs (Entry)                            │
├─────────────────────────────────────────────────────────────────────┤
│  lexer/    │  parser/   │   ast/      │   types/    │    ir/       │
│  ────────  │  ────────  │   ─────     │   ──────    │    ────      │
│  logos     │  recursive │   Expr(33)  │   check/    │   TExpr      │
│  Token     │  descent   │   Item      │   lower/    │   Type       │
│            │  Pratt     │   TypeExpr  │   context   │   TModule    │
├─────────────────────────────────────────────────────────────────────┤
│  eval/           │   codegen/       │   passes/       │  patterns/  │
│  ─────────       │   ────────       │   ───────       │  ─────────  │
│  interpreter     │   tir/c_backend  │   PassManager   │  registry   │
│  patterns/       │   ast/c_backend  │   PatternLower  │  handlers   │
│  value.rs        │                  │   ConstFold     │             │
└─────────────────────────────────────────────────────────────────────┘
```

### SOLID Violations Identified

| Principle | Violation | Location | Impact |
|-----------|-----------|----------|--------|
| **Single Responsibility** | TypeContext manages 4 registries | `types/context.rs` | Hard to extend |
| **Single Responsibility** | Pattern logic in 4+ places | `ast/`, `eval/`, `types/`, `passes/` | Inconsistency |
| **Open/Closed** | Passes hardcoded in PassManager | `passes/mod.rs` | Can't add passes without modification |
| **Liskov Substitution** | PatternHandler::lower() optional | `patterns/` | Breaks contract |
| **Dependency Inversion** | Eval depends on concrete TypeContext | `eval/patterns/` | Tight coupling |
| **DRY** | Pattern evaluation duplicated | `eval/patterns/`, `patterns/builtins/` | Bug divergence |

### Dependency Graph (Current)

```
                 ┌──────────┐
                 │  lexer   │  (no dependencies - good!)
                 └────┬─────┘
                      ▼
                 ┌──────────┐
                 │  parser  │ ──► ast
                 └────┬─────┘
                      ▼
              ┌───────────────┐
              │    types/     │ ◄──┬──► ir/
              │  check, lower │    │
              └───────┬───────┘    │
                      ▼            │
              ┌───────────────┐    │
              │    eval/      │ ───┘ (bidirectional!)
              │  patterns/    │
              └───────┬───────┘
                      ▼
              ┌───────────────┐
              │   codegen/    │
              └───────────────┘
```

---

## Target Architecture

### Guiding Principles

1. **Unidirectional Dependencies** - Data flows one way through the pipeline
2. **Trait-Based Abstractions** - Interfaces over concrete implementations
3. **Registry Pattern** - Extensible collections of handlers/passes
4. **Phase Separation** - Clear boundaries between compilation phases
5. **Centralized Symbols** - Single source of truth for name resolution

### Target Dependency Graph

```
                 ┌──────────┐
                 │  lexer   │
                 └────┬─────┘
                      ▼
                 ┌──────────┐     ┌─────────────┐
                 │  parser  │ ──► │    ast/     │
                 └────┬─────┘     │  + symbols  │
                      ▼           └──────┬──────┘
              ┌───────────────┐          │
              │    types/     │ ◄────────┘
              │  (uses traits)│
              └───────┬───────┘
                      ▼
              ┌───────────────┐
              │      ir/      │  (pure data, no deps)
              └───────┬───────┘
                      ▼
              ┌───────────────┐
              │   passes/     │ ◄── PassRegistry (extensible)
              └───────┬───────┘
                      ▼
              ┌───────────────┐
              │   codegen/    │ ◄── BackendRegistry (extensible)
              └───────┬───────┘
                      │
              ┌───────┴───────┐
              ▼               ▼
          ┌──────┐       ┌───────┐
          │  C   │       │ LLVM  │  (future)
          └──────┘       └───────┘
```

---

## Phase 1: Foundation - Error Handling & Diagnostics

**Goal**: Establish consistent, structured error handling across all compiler phases.

**Duration**: Foundation work

### 1.1 Define Core Error Types

**File**: `compiler/sigilc/src/errors/diagnostic.rs`

```rust
/// Severity levels for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A labeled span within source code
#[derive(Debug, Clone)]
pub struct Label {
    pub span: Span,
    pub message: String,
    pub style: LabelStyle,
}

#[derive(Debug, Clone, Copy)]
pub enum LabelStyle {
    Primary,   // The main error location
    Secondary, // Related locations
}

/// A single diagnostic message
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Option<ErrorCode>,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub suggestions: Vec<Suggestion>,
}

/// An actionable suggestion
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub message: String,
    pub span: Span,
    pub replacement: String,
}

/// Error codes for documentation
#[derive(Debug, Clone, Copy)]
pub struct ErrorCode(pub &'static str);

impl ErrorCode {
    pub const UNDEFINED_VARIABLE: Self = Self("E0001");
    pub const TYPE_MISMATCH: Self = Self("E0002");
    pub const MISSING_TEST: Self = Self("E0500");
    // ... etc
}
```

### 1.2 Create Diagnostic Collector

**File**: `compiler/sigilc/src/errors/collector.rs`

```rust
/// Collects diagnostics during compilation
pub struct DiagnosticCollector {
    diagnostics: Vec<Diagnostic>,
    error_count: usize,
    warning_count: usize,
}

impl DiagnosticCollector {
    pub fn new() -> Self { ... }

    pub fn emit(&mut self, diagnostic: Diagnostic) { ... }

    pub fn error(&mut self, span: Span, message: impl Into<String>) -> DiagnosticBuilder { ... }

    pub fn warning(&mut self, span: Span, message: impl Into<String>) -> DiagnosticBuilder { ... }

    pub fn has_errors(&self) -> bool { ... }

    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> { ... }
}

/// Builder pattern for constructing diagnostics
pub struct DiagnosticBuilder<'a> {
    collector: &'a mut DiagnosticCollector,
    diagnostic: Diagnostic,
}

impl<'a> DiagnosticBuilder<'a> {
    pub fn with_code(mut self, code: ErrorCode) -> Self { ... }
    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self { ... }
    pub fn with_note(mut self, note: impl Into<String>) -> Self { ... }
    pub fn with_suggestion(mut self, span: Span, message: &str, replacement: &str) -> Self { ... }
    pub fn emit(self) { ... }
}
```

### 1.3 Create Phase Result Type

**File**: `compiler/sigilc/src/errors/result.rs`

```rust
/// Result type that carries diagnostics even on success
pub struct PhaseResult<T> {
    pub value: Option<T>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<T> PhaseResult<T> {
    pub fn ok(value: T) -> Self { ... }

    pub fn ok_with_warnings(value: T, diagnostics: Vec<Diagnostic>) -> Self { ... }

    pub fn err(diagnostics: Vec<Diagnostic>) -> Self { ... }

    pub fn has_errors(&self) -> bool { ... }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> PhaseResult<U> { ... }

    pub fn and_then<U>(self, f: impl FnOnce(T) -> PhaseResult<U>) -> PhaseResult<U> { ... }
}
```

### 1.4 Migration Strategy

1. **Add new error types** alongside existing `Result<T, String>`
2. **Create adapter functions** to convert old errors to new diagnostics
3. **Migrate phase by phase**:
   - Lexer (simplest)
   - Parser
   - Type checker
   - Lowering
   - Passes
4. **Remove old error handling** once all phases migrated

### 1.5 Deliverables

- [ ] `errors/diagnostic.rs` - Core diagnostic types
- [ ] `errors/collector.rs` - Diagnostic collection
- [ ] `errors/result.rs` - Phase result type
- [ ] `errors/render.rs` - Terminal rendering (colors, source snippets)
- [ ] `errors/json.rs` - JSON output for tooling
- [ ] `errors/codes.rs` - All error codes with documentation
- [ ] Migrate lexer to new error system
- [ ] Migrate parser to new error system
- [ ] Migrate type checker to new error system

---

## Phase 2: Symbol Table Integration

**Goal**: Integrate the existing `symbols.rs` infrastructure as the backbone of name resolution.

### 2.1 Symbol Table Design

**File**: `compiler/sigilc/src/symbols/mod.rs`

```rust
/// Unique identifier for a symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(u32);

/// Symbol kinds
#[derive(Debug, Clone)]
pub enum SymbolKind {
    Function {
        params: Vec<SymbolId>,
        return_type: TypeId,
        is_test: bool,
    },
    Variable {
        type_id: TypeId,
        mutable: bool,
    },
    Type {
        definition: TypeDefinition,
    },
    Config {
        type_id: TypeId,
    },
    Pattern {
        handler: PatternKind,
    },
    Module {
        exports: Vec<SymbolId>,
    },
    Trait {
        methods: Vec<SymbolId>,
    },
}

/// A symbol in the symbol table
#[derive(Debug, Clone)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub span: Span,
    pub visibility: Visibility,
    pub module: ModuleId,
}

/// The global symbol table
pub struct SymbolTable {
    symbols: Vec<Symbol>,
    name_to_id: HashMap<(ModuleId, String), SymbolId>,
    scopes: Vec<Scope>,
}

impl SymbolTable {
    pub fn define(&mut self, name: &str, kind: SymbolKind, span: Span) -> SymbolId { ... }
    pub fn lookup(&self, name: &str) -> Option<SymbolId> { ... }
    pub fn lookup_in_module(&self, module: ModuleId, name: &str) -> Option<SymbolId> { ... }
    pub fn get(&self, id: SymbolId) -> &Symbol { ... }
    pub fn enter_scope(&mut self) { ... }
    pub fn exit_scope(&mut self) { ... }
}
```

### 2.2 Scope Management

**File**: `compiler/sigilc/src/symbols/scope.rs`

```rust
/// A lexical scope
#[derive(Debug)]
pub struct Scope {
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub symbols: HashMap<String, SymbolId>,
}

#[derive(Debug, Clone, Copy)]
pub enum ScopeKind {
    Module,
    Function,
    Block,
    Loop,
    Match,
}

/// Scope manager for symbol resolution
pub struct ScopeManager {
    scopes: Vec<Scope>,
    current: ScopeId,
}

impl ScopeManager {
    pub fn enter(&mut self, kind: ScopeKind) -> ScopeId { ... }
    pub fn exit(&mut self) { ... }
    pub fn define(&mut self, name: &str, symbol: SymbolId) -> Result<(), SymbolId> { ... }
    pub fn resolve(&self, name: &str) -> Option<SymbolId> { ... }
    pub fn resolve_in_scope(&self, scope: ScopeId, name: &str) -> Option<SymbolId> { ... }
}
```

### 2.3 Type Registry Integration

**File**: `compiler/sigilc/src/symbols/types.rs`

```rust
/// Unique type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(u32);

/// Type definitions in the symbol table
pub struct TypeRegistry {
    types: Vec<TypeDefinition>,
    primitives: HashMap<&'static str, TypeId>,
}

impl TypeRegistry {
    pub fn register(&mut self, def: TypeDefinition) -> TypeId { ... }
    pub fn get(&self, id: TypeId) -> &TypeDefinition { ... }
    pub fn primitive(&self, name: &str) -> TypeId { ... }

    // Built-in types
    pub fn int(&self) -> TypeId { ... }
    pub fn float(&self) -> TypeId { ... }
    pub fn bool(&self) -> TypeId { ... }
    pub fn str(&self) -> TypeId { ... }
    pub fn void(&self) -> TypeId { ... }
}
```

### 2.4 Resolution Pass

Create a dedicated name resolution pass that populates the symbol table:

**File**: `compiler/sigilc/src/resolve/mod.rs`

```rust
/// Name resolution pass
pub struct Resolver<'a> {
    symbols: &'a mut SymbolTable,
    scopes: ScopeManager,
    collector: &'a mut DiagnosticCollector,
}

impl<'a> Resolver<'a> {
    pub fn resolve_module(&mut self, module: &Module) -> PhaseResult<()> { ... }

    fn resolve_item(&mut self, item: &Item) { ... }
    fn resolve_function(&mut self, func: &FunctionDef) { ... }
    fn resolve_expr(&mut self, expr: &Expr) { ... }
}
```

### 2.5 Deliverables

- [ ] `symbols/mod.rs` - Core symbol types and table
- [ ] `symbols/scope.rs` - Scope management
- [ ] `symbols/types.rs` - Type registry
- [ ] `resolve/mod.rs` - Name resolution pass
- [ ] Integration with parser (attach SymbolIds to AST nodes)
- [ ] Migration of TypeContext to use SymbolTable

---

## Phase 3: Pattern Abstraction Layer

**Goal**: Create a single source of truth for pattern definitions, eliminating duplication.

### 3.1 Pattern Definition Trait

**File**: `compiler/sigilc/src/patterns/definition.rs`

```rust
/// Complete definition of a pattern
pub trait PatternDefinition: Send + Sync {
    /// Pattern keyword (e.g., "fold", "map")
    fn keyword(&self) -> &'static str;

    /// Required parameters
    fn required_params(&self) -> &[ParamSpec];

    /// Optional parameters with defaults
    fn optional_params(&self) -> &[ParamSpec];

    /// Validate parameter types
    fn check_types(
        &self,
        args: &PatternArgs,
        ctx: &dyn TypeContext,
    ) -> Result<Type, Vec<Diagnostic>>;

    /// Infer result type from parameters
    fn infer_type(
        &self,
        args: &PatternArgs,
        ctx: &dyn TypeContext,
    ) -> Result<Type, Vec<Diagnostic>>;

    /// Evaluate pattern (for interpreter)
    fn evaluate(
        &self,
        args: &PatternArgs,
        env: &mut Environment,
    ) -> Result<Value, EvalError>;

    /// Lower to IR (for codegen)
    fn lower(
        &self,
        args: &PatternArgs,
        ctx: &mut LoweringContext,
    ) -> Result<TExpr, Vec<Diagnostic>>;
}

/// Parameter specification
#[derive(Debug, Clone)]
pub struct ParamSpec {
    pub name: &'static str,
    pub type_constraint: TypeConstraint,
    pub description: &'static str,
}

#[derive(Debug, Clone)]
pub enum TypeConstraint {
    Any,
    Exact(Type),
    Iterable,
    Callable { params: usize, returns: Option<Type> },
    Numeric,
    Boolean,
}
```

### 3.2 Pattern Registry

**File**: `compiler/sigilc/src/patterns/registry.rs`

```rust
/// Global registry of all patterns
pub struct PatternRegistry {
    patterns: HashMap<&'static str, Box<dyn PatternDefinition>>,
}

impl PatternRegistry {
    pub fn new() -> Self { ... }

    pub fn register(&mut self, pattern: impl PatternDefinition + 'static) { ... }

    pub fn get(&self, keyword: &str) -> Option<&dyn PatternDefinition> { ... }

    pub fn keywords(&self) -> impl Iterator<Item = &'static str> { ... }

    /// Create registry with all built-in patterns
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register(FoldPattern);
        registry.register(MapPattern);
        registry.register(FilterPattern);
        registry.register(RecursePattern);
        registry.register(CollectPattern);
        registry.register(MatchPattern);
        registry.register(RunPattern);
        registry.register(ParallelPattern);
        registry.register(TryPattern);
        // ... etc
        registry
    }
}

// Thread-safe global access
lazy_static! {
    pub static ref PATTERNS: PatternRegistry = PatternRegistry::with_builtins();
}
```

### 3.3 Example Pattern Implementation

**File**: `compiler/sigilc/src/patterns/builtins/fold.rs`

```rust
pub struct FoldPattern;

impl PatternDefinition for FoldPattern {
    fn keyword(&self) -> &'static str { "fold" }

    fn required_params(&self) -> &[ParamSpec] {
        &[
            ParamSpec {
                name: "over",
                type_constraint: TypeConstraint::Iterable,
                description: "Collection to fold over",
            },
            ParamSpec {
                name: "init",
                type_constraint: TypeConstraint::Any,
                description: "Initial accumulator value",
            },
            ParamSpec {
                name: "with",
                type_constraint: TypeConstraint::Callable { params: 2, returns: None },
                description: "Folding function (acc, item) -> acc",
            },
        ]
    }

    fn optional_params(&self) -> &[ParamSpec] { &[] }

    fn check_types(&self, args: &PatternArgs, ctx: &dyn TypeContext) -> Result<Type, Vec<Diagnostic>> {
        let over_type = ctx.type_of(&args.get("over")?)?;
        let init_type = ctx.type_of(&args.get("init")?)?;
        let with_type = ctx.type_of(&args.get("with")?)?;

        // Verify 'over' is iterable
        let element_type = ctx.element_type(&over_type)?;

        // Verify 'with' accepts (init_type, element_type) -> init_type
        ctx.check_callable(&with_type, &[init_type.clone(), element_type], &init_type)?;

        Ok(init_type)
    }

    fn infer_type(&self, args: &PatternArgs, ctx: &dyn TypeContext) -> Result<Type, Vec<Diagnostic>> {
        ctx.type_of(&args.get("init")?)
    }

    fn evaluate(&self, args: &PatternArgs, env: &mut Environment) -> Result<Value, EvalError> {
        let over = args.eval("over", env)?;
        let init = args.eval("init", env)?;
        let with = args.eval("with", env)?;

        let items = over.as_list()?;
        let mut acc = init;

        for item in items {
            acc = env.call(&with, vec![acc, item])?;
        }

        Ok(acc)
    }

    fn lower(&self, args: &PatternArgs, ctx: &mut LoweringContext) -> Result<TExpr, Vec<Diagnostic>> {
        // Lower to loop construct in TIR
        let over_expr = ctx.lower(&args.get("over")?)?;
        let init_expr = ctx.lower(&args.get("init")?)?;
        let with_expr = ctx.lower(&args.get("with")?)?;

        Ok(TExpr::FoldLoop {
            collection: Box::new(over_expr),
            init: Box::new(init_expr),
            folder: Box::new(with_expr),
        })
    }
}
```

### 3.4 Integration Points

Update existing code to use the pattern registry:

```rust
// In type checker
fn check_pattern_expr(&mut self, pattern: &PatternExpr) -> Result<Type, Diagnostic> {
    let definition = PATTERNS.get(&pattern.keyword)
        .ok_or_else(|| self.error(pattern.span, format!("unknown pattern: {}", pattern.keyword)))?;

    definition.check_types(&pattern.args, &self.context)
}

// In interpreter
fn eval_pattern(&mut self, pattern: &PatternExpr) -> Result<Value, EvalError> {
    let definition = PATTERNS.get(&pattern.keyword).unwrap();
    definition.evaluate(&pattern.args, &mut self.env)
}

// In lowering
fn lower_pattern(&mut self, pattern: &PatternExpr) -> Result<TExpr, Diagnostic> {
    let definition = PATTERNS.get(&pattern.keyword).unwrap();
    definition.lower(&pattern.args, &mut self.ctx)
}
```

### 3.5 Deliverables

- [ ] `patterns/definition.rs` - Pattern trait definition
- [ ] `patterns/registry.rs` - Pattern registry
- [ ] `patterns/args.rs` - Pattern argument handling
- [ ] Migrate all 13 pattern implementations to new trait
- [ ] Remove duplicated pattern code from `eval/patterns/`
- [ ] Update type checker to use registry
- [ ] Update interpreter to use registry
- [ ] Update lowering to use registry

---

## Phase 4: Context Decomposition

**Goal**: Split monolithic TypeContext into focused, phase-specific contexts.

### 4.1 Define Context Traits

**File**: `compiler/sigilc/src/context/traits.rs`

```rust
/// Type lookup capabilities
pub trait TypeLookup {
    fn lookup_type(&self, name: &str) -> Option<TypeId>;
    fn get_type(&self, id: TypeId) -> &Type;
    fn primitive_type(&self, name: &str) -> TypeId;
}

/// Function lookup capabilities
pub trait FunctionLookup {
    fn lookup_function(&self, name: &str) -> Option<SymbolId>;
    fn get_function_signature(&self, id: SymbolId) -> &FunctionSignature;
}

/// Variable scope capabilities
pub trait VariableScope {
    fn define_variable(&mut self, name: &str, type_id: TypeId, mutable: bool) -> SymbolId;
    fn lookup_variable(&self, name: &str) -> Option<SymbolId>;
    fn get_variable_type(&self, id: SymbolId) -> TypeId;
    fn is_mutable(&self, id: SymbolId) -> bool;
}

/// Type inference capabilities
pub trait TypeInference {
    fn unify(&mut self, expected: TypeId, actual: TypeId) -> Result<TypeId, TypeError>;
    fn instantiate_generic(&mut self, generic: TypeId, args: &[TypeId]) -> TypeId;
    fn infer_from_usage(&mut self, expr: &Expr) -> TypeId;
}

/// Combined context for type checking
pub trait TypeContext: TypeLookup + FunctionLookup + VariableScope + TypeInference {}
```

### 4.2 Phase-Specific Contexts

**File**: `compiler/sigilc/src/context/check.rs`

```rust
/// Context for type checking phase
pub struct CheckContext<'a> {
    symbols: &'a mut SymbolTable,
    types: &'a mut TypeRegistry,
    diagnostics: &'a mut DiagnosticCollector,

    // Type inference state
    type_vars: HashMap<TypeVarId, Option<TypeId>>,
    constraints: Vec<TypeConstraint>,
}

impl TypeLookup for CheckContext<'_> { ... }
impl FunctionLookup for CheckContext<'_> { ... }
impl VariableScope for CheckContext<'_> { ... }
impl TypeInference for CheckContext<'_> { ... }
impl TypeContext for CheckContext<'_> {}
```

**File**: `compiler/sigilc/src/context/lower.rs`

```rust
/// Context for lowering phase (AST -> TIR)
pub struct LowerContext<'a> {
    symbols: &'a SymbolTable,  // Immutable - resolution complete
    types: &'a TypeRegistry,
    diagnostics: &'a mut DiagnosticCollector,

    // Lowering state
    locals: LocalTable,
    current_function: Option<SymbolId>,
}
```

**File**: `compiler/sigilc/src/context/eval.rs`

```rust
/// Context for interpretation phase
pub struct EvalContext {
    symbols: SymbolTable,
    types: TypeRegistry,

    // Runtime state
    values: HashMap<SymbolId, Value>,
    call_stack: Vec<StackFrame>,
}
```

### 4.3 Context Factory

**File**: `compiler/sigilc/src/context/factory.rs`

```rust
/// Creates appropriate context for each compilation phase
pub struct ContextFactory {
    symbols: SymbolTable,
    types: TypeRegistry,
}

impl ContextFactory {
    pub fn new() -> Self { ... }

    pub fn resolve_context(&mut self, diagnostics: &mut DiagnosticCollector) -> ResolveContext { ... }

    pub fn check_context(&mut self, diagnostics: &mut DiagnosticCollector) -> CheckContext { ... }

    pub fn lower_context(&self, diagnostics: &mut DiagnosticCollector) -> LowerContext { ... }

    pub fn eval_context(self) -> EvalContext { ... }
}
```

### 4.4 Deliverables

- [ ] `context/traits.rs` - Context trait definitions
- [ ] `context/check.rs` - Type checking context
- [ ] `context/lower.rs` - Lowering context
- [ ] `context/eval.rs` - Evaluation context
- [ ] `context/factory.rs` - Context factory
- [ ] Migrate type checker to use CheckContext
- [ ] Migrate lowering to use LowerContext
- [ ] Migrate interpreter to use EvalContext
- [ ] Remove old TypeContext

---

## Phase 5: Pass System Enhancement

**Goal**: Make the pass system fully extensible with dependency management.

### 5.1 Enhanced Pass Trait

**File**: `compiler/sigilc/src/passes/pass.rs`

```rust
/// Metadata about a pass
#[derive(Debug, Clone)]
pub struct PassInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub dependencies: &'static [&'static str],
    pub required: bool,
}

/// A compiler pass that transforms IR
pub trait Pass: Send + Sync {
    fn info(&self) -> PassInfo;

    fn run(&self, module: &mut TModule, ctx: &mut PassContext) -> PassResult;

    /// Optional: Verify pass preconditions
    fn verify_pre(&self, _module: &TModule) -> Result<(), String> { Ok(()) }

    /// Optional: Verify pass postconditions
    fn verify_post(&self, _module: &TModule) -> Result<(), String> { Ok(()) }
}

/// Result of a pass execution
pub struct PassResult {
    pub changed: bool,
    pub diagnostics: Vec<Diagnostic>,
}
```

### 5.2 Pass Registry with Dependency Resolution

**File**: `compiler/sigilc/src/passes/registry.rs`

```rust
/// Registry of all available passes
pub struct PassRegistry {
    passes: HashMap<&'static str, Box<dyn Pass>>,
}

impl PassRegistry {
    pub fn new() -> Self { ... }

    pub fn register(&mut self, pass: impl Pass + 'static) { ... }

    pub fn get(&self, name: &str) -> Option<&dyn Pass> { ... }

    /// Resolve dependencies and return passes in execution order
    pub fn resolve_order(&self, requested: &[&str]) -> Result<Vec<&dyn Pass>, DependencyError> {
        // Topological sort based on dependencies
        ...
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(ConstantFoldingPass);
        registry.register(DeadCodeEliminationPass);
        registry.register(PatternLoweringPass);
        registry.register(InliningPass);
        registry.register(TypeErasurePass);
        registry
    }
}
```

### 5.3 Pass Pipeline Builder

**File**: `compiler/sigilc/src/passes/pipeline.rs`

```rust
/// Builds and executes pass pipelines
pub struct PassPipeline {
    registry: PassRegistry,
    enabled: HashSet<&'static str>,
    disabled: HashSet<&'static str>,
}

impl PassPipeline {
    pub fn new(registry: PassRegistry) -> Self { ... }

    pub fn enable(&mut self, pass: &'static str) -> &mut Self { ... }

    pub fn disable(&mut self, pass: &'static str) -> &mut Self { ... }

    pub fn run(&self, module: &mut TModule) -> PipelineResult {
        let passes = self.registry.resolve_order(&self.collect_passes())?;

        let mut ctx = PassContext::new();
        let mut changed = false;

        for pass in passes {
            if let Err(e) = pass.verify_pre(module) {
                return Err(PipelineError::PreConditionFailed(pass.info().name, e));
            }

            let result = pass.run(module, &mut ctx)?;
            changed |= result.changed;

            if let Err(e) = pass.verify_post(module) {
                return Err(PipelineError::PostConditionFailed(pass.info().name, e));
            }
        }

        Ok(PipelineResult { changed, diagnostics: ctx.diagnostics })
    }
}

/// Default optimization pipeline
pub fn default_pipeline() -> PassPipeline {
    PassPipeline::new(PassRegistry::with_defaults())
        .enable("constant_folding")
        .enable("dead_code_elimination")
        .enable("pattern_lowering")
}

/// Debug pipeline (minimal optimization)
pub fn debug_pipeline() -> PassPipeline {
    PassPipeline::new(PassRegistry::with_defaults())
        .enable("pattern_lowering")  // Required
}
```

### 5.4 Analysis Passes

**File**: `compiler/sigilc/src/passes/analysis.rs`

```rust
/// Pass that produces analysis results without modifying IR
pub trait AnalysisPass: Send + Sync {
    type Result;

    fn info(&self) -> PassInfo;

    fn analyze(&self, module: &TModule, ctx: &PassContext) -> Self::Result;
}

// Example analyses
pub struct ControlFlowAnalysis;
pub struct DataFlowAnalysis;
pub struct AliasAnalysis;
pub struct TestCoverageAnalysis;
```

### 5.5 Deliverables

- [ ] `passes/pass.rs` - Enhanced pass trait
- [ ] `passes/registry.rs` - Pass registry with dependencies
- [ ] `passes/pipeline.rs` - Pipeline builder
- [ ] `passes/analysis.rs` - Analysis pass framework
- [ ] Migrate existing passes to new framework
- [ ] Add dependency metadata to all passes
- [ ] Create default pipelines (debug, release, size)

---

## Phase 6: Backend Abstraction

**Goal**: Enable multiple code generation backends with a clean interface.

### 6.1 Backend Trait

**File**: `compiler/sigilc/src/codegen/backend.rs`

```rust
/// A code generation backend
pub trait Backend: Send + Sync {
    /// Backend identifier
    fn name(&self) -> &'static str;

    /// Supported target triples
    fn targets(&self) -> &[&'static str];

    /// Generate code from TIR module
    fn generate(
        &self,
        module: &TModule,
        options: &CodegenOptions,
    ) -> Result<GeneratedCode, CodegenError>;

    /// Emit to file
    fn emit(
        &self,
        code: &GeneratedCode,
        path: &Path,
    ) -> Result<(), io::Error>;
}

/// Generated code output
pub enum GeneratedCode {
    CSource(String),
    LlvmIR(String),
    ObjectFile(Vec<u8>),
    Assembly(String),
}

/// Code generation options
#[derive(Debug, Clone)]
pub struct CodegenOptions {
    pub optimization_level: OptLevel,
    pub debug_info: bool,
    pub target: Option<String>,
}
```

### 6.2 Backend Registry

**File**: `compiler/sigilc/src/codegen/registry.rs`

```rust
pub struct BackendRegistry {
    backends: HashMap<&'static str, Box<dyn Backend>>,
    default: &'static str,
}

impl BackendRegistry {
    pub fn new() -> Self { ... }

    pub fn register(&mut self, backend: impl Backend + 'static) { ... }

    pub fn get(&self, name: &str) -> Option<&dyn Backend> { ... }

    pub fn default(&self) -> &dyn Backend { ... }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(CBackend::new());
        // Future: registry.register(LlvmBackend::new());
        registry.default = "c";
        registry
    }
}
```

### 6.3 Deliverables

- [ ] `codegen/backend.rs` - Backend trait
- [ ] `codegen/registry.rs` - Backend registry
- [ ] Refactor C backend to implement trait
- [ ] Prepare interface for future LLVM backend

---

## Phase 7: Trait System Foundation

**Goal**: Lay groundwork for the trait system (needed for planned features).

### 7.1 Trait Definitions

**File**: `compiler/sigilc/src/traits/definition.rs`

```rust
/// A trait definition
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub id: TraitId,
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub methods: Vec<TraitMethod>,
    pub associated_types: Vec<AssociatedType>,
    pub supertraits: Vec<TraitBound>,
}

/// A method in a trait
#[derive(Debug, Clone)]
pub struct TraitMethod {
    pub name: String,
    pub signature: FunctionSignature,
    pub default_impl: Option<Expr>,
}

/// An associated type in a trait
#[derive(Debug, Clone)]
pub struct AssociatedType {
    pub name: String,
    pub bounds: Vec<TraitBound>,
    pub default: Option<TypeId>,
}

/// A trait bound (e.g., T: Display + Debug)
#[derive(Debug, Clone)]
pub struct TraitBound {
    pub trait_id: TraitId,
    pub type_args: Vec<TypeId>,
}
```

### 7.2 Trait Implementation

**File**: `compiler/sigilc/src/traits/impl.rs`

```rust
/// A trait implementation
#[derive(Debug, Clone)]
pub struct TraitImpl {
    pub id: ImplId,
    pub trait_id: TraitId,
    pub implementing_type: TypeId,
    pub type_params: Vec<TypeParam>,
    pub where_clause: Vec<WhereClause>,
    pub methods: Vec<MethodImpl>,
    pub associated_types: Vec<AssociatedTypeImpl>,
}

/// Implementation of a trait method
#[derive(Debug, Clone)]
pub struct MethodImpl {
    pub method_name: String,
    pub body: Expr,
}
```

### 7.3 Trait Registry

**File**: `compiler/sigilc/src/traits/registry.rs`

```rust
pub struct TraitRegistry {
    traits: HashMap<TraitId, TraitDef>,
    impls: Vec<TraitImpl>,
    impl_cache: HashMap<(TypeId, TraitId), Option<ImplId>>,
}

impl TraitRegistry {
    pub fn register_trait(&mut self, def: TraitDef) -> TraitId { ... }

    pub fn register_impl(&mut self, impl_: TraitImpl) -> ImplId { ... }

    pub fn find_impl(&self, type_id: TypeId, trait_id: TraitId) -> Option<&TraitImpl> { ... }

    pub fn implements(&self, type_id: TypeId, trait_id: TraitId) -> bool { ... }

    pub fn method_impl(&self, type_id: TypeId, trait_id: TraitId, method: &str) -> Option<&MethodImpl> { ... }
}
```

### 7.4 Deliverables

- [ ] `traits/definition.rs` - Trait definition types
- [ ] `traits/impl.rs` - Implementation types
- [ ] `traits/registry.rs` - Trait registry
- [ ] `traits/resolution.rs` - Trait resolution algorithm
- [ ] Integration with type checker
- [ ] Built-in traits (Display, Debug, Clone, etc.)

---

## Implementation Roadmap

### Execution Order

```
Phase 1: Error Handling        ─┬─► Phase 2: Symbol Table
         (2-3 weeks)            │            (2-3 weeks)
                                │
                                └─► Phase 3: Pattern Abstraction
                                             (2 weeks)
                                             │
                                             ▼
                                    Phase 4: Context Decomposition
                                             (2 weeks)
                                             │
                                             ▼
                                    Phase 5: Pass System
                                             (1-2 weeks)
                                             │
                                             ▼
                                    Phase 6: Backend Abstraction
                                             (1 week)
                                             │
                                             ▼
                                    Phase 7: Trait Foundation
                                             (2-3 weeks)
```

### Phase Dependencies

| Phase | Depends On | Enables |
|-------|------------|---------|
| 1. Error Handling | None | All subsequent phases |
| 2. Symbol Table | Phase 1 | Phases 3, 4, 7 |
| 3. Pattern Abstraction | Phases 1, 2 | Clean pattern extension |
| 4. Context Decomposition | Phases 1, 2 | Phase 5, cleaner code |
| 5. Pass System | Phases 1, 4 | Phase 6, optimization work |
| 6. Backend Abstraction | Phase 5 | Multiple backends |
| 7. Trait Foundation | Phases 1, 2, 4 | Full trait system |

### Risk Mitigation

1. **Incremental Migration**: Each phase uses adapters to work with existing code
2. **Feature Flags**: New systems can be toggled off during development
3. **Parallel Development**: Phases 2 and 3 can proceed in parallel after Phase 1
4. **Test Coverage**: Each phase includes migration of existing tests
5. **Rollback Points**: Git tags at each phase completion for easy rollback

---

## Testing Strategy

### Unit Tests for Each Phase

```rust
// Phase 1: Error handling
#[test]
fn diagnostic_builder_creates_correct_structure() { ... }

#[test]
fn error_collector_tracks_error_count() { ... }

// Phase 2: Symbol table
#[test]
fn symbol_table_resolves_nested_scopes() { ... }

#[test]
fn shadowing_works_correctly() { ... }

// Phase 3: Patterns
#[test]
fn fold_pattern_checks_types_correctly() { ... }

#[test]
fn pattern_registry_contains_all_builtins() { ... }
```

### Integration Tests

```rust
// Ensure existing behavior preserved
#[test]
fn all_rosetta_tests_still_pass() {
    for test_file in glob("tests/run-pass/rosetta/**/*.si") {
        assert!(compile_and_run(test_file).is_ok());
    }
}

#[test]
fn all_compile_fail_tests_still_fail() {
    for test_file in glob("tests/compile-fail/**/*.si") {
        assert!(compile(test_file).is_err());
    }
}
```

### Regression Testing

- Snapshot tests for error messages
- AST dump comparisons
- Performance benchmarks (compile time)

---

## Success Criteria

### Phase 1: Error Handling
- [ ] All compiler errors use structured Diagnostic type
- [ ] Error messages include source snippets and suggestions
- [ ] JSON error output works for tooling
- [ ] Error codes documented

### Phase 2: Symbol Table
- [ ] All name resolution uses SymbolTable
- [ ] Scoping rules correctly implemented
- [ ] Forward references work
- [ ] Shadowing behaves as documented

### Phase 3: Pattern Abstraction
- [ ] Single PatternDefinition trait for all patterns
- [ ] No duplicated pattern logic
- [ ] Adding new pattern requires only one file
- [ ] All 13 patterns migrated

### Phase 4: Context Decomposition
- [ ] Phase-specific contexts (Check, Lower, Eval)
- [ ] Contexts implement focused traits
- [ ] No monolithic TypeContext
- [ ] Clear ownership of state

### Phase 5: Pass System
- [ ] Passes registered dynamically
- [ ] Dependencies resolved automatically
- [ ] Custom pipelines easy to create
- [ ] Analysis passes supported

### Phase 6: Backend Abstraction
- [ ] Backend trait defined
- [ ] C backend implements trait
- [ ] Registry supports multiple backends
- [ ] New backends easy to add

### Phase 7: Trait Foundation
- [ ] Trait definitions parsed and stored
- [ ] Impl blocks resolved
- [ ] Basic trait bounds checked
- [ ] Built-in traits available

---

## Appendix: File Structure After Refactoring

```
compiler/sigilc/src/
├── lib.rs
├── main.rs
│
├── errors/
│   ├── mod.rs
│   ├── diagnostic.rs    # Diagnostic types
│   ├── collector.rs     # Error collection
│   ├── result.rs        # PhaseResult type
│   ├── render.rs        # Terminal rendering
│   ├── json.rs          # JSON output
│   └── codes.rs         # Error codes
│
├── symbols/
│   ├── mod.rs
│   ├── table.rs         # SymbolTable
│   ├── scope.rs         # ScopeManager
│   ├── types.rs         # TypeRegistry
│   └── ids.rs           # SymbolId, TypeId, etc.
│
├── resolve/
│   ├── mod.rs           # Name resolution pass
│   └── imports.rs       # Import resolution
│
├── context/
│   ├── mod.rs
│   ├── traits.rs        # Context traits
│   ├── check.rs         # CheckContext
│   ├── lower.rs         # LowerContext
│   ├── eval.rs          # EvalContext
│   └── factory.rs       # ContextFactory
│
├── patterns/
│   ├── mod.rs
│   ├── definition.rs    # PatternDefinition trait
│   ├── registry.rs      # PatternRegistry
│   ├── args.rs          # PatternArgs handling
│   └── builtins/
│       ├── mod.rs
│       ├── fold.rs
│       ├── map.rs
│       ├── filter.rs
│       ├── recurse.rs
│       ├── collect.rs
│       ├── match_.rs
│       ├── run.rs
│       ├── parallel.rs
│       ├── try_.rs
│       └── ... (other patterns)
│
├── traits/
│   ├── mod.rs
│   ├── definition.rs    # TraitDef
│   ├── impl.rs          # TraitImpl
│   ├── registry.rs      # TraitRegistry
│   ├── resolution.rs    # Trait resolution
│   └── builtins.rs      # Built-in traits
│
├── passes/
│   ├── mod.rs
│   ├── pass.rs          # Pass trait
│   ├── registry.rs      # PassRegistry
│   ├── pipeline.rs      # PassPipeline
│   ├── analysis.rs      # AnalysisPass trait
│   └── builtin/
│       ├── const_fold.rs
│       ├── dead_code.rs
│       ├── pattern_lower.rs
│       └── ... (other passes)
│
├── codegen/
│   ├── mod.rs
│   ├── backend.rs       # Backend trait
│   ├── registry.rs      # BackendRegistry
│   ├── options.rs       # CodegenOptions
│   └── backends/
│       ├── c/
│       │   ├── mod.rs
│       │   ├── expr.rs
│       │   ├── types.rs
│       │   └── emit.rs
│       └── llvm/         # Future
│
├── lexer/               # (unchanged)
├── parser/              # (unchanged)
├── ast/                 # (minor updates for SymbolId)
├── ir/                  # (unchanged)
├── eval/                # (uses EvalContext, patterns from registry)
├── cli/                 # (unchanged)
└── builtins/            # (unchanged)
```

---

## Conclusion

This refactoring plan transforms the Sigil compiler from a working prototype into an extensible, maintainable system ready for significant feature additions. Each phase builds on the previous, with clear deliverables and success criteria.

The key architectural improvements are:

1. **Consistent error handling** - Better UX, easier debugging
2. **Centralized symbols** - Proper scoping, enables traits/generics
3. **Unified patterns** - Single source of truth, easy to extend
4. **Focused contexts** - Clear responsibilities, testable
5. **Extensible passes** - Optimization work without core changes
6. **Backend abstraction** - Multiple targets possible
7. **Trait foundation** - Enables rich type system features

Following SOLID principles throughout ensures the compiler remains maintainable as the language grows.
