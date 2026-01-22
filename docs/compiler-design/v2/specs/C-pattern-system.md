# C: Pattern System Specification

This document specifies the pattern system architecture for the V2 compiler.

---

## Pattern Definition Trait

```rust
/// Core trait for pattern implementations
///
/// Each pattern implements this trait to provide:
/// - Parsing of pattern syntax
/// - Type checking of pattern arguments
/// - Evaluation at runtime
/// - Code generation
/// - Template information for caching
pub trait PatternDefinition: Send + Sync + 'static {
    /// Pattern keyword (e.g., "map", "filter", "fold")
    fn name(&self) -> &'static str;

    /// Required named arguments
    fn required_args(&self) -> &'static [&'static str];

    /// Optional named arguments with their default values
    fn optional_args(&self) -> &'static [(&'static str, DefaultValue)];

    /// Parse pattern from tokens
    ///
    /// Called when parser encounters `pattern_name(` in expression position
    fn parse(&self, parser: &mut PatternParser) -> Result<PatternNode, ParseError>;

    /// Type check pattern and return result type
    fn type_check(
        &self,
        ctx: &mut TypeContext,
        args: &PatternArgs,
        arena: &ExprArena,
    ) -> Result<TypeId, TypeError>;

    /// Evaluate pattern at runtime
    fn evaluate(
        &self,
        env: &mut Environment,
        args: &PatternArgs,
        arena: &ExprArena,
    ) -> Result<Value, RuntimeError>;

    /// Generate C code for pattern
    fn codegen(
        &self,
        ctx: &mut CodegenContext,
        args: &PatternArgs,
        arena: &ExprArena,
    ) -> Result<CCode, CodegenError>;

    /// Compute pattern signature for template caching
    fn signature(
        &self,
        args: &PatternArgs,
        interner: &TypeInterner,
    ) -> PatternSignature;

    /// Can this pattern be fused with the given next pattern?
    ///
    /// Default: no fusion
    fn can_fuse_with(&self, next: &dyn PatternDefinition) -> bool {
        false
    }

    /// Create fused pattern if possible
    fn fuse_with(
        &self,
        _next: &dyn PatternDefinition,
        _self_args: &PatternArgs,
        _next_args: &PatternArgs,
    ) -> Option<FusedPattern> {
        None
    }
}

/// Default value for optional pattern argument
#[derive(Clone)]
pub enum DefaultValue {
    /// No default (argument truly optional, uses pattern-specific handling)
    None,
    /// Boolean default
    Bool(bool),
    /// Integer default
    Int(i64),
    /// Lambda expression as source code
    Lambda(&'static str),
}
```

---

## Pattern Registry

```rust
/// Global registry of all available patterns
pub struct PatternRegistry {
    /// Name → Pattern implementation
    patterns: FxHashMap<Name, Arc<dyn PatternDefinition>>,

    /// Set of pattern keywords (for lexer context-sensitivity)
    keywords: FxHashSet<&'static str>,
}

impl PatternRegistry {
    /// Create registry with all built-in patterns
    pub fn new(interner: &StringInterner) -> Self {
        let mut registry = Self {
            patterns: FxHashMap::default(),
            keywords: FxHashSet::default(),
        };

        // Sequential patterns
        registry.register(interner, RunPattern);
        registry.register(interner, TryPattern);

        // Matching
        registry.register(interner, MatchPattern);

        // Collection patterns
        registry.register(interner, MapPattern);
        registry.register(interner, FilterPattern);
        registry.register(interner, FoldPattern);
        registry.register(interner, FindPattern);
        registry.register(interner, CollectPattern);

        // Recursion
        registry.register(interner, RecursePattern);

        // Concurrency
        registry.register(interner, ParallelPattern);
        registry.register(interner, TimeoutPattern);
        registry.register(interner, RetryPattern);

        // Caching
        registry.register(interner, CachePattern);

        // Validation
        registry.register(interner, ValidatePattern);

        registry
    }

    fn register<P: PatternDefinition>(&mut self, interner: &StringInterner, pattern: P) {
        let name = interner.intern(pattern.name());
        self.keywords.insert(pattern.name());
        self.patterns.insert(name, Arc::new(pattern));
    }

    /// Get pattern by name
    pub fn get(&self, name: Name) -> Option<Arc<dyn PatternDefinition>> {
        self.patterns.get(&name).cloned()
    }

    /// Check if string is a pattern keyword
    pub fn is_pattern_keyword(&self, s: &str) -> bool {
        self.keywords.contains(s)
    }

    /// Iterate all patterns
    pub fn iter(&self) -> impl Iterator<Item = (&Name, &Arc<dyn PatternDefinition>)> {
        self.patterns.iter()
    }
}
```

---

## Pattern Arguments

```rust
/// Parsed pattern arguments
#[derive(Clone)]
pub struct PatternArgs {
    /// Named arguments: .name: expr
    pub named: FxHashMap<Name, ExprId>,

    /// Span of entire argument list
    pub span: Span,
}

impl PatternArgs {
    /// Get required argument (panics if missing - should be validated)
    pub fn get(&self, name: Name) -> ExprId {
        self.named[&name]
    }

    /// Get optional argument
    pub fn get_opt(&self, name: Name) -> Option<ExprId> {
        self.named.get(&name).copied()
    }

    /// Check if argument is present
    pub fn has(&self, name: Name) -> bool {
        self.named.contains_key(&name)
    }

    /// Iterate all arguments
    pub fn iter(&self) -> impl Iterator<Item = (Name, ExprId)> + '_ {
        self.named.iter().map(|(&k, &v)| (k, v))
    }
}

/// Pattern argument specification
pub struct ArgSpec {
    pub name: &'static str,
    pub required: bool,
    pub default: Option<DefaultValue>,
}

impl ArgSpec {
    pub const fn required(name: &'static str) -> Self {
        Self { name, required: true, default: None }
    }

    pub const fn optional(name: &'static str, default: DefaultValue) -> Self {
        Self { name, required: false, default: Some(default) }
    }
}
```

---

## Pattern Signature (For Template Caching)

```rust
/// Semantic identity of a pattern instantiation
///
/// Two patterns with the same signature share the same compiled template
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct PatternSignature {
    /// Pattern kind
    pub kind: Name,

    /// Input types (in canonical order)
    pub input_types: Vec<TypeId>,

    /// Output type
    pub output_type: TypeId,

    /// Transform function signature (if applicable)
    pub transform_sig: Option<FunctionSignature>,

    /// Additional type parameters
    pub type_params: Vec<TypeId>,
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct FunctionSignature {
    pub params: Vec<TypeId>,
    pub ret: TypeId,
}
```

---

## Template Compilation

```rust
/// Compiled pattern template
pub struct CompiledTemplate {
    /// Template C code with placeholders
    pub code: String,

    /// Placeholder positions for transform functions
    pub transform_slots: Vec<TransformSlot>,

    /// Placeholder positions for type-specific operations
    pub type_slots: Vec<TypeSlot>,
}

#[derive(Clone)]
pub struct TransformSlot {
    /// Placeholder name in template
    pub placeholder: String,
    /// Which argument provides the function
    pub arg_name: Name,
}

#[derive(Clone)]
pub struct TypeSlot {
    /// Placeholder name in template
    pub placeholder: String,
    /// What kind of type operation
    pub kind: TypeSlotKind,
}

#[derive(Copy, Clone)]
pub enum TypeSlotKind {
    ElementType,
    ResultType,
    AccumulatorType,
    KeyType,
    ValueType,
}

impl CompiledTemplate {
    /// Instantiate template with concrete values
    pub fn instantiate(&self, bindings: &TemplateBindings) -> CCode {
        let mut code = self.code.clone();

        // Replace transform slots
        for slot in &self.transform_slots {
            let func_code = bindings.get_transform(&slot.arg_name);
            code = code.replace(&slot.placeholder, func_code);
        }

        // Replace type slots
        for slot in &self.type_slots {
            let type_code = bindings.get_type(slot.kind);
            code = code.replace(&slot.placeholder, type_code);
        }

        CCode(code)
    }
}
```

---

## Template Cache

```rust
/// Global cache of compiled pattern templates
pub struct PatternTemplateCache {
    /// Signature → Compiled template
    cache: DashMap<PatternSignature, Arc<CompiledTemplate>>,
}

impl PatternTemplateCache {
    pub fn new() -> Self {
        Self {
            cache: DashMap::with_capacity(1024),
        }
    }

    /// Get cached template or compile new one
    pub fn get_or_compile(
        &self,
        sig: &PatternSignature,
        compile: impl FnOnce() -> CompiledTemplate,
    ) -> Arc<CompiledTemplate> {
        // Try cache first
        if let Some(template) = self.cache.get(sig) {
            return Arc::clone(&template);
        }

        // Compile and cache
        let template = Arc::new(compile());
        self.cache.insert(sig.clone(), Arc::clone(&template));
        template
    }

    /// Statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.cache.len(),
            memory_bytes: self.estimate_memory(),
        }
    }
}
```

---

## Pattern Fusion

### Fusion Detection

```rust
/// Detect fusible pattern chains in AST
pub fn detect_fusible_chains(
    module: &Module,
    arena: &ExprArena,
    registry: &PatternRegistry,
) -> Vec<PatternChain> {
    let mut chains = Vec::new();
    let mut visitor = ChainVisitor::new(&mut chains, registry);

    for item in module.items {
        visitor.visit_item(item, arena);
    }

    chains
}

/// Chain of patterns that can be fused
pub struct PatternChain {
    /// Patterns from innermost to outermost
    pub patterns: Vec<ChainLink>,
    /// Combined input expression
    pub input: ExprId,
    /// Span covering entire chain
    pub span: Span,
}

pub struct ChainLink {
    pub kind: Name,
    pub args: PatternArgs,
    pub expr_id: ExprId,
}
```

### Fusible Combinations

| Pattern 1 | Pattern 2 | Pattern 3 | Fused Form |
|-----------|-----------|-----------|------------|
| map | filter | - | MapFilter |
| map | fold | - | MapFold |
| filter | fold | - | FilterFold |
| map | filter | fold | MapFilterFold |
| filter | map | - | FilterMap |
| map | find | - | MapFind |
| filter | find | - | FilterFind |

### Fused Pattern

```rust
/// Fused pattern representation
pub enum FusedPattern {
    MapFilter {
        input: ExprId,
        map_fn: ExprId,
        filter_fn: ExprId,
    },
    MapFold {
        input: ExprId,
        map_fn: ExprId,
        init: ExprId,
        fold_fn: ExprId,
    },
    FilterFold {
        input: ExprId,
        filter_fn: ExprId,
        init: ExprId,
        fold_fn: ExprId,
    },
    MapFilterFold {
        input: ExprId,
        map_fn: ExprId,
        filter_fn: ExprId,
        init: ExprId,
        fold_fn: ExprId,
    },
    // ... other combinations
}

impl FusedPattern {
    /// Evaluate fused pattern in single pass
    pub fn evaluate(&self, env: &mut Environment, arena: &ExprArena) -> Result<Value> {
        match self {
            FusedPattern::MapFilterFold { input, map_fn, filter_fn, init, fold_fn } => {
                let list = env.eval(*input, arena)?.as_list()?;
                let map_f = env.eval(*map_fn, arena)?.as_function()?;
                let filter_f = env.eval(*filter_fn, arena)?.as_function()?;
                let fold_f = env.eval(*fold_fn, arena)?.as_function()?;
                let mut acc = env.eval(*init, arena)?;

                // Single pass!
                for elem in list.iter() {
                    let mapped = env.call(&map_f, &[elem.clone()])?;
                    if env.call(&filter_f, &[mapped.clone()])?.as_bool()? {
                        acc = env.call(&fold_f, &[acc, mapped])?;
                    }
                }

                Ok(acc)
            }
            // ... other cases
        }
    }
}
```

---

## Built-in Patterns

### Sequential Patterns

```rust
/// run pattern: sequential execution
///
/// Syntax: run(stmt1, stmt2, ..., result)
pub struct RunPattern;

impl PatternDefinition for RunPattern {
    fn name(&self) -> &'static str { "run" }
    fn required_args(&self) -> &'static [&'static str] { &[] }
    fn optional_args(&self) -> &'static [(&'static str, DefaultValue)] { &[] }

    // run has special variadic parsing
    fn parse(&self, parser: &mut PatternParser) -> Result<PatternNode, ParseError> {
        let mut stmts = Vec::new();
        while !parser.at(TokenKind::RParen) {
            stmts.push(parser.parse_statement()?);
            if !parser.eat(TokenKind::Comma) {
                break;
            }
        }
        Ok(PatternNode::Run { stmts })
    }
}

/// try pattern: error propagation
///
/// Syntax: try(fallible_expr?, ..., result)
pub struct TryPattern;
```

### Collection Patterns

```rust
/// map pattern: transform elements
///
/// Syntax: map(.over: collection, .transform: fn)
pub struct MapPattern;

impl PatternDefinition for MapPattern {
    fn name(&self) -> &'static str { "map" }

    fn required_args(&self) -> &'static [&'static str] {
        &["over", "transform"]
    }

    fn can_fuse_with(&self, next: &dyn PatternDefinition) -> bool {
        matches!(next.name(), "filter" | "fold" | "find")
    }
}

/// filter pattern: select elements
///
/// Syntax: filter(.over: collection, .predicate: fn)
pub struct FilterPattern;

/// fold pattern: reduce to single value
///
/// Syntax: fold(.over: collection, .init: initial, .op: fn)
pub struct FoldPattern;

/// find pattern: find first matching element
///
/// Syntax: find(.over: collection, .where: predicate)
pub struct FindPattern;

/// collect pattern: generate collection
///
/// Syntax: collect(.range: 0..10, .transform: fn)
pub struct CollectPattern;
```

### Recursion Pattern

```rust
/// recurse pattern: recursive computation with optional memoization
///
/// Syntax: recurse(.cond: base_case, .base: value, .step: recursive_call, .memo: true)
pub struct RecursePattern;

impl PatternDefinition for RecursePattern {
    fn name(&self) -> &'static str { "recurse" }

    fn required_args(&self) -> &'static [&'static str] {
        &["cond", "base", "step"]
    }

    fn optional_args(&self) -> &'static [(&'static str, DefaultValue)] {
        &[("memo", DefaultValue::Bool(false))]
    }
}
```

### Concurrency Patterns

```rust
/// parallel pattern: concurrent execution
///
/// Syntax: parallel(.task1: expr1, .task2: expr2, ...)
pub struct ParallelPattern;

/// timeout pattern: time-bounded execution
///
/// Syntax: timeout(.op: expr, .after: duration)
pub struct TimeoutPattern;

/// retry pattern: retry with backoff
///
/// Syntax: retry(.op: expr, .attempts: n, .backoff: strategy)
pub struct RetryPattern;
```

### Caching Pattern

```rust
/// cache pattern: memoized computation
///
/// Syntax: cache(.key: key_expr, .compute: value_expr)
pub struct CachePattern;
```

### Validation Pattern

```rust
/// validate pattern: data validation
///
/// Syntax: validate(.value: expr, .rules: [...])
pub struct ValidatePattern;
```

---

## Pattern Type Checking

```rust
impl MapPattern {
    pub fn type_check(
        &self,
        ctx: &mut TypeContext,
        args: &PatternArgs,
        arena: &ExprArena,
    ) -> Result<TypeId, TypeError> {
        let over = args.get(name!("over"));
        let transform = args.get(name!("transform"));

        // Type check .over
        let over_ty = ctx.infer(over, arena);

        // Extract element type
        let elem_ty = match ctx.interner.resolve(over_ty) {
            TypeKind::List(elem) => elem,
            _ => {
                return Err(TypeError::new(
                    ErrorCode::E2010,
                    arena.get(over).span,
                    "map(.over:) requires a list",
                ));
            }
        };

        // Type check .transform with expected type
        let expected_fn = ctx.interner.intern(TypeKind::Function {
            params: ctx.interner.intern_range([elem_ty]),
            ret: ctx.fresh_type_var(),
        });
        ctx.check(transform, expected_fn, arena);

        // Extract result element type
        let transform_ty = ctx.infer(transform, arena);
        let result_elem = match ctx.interner.resolve(transform_ty) {
            TypeKind::Function { ret, .. } => ret,
            _ => return Err(TypeError::function_expected(arena.get(transform).span)),
        };

        // Result is List<ResultElem>
        Ok(ctx.interner.intern(TypeKind::List(result_elem)))
    }
}
```
