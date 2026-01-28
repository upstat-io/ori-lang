---
title: "Tree Walking Interpretation"
description: "Ori Compiler Design — Tree Walking Interpretation"
order: 703
section: "Evaluator"
---

# Tree Walking Interpretation

The Ori evaluator uses tree-walking interpretation, where the AST is traversed and evaluated directly without compilation to bytecode.

## How It Works

Tree-walking interpretation:
1. Receives typed AST
2. Recursively walks the tree
3. Evaluates each node, producing a Value
4. Returns final value

```rust
fn eval_expr(&mut self, id: ExprId) -> Result<Value, EvalError> {
    let expr = self.arena.get(id);

    match &expr.kind {
        ExprKind::Literal(lit) => self.eval_literal(lit),
        ExprKind::Binary { left, op, right } => {
            let left_val = self.eval_expr(*left)?;
            let right_val = self.eval_expr(*right)?;
            self.apply_binary_op(*op, left_val, right_val)
        }
        // ... more cases
    }
}
```

## Expression Evaluation

### Literals

```rust
fn eval_literal(&self, lit: &Literal) -> Result<Value, EvalError> {
    Ok(match lit {
        Literal::Int(n) => Value::Int(*n),
        Literal::Float(f) => Value::Float(*f),
        Literal::String(s) => Value::String(Arc::new(s.clone())),
        Literal::Bool(b) => Value::Bool(*b),
        Literal::Char(c) => Value::Char(*c),
        Literal::Duration(d) => Value::Duration(*d),
        Literal::Size(s) => Value::Size(*s),
    })
}
```

### Identifiers

```rust
fn eval_ident(&self, name: Name) -> Result<Value, EvalError> {
    self.env.get(name).cloned().ok_or_else(|| {
        EvalError::UndefinedVariable { name, span: self.current_span() }
    })
}
```

### Binary Operations

```rust
fn eval_binary(
    &mut self,
    left: ExprId,
    op: BinaryOp,
    right: ExprId,
) -> Result<Value, EvalError> {
    // Short-circuit for && and ||
    if op == BinaryOp::And {
        let left_val = self.eval_expr(left)?;
        if !left_val.as_bool()? {
            return Ok(Value::Bool(false));
        }
        return self.eval_expr(right);
    }

    if op == BinaryOp::Or {
        let left_val = self.eval_expr(left)?;
        if left_val.as_bool()? {
            return Ok(Value::Bool(true));
        }
        return self.eval_expr(right);
    }

    // Normal evaluation
    let left_val = self.eval_expr(left)?;
    let right_val = self.eval_expr(right)?;
    self.apply_binary_op(op, left_val, right_val)
}
```

### Function Calls

```rust
fn eval_call(
    &mut self,
    func: ExprId,
    args: &[ExprId],
) -> Result<Value, EvalError> {
    let func_val = self.eval_expr(func)?;
    let arg_vals: Vec<Value> = args
        .iter()
        .map(|&arg| self.eval_expr(arg))
        collect::<Result<_, _>>()?;

    match func_val {
        Value::Function(f) => self.call_function(&f, arg_vals),
        Value::Builtin(b) => self.call_builtin(&b, arg_vals),
        _ => Err(EvalError::NotCallable(func_val)),
    }
}

fn call_function(
    &mut self,
    func: &FunctionValue,
    args: Vec<Value>,
) -> Result<Value, EvalError> {
    // Create new scope with captured environment
    self.env.push_scope_with(func.captured_env.clone());

    // Bind parameters
    for (param, arg) in func.params.iter().zip(args) {
        self.env.bind(*param, arg);
    }

    // Evaluate body
    let result = self.eval_expr(func.body);

    self.env.pop_scope();
    result
}
```

### Let Bindings and Pattern Destructuring

Let bindings support destructuring patterns:

```rust
fn eval_let(
    &mut self,
    pattern: &BindingPattern,
    init: ExprId,
) -> Result<Value, EvalError> {
    let value = self.eval_expr(init)?;
    self.bind_pattern(pattern, &value)?;
    Ok(value)
}

fn bind_pattern(
    &mut self,
    pattern: &BindingPattern,
    value: &Value,
) -> Result<(), EvalError> {
    match pattern {
        // Simple binding: let x = value
        BindingPattern::Name(name) => {
            self.env.bind(*name, value.clone());
        }

        // Wildcard: let _ = value (discard)
        BindingPattern::Wildcard => {}

        // Tuple: let (a, b) = pair
        BindingPattern::Tuple(patterns) => {
            let values = value.as_tuple()?;
            for (pat, val) in patterns.iter().zip(values) {
                self.bind_pattern(pat, val)?;
            }
        }

        // Struct: let { x, y } = point
        BindingPattern::Struct { fields } => {
            let struct_val = value.as_struct()?;
            for (field_name, inner_pattern) in fields {
                let field_val = struct_val.get(field_name)?;
                if let Some(inner) = inner_pattern {
                    // Rename: let { x: px } = point
                    self.bind_pattern(inner, field_val)?;
                } else {
                    // Shorthand: let { x } = point
                    self.env.bind(*field_name, field_val.clone());
                }
            }
        }

        // List: let [a, b, ..rest] = items
        BindingPattern::List { elements, rest } => {
            let list = value.as_list()?;
            for (i, pat) in elements.iter().enumerate() {
                self.bind_pattern(pat, &list[i])?;
            }
            if let Some(rest_name) = rest {
                let rest_list = list[elements.len()..].to_vec();
                self.env.bind(*rest_name, Value::List(Arc::new(rest_list)));
            }
        }
    }
    Ok(())
}
```

### Control Flow

```rust
fn eval_if(
    &mut self,
    cond: ExprId,
    then: ExprId,
    else_: Option<ExprId>,
) -> Result<Value, EvalError> {
    let cond_val = self.eval_expr(cond)?;

    if cond_val.as_bool()? {
        self.eval_expr(then)
    } else if let Some(else_expr) = else_ {
        self.eval_expr(else_expr)
    } else {
        Ok(Value::Void)
    }
}

fn eval_for(
    &mut self,
    var: Name,
    iter: ExprId,
    body: ExprId,
    is_yield: bool,
) -> Result<Value, EvalError> {
    let iter_val = self.eval_expr(iter)?;
    let items = iter_val.as_iterable()?;

    if is_yield {
        // for..yield collects results
        let mut results = Vec::new();
        for item in items {
            self.env.push_scope();
            self.env.bind(var, item);
            results.push(self.eval_expr(body)?);
            self.env.pop_scope();
        }
        Ok(Value::List(Arc::new(results)))
    } else {
        // for..do executes for side effects
        for item in items {
            self.env.push_scope();
            self.env.bind(var, item);
            self.eval_expr(body)?;
            self.env.pop_scope();
        }
        Ok(Value::Void)
    }
}
```

### Match Expressions

```rust
fn eval_match(
    &mut self,
    scrutinee: ExprId,
    arms: &[MatchArm],
) -> Result<Value, EvalError> {
    let value = self.eval_expr(scrutinee)?;

    for arm in arms {
        if let Some(bindings) = self.match_pattern(&arm.pattern, &value)? {
            self.env.push_scope();
            for (name, val) in bindings {
                self.env.bind(name, val);
            }

            // Check guard if present
            if let Some(guard) = arm.guard {
                let guard_val = self.eval_expr(guard)?;
                if !guard_val.as_bool()? {
                    self.env.pop_scope();
                    continue;
                }
            }

            let result = self.eval_expr(arm.body);
            self.env.pop_scope();
            return result;
        }
    }

    Err(EvalError::NonExhaustiveMatch { value, span: self.current_span() })
}
```

### Pattern Matching for Variants

The `try_match` function handles variant patterns with any number of fields:

```rust
fn try_match(
    pattern: &MatchPattern,
    value: &Value,
) -> Result<Option<Vec<(Name, Value)>>, EvalError> {
    match (pattern, value) {
        // Unit variant: pattern matches if names match and no inner patterns
        (MatchPattern::Variant { name, inner }, Value::Variant { name: vn, fields })
            if name == vn && inner.is_empty() && fields.is_empty() =>
        {
            Ok(Some(vec![]))
        }

        // Multi-field variant: match each inner pattern against corresponding field
        (MatchPattern::Variant { name, inner }, Value::Variant { name: vn, fields })
            if name == vn && inner.len() == fields.len() =>
        {
            let mut all_bindings = Vec::new();
            for (pat, val) in inner.iter().zip(fields.iter()) {
                match try_match(pat, val)? {
                    Some(bindings) => all_bindings.extend(bindings),
                    None => return Ok(None),
                }
            }
            Ok(Some(all_bindings))
        }

        // ... other pattern cases
    }
}
```

**Key Design Decision:** The AST uses `Vec<MatchPattern>` for variant inner patterns (not `Option<Box<MatchPattern>>`), enabling:
- Unit variants: `None` → `inner: []`
- Single-field: `Some(x)` → `inner: [Binding("x")]`
- Multi-field: `Click(x, y)` → `inner: [Binding("x"), Binding("y")]`

**Variant vs Binding Disambiguation:** Uppercase pattern names are treated as variant constructors, lowercase as bindings:
- `Some` → variant pattern (matches `Value::Variant { name: "Some", ... }`)
- `x` → binding pattern (binds value to `x`)
```

## Advantages of Tree-Walking

1. **Simple implementation** - Direct mapping from AST to execution
2. **Good error messages** - Source spans available at runtime
3. **Easy debugging** - Can inspect state at any point
4. **No compilation step** - Immediate execution

## Disadvantages

1. **Slower than bytecode** - Interpretation overhead
2. **Memory overhead** - AST in memory during execution
3. **No optimizations** - Limited optimization opportunities

## Performance Considerations

Tree-walking is sufficient for:
- Small to medium programs
- Development and testing
- REPL interactions

For production, consider:
- JIT compilation
- Bytecode VM
- Ahead-of-time compilation

## Tail Call Optimization

Currently, Ori does not implement tail call optimization. Deep recursion can cause stack overflow:

```ori
// This will overflow for large n
@factorial (n: int) -> int =
    if n <= 1 then 1 else n * factorial(n - 1)
```

Future work: implement trampolining or continuation-passing for TCO.
