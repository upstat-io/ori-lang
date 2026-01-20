# Type System Design

Quick-reference guide to type system foundations, inference, and implementation patterns.

---

## Type System Foundations

### Static vs Dynamic Typing
| Aspect | Static | Dynamic |
|--------|--------|---------|
| When checked | Compile time | Runtime |
| Type annotations | Often required | Rarely required |
| Error discovery | Before running | During execution |
| Performance | Generally faster | Runtime overhead |
| Flexibility | Less flexible | More flexible |
| Examples | Rust, Go, Java, C | Python, Ruby, JS |

### Strong vs Weak Typing
| Aspect | Strong | Weak |
|--------|--------|------|
| Implicit conversions | Few/none | Many |
| Type errors | Explicit rejection | Silent conversion |
| Safety | Higher | Lower |
| Examples | Rust, Python, Haskell | C, JavaScript, PHP |

### Nominal vs Structural Typing
| Aspect | Nominal | Structural |
|--------|---------|------------|
| Type identity | By name/declaration | By shape/structure |
| Interface satisfaction | Explicit `implements` | Implicit (matching methods) |
| Flexibility | Less (explicit) | More (duck typing) |
| Examples | Java, C#, Rust (traits) | Go, TypeScript, OCaml |

---

## Type Representation

### Core Types
```rust
enum Type {
    // Primitives
    Int,
    Float,
    Bool,
    String,
    Unit,       // void/()
    Never,      // bottom type (unreachable)

    // Compound
    Array(Box<Type>),
    Tuple(Vec<Type>),
    Function { params: Vec<Type>, ret: Box<Type> },
    Struct { name: String, fields: Vec<(String, Type)> },

    // Named/User-defined
    Named(String),

    // For inference
    Var(TypeVar),       // Unknown type variable
    Generic(String),    // Generic parameter like T

    // Error recovery
    Error,
}

type TypeVar = u32;  // Fresh ID for each unknown
```

### Type Schemes (Polymorphism)
```rust
// forall a b. a -> b -> a
struct TypeScheme {
    quantified: Vec<String>,  // ["a", "b"]
    ty: Type,                 // Function(a, b) -> a
}
```

### Type Context
```rust
struct TypeContext {
    // Variable -> Type mappings
    bindings: HashMap<String, Type>,

    // For generics: type parameter -> constraint
    type_params: HashMap<String, Vec<TypeBound>>,

    // Scoping
    parent: Option<Box<TypeContext>>,
}
```

---

## Hindley-Milner Type Inference

### Overview
- Infers most general (principal) type without annotations
- Based on unification of type constraints
- Supports parametric polymorphism (generics)
- Used by: ML, Haskell, Rust (partially), OCaml

### Algorithm W (Core Steps)

1. **Generate fresh type variables** for unknowns
2. **Traverse AST**, collecting constraints
3. **Unify** constraints to find substitution
4. **Apply substitution** to get final types
5. **Generalize** let-bound functions

### Constraint Generation
```rust
fn infer(env: &Env, expr: &Expr) -> (Type, Vec<Constraint>) {
    match expr {
        Expr::Lit(n) => (Type::Int, vec![]),

        Expr::Var(name) => {
            let ty = env.lookup(name).instantiate();  // Fresh vars for generics
            (ty, vec![])
        }

        Expr::Lambda(param, body) => {
            let param_ty = fresh_type_var();
            let new_env = env.extend(param, param_ty.clone());
            let (body_ty, constraints) = infer(&new_env, body);
            (Type::Function(param_ty, body_ty), constraints)
        }

        Expr::App(func, arg) => {
            let (func_ty, c1) = infer(env, func);
            let (arg_ty, c2) = infer(env, arg);
            let ret_ty = fresh_type_var();
            let c3 = Constraint::Eq(func_ty, Type::Function(arg_ty, ret_ty.clone()));
            (ret_ty, [c1, c2, vec![c3]].concat())
        }

        Expr::Let(name, value, body) => {
            let (value_ty, c1) = infer(env, value);
            let subst = unify(c1)?;
            let generalized = generalize(env, subst.apply(value_ty));
            let new_env = env.extend(name, generalized);
            infer(&new_env, body)
        }
    }
}
```

### Unification Algorithm
```rust
fn unify(constraints: Vec<Constraint>) -> Result<Substitution, TypeError> {
    let mut subst = Substitution::empty();

    for constraint in constraints {
        match constraint {
            Constraint::Eq(t1, t2) => {
                let t1 = subst.apply(t1);
                let t2 = subst.apply(t2);
                subst = subst.compose(unify_types(t1, t2)?);
            }
        }
    }

    Ok(subst)
}

fn unify_types(t1: Type, t2: Type) -> Result<Substitution, TypeError> {
    match (t1, t2) {
        // Same type: no substitution needed
        (Type::Int, Type::Int) => Ok(Substitution::empty()),
        (Type::Bool, Type::Bool) => Ok(Substitution::empty()),

        // Type variable: bind it
        (Type::Var(v), t) | (t, Type::Var(v)) => {
            if occurs_check(v, &t) {
                Err(TypeError::InfiniteType)
            } else {
                Ok(Substitution::single(v, t))
            }
        }

        // Function types: unify params and return
        (Type::Function(p1, r1), Type::Function(p2, r2)) => {
            let s1 = unify_types(*p1, *p2)?;
            let s2 = unify_types(s1.apply(*r1), s1.apply(*r2))?;
            Ok(s1.compose(s2))
        }

        // Mismatch
        _ => Err(TypeError::Mismatch(t1, t2)),
    }
}
```

### Occurs Check
Prevents infinite types like `a = a -> b`:
```rust
fn occurs_check(var: TypeVar, ty: &Type) -> bool {
    match ty {
        Type::Var(v) => *v == var,
        Type::Function(p, r) => occurs_check(var, p) || occurs_check(var, r),
        Type::Array(t) => occurs_check(var, t),
        _ => false,
    }
}
```

### Generalization
Convert type to scheme by quantifying free variables:
```rust
fn generalize(env: &Env, ty: Type) -> TypeScheme {
    let env_vars = env.free_type_vars();
    let ty_vars = ty.free_type_vars();
    let quantified = ty_vars.difference(&env_vars);
    TypeScheme { quantified, ty }
}
```

### Instantiation
Replace quantified variables with fresh type variables:
```rust
fn instantiate(scheme: &TypeScheme) -> Type {
    let subst: HashMap<String, Type> = scheme.quantified
        .iter()
        .map(|v| (v.clone(), fresh_type_var()))
        .collect();
    subst.apply(&scheme.ty)
}
```

---

## Bidirectional Type Checking

### Overview
- Two modes: **synthesis** (infer) and **checking** (verify)
- Type annotations flow information downward
- Simpler than full HM inference
- Handles subtyping well
- Used by: Rust, TypeScript, modern languages

### Core Pattern
```rust
// Synthesize: compute type from expression
fn synth(ctx: &Context, expr: &Expr) -> Result<Type, Error>;

// Check: verify expression has expected type
fn check(ctx: &Context, expr: &Expr, expected: &Type) -> Result<(), Error>;
```

### Implementation
```rust
fn synth(ctx: &Context, expr: &Expr) -> Result<Type, Error> {
    match expr {
        // Literals synthesize their type
        Expr::Int(_) => Ok(Type::Int),
        Expr::Bool(_) => Ok(Type::Bool),

        // Variables look up type
        Expr::Var(name) => ctx.lookup(name),

        // Annotated expressions: check against annotation
        Expr::Annotated(e, ty) => {
            check(ctx, e, ty)?;
            Ok(ty.clone())
        }

        // Function application: synth function, check arg
        Expr::App(func, arg) => {
            let func_ty = synth(ctx, func)?;
            match func_ty {
                Type::Function(param_ty, ret_ty) => {
                    check(ctx, arg, &param_ty)?;
                    Ok(*ret_ty)
                }
                _ => Err(Error::NotAFunction),
            }
        }

        // Lambda without annotation: can't synthesize
        Expr::Lambda(param, body) => Err(Error::NeedsAnnotation),
    }
}

fn check(ctx: &Context, expr: &Expr, expected: &Type) -> Result<(), Error> {
    match expr {
        // Lambda: check body with param type from expected
        Expr::Lambda(param, body) => {
            match expected {
                Type::Function(param_ty, ret_ty) => {
                    let new_ctx = ctx.extend(param, param_ty.clone());
                    check(&new_ctx, body, ret_ty)
                }
                _ => Err(Error::ExpectedFunction),
            }
        }

        // If-else: check both branches
        Expr::If(cond, then_, else_) => {
            check(ctx, cond, &Type::Bool)?;
            check(ctx, then_, expected)?;
            check(ctx, else_, expected)?;
            Ok(())
        }

        // Fall back to synthesis + subsumption
        _ => {
            let inferred = synth(ctx, expr)?;
            if subtype(&inferred, expected) {
                Ok(())
            } else {
                Err(Error::TypeMismatch(inferred, expected.clone()))
            }
        }
    }
}
```

### When to Synthesize vs Check
- **Synthesize**: Literals, variables, annotated expressions, applications
- **Check**: Lambdas without annotation, if/match arms, function bodies

### Annotations Enable Inference
```rust
// Can't infer: lambda has no annotation
let f = |x| x + 1;  // Error: can't infer type of x

// Can infer: annotation pushes type down
let f: fn(int) -> int = |x| x + 1;  // OK: x is int
```

---

## Algebraic Data Types (ADTs)

### Sum Types (Tagged Unions)
```rust
enum Option<T> {
    Some(T),
    None,
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}

enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
}
```

### Product Types (Structs/Records)
```rust
struct Point {
    x: f64,
    y: f64,
}

// Tuple struct
struct Pair<A, B>(A, B);
```

### Type Checking ADTs
```rust
fn check_match(ctx: &Context, scrutinee: &Expr, arms: &[Arm]) -> Result<Type, Error> {
    let scrut_ty = synth(ctx, scrutinee)?;

    // Get all variants of the enum
    let variants = get_variants(&scrut_ty)?;
    let mut covered = HashSet::new();

    let mut result_ty: Option<Type> = None;

    for arm in arms {
        // Check pattern matches scrutinee type
        let bindings = check_pattern(&arm.pattern, &scrut_ty)?;

        // Track coverage
        covered.insert(arm.pattern.variant_name());

        // Check body with pattern bindings
        let new_ctx = ctx.extend_many(bindings);
        let arm_ty = synth(&new_ctx, &arm.body)?;

        // All arms must have same type
        match &result_ty {
            Some(ty) => {
                if !types_equal(ty, &arm_ty) {
                    return Err(Error::ArmTypeMismatch);
                }
            }
            None => result_ty = Some(arm_ty),
        }
    }

    // Exhaustiveness check
    let uncovered: Vec<_> = variants.difference(&covered).collect();
    if !uncovered.is_empty() {
        return Err(Error::NonExhaustive(uncovered));
    }

    result_ty.ok_or(Error::EmptyMatch)
}
```

### Discriminated Unions (TypeScript Pattern)
```typescript
type State =
    | { status: 'loading' }
    | { status: 'success', data: string }
    | { status: 'error', message: string };

function handle(state: State) {
    switch (state.status) {
        case 'loading': /* state is { status: 'loading' } */ break;
        case 'success': /* state is { status: 'success', data: string } */ break;
        case 'error': /* state is { status: 'error', message: string } */ break;
    }
}
```

---

## Generics & Parametric Polymorphism

### Generic Type Definitions
```rust
struct List<T> {
    head: T,
    tail: Option<Box<List<T>>>,
}

fn map<A, B>(list: List<A>, f: fn(A) -> B) -> List<B>;
```

### Type Bounds/Constraints
```rust
// T must implement Display
fn print<T: Display>(value: T) { ... }

// Multiple bounds
fn process<T: Clone + Debug>(value: T) { ... }

// Where clause for complex bounds
fn foo<T, U>(t: T, u: U) -> i32
where
    T: Clone + Into<U>,
    U: Debug,
{ ... }
```

### Type Checking Generics
```rust
fn check_generic_call(
    ctx: &Context,
    func: &GenericFunc,
    type_args: &[Type],
    args: &[Expr],
) -> Result<Type, Error> {
    // Substitute type parameters with arguments
    let subst: HashMap<String, Type> = func.type_params
        .iter()
        .zip(type_args)
        .map(|(param, arg)| (param.name.clone(), arg.clone()))
        .collect();

    // Check bounds are satisfied
    for (param, arg) in func.type_params.iter().zip(type_args) {
        for bound in &param.bounds {
            if !satisfies_bound(ctx, arg, bound) {
                return Err(Error::BoundNotSatisfied(arg.clone(), bound.clone()));
            }
        }
    }

    // Substitute in parameter types and check arguments
    let param_types: Vec<Type> = func.params
        .iter()
        .map(|p| subst.apply(&p.ty))
        .collect();

    for (arg, expected_ty) in args.iter().zip(&param_types) {
        check(ctx, arg, expected_ty)?;
    }

    // Return substituted return type
    Ok(subst.apply(&func.ret_type))
}
```

### Monomorphization vs Erasure
| Aspect | Monomorphization | Type Erasure |
|--------|------------------|--------------|
| Strategy | Generate code for each concrete type | Single generic implementation |
| Performance | Faster (no indirection) | Slower (vtable/boxing) |
| Binary size | Larger | Smaller |
| Examples | Rust, C++ | Java, Go (pre-generics) |

---

## Traits/Interfaces

### Rust Traits
```rust
trait Display {
    fn display(&self) -> String;
}

impl Display for Point {
    fn display(&self) -> String {
        format!("({}, {})", self.x, self.y)
    }
}
```

### Go Interfaces (Structural)
```go
type Reader interface {
    Read(p []byte) (n int, err error)
}

// Any type with Read method satisfies Reader
// No explicit declaration needed
```

### Trait Checking
```rust
fn satisfies_trait(ty: &Type, trait_: &Trait, impls: &ImplTable) -> bool {
    // Look up implementations for this type
    match impls.find_impl(ty, trait_) {
        Some(impl_) => {
            // Verify all required methods are implemented
            trait_.methods.iter().all(|method| {
                impl_.has_method(&method.name) &&
                method_types_match(&impl_.get_method(&method.name), method)
            })
        }
        None => false,
    }
}
```

### Compile-Time Interface Check (Go Pattern)
```go
// Ensure MyType implements Interface at compile time
var _ Interface = (*MyType)(nil)
```

---

## Type Narrowing

### Flow-Sensitive Typing
```typescript
function process(x: string | number) {
    if (typeof x === "string") {
        // x is narrowed to string here
        console.log(x.toUpperCase());
    } else {
        // x is narrowed to number here
        console.log(x.toFixed(2));
    }
}
```

### Implementation
```rust
fn narrow_type(ty: &Type, check: &TypeCheck) -> Type {
    match (ty, check) {
        // Union narrowing: remove non-matching variants
        (Type::Union(variants), TypeCheck::TypeOf(expected)) => {
            let narrowed: Vec<Type> = variants
                .iter()
                .filter(|v| matches_typeof(v, expected))
                .cloned()
                .collect();
            if narrowed.len() == 1 {
                narrowed[0].clone()
            } else {
                Type::Union(narrowed)
            }
        }

        // Discriminated union: narrow by discriminant
        (Type::Union(variants), TypeCheck::PropertyEq(prop, value)) => {
            let narrowed: Vec<Type> = variants
                .iter()
                .filter(|v| has_property_value(v, prop, value))
                .cloned()
                .collect();
            // ...
        }

        _ => ty.clone(),
    }
}
```

### Narrowing Points
- `if`/`else` branches
- `match`/`switch` arms
- Early returns (`if !valid { return }`)
- Truthiness checks (`if x { ... }`)
- `in` operator (`if 'name' in obj`)
- `instanceof` checks

---

## Null Safety

### Option Type Approach
```rust
enum Option<T> {
    Some(T),
    None,
}

fn find(id: i32) -> Option<User> { ... }

// Must handle None case
match find(1) {
    Some(user) => println!("{}", user.name),
    None => println!("Not found"),
}
```

### Nullable Annotation Approach
```typescript
// Non-nullable by default
let name: string = "hello";

// Nullable with ?
let maybeName: string | null = null;
let optionalName?: string;  // string | undefined
```

### Type Checking Null Safety
```rust
fn check_member_access(ctx: &Context, expr: &Expr, field: &str) -> Result<Type, Error> {
    let base_ty = synth(ctx, expr)?;

    match base_ty {
        Type::Nullable(inner) => {
            // Must handle null case
            Err(Error::PossibleNull(format!(
                "Cannot access '{}' on possibly null value",
                field
            )))
        }
        _ => {
            // Safe to access
            get_field_type(&base_ty, field)
        }
    }
}
```

---

## Subtyping & Variance

### Subtyping Rules
- `Never` is subtype of everything (bottom)
- Everything is subtype of `Any` (top, if present)
- `int` is subtype of `number` (in some systems)
- Structural: `{x, y, z}` is subtype of `{x, y}`

### Variance
| Position | Variance | Rule |
|----------|----------|------|
| Return type | Covariant | Subtype can return more specific |
| Parameter | Contravariant | Subtype can accept more general |
| Mutable ref | Invariant | Must match exactly |

```rust
// Covariant in output
fn get_animal() -> Animal;
fn get_dog() -> Dog;  // Dog <: Animal, so this is valid subtype

// Contravariant in input
fn accept_dog(d: Dog);
fn accept_animal(a: Animal);  // Animal >: Dog, so this accepts more

// Invariant: &mut
fn modify_dog(d: &mut Dog);
fn modify_animal(a: &mut Animal);  // NOT compatible
```

---

## Multi-Pass Type Checking

### Pass 1: Collect Declarations
```rust
fn pass1_collect(program: &Program) -> TypeEnv {
    let mut env = TypeEnv::new();

    for decl in &program.declarations {
        match decl {
            Decl::Struct(s) => env.add_type(&s.name, struct_type(s)),
            Decl::Enum(e) => env.add_type(&e.name, enum_type(e)),
            Decl::Function(f) => env.add_func(&f.name, func_signature(f)),
        }
    }

    env
}
```

### Pass 2: Check Bodies
```rust
fn pass2_check(env: &TypeEnv, program: &Program) -> Vec<TypeError> {
    let mut errors = vec![];

    for decl in &program.declarations {
        if let Decl::Function(f) = decl {
            if let Err(e) = check_function_body(env, f) {
                errors.push(e);
            }
        }
    }

    errors
}
```

### Why Multiple Passes?
- Forward references (function A calls function B defined later)
- Mutual recursion
- Cleaner error messages (collect all errors)
- Incremental compilation opportunities

---

## Type Checker Implementation Tips

### Fresh Type Variables
```rust
struct TypeChecker {
    next_var: u32,
}

impl TypeChecker {
    fn fresh(&mut self) -> Type {
        let var = self.next_var;
        self.next_var += 1;
        Type::Var(var)
    }
}
```

### Substitution Application
```rust
impl Substitution {
    fn apply(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(v) => self.get(*v).cloned().unwrap_or(ty.clone()),
            Type::Function(p, r) => Type::Function(
                Box::new(self.apply(p)),
                Box::new(self.apply(r)),
            ),
            Type::Array(t) => Type::Array(Box::new(self.apply(t))),
            _ => ty.clone(),
        }
    }

    fn compose(self, other: Substitution) -> Substitution {
        let mut result = other;
        for (v, t) in self.0 {
            result.insert(v, result.apply(&t));
        }
        result
    }
}
```

### Error Recovery
- Don't stop at first error
- Use `Type::Error` for invalid expressions
- `Type::Error` unifies with anything (prevents cascading errors)

---

## Type Checker Checklist

### Core Features
- [ ] Primitive types (int, float, bool, string)
- [ ] Function types
- [ ] Array/list types
- [ ] Struct/record types
- [ ] Enum/union types
- [ ] Unit/void type
- [ ] Never/bottom type

### Inference
- [ ] Type variables and fresh generation
- [ ] Constraint collection
- [ ] Unification with occurs check
- [ ] Generalization (if HM-style)
- [ ] Instantiation of polymorphic types

### Features
- [ ] Generic types and functions
- [ ] Type bounds/constraints
- [ ] Null safety (Option or nullable)
- [ ] Type narrowing in conditionals
- [ ] Exhaustiveness checking for match

### Infrastructure
- [ ] Type context/environment
- [ ] Multi-pass for forward references
- [ ] Error accumulation
- [ ] Good error messages with spans

---

## Key References
- HM Type Inference: https://course.ccs.neu.edu/cs4410sp19/lec_type-inference_notes.html
- Bidirectional Type Checking: https://www.haskellforall.com/2022/06/the-appeal-of-bidirectional-type.html
- Rust Traits: https://doc.rust-lang.org/book/ch10-02-traits.html
- TypeScript Narrowing: https://www.typescriptlang.org/docs/handbook/2/narrowing.html
- Go Interfaces: https://go.dev/doc/effective_go
