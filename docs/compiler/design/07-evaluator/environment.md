# Environment

The Environment manages variable bindings during evaluation. It uses a stack of scopes for lexical scoping.

## Location

```
compiler/oric/src/eval/environment.rs (~408 lines)
```

## Structure

```rust
pub struct Environment {
    /// Stack of scopes (innermost last)
    scopes: Vec<Scope>,
}

pub struct Scope {
    /// Variable bindings
    bindings: HashMap<Name, Value>,

    /// Scope kind for debugging
    kind: ScopeKind,
}

pub enum ScopeKind {
    Global,
    Function(Name),
    Block,
    Lambda,
    ForLoop,
    MatchArm,
}
```

## Operations

### Creating Environment

```rust
impl Environment {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::global()],
        }
    }

    pub fn with_builtins() -> Self {
        let mut env = Self::new();

        // Bind built-in functions
        env.bind(name("print"), Value::Builtin(BuiltinFn::Print));
        env.bind(name("len"), Value::Builtin(BuiltinFn::Len));
        // ... more builtins

        env
    }
}
```

### Scope Management

```rust
impl Environment {
    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::block());
    }

    pub fn push_function_scope(&mut self, name: Name) {
        self.scopes.push(Scope::function(name));
    }

    pub fn push_scope_with(&mut self, captured: Scope) {
        // For closures - start with captured environment
        self.scopes.push(captured);
    }

    pub fn pop_scope(&mut self) -> Scope {
        self.scopes.pop().expect("cannot pop global scope")
    }
}
```

### Variable Access

```rust
impl Environment {
    pub fn bind(&mut self, name: Name, value: Value) {
        let scope = self.scopes.last_mut().expect("no scope");
        scope.bindings.insert(name, value);
    }

    pub fn get(&self, name: Name) -> Option<&Value> {
        // Search from innermost to outermost
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.bindings.get(&name) {
                return Some(value);
            }
        }
        None
    }

    pub fn set(&mut self, name: Name, value: Value) -> Result<(), EvalError> {
        // Find and update existing binding
        for scope in self.scopes.iter_mut().rev() {
            if scope.bindings.contains_key(&name) {
                scope.bindings.insert(name, value);
                return Ok(());
            }
        }
        Err(EvalError::UndefinedVariable(name))
    }
}
```

## Scoping Rules

### Lexical Scoping

Variables are looked up in the lexical scope:

```ori
let x = 1

@foo () -> int = x + 1  // x refers to outer x

let result = run(
    let x = 10,         // Shadows outer x
    x + 1,              // Uses inner x = 10
)
// result = 11, outer x unchanged
```

### Shadowing

Inner scopes can shadow outer bindings:

```rust
fn eval_let(&mut self, name: Name, value: ExprId, body: ExprId) -> Result<Value, EvalError> {
    let value = self.eval_expr(value)?;

    self.env.push_scope();
    self.env.bind(name, value);  // May shadow outer binding

    let result = self.eval_expr(body);

    self.env.pop_scope();  // Shadowing ends
    result
}
```

### Closures

Closures capture their environment:

```ori
let multiplier = 2
let double = x -> x * multiplier  // Captures multiplier
double(5)  // 10
```

```rust
fn eval_lambda(&mut self, params: &[Name], body: ExprId) -> Result<Value, EvalError> {
    // Capture current environment
    let captured = self.env.capture_scope();

    Ok(Value::Function(FunctionValue {
        params: params.to_vec(),
        body,
        captured_env: captured,
    }))
}

impl Environment {
    pub fn capture_scope(&self) -> Scope {
        // Clone all visible bindings
        let mut bindings = HashMap::new();
        for scope in &self.scopes {
            for (name, value) in &scope.bindings {
                bindings.insert(*name, value.clone());
            }
        }
        Scope { bindings, kind: ScopeKind::Lambda }
    }
}
```

## Mutation

Mutable variables use `let mut`:

```ori
let mut x = 0
run(
    x = x + 1,  // Mutate x
    x = x + 1,
    x,
)
// x = 2
```

```rust
fn eval_assign(&mut self, name: Name, value: ExprId) -> Result<Value, EvalError> {
    let value = self.eval_expr(value)?;
    self.env.set(name, value)?;
    Ok(Value::Void)
}
```

## Function Scopes

Functions get their own scope:

```rust
fn call_function(&mut self, func: &FunctionValue, args: Vec<Value>) -> Result<Value, EvalError> {
    // Start with captured environment (for closures)
    self.env.push_scope_with(func.captured_env.clone());

    // Bind parameters
    for (param, arg) in func.params.iter().zip(args) {
        self.env.bind(*param, arg);
    }

    let result = self.eval_expr(func.body);

    self.env.pop_scope();
    result
}
```

## For Loop Scopes

For loops create a scope for the iteration variable:

```rust
fn eval_for(&mut self, var: Name, iter: ExprId, body: ExprId) -> Result<Value, EvalError> {
    let items = self.eval_expr(iter)?.as_list()?;

    for item in items.iter() {
        self.env.push_scope();
        self.env.bind(var, item.clone());
        self.eval_expr(body)?;
        self.env.pop_scope();
    }

    Ok(Value::Void)
}
```

## Debugging

Print environment state:

```rust
impl Environment {
    pub fn debug_print(&self) {
        for (i, scope) in self.scopes.iter().enumerate() {
            eprintln!("Scope {} ({:?}):", i, scope.kind);
            for (name, value) in &scope.bindings {
                eprintln!("  {} = {:?}", name, value);
            }
        }
    }
}
```

## Memory Considerations

- Values are cloned when captured in closures
- Arc-wrapped values (List, String, etc.) share memory
- Scope cleanup happens immediately on pop
