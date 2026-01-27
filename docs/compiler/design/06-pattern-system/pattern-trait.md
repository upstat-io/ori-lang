# Pattern Trait

The `PatternDefinition` trait defines the interface for all patterns in Ori.

## Trait Definition

```rust
pub trait PatternDefinition: Send + Sync {
    /// Pattern name (e.g., "map", "filter", "fold")
    fn name(&self) -> &str;

    /// Argument specifications
    fn arguments(&self) -> &[PatternArg];

    /// Type check the pattern call
    fn type_check(
        &self,
        args: &[TypedArg],
        checker: &mut TypeChecker,
    ) -> Result<Type, TypeError>;

    /// Evaluate the pattern
    fn evaluate(
        &self,
        args: &[EvalArg],
        evaluator: &mut Evaluator,
    ) -> Result<Value, EvalError>;

    /// Check if pattern can fuse with next pattern
    fn can_fuse(&self, next: &dyn PatternDefinition) -> bool {
        false
    }

    /// Fuse with following pattern (optimization)
    fn fuse_with(
        &self,
        next: &dyn PatternDefinition,
    ) -> Option<Box<dyn PatternDefinition>> {
        None
    }

    /// Capability requirements
    fn capabilities(&self) -> Vec<Capability> {
        vec![]
    }
}
```

## Argument Specification

```rust
pub struct PatternArg {
    /// Argument name (e.g., "over", "transform")
    pub name: &'static str,

    /// Expected type pattern
    pub ty: ArgType,

    /// Is this argument required?
    pub required: bool,

    /// Default value if not provided
    pub default: Option<Value>,
}

pub enum ArgType {
    /// Any type
    Any,

    /// Specific type
    Exact(Type),

    /// Generic type variable
    TypeVar(TypeVarId),

    /// List of T
    ListOf(Box<ArgType>),

    /// Function from A to B
    Function { param: Box<ArgType>, ret: Box<ArgType> },

    /// Must be iterable
    Iterable,

    /// Must be a predicate (returns bool)
    Predicate,
}
```

## Example: Map Pattern

```rust
pub struct MapPattern;

impl PatternDefinition for MapPattern {
    fn name(&self) -> &str {
        "map"
    }

    fn arguments(&self) -> &[PatternArg] {
        &[
            PatternArg {
                name: "over",
                ty: ArgType::Iterable,
                required: true,
                default: None,
            },
            PatternArg {
                name: "transform",
                ty: ArgType::Function {
                    param: Box::new(ArgType::TypeVar(TypeVarId(0))),
                    ret: Box::new(ArgType::TypeVar(TypeVarId(1))),
                },
                required: true,
                default: None,
            },
        ]
    }

    fn type_check(
        &self,
        args: &[TypedArg],
        checker: &mut TypeChecker,
    ) -> Result<Type, TypeError> {
        let over_arg = args.find("over")?;
        let transform_arg = args.find("transform")?;

        // over must be iterable
        let elem_ty = checker.extract_element_type(&over_arg.ty)?;

        // transform must be function
        let (param_ty, ret_ty) = checker.extract_function_type(&transform_arg.ty)?;

        // Parameter must match element type
        checker.unify(&param_ty, &elem_ty)?;

        // Result is list of return type
        Ok(Type::List(Box::new(ret_ty)))
    }

    fn evaluate(
        &self,
        args: &[EvalArg],
        evaluator: &mut Evaluator,
    ) -> Result<Value, EvalError> {
        let over = args.get("over")?.as_list()?;
        let transform = args.get("transform")?.as_function()?;

        let result: Vec<Value> = over
            .iter()
            .map(|item| evaluator.call_function(&transform, vec![item.clone()]))
            collect::<Result<_, _>>()?;

        Ok(Value::List(Arc::new(result)))
    }

    fn can_fuse(&self, next: &dyn PatternDefinition) -> bool {
        matches!(next.name(), "filter" | "map")
    }

    fn fuse_with(
        &self,
        next: &dyn PatternDefinition,
    ) -> Option<Box<dyn PatternDefinition>> {
        match next.name() {
            "filter" => Some(Box::new(MapFilterPattern::new(self, next))),
            "map" => Some(Box::new(MapMapPattern::new(self, next))),
            _ => None,
        }
    }
}
```

## Example: Fold Pattern

```rust
pub struct FoldPattern;

impl PatternDefinition for FoldPattern {
    fn name(&self) -> &str {
        "fold"
    }

    fn arguments(&self) -> &[PatternArg] {
        &[
            PatternArg {
                name: "over",
                ty: ArgType::Iterable,
                required: true,
                default: None,
            },
            PatternArg {
                name: "init",
                ty: ArgType::TypeVar(TypeVarId(0)),  // Accumulator type
                required: true,
                default: None,
            },
            PatternArg {
                name: "op",
                ty: ArgType::Function {
                    // (acc, elem) -> acc
                    param: Box::new(ArgType::Exact(Type::Tuple(vec![
                        Type::TypeVar(TypeVarId(0)),
                        Type::TypeVar(TypeVarId(1)),
                    ]))),
                    ret: Box::new(ArgType::TypeVar(TypeVarId(0))),
                },
                required: true,
                default: None,
            },
        ]
    }

    fn type_check(
        &self,
        args: &[TypedArg],
        checker: &mut TypeChecker,
    ) -> Result<Type, TypeError> {
        let over_ty = args.find("over")?.ty.clone();
        let init_ty = args.find("init")?.ty.clone();
        let op_ty = args.find("op")?.ty.clone();

        let elem_ty = checker.extract_element_type(&over_ty)?;
        let (param_ty, ret_ty) = checker.extract_function_type(&op_ty)?;

        // op takes (acc, elem), acc matches init
        if let Type::Tuple(params) = param_ty {
            checker.unify(&params[0], &init_ty)?;
            checker.unify(&params[1], &elem_ty)?;
        }

        // op returns acc type
        checker.unify(&ret_ty, &init_ty)?;

        Ok(init_ty)
    }

    fn evaluate(
        &self,
        args: &[EvalArg],
        evaluator: &mut Evaluator,
    ) -> Result<Value, EvalError> {
        let over = args.get("over")?.as_list()?;
        let init = args.get("init")?.clone();
        let op = args.get("op")?.as_function()?;

        let mut acc = init;
        for item in over.iter() {
            acc = evaluator.call_function(&op, vec![acc, item.clone()])?;
        }

        Ok(acc)
    }
}
```

## TypedArg and EvalArg

```rust
pub struct TypedArg {
    pub name: Name,
    pub ty: Type,
    pub span: Span,
}

impl TypedArg {
    pub fn find(args: &[Self], name: &str) -> Option<&Self> {
        args.iter().find(|a| a.name.as_str() == name)
    }
}

pub struct EvalArg {
    pub name: Name,
    pub value: Value,
}

impl EvalArg {
    pub fn as_list(&self) -> Result<&[Value], EvalError> { ... }
    pub fn as_function(&self) -> Result<&FunctionValue, EvalError> { ... }
    pub fn as_int(&self) -> Result<i64, EvalError> { ... }
}
```

## Send + Sync Requirements

Patterns must be `Send + Sync` for:
- Thread-safe registry access
- Parallel compilation
- Concurrent evaluation (parallel pattern)

```rust
// All pattern state must be thread-safe
pub struct CachePattern {
    cache: Arc<RwLock<HashMap<Value, Value>>>,
}
```
