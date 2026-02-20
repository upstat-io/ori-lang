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

Blocks are expressions. Statements are terminated by `;`. The last expression in a block (without `;`) is its value. `run()` is removed from the language.

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

// Proposed — block syntax with semicolons
@spawn_food (game: Game) -> Game uses Random = {
    let food = loop {
        let $p = Point {
            x: Random.int_in_range(min: 1, max: game.width - 2),
            y: Random.int_in_range(min: 1, max: game.height - 2),
        };
        if !occupies(snake: game.snake, p:) then break p
    };

    Game { ...game, food }
}
```

What disappeared: `run(`, `)`, `loop(run(` nesting. Semicolons replace commas as statement terminators — a universally understood convention. What remained: every meaningful line of code.

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
            let $l = eval(expr: left, env:)?;
            let $r = eval(expr: right, env:)?;

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
    let $key = unsafe(_read_key());
    if key == $KEY_QUIT then break;
    game = tick(game:);
}

// For...do body
for x in items do {
    let $processed = transform(value: x);
    output(value: processed);
}

// For...yield with block — block value is what gets yielded
for i in 0..n yield {
    let $x = f(i:);

    x * 2
}

// If...then branches
if score >= 10 then {
    let $bonus = score * 2;
    print(msg: `Winner! Bonus: {bonus}`);
} else {
    print(msg: "Try again");
}

// Lambda body
x -> {
    let $doubled = x * 2;

    doubled + 1
}

// Unsafe body
unsafe {
    _game_over(n: game.score);
    _sleep(ms: 3000);
    _cleanup();
}
```

No special syntax for each case. A block works anywhere an expression works.

### Contracts Move to the Interface

With `run()` removed, contracts (`pre_check:`/`post_check:`) need a new home. Rather than finding another body-level construct to host them, contracts move to where they belong: the function declaration, following C++26's design.

```ori
// Current — contracts buried inside {} body
@divide (a: int, b: int) -> int = {
    pre_check: b != 0;
    a div b;
    post_check: r -> r * b <= a
}

// Proposed — contracts on the declaration
@divide (a: int, b: int) -> int
    pre(b != 0)
    post(r -> r * b <= a)
= a div b
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
    let $w = 30;
    let $h = 20;

    unsafe(_init(w:, h:));
    let game = new_game(width: w, height: h);
    render(game:);

    loop {
        unsafe(_sleep(ms: 120));
        let $key = unsafe(_read_key());
        if key == $KEY_QUIT then break;
        game = handle_input(game:, key:);
        game = tick(game:);
        if !game.alive then break;
        render(game:);
    }

    unsafe {
        _game_over(n: game.score);
        _sleep(ms: 3000);
        _cleanup();
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

### Semicolons

Ori uses Rust-style semicolons consistently across the language. Semicolons terminate statements; the absence of a semicolon on the last expression in a block marks it as the block's value.

**Inside blocks** — `;` terminates every statement. The last expression without `;` is the block's value:

```ori
{
    let $x = compute();
    let $y = transform(value: x);

    x + y    // <- no semicolon: this is the block's value
}
```

A block where every expression has `;` is a void block (like Rust):

```ori
{
    setup();
    do_work();
    cleanup();    // semicolon on last line: block returns void
}
```

**Top-level items** — the universal rule is: **ends with `}`? No `;`. Everything else: `;`.** This matches Rust exactly.

```ori
// Imports — always ;
use std.math { sqrt };
use "./utils" { helper };

// Constants — always ;
let $MAX_SIZE = 1024;
let $PI = 3.14159;

// Functions — ; when body is expression, no ; when body is block
@double (x: int) -> int = x * 2;

@process (items: [int]) -> int = {
    let $total = fold(over: items, init: 0, op: (a, b) -> a + b);

    total
}

// Types — struct (ends with }) no ;, sum/newtype ;
type Point = { x: int, y: int }

type UserId = int;

type Shape = Circle(r: float) | Rect(w: float, h: float);

// Traits, impls, extends, extern blocks — end with }, no ;
trait Drawable {
    @draw (self) -> void;              // method signature — ;
    @color (self) -> str = "black";    // default method (expression body) — ;
}

impl Drawable for Point {
    @draw (self) -> void = print(msg: `({self.x}, {self.y})`);
}
```

**Match arms** — separated by newlines (no `;`). Multi-expression arm bodies use blocks with `;`:

```ori
match dir {
    Up    -> Point { ...point, y: point.y - 1 }
    Down  -> Point { ...point, y: point.y + 1 }
    Left  -> Point { ...point, x: point.x - 1 }
    Right -> Point { ...point, x: point.x + 1 }
}
```

### Block Syntax

A block is `{ stmt; stmt; expr }` where `;` terminates statements and the last expression (without `;`) is the block's value. This follows Rust's block semantics exactly.

```ori
{
    let $x = compute();
    let $y = transform(value: x);

    x + y    // <- block value (no semicolon)
}
```

### Last Expression Is the Value

The last expression in a block is its value — identified by the absence of a trailing `;`. This is the same semantic `run()` already has (last argument = result), expressed with universally understood syntax.

**Two visual signals** make the result expression unmistakable:
1. **No semicolon** — syntactically marks it as the value (compiler-enforced)
2. **Blank line above** — `ori fmt` enforces a blank line before the result in setup+result blocks

```ori
@spawn_food (game: Game) -> Game uses Random = {
    let food = loop {
        let $p = Point {
            x: Random.int_in_range(min: 1, max: game.width - 2),
            y: Random.int_in_range(min: 1, max: game.height - 2),
        };
        if !occupies(snake: game.snake, p:) then break p
    };

    Game { ...game, food }
}
```

**The type checker is the safety net.** If someone appends a `print();` after what was the value expression, the return type changes from `T` to `void` — immediate compile error. Rust proves this is sufficient over 10 years of production use.

**Void blocks** — when every expression has `;`, the block returns void:

```ori
@main () -> void = {
    unsafe(_init(w:, h:));
    let game = new_game(width: w, height: h);
    render(game:);
    loop { ... }
    unsafe(_cleanup());
}
```

### Not `return`

This is **not** implicit return. `return` is control flow — it jumps out of a function from anywhere. Last-expression-is-value is structural — the block always runs to the end, and the final expression IS the block's value. No jump, no early exit. Ori's exits remain `break` (loops), `?` (errors), and `panic` (abort).

### `match` Syntax

The scrutinee moves before the block. Arms are newline-separated. Multi-expression arm bodies use blocks with semicolons.

```ori
// Standard — newline-separated arms
match expr {
    Lit(value) -> Ok(value)
    Var(name) -> env[name].ok_or(error: "undefined: " + name)
    BinOp(op, left, right) -> {
        let $l = eval(expr: left, env:)?;
        let $r = eval(expr: right, env:)?;

        Ok(l + r)
    }
}
```

**Edge case — matching on a block**: `match { block_expr } { arms }` is valid but unusual. The parser greedily parses the expression after `match`, so `{ ... }` is consumed as a block scrutinee, then `{` is expected for the match body. If someone writes `match { P -> e }` intending it as a single-arm match, the parser interprets `{ P -> e }` as a block containing a lambda and then fails expecting `{` for arms. The error message should suggest: "did you mean `match expr { ... }`?"

### `try` Syntax

```ori
try {
    let $x = fallible()?;
    let $y = other()?;

    Ok(x + y)
}
```

### `loop`, `unsafe`, `for...do` — Block Bodies

These constructs take a block directly:

```ori
// Loop
loop {
    let $x = next();
    if x == 0 then break;
    process(value: x);
}

// Unsafe — block form
unsafe {
    _game_over(n: game.score);
    _sleep(ms: 3000);
    _cleanup();
}

// Unsafe — single-expression form retained
unsafe(_cleanup())

// For...do
for gen in 0..20 do {
    print(msg: display(grid:));
    grid = step(grid:);
}

// For...yield — block value is yielded
for i in 0..n yield {
    let $x = f(i:);

    x * 2
}
```

### `run()` — Removed

`run()` is removed from the language. Its two former roles are replaced:

1. **Sequencing** — replaced by `{ }` blocks with `;`-terminated statements
2. **Contracts** — replaced by function-level `pre()`/`post()` declarations

### Function-Level Contracts: `pre()` / `post()`

Contracts move from `run()` body-level to the function declaration, following C++26's design. They sit between the return type and the `=`:

```ori
@divide (a: int, b: int) -> int
    pre(b != 0)
    post(r -> r * b <= a)
= a div b
```

#### Syntax

```ori
@name (params) -> ReturnType
    pre(condition)                    // Optional: checked before body
    pre(condition | "message")        // Optional: with custom message
    pre(another_condition)            // Optional: multiple checks allowed
    post(result -> condition)         // Optional: checked after body
    post(result -> condition | "msg") // Optional: with custom message
= expression
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
= x + 1

// Desugars to:
@f (x: int) -> int = {
    if !(x > 0) then panic(msg: "pre failed: x > 0");
    let $__result = x + 1;
    if !(r -> r > x)(__result) then panic(msg: "post failed: r > x");

    __result
}
```

The compiler embeds the condition's source text as a string literal for default messages.

#### Examples

```ori
// Basic
@abs (x: int) -> int
    post(r -> r >= 0)
= if x < 0 then -x else x

@sqrt (x: float) -> float
    pre(x >= 0.0)
    post(r -> r >= 0.0)
= newton_raphson(x: x)

// Multiple conditions with messages
@transfer (from: Account, to: Account, amount: int) -> (Account, Account)
    pre(amount > 0 | "transfer amount must be positive")
    pre(from.balance >= amount | "insufficient funds")
    pre(from.id != to.id | "cannot transfer to same account")
    post((f, t) -> f.balance == from.balance - amount)
    post((f, t) -> t.balance == to.balance + amount)
    post((f, t) -> f.balance + t.balance == from.balance + to.balance)
= {
    let $new_from = Account { id: from.id, balance: from.balance - amount };
    let $new_to = Account { id: to.id, balance: to.balance + amount };

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
| Lexer | Add `;` as a token (if not already present) |
| IR | `FunctionSeq::Run` loses `pre_checks`/`post_checks` fields. Contract checks move to function definition node. |
| Parser | New paths: bare `{ }` -> block expression, `match expr { }`, `try { }`, `loop { }` / `unsafe { }` / `for...do { }` drop parens. Function-level `pre()`/`post()` parsing. Semicolon-terminated statements inside blocks. Semicolons on `use`, `let $`, and expression-bodied declarations. |
| Type checker | Contract validation moves from `Run` handling to function definition handling |
| Evaluator | Contract evaluation moves to function entry/exit |
| LLVM codegen | Contract codegen moves to function entry/exit |
| Formatter | New rules for `{ }` block formatting + blank-line-before-result enforcement |

### Parser Changes

The parser needs:

1. **Semicolons in blocks**: Inside `{ }`, parse `;`-terminated statements followed by an optional result expression (no `;`). A block where every expression has `;` is void.

2. **Top-level semicolons**: `use` imports and `let $` constants require `;`. Function declarations, type definitions, and other items that end with `}` do not. Expression-bodied declarations (functions, methods, newtypes, sum types) require `;`.

3. **Bare `{ }`**: When `{` appears in expression position and disambiguation says "block" (not map/struct), parse as a block expression.

4. **`match expr { }`**: When `match` is followed by an expression then `{`, parse the scrutinee, then parse newline-separated match arms inside the block.

5. **`try { }`**: When `try` is followed by `{`, parse as a try block.

6. **`loop { }` / `unsafe { }` / `for...do { }`**: When these keywords are followed by `{`, parse the block body directly.

7. **Function-level `pre()`/`post()`**: After parsing `-> ReturnType`, check for `pre` or `post` tokens before `=`.

8. **Removal**: `run()`, `match()`, `try()` paren-based forms are removed.

### Grammar Changes

```ebnf
(* Block expressions — semicolons terminate statements, last expression is value *)
block_expr     = "{" { statement } [ expression ] "}" .
statement      = ( let_expr | expression ) ";" .

(* Top-level semicolons *)
import         = "use" import_path [ import_list | "as" identifier ] ";" .
constant_decl  = "let" "$" identifier [ ":" type ] "=" expression ";" .

(* Match — scrutinee before block *)
match_expr     = "match" expr "{" match_arms "}" .
match_arms     = { match_arm NEWLINE } [ match_arm ] .
match_arm      = pattern [ "if" expr ] "->" expr .

(* Try — keyword before block *)
try_expr       = "try" block_expr .

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

- All existing `run()` / `match()` / `try()` tests rewritten to use block syntax with semicolons
- Parser tests for semicolons: required in blocks, on `use`/`let $`, on expression-bodied declarations
- Parser tests for missing semicolons (error recovery and messages)
- Parser tests for void blocks (trailing `;` on last expression)
- Parser tests for disambiguation (block vs map vs struct)
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

---

## Errata (added 2026-02-20)

> **Superseded by [match-arm-comma-separator-proposal](match-arm-comma-separator-proposal.md)**: This proposal specified match arms as newline-separated. The match-arm-comma-separator proposal changes arms to comma-separated (with optional trailing commas), aligning match syntax with Rust and making it consistent with the explicit-punctuation style introduced by this proposal's semicolons. Additionally, the guard syntax `.match(condition)` is replaced by `if condition` — `.match()` now exclusively refers to method-style pattern matching.
>
> Affected sections: "Match arms — separated by newlines" (Design § Semicolons), "`match` Syntax" (Design), grammar `match_arms` production, parser changes item 4.
