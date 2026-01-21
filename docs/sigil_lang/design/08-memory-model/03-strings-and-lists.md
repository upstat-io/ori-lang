# Strings and Lists

This document covers Sigil's handling of strings and lists: Small String Optimization (SSO), structural sharing for lists, and the distinction between value and reference types.

---

## Overview

Sigil optimizes strings and lists for immutable, functional programming:

| Type | Strategy | Benefit |
|------|----------|---------|
| `str` | Reference counted + SSO | Short strings avoid heap |
| `[T]` | Structural sharing | Efficient immutable operations |

```sigil
// Strings: short strings stored inline
name = "Alice"              // No heap allocation (5 bytes)
message = "This is a longer message..."  // Heap allocated

// Lists: structural sharing
list1 = [1, 2, 3]
list2 = list1 + [4]         // Shares [1,2,3] with list1
```

---

## String Handling

### Immutable Strings

All strings in Sigil are immutable:

```sigil
@example () -> void = run(
    greeting = "Hello",
    // greeting[0] = "h"   // ERROR: strings are immutable

    // Create new string instead
    modified = "h" + greeting.slice(1, len(greeting)),
    void
)
```

### Small String Optimization (SSO)

Short strings are stored inline, avoiding heap allocation:

**Inline storage (no heap):**
```
String value (24 bytes total):
+------------------------+
| length: 5              |  <- 1 byte
| flags: INLINE          |  <- 1 byte
| data: "Alice"          |  <- up to 22 bytes inline
+------------------------+
```

**Heap storage (longer strings):**
```
String value:                    Heap:
+------------------------+       +-------------------+
| length: 30             |       | refcount: 1       |
| flags: HEAP            |       | "This is a longer |
| ptr: ------------------|------>|  message..."      |
+------------------------+       +-------------------+
```

### SSO Threshold

Strings up to 22 bytes are stored inline:

| String | Bytes | Storage |
|--------|-------|---------|
| `"hi"` | 2 | Inline |
| `"Hello, World!"` | 13 | Inline |
| `"short identifier"` | 16 | Inline |
| `"exactly-22-characters"` | 21 | Inline |
| `"this string has 23 ch"` | 23 | Heap |

**Note:** The threshold is 22 bytes in UTF-8 encoding, not 22 characters. Multi-byte characters consume more space.

### Why SSO?

**Most strings are short:**
- Variable names, field names, identifiers
- Short messages, labels, keys
- Single words, abbreviations

**Benefits:**
- No heap allocation overhead
- Better cache locality
- Reduced memory fragmentation
- Faster creation and destruction

### String Operations

**Concatenation creates new strings:**
```sigil
@greet (name: str) -> str = "Hello, " + name + "!"
// Creates new string, original unchanged
```

**Common string operations:**
```sigil
@process (input: str) -> str = run(
    trimmed = input.trim(),          // New string
    lower = trimmed.lower(),         // New string
    replaced = lower.replace("a", "b"), // New string
    replaced
)
```

**String slicing:**
```sigil
@first_word (text: str) -> str = run(
    space_idx = text.find(" "),
    word = match(space_idx,
        Some(idx) -> text.slice(0, idx),
        None -> text
    ),
    word
)
```

### String Memory Layout

```sigil
// Small string - inline
small = "test"
```
```
Stack:
+------------------------+
| len: 4 | flags: INLINE |
| "test\0" (+ padding)   |
+------------------------+
```

```sigil
// Long string - heap allocated
long = "This is a much longer string that exceeds the inline threshold"
```
```
Stack:                          Heap:
+------------------------+      +----------------------------+
| len: 61 | flags: HEAP  |      | refcount: 1                |
| ptr: ------------------|----->| "This is a much longer..." |
+------------------------+      +----------------------------+
```

### String Sharing

When strings are assigned, short strings are copied and long strings share:

```sigil
@example () -> void = run(
    short = "hi",
    short_copy = short,  // Copied (inline - cheap)

    long = "This is a very long string that lives on the heap",
    long_ref = long,     // Shared (refcount incremented)

    void
)
```

---

## List Handling

### Immutable Lists

All lists in Sigil are immutable:

```sigil
@example () -> void = run(
    numbers = [1, 2, 3],
    // numbers[0] = 99    // ERROR: lists are immutable
    // numbers.push(4)    // ERROR: no mutation

    // Create new list instead
    updated = [99] + numbers.slice(1, len(numbers)),
    extended = numbers + [4],
    void
)
```

### Structural Sharing

When creating new lists from existing ones, Sigil shares unchanged portions:

```sigil
list1 = [1, 2, 3, 4, 5]
list2 = list1 + [6]      // list2 shares [1,2,3,4,5] with list1
```

**Memory layout with sharing:**
```
list1:                    Shared data:
+--------+               +---+---+---+---+---+
| root --|-------------->| 1 | 2 | 3 | 4 | 5 |
+--------+               +---+---+---+---+---+
                                              \
list2:                                         \
+--------+                                      +---+
| root --|------------------------------------->| 6 |
+--------+                                      +---+
```

### Why Structural Sharing?

**Immutability would be expensive without it:**
```sigil
// Without sharing: O(n) copy for every operation
@without_sharing (items: [int]) -> [int] = run(
    items = items + [1],   // Copy all, add 1
    items = items + [2],   // Copy all, add 2
    items = items + [3],   // Copy all, add 3
    items  // Total: 3 full copies
)

// With sharing: O(1) or O(log n) per operation
@with_sharing (items: [int]) -> [int] = run(
    items = items + [1],   // Share original, add node
    items = items + [2],   // Share previous, add node
    items = items + [3],   // Share previous, add node
    items  // Total: 3 small additions
)
```

**Functional patterns become efficient:**
```sigil
@efficient (items: [int]) -> [int] =
    // Each operation shares structure with input
    map(filter(items, n -> n > 0), n -> n * 2)
```

### Persistent Vector Implementation

Sigil lists use a persistent vector structure (similar to Clojure):

**Structure:**
- Tree of nodes with branching factor 32
- Leaf nodes contain actual elements
- Internal nodes contain pointers to children
- Path copying for modifications

**Complexity:**
| Operation | Complexity |
|-----------|------------|
| Access by index | O(log32 n) |
| Append | O(log32 n) |
| Prepend | O(log32 n) |
| Slice | O(log32 n) |
| Concatenate | O(log32 n) |
| Iteration | O(n) |

**Note:** log32(1,000,000) is approximately 4, so operations are effectively constant time.

### List Operations

**Creation:**
```sigil
empty = []
numbers = [1, 2, 3, 4, 5]
mixed = [1, "two", 3.0]  // ERROR: lists are homogeneous
```

**Append and prepend:**
```sigil
@example (items: [int]) -> [int] = run(
    with_end = items + [100],      // Append
    with_start = [0] + items,      // Prepend
    combined = [0] + items + [100], // Both
    combined
)
```

**Indexing:**
```sigil
@get_element (items: [int], idx: int) -> Option<int> =
    if idx >= 0 && idx < len(items) then Some(items[idx])
    else None
```

**Slicing:**
```sigil
@middle (items: [int]) -> [int] = run(
    length = len(items),
    start = length / 4,
    end = length * 3 / 4,
    items.slice(start, end)
)
```

### List Memory Layout

**Small list:**
```
List value:
+----------------+
| length: 5      |
| root: ---------|---> [1, 2, 3, 4, 5]  // Single leaf node
+----------------+
```

**Larger list (tree structure):**
```
List with 100 elements:
+----------------+
| length: 100    |
| root: ---------|---> Internal node
+----------------+           |
                    +--------+--------+--------+
                    |        |        |        |
                    v        v        v        v
                 [0-31]   [32-63]  [64-95]  [96-99]
```

### Sharing Example

```sigil
@demonstrate_sharing () -> void = run(
    // Original list
    original = [1, 2, 3, 4, 5],

    // Append - shares original
    appended = original + [6, 7],

    // Both lists valid, sharing structure
    assert_eq(original, [1, 2, 3, 4, 5]),
    assert_eq(appended, [1, 2, 3, 4, 5, 6, 7]),

    void
)
```

```
Memory:
                    Shared:
original --------> [1, 2, 3, 4, 5]
                         |
appended --------> [shares]--> [6, 7]
```

---

## Value vs Reference Types

### Automatic Selection

Sigil automatically chooses value or reference semantics:

| Type | Storage | Reasoning |
|------|---------|-----------|
| `int`, `float`, `bool` | Value | Primitive, fixed size |
| Small structs | Value | <= 32 bytes, primitives only |
| `str` | Hybrid (SSO) | Inline if short, ref if long |
| `[T]` | Reference | Variable size, sharing benefits |
| Large structs | Reference | > 32 bytes or contains references |

### Value Types

Copied when assigned or passed:

```sigil
type Point = { x: int, y: int }  // 16 bytes - value type

@example () -> void = run(
    p1 = { x: 10, y: 20 },
    p2 = p1,              // Copied (cheap for 16 bytes)
    // p1 and p2 are independent copies
    void
)
```

**Memory:**
```
Stack:
+-------------------+
| p1.x: 10          |
| p1.y: 20          |
+-------------------+
| p2.x: 10          |  <- Independent copy
| p2.y: 20          |
+-------------------+
```

### Reference Types

Shared when assigned, reference counted:

```sigil
@example () -> void = run(
    list1 = [1, 2, 3, 4, 5],
    list2 = list1,        // Shared (refcount = 2)
    // list1 and list2 reference same data
    void
)
```

**Memory:**
```
Stack:                     Heap:
+----------------+         +----------------+
| list1: ref ----|-------->| refcount: 2    |
+----------------+    /--->| [1, 2, 3, 4, 5]|
| list2: ref ----|----|    +----------------+
+----------------+
```

### Struct Classification

The compiler classifies structs automatically:

**Value type (copied):**
```sigil
// Small, all primitive fields
type Color = { r: int, g: int, b: int, a: int }  // 32 bytes
type Vector2 = { x: float, y: float }            // 16 bytes
type Bounds = { min: int, max: int }             // 16 bytes
```

**Reference type (shared):**
```sigil
// Contains reference types
type Person = {
    name: str,    // str is reference type
    age: int
}

// Too large
type Matrix4x4 = {
    // 16 floats = 128 bytes
    m00: float, m01: float, m02: float, m03: float,
    m10: float, m11: float, m12: float, m13: float,
    m20: float, m21: float, m22: float, m23: float,
    m30: float, m31: float, m32: float, m33: float
}

// Contains list
type Container = {
    items: [int]  // List is reference type
}
```

### Why Implicit Selection?

**AI doesn't need to decide:**
```sigil
// AI just writes natural code
type Point = { x: int, y: int }
type Data = { items: [int], name: str }

// Compiler chooses optimal representation
```

**Common pattern:**
- Small structs (coordinates, colors) -> value (fast copies)
- Large/complex structs -> reference (sharing)

**Following industry practice:**
- Swift: value types (struct) vs reference types (class)
- Go: small types copied, large types use pointers
- C#: struct vs class distinction

---

## Optimization Patterns

### String Building

**Inefficient - many allocations:**
```sigil
@build_string_bad (items: [str]) -> str =
    fold(items, "", (acc, item) -> acc + ", " + item)
// Each + creates new string
```

**Better - use join:**
```sigil
@build_string_good (items: [str]) -> str =
    join(items, ", ")
// Single allocation for result
```

### List Building

**Inefficient - many small appends:**
```sigil
@build_list_bad (n: int) -> [int] =
    fold(0..n, [], (acc, i) -> acc + [i])
// O(n log n) with structural sharing
```

**Better - use collect or map:**
```sigil
@build_list_good (n: int) -> [int] =
    collect(0..n, i -> i)
// Single construction pass

// Or with map:
@build_list_map (n: int) -> [int] =
    map(0..n, i -> i * 2)
```

### Avoiding Copies

**Unnecessary copy:**
```sigil
@example (data: LargeStruct) -> int = run(
    copy = data,           // Unnecessary if only reading
    result = compute(copy.field),
    result
)
```

**Direct access:**
```sigil
@example (data: LargeStruct) -> int =
    compute(data.field)    // Direct field access, no copy
```

---

## Memory Efficiency

### String Memory

| String length | Storage | Memory used |
|---------------|---------|-------------|
| 0-22 bytes | Inline | 24 bytes (fixed) |
| 23+ bytes | Heap | 24 bytes + string + refcount |

**Example:**
```sigil
short = "hi"        // 24 bytes total (inline)
medium = "hello world"  // 24 bytes total (inline)
long = "this is a longer string for demonstration"  // 24 + 42 + 8 = 74 bytes
```

### List Memory

| Elements | Approximate overhead |
|----------|---------------------|
| 0-32 | Single node + header |
| 33-1024 | 2 levels of nodes |
| 1025-32768 | 3 levels of nodes |
| 32769+ | 4+ levels of nodes |

**With structural sharing:**
```sigil
base = collect(0..1000000, i -> i)  // ~1M elements
derived = base + [1000000]           // Shares 99.99% with base
// Total memory: ~1M elements + 1 new leaf, not 2M elements
```

---

## Thread Safety

### String Thread Safety

- Short strings (inline): thread-safe (copied)
- Long strings (heap): atomic refcount operations
- String data: immutable, safe to share

### List Thread Safety

- Reference counting: atomic operations
- Tree structure: immutable nodes
- Safe to share between threads without locks

```sigil
// Safe: lists are immutable and thread-safe
@parallel_example (items: [int]) -> [int] = parallel(
    .evens: filter(items, n -> n % 2 == 0),
    .odds: filter(items, n -> n % 2 == 1),
    .doubled: map(items, n -> n * 2)
)
```

---

## Comparison with Other Languages

### Clojure

| Feature | Clojure | Sigil |
|---------|---------|-------|
| Lists | Persistent vector | Persistent vector |
| Strings | Java String | SSO + refcount |
| Sharing | Structural | Structural |
| Mutability | Immutable (default) | Always immutable |

### Rust

| Feature | Rust | Sigil |
|---------|------|-------|
| Lists | Vec (mutable) | Persistent vector |
| Strings | String (SSO in some impls) | SSO + refcount |
| Sharing | Explicit (Rc/Arc) | Automatic |
| Mutability | Opt-in | Always immutable |

### Swift

| Feature | Swift | Sigil |
|---------|-------|-------|
| Arrays | Copy-on-write | Structural sharing |
| Strings | SSO + COW | SSO + refcount |
| Value/ref | Explicit (struct/class) | Automatic |

---

## Best Practices

### 1. Use Appropriate Collection Operations

```sigil
// Good: use built-in operations
@process (items: [int]) -> [int] =
    map(filter(items, n -> n > 0), n -> n * 2)

// Avoid: manual iteration building lists
@process_manual (items: [int]) -> [int] =
    fold(items, [], (acc, n) ->
        if n > 0 then acc + [n * 2] else acc
    )
```

### 2. Prefer String Methods

```sigil
// Good: use string methods
@format (parts: [str]) -> str = join(parts, ", ")

// Avoid: manual concatenation in loop
@format_manual (parts: [str]) -> str =
    fold(parts, "", (acc, p) ->
        if acc == "" then p else acc + ", " + p
    )
```

### 3. Leverage Sharing

```sigil
// Efficient: base list shared
@variants (base: [int]) -> { a: [int], b: [int], c: [int] } = {
    a: base + [1],    // Shares base
    b: base + [2],    // Shares base
    c: base + [3]     // Shares base
}
```

### 4. Use Destructuring for Access

```sigil
// Good: destructure for multiple accesses
@process (items: [int]) -> int = run(
    [first, second, ..rest] = items,
    first + second + len(rest)
)

// Also fine: direct access for single use
@sum_first_two (items: [int]) -> int =
    items[0] + items[1]
```

---

## See Also

- [ARC Overview](01-arc-overview.md) - Reference counting details
- [Value Semantics](02-value-semantics.md) - Immutability model
- [Primitive Types](../03-type-system/01-primitive-types.md) - str type details
- [Compound Types](../03-type-system/02-compound-types.md) - List type details
