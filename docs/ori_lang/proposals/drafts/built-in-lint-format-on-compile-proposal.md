# Proposal: Built-in Linter and Format-on-Compile

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-02-18
**Affects:** Compiler pipeline, diagnostics, `ori fmt`, `ori check`, CLI, error codes

---

## Summary

Ori eliminates external linting tools and formatting debates by building both directly into the compiler:

1. **Format-on-compile**: Every `ori` command that reads source files auto-formats them on disk. Style violations are impossible because the compiler normalizes source text before proceeding. Zero configuration, zero options.

2. **Built-in lint rules**: Semantic and structural rules are compiler errors — not warnings, not suggestions. The compiler rejects code that violates any rule. There are **no escape hatches** — no `@allow`, no lint config files, no suppression mechanism of any kind.

This design is purpose-built for AI-authored code: the AI receives deterministic, non-negotiable rules from the compiler and always complies.

---

## Motivation

### The External Linter Problem

Every mainstream language bolts linting on as an afterthought:

| Language | Linter | Config Files | Escape Hatches |
|----------|--------|-------------|----------------|
| Rust | Clippy | `clippy.toml` | `#[allow(clippy::...)]` |
| JavaScript | ESLint | `.eslintrc.*` | `// eslint-disable-next-line` |
| Python | Pylint/Ruff | `pyproject.toml` | `# noqa`, `# pylint: disable=` |
| Go | `go vet` | none | none (but limited rules) |
| TypeScript | ESLint | `.eslintrc.*` | `// @ts-ignore`, `// eslint-disable` |

Problems with this model:

1. **Configuration drift**: Every project has different lint rules. AI agents must learn each project's config.
2. **Escape hatch abuse**: `#[allow]` and `// eslint-disable` become reflex rather than exception.
3. **Separate tooling**: Separate install, separate CI step, separate error format, separate documentation.
4. **Warning fatigue**: Warnings are ignored. Developers disable noisy rules rather than fix code.
5. **AI-hostile**: An AI code generator must parse lint configs, understand suppression syntax, and decide which warnings matter. This is wasted complexity.

### The Formatting Problem

Formatting tools face the same fragmentation:

| Language | Formatter | Config | Options |
|----------|-----------|--------|---------|
| Rust | `rustfmt` | `rustfmt.toml` | ~60 options |
| JavaScript | Prettier | `.prettierrc` | ~30 options |
| Python | Black | `pyproject.toml` | 5 options (but still 5 too many) |
| Go | `gofmt` | none | none |

Go got closest with `gofmt` (zero options), but it's still a separate tool you must remember to run.

### The Ori Way

Ori takes the Go philosophy to its logical conclusion:

- **Formatting is a compiler phase**, not a separate tool. Every `ori` command auto-formats source files. There is nothing to configure, nothing to run separately, nothing to forget.
- **Lint rules are compiler errors**, not warnings. There is no warning level. If the compiler flags it, you fix it. There is no suppression mechanism.
- **AI agents receive one signal**: compile or don't. No ambiguity, no configuration, no negotiation.

---

## Design

### Part 1: Format-on-Compile

#### Compilation Pipeline

The existing pipeline:

```
Source → Lex → Parse → Type Check → [Eval/Codegen]
```

Becomes:

```
Source → Lex → Parse → Auto-Format (write back) → Type Check → Lint → [Eval/Codegen]
```

Auto-formatting occurs after parsing (requires a valid AST) and before type checking. If the canonical formatted text differs from the source text on disk, the file is silently rewritten.

#### Which Commands Auto-Format

| Command | Auto-Formats | Rationale |
|---------|-------------|-----------|
| `ori check` | Yes | Full compilation pipeline |
| `ori check --no-test` | Yes | Still compiles |
| `ori run` | Yes | Compiles then runs |
| `ori build` | Yes | Compiles then codegens |
| `ori test` | Yes | Compiles then tests |
| `ori fmt` | Yes (format only) | Explicit format without type check |
| `ori fmt --check` | No (read-only) | CI gate — exit 1 if any file would change |

`ori fmt` is retained as a convenience for formatting without running the full type checker. It is functionally equivalent to the format phase of `ori check`, just without subsequent phases.

`ori fmt --check` is the only read-only formatting command. It exists solely for CI pipelines that need to verify formatting without modifying files.

#### What This Means

- **Style violations cannot exist in checked code.** The compiler normalizes style before it even considers types.
- **No `.editorconfig`, no formatter config, no style debates.** The canonical format is defined by the compiler and applied unconditionally.
- **AI agents emit whatever formatting they want.** The compiler fixes it on the next command. Zero tokens wasted on style.
- **`ori fmt --diff` and `ori fmt --stdin` remain.** These are useful for editor integration and inspection.

#### Options Removed

The following `ori fmt` options become unnecessary and are removed:

| Option | Reason |
|--------|--------|
| (none to remove) | `--check`, `--diff`, `--stdin`, `--no-ignore` all remain |

No formatting options have ever been exposed (line width, indent style, etc.), and none ever will be. The formatter is zero-config by spec (§16 Formatting).

### Part 2: Built-in Lint System

#### Philosophy

1. **Lints are errors.** There is no warning severity for lint rules. A lint violation prevents compilation.
2. **No escape hatches.** There is no `@allow`, no `@suppress`, no `#ignore`, no config file, no CLI flag to disable individual rules. The compiler's rules are absolute.
3. **Zero false positives or the rule doesn't ship.** Because there is no suppression mechanism, every rule must be precise enough that every firing represents a genuine problem. A rule with false positives is a broken rule.
4. **Deterministic.** Given the same source, the same lints fire. No heuristics, no ML, no "sometimes."
5. **Actionable.** Every lint error includes a specific fix suggestion. The compiler tells you what's wrong AND how to fix it.

#### Error Code Range

Lint errors use the **E7xxx** range, with subcategories:

| Range | Category | Description |
|-------|----------|-------------|
| E70xx | Correctness | Code that is almost certainly wrong |
| E71xx | Naming | Identifier naming convention violations |
| E72xx | Complexity | Code that exceeds structural thresholds |
| E73xx | Clarity | Code that is unnecessarily hard to understand |
| E74xx | Performance | Patterns with known better alternatives |

#### When Lints Run

Lints run as a dedicated pass **after type checking**, before evaluation or codegen. This gives lints access to:

- The full typed AST (types of all expressions resolved)
- Scope information (which bindings are used where)
- Trait and method resolution results
- Import resolution

```
Source → Lex → Parse → Auto-Format → Type Check → **Lint** → [Eval/Codegen]
```

Some simple lints (naming conventions) could technically run earlier, but placing all lints in a single post-type-check pass keeps the architecture clean: one phase, one set of rules, one diagnostic pass.

---

## Lint Rules

### Correctness (E70xx)

These rules catch code that is almost certainly a bug or dead code.

#### E7001 — Unused import

An imported name is never referenced in the module.

```ori
use std.math { sqrt, abs }  // E7001: unused import `abs`

@distance (x: float, y: float) -> float = sqrt(x * x + y * y)
```

**Fix:** Remove the unused import.

```ori
use std.math { sqrt }
```

#### E7002 — Unused variable

A `let` binding is never referenced after its definition. Variables prefixed with `_` are exempt (the `_` prefix signals intentional disuse).

```ori
@process (input: str) -> int = {
    let temp = parse(input: input);  // E7002: unused variable `temp`
    let result = compute();
    result
}
```

**Fix:** Use the variable, remove it, or prefix with `_` if the binding is needed for a side effect.

```ori
let _temp = parse(input: input)  // OK: intentionally unused
```

#### E7003 — Unused function parameter

A function parameter is never referenced in the function body. Parameters prefixed with `_` are exempt.

```ori
@greet (name: str, age: int) -> str =  // E7003: unused parameter `age`
    "Hello, " + name
```

**Fix:** Use the parameter, remove it from the signature, or prefix with `_`.

#### E7004 — Unused private function

A private (non-`pub`) function is never called from anywhere in the module.

```ori
@helper () -> int = 42  // E7004: unused private function `helper`

pub @main () -> void = print(msg: "hello")
```

**Fix:** Use it, make it `pub` if it's part of the API, or remove it.

#### E7005 — Unused private type

A private (non-`pub`) type definition is never referenced.

```ori
type TempData = { value: int }  // E7005: unused private type `TempData`
```

**Fix:** Use it, export it, or remove it.

#### E7006 — Comparison to self

Comparing a value to itself is always `true` (for `==`, `>=`, `<=`) or always `false` (for `!=`, `<`, `>`).

```ori
if x == x then "yes" else "no"  // E7006: comparison of `x` to itself is always true
```

**Fix:** Compare to the intended other value, or remove the condition.

#### E7007 — Unreachable code

Code after an expression of type `Never` (after `panic`, `todo`, `unreachable`, `break`) can never execute.

```ori
@fail () -> int = {
    panic(msg: "abort");
    42  // E7007: unreachable code — previous expression has type `Never`
}
```

**Fix:** Remove the unreachable code.

#### E7008 — Discarded non-void result

A function returning a non-`void` value is called in a position where its result is unused. This catches accidentally ignoring error returns, computation results, or other meaningful values.

```ori
@process () -> void = {
    compute_important_value();  // E7008: result of type `int` is discarded
    print(msg: "done")
}
```

**Fix:** Bind the result with `let`, or use `let _ = expr` if intentionally discarding.

```ori
let _ = compute_important_value()  // OK: explicitly discarded
```

**Exemptions:** Functions returning `void` are naturally exempt. Functions called as the last expression in a block (where the result IS the block value) are exempt.

#### E7009 — Duplicate map key

A map literal contains the same key more than once.

```ori
let m = {
    "name": "Alice",
    "age": 30,
    "name": "Bob",  // E7009: duplicate map key `"name"`
}
```

**Fix:** Remove the duplicate entry.

#### E7010 — Duplicate match pattern

A match arm has a pattern identical to a previous arm, making it unreachable.

```ori
match x {
    1 -> "one"
    2 -> "two"
    1 -> "uno"  // E7010: duplicate match pattern `1` — arm is unreachable
}
```

**Fix:** Remove the duplicate arm or change the pattern.

#### E7011 — Double negation

`!!x` is equivalent to `x` when `x` is `bool`.

```ori
let valid = !!is_ready()  // E7011: double negation — simplify to `is_ready()`
```

**Fix:** Remove the double negation.

#### E7012 — Negated boolean literal

`!true` is `false` and `!false` is `true`.

```ori
let flag = !true  // E7012: negated boolean literal — use `false`
```

**Fix:** Use the literal directly.

#### E7013 — Redundant else on Never

An `else` branch after a `then` branch that returns `Never` is redundant — the else branch always executes when the condition is false.

```ori
if x < 0 then panic(msg: "negative")
else compute(x: x)  // OK but: could simplify
```

This is informational only — actually, since this pattern is idiomatic for guard clauses, this rule is **not included**. Guard clauses using `if cond then panic(...) else expr` are a valid and common pattern.

#### E7014 — Infinite iterator consumed without bound

An iterator with no natural termination (e.g., `repeat`) is consumed by a greedy operation (e.g., `collect`, `fold` without `take`).

```ori
let items = repeat(value: 1).collect()  // E7014: infinite iterator consumed by `collect`
```

**Fix:** Add a bound: `.take(n: 100).collect()`.

*Note: This promotes the existing W2001 warning to a hard error.*

---

### Naming (E71xx)

These rules enforce Ori's naming conventions. They are checked lexically (no type information needed, but run in the lint pass for consistency).

#### E7101 — Type name must be PascalCase

Type definitions (`type`, `trait`) must use PascalCase.

```ori
type user_data = { name: str }  // E7101: type name `user_data` must be PascalCase
type HTTPClient = { ... }       // E7101: acronym `HTTP` — use `HttpClient`
```

**Acronym rule:** Acronyms of 2+ characters are treated as words: `Http`, `Json`, `Url`, `Api`, not `HTTP`, `JSON`, `URL`, `API`. Single-letter acronyms stay uppercase: `T`, `E`.

**Fix:** Rename to `UserData`, `HttpClient`.

#### E7102 — Function name must be snake_case

Function names (after `@`) must use snake_case.

```ori
@processData () -> void = ...  // E7102: function name `processData` must be snake_case
@GetUser () -> User = ...      // E7102: function name `GetUser` must be snake_case
```

**Fix:** Rename to `@process_data`, `@get_user`.

#### E7103 — Variable name must be snake_case

Variable bindings (`let`) must use snake_case.

```ori
let userName = "Alice"  // E7103: variable `userName` must be snake_case
let X = 10              // E7103: variable `X` must be snake_case
```

**Single-character exception:** Single lowercase letters (`x`, `y`, `n`, `i`, etc.) are valid snake_case. Single uppercase letters are not (they look like type parameters).

**Fix:** Rename to `user_name`, `x`.

#### E7104 — Constant name must be snake_case

Module-level constants (`let $`) must use snake_case.

```ori
let $MaxRetries = 3     // E7104: constant `MaxRetries` must be snake_case
let $API_TIMEOUT = 30s  // E7104: constant `API_TIMEOUT` must be snake_case
```

**Fix:** Rename to `$max_retries`, `$api_timeout`. Ori does not use SCREAMING_CASE for constants — the `$` prefix already distinguishes them.

#### E7105 — Type parameter must be PascalCase

Type parameters must be uppercase single letters or PascalCase words.

```ori
@identity<t> (x: t) -> t = x          // E7105: type parameter `t` must be PascalCase
@convert<input> (x: input) -> str = x  // E7105: type parameter `input` must be PascalCase
```

**Fix:** Rename to `T`, `Input`.

#### E7106 — Sum type variant must be PascalCase

Sum type variant names must be PascalCase.

```ori
type Color = red | green | blue  // E7106: variant `red` must be PascalCase
```

**Fix:** Rename to `Red | Green | Blue`.

#### E7107 — Module name must be snake_case

Module names (derived from file names) must be snake_case.

```ori
// File: MyModule.ori → E7107: module name `MyModule` must be snake_case
```

**Fix:** Rename the file to `my_module.ori`.

#### E7108 — Field name must be snake_case

Struct field names must be snake_case.

```ori
type Point = { xCoord: int, yCoord: int }  // E7108: field `xCoord` must be snake_case
```

**Fix:** Rename to `x_coord`, `y_coord`.

#### E7109 — Predicate function should use standard prefix

Functions returning `bool` should use a standard predicate prefix: `is_`, `has_`, `can_`, `should_`, or `needs_`.

```ori
@valid (input: str) -> bool = ...    // E7109: boolean function `valid` — use `is_valid`
@permission (user: User) -> bool = ...  // E7109: boolean function `permission` — use `has_permission`
```

**Exemptions:**
- Test functions (annotated with `@test`)
- Functions named with comparison-style verbs: `equals`, `contains`, `matches`, `starts_with`, `ends_with`
- Lambdas (unnamed)

**Fix:** Rename with appropriate prefix.

---

### Complexity (E72xx)

These rules enforce structural limits that keep code decomposed and reviewable. The primary metric is **cognitive complexity** — a measure of how hard code is to *understand*, not just how many paths exist.

#### E7201 — Cognitive complexity too high

A function's cognitive complexity score exceeds **15**. This metric (based on SonarSource's model, adopted by Clippy as `cognitive_complexity`) measures how difficult a function is to understand by weighting nesting depth, not just control flow branching.

**Scoring rules:**

| Construct | Increment | Nesting penalty |
|-----------|-----------|-----------------|
| `if` / `else` | +1 | +1 per nesting level |
| `for` | +1 | +1 per nesting level |
| `match` | +1 | +1 per nesting level |
| `loop` | +1 | +1 per nesting level |
| `try` | +1 | +1 per nesting level |
| `&&` / `\|\|` sequences | +1 per switch between operators | — |
| `break` with value | +1 | — |
| `recurse` (recursion) | +1 | — |
| Lambda body | — | +1 nesting level (no base increment) |

**Key principle:** A construct at the top level of a function costs 1, but the same construct nested inside a `for` inside a `match` costs 1 + its nesting depth. This is what makes cognitive complexity superior to cyclomatic complexity — it captures the exponential readability cost of nesting.

**Example — score 4 (OK):**

```ori
@categorize (value: int) -> str =         // +0
    if value > 100 then "large"            // +1 (if)
    else if value > 10 then "medium"       // +1 (else-if, no nesting penalty — chained)
    else if value > 0 then "small"         // +1 (else-if, chained)
    else "zero"                            // +1 (else)
                                           // Total: 4
```

**Example — score 18 (E7201):**

```ori
@process (items: [Item]) -> [Result] = {
    for item in items do                   // +1 (for)
        if item.active then                // +2 (if, +1 nesting from for)
            match item.kind {              // +3 (match, +2 nesting from for+if)
                Kind.A ->
                    if item.priority > 5   // +4 (if, +3 nesting)
                    then handle_a(item)
                    else skip()
                Kind.B ->
                    for sub in item.parts do  // +4 (for, +3 nesting)
                        if sub.valid then     // +5 (if, +4 nesting)
                            process_sub(sub)
            }
}
// E7201: function `process` has cognitive complexity 19 (max 15)
```

**Why 15?** This threshold is well-established:
- SonarSource's default is 15 for most languages
- Clippy's `cognitive_complexity` default is 25 (more lenient — Rust has `match` everywhere)
- Ori's threshold of 15 matches the SonarSource recommendation because Ori functions should be short (expression-based, no `return` for early exit)

**What does NOT count:**
- Linear sequences of `let` bindings (no branching = no complexity)
- `pre()` / `post()` (conditions, but structurally flat)
- Trait method dispatch (the compiler handles this, not the programmer)

**Fix:** Extract nested logic into helper functions. Each extraction removes nesting levels, dramatically reducing the score.

```ori
// Before: score 19
@process (items: [Item]) -> [Result] = {
    for item in items do
        if item.active then process_item(item: item)
}

// After: extracted helpers, each with low individual score
@process_item (item: Item) -> Result =
    match item.kind {
        Kind.A -> handle_a(item: item)
        Kind.B -> process_parts(parts: item.parts)
    }

@process_parts (parts: [SubItem]) -> Result =
    for sub in parts do
        if sub.valid then process_sub(sub: sub)
```

#### E7202 — Too many function parameters

A function has more than **5** parameters.

```ori
@send (             // E7202: function `send` has 6 parameters (max 5)
    to: str,
    from: str,
    subject: str,
    body: str,
    cc: [str],
    priority: int,
) -> void = ...
```

**Exemptions:**
- Functions where all parameters beyond the 5th have default values

**Fix:** Group related parameters into a config struct.

```ori
type EmailConfig = { to: str, from: str, subject: str, body: str, cc: [str], priority: int }
@send (config: EmailConfig) -> void = ...
```

#### E7203 — Too many match arms

A `match` expression has more than **15** arms.

```ori
match code {       // E7203: match has 20 arms (max 15) — consider lookup table or decomposition
    1 -> "one"
    2 -> "two"
    ... // 18 more
}
```

**Exemptions:**
- Match on sum types where the type itself has >15 variants (the match must be exhaustive)

**Fix:** Use a map lookup, decompose into sub-matches by category, or restructure the type.

---

### Clarity (E73xx)

These rules catch code that works correctly but is unnecessarily hard to understand.

#### E7301 — Shadowed binding

A `let` binding in an inner scope has the same name as a binding in an outer scope, creating confusion about which binding is referenced.

```ori
@process (x: int) -> int = {
    let x = x + 1;  // E7301: `x` shadows parameter `x` from outer scope
    x * 2
}
```

**Exemptions:**
- Shadowing with `_`-prefixed names (intentional discard)
- Match arm bindings that destructure (e.g., `Some(x) -> x` when outer `x` exists is allowed because the match context makes the scope unambiguous)

**Fix:** Use a different name for the inner binding.

```ori
let incremented = x + 1
```

#### E7302 — Boolean function parameter

A function parameter has type `bool`. Boolean parameters create cryptic call sites (`process(x: data, validate: true)` — what does `true` mean?).

```ori
@fetch (url: str, retry: bool) -> str = ...
// E7302: boolean parameter `retry` — use an enum for clarity

// Call site is unclear:
fetch(url: "/api", retry: true)
```

**Exemptions:**
- Functions with exactly 1 parameter of type `bool` (the function itself is a predicate-like operation)
- Test functions
- Private functions within the same module where usage is co-located

**Fix:** Define an enum.

```ori
type RetryPolicy = Retry | NoRetry

@fetch (url: str, retry: RetryPolicy) -> str = ...
fetch(url: "/api", retry: Retry)  // clear at call site
```

#### E7303 — Magic number

A numeric literal appears in an expression where its meaning is not self-evident. Constants should be used to give numbers meaningful names.

```ori
@calculate_price (base: float) -> float =
    base * 1.0825  // E7303: magic number `1.0825` — extract to named constant
```

**What counts as a magic number:**
- Any `int` literal other than `-1`, `0`, `1`, `2`
- Any `float` literal other than `0.0`, `1.0`, `-1.0`, `0.5`

**What does NOT count:**
- Literals in constant definitions (`let $tax_rate = 1.0825`)
- Literals in test assertions (`assert_eq(actual: result, expected: 42)`)
- Literals in range expressions (`0..10`, `1..=100`)
- Literals as collection sizes or capacities (`List.with_capacity(size: 16)`)
- Literals in `take`/`skip`/`chunk` calls
- Literals as array/tuple indices
- Duration/size literals (`5s`, `1mb`) — the unit provides context

**Fix:** Extract to a named constant.

```ori
let $tax_rate = 1.0825

@calculate_price (base: float) -> float =
    base * $tax_rate
```

#### E7304 — Deeply nested conditional

More than **3 levels** of `if`/`then`/`else` nesting without using `match`. Deeply nested conditionals are hard to follow; `match` or guard functions are clearer.

This rule is separate from E7201 (cognitive complexity) because nested conditionals are disproportionately confusing even when the overall function complexity is low. A function with a single 4-deep `if` chain and nothing else might score only 10 on cognitive complexity (below the threshold), but the nested conditional is still hard to read.

```ori
let result =
    if a then
        if b then           // 2
            if c then        // 3
                if d then    // 4 → E7304: conditional nesting depth 4 exceeds maximum of 3
                    x
                else y
            else z
        else w
    else v
```

**Note:** Chained `else if` does NOT count as nesting — it is a flat sequence:

```ori
// This is fine — chained, not nested:
let result = if a then x
    else if b then y
    else if c then z
    else w
```

**Fix:** Convert to `match` or extract conditions into named predicates.

```ori
let result = match true {
    _ if a && b && c && d -> x
    _ if a && b && c -> y
    _ if a && b -> z
    _ if a -> w
    _ -> v
}
```

---

### Performance (E74xx)

These rules catch patterns with known better alternatives. Each rule fires only when the improvement is unambiguous.

#### E7401 — Collect then iterate

Calling `.collect()` immediately followed by `.iter()` or another iterator method creates an unnecessary intermediate collection.

```ori
let result = items
    .filter(x -> x > 0)
    .collect()              // E7401: unnecessary collect — chain iterator operations directly
    .map(x -> x * 2)
```

**Fix:** Remove the `.collect()` and chain directly.

```ori
let result = items.filter(x -> x > 0).map(x -> x * 2)
```

#### E7402 — O(n²) list contains in loop

Using `.contains()` on a list inside a `for` loop creates O(n²) behavior. A `Set` provides O(1) lookups.

```ori
for item in items do
    if other_items.contains(value: item) then  // E7402: `.contains` in loop is O(n²) — use a Set
        process(item: item)
```

**Fix:** Convert the lookup target to a `Set` before the loop.

```ori
let other_set = Set.from(items: other_items)
for item in items do
    if other_set.contains(value: item) then
        process(item: item)
```

---

## Escape Hatch Policy

**There are no escape hatches.**

This is not an oversight — it is the central design decision. The implications:

### Why No Escape Hatches

1. **Escape hatches become the norm.** In every language with `#[allow]` or `// eslint-disable`, suppression is used far more than fixing. Entire teams add blanket `#[allow(unused)]` to their files. The escape hatch defeats the linter.

2. **AI agents will always use escape hatches.** Given a choice between restructuring code and adding `@allow(E7201)`, an AI agent will choose the annotation every time. It's fewer tokens, lower risk, and the tests still pass. The only way to ensure AI agents write well-structured code is to make ill-structured code a compilation error with no workaround.

3. **Zero false positives makes escape hatches unnecessary.** Every rule in this proposal fires only on genuine problems. If a rule causes a false positive, the rule is wrong and must be fixed — not worked around with suppression.

4. **The compiler is the single source of truth.** When there are no exceptions, every Ori codebase has exactly the same quality bar. There is nothing to configure, nothing to negotiate, nothing to debate.

### What If a Rule Is Wrong?

If a lint rule fires incorrectly (false positive), the correct fix is to **fix the rule in the compiler**, not to add suppression. This creates strong incentive to keep rules precise. A rule that cannot be made precise enough should not exist.

### The `_` Prefix Is Not an Escape Hatch

The `_` prefix for unused bindings (E7002, E7003) is a **naming convention**, not suppression. It changes the variable's name to communicate intent ("I know this is unused"). The lint rule for unused variables explicitly excludes `_`-prefixed names by definition — it's part of the rule, not an exception to it.

Similarly, `let _ = expr` for E7008 (discarded results) is an **explicit discard expression** — a language construct that says "I am intentionally ignoring this value." It is semantically meaningful, not suppression.

---

## `--strict` Flag

The current `ori check --strict` flag is removed. All checks always run at maximum strictness. There is no relaxed mode.

If `--strict` currently gates behavior that would break existing code, those checks are either:
- Promoted to always-on (if they catch real problems)
- Removed (if they are noise)

---

## Implementation

### Compiler Pipeline Integration

The lint pass is a new phase in the compilation pipeline:

```
Source → Lex → Parse → Format (write back) → Type Check → **Lint** → Eval/Codegen
```

Architecturally:
- New crate: `ori_lint` (or lint module within `ori_types`)
- Input: Typed AST + scope information from type checker
- Output: Diagnostics accumulated via existing `ori_diagnostic` infrastructure
- No separate binary, no separate configuration

### Error Code Registration

Add E7xxx variants to `ErrorCode` enum in `ori_diagnostic/src/error_code/mod.rs`:

```rust
// Lint Errors (E7xxx)
// Correctness (E70xx)
E7001, // Unused import
E7002, // Unused variable
E7003, // Unused function parameter
E7004, // Unused private function
E7005, // Unused private type
E7006, // Comparison to self
E7007, // Unreachable code
E7008, // Discarded non-void result
E7009, // Duplicate map key
E7010, // Duplicate match pattern
E7011, // Double negation
E7012, // Negated boolean literal
E7014, // Infinite iterator consumed without bound

// Naming (E71xx)
E7101, // Type name not PascalCase
E7102, // Function name not snake_case
E7103, // Variable name not snake_case
E7104, // Constant name not snake_case
E7105, // Type parameter not PascalCase
E7106, // Sum variant not PascalCase
E7107, // Module name not snake_case
E7108, // Field name not snake_case
E7109, // Predicate function missing prefix

// Complexity (E72xx)
E7201, // Cognitive complexity too high (> 15)
E7202, // Too many function parameters (> 5)
E7203, // Too many match arms (> 15)

// Clarity (E73xx)
E7301, // Shadowed binding
E7302, // Boolean function parameter
E7303, // Magic number
E7304, // Deeply nested conditional

// Performance (E74xx)
E7401, // Collect then iterate
E7402, // O(n²) contains in loop
```

Add `is_lint_error()` method to `ErrorCode` for the E7xxx range.

### Format-on-Compile Integration

Modify the Salsa query pipeline in `oric`:

1. After `parse_module` query succeeds, run `format_module`
2. Compare formatted output to source text
3. If different, write formatted text to disk
4. Subsequent phases operate on the (already valid) AST, not re-read text

The format step is idempotent: running it twice produces identical output.

### Phased Rollout

Not all rules need to ship simultaneously. Priority order:

**Phase 1 (immediate):** Rules with zero ambiguity
- E7001–E7005 (unused code)
- E7006 (comparison to self)
- E7007 (unreachable code)
- E7009–E7012 (duplicate/redundant patterns)
- E7101–E7108 (naming conventions)
- E7014 (infinite iterator — promote W2001)
- Format-on-compile integration

**Phase 2 (soon after):** Rules requiring scope/complexity analysis
- E7008 (discarded results)
- E7201 (cognitive complexity)
- E7202–E7203 (parameter count, match arms)
- E7301 (shadowing)
- E7109 (predicate naming)

**Phase 3 (later):** Rules requiring cross-function or pattern analysis
- E7302 (boolean parameters)
- E7303 (magic numbers)
- E7304 (nested conditionals)
- E7401–E7402 (performance patterns)

---

## Prior Art

### Go (`go vet` + `gofmt`)

Go is the closest prior art. `gofmt` has zero options and is universally adopted. `go vet` catches correctness issues and is run automatically by `go test`. However:
- `go vet` is still a separate tool
- `go vet` findings are warnings, not errors
- Limited rule set compared to this proposal

### Zig

Zig's compiler includes style enforcement — unused variables are compile errors, not warnings. The `zig fmt` is built into the compiler. Closest to Ori's philosophy:
- Unused variables are errors (like our E7002)
- Formatter is part of the toolchain
- But: `zig fmt` is still a separate invocation, not auto-applied

### Elm

Elm's compiler is famously opinionated with no escape hatches. Naming conventions are enforced by the compiler (types must be capitalized, etc.). The `elm-format` tool has zero options. However:
- `elm-format` is still separate from `elm make`
- Limited lint rules beyond naming

### Rust (Clippy)

Clippy is the anti-pattern this proposal aims to avoid:
- ~700 rules, half disabled by default
- `#[allow(clippy::...)]` used pervasively
- Separate tool (`cargo clippy` vs `cargo check`)
- Configuration via `clippy.toml`
- Generates advisory warnings that are routinely ignored

However, Clippy made the right call adopting **cognitive complexity** (via `clippy::cognitive_complexity`) over cyclomatic complexity. Ori adopts the same metric — it measures readability, not path count.

### SonarSource Cognitive Complexity

The cognitive complexity metric was defined in the 2017 whitepaper "Cognitive Complexity: A new way of measuring understandability" by G. Ann Campbell (SonarSource). Key insight: cyclomatic complexity (McCabe, 1976) was designed for *testing* (how many paths to cover), not *readability* (how hard is this to understand). Cognitive complexity weights nesting because humans find nested structures exponentially harder to parse. This metric has since been adopted by SonarQube, Clippy, ESLint (via plugin), and now Ori.

---

## Comparison

| Aspect | Clippy (Rust) | ESLint (JS) | `go vet` (Go) | Ori (this proposal) |
|--------|--------------|-------------|---------------|---------------------|
| Separate tool | Yes | Yes | Yes | **No** |
| Config file | `clippy.toml` | `.eslintrc.*` | None | **None** |
| Escape hatches | `#[allow]` | `// disable` | None | **None** |
| Severity | Warn/Deny/Allow | Warn/Error/Off | Warn | **Error only** |
| Auto-format | No (rustfmt) | No (Prettier) | No (gofmt) | **Yes (on compile)** |
| Rule count | ~700 | ~300+ | ~30 | **~30 (precise)** |
| False positive strategy | Suppress with `#[allow]` | Suppress with comments | Keep rules simple | **Fix the rule** |

---

## Examples

### Before and After

#### Unused import (E7001)

```
error[E7001]: unused import `sqrt`
  --> src/math.ori:1:22
   |
 1 | use std.math { abs, sqrt }
   |                     ^^^^
   |
   = help: remove unused import or use it in the module
```

#### Magic number (E7303)

```
error[E7303]: magic number `86400`
  --> src/cache.ori:12:20
   |
12 |     let expires = now + 86400,
   |                        ^^^^^
   |
   = help: extract to named constant: `let $seconds_per_day = 86400`
```

#### Cognitive complexity (E7201)

```
error[E7201]: function `process_all` has cognitive complexity 22 (maximum 15)
  --> src/pipeline.ori:10:1
   |
10 | @process_all (data: [Record]) -> [Result] = {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = help: extract nested logic into helper functions to reduce complexity
   = note: highest-cost construct: `if` at line 18 (nesting depth 4, cost +5)
```

#### Boolean parameter (E7302)

```
error[E7302]: boolean parameter `verbose` — use an enum for clarity
  --> src/logger.ori:5:25
   |
 5 | @log (msg: str, verbose: bool) -> void =
   |                 ^^^^^^^^^^^^^
   |
   = help: define `type Verbosity = Quiet | Verbose` and use `verbose: Verbosity`
```

---

## Spec Changes Required

### New: `XX-lint-rules.md`

Add a new spec section documenting all lint rules, their codes, thresholds, and exemptions.

### Update: `16-formatting.md`

Add section on format-on-compile behavior:
- "The compiler auto-formats source files on every compilation. See §16 for normalization rules."
- Document that `ori fmt --check` is the CI read-only gate.

### Update: `grammar.ebnf`

No grammar changes needed — lints operate on the typed AST, not syntax.

### Update: Error documentation

Add `E7xxx.md` files to `compiler/ori_diagnostic/src/errors/` for each lint rule.

---

## Future Extensions

### Auto-fix Lints

Some lint errors could be auto-fixed (like the formatter auto-fixes style):
- E7001 (unused import) → remove import
- E7011 (double negation) → remove `!!`
- E7012 (negated literal) → replace with literal

This is deferred to avoid scope creep. The initial version reports errors; auto-fix can be added later as `ori fix`.

### Cross-Module Lints

Rules that analyze multiple modules together:
- Unused `pub` functions (exported but never imported)
- Circular dependency detection (already handled elsewhere)
- API consistency checks

### Deeper Analysis

More sophisticated analysis building on the cognitive complexity foundation:
- Data flow analysis for more precise unused detection
- Escape analysis for performance suggestions
- Interprocedural complexity (function A calls B which calls C — total cognitive load)

---

## Summary

| Feature | Decision |
|---------|----------|
| Formatter | Auto-applied on every compilation |
| Formatter options | None. Zero config. |
| Lint severity | Error only. No warnings. |
| Escape hatches | None. No `@allow`, no config, no flags. |
| Error code range | E7xxx (E70xx correctness, E71xx naming, E72xx complexity, E73xx clarity, E74xx performance) |
| Total initial rules | ~30 |
| False positive policy | Fix the rule, don't suppress the diagnostic |
| Complexity metric | Cognitive complexity (SonarSource model), threshold 15 |
| Rule thresholds | 15 cognitive complexity, 5 params, 15 match arms, 3 nested conditionals |
| `--strict` | Removed. All checks always run. |

Ori's built-in linter and format-on-compile system ensures that **every Ori codebase meets the same quality bar, with no configuration, no negotiation, and no exceptions.** The compiler is the single authority on code quality.
