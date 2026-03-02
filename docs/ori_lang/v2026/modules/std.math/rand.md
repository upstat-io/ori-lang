# std.math.rand

Random number generation.

```ori
use std.math.rand { random, random_int, random_choice, shuffle }
```

**Capability required:** `Random`

---

## Overview

The `std.math.rand` module provides:

- Random number generation
- Random selection from collections
- Shuffling

---

## The Random Capability

```ori
trait Random {
    @int (min: int, max: int) -> int
    @float () -> float
    @bool () -> bool
    @bytes (n: int) -> [byte]
}
```

The `Random` capability represents access to random number generation. Functions that need randomness must declare `uses Random` in their signature.

```ori
@roll_dice () -> int uses Random =
    Random.int(1, 6)

@coin_flip () -> bool uses Random =
    Random.bool()
```

**Implementations:**

| Type | Description |
|------|-------------|
| `SystemRandom` | Cryptographically secure RNG (default) |
| `SeededRandom` | Reproducible RNG with seed |
| `MockRandom` | Fixed sequence for testing |

### MockRandom

For testing randomness-dependent code:

```ori
type MockRandom = {
    int_value: int,
    float_value: float,
    bool_value: bool,
}

impl Random for MockRandom {
    @int (min: int, max: int) -> int = self.int_value
    @float () -> float = self.float_value
    @bool () -> bool = self.bool_value
    @bytes (n: int) -> [byte] = [0].repeat(n)
}
```

```ori
@test_dice_roll tests @roll_dice () -> void =
    with Random = MockRandom { int_value: 4, float_value: 0.0, bool_value: false } in
    {
        let result = roll_dice()
        assert_eq(
            .actual: result,
            .expected: 4,
        )
    }
```

### SeededRandom

For reproducible random sequences:

```ori
type SeededRandom = {
    seed: int,
}

// Same seed always produces same sequence
let rng = SeededRandom { seed: 12345 }
```

---

## Functions

### @random

```ori
@random () -> float uses Random
```

Returns a random float between 0.0 (inclusive) and 1.0 (exclusive).

```ori
use std.math.rand { random }

let r = random()  // e.g., 0.7234...
```

---

### @random_int

```ori
@random_int (min: int, max: int) -> int uses Random
```

Returns a random integer in the range [min, max] (inclusive).

```ori
use std.math.rand { random_int }

let dice = random_int(1, 6)  // 1, 2, 3, 4, 5, or 6
let coin = random_int(0, 1)  // 0 or 1
```

---

### @random_bool

```ori
@random_bool () -> bool uses Random
```

Returns a random boolean.

```ori
use std.math.rand { random_bool }

if random_bool() then "heads" else "tails"
```

---

### @random_choice

```ori
@random_choice<T> (items: [T]) -> Option<T> uses Random
```

Returns a random element from a list, or None if empty.

```ori
use std.math.rand { random_choice }

let colors = ["red", "green", "blue"]
let color = random_choice(colors) ?? "default"
```

---

### @shuffle

```ori
@shuffle<T> (items: [T]) -> [T] uses Random
```

Returns a new list with elements in random order.

```ori
use std.math.rand { shuffle }

let deck = shuffle([1, 2, 3, 4, 5])
// e.g., [3, 1, 5, 2, 4]
```

---

### @random_bytes

```ori
@random_bytes (n: int) -> [byte] uses Random
```

Returns n cryptographically random bytes.

```ori
use std.math.rand { random_bytes }

let token = random_bytes(32)  // 32 random bytes
```

---

## Examples

### Generating a random password

```ori
use std.math.rand { random_choice }

@random_password (length: int) -> str uses Random = {
    let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%"
    let char_list = chars.chars()

    collect(
        .range: 0..length,
        .map: _ -> random_choice(char_list) ?? 'a',
    ).join("")
}
```

### Weighted random selection

```ori
use std.math.rand { random }

@weighted_choice<T> (items: [(T, float)]) -> Option<T> uses Random = {
    let total = items.map((_, w) -> w).sum()
    let r = random() * total

    // Fold with cumulative weight, break when threshold exceeded
    items.fold(
        initial: (0.0, None),
        f: (state, entry) -> match state {
            (_, Some(_)) -> state  // Already found, skip rest
            (cumulative, None) -> {
                let (item, weight) = entry
                let new_cumulative = cumulative + weight
                if r < new_cumulative then (new_cumulative, Some(item))
                else (new_cumulative, None)
            }
        },
    ).1  // Extract the Option<T> from tuple
}
```

### Simulating dice rolls

```ori
use std.math.rand { random_int }

@roll_dice (count: int, sides: int) -> [int] uses Random =
    collect(
        .range: 0..count,
        .map: _ -> random_int(1, sides),
    )

@roll_with_advantage () -> int uses Random = {
    let rolls = roll_dice(2, 20)

    max(rolls[0], rolls[1])
}
```

---

## See Also

- [std.math](index.md) — Mathematical functions
- [std.crypto](../std.crypto/) — Cryptographic functions
