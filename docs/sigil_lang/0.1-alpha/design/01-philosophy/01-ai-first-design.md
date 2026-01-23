# AI-First Design Philosophy

Sigil is designed with AI-authored code as the primary optimization target. This document explains why and how this shapes the language.

---

## The Core Thesis

**AI will be the primary author of code in the future.**

Sigil is designed for AI as a first-class citizen, while remaining human-readable and writable. This isn't about making the language "AI-friendly" as an afterthought—it's about fundamentally rethinking what a programming language should optimize for.

---

## Human-First vs AI-First Design

Traditional languages optimize for human developers:

| Concern | Human-First Languages | AI-First (Sigil) |
|---------|----------------------|------------------|
| Verbosity | Minimize typing | Doesn't matter (AI types fast) |
| Consistency | Nice to have | Critical (AI learns patterns) |
| Explicitness | Can rely on context | Essential (no ambiguity) |
| Error messages | Help human debug | Help AI self-correct |
| "Magic" features | Convenient shortcuts | Avoid (unpredictable) |
| Multiple ways to do X | Flexibility | Bad (AI might pick wrong one) |
| Testing | Often skipped | Mandatory (validates AI output) |

### What AI Cares About

Traditional languages optimize for typing speed and expression brevity. AI doesn't care about typing—it generates tokens instantly. AI cares about:

1. **Correctness** - Will the code work?
2. **Predictability** - Can I reason about what it does?
3. **Verifiability** - Can I check if it's right?

Sigil optimizes for these concerns.

---

## Declarative Patterns vs Imperative Code

Traditional programming requires AI to know HOW:

```python
# Python - AI must implement memoization correctly
def fib(n, memo={}):
    if n in memo: return memo[n]
    if n <= 1: return n
    memo[n] = fib(n-1, memo) + fib(n-2, memo)
    return memo[n]
```

This is error-prone. Common mistakes:
- Forgetting the base case
- Memoizing incorrectly
- Off-by-one errors
- Thread safety issues

Sigil lets AI declare WHAT:

```sigil
@fibonacci (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true
)
```

AI doesn't need to know memoization implementation details. It just says `.memo: true`. The language guarantees correctness.

### Pattern Benefits

| Pattern | AI Declares | Language Handles |
|---------|-------------|------------------|
| `recurse` | Base case, step | Stack safety, memoization |
| `map` | Transform function | Iteration, collection |
| `filter` | Predicate | Iteration, collection |
| `fold` | Accumulator, combiner | Iteration, initial value |
| `parallel` | Concurrent tasks | Thread safety, joining |
| `retry` | Attempts, backoff | Timing, jitter, errors |
| `cache` | Key, TTL | Storage, invalidation |
| `validate` | Rules | Error accumulation |
| `timeout` | Duration | Cancellation |

---

## Explicit Over Implicit

### No Hidden Control Flow

Every execution path is visible in source code:

```sigil
// Clear: try propagates errors
@process () -> Result<Data, Error> = try(
    let data = fetch()?,      // returns Result, ? propagates error
    let parsed = parse(data)?, // returns Result, ? propagates error
    Ok(transform(parsed)),
)

// Compare to exceptions:
def process():  # Which calls throw? Who knows!
    data = fetch()
    parsed = parse(data)
    return transform(parsed)
```

### No Magic

Sigil avoids "magic" features that work through hidden mechanisms:

- No implicit conversions
- No operator overloading
- No macros that transform code
- No dependency injection
- No runtime reflection

If code doesn't look like it calls a function, it doesn't call a function.

### Explicit Dependencies

```sigil
use std.json { parse, stringify }
use std.http { get, post }

// Clear what external functionality is used
// AI scans top of file to understand dependencies
```

---

## One Way to Do Things

Multiple ways to accomplish the same thing confuse AI:

```typescript
// TypeScript - three ways to define the same thing
function add(a: number, b: number) { return a + b; }
const add = (a: number, b: number) => a + b;
const add = function(a: number, b: number) { return a + b; };
```

Sigil provides one way:

```sigil
@add (a: int, b: int) -> int = a + b
```

This applies throughout the language:
- One way to define functions
- One way to define types
- One way to handle errors
- One way to iterate collections

---

## Mandatory Testing

AI-generated code needs immediate validation:

```sigil
@factorial (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: 1,
    .step: n * self(n - 1)
)

// REQUIRED - compilation fails without this
@test_factorial tests @factorial () -> void = run(
    assert_eq(factorial(0), 1),
    assert_eq(factorial(1), 1),
    assert_eq(factorial(5), 120)
)
```

### Why Mandatory Testing Matters

1. **Immediate validation** - AI output is verified at compile time
2. **Executable specification** - Tests document expected behavior
3. **Catches edge cases** - Forces thinking about boundaries
4. **No technical debt** - "Add tests later" isn't possible

---

## Structured Error Output

Errors help AI self-correct:

```json
{
  "errors": [{
    "id": "E0308",
    "message": "mismatched types",
    "location": {
      "file": "src/main.si",
      "line": 15,
      "address": "@process.body"
    },
    "expected": "int",
    "found": "str",
    "suggestions": [{
      "message": "convert int to str",
      "edit": { "op": "set", "address": "@process.body", "value": "str(x) + \"hello\"" },
      "confidence": "high"
    }]
  }]
}
```

AI workflow:
1. Generate code
2. Compile → get structured errors
3. Apply high-confidence fixes
4. Re-compile
5. Repeat until passing

---

## Semantic Addressing

Traditional editing requires regenerating entire files. Sigil enables targeted edits:

```json
{ "op": "set", "address": "@fetch_data.attempts", "value": "5" }
```

Instead of:

```sigil
// Regenerate entire function
@fetch_data (url: str) -> Result<Data, Error> = retry(
    .op: http_get(url),
    .attempts: 5,  // changed from 3
    .backoff: exponential(base: 100ms, max: 5s)
)
```

Benefits:
- Fewer tokens
- Less chance of errors in unchanged code
- Clear intent: "changing attempts, nothing else"
- Edit command IS the diff

---

## Line-Oriented Named Arguments

Patterns require named arguments, each on its own line:

```sigil
@process (items: [int]) -> [int] = filter(
    .over: items,
    .predicate: x -> x > 0,
)
```

This is deliberately more verbose than positional alternatives. The verbosity is a feature, not a bug.

### AI Benefits

1. **Line-oriented edits** — AI can add, remove, or modify a single line without touching surrounding code. No risk of breaking syntax by miscounting commas or mismatching parentheses in dense inline expressions.

   ```sigil
   // Adding a parameter: just insert one line
   fold(
       .over: items,
       .init: 0,
       .op: +,
   +   .parallel: true,  // single line addition
   )
   ```

2. **No signature lookup required** — AI doesn't need to trace callers or read documentation to understand parameter order. `.predicate:` is obviously the filter condition; `.init:` is obviously the initial accumulator value.

3. **Reduced context for understanding** — While more tokens, the structured format reduces cognitive load. AI can scan property names without parsing complex nested expressions. Reading `fold(.over: items, .init: 0, .op: +)` requires parsing; scanning a vertical list of `.property:` names is immediate.

### Human Benefits

1. **Whitespace aids comprehension** — Research shows whitespace significantly improves human understanding. Each argument gets visual separation and breathing room.

2. **Narrow column, fast scanning** — Vertical layout creates a narrow column. Humans scan narrow columns substantially faster than wide horizontal code (this is why newspapers use columns).

3. **Zero ambiguity** — No question about argument order or meaning. Compare:
   ```sigil
   // Which is the predicate? Which is the collection?
   filter(items, x -> x > 0)

   // Unambiguous
   filter(
       .over: items,
       .predicate: x -> x > 0,
   )
   ```

4. **Self-documenting** — Code explains itself without requiring jumps to function signatures. The property names ARE the documentation.

### The Tradeoff

More verbose in raw character count:

```sigil
// 22 characters
fold(items, 0, +)

// 56 characters
fold(
    .over: items,
    .init: 0,
    .op: +,
)
```

But consider total cost:
- Reading time: **named is faster** (scan property names vs parse positions)
- Understanding: **named is instant** (no signature lookup)
- Editing: **named is safer** (line ops vs range ops)
- Code review: **named is clearer** (intent visible)
- Bug prevention: **named eliminates** argument order mistakes

The token cost of verbosity is paid once at write time. The clarity benefit is paid every time the code is read, edited, or reviewed.

---

## Immutability by Default

Mutation is the #1 source of bugs AI generates:
- "Forgot to update"
- "Updated wrong variable"
- "Order-dependent bugs"

Immutable bindings are linear—AI can trace data flow top to bottom:

```sigil
@process (data: Data) -> Data = run(
    let data = step1(data),
    let data = step2(data),  // shadowing, not mutation
    let data = step3(data),
    data,
)
```

Each `data` is a new immutable binding created with `let`. Every step is visible and debuggable.

---

## Type System Design

### Static, Explicit, with Inference

- **Static types** catch AI mistakes at compile time
- **Explicit signatures** make function contracts clear
- **Inference inside functions** reduces redundant annotations

```sigil
// Explicit at boundaries
@process (user: User) -> Result<str, Error> = run(
    let name = user.name,        // inferred: str
    let upper = name.upper(),    // inferred: str
    Ok(upper),
)
```

### No Subtyping

Types match exactly or they don't. No complex hierarchy reasoning:

```sigil
// Traditional: Is [Dog] a subtype of [Animal]?
// Sigil: Types don't have subtype relationships.

// [Dog] is [Dog]. Period.
// Want both? Be explicit:
pets: [dyn Named] = [dog, cat]
```

---

## Summary

AI-first design means:

| Principle | Implementation |
|-----------|----------------|
| Declarative | Patterns replace boilerplate |
| Explicit | No hidden control flow |
| Consistent | One way to do things |
| Verifiable | Mandatory testing |
| Addressable | Semantic edit operations |
| Line-oriented | Named args, one per line |
| Immutable | No mutation bugs |
| Static | Types catch errors early |
| Structured | JSON I/O for tooling |

Sigil doesn't make humans write code that's better for AI—it makes AI write code that's better for humans to read and maintain.

---

## See Also

- [Core Principles](02-core-principles.md)
- [Patterns Overview](../02-syntax/03-patterns-overview.md)
- [Testing](../11-testing/index.md)
