# Syntax Improvements: Making Sigil Beautiful

Based on research into what developers hate about language syntax and what makes code beautiful. Decisions finalized.

---

## What People Hate (Research Summary)

| Complaint | Languages | Root Cause |
|-----------|-----------|------------|
| **Verbosity/Ceremony** | Java, COBOL | Too much boilerplate for simple tasks |
| **Inconsistency** | PHP, Perl | Multiple ways to do things, inconsistent naming |
| **Noisy Punctuation** | C++, Objective-C | Brackets, angles, symbols everywhere |
| **Hidden Behavior** | Exceptions, magic methods | Can't see what code does |
| **Repetitive Boilerplate** | Go `if err != nil` | Same pattern repeated endlessly |
| **Cryptic Symbols** | Perl, APL, Rust lifetimes | Single characters with heavy meaning |
| **Ambiguous Parsing** | C++ `<>` generics | Can't tell what syntax means |

---

## Current Sigil: What's Already Good

| Feature | Why It's Good |
|---------|---------------|
| `@` for functions | Visually distinctive, unambiguous |
| `$` for config | Clear namespace separation |
| `.name:` for pattern args | Self-documenting, no positional confusion |
| `try` pattern | Explicit error flow, no hidden exceptions |
| Expression-based | No ternary needed, clean conditionals |
| Named patterns | `fold`, `map`, `filter` read like intent |
| Context-sensitive keywords | Fewer reserved words |
| `let` / `let mut` | Explicit mutability (newly added) |

---

## Decisions

### 1. Enforce Stacked Pattern Formatting

**Decision: YES**

Patterns with 2+ properties must be stacked (one property per line):

```sigil
@sum (arr: [int]) -> int = fold(
    .over: arr,
    .init: 0,
    .op: +,
)
```

**Why:**
- Visual rhythm (Gestalt principle)
- One property per line = cleaner diffs
- Aligned `.` creates scannable column
- Trailing comma = consistent closure
- Research: aligned layouts reduce cognitive load

**Formatter rule:** Always stack pattern calls with 2+ properties.

---

### 2. Natural Line Continuation

**Decision: YES**

Lines continue naturally after binary operators:

```sigil
if a > 0
   && b > 0
   && c > 0
then true
else false
```

**Why:**
- `_` is unusual and unfamiliar
- Most languages allow continuation after binary operators
- Cleaner visual appearance
- Less "syntax noise"

**Alternative:** Use parentheses for grouping if needed:
```sigil
if (a > 0
    && b > 0
    && c > 0)
then true
else false
```

---

### 3. Generic Syntax

**Decision: Keep `<>`**

```sigil
@identity<T> (x: T) -> T = x
@map<T, U> (items: [T], f: (T) -> U) -> [U] = ...
```

**Why:**
- Familiar from Rust/TypeScript/Java
- Current grammar handles it unambiguously
- No compelling reason to change

---

### 4. Stack Long `if` Expressions

**Decision: YES**

**Short (single line OK):**
```sigil
let result = if x > 0 then "positive" else "negative"
```

**Long (formatter stacks):**
```sigil
let result = if very_long_condition_here
    then value_when_true
    else value_when_false
```

**Formatter rule:** Stack `if` expressions when they exceed line length.

---

### 5. Keep `run` Pattern

**Decision: Keep `run`**

```sigil
@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: x -> x * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: x -> x > 10,
    ),
    fold(
        .over: filtered,
        .init: 0,
        .op: +,
    ),
)
```

**Why:**
- Explicit about "sequential execution"
- Consistent with other patterns (`try`, `parallel`)
- Minimal ceremony
- Helps AI understand intent

**Not doing:** `{}` block syntax alternative.

---

### 6. Lambda Type Annotation Syntax

**Decision: YES**

Typed lambdas use `=` to separate signature from body:

```sigil
(x: int) -> int = x * 2
(a: int, b: int) -> int = a + b
```

**Why:**
- Consistent with function definitions: `@double (x: int) -> int = x * 2`
- `=` clearly separates signature from implementation

**Standard lambda (inferred types):**
```sigil
x -> x * 2
(a, b) -> a + b
```

---

### 7. `#` Length Operator

**Decision: Keep `#`, add `.first`/`.last`**

**Keep:**
```sigil
arr[# - 1]    // last element
arr[# / 2]    // middle element
```

**Add convenience accessors:**
```sigil
arr.first     // first element (Option)
arr.last      // last element (Option)
```

**Why:**
- `#` is elegant for index math once understood
- `.first`/`.last` cover the common cases clearly
- Both options available

---

### 8. Error Conversion Syntax

**Decision: Keep as-is**

```sigil
content = read_file(.path: path) | e -> AppError.Io(e)
```

**Why:**
- The `|` pipe is clear and chainable
- Reads naturally: "or if error, transform it"

**Formatter rule:** Enforce spacing: `| e -> ...` not `|e->...`

---

### 9. Trailing Commas

**Decision: YES — Enforce everywhere**

```sigil
@fetch (
    url: str,
    timeout: Duration,
    retries: int,
) -> Result<Data, Error> = retry(
    .op: http_get(.url: url),
    .attempts: retries,
    .timeout: timeout,
)
```

**Applies to:**
- Pattern arguments
- Function parameters (multi-line)
- List literals (multi-line)
- Struct fields
- Match arms

**Why:**
- Cleaner diffs (adding item = one line change)
- Visual closure (Gestalt)
- Consistent style

---

### 10. Stack Complex `where` Clauses

**Decision: YES**

**Simple (inline OK):**
```sigil
@sort<T> (items: [T]) -> [T] where T: Comparable = ...
```

**Complex (formatter stacks):**
```sigil
@complex<T, U> (a: T, b: U) -> Result<T, U>
where
    T: Serializable + Comparable,
    U: Error,
= try(
    ...
)
```

**Formatter rule:** Stack `where` clause if it exceeds line length or has 2+ constraints.

---

### 11. Require `let` for All Bindings

**Decision: YES**

All bindings require explicit `let`:

```sigil
run(
    let x = compute(),
    let y = transform(.value: x),
    y,
)
```

**Why:**
- `let` is explicit intent marker
- Distinguishes binding from reassignment
- Consistent with `let mut` for mutable bindings
- Clearer for AI code generation

---

### 12. Warn on Unused `mut`

**Decision: YES — Linter warning**

```sigil
let mut x = 5    // Warning: mutable binding 'x' is never reassigned
let y = x + 1
```

```
warning: mutable binding `x` is never reassigned
  --> src/main.si:2:5
   |
 2 |     let mut x = 5
   |         ^^^ help: consider using `let` instead
```

**Why:**
- Catches over-use of mutability
- Encourages immutable-first style
- Warning not error (developer may have intent)

---

## Things NOT to Change

### Keep `@` for Functions
- Visually distinctive (easy to scan)
- Unambiguous (LL(1) parseable)
- Consistent (always means function)

### Keep `.name:` for Pattern Arguments
- Standout feature
- Self-documenting
- No positional alternatives

### Keep `try` Pattern (Not `?` Operator)
- Research shows `?` is easy to miss
- Explicit `try` block is clearer for AI and humans

### Keep Mandatory Testing
- Core philosophy, not syntax
- Don't compromise

---

## Formatter Rules Summary

1. **Patterns with 2+ properties:** Always stack, one property per line
2. **Trailing commas:** Always in multi-line constructs
3. **Binary operators:** Allow continuation on next line (no `_` needed)
4. **`if` expressions:** Stack if exceeds line length
5. **`where` clauses:** Stack if exceeds line length or 2+ constraints
6. **Match arms:** One arm per line for 2+ arms
7. **Error conversion:** Enforce spacing `| e -> ...`
8. **Indentation:** 4 spaces, no tabs
9. **Max line length:** 100 characters
10. **Blank lines:** One between top-level definitions

---

## Linter Rules Summary

1. **Unused `mut`:** Warn when `let mut` binding is never reassigned

---

## The Result

After these changes, Sigil code looks like:

```sigil
@fetch_user_dashboard (user_id: str) -> Result<Dashboard, Error> = try(
    let user = fetch_user(.id: user_id),
    let posts = fetch_posts(.user_id: user_id),
    let notifications = fetch_notifications(.user_id: user_id),

    let stats = parallel(
        .followers: count_followers(.user_id: user_id),
        .following: count_following(.user_id: user_id),
        .posts: count_posts(.user_id: user_id),
    ),

    Ok(Dashboard {
        user: user,
        posts: posts,
        notifications: notifications,
        stats: stats,
    }),
)
```

**Qualities achieved:**
- **Rhythmic:** Consistent vertical patterns
- **Quiet:** No unnecessary noise
- **Scannable:** Aligned columns, clear boundaries
- **Inevitable:** The "obvious" way to write it
- **Honest:** What you see is what happens
- **Explicit:** `let` marks bindings, `let mut` marks mutable

---

## Action Items

### Language Changes
- [ ] Remove `_` line continuation syntax
- [ ] Change lambda type annotation from `: body` to `= body`
- [ ] Add `.first` and `.last` accessors to lists
- [ ] Require `let` for all bindings (deprecate bare `=`)

### Formatter Implementation
- [ ] Stack patterns with 2+ properties
- [ ] Stack long `if` expressions
- [ ] Stack complex `where` clauses
- [ ] Enforce trailing commas in multi-line constructs
- [ ] Enforce spacing around `|` in error conversion
- [ ] Natural line continuation after operators

### Linter Implementation
- [ ] Warn on `let mut` when never reassigned
