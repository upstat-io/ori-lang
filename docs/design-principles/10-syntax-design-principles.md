# Syntax Design Principles

Lessons learned from language designers (Go, Rust, Python, TypeScript, Zig, Erlang) distilled into actionable principles. Focused specifically on **syntax** decisions for AI-first code generation and parsing.

---

## 1. Parser-Friendliness

### 1.1 Leading Keywords for Every Construct

Every major construct should start with a distinctive keyword.

**Why:** Rust's `fn`, `struct`, `enum`, `impl` keywords make parsing LL(1)-friendly. You dispatch on the current token without lookahead. This helps:
- Recursive descent parsers (simple implementation)
- IDE incremental parsing (synchronize on keywords after syntax errors)
- AI code generation (clear start markers)
- Human scanning (find definitions by keyword)

```sigil
// Good: Every construct starts with a keyword
@function_name (...)   // @ signals function
type Point = { ... }   // type signals type definition
use module { ... }     // use signals import
$config = value        // $ signals config

// Bad: Ambiguous starts
Point = { x, y }       // Is this assignment or definition?
```

**Source:** [Rust's Ugly Syntax](https://matklad.github.io/2023/01/26/rusts-ugly-syntax.html) - "Every construct in Rust is introduced by a leading keyword, which makes it much easier to read the code for a human."

### 1.2 Unambiguous Grammar (LL(1) Where Possible)

Design grammar so one token of lookahead determines the parse.

**Why:** Ambiguous grammars cause:
- Parser complexity (backtracking, GLR)
- AI generation errors (wrong parse interpretation)
- Confusing error messages

```sigil
// Good: Keywords eliminate dangling else ambiguity
if condition then result_a else result_b

if condition
then result_a
else result_b

// Bad: C-style optional braces
if condition
    statement;  // Does this else attach to which if?
else
    other;
```

**Sigil's approach:** Use `then`/`else` keywords instead of braces. This makes `else if` unambiguous and prevents ["goto fail" bugs](https://dwheeler.com/essays/apple-goto-fail.html). Expression-based `if` always requires both branches.

### 1.3 Avoid Syntactic Ambiguity with Types

Don't require type information to parse.

**Why:** IDEs need to parse without running the type checker. AI generates code incrementally.

```sigil
// Good: Can parse without knowing types
foo(x, y)              // Always a function call
arr[i]                 // Always indexing
obj.field              // Always field access

// Bad: Parse depends on types (C++)
foo<T>(x)              // Is < comparison or generic?
T * x                  // Is * multiplication or pointer declaration?
```

**Go's approach:** No generics originally (until Go 1.18 with careful syntax design).

---

## 2. Consistency

### 2.1 One Obvious Way

There should be exactly one way to express common operations.

**Why (Go philosophy):** "Less is exponentially more." Multiple ways to do the same thing:
- Confuse AI model training
- Split community conventions
- Make code review harder
- Increase cognitive load

```sigil
// Good: One way to define functions
@add (a: int, b: int) -> int = a + b

// Bad: TypeScript has three
function add(a: number, b: number) { return a + b; }
const add = (a: number, b: number) => a + b;
const add = function(a: number, b: number) { return a + b; };
```

**Python's Zen:** "There should be one-- and preferably only one --obvious way to do it."

### 2.2 Similar Things Look Similar

Syntactic patterns should be visually consistent.

```sigil
// Good: All patterns use same named-property syntax
fold(
    over: arr,
    init: 0,
    op: +,
)
map(
    over: arr,
    transform: x -> x * 2,
)
filter(
    over: arr,
    predicate: x -> x > 0,
)
recurse(
    cond: n <= 1,
    base: n,
    step: ...,
)

// Bad: Inconsistent argument styles
fold(arr, 0, +)           // positional
map(arr, transform: fn)   // mixed
filter(predicate=fn, arr) // keyword first
```

### 2.3 Predictable Precedence

Follow mathematical convention. Don't invent new precedence rules.

**Why:** C's precedence mistakes persist for decades:

```c
// C bug: bitwise has lower precedence than comparison
if (flags & FLAG_MASK == FLAG_MASK)  // Parsed as: flags & (FLAG_MASK == FLAG_MASK)
```

**Sigil approach:**
- Math operators: standard precedence
- Boolean: `&&` binds tighter than `||`
- Comparison: all same level, don't chain
- When in doubt: require parentheses

---

## 3. Explicitness

### 3.1 Explicit Over Implicit

Make behavior visible in syntax.

**Python's Zen:** "Explicit is better than implicit."

```sigil
// Good: Explicit conversion
result = str(value: number) + suffix

// Bad: Implicit conversion
result = number + suffix  // Does this work? What type is result?
```

```sigil
// Good: Explicit error propagation
data = try(
    let result = fetch()?,
    Ok(result),
)

// Bad: Hidden exceptions
data = fetch()        // Might throw? Who knows!
```

### 3.2 Visible Mutability

Mutation should be syntactically obvious.

**Rust's approach:** `let` vs `let mut`

```sigil
// Good: Mutation is explicit
let x = 5           // immutable
let mut y = 5       // mutable, clearly marked

// Bad: Default mutable (most languages)
var x = 5          // Mutable? Depends on language!
```

### 3.3 No Hidden Control Flow

If it doesn't look like a function call, it shouldn't be one.

```sigil
// Good: All calls look like calls
result = compute()
formatted = obj.to_string()

// Bad: Hidden calls
result = x + y      // Actually calls operator+() method
arr[i]              // Actually calls operator[]()
```

**Exception:** Well-known operators (`+`, `-`, `==`) can desugar to trait methods, but only with standard semantics.

---

## 4. Sigils and Prefixes

### 4.1 Use Sigils for Namespacing

Sigils visually separate different kinds of names.

| Sigil | Meaning | Benefit |
|-------|---------|---------|
| `@` | Function definition | Instantly recognizable |
| `$` | Configuration | Distinguishes from variables |
| `name:` | Named argument | Can't confuse with variables |
| `_` | Unused binding | Explicit discard |

**Why sigils work:**
- "Sigils are like capital letters: both add information to an existing word without altering the word's meaning."
- They provide information to both compiler and human reader
- They enable context-sensitive keywords (keywords only special in certain contexts)

**Source:** [Raku Advent Calendar - Sigils](https://raku-advent.blog/2022/12/20/sigils/)

### 4.2 Consistent Sigil Meaning

A sigil should mean the same thing everywhere.

```sigil
// Good: @ always means function definition
@add (a: int, b: int) -> int = a + b
@main () -> void = print(msg: "hello")

// Good: $ always means config
$timeout = 30s
$max_retries = 3

// Bad: Symbol means different things
@decorator     // In Python: decorator
@variable      // In Ruby: instance variable
@"string"      // In Zig: raw string
```

---

## 5. Readability

### 5.1 Names Before Types

Put the name first, type second.

**Why (Rust analysis):** "It's more readable, because you put the most important part, the name, first."

```sigil
// Good: Name first
@calculate (amount: int, rate: float) -> float

// Less good: Type first (C-style)
float calculate(int amount, float rate)
```

**Practical benefit:** In recursive descent parsers, making the type optional is easier when it comes second.

### 5.2 Optimize for Reading, Not Writing

Code is read far more than written. AI writes instantly anyway.

**Go philosophy:** "Readable: Prioritizes reading code over writing it."

```sigil
// Good: Verbose but clear
@retry_with_backoff (
    operation: () -> Result<T, Error>,
    max_attempts: int,
    backoff_strategy: BackoffStrategy
) -> Result<T, Error>

// Bad: Terse but cryptic
@rwb<T,E>(op:()->R<T,E>,n:int,bs:BS)->R<T,E>
```

### 5.3 Context Without Syntax Highlighting

Code should be readable in error messages, diffs, and grep output.

**Rust Style Guide:** "Readability of code in contexts without syntax highlighting or IDE assistance."

```sigil
// Good: Keywords provide context
@sum (arr: [int]) -> int = fold(
    over: arr,
    init: 0,
    op: +,
)

// Bad: Relies on color to distinguish
sum = arr => arr.reduce(0, +)  // What's a keyword here?
```

---

## 6. AI-Specific Considerations

### 6.1 Tokenization Efficiency

AI models work with tokens, not characters.

**Research insight:** "Grammar tokens and formatting tokens are used to make code easier for humans to read" but add cost for AI.

**Practical tradeoffs:**
- Keywords: Clear semantically, one token each
- Punctuation: Dense but may tokenize badly
- Whitespace: Usually ignored by tokenizers

```sigil
// Reasonable: Clear keywords, minimal punctuation
@sum (arr: [int]) -> int = fold(
    over: arr,
    init: 0,
    op: +,
)

// Could be more token-efficient but less clear
sum:[int]->int=fold(arr,0,+)
```

**Decision:** Prioritize parseability and human readability over token count. AI speed is less important than AI correctness.

### 6.2 Unambiguous Error Recovery

When AI generates broken syntax, errors should point to exact location.

**Why:** AI self-correction loop:
1. Generate code
2. Parse error at line X, column Y
3. Fix that specific location
4. Retry

```
error[E001]: unexpected token
  --> src/mainsi:15:10
   |
15 |     @foo (x int) -> int = x
   |           ^^^ expected ':' before type
   |
   = help: write "x: int" not "x int"
```

### 6.3 Semantic Addressing Support

Syntax should support fine-grained references.

```sigil
// Structure enables addressing
@fetch_data (url: str) -> Result<Data, Error> = retry(
    op: http_get(url: url),        // Address: @fetch_data.retry.op
    attempts: 3,                    // Address: @fetch_data.retry.attempts
    backoff: exponential(           // Address: @fetch_data.retry.backoff
        base: 100ms,                // Address: @fetch_data.retry.backoff.base
        max: 5s,                    // Address: @fetch_data.retry.backoff.max
    ),
)
```

---

## 7. Avoiding Common Mistakes

### 7.1 Don't Overload Symbols

One symbol, one meaning.

```sigil
// Bad: << means different things (C++)
cout << value;       // Stream output
flags << 2;          // Bit shift

// Good: Different operations have different syntax
print(msg: value)       // Output
flags.shift_left(n: 2)  // Bit operations
```

### 7.2 Avoid Positional Sensitivity

Don't make meaning depend on position.

```sigil
// Bad: Trailing comma changes meaning (Python)
x = (1)    // int
x = (1,)   // tuple

// Good: Consistent syntax
x = 1                  // int
x = tuple(value: 1)   // tuple (explicit)
```

### 7.3 Don't Fight Evolution

Design syntax that can grow without breaking.

**Hejlsberg's approach:** "Anders tends to add features carefully rather than overhaul a language all at once."

**Practical:**
- Reserve sigils/keywords for future use
- Use delimiters that allow trailing commas
- Design grammar with extension points

---

## 8. Error Message Quality

### 8.1 Parse Errors Should Suggest Fixes

**Go's error philosophy:** "Readable error messages over clever implementations."

```
error[E002]: missing return type
  --> src/mainsi:5:1
   |
5  | @add (a: int, b: int) = a + b
   |                       ^ expected '->' and return type
   |
   = help: add return type: @add (a: int, b: int) -> int = a + b
```

### 8.2 Synchronize on Keywords

After an error, recover by scanning for the next keyword.

**Why (Rust insight):** "Parser resilience is easy because you can synchronize on leading keywords like `fn`, `struct`, etc."

---

## Summary: Syntax Design Checklist

Before finalizing any syntax:

- [ ] **Parseable:** Can parse with one token lookahead?
- [ ] **Unambiguous:** No parse depends on type information?
- [ ] **Consistent:** Similar constructs have similar syntax?
- [ ] **Explicit:** Can see what code does without context?
- [ ] **Readable:** Makes sense without syntax highlighting?
- [ ] **Addressable:** Can reference any part semantically?
- [ ] **Recoverable:** Can resync after syntax errors?
- [ ] **Extensible:** Room to grow without breaking changes?

---

## Key Sources

- [Go at Google: Language Design in the Service of Software Engineering](https://go.dev/talks/2012/splash.article)
- [Rust's Ugly Syntax](https://matklad.github.io/2023/01/26/rusts-ugly-syntax.html)
- [The Zen of Python (PEP 20)](https://peps.python.org/pep-0020/)
- [Zig Language Overview](https://ziglang.org/learn/overview/)
- [TypeScript's Rise in the AI Era](https://github.blog/developer-skills/programming-languages-and-frameworks/typescripts-rise-in-the-ai-era-insights-from-lead-architect-anders-hejlsberg/)
- [Sigils in Programming Languages](https://en.wikipedia.org/wiki/Sigil_(computer_programming))
- [AI Coders Are Among Us: Rethinking Grammar](https://arxiv.org/abs/2404.16333)
- [5 Mistakes in Programming Language Design](https://beza1e1.tuxen.de/articles/proglang_mistakes.html)
- [Hundred Year Mistakes (Eric Lippert)](https://ericlippert.com/2020/02/27/hundred-year-mistakes/)
