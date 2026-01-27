# Proposal: Remove Dot Prefix from Named Arguments

**Status:** Rejected
**Author:** Claude (with Eric)
**Created:** 2026-01-24

---

## Summary

Remove the `.` prefix from named arguments. The colon alone is sufficient to indicate a named argument.

```ori
// Before
map(
    .over: items,
    .transform: x -> x * 2,
)

// After
map(
    over: items,
    transform: x -> x * 2,
)
```

---

## Motivation

### The Dot is Redundant

The colon already unambiguously signals "named argument" in call position. There is no grammatical construct where `identifier: expression` inside a function call could mean anything else.

```ori
// The colon does all the work
fetch_user(id: 1)        // Clearly named argument
map(over: items)         // Clearly named argument

// Compare to struct literals — same pattern, different context
User { id: 1, name: "Alice" }
```

The parser knows the context. The dot adds no disambiguation.

### Ori's Other Oris Earn Their Place

| Ori | Purpose | Justification |
|-------|---------|---------------|
| `@` | Functions | Distinguishes from types/variables at declaration site |
| `$` | Config constants | Distinguishes compile-time from runtime values |
| `.` | Named args | Redundant — colon already signals this |

The `@` and `$` oris provide meaningful disambiguation. The `.` does not.

### Reduced Visual Noise

Every character should earn its place. The dot adds 1 character + mental parsing per argument:

```ori
// 48 characters of dots in a moderately complex call
parallel(
    .tasks: map(
        .over: ids,
        .transform: id -> fetch_user(.id: id),
    ),
    .max_concurrent: 10,
    .timeout: 60s,
)

// Same code, cleaner
parallel(
    tasks: map(
        over: ids,
        transform: id -> fetch_user(id: id),
    ),
    max_concurrent: 10,
    timeout: 60s,
)
```

### Familiar to Developers

`name: value` syntax for named arguments is established in:
- Swift: `greet(name: "Alice", age: 30)`
- Kotlin: `greet(name = "Alice", age = 30)`
- Python: `greet(name="Alice", age=30)`
- TypeScript/JS objects: `{ name: "Alice", age: 30 }`

The dot is Ori-specific friction with no offsetting benefit.

### Shorthand Syntax Works Naturally

With the dot removed, shorthand for "variable name matches parameter name" becomes cleaner:

```ori
let id = 42
let name = "Alice"

// Explicit
create_user(id: id, name: name)

// Shorthand (variable name = param name)
create_user(id, name)
```

This mirrors TypeScript/JavaScript object shorthand, which is well-understood.

---

## Design

### Grammar Change

**Before:**
```
named_arg := '.' IDENTIFIER ':' expression
```

**After:**
```
named_arg := IDENTIFIER ':' expression
```

### No Shorthand, No Positional

User-defined functions always require explicit `name: value` syntax. No shorthand. No positional arguments.

```ori
// Valid
fetch_user(id: 42)
fetch_user(id: some_variable)
retry(op: fetch(), attempts: 3)

// Invalid
fetch_user(42)           // No positional
fetch_user(id)           // No shorthand (even if variable named 'id' exists)
retry(fetch(), 3)        // No positional
```

**Rationale:** Shorthand like `foo(id)` meaning `foo(id: id)` appears self-documenting but isn't. The variable name `id` at the call site doesn't indicate *which* id or *what kind* of id. The explicit form `foo(id: uuid)` shows the mapping clearly.

### Built-in Conversions

Built-in type conversions remain positional (they have a single, obvious argument):

```ori
int(x)                  // Positional OK
str(value)              // Positional OK
float(n)                // Positional OK
byte(c)                 // Positional OK
```

### Call Site Summary

| Call type | Syntax | Example |
|-----------|--------|---------|
| User function | Named required | `fetch_user(id: 42)` |
| Patterns (function_exp) | Named required | `map(over: items, transform: fn)` |
| Built-in conversion | Positional allowed | `int(x)`, `str(value)` |

### Cleaner Function Names

Mandatory named parameters allow simpler function names — the parameter name carries semantic information:

```ori
// Without named params — function name encodes parameter info
fetch_user_by_id(42)
fetch_user_by_email("alice@example.com")
send_email_to_user_with_subject(user, "Hello")

// With named params — clean function names, clear call sites
fetch_user(id: 42)
fetch_user(email: "alice@example.com")
send_email(to: user, subject: "Hello")
```

The call site `fetch_user(id: 42)` reads naturally: "fetch user, id 42."

### Examples

```ori
// Pattern calls
map(
    over: items,
    transform: x -> x * 2,
)

filter(
    over: users,
    predicate: u -> u.active,
)

fold(
    over: numbers,
    init: 0,
    op: (acc, n) -> acc + n,
)

// User function calls
pub @send_email (to: str, subject: str, body: str) -> Result<void, Error> uses Email =
    Email.send(to, subject, body)

// Called with:
send_email(
    to: "alice@example.com",
    subject: "Hello",
    body: "Welcome!",
)

// Variables with different names — explicit mapping is clear
let recipient = "alice@example.com"
let title = "Hello"
let content = "Welcome!"
send_email(
    to: recipient,
    subject: title,
    body: content,
)
```

### Method Calls

Method calls follow the same pattern:

```ori
// Before
user.update(
    .name: "New Name",
    .email: "new@example.com",
)

// After
user.update(
    name: "New Name",
    email: "new@example.com",
)
```

### Struct Literals (No Change)

Struct literals already use `name: value` without a dot:

```ori
User {
    id: 1,
    name: "Alice",
    email: "alice@example.com",
}
```

This change makes function calls consistent with struct literals.

---

## Migration

### Automated Migration

The change is fully mechanical:

1. Find all `.identifier:` patterns in call expressions
2. Remove the leading `.`

A `ori fix` command or formatter update can handle this automatically.

### Example Diff

```diff
 map(
-    .over: items,
-    .transform: x -> x * 2,
+    over: items,
+    transform: x -> x * 2,
 )
```

---

## Tradeoffs

| Cost | Mitigation |
|------|------------|
| Loss of visual "rail" from aligned dots | Colons still create alignment; parameter names provide the rail |
| Slightly harder to grep for param usage | `param:` is still greppable; LSP provides "find usages" |
| Breaking change to existing code | Fully automatable migration |
| Less "distinctive" Ori syntax | Other oris (`@`, `$`) remain distinctive |

### The Visual Rail Argument

The previous proposal for always-stacking argued the dots create a "visual rail":

```ori
// Dots align
retry(
    .op: fetch(),
    .attempts: 3,
    .backoff: exponential(),
)
```

Without dots, the colons and parameter names still provide structure:

```ori
// Colons align, names provide context
retry(
    op: fetch(),
    attempts: 3,
    backoff: exponential(),
)
```

The vertical alignment is preserved. The parameter names are actually more prominent without the dot prefix competing for attention.

---

## Benefits

| Benefit | Description |
|---------|-------------|
| **Less noise** | Remove redundant character from every named argument |
| **Familiar syntax** | Matches Swift, Kotlin, struct literals |
| **Consistent** | Function calls match struct literal syntax |
| **Simpler function names** | Parameter names carry semantic info, not function names |
| **Easier typing** | One less character per argument |
| **Cleaner diffs** | Shorter lines, same semantic content |

---

## Implementation

### Compiler Changes

1. **Lexer:** No changes needed
2. **Parser:** Update `parse_call_args` to expect `IDENTIFIER ':' expr` instead of `'.' IDENTIFIER ':' expr`
3. **Parser:** Add shorthand detection — bare identifier in arg position checks against param names

### Formatter Changes

1. Remove `.` emission before named argument identifiers
2. Add shorthand collapsing (optional, could be a separate proposal)

### Files to Update

- `compiler/oric/src/parser/grammar/expr.rs` — Call argument parsing
- `compiler/oric/src/lexer.rs` — No changes expected
- `docs/ori_lang/0.1-alpha/spec/` — Update grammar and examples
- `docs/ori_lang/0.1-alpha/design/` — Update syntax documentation
- `CLAUDE.md` — Update quick reference

### Migration Tool

Add `ori migrate remove-dot-args` command to automatically update existing code.

---

## Alternatives Considered

### Keep the Dot

Status quo. Rejected because the dot provides no semantic value and adds visual noise.

### Use `=` Instead of `:`

```ori
map(over = items, transform = fn)
```

Rejected: Conflicts with assignment semantics. The `:` is well-established for key-value pairs.

### Require Dot Only for Disambiguation

Only require `.` when there's ambiguity. Rejected: There is no ambiguity, so this reduces to "never require dot."

---

## Open Questions

1. **Method chaining:** Any special considerations for chained method calls?
   - **Recommendation:** No, same rules apply uniformly.

2. **Error messages:** What should the error say for `fetch_user(42)`?
   - **Recommendation:** "Named argument required. Try: `fetch_user(.id: 42)`"

---

## Rejection Rationale

After discussion, this proposal was rejected. The dot prefix serves important purposes:

### 1. Disambiguates Parameter Names from Values

```ori
fetch_user(.id: id)
//         ^^^  ^^
//         |    └── variable name
//         └─────── parameter name (marked by dot)
```

Without the dot, `fetch_user(id: id)` has two `id`s with different roles but no visual distinction. The dot explicitly marks which is the parameter name.

### 2. Prevents Shorthand Confusion

Many languages allow `foo(id)` as shorthand for `foo(id: id)`. Without the dot:

```ori
fetch_user(id)  // Is 'id' a param name (shorthand)? Or positional variable?
```

With the dot, even potential shorthand would be unambiguous:

```ori
fetch_user(.id)  // Clearly a parameter name, even as shorthand
```

### 3. Consistent Ori Philosophy

Ori uses oris to mark different kinds of identifiers:

| Ori | Marks | Example |
|-------|-------|---------|
| `@` | Function names | `@fetch_user` |
| `$` | Config constants | `$timeout` |
| `.` | Parameter names | `.id: value` |

Each ori explicitly identifies what kind of thing follows. Removing the dot breaks this consistency.

### 4. Visual Scanning

The dots create a "rail" when stacked vertically:

```ori
retry(
    .op: fetch(),
    .attempts: 3,
    .backoff: exponential(),
)
```

The aligned dots make parameters instantly visible without reading the text.

### Conclusion

The dot is not redundant — it disambiguates parameter names from values and maintains Ori's explicit, self-documenting philosophy. The minor reduction in typing does not justify the loss of clarity.

---

## Original Proposal Summary

Remove the `.` prefix from named arguments:

- The colon already signals "named argument" — the dot is redundant
- Reduces visual noise without losing clarity
- Aligns with familiar syntax from other languages
- Enables simpler function names (parameter names carry semantic info)
- Fully automatable migration

**No shorthand syntax.** Always require explicit `name: value`. The apparent convenience of `foo(x)` meaning `foo(x: x)` is misleading — variable names at call sites don't reliably document intent. Explicit mapping like `fetch_user(id: uuid)` is always clear.

The change simplifies the language while maximizing self-documentation at call sites.
