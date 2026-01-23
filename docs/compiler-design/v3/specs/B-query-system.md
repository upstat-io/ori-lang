# B: Query System Specification

This document specifies all Salsa queries for the V2 compiler.

---

## Database Trait

```rust
/// Main compiler database trait
#[salsa::db]
pub trait Db: salsa::Database {
    /// Access string interner
    fn interner(&self) -> &StringInterner;

    /// Access type interner
    fn type_interner(&self) -> &TypeInterner;

    /// Access pattern registry
    fn pattern_registry(&self) -> &PatternRegistry;

    /// Access pattern template cache
    fn pattern_template_cache(&self) -> &PatternTemplateCache;
}
```

---

## Input Queries

### SourceFile

```rust
/// Input: Source file content
///
/// Durability: Configurable per file
/// - LOW: User code being edited
/// - MEDIUM: Project configuration
/// - HIGH: Standard library, dependencies
#[salsa::input]
pub struct SourceFile {
    /// Absolute path to file
    #[return_ref]
    pub path: PathBuf,

    /// Source text content
    #[return_ref]
    pub text: String,

    /// Modification timestamp (for cache validation)
    #[default]
    pub mtime: Option<SystemTime>,

    /// Durability level
    #[default]
    pub durability: Durability,
}
```

### ProjectConfig

```rust
/// Input: Project-wide configuration
#[salsa::input]
pub struct ProjectConfig {
    /// Project root directory
    #[return_ref]
    pub root: PathBuf,

    /// Target platform
    pub target: Target,

    /// Optimization level
    pub opt_level: OptLevel,

    /// Feature flags
    #[return_ref]
    pub features: FxHashSet<Name>,
}
```

---

## Lexing Queries

### tokens

```rust
/// Query: Lex source file into tokens
///
/// Input: SourceFile
/// Output: TokenList (owned, cached)
///
/// Invalidation: When SourceFile.text changes
/// Early cutoff: Never (tokens always differ if text differs)
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    let source = file.text(db);
    let interner = db.interner();

    let lexer = Lexer::new(source, interner);
    lexer.lex_all()
}
```

### TokenList

```rust
/// Cached token list
#[derive(Clone, Eq, PartialEq)]
pub struct TokenList {
    /// Tokens with spans
    pub tokens: Vec<Token>,
    /// Trivia (whitespace, comments) between tokens
    pub trivia: Vec<TriviaRange>,
}
```

---

## Parsing Queries

### parsed_module

```rust
/// Query: Parse tokens into module AST
///
/// Input: SourceFile
/// Output: Module (tracked struct)
///
/// Invalidation: When tokens(file) changes
/// Early cutoff: Yes - if AST structure unchanged, dependents don't rerun
#[salsa::tracked]
pub fn parsed_module<'db>(db: &'db dyn Db, file: SourceFile) -> Module<'db> {
    let tokens = tokens(db, file);
    let interner = db.interner();

    let mut arena = ExprArena::new();
    let mut parser = Parser::new(&tokens, interner, &mut arena);

    let items = parser.parse_module();
    let imports = parser.imports();
    let diagnostics = parser.diagnostics();

    Module::new(db, file, items, arena, imports, diagnostics)
}
```

### Module (Tracked Struct)

```rust
/// Parsed module
#[salsa::tracked]
pub struct Module<'db> {
    /// Source file
    pub file: SourceFile,

    /// Top-level items (functions, types, configs)
    #[return_ref]
    pub items: Vec<ItemId>,

    /// Expression arena for this module
    #[return_ref]
    pub expr_arena: ExprArena,

    /// Import declarations
    #[return_ref]
    pub imports: Vec<Import>,

    /// Parse diagnostics
    #[return_ref]
    pub diagnostics: Vec<Diagnostic>,
}
```

### Function (Tracked Struct)

```rust
/// Function definition
#[salsa::tracked]
pub struct Function<'db> {
    /// Function name
    pub name: Name,

    /// Parent module
    pub module: Module<'db>,

    /// Parameters
    #[return_ref]
    pub params: Vec<(Name, TypeId)>,

    /// Return type
    pub return_type: TypeId,

    /// Body expression
    pub body: ExprId,

    /// Visibility
    pub visibility: Visibility,

    /// Capabilities
    #[return_ref]
    pub capabilities: Vec<Name>,

    /// Definition span
    pub span: Span,
}
```

---

## Name Resolution Queries

### resolved_imports

```rust
/// Query: Resolve import declarations
///
/// Input: Module
/// Output: ResolvedImports
///
/// Invalidation: When module.imports changes or imported modules change
#[salsa::tracked]
pub fn resolved_imports<'db>(db: &'db dyn Db, module: Module<'db>) -> ResolvedImports<'db> {
    let mut resolved = Vec::new();
    let mut errors = Vec::new();

    for import in module.imports(db) {
        match resolve_single_import(db, module, import) {
            Ok(r) => resolved.push(r),
            Err(e) => errors.push(e),
        }
    }

    ResolvedImports { resolved, errors }
}
```

### module_exports

```rust
/// Query: Get publicly exported symbols from module
///
/// Input: Module
/// Output: ModuleExports
#[salsa::tracked]
pub fn module_exports<'db>(db: &'db dyn Db, module: Module<'db>) -> ModuleExports<'db> {
    let mut functions = Vec::new();
    let mut types = Vec::new();
    let mut configs = Vec::new();

    for item in module.items(db) {
        match item {
            ItemId::Function(f) if f.visibility(db) == Visibility::Public => {
                functions.push(f);
            }
            ItemId::Type(t) if t.visibility(db) == Visibility::Public => {
                types.push(t);
            }
            ItemId::Config(c) if c.visibility(db) == Visibility::Public => {
                configs.push(c);
            }
            _ => {}
        }
    }

    ModuleExports { functions, types, configs }
}
```

---

## Type Checking Queries

### typed_function

```rust
/// Query: Type check a single function
///
/// Input: Function
/// Output: TypedFunction
///
/// Invalidation: When function body changes or dependencies change
/// Early cutoff: Yes - if inferred types unchanged
#[salsa::tracked]
pub fn typed_function<'db>(db: &'db dyn Db, func: Function<'db>) -> TypedFunction<'db> {
    let module = func.module(db);
    let arena = module.expr_arena(db);

    let mut ctx = TypeContext::new(db, module);

    // Type check function body
    let body_type = ctx.check_function(func, arena);

    TypedFunction::new(
        db,
        func,
        body_type,
        ctx.into_diagnostics(),
    )
}
```

### typed_module

```rust
/// Query: Type check entire module
///
/// Input: Module
/// Output: TypedModule
#[salsa::tracked]
pub fn typed_module<'db>(db: &'db dyn Db, module: Module<'db>) -> TypedModule<'db> {
    // Resolve imports first
    let imports = resolved_imports(db, module);

    // Type check all functions
    let typed_functions: Vec<_> = module.functions(db)
        .iter()
        .map(|f| typed_function(db, *f))
        .collect();

    // Collect diagnostics
    let mut diagnostics = imports.errors.clone();
    for tf in &typed_functions {
        diagnostics.extend(tf.diagnostics(db).iter().cloned());
    }

    TypedModule::new(db, module, typed_functions, diagnostics)
}
```

### TypedFunction

```rust
/// Type-checked function
#[salsa::tracked]
pub struct TypedFunction<'db> {
    pub func: Function<'db>,
    pub body_type: TypeId,
    #[return_ref]
    pub diagnostics: Vec<Diagnostic>,
}
```

---

## Test Queries

### test_result

```rust
/// Query: Run a single test
///
/// Input: Test
/// Output: TestResult
///
/// Invalidation: When test function or dependencies change
#[salsa::tracked]
pub fn test_result<'db>(db: &'db dyn Db, test: Test<'db>) -> TestResult {
    let typed = typed_function(db, test.function(db));

    if typed.has_errors() {
        return TestResult::CompileError(typed.diagnostics(db).clone());
    }

    // Run test
    let start = Instant::now();
    let result = std::panic::catch_unwind(|| {
        eval_test(db, &typed)
    });
    let duration = start.elapsed();

    match result {
        Ok(Ok(())) => TestResult::Pass { duration },
        Ok(Err(e)) => TestResult::Fail { error: e, duration },
        Err(panic) => TestResult::Panic { panic, duration },
    }
}
```

### tests_for_function

```rust
/// Query: Find all tests targeting a function
///
/// Input: Function
/// Output: Vec<Test>
#[salsa::tracked]
pub fn tests_for_function<'db>(db: &'db dyn Db, func: Function<'db>) -> Vec<Test<'db>> {
    let module = func.module(db);

    module.tests(db)
        .iter()
        .filter(|test| test.targets(db).contains(&func))
        .copied()
        .collect()
}
```

### function_test_coverage

```rust
/// Query: Check test coverage for module
///
/// Input: Module
/// Output: CoverageReport
#[salsa::tracked]
pub fn function_test_coverage<'db>(db: &'db dyn Db, module: Module<'db>) -> CoverageReport {
    let functions = module.functions(db);
    let tests = module.tests(db);

    let mut coverage = FxHashMap::default();

    for func in functions {
        // Skip main
        if func.name(db) == name!("main") {
            continue;
        }
        coverage.insert(func.name(db), false);
    }

    for test in tests {
        for target in test.targets(db) {
            coverage.insert(target.name(db), true);
        }
    }

    let uncovered: Vec<_> = coverage
        .iter()
        .filter(|(_, covered)| !**covered)
        .map(|(name, _)| *name)
        .collect();

    if uncovered.is_empty() {
        CoverageReport::Complete
    } else {
        CoverageReport::Incomplete { uncovered }
    }
}
```

---

## Code Generation Queries

### generated_function

```rust
/// Query: Generate C code for function
///
/// Input: Function
/// Output: GeneratedFunction
#[salsa::tracked]
pub fn generated_function<'db>(db: &'db dyn Db, func: Function<'db>) -> GeneratedFunction {
    let typed = typed_function(db, func);

    let mut ctx = CodegenContext::new(db);
    let code = ctx.generate_function(&typed);

    GeneratedFunction {
        func,
        code,
        dependencies: ctx.dependencies(),
    }
}
```

### generated_module

```rust
/// Query: Generate C code for module
///
/// Input: Module
/// Output: GeneratedModule
#[salsa::tracked]
pub fn generated_module<'db>(db: &'db dyn Db, module: Module<'db>) -> GeneratedModule {
    let typed = typed_module(db, module);

    let functions: Vec<_> = typed.functions(db)
        .iter()
        .map(|f| generated_function(db, f.func(db)))
        .collect();

    let header = generate_module_header(db, module);

    GeneratedModule {
        module,
        functions,
        header,
    }
}
```

---

## Durability Levels

### Configuration

```rust
/// Durability levels for query caching
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Durability {
    /// User code being edited - check every revision
    Low = 0,

    /// Project config - check occasionally
    Medium = 1,

    /// Standard library - never recheck unless explicitly invalidated
    High = 2,
}
```

### Application

```rust
/// Set durability on file load
fn load_file(db: &mut CompilerDb, path: &Path, durability: Durability) -> SourceFile {
    let text = std::fs::read_to_string(path).unwrap();
    let file = SourceFile::new(db, path.to_path_buf(), text);

    file.set_durability(db).to(durability);

    file
}

/// Load standard library with HIGH durability
fn load_stdlib(db: &mut CompilerDb) {
    for (path, content) in STDLIB_FILES {
        let file = SourceFile::new(db, PathBuf::from(path), content.to_string());
        file.set_durability(db).to(Durability::High);
    }
}
```

### Optimization

When only LOW durability inputs change, Salsa skips validation of MEDIUM and HIGH durability queries entirely. This means:

- Editing user code never revalidates stdlib
- Changing project config doesn't revalidate stdlib
- Only stdlib source changes trigger stdlib revalidation

---

## Early Cutoff

### How It Works

Salsa tracks dependencies at query granularity. When a dependency changes, Salsa:

1. Recomputes the dependent query
2. Compares new output to cached output
3. If unchanged, stops propagation (early cutoff)

### Example

```
tokens(file) → parsed_module(file) → typed_function(func) → generated_function(func)
```

If the user adds a comment:
1. `tokens` recomputes (tokens changed)
2. `parsed_module` recomputes (tokens changed)
3. `parsed_module` output equals cached (AST unchanged due to trivia handling)
4. Early cutoff: `typed_function` and `generated_function` don't rerun

### Enabling Cutoff

Tracked structs automatically enable cutoff. For tracked functions returning owned data:

```rust
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    // TokenList derives Eq, enabling cutoff
}
```

---

## Query Dependencies

### Automatic Tracking

Salsa automatically tracks which queries a query reads:

```rust
#[salsa::tracked]
pub fn typed_function(db: &dyn Db, func: Function) -> TypedFunction {
    let module = func.module(db);       // Depends on: func
    let arena = module.expr_arena(db);  // Depends on: parsed_module(file)
    let imports = resolved_imports(db, module);  // Depends on: resolved_imports

    // ...
}
```

### Dependency Graph

```
SourceFile
    ↓
tokens
    ↓
parsed_module
    ↓           ↘
resolved_imports  module_exports
    ↓           ↙
typed_function
    ↓
generated_function
```

---

## Query Statistics

For debugging and optimization, track query execution:

```rust
impl CompilerDb {
    /// Get query execution statistics
    pub fn query_stats(&self) -> QueryStats {
        QueryStats {
            tokens_computed: self.tokens_count(),
            tokens_cached: self.tokens_cache_hits(),
            parsed_modules_computed: self.parsed_module_count(),
            // ...
        }
    }
}
```
