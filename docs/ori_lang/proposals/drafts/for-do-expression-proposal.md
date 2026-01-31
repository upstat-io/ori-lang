# Proposal: For-Do Expression

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-31
**Affects:** Compiler, expressions, iteration

---

## Summary

This proposal formalizes the `for...in...do` iteration expression syntax, including binding patterns, guard conditions, break/continue semantics, and interaction with labeled loops.

---

## Problem Statement

The spec documents `for...do` syntax but leaves unclear:

1. **Return type**: What is the type of a `for...do` expression?
2. **Break behavior**: Can `break` have a value in `for...do`?
3. **Continue behavior**: What happens with `continue` (with and without value)?
4. **Binding patterns**: What patterns are allowed in the binding position?
5. **Guard evaluation**: When is the `if` guard evaluated?
6. **Empty iteration**: What happens when the source is empty?

---

## Syntax

### Basic Form

```ori
for binding in iterable do body
```

### With Guard

```ori
for binding in iterable if condition do body
```

### With Binding Pattern

```ori
for (key, value) in map do body
for { x, y } in points do body
for [first, ..rest] in lists do body
```

### With Label

```ori
for:name binding in iterable do body
```

---

## Semantics

### Iteration

The `for...do` expression iterates over items from an `Iterable` source, executing the body for each item.

```ori
for x in [1, 2, 3] do print(msg: `{x}`)
// Prints: 1, 2, 3
```

### Return Type

`for...do` always returns `void`. The body expression's value is discarded:

```ori
let result = for x in items do x * 2
// result: void (not a collection)
```

To collect results, use `for...yield` instead.

### Source Requirements

The source must implement `Iterable`:

```ori
trait Iterable {
    type Item
    @iter (self) -> impl Iterator<Item = Self.Item>
}
```

Built-in iterables include:
- `[T]` (lists)
- `{K: V}` (maps, iterates key-value tuples)
- `Range<int>` (ranges)
- `str` (iterates codepoints as single-character strings)

### Desugaring

`for...do` desugars to iterator consumption:

```ori
// This:
for x in items do process(x)

// Desugars to:
items.iter().for_each(f: x -> process(x))
```

With guard:

```ori
// This:
for x in items if predicate(x) do process(x)

// Desugars to:
items.iter().filter(predicate: x -> predicate(x)).for_each(f: x -> process(x))
```

---

## Binding Patterns

### Simple Binding

```ori
for x in items do process(x)
```

The binding `x` is mutable within the body scope.

### Immutable Binding

```ori
for $x in items do process(x)
```

### Tuple Destructuring

```ori
for (key, value) in map do
    print(msg: `{key}: {value}`)
```

### Struct Destructuring

```ori
for { name, age } in users do
    print(msg: `{name} is {age}`)
```

### List Destructuring

```ori
for [first, second, ..rest] in nested do
    process(first, second, rest)
```

### With Rename

```ori
for { name: user_name, age: user_age } in users do
    print(msg: `{user_name}`)
```

### Nested Patterns

```ori
for (id, { name, email }) in user_map do
    send_email(id, name, email)
```

---

## Guard Condition

### Evaluation

The guard is evaluated for each item before the body:

```ori
for x in items if x > 0 do process(x)
// Only processes positive items
```

### Multiple Conditions

Use `&&` for multiple conditions:

```ori
for x in items if x > 0 && x < 100 do process(x)
```

### Binding in Guard

The binding is available in the guard:

```ori
for { name, active } in users if active do
    print(msg: name)
```

---

## Break and Continue

### Continue Without Value

`continue` skips to the next iteration:

```ori
for x in items do
    if skip(x) then continue,
    process(x),
```

### Continue With Value

`continue value` is an error in `for...do` context. There is no collection to contribute to:

```ori
for x in items do
    if special(x) then continue x * 2,  // ERROR E0873
    process(x),
```

Use `for...yield` to collect values.

### Break Without Value

`break` exits the loop immediately:

```ori
for x in items do
    if done(x) then break,
    process(x),
```

### Break With Value

`break value` in `for...do` is an error. The loop returns `void`:

```ori
for x in items do
    if found(x) then break x,  // ERROR: for-do returns void
    process(x),
```

To return a value from iteration, use `loop` with explicit break value.

---

## Labeled Loops

### Label Syntax

```ori
for:name binding in iterable do body
```

### Break to Label

```ori
for:outer x in xs do
    for y in ys do
        if done(x, y) then break:outer,
        process(x, y),
```

### Continue to Label

```ori
for:outer x in xs do
    for y in ys do
        if skip_row(x) then continue:outer,
        process(x, y),
```

See [Labeled Loops Proposal](../approved/labeled-loops-proposal.md) for complete label semantics.

---

## Empty Iteration

When the source is empty, the body is never executed:

```ori
for x in [] do panic(msg: "never")  // No panic; body never runs
```

The expression still returns `void`.

---

## Nested For Loops

Nested `for...do` loops execute the inner loop completely for each outer iteration:

```ori
for x in [1, 2] do
    for y in [3, 4] do
        print(msg: `{x},{y}`),
// Output: 1,3  1,4  2,3  2,4
```

Unlike `for...yield`, nested `for...do` does not flatten. Each is an independent loop.

---

## Interaction with Patterns

### In Run Pattern

```ori
run(
    let items = load_items(),
    for item in items do validate(item),
    save_all(items),
)
```

### With Error Propagation

```ori
@process_all (items: [Item]) -> Result<void, Error> = run(
    for item in items do
        validate(item)?,  // Propagates error, exits loop and function
    Ok(()),
)
```

### In Conditional

```ori
if should_process then
    for x in items do process(x)
```

---

## Mutation Within Loop

The binding is a copy of each element. Mutating it does not affect the source:

```ori
let items = [1, 2, 3]
for x in items do
    x = x * 2,  // Mutates local copy only
// items is still [1, 2, 3]
```

To modify the source, use indexed access or collect with `for...yield`.

---

## Error Messages

### Non-Iterable Source

```
error[E0880]: `for` requires `Iterable` source
  --> src/main.ori:5:10
   |
 5 | for x in 42 do process(x)
   |          ^^ `int` does not implement `Iterable`
   |
   = help: use a range: `0..42`
```

### Continue With Value in For-Do

```
error[E0873]: `continue` with value in `for...do`
  --> src/main.ori:5:20
   |
 5 |     if special(x) then continue 42,
   |                        ^^^^^^^^^^^ `for...do` does not collect values
   |
   = help: use `for...yield` to collect values
   = help: or remove the value: `continue`
```

### Break With Value in For-Do

```
error[E0881]: `break` with value in `for...do`
  --> src/main.ori:5:20
   |
 5 |     if found(x) then break x,
   |                      ^^^^^^^ `for...do` returns `void`
   |
   = help: use `loop` with break value for iteration with result
   = help: use `for...yield` with break for collection with early exit
```

### Non-Bool Guard

```
error[E0882]: guard condition must be `bool`
  --> src/main.ori:5:18
   |
 5 | for x in items if x do process(x)
   |                   ^ expected `bool`, found `int`
```

### Irrefutable Pattern Required

```
error[E0883]: refutable pattern in `for` binding
  --> src/main.ori:5:5
   |
 5 | for Some(x) in options do process(x)
   |     ^^^^^^^ pattern may not match all items
   |
   = help: use `for...yield` with guard: `for opt in options if is_some(opt) yield ...`
   = help: or filter first: `for x in options.filter_map(identity) do ...`
```

---

## Examples

### Basic Iteration

```ori
@print_all (items: [str]) -> void =
    for item in items do print(msg: item)
```

### With Guard

```ori
@print_positive (numbers: [int]) -> void =
    for n in numbers if n > 0 do print(msg: `{n}`)
```

### Destructuring

```ori
@print_entries (map: {str: int}) -> void =
    for (key, value) in map do
        print(msg: `{key} = {value}`)
```

### Nested Loops

```ori
@print_matrix (matrix: [[int]]) -> void =
    for row in matrix do
        for cell in row do
            print(msg: `{cell} `),
        print(msg: "\n"),
```

### Early Exit

```ori
@find_and_process (items: [Item]) -> void =
    for item in items do
        if is_target(item) then run(
            process(item),
            break,
        ),
```

### Labeled Continue

```ori
@skip_invalid_rows (data: [[Option<int>]]) -> void =
    for:row row in data do
        for cell in row do
            if is_none(cell) then continue:row,  // Skip entire row
            process(cell.unwrap()),
```

---

## Grammar

The grammar is defined in `grammar.ebnf` under the `for_expr` production:

```
for_expr = "for" [ label ] binding "in" expr [ "if" expr ] ( "do" | "yield" ) expr .
label = ":" identifier .
binding = identifier | pattern .
```

---

## Spec Changes Required

### Update `09-expressions.md`

Expand For Expression section with:

1. Binding pattern details
2. Guard evaluation semantics
3. Break/continue restrictions
4. Empty iteration behavior

### Update `19-control-flow.md`

Add section on for-do specific control flow behavior.

---

## Summary

| Aspect | Behavior |
|--------|----------|
| Syntax | `for binding in iterable [if guard] do body` |
| Return type | `void` |
| Source | Must implement `Iterable` |
| Guard | Evaluated per item, must be `bool` |
| Binding | Mutable copy of each item |
| Continue | Skips iteration; value is error |
| Break | Exits loop; value is error |
| Empty source | Body never executes, returns `void` |
| Labeled | `for:name` with `break:name`/`continue:name` |
| Desugars to | `.iter().for_each()` or `.iter().filter().for_each()` |
