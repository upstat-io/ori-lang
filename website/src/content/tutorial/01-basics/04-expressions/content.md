---
title: "Expressions"
order: 4
description: "Everything in Ori is an expression"
---

# Everything Is an Expression

Ori is **expression-based** — every construct produces a value. There is no `return` keyword. The last expression in a block becomes its value:

```ori
let $result = {
    let $a = 10;
    let $b = 20;
    a + b
};
// result is 30
```

Notice that `let` statements end with `;`, but the last expression (`a + b`) does **not** — that's the block's value. The outer `let $result = { ... };` also ends with `;` because it's a statement.

This applies everywhere — `if/else`, blocks, even function bodies.

## Your task

Create a block that computes the area of a rectangle with width 7 and height 3. Store the result in a variable called `area` and print it.
