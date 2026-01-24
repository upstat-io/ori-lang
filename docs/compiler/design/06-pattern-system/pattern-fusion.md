# Pattern Fusion

Pattern fusion is an optimization that combines multiple patterns into a single pass, eliminating intermediate data structures.

## Location

```
compiler/sigilc/src/patterns/fusion.rs (~420 lines)
```

## Motivation

Without fusion:
```sigil
// Creates intermediate list after map
filter(
    .over: map(.over: items, .transform: x -> x * 2),
    .predicate: x -> x > 10,
)
```

Execution:
1. map creates new list: `[2, 4, 6, 8, ...]`
2. filter creates another list: `[12, 14, ...]`

With fusion:
```
map → filter becomes map_filter (single pass)
```

Execution:
1. Single pass: transform and test each element
2. One output list: `[12, 14, ...]`

## Fusible Patterns

| Pattern | Can Fuse With |
|---------|---------------|
| map | map, filter |
| filter | map, filter |
| fold | (terminal) |
| find | (terminal) |

## Implementation

### FusedPattern

```rust
pub struct FusedPattern {
    /// Original patterns (in order)
    patterns: Vec<Arc<dyn PatternDefinition>>,

    /// Combined name for debugging
    name: String,
}

impl PatternDefinition for FusedPattern {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_check(
        &self,
        args: &[TypedArg],
        checker: &mut TypeChecker,
    ) -> Result<Type, TypeError> {
        // Type check flows through all patterns
        let mut current_ty = self.patterns[0].type_check(args, checker)?;

        for pattern in &self.patterns[1..] {
            // Synthesize args for intermediate pattern
            let intermediate_args = self.synthesize_args(&current_ty, pattern);
            current_ty = pattern.type_check(&intermediate_args, checker)?;
        }

        Ok(current_ty)
    }

    fn evaluate(
        &self,
        args: &[EvalArg],
        evaluator: &mut Evaluator,
    ) -> Result<Value, EvalError> {
        // Fused evaluation in single pass
        let input = args.get("over")?.as_list()?;
        let transforms = self.collect_transforms(args)?;

        let result: Vec<Value> = input
            .iter()
            .filter_map(|item| {
                self.apply_fused(&transforms, item.clone(), evaluator).ok()
            })
            .collect();

        Ok(Value::List(Arc::new(result)))
    }
}
```

### MapFilterFusion

```rust
pub struct MapFilterPattern {
    map_transform: ExprId,
    filter_predicate: ExprId,
}

impl PatternDefinition for MapFilterPattern {
    fn name(&self) -> &str {
        "map_filter"
    }

    fn evaluate(
        &self,
        args: &[EvalArg],
        evaluator: &mut Evaluator,
    ) -> Result<Value, EvalError> {
        let input = args.get("over")?.as_list()?;
        let transform = args.get("transform")?.as_function()?;
        let predicate = args.get("predicate")?.as_function()?;

        let mut result = Vec::new();

        for item in input.iter() {
            // Apply transform
            let transformed = evaluator.call_function(&transform, vec![item.clone()])?;

            // Apply filter
            let keep = evaluator.call_function(&predicate, vec![transformed.clone()])?;

            if keep.as_bool()? {
                result.push(transformed);
            }
        }

        Ok(Value::List(Arc::new(result)))
    }
}
```

### FusionOptimizer

```rust
pub struct FusionOptimizer {
    registry: SharedPatternRegistry,
}

impl FusionOptimizer {
    pub fn optimize(&self, expr: &Expr, arena: &ExprArena) -> Option<Expr> {
        match &expr.kind {
            ExprKind::Pattern { name, args } => {
                // Check if any argument is also a pattern
                for arg in args {
                    if let ExprKind::Pattern { name: inner_name, .. } = &arena.get(arg.value).kind {
                        // Try to fuse
                        if let Some(fused) = self.try_fuse(*inner_name, *name) {
                            return Some(self.create_fused_expr(fused, args, arena));
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn try_fuse(&self, inner: Name, outer: Name) -> Option<Box<dyn PatternDefinition>> {
        let inner_pattern = self.registry.get(inner)?;
        let outer_pattern = self.registry.get(outer)?;

        if inner_pattern.can_fuse(&*outer_pattern) {
            inner_pattern.fuse_with(&*outer_pattern)
        } else {
            None
        }
    }
}
```

## Fusion Rules

### map → map

```sigil
map(.over: map(.over: xs, .transform: f), .transform: g)
// Becomes:
map(.over: xs, .transform: x -> g(f(x)))
```

```rust
impl MapPattern {
    fn fuse_map(&self, other: &MapPattern) -> MapMapPattern {
        MapMapPattern {
            // Compose transforms: g ∘ f
            transform: compose(other.transform, self.transform),
        }
    }
}
```

### map → filter

```sigil
filter(.over: map(.over: xs, .transform: f), .predicate: p)
// Becomes:
map_filter(.over: xs, .transform: f, .predicate: p)
```

### filter → filter

```sigil
filter(.over: filter(.over: xs, .predicate: p1), .predicate: p2)
// Becomes:
filter(.over: xs, .predicate: x -> p1(x) && p2(x))
```

### filter → map

```sigil
map(.over: filter(.over: xs, .predicate: p), .transform: f)
// Becomes:
filter_map(.over: xs, .predicate: p, .transform: f)
```

## When Fusion Applies

Fusion is applied during:
1. **AST optimization pass** (before evaluation)
2. **Type checking** (verify fused types match)

Conditions:
- Inner pattern is direct argument to outer
- Patterns are fusible according to registry
- No side effects between patterns

## Benefits

1. **Memory** - No intermediate allocations
2. **Cache** - Single pass is cache-friendly
3. **Parallelism** - Fused operations can be parallelized

## Limitations

1. **Side effects** - Can't fuse if inner has side effects
2. **Debugging** - Fused patterns harder to debug
3. **Complexity** - Fusion logic adds compiler complexity

## Debug Mode

Disable fusion for debugging:

```bash
SIGIL_NO_FUSION=1 sigil run file.si
```

Or in code:
```rust
optimizer.set_fusion_enabled(false);
```
