---
title: "Variables"
order: 2
description: "Bind values with let and $"
---

# Variables

In Ori, you create variables with `let`. By default, variables are **mutable** — you can reassign them:

```ori
let name = "Ori";
let age = 1;
age = age + 1;
```

To make a variable **immutable**, prefix the name with `$`:

```ori
let $name = "Ori";
// name = "other";  // compile error! $ means immutable
```

Ori infers the type automatically, but you can be explicit:

```ori
let $name: str = "Ori";
```

Note that statements end with `;` — the last expression in a block (the block's value) does not.

## Your task

1. Create a variable `language` with the value `"Ori"`
2. Create a variable `version` with the value `1`
3. Print both using string concatenation (`+` joins strings, `str()` converts numbers)
