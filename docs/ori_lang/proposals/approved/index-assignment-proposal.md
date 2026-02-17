# Proposal: Index and Field Assignment via Copy-on-Write Desugaring

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-02-17
**Approved:** 2026-02-17
**Affects:** Grammar, type system, type checker, evaluator, standard library

---

## Executive Summary

This proposal introduces index and field assignment syntax as syntactic sugar for copy-on-write reassignment. The expression `list[i] = x` desugars to `list = list.updated(key: i, value: x)`, where `updated` returns a new collection with the specified element replaced. Field assignment (`state.name = x`) desugars to struct spread reconstruction (`state = { ...state, name: x }`). Under ARC with refcount == 1 (the common case for local mutable bindings), the "copy" is optimized to in-place mutation automatically.

**Key features:**
1. **`IndexSet` trait** with `@updated (self, key: Key, value: Value) -> Self` method
2. **Grammar extension** to allow index and field expressions on the left side of assignment
3. **Nested index assignment** (`list[i][j] = x`) via recursive desugaring
4. **Field assignment** (`state.name = x`) via struct spread reconstruction
5. **Mixed chains** (`state.items[i] = x`, `list[i].name = x`) combining both forms
6. **Compound assignment** (`list[i] += 1`, `state.count += 1`) via two-step desugaring
7. **Built-in implementations** for `[T]` and `{K: V}`

---

## Problem Statement

### No Way to Update a Collection Element

There is currently **no way** to replace an element in a list at a given index. The language provides:

- **Read access**: `list[i]` desugars to `list.index(key: i)` via the `Index` trait
- **No write access**: No `set()`, `updated()`, or `replace()` method exists on any built-in type
- **No index assignment**: The grammar restricts assignment to bare identifiers: `assignment = identifier "=" expression`

The only workaround is manual slice-and-concatenate:

```ori
// To set list[2] = 99 in a list of length 5:
let list = list.take(count: 2) + [99] + list.skip(count: 3)
```

This is O(n), error-prone (off-by-one on indices), and unacceptable ergonomically.

### Maps Are Equally Impaired

There is no way to insert or update a key-value pair in a map. The only option is spread syntax to build a new map from scratch:

```ori
let map = {..."old_map", "new_key": value}
```

This works for insertion but cannot express "replace the value at an existing key" without knowing all keys.

### No Way to Update a Struct Field

Updating a single field of a struct requires spread syntax with the full type name:

```ori
let state = GameState { ...state, score: state.score + 1 }
```

This is verbose, requires knowing the type name, and scales poorly with nesting. The natural syntax `state.score = state.score + 1` is not supported.

### Previous Rejection Was Based on Stale Reasoning

Both the [Index Trait Proposal](../approved/index-trait-proposal.md) and the [Custom Subscripting Proposal](../approved/custom-subscripting-proposal.md) rejected index assignment. Their reasoning:

> *"Ori's memory model prefers immutable updates. Index assignment would require mutable references, which Ori avoids."*

This conflates **in-place mutation** (modifying memory through a reference) with **reassignment** (rebinding an identifier to a new value). Per [spec/05-variables.md](../../0.1-alpha/spec/05-variables.md), Ori supports mutable bindings:

```ori
let x = 0
x = x + 1       // Valid: reassignment, not mutation
```

Index and field assignment can be expressed as reassignment without mutable references:

```ori
list[i] = x
// Desugars to:
list = list.updated(key: i, value: x)

state.name = x
// Desugars to:
state = { ...state, name: x }
```

Errata have been appended to both prior proposals documenting this correction.

---

## Proposed Design

### The `IndexSet` Trait

```ori
/// Trait for types that support producing a copy with an element replaced.
///
/// `updated` returns a new value identical to `self` except at the given key.
/// Combined with ARC copy-on-write, this enables efficient index assignment.
trait IndexSet<Key, Value> {
    @updated (self, key: Key, value: Value) -> Self
}
```

The trait is added to the prelude alongside `Index`. The `updated` method is publicly callable — users who prefer functional style can call `list.updated(key: 0, value: x)` directly without using the assignment sugar.

**Design rationale**: The method is named `updated` (not `set` or `replace`) because it returns a new value rather than modifying in place. This matches Ori's value-semantics vocabulary and avoids implying mutation. The name follows Swift's convention where `update` is the mutating form and `updated` is the non-mutating form.

### Desugaring: Index Assignment (Simple Case)

```ori
list[i] = x
```

Desugars to:

```ori
list = list.updated(key: i, value: x)
```

This is pure syntactic sugar. The compiler transforms the assignment target from an index expression to a call to `updated`, then reassigns the binding. The binding must be mutable (non-`$`).

### Desugaring: Field Assignment

```ori
state.name = x
```

Desugars to struct spread reconstruction:

```ori
state = { ...state, name: x }
```

The compiler identifies the root binding (`state`), determines its struct type via type inference, and generates a spread expression that copies all fields except the one being assigned. The binding must be mutable (non-`$`).

Field assignment does not require the `IndexSet` trait — it uses the existing struct spread mechanism. No new trait or method is needed for pure field updates.

### Desugaring: Nested Index Case

```ori
list[i][j] = x
```

Desugars inside-out. The innermost index assignment becomes an `updated` call, and each outer level wraps it:

```ori
list = list.updated(key: i, value: list[i].updated(key: j, value: x))
```

**Three levels deep:**

```ori
grid[x][y][z] = val
// Desugars to:
grid = grid.updated(
    key: x,
    value: grid[x].updated(
        key: y,
        value: grid[x][y].updated(key: z, value: val),
    ),
)
```

The pattern generalizes: for `target[k1][k2]...[kN] = val`, the compiler builds N nested `updated` calls, reading intermediate values via `index` for each level.

### Desugaring: Mixed Chains (Field + Index, Index + Field)

Assignment targets can freely mix field access and indexing in any order. The compiler processes the chain from the assignment point back to the root binding, wrapping each step appropriately:

- **Field step** → struct spread reconstruction
- **Index step** → `updated()` call

**Field then index** (`state.items[i] = x`):

```ori
state.items[i] = x
// Desugars to:
state = { ...state, items: state.items.updated(key: i, value: x) }
```

**Index then field** (`list[i].name = x`):

```ori
list[i].name = x
// Desugars to:
list = list.updated(key: i, value: { ...list[i], name: x })
```

**Deep mixed chain** (`game.levels[i].enemies[j].hp = 0`):

```ori
game.levels[i].enemies[j].hp = 0
// Desugars to:
game = { ...game,
    levels: game.levels.updated(key: i, value: { ...game.levels[i],
        enemies: game.levels[i].enemies.updated(key: j, value: { ...game.levels[i].enemies[j],
            hp: 0,
        }),
    }),
}
```

The algorithm is mechanical: walk the chain from right (assignment point) to left (root binding), wrapping each field step in `{ ...receiver, field: inner }` and each index step in `receiver.updated(key: k, value: inner)`.

### Desugaring: Compound Assignment

Compound assignment operators (`+=`, `-=`, `*=`, etc.) compose with index and field assignment via two-step desugaring:

```ori
list[i] += 1
// Step 1: desugar compound assignment
list[i] = list[i] + 1
// Step 2: desugar index assignment
list = list.updated(key: i, value: list[i] + 1)
```

This works for all compound operators and all assignment target forms:

```ori
state.count += 1
// Step 1: state.count = state.count + 1
// Step 2: state = { ...state, count: state.count + 1 }

matrix[i][j] *= 2.0
// Step 1: matrix[i][j] = matrix[i][j] * 2.0
// Step 2: matrix = matrix.updated(key: i, value: matrix[i].updated(key: j, value: matrix[i][j] * 2.0))
```

No new traits or mechanisms are needed — compound assignment is purely a pre-existing desugaring that now applies to the expanded set of assignment targets.

### Grammar Changes

The current grammar production for assignment is:

```ebnf
assignment = identifier "=" expression .
```

This proposal extends it to:

```ebnf
assignment = assignment_target "=" expression .
assignment_target = identifier { index_suffix | field_suffix } .
index_suffix = "[" expression "]" .
field_suffix = "." identifier .
```

The left-hand side of an assignment (including compound assignment) can now be an identifier followed by any number of index and field access operations. The root must still be a bare identifier (a mutable binding in scope).

**What is NOT allowed:** Arbitrary expressions on the left side. Only identifier-rooted chains of field access and indexing are valid. Function calls, method calls, and other expressions cannot appear as assignment targets.

### Type Checking Rules

1. **Root binding must be mutable**: The identifier at the root of the assignment target must refer to a mutable binding (not `$`-prefixed, not a parameter, not a loop variable).

2. **Field access validation**: Each `.field` in the chain must be a valid field of the receiver's struct type. The receiver type must be a struct (not an enum, trait object, or primitive).

3. **Index trait required for reads**: Each `[key]` in the chain (except the last) must resolve via the `Index<Key, Value>` trait on the receiver type.

4. **IndexSet trait required for index writes**: The final `[key]` in the chain (if present) must resolve via `IndexSet<Key, Value>` on the receiver type. The value type of the `Index` impl and the value type of the `IndexSet` impl must agree.

5. **Type of assigned value**: The right-hand side expression must be assignable to the type at the assignment point — the `Value` type parameter of the `IndexSet` impl for index targets, or the field type for field targets.

6. **Self type returned**: `IndexSet.updated` returns `Self`, so the reassignment is always type-correct. Struct spread expressions preserve the struct type.

### Implementation Note: Type-Directed Desugaring

The desugaring described above is **not** a parser-level transformation. It is **type-directed** and must occur during or after type inference:

- **Field assignment** requires knowing the struct type to generate correct spread syntax.
- **Index assignment** requires resolving `IndexSet` trait implementations.
- **Mixed chains** require type information at every step to determine whether to use spread or `updated`.

The parser's role is limited to accepting the extended `assignment_target` grammar and emitting an AST node that captures the chain of field/index accesses. The actual desugaring into `updated` calls and spread expressions is performed by the type checker or a type-directed lowering pass.

### Constraint: Index and IndexSet Consistency

For a type `T` implementing both `Index<K, V>` and `IndexSet<K, V>`, the following must hold:

```
For all t: T, k: K, v: V:
    t.updated(key: k, value: v).index(key: k) == v
```

This is a semantic contract (not enforced by the compiler) stating that reading back a just-written key returns the written value.

---

## ARC Optimization

Ori uses ARC (Automatic Reference Counting) with copy-on-write semantics, as described in [spec/15-memory-model.md](../../0.1-alpha/spec/15-memory-model.md).

When `list.updated(key: i, value: x)` is called and `list` has a refcount of 1 (i.e., no other binding shares the same backing storage), the runtime can perform the update **in place** rather than copying. This is an existing ARC optimization, not something new introduced by this proposal.

The common case for local mutable bindings is refcount == 1:

```ori
let list = [1, 2, 3]    // refcount = 1
list[0] = 10            // desugars to list = list.updated(key: 0, value: 10)
                         // refcount was 1, so updated() mutates in-place
                         // result: list = [10, 2, 3], no allocation
```

When refcount > 1 (the value is shared), `updated` allocates a new collection with the modification applied, and the binding is rebound to the new value. The old value's refcount decreases by 1:

```ori
let list = [1, 2, 3]    // refcount = 1
let alias = list         // refcount = 2
list[0] = 10            // refcount > 1, so updated() copies
                         // list -> [10, 2, 3] (new allocation, refcount = 1)
                         // alias -> [1, 2, 3] (old allocation, refcount = 1)
```

This is identical to Swift's copy-on-write behavior for `Array`, `Dictionary`, and other value types under ARC.

---

## Standard Implementations

### List: `[T]`

```ori
impl<T> IndexSet<int, T> for [T] {
    @updated (self, key: int, value: T) -> [T] =
        if key < 0 || key >= self.len() then
            panic(msg: "index out of bounds: " + key.to_str())
        else
            // intrinsic: compiler-provided
            // When refcount == 1, modifies in place
}
```

**Behavior:** Returns a new list identical to `self` except at position `key`. Panics if `key` is out of bounds (negative or >= length). Matches the panic behavior of `Index<int, T>` for `[T]`.

### Map: `{K: V}`

```ori
impl<K: Eq + Hashable, V> IndexSet<K, V> for {K: V} {
    @updated (self, key: K, value: V) -> {K: V} =
        // intrinsic: compiler-provided
        // Inserts or replaces the entry for key
        // When refcount == 1, modifies in place
}
```

**Behavior:** Returns a new map identical to `self` except the entry for `key` is set to `value`. If `key` already exists, its value is replaced. If `key` does not exist, a new entry is inserted. This never panics.

Note: Map's `Index` returns `Option<V>`, but `IndexSet` takes a bare `V`. This asymmetry is intentional — reading may find nothing, but writing always provides a value.

### String: NOT Supported

```ori
// str does NOT implement IndexSet.
// Strings are immutable sequences of bytes/codepoints.
"hello"[0] = "H"  // ERROR: `str` does not implement `IndexSet<int, str>`
```

**Rationale:** String indexing by integer returns a single codepoint as `str`, but replacing a codepoint may change the byte length of the string, making index-based replacement semantically confusing and potentially O(n). String manipulation should use dedicated methods (`replace`, `splice`, etc.) that make the cost explicit.

### Fixed-Capacity List: `[T, max N]`

```ori
impl<T, $N: int> IndexSet<int, T> for [T, max N] {
    @updated (self, key: int, value: T) -> [T, max N] =
        if key < 0 || key >= self.len() then
            panic(msg: "index out of bounds: " + key.to_str())
        else
            // intrinsic: compiler-provided
}
```

**Behavior:** Same as `[T]` but preserves the fixed-capacity type.

---

## Error Cases

### Immutable Binding

```
error[E____]: cannot assign to immutable binding
  --> src/main.ori:3:5
   |
 2 | let $list = [1, 2, 3]
   |     ----- binding declared as immutable
 3 | $list[0] = 10
   | ^^^^^^^^^^^^^ cannot assign through immutable binding
   |
   = help: remove the `$` prefix to make the binding mutable
```

### Parameter Assignment

```
error[E____]: cannot assign to parameter
  --> src/main.ori:2:5
   |
 1 | @update (items: [int]) -> [int] = run(
   |          ----- parameters are always immutable
 2 |     items[0] = 99,
   |     ^^^^^^^^^^^^^^ cannot assign to parameter
   |
   = help: bind to a local variable first: `let items = items`
```

### Loop Variable Assignment

```
error[E____]: cannot assign to loop variable
  --> src/main.ori:2:9
   |
 1 | for item in items do
   |     ---- loop variable
 2 |     item[0] = 99
   |     ^^^^^^^^^^^^ cannot assign to loop variable
```

### Type Not Indexable for Write

```
error[E____]: type `str` does not support index assignment
  --> src/main.ori:3:5
   |
 3 | text[0] = "H"
   | ^^^^^^^^^^^^^^ `IndexSet<int, str>` is not implemented for `str`
   |
   = note: strings do not support element replacement
   = help: use string methods like `replace` instead
```

### Invalid Field for Assignment

```
error[E____]: no field `missing` on type `GameState`
  --> src/main.ori:3:5
   |
 3 | state.missing = 42
   |       ^^^^^^^ unknown field
   |
   = note: `GameState` has fields: score, level, lives
```

### Type Mismatch on Value

```
error[E____]: mismatched types in index assignment
  --> src/main.ori:3:16
   |
 3 | list[0] = "hello"
   |           ^^^^^^^ expected `int`, found `str`
   |
   = note: `[int]` implements `IndexSet<int, int>`
```

### Key Type Mismatch

```
error[E____]: mismatched types in index expression
  --> src/main.ori:3:10
   |
 3 | list["key"] = 10
   |      ^^^^^ expected `int`, found `str`
   |
   = note: `[int]` implements `IndexSet<int, int>`, not `IndexSet<str, _>`
```

---

## Comparison with Other Languages

### Swift (Closest Model)

Swift's approach is the closest prior art. Swift arrays and dictionaries are value types with copy-on-write under ARC:

```swift
var array = [1, 2, 3]
array[0] = 10  // In-place if uniquely referenced, copies otherwise
```

Swift uses a `subscript` declaration with `get` and `set` accessors. The `set` accessor receives `newValue` implicitly and mutates `self` (which is `inout` in a mutating context).

**Ori's difference:** Ori does not have `inout`, `mutating`, or mutable method receivers. Instead, `updated` returns a new value and the compiler generates a reassignment. The ARC optimization produces the same runtime behavior as Swift, but the semantic model is purely functional (no mutation vocabulary).

### Rust (`IndexMut` with `&mut`)

Rust uses `IndexMut` which requires a mutable borrow:

```rust
let mut v = vec![1, 2, 3];
v[0] = 10;  // Calls IndexMut::index_mut(&mut v, 0) = 10
```

This is true in-place mutation through a mutable reference. Rust's borrow checker ensures no aliasing.

**Ori's difference:** Ori has no mutable references or borrow checker. The copy-on-write approach achieves the same ergonomics without the conceptual overhead of borrowing. The tradeoff is that shared values incur a copy on write, whereas Rust would reject the program at compile time if aliasing is detected.

### Python (`__setitem__`)

Python uses the `__setitem__` dunder method:

```python
lst = [1, 2, 3]
lst[0] = 10  # Calls lst.__setitem__(0, 10)
```

This is true in-place mutation with no copy. Python objects are always heap-allocated and reference-counted, so there is no concept of value semantics or copy-on-write.

**Ori's difference:** Ori's desugaring to `updated` + reassignment preserves value semantics. Two bindings pointing to the same list remain independent after one is modified via index assignment.

### Kotlin (Operator Overloading)

Kotlin desugars index assignment similarly to this proposal:

```kotlin
list[i] = x
// Desugars to:
list.set(i, x)
```

However, Kotlin's `set` mutates in place (Kotlin collections are reference types).

**Ori's difference:** Ori's `updated` returns a new value; the reassignment is explicit in the desugared form.

| Language | Mechanism | Semantics | Copy-on-Write | Value Semantics |
|----------|-----------|-----------|---------------|-----------------|
| **Ori** | `IndexSet` trait + desugaring | Reassignment | Yes (ARC) | Yes |
| Swift | `subscript { set }` | In-place via `inout` | Yes (ARC) | Yes |
| Rust | `IndexMut` trait | `&mut` borrow | No (borrow checker) | No (references) |
| Python | `__setitem__` | In-place mutation | No | No |
| Kotlin | `operator set` | In-place mutation | No | No |

---

## Grammar Changes Required

### Current Grammar (grammar.ebnf)

```ebnf
assignment = identifier "=" expression .
binding  = let_expr | assignment .
```

### Proposed Grammar

```ebnf
assignment = assignment_target "=" expression
           | assignment_target compound_op expression .
assignment_target = identifier { "[" expression "]" | "." identifier } .
compound_op = "+=" | "-=" | "*=" | "/=" | "%=" .
binding  = let_expr | assignment .
```

The `assignment_target` production allows an identifier optionally followed by any combination of index access (`[expr]`) and field access (`.ident`). This is intentionally more restrictive than a general `postfix_expr` — only identifiers, indexing, and field access are valid. Method calls, function calls, `?`, `as`, and other postfix operations are not valid assignment targets.

Compound assignment operators apply to all assignment target forms uniformly.

---

## Spec Changes Required

### `grammar.ebnf`

Update the `assignment` and add the `assignment_target` production as shown above.

### `05-variables.md`

Add a section on extended assignment describing:

- Index assignment syntax and what it desugars to
- Field assignment syntax and what it desugars to
- Compound assignment with extended targets
- Requirement that the root binding be mutable
- Nested and mixed chain examples
- Note that desugaring is type-directed

### `09-expressions.md`

Update the Index Trait section to cross-reference `IndexSet` and index assignment. Add examples of index and field assignment in the expressions chapter.

### `07-properties-of-types.md`

Add `IndexSet` trait definition alongside the existing `Index` trait documentation. Document the semantic contract between `Index` and `IndexSet`.

### `15-memory-model.md`

Add a note about how ARC copy-on-write applies to index assignment, demonstrating that the common case (refcount == 1) results in in-place modification.

---

## Implementation Plan

### Phase 1: `IndexSet` Trait and `updated` Method

1. Define `IndexSet` trait in prelude
2. Register `updated` as a built-in method on `[T]`, `{K: V}`, and `[T, max N]` in the evaluator
3. Implement `updated` with ARC-aware copy-on-write in `ori_patterns`/`ori_eval`

### Phase 2: Parser Changes

1. Extend the parser to accept `assignment_target` (identifier + index/field chains) on the left side of `=` and compound assignment operators
2. Emit a new AST node (or annotated assignment node) that captures the chain of index/field accesses

### Phase 3: Type-Directed Desugaring

1. In the type checker or a type-directed lowering pass, desugar assignment targets:
   - `[key]` steps → `updated()` calls (requires `IndexSet` trait resolution)
   - `.field` steps → struct spread reconstruction (requires struct type information)
2. Handle nested cases, mixed field-index chains, and compound assignment
3. This phase cannot occur in the parser — it requires type information

### Phase 4: Type Checker Integration

1. Resolve `IndexSet` trait in the type checker
2. Validate mutability of root binding
3. Validate field names against struct types
4. Validate key and value types against `IndexSet` impl
5. Emit appropriate diagnostics for errors

### Phase 5: Tests

1. Spec tests in `tests/spec/` for all cases (simple index, nested index, field, mixed chains, maps, compound assignment, errors)
2. Rust unit tests for parser, desugaring, and type checker changes
3. Error case tests for immutable bindings, parameters, missing trait impls, type mismatches, invalid fields

---

## Alternatives Considered

### 1. Add a `set()` Method Instead

Add `list.set(index: 0, value: 42)` without any syntax support.

**Rejected.** This solves the capability gap but not the ergonomics gap. `list = list.set(index: i, value: x)` is verbose and the explicit reassignment is easy to forget (writing `list.set(...)` without rebinding silently discards the result). Index assignment syntax makes the intent clear and prevents this class of bug.

### 2. Add Both `set()` and Index Assignment

Provide `set()` as the method and `list[i] = x` as sugar for it.

**Considered.** This is viable but introduces two names for the same operation (`set` vs `updated`). If both are provided, `set` should be documented as the underlying method and `[i] = x` as the preferred syntax. This proposal uses `updated` as the single canonical method to avoid confusion.

### 3. Use `Index` Trait with a `set` Method

Extend the existing `Index` trait with an optional `set` method instead of creating a new `IndexSet` trait.

**Rejected.** Not all indexable types support write. Strings implement `Index` but should not implement write. Separating read and write into distinct traits follows the Interface Segregation Principle and matches Rust's `Index`/`IndexMut` separation.

### 4. Lenses / Optics

Use a lens-based system where index paths are first-class values that can be composed.

**Deferred.** Lenses are more powerful but significantly more complex to implement and explain. Index and field assignment covers the 95% use case. A lens system could be layered on top in a future proposal.

---

## Open Questions

1. **Slice assignment**: Should `list[0..3] = [a, b, c]` be supported? This is significantly more complex (different lengths, different types) and should be a separate proposal if desired.

2. **Evaluation order for nested index reads**: In `list = list.updated(key: i, value: list[i].updated(key: j, value: x))`, `list[i]` is read before the outer `updated` is called. If `i` or `j` involve side effects, the evaluation order matters. This proposal follows left-to-right evaluation, consistent with Ori's general evaluation order.

---

## Resolved Questions

1. **Should `IndexSet` be in the prelude?** **Yes.** `Index` is in the prelude and they are complementary. The `updated` method is publicly callable for users who prefer functional style.

2. **Should compound assignment operators (`list[i] += 1`) be supported?** **Yes.** Compound assignment composes naturally via two-step desugaring. No new traits needed.

---

## Compatibility

- **Fully backward compatible**: No existing valid program changes meaning
- **New syntax**: `target[key] = value` and `target.field = value` were previously parse errors; they now have defined semantics
- **New trait**: `IndexSet` is new; no existing code can conflict with it
- **New method**: `updated` is new on built-in types; no existing code calls it
- **Compound assignment**: Extended to work with new assignment targets; existing compound assignment on identifiers is unchanged

---

## References

- [Ori Index Trait Proposal](../approved/index-trait-proposal.md) — existing read-only `Index` trait
- [Ori Custom Subscripting Proposal](../approved/custom-subscripting-proposal.md) — original motivation for `Index`
- [Ori spec/05-variables.md](../../0.1-alpha/spec/05-variables.md) — mutability model
- [Ori spec/15-memory-model.md](../../0.1-alpha/spec/15-memory-model.md) — ARC and value semantics
- [Swift Copy-on-Write](https://developer.apple.com/documentation/swift/array) — closest prior art
- [Rust `IndexMut` trait](https://doc.rust-lang.org/std/ops/trait.IndexMut.html) — reference-based alternative
- [Kotlin Operator Overloading: Indexed Access](https://kotlinlang.org/docs/operator-overloading.html#indexed-access-operator) — desugaring-based approach

---

## Changelog

- 2026-02-17: Approved — expanded scope to include field assignment, mixed chains, and compound assignment; clarified type-directed desugaring; resolved open questions on prelude placement and compound operators
- 2026-02-17: Initial draft
