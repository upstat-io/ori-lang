---
title: "Lambdas"
order: 3
description: "Anonymous functions with the arrow syntax"
---

# Lambdas

Lambdas (anonymous functions) use the arrow syntax `->`:

```ori
// Single parameter
x -> x * 2

// Multiple parameters
(a, b) -> a + b
```

Lambdas are commonly used with list operations like `map`, `filter`, and `fold`:

```ori
let $numbers = [1, 2, 3, 4, 5];
let $doubled = numbers.map(transform: x -> x * 2);
// doubled is [2, 4, 6, 8, 10]
```

## Your task

Starting with the list `[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]`:

1. Use `.filter` to keep only even numbers (hint: `x % 2 == 0`)
2. Use `.map` to square each remaining number
3. Use `.fold` to sum all the values

Print the final result. (The answer should be `220`.)
