---
paths:
  - "**patterns**"
---

# Pattern System

## PatternDefinition Trait
**Required:**
- `name() -> &'static str`
- `required_props() -> &'static [&'static str]`
- `type_check(&mut TypeCheckContext) -> Type`
- `evaluate(&EvalContext, &mut dyn PatternExecutor) -> EvalResult`

**Optional:** `optional_props()` | `scoped_bindings()` | `can_fuse_with()`

## Registry
- Static ZST instances: `static RECURSE: RecursePattern = RecursePattern;`
- Enum dispatch on `FunctionExpKind`
- Patterns: Recurse, Parallel, Spawn, Timeout, Cache, With, Print, Panic, Catch

## PatternExecutor
- `eval(expr_id)` -- evaluate expression
- `call(func, args)` -- call function
- `lookup_capability(name)` -- get capability
- `call_method(receiver, method, args)`

## Adding New Pattern
1. Create `ori_patterns/src/<name>.rs` | implement `PatternDefinition`
2. Register in `registry.rs` | add `FunctionExpKind` variant
3. Add parsing in `ori_parse`

## Tracing
- Target: `ori_patterns` (instrumentation in progress) | debug via `ORI_LOG=ori_eval=debug` (pattern evaluation) | `ori_types=debug` (pattern type checking)
- See compiler.md for full debugging reference

## Key Files
- `lib.rs`: PatternDefinition trait
- `registry/`: Pattern lookup dispatch
- `recurse/`: Recurse pattern impl
- `value/`: Value types, iterators, `IteratorValue`
- `errors/`: Error factories (`wrong_arg_type`, `wrong_arg_count`)
