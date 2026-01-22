# Phase 2: Type System (Weeks 5-8)

## Goal

Build the complete type checking infrastructure with interned types:
- Type interner for O(1) type comparison
- Name resolution with scope tracking
- Bidirectional type inference
- Full error recovery

**Deliverable:** Can type check programs and report multiple errors.

---

## Week 5: Type Interner

### Objective

Replace cloned types with `TypeId(u32)` indices into a global type interner.

### Data Structures

```rust
/// Interned type identifier - Copy, cheap to compare
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct TypeId(u32);

/// Range of types (for function params, tuple elements, etc.)
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct TypeRange {
    start: u32,
    len: u16,
}

/// Type representation
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum TypeKind {
    // Primitives (singletons, pre-interned)
    Int,
    Float,
    Bool,
    Str,
    Char,
    Byte,
    Void,
    Never,

    // Compound types
    List(TypeId),
    Map(TypeId, TypeId),
    Set(TypeId),
    Tuple(TypeRange),
    Function { params: TypeRange, ret: TypeId },

    // User-defined types
    Named(Name),
    Generic { base: Name, args: TypeRange },

    // Special types
    Option(TypeId),
    Result(TypeId, TypeId),

    // Type variables (for inference)
    Var(TypeVarId),
    Infer,  // Placeholder during inference
}

/// Type variable for inference
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct TypeVarId(u32);
```

### Type Interner Implementation

```rust
/// Thread-safe type interner
pub struct TypeInterner {
    /// Type → TypeId lookup
    map: DashMap<TypeKind, TypeId>,
    /// TypeId → TypeKind lookup
    types: RwLock<Vec<TypeKind>>,
    /// Type ranges storage
    ranges: RwLock<Vec<TypeId>>,
}

impl TypeInterner {
    pub fn new() -> Self {
        let interner = Self {
            map: DashMap::with_capacity(4096),
            types: RwLock::new(Vec::with_capacity(4096)),
            ranges: RwLock::new(Vec::with_capacity(1024)),
        };

        // Pre-intern primitive types at known indices
        interner.intern_primitive(TypeKind::Int);    // TypeId(0)
        interner.intern_primitive(TypeKind::Float);  // TypeId(1)
        interner.intern_primitive(TypeKind::Bool);   // TypeId(2)
        interner.intern_primitive(TypeKind::Str);    // TypeId(3)
        interner.intern_primitive(TypeKind::Char);   // TypeId(4)
        interner.intern_primitive(TypeKind::Byte);   // TypeId(5)
        interner.intern_primitive(TypeKind::Void);   // TypeId(6)
        interner.intern_primitive(TypeKind::Never);  // TypeId(7)

        interner
    }

    /// Intern a type, returning its unique ID
    pub fn intern(&self, kind: TypeKind) -> TypeId {
        // Fast path: already exists
        if let Some(id) = self.map.get(&kind) {
            return *id;
        }

        // Slow path: insert new
        let mut types = self.types.write();
        let id = TypeId(types.len() as u32);
        types.push(kind.clone());
        drop(types);

        self.map.insert(kind, id);
        id
    }

    /// Intern a range of types
    pub fn intern_range(&self, types: impl IntoIterator<Item = TypeId>) -> TypeRange {
        let mut ranges = self.ranges.write();
        let start = ranges.len() as u32;
        ranges.extend(types);
        let len = (ranges.len() as u32 - start) as u16;
        TypeRange { start, len }
    }

    /// Resolve TypeId to TypeKind
    pub fn resolve(&self, id: TypeId) -> TypeKind {
        self.types.read()[id.0 as usize].clone()
    }

    /// Resolve type range
    pub fn resolve_range(&self, range: TypeRange) -> Vec<TypeId> {
        let ranges = self.ranges.read();
        let start = range.start as usize;
        let end = start + range.len as usize;
        ranges[start..end].to_vec()
    }
}

// Singleton accessors
impl TypeInterner {
    pub fn int(&self) -> TypeId { TypeId(0) }
    pub fn float(&self) -> TypeId { TypeId(1) }
    pub fn bool(&self) -> TypeId { TypeId(2) }
    pub fn str(&self) -> TypeId { TypeId(3) }
    pub fn char(&self) -> TypeId { TypeId(4) }
    pub fn byte(&self) -> TypeId { TypeId(5) }
    pub fn void(&self) -> TypeId { TypeId(6) }
    pub fn never(&self) -> TypeId { TypeId(7) }
}
```

### Type Comparison

```rust
// Type comparison is now O(1)!
impl PartialEq for TypeId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0  // Single integer comparison
    }
}

// Before (V1): O(n) recursive comparison
fn types_equal_v1(a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::List(a), Type::List(b)) => types_equal_v1(a, b),
        (Type::Function { params: p1, ret: r1 },
         Type::Function { params: p2, ret: r2 }) => {
            p1.len() == p2.len() &&
            p1.iter().zip(p2).all(|(a, b)| types_equal_v1(a, b)) &&
            types_equal_v1(r1, r2)
        }
        // ... many more cases
    }
}

// After (V2): O(1)
fn types_equal_v2(a: TypeId, b: TypeId) -> bool {
    a == b  // Single integer comparison
}
```

---

## Week 6: Name Resolution

### Objective

Build scope-aware name resolution with import handling.

### Scope Structure

```rust
/// Scope identifier
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct ScopeId(u32);

/// Binding in a scope
#[derive(Clone)]
pub struct Binding {
    pub name: Name,
    pub kind: BindingKind,
    pub ty: TypeId,
    pub span: Span,
    pub mutable: bool,
}

#[derive(Clone)]
pub enum BindingKind {
    Variable,
    Function,
    Config,
    Type,
    Pattern,
    Import { from: ModuleId, original: Name },
}

/// Scope with parent link
pub struct Scope {
    pub id: ScopeId,
    pub parent: Option<ScopeId>,
    pub kind: ScopeKind,
    pub bindings: FxHashMap<Name, Binding>,
}

#[derive(Copy, Clone)]
pub enum ScopeKind {
    Module,
    Function,
    Block,
    Loop,
    Match,
    Pattern,
}

/// Scope manager
pub struct Scopes {
    scopes: Vec<Scope>,
    current: ScopeId,
}
```

### Resolution Implementation

```rust
impl Scopes {
    pub fn new() -> Self {
        let root = Scope {
            id: ScopeId(0),
            parent: None,
            kind: ScopeKind::Module,
            bindings: FxHashMap::default(),
        };

        Self {
            scopes: vec![root],
            current: ScopeId(0),
        }
    }

    /// Enter a new scope
    pub fn enter(&mut self, kind: ScopeKind) -> ScopeId {
        let id = ScopeId(self.scopes.len() as u32);
        self.scopes.push(Scope {
            id,
            parent: Some(self.current),
            kind,
            bindings: FxHashMap::default(),
        });
        self.current = id;
        id
    }

    /// Exit current scope
    pub fn exit(&mut self) {
        let parent = self.scopes[self.current.0 as usize].parent
            .expect("cannot exit root scope");
        self.current = parent;
    }

    /// Define a binding in current scope
    pub fn define(&mut self, binding: Binding) -> Result<(), Diagnostic> {
        let scope = &mut self.scopes[self.current.0 as usize];

        if let Some(existing) = scope.bindings.get(&binding.name) {
            return Err(Diagnostic::error(ErrorCode::E3003)
                .with_message(format!(
                    "duplicate definition of `{}`",
                    binding.name
                ))
                .with_label(binding.span, "redefined here")
                .with_label(existing.span, "first defined here"));
        }

        scope.bindings.insert(binding.name, binding);
        Ok(())
    }

    /// Resolve a name, searching up scope chain
    pub fn resolve(&self, name: Name) -> Option<&Binding> {
        let mut scope_id = Some(self.current);

        while let Some(id) = scope_id {
            let scope = &self.scopes[id.0 as usize];
            if let Some(binding) = scope.bindings.get(&name) {
                return Some(binding);
            }
            scope_id = scope.parent;
        }

        None
    }

    /// Check if inside a loop (for break/continue validation)
    pub fn in_loop(&self) -> bool {
        let mut scope_id = Some(self.current);

        while let Some(id) = scope_id {
            let scope = &self.scopes[id.0 as usize];
            if matches!(scope.kind, ScopeKind::Loop) {
                return true;
            }
            if matches!(scope.kind, ScopeKind::Function) {
                return false;  // Don't cross function boundary
            }
            scope_id = scope.parent;
        }

        false
    }
}
```

### Import Resolution

```rust
/// Salsa query for module resolution
#[salsa::tracked]
pub fn resolved_imports(db: &dyn Db, module: Module) -> ResolvedImports {
    let mut imports = Vec::new();
    let mut errors = Vec::new();

    for import in module.imports(db) {
        match resolve_import(db, module, import) {
            Ok(resolved) => imports.push(resolved),
            Err(e) => errors.push(e),
        }
    }

    ResolvedImports { imports, errors }
}

fn resolve_import(
    db: &dyn Db,
    from: Module,
    import: &Import,
) -> Result<ResolvedImport, Diagnostic> {
    // Resolve module path
    let target_module = match &import.path {
        ImportPath::Relative(path) => {
            resolve_relative_path(db, from, path)?
        }
        ImportPath::Module(name) => {
            resolve_module_name(db, name)?
        }
    };

    // Resolve imported items
    let items: Vec<_> = import.items.iter()
        .map(|item| resolve_import_item(db, target_module, item))
        .collect::<Result<_, _>>()?;

    Ok(ResolvedImport {
        module: target_module,
        items,
        visibility: import.visibility,
    })
}
```

---

## Weeks 7-8: Type Checker

### Objective

Implement bidirectional type inference with full error recovery.

### Bidirectional Inference

```rust
/// Type checking context
pub struct TypeContext<'db> {
    db: &'db dyn Db,
    scopes: Scopes,
    interner: &'db TypeInterner,
    constraints: Vec<TypeConstraint>,
    diagnostics: Vec<Diagnostic>,
}

/// Type constraint for inference
pub struct TypeConstraint {
    pub expected: TypeId,
    pub found: TypeId,
    pub span: Span,
    pub context: &'static str,
}

impl<'db> TypeContext<'db> {
    /// Infer type of expression (synthesis mode)
    pub fn infer(&mut self, expr: ExprId, arena: &ExprArena) -> TypeId {
        let expr_node = arena.get(expr);

        match &expr_node.kind {
            ExprKind::Int(_) => self.interner.int(),
            ExprKind::Float(_) => self.interner.float(),
            ExprKind::Bool(_) => self.interner.bool(),
            ExprKind::String(_) => self.interner.str(),
            ExprKind::Char(_) => self.interner.char(),

            ExprKind::Ident(name) => {
                match self.scopes.resolve(*name) {
                    Some(binding) => binding.ty,
                    None => {
                        self.error_undefined(*name, expr_node.span);
                        self.interner.intern(TypeKind::Infer)
                    }
                }
            }

            ExprKind::Binary { op, left, right } => {
                self.infer_binary(*op, *left, *right, arena)
            }

            ExprKind::Call { func, args } => {
                self.infer_call(*func, *args, arena)
            }

            ExprKind::If { cond, then_branch, else_branch } => {
                self.infer_if(*cond, *then_branch, *else_branch, arena)
            }

            ExprKind::Lambda { params, body } => {
                self.infer_lambda(params, *body, arena)
            }

            ExprKind::Pattern { kind, args } => {
                self.infer_pattern(*kind, *args, arena)
            }

            // ... other cases
            _ => {
                self.error_not_implemented(expr_node.span);
                self.interner.intern(TypeKind::Infer)
            }
        }
    }

    /// Check expression against expected type (checking mode)
    pub fn check(&mut self, expr: ExprId, expected: TypeId, arena: &ExprArena) {
        let expr_node = arena.get(expr);

        // Special cases that benefit from expected type
        match &expr_node.kind {
            ExprKind::Lambda { params, body } if self.is_function_type(expected) => {
                self.check_lambda_against(params, *body, expected, arena);
                return;
            }

            ExprKind::If { cond, then_branch, else_branch: Some(else_branch) } => {
                self.check(*cond, self.interner.bool(), arena);
                self.check(*then_branch, expected, arena);
                self.check(*else_branch, expected, arena);
                return;
            }

            _ => {}
        }

        // Default: infer and compare
        let inferred = self.infer(expr, arena);
        self.unify(expected, inferred, expr_node.span);
    }

    /// Unify two types, adding constraint if needed
    fn unify(&mut self, expected: TypeId, found: TypeId, span: Span) {
        if expected == found {
            return;  // Fast path: same TypeId
        }

        let expected_kind = self.interner.resolve(expected);
        let found_kind = self.interner.resolve(found);

        match (&expected_kind, &found_kind) {
            // Type variable unification
            (TypeKind::Var(v), _) => {
                self.bind_var(*v, found);
            }
            (_, TypeKind::Var(v)) => {
                self.bind_var(*v, expected);
            }

            // Structural unification
            (TypeKind::List(e), TypeKind::List(f)) => {
                self.unify(*e, *f, span);
            }
            (TypeKind::Option(e), TypeKind::Option(f)) => {
                self.unify(*e, *f, span);
            }
            (TypeKind::Function { params: p1, ret: r1 },
             TypeKind::Function { params: p2, ret: r2 }) => {
                self.unify_ranges(*p1, *p2, span);
                self.unify(*r1, *r2, span);
            }

            // Inference placeholder accepts anything
            (TypeKind::Infer, _) | (_, TypeKind::Infer) => {}

            // Type mismatch
            _ => {
                self.diagnostics.push(type_mismatch(
                    span,
                    expected,
                    found,
                    "expression",
                ));
            }
        }
    }
}
```

### Function Type Checking

```rust
impl<'db> TypeContext<'db> {
    /// Type check a function definition
    pub fn check_function(&mut self, func: &Function, arena: &ExprArena) -> TypeId {
        // Enter function scope
        self.scopes.enter(ScopeKind::Function);

        // Bind parameters
        for (name, ty) in &func.params {
            self.scopes.define(Binding {
                name: *name,
                kind: BindingKind::Variable,
                ty: *ty,
                span: func.span,
                mutable: false,
            }).ok();  // Ignore duplicate param errors (handled in parsing)
        }

        // Check body against declared return type
        self.check(func.body, func.return_type, arena);

        // Exit scope
        self.scopes.exit();

        // Return function type
        let params = self.interner.intern_range(
            func.params.iter().map(|(_, ty)| *ty)
        );
        self.interner.intern(TypeKind::Function {
            params,
            ret: func.return_type,
        })
    }
}
```

### Pattern Type Checking

```rust
impl<'db> TypeContext<'db> {
    fn infer_pattern(
        &mut self,
        kind: PatternKind,
        args: PatternArgsId,
        arena: &ExprArena,
    ) -> TypeId {
        let pattern_def = self.get_pattern_definition(kind);
        let args_node = arena.get_pattern_args(args);

        // Validate required arguments
        for required in pattern_def.required_args() {
            if !args_node.has(required) {
                self.error_missing_pattern_arg(kind, required, args_node.span);
            }
        }

        // Type check arguments
        let mut typed_args = FxHashMap::default();
        for (name, expr) in args_node.iter() {
            let expected = pattern_def.arg_type(name);
            self.check(expr, expected, arena);
            typed_args.insert(name, expected);
        }

        // Compute result type
        pattern_def.result_type(&typed_args, self.interner)
    }
}

/// Pattern definition trait for type checking
pub trait PatternTypeCheck {
    fn required_args(&self) -> &[Name];
    fn optional_args(&self) -> &[Name];
    fn arg_type(&self, name: Name) -> TypeId;
    fn result_type(
        &self,
        args: &FxHashMap<Name, TypeId>,
        interner: &TypeInterner,
    ) -> TypeId;
}

// Example: map pattern
impl PatternTypeCheck for MapPattern {
    fn required_args(&self) -> &[Name] {
        &[name!("over"), name!("transform")]
    }

    fn arg_type(&self, name: Name) -> TypeId {
        // Determined from context
        unimplemented!()
    }

    fn result_type(
        &self,
        args: &FxHashMap<Name, TypeId>,
        interner: &TypeInterner,
    ) -> TypeId {
        // If over: [T] and transform: T -> U, result is [U]
        let over_ty = args[&name!("over")];
        let transform_ty = args[&name!("transform")];

        match (interner.resolve(over_ty), interner.resolve(transform_ty)) {
            (TypeKind::List(elem), TypeKind::Function { ret, .. }) => {
                interner.intern(TypeKind::List(ret))
            }
            _ => interner.intern(TypeKind::Infer)
        }
    }
}
```

### Error Recovery

```rust
impl<'db> TypeContext<'db> {
    /// Type check module with error recovery
    pub fn check_module(&mut self, module: Module) -> TypeCheckResult {
        let arena = module.expr_arena(self.db);

        // Check all functions, accumulating errors
        for func_id in module.functions(self.db) {
            let func = self.db.get_function(func_id);

            // Each function gets a fresh scope
            if let Err(e) = std::panic::catch_unwind(|| {
                self.check_function(&func, &arena)
            }) {
                // Internal error - log and continue
                self.diagnostics.push(internal_error(func.span));
            }
        }

        // Return typed module even with errors
        TypeCheckResult {
            module: self.build_typed_module(module),
            diagnostics: std::mem::take(&mut self.diagnostics),
        }
    }
}
```

---

## Salsa Integration

### Type Checking Queries

```rust
/// Type check a single function
#[salsa::tracked]
pub fn typed_function(db: &dyn Db, func: Function) -> TypedFunction {
    let module = func.module(db);
    let arena = module.expr_arena(db);

    let mut ctx = TypeContext::new(db);
    ctx.check_function(&func, &arena);

    TypedFunction {
        func,
        body_type: ctx.result_type,
        diagnostics: ctx.diagnostics,
    }
}

/// Type check entire module
#[salsa::tracked]
pub fn typed_module(db: &dyn Db, module: Module) -> TypedModule {
    // Resolve imports first
    let imports = resolved_imports(db, module);

    // Type check all functions
    let typed_functions: Vec<_> = module.functions(db)
        .iter()
        .map(|f| typed_function(db, *f))
        .collect();

    // Collect all diagnostics
    let diagnostics: Vec<_> = imports.errors.iter()
        .chain(typed_functions.iter().flat_map(|f| &f.diagnostics))
        .cloned()
        .collect();

    TypedModule {
        module,
        functions: typed_functions,
        diagnostics,
    }
}
```

---

## Phase 2 Deliverables Checklist

### Week 5: Type Interner
- [ ] `TypeId(u32)` with Copy, Clone, Eq, PartialEq, Hash
- [ ] `TypeKind` enum with all type variants
- [ ] `TypeInterner` with concurrent access
- [ ] Pre-interned primitive types
- [ ] O(1) type equality via TypeId comparison

### Week 6: Name Resolution
- [ ] `ScopeId` and `Scope` structures
- [ ] `Scopes` manager with enter/exit/define/resolve
- [ ] Import resolution query
- [ ] Module path resolution
- [ ] Duplicate definition errors

### Weeks 7-8: Type Checker
- [ ] `TypeContext` with bidirectional inference
- [ ] `infer` for synthesis mode
- [ ] `check` for checking mode
- [ ] Unification algorithm
- [ ] Error recovery (continue after errors)
- [ ] All expression types covered
- [ ] Pattern type checking

### Salsa Queries
- [ ] `typed_function` query
- [ ] `typed_module` query
- [ ] Incrementality for type checking

### Tests
- [ ] Type inference tests
- [ ] Error message tests
- [ ] Recovery tests (multiple errors)
- [ ] Pattern type checking tests

---

## Next Phase

With type system complete, proceed to [Phase 3: Patterns](05-phase-3-patterns.md) to build template compilation and fusion.
