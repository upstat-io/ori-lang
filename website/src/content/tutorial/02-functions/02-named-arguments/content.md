---
title: "Named Arguments"
order: 2
description: "All function arguments are named at the call site"
---

# Named Arguments

In Ori, **all arguments are named at the call site**. This makes function calls self-documenting:

```ori
@create_user (name: str, age: int) -> str =
    name + " (age " + str(age) + ")";

// Calling: every argument is labeled
create_user(name: "Alice", age: 30);
```

This eliminates confusion about argument order and makes code more readable. You can also call arguments in any order:

```ori
create_user(age: 30, name: "Alice");  // same result
```

## Your task

Write a function `@format_date` that takes `year: int`, `month: int`, and `day: int` and returns a string in the format `"YYYY-MM-DD"`. Use `str()` to convert numbers.

For simplicity, don't worry about zero-padding single digits.
