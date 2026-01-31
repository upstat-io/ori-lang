# Proposal: Labeled Loops

**Status:** Approved
**Approved:** 2026-01-31
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, control flow

---

## Summary

This proposal formalizes labeled loop semantics, including label syntax, break/continue targeting, scope rules, and interaction with loop values.

---

## Problem Statement

The spec documents labeled loop syntax but leaves unclear:

1. **Label scope**: How far does a label's visibility extend?
2. **Shadowing**: Can labels be shadowed?
3. **Break with value**: How does `break:label value` interact with loop types?
4. **Continue with value**: What does `continue:label value` mean?
5. **Nesting limits**: Are there restrictions on label nesting depth?

---

## Syntax

### Label Declaration

Labels attach to `loop` or `for` with a colon and identifier:

```ori
loop:name(...)
for:name x in items do ...
for:name x in items yield ...
```

No space around the colon. The label is part of the loop keyword.

### Label Reference

Reference labels with `break:name` or `continue:name`:

```ori
break:outer
break:outer value
continue:outer
continue:outer value
```

---

## Label Scope

### Visibility

A label is visible within the loop body it labels:

```ori
loop:outer(
    // :outer visible here
    for x in items do
        // :outer still visible
        if done then break:outer,
)
// :outer not visible here
```

### Nesting

Labels scope correctly through arbitrary nesting:

```ori
loop:a(
    loop:b(
        loop:c(
            break:a,  // OK: exits outermost
            break:b,  // OK: exits middle
            break:c,  // OK: exits innermost
        ),
    ),
)
```

There is no language-imposed limit on label nesting depth. Practical limits arise from stack constraints and code readability.

### No Shadowing

Labels cannot be shadowed within their scope:

```ori
loop:outer(
    loop:outer(  // ERROR: label 'outer' already in scope
        ...
    ),
)
```

This prevents confusion about which loop `break:outer` targets.

---

## Break Semantics

### Break Without Value

`break:name` exits the labeled loop with no value:

```ori
loop:search(
    for item in items do
        if found(item) then break:search,  // Exit outer loop
)
```

### Break With Value

`break:name value` exits the labeled loop and makes the loop evaluate to `value`:

```ori
let result = loop:outer(
    for x in items do
        if match(x) then break:outer x,  // Loop evaluates to x
    None,  // Default if no match
)
```

### Type Consistency

All `break` paths for a labeled loop must produce values of the same type:

```ori
let x: int = loop:outer(
    for item in items do
        if a(item) then break:outer 1,      // int
        if b(item) then break:outer "two",  // ERROR: type mismatch
    0,
)
```

---

## Continue Semantics

### Continue Without Value

`continue:name` skips to the next iteration of the labeled loop:

```ori
for:outer x in xs do
    for y in ys do
        if skip_row(x) then continue:outer,  // Skip to next x
        process(x, y),
```

### Continue With Value in For-Yield

In `for...yield` context, `continue:name value` contributes `value` to the outer loop's collection:

```ori
let results = for:outer x in xs yield
    for:inner y in ys yield
        if special(x, y) then continue:outer x * y,  // Contribute to outer
        transform(x, y),
```

The value in `continue:label value` must have the same type as the target loop's yield element type. This is verified at compile time.

When `continue:label value` exits an inner `for...yield` to contribute to an outer `for...yield`, the inner loop's partially-built collection is discarded. Only `value` is contributed to the outer loop for this iteration.

### Continue With Value in For-Do

In `for...do` context, `continue:name value` is an error — there's no collection to contribute to:

```ori
for:outer x in xs do
    for y in ys do
        if skip(x, y) then continue:outer 42,  // ERROR: for-do doesn't collect
        process(x, y),
```

---

## Unlabeled Defaults

Unlabeled `break` and `continue` target the innermost loop:

```ori
loop:outer(
    for x in items do
        if a then break,        // Exits for loop
        if b then break:outer,  // Exits loop:outer
)
```

This is consistent with most languages.

---

## Interaction with Patterns

### In Run Pattern

Labels work inside `run`:

```ori
let result = run(
    let data = prepare(),
    loop:process(
        let batch = next_batch(data),
        if is_empty(batch) then break:process result,
        process_batch(batch),
    ),
)
```

### In Match Arms

Labels can be referenced from match arms:

```ori
loop:outer(
    let item = get_next(),
    match(item,
        Done -> break:outer,
        Value(v) -> process(v),
    ),
)
```

---

## Valid Label Names

Labels follow identifier rules:
- Start with letter or underscore
- Contain letters, digits, underscores
- Cannot be keywords

```ori
loop:search(...)     // OK
loop:_private(...)   // OK
loop:loop123(...)    // OK
loop:for(...)        // ERROR: 'for' is a keyword
loop:123start(...)   // ERROR: cannot start with digit
```

---

## Error Messages

### Undefined Label

```
error[E0870]: undefined loop label `outer`
  --> src/main.ori:5:20
   |
 5 |         if done then break:outer,
   |                      ^^^^^^^^^^^ no loop labeled 'outer' in scope
   |
   = help: did you mean `inner`?
```

### Shadowed Label

```
error[E0871]: label `search` already in scope
  --> src/main.ori:3:1
   |
 2 | loop:search(
   |      ------ first declaration
 3 |     loop:search(
   |          ^^^^^^ duplicate label
   |
   = help: use a different label name
```

### Type Mismatch

```
error[E0872]: mismatched types in labeled break
  --> src/main.ori:6:30
   |
 4 |         if a then break:outer 1,
   |                               - expected `int` due to this
 5 |         if b then break:outer "two",
   |                               ^^^^^ expected `int`, found `str`
```

### Continue Value in For-Do

```
error[E0873]: `continue` with value in `for...do`
  --> src/main.ori:4:20
   |
 4 |         continue:outer 42,
   |                        ^^ `for...do` does not collect values
   |
   = help: use `for...yield` to collect values
   = help: or remove the value: `continue:outer`
```

---

## Examples

### Nested Search

```ori
@find_pair (matrix: [[int]], target: int) -> Option<(int, int)> =
    loop:search(
        for:row (i, row) in matrix.iter().enumerate() do
            for (j, val) in row.iter().enumerate() do
                if val == target then break:search Some((i, j)),
        None,
    )
```

### Early Exit from Nested Processing

```ori
@process_until_error (batches: [[Task]]) -> Result<void, Error> =
    loop:main(
        for batch in batches do
            for task in batch do
                match(task.execute(),
                    Err(e) -> break:main Err(e),
                    Ok(_) -> (),
                ),
        Ok(()),
    )
```

### Continue Outer Loop

```ori
@process_valid_rows (data: [[Option<int>]]) -> [[int]] =
    for:outer row in data yield
        for cell in row yield
            match(cell,
                None -> continue:outer,  // Skip entire row
                Some(v) -> v,
            ),
```

---

## Spec Changes Required

### Update `19-control-flow.md`

Expand Labeled Loops section with:
1. Complete scope rules
2. No-shadowing rule
3. Type consistency requirements
4. Continue with value semantics
5. Interaction with for-do vs for-yield

---

## Summary

| Aspect | Behavior |
|--------|----------|
| Syntax | `loop:name`, `for:name`, `break:name`, `continue:name` |
| Scope | Within labeled loop body |
| Shadowing | Not allowed |
| Default target | Innermost loop |
| Break with value | Loop evaluates to value |
| Continue with value | Contributes to for-yield collection |
| Continue value in for-do | Error |
| Valid names | Identifiers, not keywords |
