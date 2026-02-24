---
title: "Defining Functions"
order: 1
description: "Create reusable functions with @"
---

# Defining Functions

Functions in Ori are declared with the `@` prefix. The body is an expression — the last expression becomes the return value (there is no `return` keyword):

```ori
@greet (name: str) -> str = "Hello, " + name + "!";
```

For multi-line functions, use a block. Blocks don't need a trailing `;`:

```ori
@add (a: int, b: int) -> int = {
    let $sum = a + b;
    sum
}
```

Note: `let` statements end with `;`, but the last expression in a block (the return value) does **not**.

## Your task

Write a function `@square` that takes `n: int` and returns `n * n`. Then write `@sum_of_squares` that takes `a: int` and `b: int` and returns the sum of their squares.
