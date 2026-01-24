# Tree Walking Interpretation

The Sigil evaluator uses tree-walking interpretation, where the AST is traversed and evaluated directly without compilation to bytecode.

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
        .collect::<Result<_, _>>()?;

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

Currently, Sigil does not implement tail call optimization. Deep recursion can cause stack overflow:

```sigil
// This will overflow for large n
@factorial (n: int) -> int =
    if n <= 1 then 1 else n * factorial(n - 1)
```

Future work: implement trampolining or continuation-passing for TCO.
