---
title: "Module Loading"
description: "Ori Compiler Design — Module Loading"
order: 702
section: "Evaluator"
---

# Module Loading

Module loading handles `use` statements, resolving imports and making external functions available.

## Location

```
compiler/oric/src/eval/module/import.rs (~240 lines)
```

## Import Types

### Relative Imports

```ori
use './math' { add, subtract }
use '../utils' { helper }
use './http/client' { get, post }
```

### Module Imports

```ori
use std.math { sqrt, abs }
use std.time { Duration }
```

### Private Imports

```ori
use './internal' { ::private_helper }
```

## Import Resolution

```rust
pub fn resolve_import(
    current_file: &Path,
    import: &Import,
) -> Result<PathBuf, ImportError> {
    match &import.path {
        ImportPath::Relative(rel_path) => {
            // Relative to current file
            let base = current_file.parent().unwrap_or(Path::new("."));
            let resolved = base.join(rel_path).with_extension("si");

            if resolved.exists() {
                Ok(resolved.canonicalize()?)
            } else {
                Err(ImportError::FileNotFound(resolved))
            }
        }

        ImportPath::Module(module_path) => {
            // Look in standard library / packages
            resolve_module_path(module_path)
        }
    }
}
```

## Module Loading Process

```rust
impl Evaluator {
    pub fn load_import(&mut self, import: &Import) -> Result<(), EvalError> {
        let path = resolve_import(&self.current_file, import)?;

        // Check cache first
        let module_result = if let Some(cached) = self.module_cache.get(&path) {
            cached.clone()
        } else {
            // Load and evaluate module
            let result = self.load_and_evaluate_module(&path)?;
            self.module_cache.insert(path.clone(), result.clone());
            result
        };

        // Import requested items into current scope
        for item in &import.items {
            match item {
                ImportItem::Named { name, alias } => {
                    let value = module_result.exports.get(name)
                        .ok_or(ImportError::ItemNotExported(*name))?;

                    let bind_name = alias.unwrap_or(*name);
                    self.env.bind(bind_name, value.clone());
                }

                ImportItem::Private { name } => {
                    // Private import - check for :: prefix
                    let value = module_result.all_items.get(name)
                        .ok_or(ImportError::ItemNotFound(*name))?;

                    self.env.bind(*name, value.clone());
                }
            }
        }

        Ok(())
    }
}
```

## Module Evaluation

```rust
impl Evaluator {
    fn load_and_evaluate_module(&mut self, path: &Path) -> Result<ModuleEvalResult, EvalError> {
        // Read source
        let source = std::fs::read_to_string(path)
            .map_err(|e| EvalError::IoError(e))?;

        // Create new evaluator for module
        let mut module_eval = Evaluator::new_for_module(
            self.pattern_registry.clone(),
            self.type_registry.clone(),
        );

        module_eval.current_file = path.to_path_buf();

        // Compile and evaluate
        let tokens = lexer::tokenize(&source);
        let parsed = parser::parse(&tokens)?;
        let typed = typeck::type_check(&parsed)?;
        let result = module_eval.evaluate(&typed.module)?;

        // Collect exports
        let exports = module_eval.collect_exports(&parsed.module);
        let all_items = module_eval.collect_all_items(&parsed.module);

        Ok(ModuleEvalResult {
            value: result,
            exports,
            all_items,
            output: module_eval.output,
        })
    }
}
```

## Export Collection

```rust
fn collect_exports(&self, module: &Module) -> HashMap<Name, Value> {
    let mut exports = HashMap::new();

    // Public functions
    for func in &module.functions {
        if func.is_public {
            let value = self.env.get(func.name).cloned().unwrap();
            exports.insert(func.name, value);
        }
    }

    // Public types
    for type_def in &module.types {
        if type_def.is_public {
            // Types are exported as constructors
            exports.insert(type_def.name, Value::TypeConstructor(type_def.name));
        }
    }

    // Public configs
    for config in &module.configs {
        if config.is_public {
            let value = self.env.get(config.name).cloned().unwrap();
            exports.insert(config.name, value);
        }
    }

    exports
}
```

## Circular Import Detection

```rust
impl Evaluator {
    fn load_with_cycle_check(
        &mut self,
        path: &Path,
        loading_stack: &mut Vec<PathBuf>,
    ) -> Result<ModuleEvalResult, EvalError> {
        // Check for cycle
        if loading_stack.contains(path) {
            return Err(EvalError::CircularImport {
                path: path.to_path_buf(),
                stack: loading_stack.clone(),
            });
        }

        loading_stack.push(path.to_path_buf());
        let result = self.load_and_evaluate_module(path);
        loading_stack.pop();

        result
    }
}
```

## Module Cache

```rust
pub struct ModuleCache {
    cache: HashMap<PathBuf, ModuleEvalResult>,
}

impl ModuleCache {
    pub fn get(&self, path: &Path) -> Option<&ModuleEvalResult> {
        self.cache.get(path)
    }

    pub fn insert(&mut self, path: PathBuf, result: ModuleEvalResult) {
        self.cache.insert(path, result);
    }

    pub fn invalidate(&mut self, path: &Path) {
        self.cache.remove(path);
        // Also invalidate dependents (for incremental compilation)
    }
}
```

## Standard Library Resolution

```rust
fn resolve_module_path(module_path: &[Name]) -> Result<PathBuf, ImportError> {
    // std.math -> $ORI_STDLIB/std/math.ori
    let stdlib_path = std::env::var("ORI_STDLIB")
        .unwrap_or_else(|_| "/usr/local/lib/ori/stdlib".into());

    let mut path = PathBuf::from(stdlib_path);
    for component in module_path {
        path.push(component.as_str());
    }
    path.set_extension("si");

    if path.exists() {
        Ok(path)
    } else {
        Err(ImportError::ModuleNotFound(module_path.to_vec()))
    }
}
```

## Re-exports

```ori
// In lib.ori
pub use './internal' { Widget }  // Re-export Widget
```

```rust
fn process_reexports(&mut self, module: &Module, exports: &mut HashMap<Name, Value>) {
    for import in &module.imports {
        if import.is_reexport {
            for item in &import.items {
                if let ImportItem::Named { name, alias } = item {
                    let value = self.env.get(*name).cloned().unwrap();
                    exports.insert(alias.unwrap_or(*name), value);
                }
            }
        }
    }
}
```

## Error Handling

```rust
pub enum ImportError {
    FileNotFound(PathBuf),
    ModuleNotFound(Vec<Name>),
    ItemNotExported(Name),
    ItemNotFound(Name),
    CircularImport { path: PathBuf, stack: Vec<PathBuf> },
    ParseError(ParseError),
    TypeError(TypeError),
}
```
