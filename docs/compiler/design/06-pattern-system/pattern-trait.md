---
title: "Pattern Trait"
description: "Ori Compiler Design — Pattern Trait"
order: 604
section: "Pattern System"
---

# Pattern Trait

The `PatternDefinition` trait defines the interface for all patterns in Ori.

## Trait Definition

```rust
pub trait PatternDefinition: Send + Sync {
    /// Pattern name (e.g., "recurse", "parallel")
    fn name(&self) -> &'static str;

    /// Required property names for this pattern.
    fn required_props(&self) -> &'static [&'static str];

    /// Optional property names for this pattern.
    fn optional_props(&self) -> &'static [&'static str] {
        &[]
    }

    /// Optional arguments with their default values.
    fn optional_args(&self) -> &'static [OptionalArg] {
        &[]
    }

    /// Scoped bindings to introduce during type checking.
    ///
    /// Some patterns introduce identifiers only available within certain
    /// property expressions. For example, `recurse` introduces `self` which
    /// is available in the `step` property.
    fn scoped_bindings(&self) -> &'static [ScopedBinding] {
        &[]
    }

    /// Whether this pattern allows arbitrary additional properties.
    /// Only `parallel` uses this (for dynamic task properties).
    fn allows_arbitrary_props(&self) -> bool {
        false
    }

    /// Evaluate this pattern.
    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult;

    /// Check if this pattern can be fused with the given next pattern.
    fn can_fuse_with(&self, next: &dyn PatternDefinition) -> bool {
        false
    }

    /// Create a fused pattern combining this pattern with the next one.
    fn fuse_with(
        &self,
        next: &dyn PatternDefinition,
        self_ctx: &EvalContext,
        next_ctx: &EvalContext,
    ) -> Option<FusedPattern> {
        None
    }
}
```

## Context Types

### TypeCheckContext

Provides access to property types during type checking:

```rust
pub struct TypeCheckContext<'a> {
    pub interner: &'a StringInterner,
    pub props: &'a HashMap<Name, Type>,
    // ...
}

impl TypeCheckContext<'_> {
    /// Get the type of a required property.
    pub fn get_prop_type(&self, name: &str) -> Option<Type>;

    /// Get the type of a required property, returning Error on failure.
    pub fn require_prop_type(&self, name: &str) -> Type;

    /// Create a fresh type variable.
    pub fn fresh_var(&mut self) -> Type;

    /// Extract return type from function property.
    pub fn get_function_return_type(&self, prop: &str) -> Type;

    /// Type constructors
    pub fn list_of(&self, elem: Type) -> Type;
    pub fn option_of(&self, inner: Type) -> Type;
    pub fn result_of(&self, ok: Type, err: Type) -> Type;
}
```

### EvalContext

Provides access to property expressions during evaluation:

```rust
pub struct EvalContext<'a> {
    pub props: &'a HashMap<Name, ExprId>,
    pub span: Span,
    // ...
}

impl EvalContext<'_> {
    /// Get a required property expression.
    pub fn get_prop(&self, name: &str) -> Option<ExprId>;

    /// Get an optional property expression.
    pub fn get_prop_opt(&self, name: &str) -> Option<ExprId>;

    /// Evaluate a property expression.
    pub fn eval_prop(&self, name: &str, exec: &mut dyn PatternExecutor) -> EvalResult;

    /// Evaluate with span attachment for error reporting.
    pub fn eval_prop_spanned(&self, name: &str, exec: &mut dyn PatternExecutor) -> EvalResult;

    /// Get the span of a property for error messages.
    pub fn prop_span(&self, name: &str) -> Option<Span>;

    /// Create an error with the property's span.
    pub fn error_with_prop_span(&self, msg: &str, name: &str) -> EvalError;
}
```

### PatternExecutor

Abstraction layer between patterns and the evaluator:

```rust
pub trait PatternExecutor {
    /// Evaluate an expression by ID.
    fn eval(&mut self, expr_id: ExprId) -> EvalResult;

    /// Call a function value with arguments.
    fn call(&mut self, func: &Value, args: Vec<Value>) -> EvalResult;

    /// Look up a capability by name.
    fn lookup_capability(&self, name: &str) -> Option<Value>;

    /// Call a method on a value.
    fn call_method(&mut self, receiver: Value, method: &str, args: Vec<Value>) -> EvalResult;

    /// Look up a variable by name.
    fn lookup_var(&self, name: &str) -> Option<Value>;

    /// Bind a variable in the current scope.
    fn bind_var(&mut self, name: &str, value: Value);
}
```

## Scoped Bindings

The `ScopedBinding` system enables patterns like `recurse` to introduce identifiers:

```rust
pub struct ScopedBinding {
    /// The name to introduce (e.g., "self")
    pub name: &'static str,

    /// Which properties this binding is available in
    pub for_props: &'static [&'static str],

    /// How to compute the binding's type
    pub type_from: ScopedBindingType,
}

pub enum ScopedBindingType {
    /// Same type as another property
    SameAs(&'static str),

    /// Function returning the type of a property
    FunctionReturning(&'static str),

    /// The enclosing function's type (for recursion)
    EnclosingFunction,
}
```

Example: `recurse` introduces `self` with type `(...) -> T`:

```rust
fn scoped_bindings(&self) -> &'static [ScopedBinding] {
    &[ScopedBinding {
        name: "self",
        for_props: &["step"],
        type_from: ScopedBindingType::EnclosingFunction,
    }]
}
```

## Note on Type Checking

Type checking for patterns is handled by `ori_types`, not by the patterns themselves. The `PatternDefinition` trait provides metadata (required/optional properties, scoped bindings) that `ori_types` uses to drive type inference, but the trait itself has no `type_check()` method.

## Example: Recurse Pattern

```rust
pub struct RecursePattern;

impl PatternDefinition for RecursePattern {
    fn name(&self) -> &'static str {
        "recurse"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["condition", "base", "step"]
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["memo", "parallel"]
    }

    fn scoped_bindings(&self) -> &'static [ScopedBinding] {
        &[ScopedBinding {
            name: "self",
            for_props: &["step"],
            type_from: ScopedBindingType::EnclosingFunction,
        }]
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        // Check condition
        let cond = ctx.eval_prop("condition", exec)?;
        if cond.as_bool()? {
            // Base case
            ctx.eval_prop("base", exec)
        } else {
            // Recursive case - `self` is available in step
            ctx.eval_prop("step", exec)
        }
    }
}
```

## Example: Timeout Pattern

```rust
pub struct TimeoutPattern;

impl PatternDefinition for TimeoutPattern {
    fn name(&self) -> &'static str {
        "timeout"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["op", "after"]
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let duration = ctx.eval_prop("after", exec)?.as_duration()?;

        // Evaluate with timeout
        match exec.eval_with_timeout(ctx.get_prop("op").unwrap(), duration) {
            Ok(value) => Ok(Value::ok(value)),
            Err(EvalError::Timeout) => Ok(Value::err("TimeoutError")),
            Err(e) => Err(e),
        }
    }
}
```

## Send + Sync Requirements

Patterns must be `Send + Sync` for:
- Thread-safe registry access
- Parallel compilation
- Concurrent evaluation (parallel pattern)

All patterns are zero-sized types (ZSTs) with static lifetime, so this is automatic.

## Note on map/filter/fold

These are **collection methods** in stdlib, NOT patterns. They don't require compiler support:

```ori
// These are method calls, not patterns
items.map(transform: x -> x * 2)
items.filter(predicate: x -> x > 0)
items.fold(initial: 0, op: (acc, x) -> acc + x)
```

Patterns are reserved for constructs requiring special compiler handling (recursion with `self`, concurrency, capability-aware caching, etc.).
