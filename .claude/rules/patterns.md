---
paths:
  - "**patterns**"
---

# Pattern System

## PatternDefinition Trait

`pub trait PatternDefinition: Send + Sync`

**Required methods:**
- `name() -> &'static str`
- `required_props() -> &'static [&'static str]`
- `evaluate(&EvalContext, &mut dyn PatternExecutor) -> EvalResult`

**Optional methods (all have defaults):**
- `optional_props() -> &'static [&'static str]` → `&[]`
- `optional_args() -> &'static [OptionalArg]` → `&[]`
- `scoped_bindings() -> &'static [ScopedBinding]` → `&[]`
- `allows_arbitrary_props() -> bool` → `false` (only `parallel` returns `true`)
- `can_fuse_with(&dyn PatternDefinition) -> bool` → `false`
- `fuse_with(next, self_ctx, next_ctx) -> Option<FusedPattern>` → `None`

**No `type_check` method.** Type-checking of patterns is done elsewhere — the trait is evaluation-only.

## Registry

- Static ZST dispatch via `Pattern` enum — no vtable, no trait objects
- `PatternRegistry` is zero-sized with private `_private: ()` to prevent external construction
- Enum dispatch on `FunctionExpKind`

**Registered patterns (15 FunctionExpKind variants → 12 Pattern structs):**

| FunctionExpKind | Pattern struct |
|---|---|
| `Recurse` | `RecursePattern` |
| `Parallel` | `ParallelPattern` |
| `Spawn` | `SpawnPattern` |
| `Timeout` | `TimeoutPattern` |
| `Cache` | `CachePattern` |
| `With` | `WithPattern` |
| `Print` | `PrintPattern` |
| `Panic` | `PanicPattern` |
| `Catch` | `CatchPattern` |
| `Todo` | `TodoPattern` |
| `Unreachable` | `UnreachablePattern` |
| `Channel`, `ChannelIn`, `ChannelOut`, `ChannelAll` | `ChannelPattern` (shared) |

## PatternExecutor
- `eval(expr_id)` — evaluate expression (legacy path — returns error; use `eval_can`)
- `call(func, args)` — call function
- `lookup_capability(name)` — get capability
- `call_method(receiver, method, args)`

## Tracing
- Target: `ori_patterns` (instrumentation in progress) | debug via `ORI_LOG=ori_eval=debug` (pattern evaluation) | `ori_types=debug` (pattern type checking)
- See compiler.md for full debugging reference

## Key Files
- `lib.rs`: PatternDefinition trait
- `registry/mod.rs`: Pattern enum, PatternRegistry, FunctionExpKind dispatch
- `recurse/`: Recurse pattern impl
- `value/`: Value types, iterators, `IteratorValue`
- `errors/`: Error factories (`wrong_arg_type`, `wrong_arg_count`)
