---
title: "Getting Started"
description: "Install Ori and write your first program."
order: 1
part: "Foundations"
---

# Getting Started

Welcome to Ori. This guide will take you from zero to running your first program, and by the end you'll understand what makes Ori different from other languages.

## What is Ori?

Ori is a general-purpose, expression-based language built on a simple premise: **if your code compiles, it works**. The compiler enforces testing, tracks dependencies, and makes side effects explicit.

### The Four Pillars

Ori is built on four core principles:

**1. Mandatory Verification**

Every function requires tests or it doesn't compile. Tests are bound to functions (`@test tests @target`), and contracts (`pre_check:`/`post_check:`) enforce invariants. The compiler refuses to produce code it can't verify.

**2. Dependency-Aware Integrity**

Tests aren't external to your code — they're part of the dependency graph. Change a function and its tests run. Change a function and its callers' tests run too. You get fast feedback because only affected tests execute.

**3. Explicit Effects**

Functions must declare what they can do. `uses Http` means it makes network requests. `uses FileSystem` means it touches files. No hidden side effects. This makes mocking trivial and tests fast.

**4. ARC-Safe by Design**

Memory is managed via Automatic Reference Counting. No garbage collector pauses. No borrow checker complexity. Closures capture by value, preventing reference cycles by design.

### The Virtuous Cycle

These principles work together:

```
Capabilities make mocking easy
    -> Tests are fast
        -> Dependency-aware testing is practical
            -> Mandatory testing isn't painful
                -> Code integrity is enforced
                    -> Code that works, stays working
```

## Installation

Install Ori with a single command:

```bash
curl -fsSL https://ori-lang.com/install.sh | sh
```

This installs the `ori` command-line tool. Verify it worked:

```bash
ori --version
```

You should see something like `ori 0.1.0`.

### What You Just Installed

The `ori` command does several things:

- **Compiles** your code and checks for errors
- **Runs tests** automatically when you change functions
- **Executes** programs
- **Formats** code to a consistent style

You'll use it constantly during development.

## Your First Program

Create a new file called `hello.ori` and add this:

```ori
@main () -> void = print(msg: "Hello, World!")
```

Run it:

```bash
ori run hello.ori
```

You should see:

```
Hello, World!
```

Congratulations — you've written your first Ori program. Now let's understand what you wrote.

## Understanding the Syntax

Let's break down `@main () -> void = print(msg: "Hello, World!")`:

```ori
@main () -> void = print(msg: "Hello, World!")
|     |     |    | └─ Function body (what it does)
|     |     |    └─── Body follows
|     |     └──────── Returns nothing (void)
|     └────────────── Takes no parameters
└──────────────────── Function named "main"
```

### The `@` Sigil

In Ori, **functions are declared with `@`**:

```ori
@greet (name: str) -> str = `Hello, {name}!`
@add (a: int, b: int) -> int = a + b
@main () -> void = print(msg: "Starting...")
```

This visual distinction makes functions immediately recognizable in your code. When you see `@`, you know it's a function declaration.

### Named Arguments

Notice we wrote `print(msg: "Hello, World!")` not `print("Hello, World!")`. In Ori, **all function arguments must be named**:

```ori
print(msg: "Hello")              // Correct
add(a: 2, b: 3)                  // Correct
greet(name: "Alice")             // Correct

print("Hello")                   // ERROR: missing argument name
add(2, 3)                        // ERROR: missing argument names
```

This might feel verbose at first, but it has real benefits:

**Self-documenting code:**
```ori
// What do these arguments mean?
create_user("Alice", 30, true, false)

// vs. named arguments (actual Ori code):
create_user(name: "Alice", age: 30, admin: true, verified: false)
```

**Argument order doesn't matter:**
```ori
// These are equivalent:
create_user(name: "Alice", age: 30)
create_user(age: 30, name: "Alice")
```

**Catches mistakes at compile time:**
```ori
// You can't accidentally swap similar-typed arguments
send_email(from: alice, to: bob)   // Clear intent
send_email(to: bob, from: alice)   // Same result, still clear
```

### Template Strings

Backtick strings support interpolation with `{...}`:

```ori
let name = "Alice"
let greeting = `Hello, {name}!`    // "Hello, Alice!"

let a = 10
let b = 20
let result = `{a} + {b} = {a + b}` // "10 + 20 = 30"
```

Regular strings use double quotes and don't support interpolation:

```ori
let plain = "Hello, World!"
let escaped = "Line 1\nLine 2"
```

## Variables and Bindings

Let's make our program more interesting. Update `hello.ori`:

```ori
@main () -> void = run(
    let name = "World",
    print(msg: `Hello, {name}!`),
)
```

### The `run` Pattern

When a function needs multiple steps, wrap them in `run(...)`:

```ori
@main () -> void = run(
    let first = "Hello",
    let second = "World",
    print(msg: `{first}, {second}!`),
)
```

Each expression is separated by commas. The last expression's value becomes the function's return value.

### Variables with `let`

Create variables with `let`:

```ori
let name = "Alice"
let age = 30
let score = 95.5
```

Ori infers the type automatically. You can be explicit if you prefer:

```ori
let name: str = "Alice"
let age: int = 30
let score: float = 95.5
```

### Immutable Bindings with `$`

Sometimes you want to ensure a value never changes. Use `$`:

```ori
let $max_retries = 3        // Cannot be reassigned
let counter = 0             // Can be reassigned

counter = counter + 1       // OK
max_retries = 5             // ERROR: cannot reassign immutable binding
```

**When to use `$`:**
- Configuration values
- Constants
- Values that changing would be a bug

**Rule of thumb:** Start with `$`. Remove it only when you need reassignment.

## Writing Your First Test

Let's write a function that does something useful:

```ori
@greet (name: str) -> str = `Hello, {name}!`

@main () -> void = run(
    let message = greet(name: "Alice"),
    print(msg: message),
)
```

Run this:

```bash
ori run hello.ori
```

Wait — you'll get an error:

```
error: function 'greet' has no tests
  --> hello.ori:1:1
   |
 1 | @greet (name: str) -> str = `Hello, {name}!`
   | ^^^^^ untested function
```

This is Ori's **mandatory testing** at work. Every function needs at least one test.

Add a test for `greet`:

```ori
@greet (name: str) -> str = `Hello, {name}!`

@test_greet tests @greet () -> void = run(
    assert_eq(actual: greet(name: "Alice"), expected: "Hello, Alice!"),
    assert_eq(actual: greet(name: "Bob"), expected: "Hello, Bob!"),
)

@main () -> void = run(
    let message = greet(name: "Alice"),
    print(msg: message),
)
```

Let's understand the test:

```ori
@test_greet tests @greet () -> void = run(...)
|           |           |     |
|           |           |     └─ Returns nothing
|           |           └─────── Takes no parameters
|           └─────────────────── Links to the greet function
└─────────────────────────────── Test function name
```

The `tests @greet` part is crucial — it binds the test to a specific function. When you change `greet`, this test runs automatically.

Now run it:

```bash
ori run hello.ori
```

The compiler runs the tests first, then executes `main`:

```
Running tests...
  test_greet ... ok

Hello, Alice!
```

### Why Mandatory Testing?

You might wonder: "Why force me to write tests?"

Ori is designed around a principle: **code that compiles should work**. Testing isn't optional — it's part of the compilation process.

The benefits compound:

1. **Change a function?** Its tests run automatically
2. **Tests pass?** The function probably works
3. **Tests fail?** You find out immediately, not in production
4. **No untested code** can sneak into your project

This isn't about bureaucracy — it's about catching bugs early when they're cheap to fix.

## CLI Commands Overview

Here are the commands you'll use most:

| Command | What It Does |
|---------|--------------|
| `ori run file.ori` | Run a program (compiles, tests, then executes) |
| `ori check file.ori` | Compile and test without running |
| `ori check --no-test` | Compile only (useful for quick syntax checks) |
| `ori test` | Run all tests in the project |
| `ori fmt file.ori` | Format code to standard style |

### The Development Loop

A typical workflow:

1. **Write code** — Add or modify functions
2. **Run `ori check`** — See if it compiles and tests pass
3. **Fix issues** — Address any errors
4. **Run `ori run`** — Execute the program

Because tests run automatically during `check`, you get immediate feedback when something breaks.

## Program Entry Points

The `@main` function is where execution starts. There are four valid signatures:

```ori
// Basic: no args, no return value
@main () -> void = ...

// Return an exit code (0 = success)
@main () -> int = ...

// Accept command-line arguments
@main (args: [str]) -> void = ...

// Both: args and exit code
@main (args: [str]) -> int = ...
```

### Working with Command-Line Arguments

```ori
@main (args: [str]) -> void = run(
    if is_empty(collection: args) then
        print(msg: "No arguments provided")
    else run(
        print(msg: `Got {len(collection: args)} arguments:`),
        for arg in args do print(msg: `  - {arg}`),
    ),
)
```

Run with:

```bash
ori run program.ori -- first second third
```

Note: `args` contains only the arguments, not the program name.

### Exit Codes

Return an integer to indicate success or failure:

```ori
@main (args: [str]) -> int = run(
    if is_empty(collection: args) then run(
        print(msg: "Error: no arguments provided"),
        1,  // Non-zero = failure
    ) else run(
        print(msg: `Processing {len(collection: args)} items`),
        0,  // Zero = success
    ),
)
```

## The Complete Example

Here's everything we've covered in one program:

```ori
// A function that creates a greeting
@greet (name: str) -> str = `Hello, {name}!`

// Test for greet - required for compilation
@test_greet tests @greet () -> void = run(
    assert_eq(actual: greet(name: "Alice"), expected: "Hello, Alice!"),
    assert_eq(actual: greet(name: "Bob"), expected: "Hello, Bob!"),
    assert_eq(actual: greet(name: ""), expected: "Hello, !"),
)

// A function that creates a formal greeting
@formal_greet (title: str, name: str) -> str =
    `Good day, {title} {name}.`

@test_formal tests @formal_greet () -> void = run(
    assert_eq(
        actual: formal_greet(title: "Dr.", name: "Smith"),
        expected: "Good day, Dr. Smith.",
    ),
)

// Program entry point
@main () -> void = run(
    let $names = ["Alice", "Bob", "Charlie"],
    for name in names do run(
        print(msg: greet(name: name)),
    ),
)
```

Save this as `greetings.ori` and run:

```bash
ori run greetings.ori
```

Output:

```
Running tests...
  test_greet ... ok
  test_formal ... ok

Hello, Alice!
Hello, Bob!
Hello, Charlie!
```

## Key Concepts Preview

Before moving on, here's what makes Ori distinctive:

### Everything Is an Expression

There are no statements. Everything returns a value:

```ori
// if/else returns a value
let status = if age >= 18 then "adult" else "minor"

// run returns its last expression
let result = run(
    let x = compute(),
    let y = transform(input: x),
    x + y,  // This is the return value
)
```

### No Null, No Exceptions

Ori doesn't have `null` or exceptions. Instead:

- **Optional values** use `Option<T>`: either `Some(value)` or `None`
- **Operations that can fail** use `Result<T, E>`: either `Ok(value)` or `Err(error)`

You'll learn these in [Option and Result](/guide/07-option-result).

### Explicit Effects

Functions that do I/O must declare it:

```ori
@fetch_data (url: str) -> Result<str, Error> uses Http = ...
@save_file (path: str, data: str) -> Result<void, Error> uses FileSystem = ...
```

The `uses` clause makes side effects visible in the type signature. You'll learn more in [Capabilities](/guide/13-capabilities).

## Try It Yourself

Before continuing, try these exercises:

1. **Modify the greeting:** Change `greet` to say "Hi" instead of "Hello" and update the test

2. **Add a new function:** Write a `farewell` function that says "Goodbye, {name}!" with its test

3. **Combine functions:** Write a `conversation` function that uses both `greet` and `farewell`

4. **Work with numbers:** Write an `add` function that adds two integers, with tests for positive, negative, and zero

## What's Next

You now know enough to write basic Ori programs. Continue with:

- **[Language Basics](/guide/02-language-basics)** — Types, variables, operators, and control flow
- **[Functions](/guide/03-functions)** — Deep dive into function definitions, generics, and lambdas
