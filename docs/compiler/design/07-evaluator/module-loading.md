---
title: "Module Loading"
description: "Ori Compiler Design — Module Loading"
order: 702
section: "Evaluator"
---

# Module Loading

Module loading handles `use` statements, resolving imports and making external functions available. The implementation is split between:

- **`ori_eval/src/module_registration.rs`** - Pure registration functions (Salsa-free)
- **`oric/src/eval/module/import.rs`** - Import resolution with Salsa tracking
- **`oric/src/eval/evaluator/module_loading.rs`** - High-level module loading orchestration

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     ori_eval                            │
│  module_registration.rs:                               │
│    - register_module_functions()                        │
│    - collect_impl_methods()                            │
│    - collect_extend_methods()                          │
│    - register_variant_constructors()                   │
│    - register_newtype_constructors()                   │
└─────────────────────────────────────────────────────────┘
              ▲
              │ delegates to
    ┌─────────┴─────────┐
    │       oric        │
    │  module/import.rs │  ← Salsa-tracked file loading
    │  module_loading.rs│  ← Orchestration
    └───────────────────┘
```

## Import Types

### Relative Imports

```ori
use "./math" { add, subtract }
use "../utils" { helper }
use "./http/client" { get, post }
```

### Module Imports

```ori
use std.math { sqrt, abs }
use std.time { Duration }
```

### Module Alias

```ori
use std.net.http as http
```

When a module alias is used, no individual items are imported. Instead, the entire module is bound to the alias name as a `Value::ModuleNamespace`, enabling qualified access like `http.get(...)`.

### Re-exports

```ori
pub use "./internal" { helper, Widget }
```

Re-exports make imported items available to modules that import this module. The `is_public` flag on `UseDef` marks the import for re-export.

### Private Imports

```ori
use "./internal" { ::private_helper }
```

## Import Resolution

Import resolution goes through Salsa for proper dependency tracking:

```rust
/// Result of resolving an import through the Salsa database.
pub struct ResolvedImport {
    /// The loaded source file as a Salsa input.
    pub file: SourceFile,
    /// The resolved file path (for error messages and cycle detection).
    pub path: PathBuf,
}

pub fn resolve_import(
    db: &dyn Db,
    import_path: &ImportPath,
    current_file: &Path,
) -> Result<ResolvedImport, ImportError> {
    match import_path {
        ImportPath::Relative(name) => {
            let path = resolve_relative_path_to_pathbuf(*name, current_file, interner);
            match db.load_file(&path) {
                Some(file) => Ok(ResolvedImport { file, path }),
                None => Err(ImportError::new(format!(
                    "cannot find import '{}' at '{}'",
                    interner.lookup(*name),
                    path.display()
                ))),
            }
        }
        ImportPath::Module(segments) => resolve_module_import_tracked(db, segments, current_file),
    }
}
```

## Module Loading Process

The `Evaluator::load_module` method orchestrates the full loading process:

```rust
impl Evaluator<'_> {
    pub fn load_module(
        &mut self,
        parse_result: &ParseOutput,
        file_path: &Path,
        canon: Option<&SharedCanonResult>,
    ) -> Result<(), String> {
        // 1. Auto-load prelude if not already loaded
        if !self.prelude_loaded {
            self.load_prelude(file_path)?;
        }

        // 2. Resolve and load imports via Salsa
        for imp in &parse_result.module.imports {
            let resolved = import::resolve_import(self.db, &imp.path, file_path)?;
            let imported_result = parsed(self.db, resolved.file);
            // ... register imported items ...
        }

        // 3. Create shared arena for this module
        let shared_arena = SharedArena::new(parse_result.arena.clone());

        // 4. Register module functions (delegates to ori_eval)
        register_module_functions(&parse_result.module, &shared_arena, self.env_mut());

        // 5. Register constructors (delegates to ori_eval)
        register_variant_constructors(&parse_result.module, self.env_mut());
        register_newtype_constructors(&parse_result.module, self.env_mut());

        // 6. Collect impl/extend methods (delegates to ori_eval)
        let mut user_methods = UserMethodRegistry::new();
        let captures = self.env().capture();
        collect_impl_methods(&parse_result.module, &shared_arena, &captures, &mut user_methods);
        collect_extend_methods(&parse_result.module, &shared_arena, &captures, &mut user_methods);

        // 7. Process derived traits
        process_derives(&parse_result.module, &type_registry, &mut user_methods, self.interner());

        // 8. Merge methods into interpreter's registry
        self.user_method_registry().write().merge(user_methods);

        Ok(())
    }
}
```

## Registration Functions (ori_eval)

These pure functions work without Salsa and can be used by any client:

### register_module_functions

Registers all functions from a module into the environment:

```rust
pub fn register_module_functions(module: &Module, arena: &SharedArena, env: &mut Environment) {
    for func in &module.functions {
        let params = arena.get_param_names(func.params);
        let capabilities: Vec<_> = func.capabilities.iter().map(|c| c.name).collect();
        let captures = env.capture();

        let func_value = FunctionValue::with_capabilities(
            params,
            func.body,
            captures,
            arena.clone(),
            capabilities,
        );
        env.define(func.name, Value::Function(func_value), false);
    }
}
```

### collect_impl_methods

Collects methods from impl blocks, including default trait methods:

```rust
pub fn collect_impl_methods(
    module: &Module,
    arena: &SharedArena,
    captures: &HashMap<Name, Value>,
    registry: &mut UserMethodRegistry,
) {
    // Build trait map for default method lookup
    let trait_map: HashMap<Name, &TraitDef> = module.traits
        .iter()
        .map(|t| (t.name, t))
        .collect();

    for impl_def in &module.impls {
        let type_name = impl_def.self_path.last().unwrap();

        // Register explicit methods
        for method in &impl_def.methods {
            let user_method = UserMethod::new(
                arena.get_param_names(method.params),
                method.body,
                captures.clone(),
                arena.clone(),
            );
            registry.register(*type_name, method.name, user_method);
        }

        // Register default trait methods not overridden
        if let Some(trait_path) = &impl_def.trait_path {
            // ... register unoverridden default methods ...
        }
    }
}
```

### collect_extend_methods

Collects methods from extend blocks (trait extensions):

```rust
pub fn collect_extend_methods(
    module: &Module,
    arena: &SharedArena,
    captures: &HashMap<Name, Value>,
    registry: &mut UserMethodRegistry,
) {
    for extend_def in &module.extends {
        let type_name = extend_def.target_type_name;

        for method in &extend_def.methods {
            let user_method = UserMethod::new(
                arena.get_param_names(method.params),
                method.body,
                captures.clone(),
                arena.clone(),
            );
            registry.register(type_name, method.name, user_method);
        }
    }
}
```

### register_variant_constructors

Registers constructors for sum type variants:

```rust
pub fn register_variant_constructors(module: &Module, env: &mut Environment) {
    for type_decl in &module.types {
        if let TypeDeclKind::Sum(variants) = &type_decl.kind {
            for variant in variants {
                if variant.fields.is_empty() {
                    // Unit variant: bind directly as Value::Variant
                    let value = Value::variant(type_decl.name, variant.name, vec![]);
                    env.define_global(variant.name, value);
                } else {
                    // Variant with fields: create constructor function
                    let value = Value::variant_constructor(
                        type_decl.name,
                        variant.name,
                        variant.fields.len(),
                    );
                    env.define_global(variant.name, value);
                }
            }
        }
    }
}
```

### register_newtype_constructors

Registers constructors for newtypes:

```rust
pub fn register_newtype_constructors(module: &Module, env: &mut Environment) {
    for type_decl in &module.types {
        if let TypeDeclKind::Newtype(_) = &type_decl.kind {
            let value = Value::newtype_constructor(type_decl.name);
            env.define_global(type_decl.name, value);
        }
    }
}
```

### register_module_alias

Registers a module alias by collecting all public functions into a namespace value:

```rust
fn register_module_alias(
    import: &UseDef,
    imported: &ImportedModule<'_>,
    env: &mut Environment,
    alias: Name,
    import_path: &Path,
) -> Result<(), ImportError> {
    let mut namespace: HashMap<Name, Value> = HashMap::new();

    for func in &imported.result.module.functions {
        if func.is_public {
            let params = imported.result.arena.get_param_names(func.params);
            let captures = env.capture();
            let capabilities: Vec<_> = func.capabilities.iter().map(|c| c.name).collect();

            let func_value = FunctionValue::with_details(
                params,
                func.body,
                Some(captures),
                SharedArena::new(imported.result.arena.clone()),
                Some(func.name),
                capabilities,
            );
            namespace.insert(func.name, Value::Function(func_value));
        }
    }

    env.define(alias, Value::module_namespace(namespace), Mutability::Immutable);
    Ok(())
}
```

The resulting `ModuleNamespace` value enables qualified access like `http.get(...)`. Field access on a module namespace looks up the function by name and returns it.

## Test Module Access

Test modules in `_test/` directories with `.test.ori` extension can access private items from their parent module without the `::` prefix:

```rust
pub fn is_test_module(path: &Path) -> bool {
    let has_test_extension = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".test.ori"));

    if !has_test_extension {
        return false;
    }

    path.parent().is_some_and(|parent| {
        parent.components().any(|c| c.as_os_str().to_str() == Some("_test"))
    })
}
```

## Circular Import Detection

```rust
pub struct LoadingContext {
    loading_stack: Vec<PathBuf>,
    loaded: HashSet<PathBuf>,
}

impl LoadingContext {
    pub fn start_loading(&mut self, path: PathBuf) -> Result<(), ImportError> {
        if self.would_cycle(&path) {
            let cycle: Vec<String> = self.loading_stack
                .iter()
                .chain(std::iter::once(&path))
                .map(|p| p.display().to_string())
                .collect();
            return Err(ImportError::new(format!(
                "circular import detected: {}",
                cycle.join(" -> ")
            )));
        }
        self.loading_stack.push(path);
        Ok(())
    }

    pub fn finish_loading(&mut self, path: PathBuf) {
        self.loading_stack.pop();
        self.loaded.insert(path);
    }
}
```

## Standard Library Resolution

Module paths are resolved by searching standard locations:

```rust
fn generate_module_candidates(components: &[&str], current_file: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // 1. ORI_STDLIB environment variable
    if let Ok(stdlib_path) = std::env::var("ORI_STDLIB") {
        let mut path = PathBuf::from(stdlib_path);
        for component in components {
            path.push(component);
        }
        candidates.push(path.with_extension("ori"));
    }

    // 2. Walk up directory tree for library/ directories
    let mut dir = current_file.parent();
    while let Some(d) = dir {
        let library_dir = d.join("library");
        // Try library/std/math.ori pattern
        // Try library/std/math/mod.ori pattern
        dir = d.parent();
    }

    // 3. System locations (/usr/local/lib/ori/stdlib, etc.)

    candidates
}
```

## Error Handling

```rust
pub struct ImportError {
    pub message: String,
}

// Common error cases:
// - "cannot find import 'math' at './math.ori'"
// - "module 'std.math' not found"
// - "'private_fn' is private in './mod'. Use '::private_fn' to import private items."
// - "'unknown_fn' not found in './mod'"
// - "circular import detected: a.ori -> b.ori -> a.ori"
```
