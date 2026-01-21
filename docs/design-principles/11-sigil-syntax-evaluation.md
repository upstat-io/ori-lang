# Sigil Syntax Evaluation

Evaluation of Sigil's current design against the syntax principles in `10-syntax-design-principles.md`.

**Rating Scale:** Pass | Partial | Needs Review | Fail

---

## 1. Parser-Friendliness

### 1.1 Leading Keywords for Every Construct

**Rating: Pass**

Sigil uses distinctive leading markers for all major constructs:

| Construct | Marker | Example |
|-----------|--------|---------|
| Functions | `@` | `@add (a: int, b: int) -> int = ...` |
| Config | `$` | `$timeout = 30s` |
| Types | `type` | `type Point = { x: int, y: int }` |
| Imports | `use` | `use std.math { sqrt }` |
| Tests | `@...tests` | `@test_add tests @add () -> void = ...` |
| Traits | `trait` | `trait Eq { ... }` |
| Impl | `impl` | `impl Eq for Point { ... }` |
| Visibility | `pub` | `pub @add ...` |

The `@` sigil for functions is particularly strong - it's visually distinctive and enables LL(1) dispatch immediately.

### 1.2 Unambiguous Grammar (LL(1) Where Possible)

**Rating: Pass**

From the grammar reference, Sigil is largely LL(1):

- **No dangling else:** `if condition then expr else expr` - always requires both branches
- **Clear expression boundaries:** Patterns use `(...)`, blocks use `run(...)`
- **Unambiguous generics:** `<T>` appears only after identifiers in specific positions

**Potential concern:** The `for` expression has two forms (imperative `for x in xs yield/do` and pattern `for(.over: ...)`) - but these are syntactically distinct via the leading paren.

### 1.3 Avoid Syntactic Ambiguity with Types

**Rating: Pass**

Sigil can parse without type information:

- Function calls: `foo(x, y)` - always a call
- Indexing: `arr[i]` - always indexing
- Field access: `obj.field` - always field access
- Generics: `@identity<T>` - `<` after `@ident` is unambiguously generics

No C++ style `T * x` ambiguity or `foo<T>(x)` parse confusion.

---

## 2. Consistency

### 2.1 One Obvious Way

**Rating: Pass**

Sigil enforces singular approaches:

| Operation | One Way |
|-----------|---------|
| Define function | `@name (params) -> type = expr` |
| Define type | `type Name = ...` |
| Handle errors | `Result<T, E>` + `try` pattern |
| Iterate | `for x in xs` or pattern equivalents |
| Conditional | `if cond then a else b` |

No TypeScript-style "three ways to define functions" problem.

### 2.2 Similar Things Look Similar

**Rating: Pass**

Pattern syntax is remarkably consistent:

```sigil
fold(
    .over: arr,
    .init: 0,
    .op: +,
)
map(
    .over: arr,
    .transform: x -> x * 2,
)
filter(
    .over: arr,
    .predicate: x -> x > 0,
)
recurse(
    .cond: n <= 1,
    .base: n,
    .step: ...,
)
retry(
    .op: fetch(),
    .attempts: 3,
    .backoff: ...,
)
parallel(
    .a: task_a(),
    .b: task_b(),
)
```

All patterns use `.name:` property syntax exclusively - no mixing of positional/keyword arguments.

### 2.3 Predictable Precedence

**Rating: Pass**

Precedence follows mathematical convention:

1. Unary: `!`, `-`
2. Multiplicative: `*`, `/`, `%`, `div`
3. Additive: `+`, `-`
4. Comparison: `<`, `>`, `<=`, `>=`
5. Equality: `==`, `!=`
6. Logical: `&&` before `||`

No surprises. The `??` coalesce operator is lowest, which is intuitive.

---

## 3. Explicitness

### 3.1 Explicit Over Implicit

**Rating: Pass**

Sigil strongly favors explicitness:

- **No implicit conversions:** `str(.value: number)` required
- **Explicit error propagation:** `try` pattern makes it visible
- **Explicit imports:** No automatic imports except prelude
- **Explicit mutability:** Default immutable with visible rebinding

From the docs: "If code doesn't look like it calls a function, it shouldn't call a function."

### 3.2 Visible Mutability

**Rating: Partial**

Variables are immutable by default, and rebinding uses shadowing:

```sigil
@process (data: Data) -> Data = run(
    data = step1(data),
    data = step2(data),  // shadowing, not mutation
    data
)
```

**Concern:** No explicit `let mut` vs `let` distinction documented. The shadowing approach is clean but some might prefer more explicit mutability markers like Rust's `let mut`.

### 3.3 No Hidden Control Flow

**Rating: Pass**

- `try` pattern explicitly shows error propagation
- No exceptions with hidden throws
- `match` requires exhaustive handling
- Operators don't hide arbitrary function calls (standard semantics only)

---

## 4. Sigils and Prefixes

### 4.1 Use Sigils for Namespacing

**Rating: Pass**

Sigil uses sigils effectively:

| Sigil | Meaning | Consistency |
|-------|---------|-------------|
| `@` | Function definition | Always |
| `$` | Config variable | Always |
| `.name:` | Named pattern argument | Always in patterns |
| `_` | Unused binding | Standard convention |
| `#` | Length in index context | Context-specific |

The `.name:` prefix for pattern properties is particularly clever - it's visually distinct and can't be confused with regular variables.

### 4.2 Consistent Sigil Meaning

**Rating: Pass**

- `@` always means function definition (never used for decorators, instance variables, etc.)
- `$` always means config (never used for variables, interpolation, etc.)
- `.name:` always means named pattern argument

No symbol overloading across different contexts.

---

## 5. Readability

### 5.1 Names Before Types

**Rating: Pass**

```sigil
@calculate (amount: int, rate: float) -> float
//          ^^^^^^ name first, then type
```

Follows the Rust/TypeScript convention of `name: Type` rather than C's `Type name`.

### 5.2 Optimize for Reading, Not Writing

**Rating: Pass**

Sigil favors clarity over brevity:

```sigil
@retry_with_backoff (
    operation: () -> Result<T, Error>,
    max_attempts: int,
    backoff_strategy: BackoffStrategy
) -> Result<T, Error>
```

Named properties (`.over:`, `.init:`, `.transform:`) are more verbose than positional args but much clearer.

### 5.3 Context Without Syntax Highlighting

**Rating: Pass**

```sigil
@sum (arr: [int]) -> int = fold(
    .over: arr,
    .init: 0,
    .op: +,
)
```

In plain text:
- `@sum` - clearly a function
- `(arr: [int])` - clearly parameters with types
- `-> int` - clearly return type
- `fold(...)` - clearly a pattern call
- `.over:`, `.init:`, `.op:` - clearly named arguments

The `@` sigil and `.name:` syntax work without color.

---

## 6. AI-Specific Considerations

### 6.1 Tokenization Efficiency

**Rating: Pass**

Sigil prioritizes clarity over token efficiency, which is the correct trade-off:

- Keywords are clear single tokens
- Named properties explicit but redundant for humans (fine for AI)
- No cryptic abbreviations

### 6.2 Unambiguous Error Recovery

**Rating: Pass**

From the grammar, parser can synchronize on:
- `@` (next function)
- `type` (next type definition)
- `use` (next import)
- `pub` (next public item)

Error messages show exact locations with suggestions.

### 6.3 Semantic Addressing Support

**Rating: Pass**

Pattern syntax naturally supports addressing:

```sigil
@fetch_data (url: str) -> Result<Data, Error> = retry(
    .op: http_get(.url: url),        // @fetch_data.retry.op
    .attempts: 3,                    // @fetch_data.retry.attempts
    .backoff: exponential(           // @fetch_data.retry.backoff
        .base: 100ms,                // @fetch_data.retry.backoff.base
    ),
)
```

The AI-first design doc explicitly mentions this as a feature.

---

## 7. Avoiding Common Mistakes

### 7.1 Don't Overload Symbols

**Rating: Pass**

No symbol overloading observed:
- `+` is always addition (strings concatenate with `+` but same semantics)
- `<` is always less-than (generics use `<T>` but only in type position)
- `->` is always "produces/returns" (function types, lambdas, match arms)

### 7.2 Avoid Positional Sensitivity

**Rating: Pass**

No Python-style `(1)` vs `(1,)` tuple ambiguity. Tuples are explicit:

```sigil
x = (1, 2)        // tuple
y = Point { x: 1 } // struct
```

Trailing commas allowed but don't change meaning.

### 7.3 Don't Fight Evolution

**Rating: Pass**

Design allows growth:
- Context-sensitive keywords (`map`, `filter`, `fold`) can be identifiers elsewhere
- Trailing commas allowed in lists, patterns
- Derive syntax `#[derive(...)]` extensible
- Pattern syntax extensible (new patterns can be added)

---

## 8. Error Message Quality

### 8.1 Parse Errors Should Suggest Fixes

**Rating: Pass (by design)**

From the docs, errors follow this format:

```
error[E0308]: mismatched types
  --> src/main.si:15:10
   |
15 |     result = x + "hello"
   |              ^ expected int, found str
   |
   = help: try: str(.value: x) + "hello"
```

### 8.2 Synchronize on Keywords

**Rating: Pass**

Grammar designed for keyword-based recovery. All major constructs start with distinctive markers.

---

## Summary Scorecard

| Principle | Rating | Notes |
|-----------|--------|-------|
| **1.1** Leading Keywords | Pass | `@`, `$`, `type`, `use` |
| **1.2** Unambiguous Grammar | Pass | LL(1) friendly |
| **1.3** Parse Without Types | Pass | No C++ ambiguities |
| **2.1** One Obvious Way | Pass | Single syntax for each operation |
| **2.2** Similar Things Look Similar | Pass | Consistent `.name:` pattern syntax |
| **2.3** Predictable Precedence | Pass | Standard math precedence |
| **3.1** Explicit Over Implicit | Pass | No implicit conversions |
| **3.2** Visible Mutability | Partial | Shadowing works but no `mut` keyword |
| **3.3** No Hidden Control Flow | Pass | `try` is explicit |
| **4.1** Use Sigils | Pass | `@`, `$`, `.name:` |
| **4.2** Consistent Sigil Meaning | Pass | No overloading |
| **5.1** Names Before Types | Pass | `name: Type` syntax |
| **5.2** Reading Over Writing | Pass | Verbose but clear |
| **5.3** Works Without Highlighting | Pass | Sigils provide context |
| **6.1** Tokenization Efficiency | Pass | Clarity over brevity |
| **6.2** Error Recovery | Pass | Keyword synchronization |
| **6.3** Semantic Addressing | Pass | Explicit design goal |
| **7.1** No Symbol Overload | Pass | Clean symbol usage |
| **7.2** No Positional Sensitivity | Pass | No trailing comma issues |
| **7.3** Evolution-Friendly | Pass | Context-sensitive keywords |
| **8.1** Helpful Errors | Pass | Suggestions included |
| **8.2** Keyword Sync | Pass | Parser-friendly grammar |

**Overall: 21 Pass, 1 Partial, 0 Needs Review, 0 Fail**

---

## Recommendations

### Minor Improvements

1. **Mutability clarity:** Consider whether to add explicit `mut` syntax or document the shadowing-based approach more prominently. Current design is valid but could be more explicit.

2. **Line continuation:** The `_` for line continuation is unusual. Consider whether this adds value over just allowing expressions to span lines naturally (like JavaScript/Rust).

3. **Length operator:** `#` inside brackets (`arr[# - 1]`) is clever but might be confusing. Consider whether `arr.len - 1` or `arr[len - 1]` would be clearer, or document this prominently.

### Strengths to Preserve

1. **Pattern property syntax** (`.name:`) - This is excellent. Clear, unambiguous, self-documenting.

2. **`@` sigil for functions** - Instantly recognizable, enables LL(1) parsing.

3. **Context-sensitive keywords** - Allows `map`, `filter`, `fold` as both pattern names and identifiers without conflict.

4. **Mandatory named arguments for patterns** - Prevents positional confusion in complex patterns.

5. **Expression-based design** - `if`/`match` as expressions eliminates ternary operator need.

---

## Conclusion

Sigil's syntax design is remarkably well-aligned with established syntax design principles. It demonstrates strong awareness of parser-friendliness, consistency, and AI-first considerations. The few areas marked "Partial" are design choices rather than problems - reasonable people could disagree on whether explicit `mut` keywords are necessary when shadowing achieves immutability.

The pattern system with `.name:` property syntax is a standout design decision that other languages could learn from.
