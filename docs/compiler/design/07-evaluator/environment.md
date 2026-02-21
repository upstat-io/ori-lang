---
title: "Environment"
description: "Ori Compiler Design — Environment"
order: 701
section: "Evaluator"
---

# Environment

The Environment manages variable bindings during evaluation. It uses a stack of scopes for lexical scoping.

## Location

```
compiler/ori_eval/src/environment.rs
```

## Structure

The environment uses a parent-linked scope chain with `Rc<RefCell<_>>` wrapped in `LocalScope<T>`:

```rust
/// Single-threaded scope wrapper (Rc<RefCell<T>>)
#[repr(transparent)]
pub struct LocalScope<T>(Rc<RefCell<T>>);

pub struct Scope {
    /// Variable bindings (FxHashMap for faster Name hashing)
    bindings: FxHashMap<Name, Binding>,
    /// Parent scope for lexical scoping
    parent: Option<LocalScope<Scope>>,
}

/// A variable binding with mutability tracking
struct Binding {
    value: Value,
    mutability: Mutability,
}

pub enum Mutability {
    Mutable,    // let x = ...
    Immutable,  // let $x = ...
}

pub struct Environment {
    /// Stack of scopes (for push/pop during evaluation)
    scopes: Vec<LocalScope<Scope>>,
    /// Global scope (always accessible)
    global: LocalScope<Scope>,
}
```

**Key design decisions:**
- `LocalScope<T>` uses `Rc<RefCell<T>>` (not `Arc`) for single-threaded interpreter
- `FxHashMap` provides faster hashing for `Name` keys
- Parent chain enables closure capture lookup
- `Mutability` prevents reassignment of `$` constants

## Operations

### Creating Environment

```rust
impl Environment {
    pub fn new() -> Self {
        let global = LocalScope::new(Scope::new());
        Self {
            scopes: vec![global.clone()],
            global,
        }
    }
}
```

### Variable Binding

```rust
impl Scope {
    /// Define a variable with specified mutability
    pub fn define(&mut self, name: Name, value: Value, mutability: Mutability) {
        self.bindings.insert(name, Binding { value, mutability });
    }

    /// Look up a variable (checks parent chain)
    pub fn lookup(&self, name: Name) -> Option<Value> {
        if let Some(binding) = self.bindings.get(&name) {
            return Some(binding.value.clone());
        }
        if let Some(parent) = &self.parent {
            return parent.borrow().lookup(name);
        }
        None
    }

    /// Assign to a mutable variable.
    /// Returns `AssignError::Immutable` if the variable is immutable,
    /// or `AssignError::Undefined` if the variable is not found.
    pub fn assign(&mut self, name: Name, value: Value) -> Result<(), AssignError> {
        if let Some(binding) = self.bindings.get_mut(&name) {
            if !binding.mutability.is_mutable() {
                return Err(AssignError::Immutable);
            }
            binding.value = value;
            return Ok(());
        }
        if let Some(parent) = &self.parent {
            return parent.borrow_mut().assign(name, value);
        }
        Err(AssignError::Undefined)
    }
}

/// Error type for variable assignment failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssignError {
    /// Variable exists but is immutable.
    Immutable,
    /// Variable not found in any scope.
    Undefined,
}
```

### Scope Management

```rust
impl Environment {
    pub fn push_scope(&mut self) {
        let current = self.scopes.last().cloned();
        let new_scope = LocalScope::new(match current {
            Some(parent) => Scope::with_parent(parent),
            None => Scope::new(),
        });
        self.scopes.push(new_scope);
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn push_scope_with(&mut self, captured: Scope) {
        // For closures - start with captured environment
        self.scopes.push(captured);
    }
}
```

### Variable Access

The `Environment` delegates to the current scope (top of the stack) for all variable operations. The `Scope` handles parent-chain traversal for lookups and assignments:

```rust
impl Environment {
    /// Define a variable in the current scope.
    pub fn define(&mut self, name: Name, value: Value, mutability: Mutability) {
        self.scopes.last()
            .unwrap_or(&self.global)
            .borrow_mut()
            .define(name, value, mutability);
    }

    /// Look up a variable (delegates to current scope's parent chain).
    pub fn lookup(&self, name: Name) -> Option<Value> {
        self.scopes.last()
            .unwrap_or(&self.global)
            .borrow()
            .lookup(name)
    }

    /// Assign to a mutable variable. Returns `AssignError` on failure.
    pub fn assign(&mut self, name: Name, value: Value) -> Result<(), AssignError> {
        self.scopes.last()
            .unwrap_or(&self.global)
            .borrow_mut()
            .assign(name, value)
    }

    /// Define a global variable (immutable).
    pub fn define_global(&mut self, name: Name, value: Value) {
        self.global.borrow_mut().define(name, value, Mutability::Immutable);
    }

    /// Get the current scope depth.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Create a child environment that shares the global scope
    /// but has its own local scope stack.
    pub fn child(&self) -> Self {
        let global = self.global.clone();
        Environment {
            scopes: vec![global.clone()],
            global,
        }
    }
}
```

## Scoping Rules

### Lexical Scoping

Variables are looked up in the lexical scope:

```ori
let x = 1;

@foo () -> int = x + 1;  // x refers to outer x

let result = {
    let x = 10;         // Shadows outer x
    x + 1               // Uses inner x = 10
};
// result = 11, outer x unchanged
```

### Shadowing

Inner scopes can shadow outer bindings:

```rust
fn eval_let(&mut self, name: Name, value: ExprId, body: ExprId) -> Result<Value, EvalError> {
    let value = self.eval_expr(value)?;

    self.env.push_scope();
    self.env.define(name, value, Mutability::Immutable);  // May shadow outer binding

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
    // Capture current environment as a flat map of all visible bindings
    let captured = self.env.capture();

    Ok(Value::Function(FunctionValue::new(
        params.to_vec(),
        captured,
        self.arena.clone(),
    )))
}

impl Environment {
    /// Capture all visible bindings as a flat `FxHashMap<Name, Value>`.
    /// Returns a map of all visible bindings that can be used
    /// when the closure is called later.
    pub fn capture(&self) -> FxHashMap<Name, Value> {
        // Collects bindings from all scopes, innermost-first
        // (inner bindings shadow outer ones via entry API)
        let mut captures = FxHashMap::default();
        // ... recursive collection from scope chain
        captures
    }
}
```

## Mutation

Mutable variables use `let mut`:

```ori
let x = 0;
{
    x = x + 1;  // Mutate x
    x = x + 1;
    x
}
// x = 2
```

```rust
fn eval_assign(&mut self, name: Name, value: ExprId) -> Result<Value, EvalError> {
    let value = self.eval_expr(value)?;
    self.env.assign(name, value)?;
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
        self.env.define(*param, arg, Mutability::Immutable);
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
        self.env.define(var, item.clone(), Mutability::Immutable);
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
            eprintln!("Scope {}:", i);
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
