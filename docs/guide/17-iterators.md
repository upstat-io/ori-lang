---
title: "Iterators"
description: "Functional iteration, transformation, and collection processing."
order: 17
part: "Abstraction"
---

# Iterators

Iterators provide a uniform way to traverse collections. They're at the heart of Ori's functional data processing.

## The Iterator Trait

```ori
trait Iterator {
    type Item
    @next (self) -> (Option<Self.Item>, Self)
}
```

Key insight: `next` returns both the next value AND the new iterator state. This enables immutable iteration.

## Creating Iterators

Every collection has an `.iter()` method:

```ori
let numbers = [1, 2, 3, 4, 5]
let iter = numbers.iter()

// Manually iterate
let (first, iter) = iter.next()   // (Some(1), ...)
let (second, iter) = iter.next()  // (Some(2), ...)
```

In practice, you'll use higher-level methods instead of calling `next` directly.

## Transformation Methods

### map — Transform Each Element

```ori
let numbers = [1, 2, 3, 4, 5]
let doubled = numbers.iter()
    .map(transform: x -> x * 2)
    .collect()  // [2, 4, 6, 8, 10]
```

### filter — Keep Matching Elements

```ori
let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
let evens = numbers.iter()
    .filter(predicate: x -> x % 2 == 0)
    .collect()  // [2, 4, 6, 8, 10]
```

### flat_map — Transform and Flatten

```ori
let words = ["hello", "world"]
let chars = words.iter()
    .flat_map(transform: word -> word.chars())
    .collect()  // ['h', 'e', 'l', 'l', 'o', 'w', 'o', 'r', 'l', 'd']
```

### take — First N Elements

```ori
let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
let first_three = numbers.iter()
    .take(count: 3)
    .collect()  // [1, 2, 3]
```

### skip — Skip First N Elements

```ori
let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
let after_three = numbers.iter()
    .skip(count: 3)
    .collect()  // [4, 5, 6, 7, 8, 9, 10]
```

### Combining take and skip

```ori
let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
let middle = numbers.iter()
    .skip(count: 2)
    .take(count: 5)
    .collect()  // [3, 4, 5, 6, 7]
```

## Reduction Methods

### fold — Reduce to Single Value

```ori
let numbers = [1, 2, 3, 4, 5]
let sum = numbers.iter()
    .fold(initial: 0, op: (acc, x) -> acc + x)  // 15

let product = numbers.iter()
    .fold(initial: 1, op: (acc, x) -> acc * x)  // 120
```

### count — Count Elements

```ori
let numbers = [1, 2, 3, 4, 5]
let count = numbers.iter().count()  // 5

let even_count = numbers.iter()
    .filter(predicate: x -> x % 2 == 0)
    .count()  // 2
```

### any — Check If Any Match

```ori
let numbers = [1, 2, 3, 4, 5]
numbers.iter().any(predicate: x -> x > 3)   // true
numbers.iter().any(predicate: x -> x > 10)  // false
```

### all — Check If All Match

```ori
let numbers = [1, 2, 3, 4, 5]
numbers.iter().all(predicate: x -> x > 0)   // true
numbers.iter().all(predicate: x -> x > 3)   // false
```

## Selection Methods

### find — First Matching Element

```ori
let numbers = [1, 2, 3, 4, 5]
let found = numbers.iter()
    .find(predicate: x -> x > 3)  // Some(4)

let not_found = numbers.iter()
    .find(predicate: x -> x > 10)  // None
```

### last — Last Element

```ori
let numbers = [1, 2, 3, 4, 5]
let last = numbers.iter().last()  // Some(5)
```

## Combining Iterators

### zip — Combine Two Iterators

```ori
let names = ["Alice", "Bob", "Charlie"]
let ages = [30, 25, 35]

let people = names.iter()
    .zip(other: ages.iter())
    .map(transform: (name, age) -> `{name} is {age}`)
    .collect()  // ["Alice is 30", "Bob is 25", "Charlie is 35"]
```

### chain — Concatenate Iterators

```ori
let first = [1, 2, 3]
let second = [4, 5, 6]

let combined = first.iter()
    .chain(other: second.iter())
    .collect()  // [1, 2, 3, 4, 5, 6]
```

### enumerate — Add Indices

```ori
let items = ["a", "b", "c"]

for (index, item) in items.iter().enumerate() do
    print(msg: `{index}: {item}`)
// 0: a
// 1: b
// 2: c
```

### flatten — Flatten Nested Iterators

```ori
let nested = [[1, 2], [3, 4], [5, 6]]

let flat = nested.iter()
    .flatten()
    .collect()  // [1, 2, 3, 4, 5, 6]
```

### cycle — Repeat Infinitely

```ori
let pattern = [1, 2, 3]

let repeated = pattern.iter()
    .cycle()
    .take(count: 7)
    .collect()  // [1, 2, 3, 1, 2, 3, 1]
```

## DoubleEndedIterator

Some iterators can traverse from both ends:

```ori
trait DoubleEndedIterator: Iterator {
    @next_back (self) -> (Option<Self.Item>, Self)
}
```

This enables:

### rev — Reverse Iteration

```ori
let numbers = [1, 2, 3, 4, 5]
let reversed = numbers.iter()
    .rev()
    .collect()  // [5, 4, 3, 2, 1]
```

### rfind — Find from Back

```ori
let numbers = [1, 2, 3, 4, 5]
let last_even = numbers.iter()
    .rfind(predicate: x -> x % 2 == 0)  // Some(4)
```

### rfold — Fold from Back

```ori
let numbers = [1, 2, 3]
let result = numbers.iter()
    .rfold(initial: "", op: (acc, x) -> `{acc}{x}`)  // "321"
```

## Chaining Operations

The real power of iterators is chaining:

```ori
type Transaction = { amount: float, category: str }

@total_by_category (transactions: [Transaction], category: str) -> float =
    transactions.iter()
        .filter(predicate: t -> t.category == category)
        .map(transform: t -> t.amount)
        .fold(initial: 0.0, op: (sum, amount) -> sum + amount)

@test_total_by_category tests @total_by_category () -> void = {
    let transactions = [
        Transaction { amount: 100.0, category: "food" }
        Transaction { amount: 50.0, category: "transport" }
        Transaction { amount: 75.0, category: "food" }
    ]
    assert_eq(actual: total_by_category(transactions: transactions, category: "food"), expected: 175.0)
}

@top_transactions (transactions: [Transaction], n: int) -> [Transaction] =
    transactions.iter()
        .filter(predicate: t -> t.amount > 0.0)
        .collect()
        .sort_by(key: t -> -t.amount)  // Descending
        .iter()
        .take(count: n)
        .collect()
```

## Lazy Evaluation

Iterators are lazy — they don't compute until you consume them:

```ori
let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]

// This does almost no work — just sets up the pipeline
let pipeline = numbers.iter()
    .map(transform: x -> {
        print(msg: `Processing {x}`)
        x * 2
    })
    .filter(predicate: x -> x > 10)

// Work happens here when we consume the iterator
let result = pipeline.take(count: 2).collect()
// Prints: Processing 1, Processing 2, ..., Processing 6
// Result: [12, 14]
```

Notice only the elements needed are processed.

## The Iterable Trait

Types that can be iterated implement `Iterable`:

```ori
trait Iterable {
    type Item
    @iter (self) -> impl Iterator
}
```

All standard collections implement this:
- `[T]` — lists
- `{K: V}` — maps (iterates over `(K, V)` tuples)
- `Set<T>` — sets
- Ranges

## The Collect Trait

Convert iterators back to collections:

```ori
trait Collect<T> {
    @from_iter (iter: impl Iterator) -> Self
}
```

The `.collect()` method uses type inference:

```ori
let numbers = [1, 2, 3, 4, 5]

// Collect to list (inferred)
let doubled: [int] = numbers.iter().map(transform: x -> x * 2).collect()

// Collect to set
let unique: Set<int> = [1, 2, 2, 3, 3, 3].iter().collect()
```

## Custom Iterators

Create your own iterator by implementing the trait:

```ori
type Counter = { current: int, max: int }

impl Iterator for Counter {
    type Item = int

    @next (self) -> (Option<int>, Counter) = {
        if self.current >= self.max then
            (None, self)
        else
            (Some(self.current), Counter { current: self.current + 1, max: self.max })
    }
}

@count_from (start: int, end: int) -> Counter =
    Counter { current: start, max: end }

// Use like any iterator
let numbers = count_from(start: 1, end: 5).collect()  // [1, 2, 3, 4]

@test_counter tests @count_from () -> void = {
    let counter = count_from(start: 0, end: 3)
    let result = counter.collect()
    assert_eq(actual: result, expected: [0, 1, 2])
}
```

## Fused Guarantee

Ori iterators are fused: once `next()` returns `None`, it always returns `None`:

```ori
let numbers = [1, 2]
let iter = numbers.iter()

let (a, iter) = iter.next()  // Some(1)
let (b, iter) = iter.next()  // Some(2)
let (c, iter) = iter.next()  // None
let (d, iter) = iter.next()  // None (always)
```

## For Loop Desugaring

For loops desugar to iterator operations:

```ori
// This for loop
for x in items do
    process(x: x)

// Desugars to
let iter = items.iter()
loop {
    let (maybe_x, iter) = iter.next()
    match maybe_x {
        Some(x) -> process(x: x)
        None -> break
    }
}
```

And `for...yield`:

```ori
// This for loop
let result = for x in items yield x * 2

// Desugars to
let result = items.iter().map(transform: x -> x * 2).collect()
```

## Common Patterns

### Pagination

```ori
@paginate<T> (items: [T], page: int, page_size: int) -> [T] =
    items.iter()
        .skip(count: page * page_size)
        .take(count: page_size)
        .collect()

@test_paginate tests @paginate () -> void = {
    let items = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    assert_eq(actual: paginate(items: items, page: 0, page_size: 3), expected: [1, 2, 3])
    assert_eq(actual: paginate(items: items, page: 1, page_size: 3), expected: [4, 5, 6])
    assert_eq(actual: paginate(items: items, page: 2, page_size: 3), expected: [7, 8, 9])
}
```

### Grouping

```ori
@group_by<T, K: Eq + Hashable> (items: [T], key: (T) -> K) -> {K: [T]} = {
    let groups: {K: [T]} = {}
    for item in items do {
        let k = key(item)
        let current = groups[k] ?? []
        groups = { ...groups, k: [...current, item] }
    }
    groups
}
```

### Windowing

```ori
@windows<T: Clone> (items: [T], size: int) -> [[T]] = {
    if size <= 0 || size > len(collection: items) then return []

    for i in 0..(len(collection: items) - size + 1) yield
        items.iter()
            .skip(count: i)
            .take(count: size)
            .collect()
}

@test_windows tests @windows () -> void = {
    let items = [1, 2, 3, 4, 5]
    assert_eq(actual: windows(items: items, size: 3), expected: [[1, 2, 3], [2, 3, 4], [3, 4, 5]])
}
```

### Deduplication

```ori
@dedupe<T: Eq> (items: [T]) -> [T] = {
    let seen: [T] = []
    for item in items do
        if !seen.iter().any(predicate: x -> x == item) then
            seen = [...seen, item]
    seen
}
```

## Complete Example

```ori
type Order = {
    id: int,
    customer: str,
    items: [OrderItem],
    status: OrderStatus,
}

type OrderItem = { product: str, quantity: int, price: float }

type OrderStatus = Pending | Shipped | Delivered | Cancelled

type OrderSummary = {
    total_orders: int,
    total_revenue: float,
    orders_by_status: {str: int},
    top_customers: [(str, float)],
}

@order_total (order: Order) -> float =
    order.items.iter()
        .map(transform: item -> item.quantity as float * item.price)
        .fold(initial: 0.0, op: (a, b) -> a + b)

@test_order_total tests @order_total () -> void = {
    let order = Order {
        id: 1
        customer: "Alice"
        items: [
            OrderItem { product: "Widget", quantity: 2, price: 10.0 }
            OrderItem { product: "Gadget", quantity: 1, price: 25.0 }
        ]
        status: Pending
    }
    assert_eq(actual: order_total(order: order), expected: 45.0)
}

@status_to_str (status: OrderStatus) -> str = match status {
    Pending -> "pending"
    Shipped -> "shipped"
    Delivered -> "delivered"
    Cancelled -> "cancelled"
}

@summarize_orders (orders: [Order]) -> OrderSummary = {
    // Filter out cancelled orders for revenue
    let active_orders = orders.iter()
        .filter(predicate: o -> match o.status { Cancelled -> false, _ -> true})
        .collect()

    // Calculate total revenue
    let total_revenue = active_orders.iter()
        .map(transform: o -> order_total(order: o))
        .fold(initial: 0.0, op: (a, b) -> a + b)

    // Count by status
    let status_counts: {str: int} = {}
    for order in orders do {
        let key = status_to_str(status: order.status)
        let current = status_counts[key] ?? 0
        status_counts = { ...status_counts, key: current + 1 }
    }

    // Top customers by spending
    let customer_spending: {str: float} = {}
    for order in active_orders do {
        let total = order_total(order: order)
        let current = customer_spending[order.customer] ?? 0.0
        customer_spending = { ...customer_spending, order.customer: current + total }
    }

    let top_customers = customer_spending.iter()
        .collect()
        .sort_by(key: (_, spending) -> -spending)
        .iter()
        .take(count: 5)
        .collect()

    OrderSummary {
        total_orders: len(collection: orders)
        total_revenue
        orders_by_status: status_counts
        top_customers
    }
}

@test_summarize_orders tests @summarize_orders () -> void = {
    let orders = [
        Order {
            id: 1
            customer: "Alice"
            items: [OrderItem { product: "A", quantity: 1, price: 100.0 }]
            status: Delivered
        }
        Order {
            id: 2
            customer: "Bob"
            items: [OrderItem { product: "B", quantity: 2, price: 50.0 }]
            status: Shipped
        }
        Order {
            id: 3
            customer: "Alice"
            items: [OrderItem { product: "C", quantity: 1, price: 75.0 }]
            status: Cancelled
        }
    ]

    let summary = summarize_orders(orders: orders)
    assert_eq(actual: summary.total_orders, expected: 3)
    assert_eq(actual: summary.total_revenue, expected: 200.0),  // Excluding cancelled
}
```

## Quick Reference

### Transform

```ori
.map(transform: x -> ...)
.filter(predicate: x -> ...)
.flat_map(transform: x -> ...)
.take(count: n)
.skip(count: n)
```

### Reduce

```ori
.fold(initial: val, op: (acc, x) -> ...)
.count()
.any(predicate: x -> ...)
.all(predicate: x -> ...)
```

### Find

```ori
.find(predicate: x -> ...)
.last()
```

### Combine

```ori
.zip(other: iter)
.chain(other: iter)
.enumerate()
.flatten()
.cycle()
```

### Reverse (DoubleEndedIterator)

```ori
.rev()
.rfind(predicate: x -> ...)
.rfold(initial: val, op: (acc, x) -> ...)
```

### Collect

```ori
.collect()
```

## What's Next

Now that you understand iterators:

- **[Extensions](/guide/18-extensions)** — Adding methods to existing traits
- **[Compiler Patterns](/guide/19-compiler-patterns)** — Advanced pattern usage

