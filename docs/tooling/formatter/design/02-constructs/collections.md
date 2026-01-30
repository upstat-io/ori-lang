---
title: "Collections"
description: "Ori Formatter Design — Collection Formatting"
order: 5
section: "Constructs"
---

# Collections

Formatting rules for collections: lists, maps, tuples, struct literals, and ranges.

## Lists

### Inline If Fits

```ori
let nums = [1, 2, 3, 4, 5]
let names = ["alice", "bob", "charlie"]
let empty = []
```

### Simple Items — Wrap Multiple Per Line

**Simple items** are literals (integers, floats, strings) and identifiers. When a list of simple items exceeds 100 characters, wrap multiple items per line:

```ori
let numbers = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    21, 22, 23, 24, 25,
]

let names = [
    "alpha", "beta", "gamma", "delta", "epsilon",
    "zeta", "eta", "theta", "iota", "kappa",
]
```

### Complex Items — One Per Line

**Complex items** are structs, function calls, and nested collections. When a list contains complex items, format one item per line:

```ori
let users = [
    User { id: 1, name: "Alice" },
    User { id: 2, name: "Bob" },
    User { id: 3, name: "Charlie" },
]

let tasks = [
    fetch_user(id: 1),
    fetch_user(id: 2),
    fetch_user(id: 3),
]

let nested = [
    [1, 2, 3],
    [4, 5, 6],
    [7, 8, 9],
]
```

### List in Function Arguments

Same rules apply inside function calls:

```ori
// Short list stays inline
process(items: [1, 2, 3, 4, 5])

// Long list wraps
process(
    items: [
        "first", "second", "third", "fourth", "fifth",
        "sixth", "seventh", "eighth", "ninth", "tenth",
    ],
)
```

### Spread Operator

```ori
let combined = [...first, ...second, ...third]

let extended = [
    ...existing_items,
    new_item_one,
    new_item_two,
]
```

### Empty List

No space inside:

```ori
let empty = []
```

## Maps

### Inline If Fits

Short maps (typically ≤2 entries) stay inline:

```ori
let scores = {"alice": 100, "bob": 95}
let config = {"timeout": 30, "retries": 3}
let empty = {}
```

### One Entry Per Line

When a map exceeds 100 characters, break to one entry per line:

```ori
let user_scores = {
    "alice": 100,
    "bob": 95,
    "charlie": 87,
    "diana": 92,
}

let config = {
    "timeout": 30,
    "max_retries": 3,
    "base_url": "https://api.example.com",
    "debug_mode": false,
}
```

### Map with Complex Values

```ori
let handlers = {
    "click": event -> handle_click(event),
    "keypress": event -> handle_keypress(event),
    "scroll": event -> handle_scroll(event),
}
```

### Spread Operator

```ori
let merged = {...defaults, ...overrides}

let updated = {
    ...existing_config,
    "timeout": 60,
    "debug_mode": true,
}
```

### Empty Map

No space inside:

```ori
let empty = {}
```

## Tuples

### Inline If Fits

```ori
let pair = (1, "hello")
let triple = (x, y, z)
let unit = ()
```

### One Element Per Line

When a tuple exceeds 100 characters:

```ori
let data = (
    first_very_long_value,
    second_very_long_value,
    third_very_long_value,
)
```

### Tuple Destructuring

```ori
let (first, second) = pair
let (x, y, z) = coordinates

let (
    user_id,
    session_token,
    expiration_time,
) = authenticate(credentials)
```

### Unit

No space inside:

```ori
let unit = ()
```

## Struct Literals

### Inline If Fits

```ori
let p = Point { x: 0, y: 0 }
let u = User { id: 1, name: "Alice", active: true }
```

### One Field Per Line

When a struct literal exceeds 100 characters:

```ori
let config = Config {
    timeout: 30s,
    max_retries: 3,
    base_url: "https://api.example.com",
    debug_mode: false,
}
```

### Field Shorthand

When variable name matches field name:

```ori
let p = Point { x, y }

let user = User {
    id,
    name,
    email,
    created_at: Timestamp.now(),
}
```

### Spread Operator

```ori
let updated = Point { ...original, x: 10 }

let new_config = Config {
    ...default_config,
    timeout: 60s,
    debug_mode: true,
}
```

### Empty Struct

No space inside:

```ori
let empty = Empty {}
```

## Ranges

### Always Inline

Ranges are always inline (they're short by nature):

```ori
let r = 0..10
let inclusive = 0..=100
let stepped = 0..100 by 5
let descending = 10..0 by -1
```

### In For Loops

```ori
for i in 0..10 do process(i)

for i in 0..=100 by 2 do
    print(msg: `Even: {i}`)
```

### Collected

```ori
let numbers = (0..10).collect()
let evens = (0..100 by 2).collect()
```

## Sets

Sets follow the same rules as lists:

### Inline If Fits

```ori
let s = Set.from([1, 2, 3])
```

### Wrapped If Long

```ori
let s = Set.from([
    "alpha", "beta", "gamma", "delta", "epsilon",
    "zeta", "eta", "theta", "iota", "kappa",
])
```

## Nesting Collections

Collections inside collections each follow their own rules:

```ori
// List of lists
let matrix = [
    [1, 2, 3],
    [4, 5, 6],
    [7, 8, 9],
]

// Map of lists
let groups = {
    "admins": ["alice", "bob"],
    "users": ["charlie", "diana", "eve"],
}

// List of structs
let points = [
    Point { x: 0, y: 0 },
    Point { x: 1, y: 1 },
    Point { x: 2, y: 2 },
]
```

Deeply nested but each level fits:

```ori
let config = {
    "database": { "host": "localhost", "port": 5432 },
    "cache": { "host": "localhost", "port": 6379 },
}
```

Each level breaks independently based on its own width:

```ori
let config = {
    "database": {
        "host": "production-db.example.com",
        "port": 5432,
        "pool_size": 20,
        "timeout": 30s,
    },
    "cache": { "host": "localhost", "port": 6379 },
}
```
