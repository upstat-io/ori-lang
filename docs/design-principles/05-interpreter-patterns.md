# Interpreter/Evaluator Patterns

Quick-reference guide to tree-walking interpreter design and implementation.

---

## Interpreter Architecture Choices

### Tree-Walking Interpreter
- Directly executes the AST
- Simple to implement and understand
- Slower than bytecode (traversal overhead)
- Good for: scripting, prototypes, DSLs, learning

### Bytecode VM
- Compile AST to bytecode instructions
- Execute on virtual machine
- Faster than tree-walking
- More complex to implement
- Good for: production languages, performance-critical

### When to Choose Tree-Walking
- Rapid prototyping
- DSLs where performance isn't critical
- Educational projects
- Languages with heavy metaprogramming
- When simplicity > performance

---

## Value Representation

### Tagged Union (Recommended)
```rust
enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Rc<String>),
    Array(Rc<RefCell<Vec<Value>>>),
    Function(Rc<Function>),
    NativeFunction(NativeFn),
    Struct(Rc<RefCell<HashMap<String, Value>>>),
}
```

### Boxed Approach
```rust
enum Value {
    Nil,
    Bool(bool),
    Number(f64),
    String(Box<str>),
    Object(Box<dyn Object>),
}

trait Object {
    fn type_name(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}
```

### Host Language Mapping
```java
// Use host language types directly
// Lox value -> Java Object
// nil -> null
// bool -> Boolean
// number -> Double
// string -> String
```

### Considerations
- **Copying**: Small values (nil, bool, int) should copy cheaply
- **Sharing**: Strings, arrays, objects need reference counting or GC
- **Mutability**: Wrap mutable values in `RefCell` or similar
- **Type checking**: `match` on enum, or `instanceof` with host types

---

## Expression Evaluation

### Core Eval Pattern
```rust
fn eval(&mut self, expr: &Expr, env: &Env) -> Result<Value, Error> {
    match expr {
        Expr::Literal(lit) => self.eval_literal(lit),
        Expr::Ident(name) => self.eval_ident(name, env),
        Expr::Binary(op, left, right) => self.eval_binary(op, left, right, env),
        Expr::Unary(op, operand) => self.eval_unary(op, operand, env),
        Expr::Call(callee, args) => self.eval_call(callee, args, env),
        Expr::If(cond, then_, else_) => self.eval_if(cond, then_, else_, env),
        Expr::Block(stmts) => self.eval_block(stmts, env),
        // ...
    }
}
```

### Literal Evaluation
```rust
fn eval_literal(&self, lit: &Literal) -> Result<Value, Error> {
    Ok(match lit {
        Literal::Nil => Value::Nil,
        Literal::Bool(b) => Value::Bool(*b),
        Literal::Int(n) => Value::Int(*n),
        Literal::Float(n) => Value::Float(*n),
        Literal::String(s) => Value::String(Rc::new(s.clone())),
    })
}
```

### Binary Operations
```rust
fn eval_binary(&mut self, op: &BinOp, left: &Expr, right: &Expr, env: &Env)
    -> Result<Value, Error>
{
    let lval = self.eval(left, env)?;
    let rval = self.eval(right, env)?;

    match op {
        BinOp::Add => match (&lval, &rval) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::String(a), Value::String(b)) => {
                Ok(Value::String(Rc::new(format!("{}{}", a, b))))
            }
            _ => Err(Error::TypeMismatch("+ requires numbers or strings")),
        },
        BinOp::Sub => self.numeric_binop(lval, rval, |a, b| a - b),
        BinOp::Mul => self.numeric_binop(lval, rval, |a, b| a * b),
        BinOp::Div => {
            let (a, b) = self.expect_numbers(lval, rval)?;
            if b == 0.0 {
                Err(Error::DivisionByZero)
            } else {
                Ok(Value::Float(a / b))
            }
        }
        BinOp::Eq => Ok(Value::Bool(self.values_equal(&lval, &rval))),
        BinOp::Lt => self.compare(lval, rval, |a, b| a < b),
        // ... other operators
    }
}
```

### Short-Circuit Evaluation
```rust
fn eval_binary(&mut self, op: &BinOp, left: &Expr, right: &Expr, env: &Env)
    -> Result<Value, Error>
{
    match op {
        // Short-circuit: don't eval right if unnecessary
        BinOp::And => {
            let lval = self.eval(left, env)?;
            if !self.is_truthy(&lval) {
                Ok(lval)  // Return falsy left
            } else {
                self.eval(right, env)  // Return right
            }
        }
        BinOp::Or => {
            let lval = self.eval(left, env)?;
            if self.is_truthy(&lval) {
                Ok(lval)  // Return truthy left
            } else {
                self.eval(right, env)  // Return right
            }
        }
        _ => {
            // Eager evaluation for other ops
            let lval = self.eval(left, env)?;
            let rval = self.eval(right, env)?;
            // ...
        }
    }
}
```

### Truthiness
```rust
fn is_truthy(&self, value: &Value) -> bool {
    match value {
        Value::Nil => false,
        Value::Bool(b) => *b,
        Value::Int(0) => false,     // Optional: 0 is falsy
        Value::String(s) if s.is_empty() => false,  // Optional
        _ => true,
    }
}
```

---

## Environment/Scope Management

### Environment Structure
```rust
struct Environment {
    values: HashMap<String, Value>,
    parent: Option<Rc<RefCell<Environment>>>,
}

impl Environment {
    fn new() -> Self {
        Environment { values: HashMap::new(), parent: None }
    }

    fn with_parent(parent: Rc<RefCell<Environment>>) -> Self {
        Environment { values: HashMap::new(), parent: Some(parent) }
    }
}
```

### Variable Definition
```rust
impl Environment {
    fn define(&mut self, name: String, value: Value) {
        // Always define in current scope
        self.values.insert(name, value);
    }
}
```

### Variable Lookup (Walk Chain)
```rust
impl Environment {
    fn get(&self, name: &str) -> Result<Value, Error> {
        if let Some(value) = self.values.get(name) {
            Ok(value.clone())
        } else if let Some(parent) = &self.parent {
            parent.borrow().get(name)
        } else {
            Err(Error::UndefinedVariable(name.to_string()))
        }
    }
}
```

### Variable Assignment
```rust
impl Environment {
    fn assign(&mut self, name: &str, value: Value) -> Result<(), Error> {
        if self.values.contains_key(name) {
            self.values.insert(name.to_string(), value);
            Ok(())
        } else if let Some(parent) = &self.parent {
            parent.borrow_mut().assign(name, value)
        } else {
            Err(Error::UndefinedVariable(name.to_string()))
        }
    }
}
```

### Block Scope
```rust
fn eval_block(&mut self, stmts: &[Stmt], env: &Env) -> Result<Value, Error> {
    // Create child environment
    let child_env = Rc::new(RefCell::new(
        Environment::with_parent(Rc::clone(env))
    ));

    let mut result = Value::Nil;
    for stmt in stmts {
        result = self.exec(stmt, &child_env)?;
    }
    Ok(result)
}
```

---

## Function Implementation

### Function Value
```rust
struct Function {
    name: Option<String>,
    params: Vec<String>,
    body: Vec<Stmt>,
    closure: Rc<RefCell<Environment>>,  // Captured at definition
}

impl Function {
    fn arity(&self) -> usize {
        self.params.len()
    }
}
```

### Function Definition
```rust
fn eval_function_def(&mut self, def: &FnDef, env: &Env) -> Result<Value, Error> {
    let function = Function {
        name: Some(def.name.clone()),
        params: def.params.clone(),
        body: def.body.clone(),
        closure: Rc::clone(env),  // Capture current environment
    };
    env.borrow_mut().define(def.name.clone(), Value::Function(Rc::new(function)));
    Ok(Value::Nil)
}
```

### Function Call
```rust
fn eval_call(&mut self, callee: &Expr, args: &[Expr], env: &Env)
    -> Result<Value, Error>
{
    // Evaluate callee
    let callee_val = self.eval(callee, env)?;

    // Evaluate arguments
    let arg_vals: Vec<Value> = args
        .iter()
        .map(|a| self.eval(a, env))
        .collect::<Result<_, _>>()?;

    match callee_val {
        Value::Function(func) => self.call_function(&func, arg_vals),
        Value::NativeFunction(native) => (native.func)(self, arg_vals),
        _ => Err(Error::NotCallable),
    }
}

fn call_function(&mut self, func: &Function, args: Vec<Value>)
    -> Result<Value, Error>
{
    // Check arity
    if args.len() != func.arity() {
        return Err(Error::WrongArity(func.arity(), args.len()));
    }

    // Create new environment with closure as parent
    let call_env = Rc::new(RefCell::new(
        Environment::with_parent(Rc::clone(&func.closure))
    ));

    // Bind parameters
    for (param, arg) in func.params.iter().zip(args) {
        call_env.borrow_mut().define(param.clone(), arg);
    }

    // Execute body
    match self.exec_block(&func.body, &call_env) {
        Ok(val) => Ok(val),
        Err(Error::Return(val)) => Ok(val),  // Catch return
        Err(e) => Err(e),
    }
}
```

---

## Closure Implementation

### Closure Capture
```rust
// When function is defined, capture the current environment
fn define_function(&mut self, name: &str, params: Vec<String>, body: Vec<Stmt>, env: &Env) {
    let closure = Function {
        name: Some(name.to_string()),
        params,
        body,
        closure: Rc::clone(env),  // KEY: capture current env
    };
    // ...
}
```

### Why Closures Work
1. Function stores reference to environment where it was **defined**
2. When called, creates new env with closure as parent
3. Body can access variables from definition site
4. Even if definition site's function has returned!

### Example Closure Chain
```
outer_env:  { x: 1 }
    |
    v
closure_env: { make_adder closure }
    |
    v
adder_env:  { n: 5 }  (captured when make_adder(5) called)
    |
    v
call_env:   { m: 3 }  (when adder(3) called)
```

---

## Return Handling

### Return as Exception
```rust
// Special error type for returns
enum Error {
    Return(Value),
    RuntimeError(String),
    // ...
}

fn exec_return(&mut self, value: Option<&Expr>, env: &Env) -> Result<Value, Error> {
    let ret_val = match value {
        Some(expr) => self.eval(expr, env)?,
        None => Value::Nil,
    };
    Err(Error::Return(ret_val))  // Unwind via exception
}

// Catch in call_function
fn call_function(&mut self, func: &Function, args: Vec<Value>) -> Result<Value, Error> {
    // ... setup ...
    match self.exec_block(&func.body, &call_env) {
        Ok(val) => Ok(val),
        Err(Error::Return(val)) => Ok(val),  // Catch return
        Err(e) => Err(e),  // Propagate other errors
    }
}
```

### Alternative: Explicit Result
```rust
enum StmtResult {
    Continue,
    Return(Value),
    Break,
}

fn exec(&mut self, stmt: &Stmt, env: &Env) -> Result<StmtResult, Error> {
    match stmt {
        Stmt::Return(expr) => {
            let val = self.eval(expr, env)?;
            Ok(StmtResult::Return(val))
        }
        Stmt::Block(stmts) => {
            for stmt in stmts {
                match self.exec(stmt, env)? {
                    StmtResult::Continue => {}
                    result => return Ok(result),  // Propagate return/break
                }
            }
            Ok(StmtResult::Continue)
        }
        // ...
    }
}
```

---

## Builtin/Native Functions

### Native Function Type
```rust
type NativeFnPtr = fn(&mut Interpreter, Vec<Value>) -> Result<Value, Error>;

struct NativeFunction {
    name: String,
    arity: usize,
    func: NativeFnPtr,
}
```

### Registering Builtins
```rust
fn setup_builtins(env: &mut Environment) {
    env.define("print".to_string(), Value::NativeFunction(NativeFunction {
        name: "print".to_string(),
        arity: 1,
        func: |_, args| {
            println!("{}", format_value(&args[0]));
            Ok(Value::Nil)
        },
    }));

    env.define("clock".to_string(), Value::NativeFunction(NativeFunction {
        name: "clock".to_string(),
        arity: 0,
        func: |_, _| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64();
            Ok(Value::Float(now))
        },
    }));

    env.define("len".to_string(), Value::NativeFunction(NativeFunction {
        name: "len".to_string(),
        arity: 1,
        func: |_, args| match &args[0] {
            Value::String(s) => Ok(Value::Int(s.len() as i64)),
            Value::Array(arr) => Ok(Value::Int(arr.borrow().len() as i64)),
            _ => Err(Error::TypeError("len requires string or array")),
        },
    }));
}
```

---

## Control Flow

### If Expression
```rust
fn eval_if(&mut self, cond: &Expr, then_: &Expr, else_: &Option<Box<Expr>>, env: &Env)
    -> Result<Value, Error>
{
    let cond_val = self.eval(cond, env)?;

    if self.is_truthy(&cond_val) {
        self.eval(then_, env)
    } else if let Some(else_expr) = else_ {
        self.eval(else_expr, env)
    } else {
        Ok(Value::Nil)
    }
}
```

### While Loop
```rust
fn exec_while(&mut self, cond: &Expr, body: &Stmt, env: &Env) -> Result<Value, Error> {
    while self.is_truthy(&self.eval(cond, env)?) {
        match self.exec(body, env) {
            Ok(_) => {}
            Err(Error::Break) => break,
            Err(Error::Continue) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(Value::Nil)
}
```

### For Loop (Desugared)
```rust
// for x in iter { body }
// becomes:
// let _iter = iter.into_iter();
// while let Some(x) = _iter.next() { body }

fn exec_for(&mut self, var: &str, iter: &Expr, body: &Stmt, env: &Env)
    -> Result<Value, Error>
{
    let iter_val = self.eval(iter, env)?;
    let items = self.to_iterable(iter_val)?;

    let loop_env = Rc::new(RefCell::new(Environment::with_parent(Rc::clone(env))));

    for item in items {
        loop_env.borrow_mut().define(var.to_string(), item);
        match self.exec(body, &loop_env) {
            Ok(_) => {}
            Err(Error::Break) => break,
            Err(Error::Continue) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(Value::Nil)
}
```

---

## Pattern Matching

### Match Expression
```rust
fn eval_match(&mut self, scrutinee: &Expr, arms: &[MatchArm], env: &Env)
    -> Result<Value, Error>
{
    let value = self.eval(scrutinee, env)?;

    for arm in arms {
        if let Some(bindings) = self.match_pattern(&arm.pattern, &value)? {
            let arm_env = Rc::new(RefCell::new(Environment::with_parent(Rc::clone(env))));
            for (name, val) in bindings {
                arm_env.borrow_mut().define(name, val);
            }
            return self.eval(&arm.body, &arm_env);
        }
    }

    Err(Error::NoMatchingArm)
}

fn match_pattern(&self, pattern: &Pattern, value: &Value)
    -> Result<Option<Vec<(String, Value)>>, Error>
{
    match (pattern, value) {
        // Wildcard matches anything
        (Pattern::Wildcard, _) => Ok(Some(vec![])),

        // Variable binds the value
        (Pattern::Var(name), val) => Ok(Some(vec![(name.clone(), val.clone())])),

        // Literal must match exactly
        (Pattern::Literal(lit), val) => {
            if self.literal_matches(lit, val) {
                Ok(Some(vec![]))
            } else {
                Ok(None)
            }
        }

        // Struct/enum pattern
        (Pattern::Struct { name, fields }, Value::Struct(s)) => {
            let s = s.borrow();
            if s.type_name != *name { return Ok(None); }

            let mut bindings = vec![];
            for (field_name, field_pattern) in fields {
                if let Some(field_val) = s.fields.get(field_name) {
                    if let Some(field_bindings) = self.match_pattern(field_pattern, field_val)? {
                        bindings.extend(field_bindings);
                    } else {
                        return Ok(None);
                    }
                } else {
                    return Ok(None);
                }
            }
            Ok(Some(bindings))
        }

        _ => Ok(None),
    }
}
```

---

## Memoization

### Memoization Decorator
```rust
fn eval_memoized_call(&mut self, func: &Function, args: Vec<Value>)
    -> Result<Value, Error>
{
    // Create cache key from args
    let key = args.iter()
        .map(|v| format!("{:?}", v))
        .collect::<Vec<_>>()
        .join(",");

    // Check cache
    if let Some(cached) = func.cache.borrow().get(&key) {
        return Ok(cached.clone());
    }

    // Compute result
    let result = self.call_function_impl(func, args)?;

    // Store in cache
    func.cache.borrow_mut().insert(key, result.clone());

    Ok(result)
}
```

### Recursive Memoization
```rust
// For recursive functions, need to check cache at start of body
fn setup_recursive_memo(&mut self, func: &Function) {
    // Inject cache check at function entry
    // Or use Y-combinator style approach
}
```

---

## Recursion Handling

### Stack Overflow Protection
```rust
const MAX_STACK_DEPTH: usize = 1000;

struct Interpreter {
    call_depth: usize,
    // ...
}

fn call_function(&mut self, func: &Function, args: Vec<Value>) -> Result<Value, Error> {
    if self.call_depth >= MAX_STACK_DEPTH {
        return Err(Error::StackOverflow);
    }

    self.call_depth += 1;
    let result = self.call_function_impl(func, args);
    self.call_depth -= 1;

    result
}
```

### Tail Call Optimization (Advanced)
```rust
// Check if call is in tail position
fn is_tail_call(expr: &Expr, func_name: &str) -> bool {
    match expr {
        Expr::Call(callee, _) => {
            if let Expr::Ident(name) = callee.as_ref() {
                name == func_name
            } else {
                false
            }
        }
        Expr::If(_, then_, else_) => {
            is_tail_call(then_, func_name) &&
            else_.as_ref().map_or(true, |e| is_tail_call(e, func_name))
        }
        _ => false,
    }
}

// Trampoline-based TCO
enum Trampoline {
    Done(Value),
    Call(Function, Vec<Value>),
}

fn eval_with_tco(&mut self, expr: &Expr, env: &Env) -> Result<Value, Error> {
    let mut current = self.eval_to_trampoline(expr, env)?;
    loop {
        match current {
            Trampoline::Done(v) => return Ok(v),
            Trampoline::Call(func, args) => {
                current = self.call_to_trampoline(&func, args)?;
            }
        }
    }
}
```

---

## REPL Implementation

### Basic REPL
```rust
fn repl(&mut self) {
    let mut env = Rc::new(RefCell::new(Environment::new()));
    setup_builtins(&mut env.borrow_mut());

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }

        let line = line.trim();
        if line.is_empty() { continue; }
        if line == "exit" || line == "quit" { break; }

        match self.run(line, &env) {
            Ok(value) => {
                if !matches!(value, Value::Nil) {
                    println!("{}", format_value(&value));
                }
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
}
```

### REPL with History
```rust
fn repl_with_history(&mut self) {
    let mut rl = rustyline::DefaultEditor::new().unwrap();
    let mut env = setup_env();

    loop {
        match rl.readline("> ") {
            Ok(line) => {
                rl.add_history_entry(&line);
                // ... eval ...
            }
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("Error: {:?}", e);
                break;
            }
        }
    }
}
```

---

## Error Handling

### Runtime Error Type
```rust
enum RuntimeError {
    TypeError { expected: &'static str, got: String, span: Span },
    UndefinedVariable { name: String, span: Span },
    DivisionByZero { span: Span },
    StackOverflow,
    NotCallable { span: Span },
    WrongArity { expected: usize, got: usize, span: Span },
    IndexOutOfBounds { index: i64, len: usize, span: Span },
    // Control flow (not errors)
    Return(Value),
    Break,
    Continue,
}
```

### Error with Context
```rust
fn eval(&mut self, expr: &Expr, env: &Env) -> Result<Value, RuntimeError> {
    self.eval_inner(expr, env)
        .map_err(|e| e.with_span(expr.span()))
}
```

---

## Performance Tips

### Avoid Cloning
- Use `Rc` for shared values
- Pass references where possible
- Clone only when mutation needed

### Inline Hot Paths
```rust
// Inline common operations
#[inline]
fn is_truthy(&self, v: &Value) -> bool { ... }

#[inline]
fn add_numbers(&self, a: f64, b: f64) -> Value { ... }
```

### Pre-resolve Variable Lookups
```rust
// During semantic analysis, resolve to slot indices
struct ResolvedVar {
    depth: usize,   // How many scopes up
    index: usize,   // Slot in that scope
}

// At runtime, direct indexing instead of name lookup
fn get_var(&self, resolved: &ResolvedVar) -> Value {
    self.env_at_depth(resolved.depth)[resolved.index].clone()
}
```

---

## Interpreter Checklist

### Core
- [ ] Value representation (tagged union)
- [ ] Expression evaluation
- [ ] Statement execution
- [ ] Environment with scopes

### Control Flow
- [ ] If/else expressions
- [ ] While/for loops
- [ ] Break/continue
- [ ] Return statements

### Functions
- [ ] Function definition
- [ ] Function calls with arity check
- [ ] Closures (capture environment)
- [ ] Recursion with stack limit

### Builtins
- [ ] print/println
- [ ] Type checking (typeof, is)
- [ ] Collection operations (len, push, etc.)
- [ ] String operations

### Error Handling
- [ ] Type errors with context
- [ ] Undefined variable errors
- [ ] Stack overflow protection
- [ ] Graceful REPL error recovery

---

## Key References
- Crafting Interpreters (Tree-Walking): https://craftinginterpreters.com/evaluating-expressions.html
- Crafting Interpreters (State): https://craftinginterpreters.com/statements-and-state.html
- Crafting Interpreters (Functions): https://craftinginterpreters.com/functions.html
