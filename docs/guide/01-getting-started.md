---
title: "Getting Started"
description: "Install Ori and write your first program."
order: 1
---

> **Early Development Notice:** Ori is under active development. Many features are still evolving, with some in early prototype stages. Syntax, semantics, and APIs are subject to change.

# Getting Started

Ori is a general-purpose language where **if your code compiles, it works**. The compiler enforces testing, tracks dependencies, and makes side effects explicit.

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
@main () -> void = print(msg: "Hello, World!");
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

Let's break down `@main () -> void = print(msg: "Hello, World!");`:

```ori
@main () -> void = print(msg: "Hello, World!");
|     |     |    | |                            |
|     |     |    | └─ Function body              └─ ; ends top-level declaration
|     |     |    └─── Body follows
|     |     └──────── Returns nothing (void)
|     └────────────── Takes no parameters
└──────────────────── Function named "main"
```

### The `@` Sigil

In Ori, **functions are declared with `@`**:

```ori
@greet (name: str) -> str = `Hello, {name}!`;
@add (a: int, b: int) -> int = a + b;
@main () -> void = print(msg: "Starting...");
```

This visual distinction makes functions immediately recognizable in your code. When you see `@`, you know it's a function declaration.

### Named Arguments

Notice we wrote `print(msg: "Hello, World!")` not `print("Hello, World!")`. In Ori, **all function arguments must be named**:

```ori
print(msg: "Hello");              // Correct
add(a: 2, b: 3);                  // Correct
greet(name: "Alice");             // Correct

print("Hello");                   // ERROR: missing argument name
add(2, 3);                        // ERROR: missing argument names
```

This might feel verbose at first, but it has real benefits:

**Self-documenting code:**
```ori
// What do these arguments mean?
create_user("Alice", 30, true, false);

// vs. named arguments (actual Ori code):
create_user(name: "Alice", age: 30, admin: true, verified: false);
```

**Argument order doesn't matter:**
```ori
// These are equivalent:
create_user(name: "Alice", age: 30);
create_user(age: 30, name: "Alice");
```

**Catches mistakes at compile time:**
```ori
// You can't accidentally swap similar-typed arguments
send_email(from: alice, to: bob);   // Clear intent
send_email(to: bob, from: alice);   // Same result, still clear
```

### Template Strings

Backtick strings support interpolation with `{...}`:

```ori
let name = "Alice";
let greeting = `Hello, {name}!`;    // "Hello, Alice!"

let a = 10;
let b = 20;
let result = `{a} + {b} = {a + b}`; // "10 + 20 = 30"
```

Regular strings use double quotes and don't support interpolation:

```ori
let plain = "Hello, World!";
let escaped = "Line 1\nLine 2";
```

## Variables and Bindings

Let's make our program more interesting. Update `hello.ori`:

```ori
@main () -> void = {
    let name = "World";
    print(msg: `Hello, {name}!`);
}
```

### Block Expressions

When a function needs multiple steps, use a block `{ }`:

```ori
@main () -> void = {
    let first = "Hello";
    let second = "World";
    print(msg: `{first}, {second}!`);
}
```

Statements are terminated with `;`. The last expression (without `;`) becomes the function's return value. In a void function, all expressions have `;`.

### Variables with `let`

Create variables with `let`:

```ori
let name = "Alice";
let age = 30;
let score = 95.5;
```

Ori infers the type automatically. You can be explicit if you prefer:

```ori
let name: str = "Alice";
let age: int = 30;
let score: float = 95.5;
```

### Immutable Bindings with `$`

Sometimes you want to ensure a value never changes. Use `$`:

```ori
let $max_retries = 3;       // Cannot be reassigned
let counter = 0;            // Can be reassigned

counter = counter + 1;      // OK
max_retries = 5;            // ERROR: cannot reassign immutable binding
```

**When to use `$`:**
- Configuration values
- Constants
- Values that changing would be a bug

**Rule of thumb:** Start with `$`. Remove it only when you need reassignment.

## Writing Your First Test

Let's write a function that does something useful:

```ori
@greet (name: str) -> str = `Hello, {name}!`;

@main () -> void = {
    let message = greet(name: "Alice");
    print(msg: message);
}
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
 1 | @greet (name: str) -> str = `Hello, {name}!`;
   | ^^^^^ untested function
```

This is Ori's **mandatory testing** at work. Every function needs at least one test.

Add a test for `greet`:

```ori
use std.testing { assert_eq };

@greet (name: str) -> str = `Hello, {name}!`;

@test_greet tests @greet () -> void = {
    assert_eq(actual: greet(name: "Alice"), expected: "Hello, Alice!");
    assert_eq(actual: greet(name: "Bob"), expected: "Hello, Bob!");
}

@main () -> void = {
    let message = greet(name: "Alice");
    print(msg: message);
}
```

Let's understand the test:

```ori
@test_greet tests @greet () -> void = { ... }
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
@main () -> void = ...;

// Return an exit code (0 = success)
@main () -> int = ...;

// Accept command-line arguments
@main (args: [str]) -> void = ...;

// Both: args and exit code
@main (args: [str]) -> int = ...;
```

### Working with Command-Line Arguments

> **Coming Soon:** The `args` parameter for `@main` is planned but not yet implemented. The examples below show the intended syntax.

```ori
@main (args: [str]) -> void = {
    if is_empty(collection: args) then {
        print(msg: "No arguments provided");
    } else {
        print(msg: `Got {len(collection: args)} arguments:`);
        for arg in args do print(msg: `  - {arg}`);
    };
}
```

Run with:

```bash
ori run program.ori -- first second third
```

Note: `args` contains only the arguments, not the program name.

### Exit Codes

> **Coming Soon:** Exit code support via `@main () -> int` is planned but not yet implemented. The examples below show the intended syntax.

```ori
@main (args: [str]) -> int =
    if is_empty(collection: args) then {
        print(msg: "Error: no arguments provided");
        1  // no semicolon: this is the block's value (exit code)
    } else {
        print(msg: `Processing {len(collection: args)} items`);
        0  // no semicolon: this is the block's value (exit code)
    }
```

## The Complete Example

Here's everything we've covered in one program:

```ori
use std.testing { assert_eq };

// A function that creates a greeting
@greet (name: str) -> str = `Hello, {name}!`;

// Test for greet - required for compilation
@test_greet tests @greet () -> void = {
    assert_eq(actual: greet(name: "Alice"), expected: "Hello, Alice!");
    assert_eq(actual: greet(name: "Bob"), expected: "Hello, Bob!");
    assert_eq(actual: greet(name: ""), expected: "Hello, !");
}

// A function that creates a formal greeting
@formal_greet (title: str, name: str) -> str =
    `Good day, {title} {name}.`;

@test_formal tests @formal_greet () -> void = {
    assert_eq(
        actual: formal_greet(title: "Dr.", name: "Smith"),
        expected: "Good day, Dr. Smith.",
    );
}

// Program entry point
@main () -> void = {
    let $names = ["Alice", "Bob", "Charlie"];
    for name in names do {
        print(msg: greet(name: name));
    };
}
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
let status = if age >= 18 then "adult" else "minor";

// blocks return their last expression (no trailing ;)
let result = {
    let x = compute();
    let y = transform(input: x);

    x + y  // no semicolon: this is the block's value
};
```

### No Null, No Exceptions

Ori doesn't have `null` or exceptions. Instead:

- **Optional values** use `Option<T>`: either `Some(value)` or `None`
- **Operations that can fail** use `Result<T, E>`: either `Ok(value)` or `Err(error)`

You'll learn these in [Option and Result](/guide/07-option-result).

### Explicit Effects

Functions that do I/O must declare it:

```ori
@fetch_data (url: str) -> Result<str, Error> uses Http = ...;
@save_file (path: str, data: str) -> Result<void, Error> uses FileSystem = ...;
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
