# Pattern Registry

The PatternRegistry stores pattern definitions and provides lookup by name. It enables extensibility without modifying core compiler code.

## Location

```
compiler/sigilc/src/patterns/registry.rs (~289 lines)
```

## Structure

```rust
pub struct PatternRegistry {
    /// Pattern name -> Definition
    patterns: HashMap<Name, Arc<dyn PatternDefinition>>,

    /// Interner for name resolution
    interner: Arc<Interner>,
}
```

## Registration

### Built-in Patterns

```rust
impl PatternRegistry {
    pub fn with_builtins(interner: Arc<Interner>) -> Self {
        let mut registry = Self::new(interner);

        // Data transformation
        registry.register(MapPattern);
        registry.register(FilterPattern);
        registry.register(FoldPattern);
        registry.register(FindPattern);
        registry.register(CollectPattern);

        // Control flow
        registry.register(RunPattern);
        registry.register(TryPattern);
        registry.register(MatchPattern);

        // Recursion
        registry.register(RecursePattern);

        // Concurrency
        registry.register(ParallelPattern);
        registry.register(SpawnPattern);
        registry.register(TimeoutPattern);
        registry.register(RetryPattern);

        // Caching/validation
        registry.register(CachePattern::new());
        registry.register(ValidatePattern);

        // Resource management
        registry.register(WithPattern);

        registry
    }
}
```

### Manual Registration

```rust
impl PatternRegistry {
    pub fn register<P: PatternDefinition + 'static>(&mut self, pattern: P) {
        let name = self.interner.intern(pattern.name());
        self.patterns.insert(name, Arc::new(pattern));
    }

    pub fn register_arc(&mut self, pattern: Arc<dyn PatternDefinition>) {
        let name = self.interner.intern(pattern.name());
        self.patterns.insert(name, pattern);
    }
}
```

## Lookup

```rust
impl PatternRegistry {
    /// Get pattern by name
    pub fn get(&self, name: Name) -> Option<Arc<dyn PatternDefinition>> {
        self.patterns.get(&name).cloned()
    }

    /// Check if name is a pattern
    pub fn is_pattern(&self, name: Name) -> bool {
        self.patterns.contains_key(&name)
    }

    /// Get all pattern names
    pub fn pattern_names(&self) -> impl Iterator<Item = Name> + '_ {
        self.patterns.keys().copied()
    }

    /// Find similar pattern names (for suggestions)
    pub fn suggest_similar(&self, name: Name) -> Vec<Name> {
        let target = self.interner.resolve(name);

        self.patterns
            .keys()
            .filter(|&n| {
                let s = self.interner.resolve(*n);
                levenshtein_distance(target, s) <= 2
            })
            .copied()
            .collect()
    }
}
```

## Usage in Type Checker

```rust
impl TypeChecker {
    fn infer_pattern_call(
        &mut self,
        name: Name,
        args: &[NamedArg],
    ) -> Result<Type, TypeError> {
        // Get pattern from registry
        let pattern = self.pattern_registry
            .get(name)
            .ok_or_else(|| TypeError::UndefinedPattern(name))?;

        // Type check arguments
        let typed_args: Vec<TypedArg> = args
            .iter()
            .map(|arg| TypedArg {
                name: arg.name,
                ty: self.infer_expr(arg.value),
                span: arg.span,
            })
            .collect();

        // Validate required arguments
        for spec in pattern.arguments() {
            if spec.required && !typed_args.iter().any(|a| a.name.as_str() == spec.name) {
                return Err(TypeError::MissingPatternArg {
                    pattern: name,
                    arg: spec.name,
                });
            }
        }

        // Delegate to pattern's type checking
        pattern.type_check(&typed_args, self)
    }
}
```

## Usage in Evaluator

```rust
impl Evaluator {
    fn eval_pattern_call(
        &mut self,
        name: Name,
        args: &[NamedArg],
    ) -> Result<Value, EvalError> {
        // Get pattern from registry
        let pattern = self.pattern_registry
            .get(name)
            .ok_or_else(|| EvalError::UndefinedPattern(name))?;

        // Evaluate arguments
        let eval_args: Vec<EvalArg> = args
            .iter()
            .map(|arg| Ok(EvalArg {
                name: arg.name,
                value: self.eval_expr(arg.value)?,
            }))
            collect::<Result<_, _>>()?;

        // Delegate to pattern's evaluation
        pattern.evaluate(&eval_args, self)
    }
}
```

## SharedRegistry Pattern

For dependency injection in tests:

```rust
/// Newtype wrapper for shared registry
pub struct SharedRegistry<T>(Arc<T>);

impl<T> SharedRegistry<T> {
    pub fn new(inner: T) -> Self {
        Self(Arc::new(inner))
    }

    pub fn inner(&self) -> &T {
        &self.0
    }
}

impl<T> Clone for SharedRegistry<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

// Usage
pub type SharedPatternRegistry = SharedRegistry<PatternRegistry>;
```

## Thread Safety

The registry is designed for concurrent access:

```rust
// Patterns are Arc<dyn PatternDefinition>
// Registry can be cloned cheaply
let registry: SharedPatternRegistry = ...;

// Safe to use from multiple threads
thread::spawn({
    let registry = registry.clone();
    move || {
        let pattern = registry.get(name);
        // ...
    }
});
```

## Error Handling

```rust
impl PatternRegistry {
    pub fn get_or_error(&self, name: Name, span: Span) -> Result<Arc<dyn PatternDefinition>, PatternError> {
        self.get(name).ok_or_else(|| {
            let similar = self.suggest_similar(name);
            PatternError::UndefinedPattern {
                name,
                span,
                similar,
            }
        })
    }
}
```

## Extension Points

### Custom Patterns

Users can register custom patterns:

```rust
// In user code
registry.register(MyCustomPattern::new());

// Pattern is now available
my_pattern(arg: value)
```

### Plugin System (Future)

```rust
// Load patterns from dynamic library
registry.load_plugin("path/to/plugin.so")?;
```
