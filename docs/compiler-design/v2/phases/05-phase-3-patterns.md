# Phase 3: Patterns & Evaluation (Weeks 9-12)

## Goal

Build the pattern system with template compilation and fusion:
- Pattern infrastructure with self-registration
- Template compilation for code reuse
- Pattern fusion for optimization
- Tree-walking interpreter

**Deliverable:** Can run programs via interpreter with optimized patterns.

---

## Week 9: Pattern Infrastructure

### Objective

Build extensible pattern system with self-registration and template support.

### Pattern Definition Trait

```rust
/// Core trait for pattern definitions
pub trait PatternDefinition: Send + Sync + 'static {
    /// Pattern name (e.g., "map", "filter", "fold")
    fn name(&self) -> &'static str;

    /// Required named arguments
    fn required_args(&self) -> &'static [&'static str];

    /// Optional named arguments with defaults
    fn optional_args(&self) -> &'static [(&'static str, DefaultValue)];

    /// Parse pattern from tokens
    fn parse(&self, parser: &mut PatternParser) -> Result<PatternNode>;

    /// Type check pattern
    fn type_check(
        &self,
        ctx: &mut TypeContext,
        args: &PatternArgs,
    ) -> Result<TypeId>;

    /// Evaluate pattern at runtime
    fn evaluate(
        &self,
        env: &mut Environment,
        args: &PatternArgs,
    ) -> Result<Value>;

    /// Generate C code for pattern
    fn codegen(
        &self,
        ctx: &mut CodegenContext,
        args: &PatternArgs,
    ) -> Result<CCode>;

    /// Pattern signature for template lookup
    fn signature(&self, args: &PatternArgs) -> PatternSignature;

    /// Can this pattern be fused with the next one?
    fn can_fuse_with(&self, next: &dyn PatternDefinition) -> bool {
        false  // Default: no fusion
    }
}

/// Default value for optional arguments
pub enum DefaultValue {
    None,
    Bool(bool),
    Int(i64),
    Lambda(&'static str),  // Source code for default lambda
}
```

### Pattern Registry

```rust
/// Global registry of pattern definitions
pub struct PatternRegistry {
    patterns: FxHashMap<Name, Arc<dyn PatternDefinition>>,
    keywords: FxHashSet<&'static str>,
}

impl PatternRegistry {
    /// Create registry with all built-in patterns
    pub fn new(interner: &StringInterner) -> Self {
        let mut registry = Self {
            patterns: FxHashMap::default(),
            keywords: FxHashSet::default(),
        };

        // Register all built-in patterns
        registry.register(interner, Box::new(RunPattern));
        registry.register(interner, Box::new(TryPattern));
        registry.register(interner, Box::new(MatchPattern));
        registry.register(interner, Box::new(MapPattern));
        registry.register(interner, Box::new(FilterPattern));
        registry.register(interner, Box::new(FoldPattern));
        registry.register(interner, Box::new(FindPattern));
        registry.register(interner, Box::new(CollectPattern));
        registry.register(interner, Box::new(RecursePattern));
        registry.register(interner, Box::new(ParallelPattern));
        registry.register(interner, Box::new(TimeoutPattern));
        registry.register(interner, Box::new(RetryPattern));
        registry.register(interner, Box::new(CachePattern));
        registry.register(interner, Box::new(ValidatePattern));

        registry
    }

    fn register(&mut self, interner: &StringInterner, pattern: Box<dyn PatternDefinition>) {
        let name = interner.intern(pattern.name());
        self.keywords.insert(pattern.name());
        self.patterns.insert(name, Arc::from(pattern));
    }

    /// Get pattern by name
    pub fn get(&self, name: Name) -> Option<&Arc<dyn PatternDefinition>> {
        self.patterns.get(&name)
    }

    /// Is this name a pattern keyword?
    pub fn is_pattern_keyword(&self, s: &str) -> bool {
        self.keywords.contains(s)
    }
}
```

### Pattern Signature

```rust
/// Semantic identity of a pattern instantiation
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct PatternSignature {
    pub kind: Name,
    pub input_types: Vec<TypeId>,
    pub output_type: TypeId,
    pub transform_sig: Option<FunctionSignature>,
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct FunctionSignature {
    pub params: Vec<TypeId>,
    pub ret: TypeId,
}

impl PatternSignature {
    /// Create signature for map pattern
    pub fn for_map(
        input_elem: TypeId,
        output_elem: TypeId,
        interner: &TypeInterner,
    ) -> Self {
        Self {
            kind: name!("map"),
            input_types: vec![interner.intern(TypeKind::List(input_elem))],
            output_type: interner.intern(TypeKind::List(output_elem)),
            transform_sig: Some(FunctionSignature {
                params: vec![input_elem],
                ret: output_elem,
            }),
        }
    }
}
```

---

## Week 10: Core Patterns

### Example: Map Pattern

```rust
pub struct MapPattern;

impl PatternDefinition for MapPattern {
    fn name(&self) -> &'static str { "map" }

    fn required_args(&self) -> &'static [&'static str] {
        &["over", "transform"]
    }

    fn optional_args(&self) -> &'static [(&'static str, DefaultValue)] {
        &[]
    }

    fn parse(&self, parser: &mut PatternParser) -> Result<PatternNode> {
        let args = parser.parse_named_args(&[
            ArgSpec::required("over"),
            ArgSpec::required("transform"),
        ])?;

        Ok(PatternNode::Map {
            over: args.get("over").unwrap(),
            transform: args.get("transform").unwrap(),
        })
    }

    fn type_check(
        &self,
        ctx: &mut TypeContext,
        args: &PatternArgs,
    ) -> Result<TypeId> {
        let over_ty = ctx.infer(args.over);
        let transform_ty = ctx.infer(args.transform);

        // over must be [T]
        let elem_ty = match ctx.interner.resolve(over_ty) {
            TypeKind::List(elem) => elem,
            _ => {
                ctx.error(ErrorCode::E2010, args.over_span,
                    "map(.over:) requires a list");
                return Err(());
            }
        };

        // transform must be T -> U
        let result_ty = match ctx.interner.resolve(transform_ty) {
            TypeKind::Function { params, ret } => {
                let params = ctx.interner.resolve_range(params);
                if params.len() != 1 {
                    ctx.error(ErrorCode::E2011, args.transform_span,
                        "transform function must take exactly one argument");
                    return Err(());
                }
                ctx.unify(params[0], elem_ty, args.transform_span);
                ret
            }
            _ => {
                ctx.error(ErrorCode::E2012, args.transform_span,
                    "map(.transform:) requires a function");
                return Err(());
            }
        };

        Ok(ctx.interner.intern(TypeKind::List(result_ty)))
    }

    fn evaluate(
        &self,
        env: &mut Environment,
        args: &PatternArgs,
    ) -> Result<Value> {
        let over = env.eval(args.over)?;
        let transform = env.eval(args.transform)?;

        let list = over.as_list()?;
        let func = transform.as_function()?;

        let results: Vec<Value> = list.iter()
            .map(|elem| env.call(&func, &[elem.clone()]))
            .collect::<Result<_>>()?;

        Ok(Value::List(results))
    }

    fn codegen(
        &self,
        ctx: &mut CodegenContext,
        args: &PatternArgs,
    ) -> Result<CCode> {
        let over_code = ctx.codegen(args.over)?;
        let transform_code = ctx.codegen(args.transform)?;

        // Check for compiled template
        let sig = self.signature(args);
        if let Some(template) = ctx.get_template(&sig) {
            return template.instantiate(&over_code, &transform_code);
        }

        // Generate fresh code
        ctx.emit(format!(r#"
            sigil_list_t* __result = sigil_list_new();
            for (size_t __i = 0; __i < {over}.len; __i++) {{
                {elem_type} __elem = {over}.data[__i];
                {result_type} __mapped = {transform}(__elem);
                sigil_list_push(__result, __mapped);
            }}
        "#,
            over = over_code,
            transform = transform_code,
            elem_type = ctx.c_type(args.elem_type),
            result_type = ctx.c_type(args.result_type),
        ))
    }

    fn signature(&self, args: &PatternArgs) -> PatternSignature {
        PatternSignature::for_map(args.elem_type, args.result_type, args.interner)
    }

    fn can_fuse_with(&self, next: &dyn PatternDefinition) -> bool {
        matches!(next.name(), "filter" | "fold" | "find")
    }
}
```

### Example: Fold Pattern

```rust
pub struct FoldPattern;

impl PatternDefinition for FoldPattern {
    fn name(&self) -> &'static str { "fold" }

    fn required_args(&self) -> &'static [&'static str] {
        &["over", "init", "op"]
    }

    fn optional_args(&self) -> &'static [(&'static str, DefaultValue)] {
        &[]
    }

    fn type_check(
        &self,
        ctx: &mut TypeContext,
        args: &PatternArgs,
    ) -> Result<TypeId> {
        let over_ty = ctx.infer(args.over);
        let init_ty = ctx.infer(args.init);
        let op_ty = ctx.infer(args.op);

        // over must be [T]
        let elem_ty = ctx.expect_list_type(over_ty, args.over_span)?;

        // op must be (Acc, T) -> Acc
        match ctx.interner.resolve(op_ty) {
            TypeKind::Function { params, ret } => {
                let params = ctx.interner.resolve_range(params);
                if params.len() != 2 {
                    ctx.error(ErrorCode::E2020, args.op_span,
                        "fold(.op:) must take two arguments (accumulator, element)");
                    return Err(());
                }
                ctx.unify(params[0], init_ty, args.op_span);  // Acc
                ctx.unify(params[1], elem_ty, args.op_span);  // T
                ctx.unify(ret, init_ty, args.op_span);        // -> Acc
            }
            _ => {
                ctx.error(ErrorCode::E2021, args.op_span,
                    "fold(.op:) requires a function");
                return Err(());
            }
        }

        Ok(init_ty)  // Result type is accumulator type
    }

    fn evaluate(
        &self,
        env: &mut Environment,
        args: &PatternArgs,
    ) -> Result<Value> {
        let over = env.eval(args.over)?;
        let mut acc = env.eval(args.init)?;
        let op = env.eval(args.op)?;

        let list = over.as_list()?;
        let func = op.as_function()?;

        for elem in list {
            acc = env.call(&func, &[acc, elem.clone()])?;
        }

        Ok(acc)
    }

    fn can_fuse_with(&self, _next: &dyn PatternDefinition) -> bool {
        false  // fold is terminal
    }
}
```

---

## Week 10 (continued): Template Compilation

### Template Cache

```rust
/// Global cache of compiled pattern templates
pub struct PatternTemplateCache {
    templates: DashMap<PatternSignature, CompiledTemplate>,
}

impl PatternTemplateCache {
    pub fn new() -> Self {
        Self {
            templates: DashMap::with_capacity(1024),
        }
    }

    /// Get or compile template for signature
    pub fn get_or_compile(
        &self,
        sig: &PatternSignature,
        compile: impl FnOnce() -> CompiledTemplate,
    ) -> Arc<CompiledTemplate> {
        if let Some(template) = self.templates.get(sig) {
            return Arc::clone(&*template);
        }

        let template = Arc::new(compile());
        self.templates.insert(sig.clone(), Arc::clone(&template));
        template
    }
}

/// Compiled template with holes for specialization
pub struct CompiledTemplate {
    /// Template code with placeholders
    pub code: String,
    /// Positions of transform function slots
    pub transform_slots: Vec<usize>,
    /// Type-specific operation slots
    pub type_slots: Vec<(usize, TypeSlotKind)>,
}

#[derive(Clone)]
pub enum TypeSlotKind {
    ElementType,
    ResultType,
    AccumulatorType,
}

impl CompiledTemplate {
    /// Instantiate template with concrete values
    pub fn instantiate(
        &self,
        over: &str,
        transform: &str,
    ) -> CCode {
        let mut code = self.code.clone();

        // Replace placeholders
        code = code.replace("__OVER__", over);
        code = code.replace("__TRANSFORM__", transform);

        CCode(code)
    }
}
```

### Template Generation

```rust
impl MapPattern {
    fn compile_template(&self, interner: &TypeInterner) -> CompiledTemplate {
        CompiledTemplate {
            code: r#"
                sigil_list_t* __result = sigil_list_new();
                for (size_t __i = 0; __i < __OVER__.len; __i++) {
                    __ELEM_TYPE__ __elem = __OVER__.data[__i];
                    __RESULT_TYPE__ __mapped = __TRANSFORM__(__elem);
                    sigil_list_push(__result, __mapped);
                }
            "#.to_string(),
            transform_slots: vec![],
            type_slots: vec![
                (0, TypeSlotKind::ElementType),
                (1, TypeSlotKind::ResultType),
            ],
        }
    }
}
```

---

## Week 11: Pattern Fusion

### Fusion Detection

```rust
/// Detect fusible pattern chains in AST
pub fn detect_pattern_chains(
    expr: ExprId,
    arena: &ExprArena,
) -> Vec<PatternChain> {
    let mut chains = Vec::new();
    let mut visitor = ChainDetector::new(&mut chains);
    visitor.visit(expr, arena);
    chains
}

struct ChainDetector<'a> {
    chains: &'a mut Vec<PatternChain>,
}

impl ChainDetector<'_> {
    fn visit(&mut self, expr: ExprId, arena: &ExprArena) {
        let node = arena.get(expr);

        if let ExprKind::Pattern { kind, args } = &node.kind {
            // Try to build chain starting here
            if let Some(chain) = self.build_chain(expr, arena) {
                if chain.len() >= 2 {
                    self.chains.push(chain);
                    return;  // Don't recurse into fused chain
                }
            }
        }

        // Recurse into children
        self.visit_children(expr, arena);
    }

    fn build_chain(&self, expr: ExprId, arena: &ExprArena) -> Option<PatternChain> {
        let mut chain = Vec::new();
        let mut current = expr;

        loop {
            let node = arena.get(current);

            match &node.kind {
                ExprKind::Pattern { kind, args } => {
                    chain.push(PatternLink {
                        kind: *kind,
                        args: *args,
                        expr: current,
                    });

                    // Check if input is another pattern
                    let args_node = arena.get_pattern_args(*args);
                    if let Some(input) = args_node.get("over") {
                        let input_node = arena.get(input);
                        if let ExprKind::Pattern { .. } = &input_node.kind {
                            // Check if patterns can fuse
                            let current_def = self.registry.get(*kind)?;
                            let input_def = self.get_pattern_def(input, arena)?;

                            if current_def.can_fuse_with(&*input_def) {
                                current = input;
                                continue;
                            }
                        }
                    }
                }
                _ => {}
            }
            break;
        }

        if chain.len() >= 2 {
            chain.reverse();  // Order from input to output
            Some(PatternChain(chain))
        } else {
            None
        }
    }
}
```

### Fusion Rules

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
}

/// Attempt to fuse a pattern chain
pub fn fuse_chain(chain: &PatternChain) -> Option<FusedPattern> {
    let kinds: Vec<_> = chain.0.iter().map(|l| l.kind).collect();

    match kinds.as_slice() {
        // map -> filter
        [map, filter] if is_map(map) && is_filter(filter) => {
            Some(FusedPattern::MapFilter {
                input: get_input(&chain.0[0]),
                map_fn: get_transform(&chain.0[0]),
                filter_fn: get_predicate(&chain.0[1]),
            })
        }

        // map -> fold
        [map, fold] if is_map(map) && is_fold(fold) => {
            Some(FusedPattern::MapFold {
                input: get_input(&chain.0[0]),
                map_fn: get_transform(&chain.0[0]),
                init: get_init(&chain.0[1]),
                fold_fn: get_op(&chain.0[1]),
            })
        }

        // filter -> fold
        [filter, fold] if is_filter(filter) && is_fold(fold) => {
            Some(FusedPattern::FilterFold {
                input: get_input(&chain.0[0]),
                filter_fn: get_predicate(&chain.0[0]),
                init: get_init(&chain.0[1]),
                fold_fn: get_op(&chain.0[1]),
            })
        }

        // map -> filter -> fold
        [map, filter, fold]
            if is_map(map) && is_filter(filter) && is_fold(fold) =>
        {
            Some(FusedPattern::MapFilterFold {
                input: get_input(&chain.0[0]),
                map_fn: get_transform(&chain.0[0]),
                filter_fn: get_predicate(&chain.0[1]),
                init: get_init(&chain.0[2]),
                fold_fn: get_op(&chain.0[2]),
            })
        }

        _ => None,
    }
}
```

### Fused Evaluation

```rust
impl FusedPattern {
    pub fn evaluate(&self, env: &mut Environment) -> Result<Value> {
        match self {
            FusedPattern::MapFilterFold {
                input, map_fn, filter_fn, init, fold_fn
            } => {
                let list = env.eval(*input)?.as_list()?;
                let map_f = env.eval(*map_fn)?.as_function()?;
                let filter_f = env.eval(*filter_fn)?.as_function()?;
                let fold_f = env.eval(*fold_fn)?.as_function()?;
                let mut acc = env.eval(*init)?;

                // Single pass over data!
                for elem in list {
                    let mapped = env.call(&map_f, &[elem])?;
                    let keep = env.call(&filter_f, &[mapped.clone()])?;
                    if keep.as_bool()? {
                        acc = env.call(&fold_f, &[acc, mapped])?;
                    }
                }

                Ok(acc)
            }

            // ... other fused patterns
            _ => unimplemented!()
        }
    }
}
```

---

## Weeks 11-12: Interpreter

### Value Representation

```rust
/// Runtime value
pub enum Value {
    // Primitives
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Arc<str>),
    Char(char),
    Byte(u8),
    Void,

    // Collections
    List(Arc<Vec<Value>>),
    Map(Arc<FxHashMap<Value, Value>>),

    // Struct with indexed fields (not HashMap!)
    Struct {
        layout: StructLayoutId,
        fields: Arc<Vec<Value>>,
    },

    // Function values
    Function(Arc<FunctionValue>),
    Closure {
        func: Arc<FunctionValue>,
        captures: Arc<Vec<Value>>,
    },

    // Special
    Option(Option<Box<Value>>),
    Result(Result<Box<Value>, Box<Value>>),
}

/// Struct layout (computed at type check time)
pub struct StructLayout {
    pub name: Name,
    pub field_names: Vec<Name>,
    pub field_indices: FxHashMap<Name, u32>,
}

impl Value {
    /// O(1) field access via index
    pub fn get_field(&self, field: Name, layouts: &StructLayouts) -> &Value {
        match self {
            Value::Struct { layout, fields } => {
                let layout = layouts.get(*layout);
                let idx = layout.field_indices[&field] as usize;
                &fields[idx]
            }
            _ => panic!("not a struct"),
        }
    }
}
```

### Environment Without Cloning

```rust
/// Evaluation environment with persistent bindings
pub struct Environment<'db> {
    db: &'db dyn Db,
    /// Persistent global bindings
    globals: Arc<FxHashMap<Name, Value>>,
    /// Local scope stack (shallow)
    locals: Vec<FxHashMap<Name, Value>>,
    /// Function being evaluated (for recursion)
    current_function: Option<FunctionId>,
}

impl<'db> Environment<'db> {
    pub fn new(db: &'db dyn Db) -> Self {
        Self {
            db,
            globals: Arc::new(FxHashMap::default()),
            locals: vec![FxHashMap::default()],
            current_function: None,
        }
    }

    /// Enter local scope
    pub fn push_scope(&mut self) {
        self.locals.push(FxHashMap::default());
    }

    /// Exit local scope
    pub fn pop_scope(&mut self) {
        self.locals.pop();
    }

    /// Define local binding
    pub fn define(&mut self, name: Name, value: Value) {
        self.locals.last_mut().unwrap().insert(name, value);
    }

    /// Lookup binding (locals then globals)
    pub fn get(&self, name: Name) -> Option<&Value> {
        // Search locals from innermost to outermost
        for scope in self.locals.iter().rev() {
            if let Some(v) = scope.get(&name) {
                return Some(v);
            }
        }
        // Fall back to globals
        self.globals.get(&name)
    }

    /// Call function without cloning environment
    pub fn call(&mut self, func: &FunctionValue, args: &[Value]) -> Result<Value> {
        // Push new scope for function body
        self.push_scope();

        // Bind parameters
        for (param, arg) in func.params.iter().zip(args) {
            self.define(*param, arg.clone());
        }

        // Evaluate body
        let result = self.eval(func.body);

        // Pop scope (discards locals)
        self.pop_scope();

        result
    }
}
```

### Expression Evaluation

```rust
impl Environment<'_> {
    pub fn eval(&mut self, expr: ExprId) -> Result<Value> {
        let arena = self.current_arena();
        let node = arena.get(expr);

        match &node.kind {
            ExprKind::Int(n) => Ok(Value::Int(*n)),
            ExprKind::Float(n) => Ok(Value::Float(*n)),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::String(s) => Ok(Value::Str(self.db.interner().resolve(*s).into())),

            ExprKind::Ident(name) => {
                self.get(*name)
                    .cloned()
                    .ok_or_else(|| runtime_error!("undefined: {}", name))
            }

            ExprKind::Binary { op, left, right } => {
                let l = self.eval(*left)?;
                let r = self.eval(*right)?;
                self.eval_binary(*op, l, r)
            }

            ExprKind::Call { func, args } => {
                let func_val = self.eval(*func)?;
                let args: Vec<_> = arena.get_list(*args)
                    .iter()
                    .map(|e| self.eval(*e))
                    .collect::<Result<_>>()?;

                match func_val {
                    Value::Function(f) => self.call(&f, &args),
                    Value::Closure { func, captures } => {
                        self.call_closure(&func, &captures, &args)
                    }
                    _ => Err(runtime_error!("not callable")),
                }
            }

            ExprKind::If { cond, then_branch, else_branch } => {
                let cond_val = self.eval(*cond)?;
                if cond_val.as_bool()? {
                    self.eval(*then_branch)
                } else if let Some(else_br) = else_branch {
                    self.eval(*else_br)
                } else {
                    Ok(Value::Void)
                }
            }

            ExprKind::Pattern { kind, args } => {
                self.eval_pattern(*kind, *args, arena)
            }

            ExprKind::Let { name, init, .. } => {
                let value = self.eval(*init)?;
                self.define(*name, value);
                Ok(Value::Void)
            }

            // ... other cases
            _ => Err(runtime_error!("not implemented: {:?}", node.kind))
        }
    }

    fn eval_pattern(
        &mut self,
        kind: PatternKind,
        args: PatternArgsId,
        arena: &ExprArena,
    ) -> Result<Value> {
        // Check for fused pattern first
        if let Some(fused) = self.get_fused_pattern(kind, args) {
            return fused.evaluate(self);
        }

        // Regular pattern evaluation
        let pattern_def = self.db.pattern_registry().get(kind)
            .ok_or_else(|| runtime_error!("unknown pattern"))?;

        let args_node = arena.get_pattern_args(args);
        pattern_def.evaluate(self, &args_node)
    }
}
```

---

## Phase 3 Deliverables Checklist

### Week 9: Pattern Infrastructure
- [ ] `PatternDefinition` trait with all methods
- [ ] `PatternRegistry` with self-registration
- [ ] `PatternSignature` for template lookup
- [ ] Context-sensitive keyword handling in parser

### Week 10: Core Patterns
- [ ] `run` pattern implementation
- [ ] `try` pattern implementation
- [ ] `match` pattern implementation
- [ ] `map` pattern with template
- [ ] `filter` pattern with template
- [ ] `fold` pattern implementation
- [ ] `find` pattern implementation
- [ ] `collect` pattern implementation
- [ ] `recurse` pattern with memoization
- [ ] `parallel` pattern implementation
- [ ] `timeout` pattern implementation
- [ ] `retry` pattern implementation
- [ ] `cache` pattern implementation
- [ ] `validate` pattern implementation

### Week 11: Pattern Fusion
- [ ] Pattern chain detection
- [ ] `MapFilter` fusion
- [ ] `MapFold` fusion
- [ ] `FilterFold` fusion
- [ ] `MapFilterFold` fusion
- [ ] Fused codegen templates

### Weeks 11-12: Interpreter
- [ ] `Value` enum with all variants
- [ ] `StructLayout` with indexed fields
- [ ] `Environment` without cloning
- [ ] All expression evaluation
- [ ] Pattern evaluation with fusion

### Tests
- [ ] Pattern parsing tests
- [ ] Pattern type checking tests
- [ ] Pattern evaluation tests
- [ ] Fusion correctness tests
- [ ] Fusion benchmark (single vs multi pass)

---

## Next Phase

With patterns and interpreter complete, proceed to [Phase 4: Parallelism](06-phase-4-parallelism.md) to build the parallel compilation pipeline.
