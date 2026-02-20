# Proposal: Block Expression Syntax — `{ }` Replaces `run()` / `match()` / `try()`

**Status:** Approved
**Author:** Eric
**Created:** 2026-02-19
**Approved:** 2026-02-19

---

## Summary

Replace the parenthesized `function_seq` syntax with curly-brace block syntax. Move contracts from `run()` body-level to C++-style function-level declarations. Remove `run()` entirely.

| Current | Proposed |
|---------|----------|
| `run(let $x = 1, x + 2)` | `{ let $x = 1 \n x + 2 }` |
| `match(expr, P -> e, ...)` | `match expr { P -> e \n ... }` |
| `try(let $x = f()?, Ok(x))` | `try { let $x = f()? \n Ok(x) }` |
| `loop(run(...))` | `loop { ... }` |
| `unsafe(run(...))` | `unsafe { ... }` |
| `run(pre_check: c, body, post_check: r -> c)` | `pre(c) post(r -> c)` on function |

Blocks are expressions. The last expression in a block is its value. Newlines separate expressions. `run()` is removed from the language.

---

## Motivation

### `run()` Is Ceremony

`run()` is the most-typed construct in Ori. Almost every non-trivial function body is `run(...)` with comma-separated expressions. Four real programs (expression evaluator, Game of Life, Snake, dice game) confirmed this — `run()` appeared on nearly every function with more than one expression.

The commas and trailing parenthesis add visual noise without semantic value:

```ori
// Current — every function body wrapped in {}
@spawn_food (game: Game) -> Game uses Random =
    {
        let food = loop {
            let $p = Point {
                x: Random.int_in_range(min: 1, max: game.width - 2)
                y: Random.int_in_range(min: 1, max: game.height - 2)
            }
            if !occupies(snake: game.snake, p:) then break p
        }
        Game { ...game, food }
    }

// Proposed — block syntax
@spawn_food (game: Game) -> Game uses Random = {
    let food = loop {
        let $p = Point {
            x: Random.int_in_range(min: 1, max: game.width - 2),
            y: Random.int_in_range(min: 1, max: game.height - 2),
        }
        if !occupies(snake: game.snake, p:) then break p
    }

    Game { ...game, food }
}
```

What disappeared: `run(`, `)`, every comma between statements, `loop(run(` nesting. What remained: every meaningful line of code.

### `match()` and `try()` Have the Same Problem

```ori
// Current
@eval (expr: Expr, env: {str: float}) -> Result<float, str> =
    match expr {
        Lit(value) -> Ok(value)
        Var(name) -> env[name].ok_or(error: "undefined: " + name)
        BinOp(op, left, right) -> {
            let l = eval(expr: left, env:)?
            let r = eval(expr: right, env:)?
            Ok(l + r)
        }
    }

// Proposed
@eval (expr: Expr, env: {str: float}) -> Result<float, str> =
    match expr {
        Lit(value) -> Ok(value)
        Var(name) -> env[name].ok_or(error: "undefined: " + name)
        BinOp(op, left, right) -> {
            let l = eval(expr: left, env:)?
            let r = eval(expr: right, env:)?
            Ok(l + r)
        }
    }
```

The scrutinee moves outside the parens. Arms are newline-separated. Multi-expression arms use blocks. No nested `run()` needed.

### Blocks Compose Naturally

Because `{ }` is an expression form, every construct that takes an expression gets multi-statement bodies for free:

```ori
// Loop body
loop {
    let $key = unsafe(_read_key())
    if key == $KEY_QUIT then break
    game = tick(game:)
}

// For...do body
for x in items do {
    let $processed = transform(value: x)
    output(value: processed)
}

// For...yield with block — block value is what gets yielded
for i in 0..n yield {
    let $x = f(i:)
    x * 2
}

// If...then branches
if score >= 10 then {
    let $bonus = score * 2
    print(msg: `Winner! Bonus: {bonus}`)
} else {
    print(msg: "Try again")
}

// Lambda body
x -> {
    let $doubled = x * 2
    doubled + 1
}

// Unsafe body
unsafe {
    _game_over(n: game.score)
    _sleep(ms: 3000)
    _cleanup()
}
```

No special syntax for each case. A block works anywhere an expression works.

### Contracts Move to the Interface

With `run()` removed, contracts (`pre_check:`/`post_check:`) need a new home. Rather than finding another body-level construct to host them, contracts move to where they belong: the function declaration, following C++26's design.

```ori
// Current — contracts buried inside {} body
@divide (a: int, b: int) -> int = {
    pre_check: b != 0
    a div b
    post_check: r -> r * b <= a
}

// Proposed — contracts on the declaration
@divide (a: int, b: int) -> int
    pre(b != 0)
    post(r -> r * b <= a)
= {
    a div b
}
```

Contracts describe what a function *promises*, not how it *works*. They are part of the interface. Placing them between the signature and the body makes this explicit, enables tooling (LSP hover, documentation) to surface them without parsing bodies, and eliminates the need for `run()` to exist.

### Comparison with Rust (Readability)

Ori's sigil system (`@` for functions, `$` for statics) provides strong visual landmarks that Rust's keyword-based syntax lacks. With block syntax, Ori achieves Rust-like block structure without Rust's readability costs:

```ori
// Ori — @ marks functions, $ marks statics, named params self-document
@advance (point: Point, dir: Direction) -> Point =
    match dir {
        Up    -> Point { ...point, y: point.y - 1 }
        Down  -> Point { ...point, y: point.y + 1 }
        Left  -> Point { ...point, x: point.x - 1 }
        Right -> Point { ...point, x: point.x + 1 }
    }

@main () -> void uses Random, FFI = {
    let $w = 30
    let $h = 20

    unsafe(_init(w:, h:))
    let game = new_game(width: w, height: h)
    render(game:)

    loop {
        unsafe(_sleep(ms: 120))
        let $key = unsafe(_read_key())
        if key == $KEY_QUIT then break
        game = handle_input(game:, key:)
        game = tick(game:)
        if !game.alive then break
        render(game:)
    }

    unsafe {
        _game_over(n: game.score)
        _sleep(ms: 3000)
        _cleanup()
    }
}
```

Key advantages over Rust's equivalent:
- `@` is a single-character visual landmark vs `fn` (2 lowercase letters that blend into code)
- `$` marks statics with one character vs `const` (5 characters) or `let`/`let mut` juggling
- Named parameters (`game:`, `key:`) eliminate the need to look up function signatures
- Argument punning (`game:` = `game: game`) removes repetition without losing clarity
- No `Rc<RefCell<>>` — ARC handles shared mutable state without wrapper ceremony
- No `::` namespace chains — `HBRUSH.create_solid(color: c)` vs `HBRUSH::CreateSolidBrush(c).unwrap()`

---

## Design

### Block Syntax

A block is `{ expr1 \n expr2 \n ... \n exprn }` where newlines separate expressions and the last expression is the block's value.

```ori
{
    let $x = compute()
    let $y = transform(value: x)

    x + y    // <- block value
}
```

### Newline Separation

Inside `{ }` blocks, newlines separate expressions. Commas are optionally allowed for one-liner blocks:

```ori
// Multiline — newlines separate
{
    let $x = 1
    let $y = 2
    x + y
}

// One-liner — commas allowed
match dir { Up -> Down, Down -> Up, Left -> Right, Right -> Left }
```

### Continuation Rules

Newline handling follows the **balanced delimiter** approach (Go, Kotlin):

- **Inside `()`, `[]`, or nested `{}`**: Newlines do NOT end statements. Expressions can span multiple lines freely.
- **Outside balanced delimiters**: Newlines ARE statement separators.
- **No trailing-operator continuation**: A binary operator at line-end does NOT automatically continue to the next line. Wrap multi-line expressions in parentheses.
- **Keyword constructs**: `match`, `if...then`, `for...do`, `loop`, `unsafe`, `try` consume their full syntactic form regardless of newlines — the parser knows these constructs require additional tokens.

```ori
// Balanced delimiters suppress newlines
let $result = some_function(
    arg1: value1,
    arg2: value2,
)

// Multi-line binary expression — use parens
let $total = (
    base_price
    + tax
    + shipping
)

// Keywords consume their full form across newlines
match expr {
    Pattern1 -> result1
    Pattern2 -> result2
}
```

### Last Expression Is the Value

The last expression in a block is its value. This is the same semantic `run()` already has (last argument = result), just with different visual framing.

**The type checker is the safety net.** If someone appends a `print()` after what was the value expression, the return type changes from `T` to `void` — immediate compile error. Rust proves this is sufficient over 10 years of production use.

**Blocks naturally fall into two readable shapes:**

**Shape 1 — Setup + result**: `let` bindings cluster at the top, value expression sits alone at the bottom. The `let` lines look visually different from the result line.

```ori
@spawn_food (game: Game) -> Game uses Random = {
    let food = loop {
        let $p = Point {
            x: Random.int_in_range(min: 1, max: game.width - 2),
            y: Random.int_in_range(min: 1, max: game.height - 2),
        }
        if !occupies(snake: game.snake, p:) then break p
    }

    Game { ...game, food }
}
```

**Shape 2 — Pure side effects**: the block is `void`, every line is a statement. No "value" to confuse.

```ori
@main () -> void = {
    unsafe(_init(w:, h:))
    let game = new_game(width: w, height: h)
    render(game:)
    loop { ... }
    unsafe(_cleanup())
}
```

**Formatting convention**: `ori fmt` enforces a blank line before the result expression when a block has setup + value. This provides a visual "here's the value" signal without syntax cost.

### Not `return`

This is **not** implicit return. `return` is control flow — it jumps out of a function from anywhere. Last-expression-is-value is structural — the block always runs to the end, and the final expression IS the block's value. No jump, no early exit. Ori's exits remain `break` (loops), `?` (errors), and `panic` (abort).

### `match` Syntax

The scrutinee moves before the block. Arms are newline-separated (commas optional for one-liners). Multi-expression arms use blocks.

```ori
// Simple — one-liner with commas
match dir { Up -> Down, Down -> Up, Left -> Right, Right -> Left }

// Standard — newline-separated arms
match expr {
    Lit(value) -> Ok(value)
    Var(name) -> env[name].ok_or(error: "undefined: " + name)
    BinOp(op, left, right) -> {
        let l = eval(expr: left, env:)?
        let r = eval(expr: right, env:)?
        Ok(l + r)
    }
}
```

**Edge case — matching on a block**: `match { block_expr } { arms }` is valid but unusual. The parser greedily parses the expression after `match`, so `{ ... }` is consumed as a block scrutinee, then `{` is expected for the match body. If someone writes `match { P -> e }` intending it as a single-arm match, the parser interprets `{ P -> e }` as a block containing a lambda and then fails expecting `{` for arms. The error message should suggest: "did you mean `match expr { ... }`?"

### `try` Syntax

```ori
try {
    let $x = fallible()?
    let $y = other()?
    Ok(x + y)
}
```

### `loop`, `unsafe`, `for...do` — Block Bodies

These constructs take a block directly:

```ori
// Loop
loop {
    let $x = next()
    if x == 0 then break
    process(value: x)
}

// Unsafe — block form
unsafe {
    _game_over(n: game.score)
    _sleep(ms: 3000)
    _cleanup()
}

// Unsafe — single-expression form retained
unsafe(_cleanup())

// For...do
for gen in 0..20 do {
    print(msg: display(grid:))
    grid = step(grid:)
}

// For...yield — block value is yielded
for i in 0..n yield {
    let $x = f(i:)
    x * 2
}
```

### `run()` — Removed

`run()` is removed from the language. Its two former roles are replaced:

1. **Sequencing** — replaced by `{ }` blocks (with commas for inline one-liners)
2. **Contracts** — replaced by function-level `pre()`/`post()` declarations

### Function-Level Contracts: `pre()` / `post()`

Contracts move from `run()` body-level to the function declaration, following C++26's design. They sit between the return type and the `=`:

```ori
@divide (a: int, b: int) -> int
    pre(b != 0)
    post(r -> r * b <= a)
= {
    a div b
}
```

#### Syntax

```ori
@name (params) -> ReturnType
    pre(condition)                    // Optional: checked before body
    pre(condition | "message")        // Optional: with custom message
    pre(another_condition)            // Optional: multiple checks allowed
    post(result -> condition)         // Optional: checked after body
    post(result -> condition | "msg") // Optional: with custom message
= {
    // body
}
```

#### Semantics

All semantic decisions from the approved `checks-proposal` are preserved:

**Evaluation order:**
1. Evaluate all `pre()` conditions in order
2. If any `pre()` fails, panic with message
3. Execute function body
4. Bind result to each `post()` lambda parameter
5. Evaluate all `post()` conditions in order
6. If any `post()` fails, panic with message
7. Return result

**Scope constraints:**
- `pre()` expressions may only reference function parameters and module-level bindings
- `post()` lambdas may reference the result (via lambda parameter) plus everything visible to `pre()`

**Type constraints:**
- `pre()` condition must have type `bool`
- `post()` must be a lambda from the result type to `bool`
- Compile error if `post()` used on a function returning `void`
- Message expressions (after `|`) must have type `str`

**Desugaring:**
```ori
@f (x: int) -> int
    pre(x > 0)
    post(r -> r > x)
= { x + 1 }

// Desugars to:
@f (x: int) -> int = {
    if !(x > 0) then panic(msg: "pre failed: x > 0")
    let $__result = { x + 1 }
    if !(r -> r > x)(__result) then panic(msg: "post failed: r > x")
    __result
}
```

The compiler embeds the condition's source text as a string literal for default messages.

#### Examples

```ori
// Basic
@abs (x: int) -> int
    post(r -> r >= 0)
= {
    if x < 0 then -x else x
}

@sqrt (x: float) -> float
    pre(x >= 0.0)
    post(r -> r >= 0.0)
= {
    newton_raphson(x: x)
}

// Multiple conditions with messages
@transfer (from: Account, to: Account, amount: int) -> (Account, Account)
    pre(amount > 0 | "transfer amount must be positive")
    pre(from.balance >= amount | "insufficient funds")
    pre(from.id != to.id | "cannot transfer to same account")
    post((f, t) -> f.balance == from.balance - amount)
    post((f, t) -> t.balance == to.balance + amount)
    post((f, t) -> f.balance + t.balance == from.balance + to.balance)
= {
    let $new_from = Account { id: from.id, balance: from.balance - amount }
    let $new_to = Account { id: to.id, balance: to.balance + amount }
    (new_from, new_to)
}

// Simple function — no block needed for single expression
@get<T> (items: [T], index: int) -> T
    pre(index >= 0 && index < len(collection: items))
= items[index]
```

### Disambiguation: Blocks vs Maps vs Structs

`{ }` is already used for map literals and struct literals. The parser disambiguates with at most two tokens of lookahead:

| Expression | Rule |
|---|---|
| `Point { x: 1, y: 2 }` | Type name before `{` -> **struct literal** |
| `{ "key": val }` | String key with `:` -> **map literal** |
| `{ key: val }` | `identifier: expr` pattern -> **map literal** |
| `{ [expr]: val }` | Computed key -> **map literal** |
| `{ let $x = ... }` | Starts with keyword -> **block** |
| `{ foo() \n bar() }` | No `:` after first token -> **block** |
| `{ x }` | Lone identifier, no `:` -> **block** (evaluates `x`) |

**Rule**: After `{`, if the parser sees `ident :` or `string :` or `[expr] :`, it's a map. If preceded by a type name, it's a struct. Otherwise it's a block.

**Empty `{ }`**: Always parsed as an empty map. Empty void blocks have no practical use; use `()` for void if needed.

---

## Implementation Impact

| Layer | Change |
|-------|--------|
| Lexer | None — `{` `}` already tokenized |
| IR | `FunctionSeq::Run` loses `pre_checks`/`post_checks` fields. Contract checks move to function definition node. |
| Parser | New paths: bare `{ }` -> block expression, `match expr { }`, `try { }`, `loop { }` / `unsafe { }` / `for...do { }` drop parens. Function-level `pre()`/`post()` parsing. Newline-as-separator logic. |
| Type checker | Contract validation moves from `Run` handling to function definition handling |
| Evaluator | Contract evaluation moves to function entry/exit |
| LLVM codegen | Contract codegen moves to function entry/exit |
| Formatter | New rules for `{ }` block formatting + blank-line-before-result enforcement |

### Parser Changes

The parser needs:

1. **Bare `{ }`**: When `{` appears in expression position and disambiguation says "block" (not map/struct), parse as a block expression — newline-separated bindings + result expression.

2. **`match expr { }`**: When `match` is followed by an expression then `{`, parse the scrutinee, then parse newline-separated match arms inside the block.

3. **`try { }`**: When `try` is followed by `{`, parse as a try block with newline-separated bindings + result expression.

4. **`loop { }` / `unsafe { }` / `for...do { }`**: When these keywords are followed by `{`, parse the block body directly.

5. **Newline handling**: Track newlines as statement separators inside blocks, suppressed inside balanced `()`, `[]`, `{}`.

6. **Function-level `pre()`/`post()`**: After parsing `-> ReturnType`, check for `pre` or `post` tokens before `=`.

7. **Removal**: `run()`, `match()`, `try()` paren-based forms are removed.

### Grammar Changes

```ebnf
(* Block expressions *)
block_expr     = "{" block_body "}" .
block_body     = { block_item sep } result_expr [ sep ] .
block_item     = let_binding | stmt_expr .
sep            = NEWLINE | "," .

(* Match — scrutinee before block *)
match_expr     = "match" expr "{" match_arms "}" .
match_arms     = { match_arm sep } [ match_arm ] .
match_arm      = pattern [ "if" expr ] "->" expr .

(* Try — keyword before block *)
try_expr       = "try" "{" block_body "}" .

(* Loop — direct block *)
loop_expr      = "loop" [ label ] block_expr .

(* Unsafe — block or single expression *)
unsafe_expr    = "unsafe" block_expr .
unsafe_expr    = "unsafe" "(" expr ")" .

(* For...do/yield — block as body *)
for_do_expr    = "for" pattern "in" expr ( "do" | "yield" ) expr .

(* Function-level contracts *)
function_def   = "@" name [ generics ] "(" params ")" "->" type { contract } "=" expr .
contract       = "pre" "(" check_expr ")" | "post" "(" postcheck_expr ")" .
check_expr     = expression [ "|" string_literal ] .
postcheck_expr = lambda_params "->" check_expr .
```

---

## Testing

- All existing `run()` / `match()` / `try()` tests rewritten to use block syntax
- Parser tests for disambiguation (block vs map vs struct)
- Parser tests for newline separation and optional comma tolerance
- Parser tests for continuation rules (balanced delimiters suppress newlines)
- Parser tests for function-level `pre()` / `post()` contracts
- Formatter tests for blank-line-before-result enforcement
- Error message tests for common mistakes (e.g., writing `match(expr, ...)` with old syntax)
- Error message tests for `match { P -> e }` edge case (missing scrutinee)
- Contract integration tests (eval order, scope, type checking, messages)

---

## Supersedes

This proposal supersedes the contract placement decision in the approved `checks-proposal.md`. The checks-proposal's semantic decisions (evaluation order, scope constraints, type constraints, `| "message"` syntax, desugaring to panic) remain valid. Only the syntax and placement change: `pre_check:`/`post_check:` inside `run()` becomes `pre()`/`post()` on the function declaration.

See errata added to `checks-proposal.md`.

---

## Origin

Discovered during spec experiments (2026-02-19) writing an expression evaluator, Game of Life, Snake game, and dice game in Ori. The `run()` ceremony was the most consistent friction point across all four programs. Discussion progressed from "add `{ }` blocks" to "all `function_seq` constructs become blocks, `run` is the unnamed default, contracts move to function declarations."

Data from experiments: 43 immutable bindings (`let $`), 3 mutable bindings (`let`), confirming the `$` = static convention works well with block syntax. The mutable-by-default design with `$` opt-in immutability provides the lightest ceremony of any mutable-by-default language (1 character vs TypeScript's `const` at 5, Java's `final` at 5, C#'s `readonly` at 8).
