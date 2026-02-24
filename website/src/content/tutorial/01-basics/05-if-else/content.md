---
title: "If / Else"
order: 5
description: "Conditional expressions with if/then/else"
---

# Conditional Expressions

In Ori, `if` is an **expression** — it produces a value. The syntax uses `then` and `else` keywords:

```ori
let $status = if score >= 60 then "pass" else "fail";
```

For multi-line bodies, use blocks:

```ori
let $message = if temperature > 30 then {
    "It's hot!"
} else {
    "It's comfortable."
};
```

Since `if` is an expression, you can use it anywhere a value is expected — in assignments, function arguments, or even inside other expressions.

## Your task

Write a function `@classify` that takes an `age: int` and returns a string:
- If age < 13, return `"child"`
- If age < 18, return `"teenager"`
- Otherwise, return `"adult"`

The `@main` function will test it with three values.
