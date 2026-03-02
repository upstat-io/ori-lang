---
section: "05"
title: "Expressions (§14)"
status: not-started
goal: "Fill expression gaps: conversion table, composite literal typing, method values, type narrowing"
inspired_by:
  - "Go spec 'Expressions' — operands, composite literals, conversions, constant expressions"
  - "Go spec 'Conversions' — complete table of legal conversions"
depends_on: ["02", "03"]
sections:
  - id: "05.1"
    title: "Conversion Rules (as / as? / Into)"
    status: not-started
  - id: "05.2"
    title: "Composite Literal Typing"
    status: not-started
  - id: "05.3"
    title: "Method Values and References"
    status: not-started
  - id: "05.4"
    title: "String Interpolation Semantics"
    status: not-started
  - id: "05.5"
    title: "Type Narrowing"
    status: not-started
  - id: "05.6"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Expressions (§14)

**Status:** Not Started
**Goal:** §14 is already 1115 lines and covers most expression forms well. The gaps are targeted: no conversion table, no composite literal typing rules, no method value semantics, and no type narrowing formalism. Target ~1500 lines.

**Context:** Go's expression chapter has a dedicated "Conversions" subsection listing every legal conversion between types. Ori's conversions are scattered: `as` in §14.1.5, `Into` in §9.14, numeric behavior in §14.3.2. A reader searching "can I convert str to int?" has to check three places. Similarly, composite literal typing (how does `[1, 2, 3]` get typed as `[int]`?) is implicit knowledge, not specified.

**Reference implementations:**
- **Go** `ref/spec#Conversions`: Complete table of type-to-type conversions
- **Go** `ref/spec#Composite_literals`: Typing rules for struct, array, slice, map literals
- **TypeScript** `spec#Type_Narrowing`: Type narrowing in control flow

---

## 05.1 Conversion Rules (as / as? / Into)

**File:** `docs/ori_lang/v2026/spec/14-expressions.md` — expand §14.1.5

Currently shows examples but no exhaustive table. Need a complete conversion matrix.

- [ ] Add complete `as` (infallible) conversion table:

  | Source | Target | Behavior |
  |--------|--------|----------|
  | `int` | `float` | Exact (within i64 range) |
  | `int` | `byte` | Panic if out of 0..255 range |
  | `byte` | `int` | Zero-extend |
  | `char` | `int` | Unicode codepoint value |
  | `int` | `char` | Panic if not valid Unicode scalar |
  | `char` | `str` | Single-character string |
  | `byte` | `char` | As ASCII (panic if > 127?) |

  - Verify each conversion against compiler behavior
  - Identify all legal `as` conversions — list is exhaustive (anything not listed is compile error)

- [ ] Add complete `as?` (fallible) conversion table:

  | Source | Target | Returns |
  |--------|--------|---------|
  | `str` | `int` | `Some(n)` if parseable, `None` otherwise |
  | `str` | `float` | `Some(f)` if parseable, `None` otherwise |
  | `str` | `bool` | `Some(true/false)` for "true"/"false", `None` otherwise |

  - Verify against compiler
  - List is exhaustive

- [ ] Clarify relationship between `as`/`as?` and `As`/`TryAs` traits:
  - Built-in conversions use compiler intrinsics
  - User types implement `As<T>` / `TryAs<T>` traits
  - State: can user types implement `As<int>`? Does it override built-in?

- [ ] Cross-reference to §9.14 (Into trait) for semantic conversions vs representation conversions

---

## 05.2 Composite Literal Typing

**File:** `docs/ori_lang/v2026/spec/14-expressions.md`

How do literals get their types? This is implicit knowledge that should be explicit.

- [ ] List literal typing:
  - `[]` — requires type context: `let x: [int] = [];` or type error
  - `[1, 2, 3]` — type is `[T]` where `T` is the unified type of all elements
  - `[1, "hello"]` — compile error: cannot unify `int` and `str`
  - `[1, 2.0]` — compile error: no implicit numeric conversion
  - `[[1, 2], [3, 4]]` — type is `[[int]]`

- [ ] Map literal typing:
  - `{}` — ambiguous: could be empty map or empty block. Decision needed / document resolution
    - Currently: `{ }` with space = empty map? Or always a block?
    - Check parser behavior
  - `{"key": 1}` — type is `{str: int}`
  - `{key: value}` — struct or map? Document disambiguation rule
  - Map keys shall implement `Eq + Hashable`

- [ ] Struct literal typing:
  - `TypeName { field: value }` — type is `TypeName`
  - All fields shall be provided (unless spread `...` provides rest)
  - Field type shall match or be assignable to declared field type
  - Unknown field name is compile-time error

- [ ] Tuple literal typing:
  - `(a, b)` — type is `(A, B)` where `A` = type of `a`, `B` = type of `b`
  - `(a,)` — single-element tuple or trailing comma? Decision needed
  - `()` — type is `void`

---

## 05.3 Method Values and References

**File:** `docs/ori_lang/v2026/spec/14-expressions.md`

Can you get a reference to a method? Go distinguishes method expressions (`T.Method`) and method values (`x.Method`).

- [ ] Document whether method references are first-class:
  ```ori
  let f = list.contains;  // Is this legal?
  let g = Printable.to_str;  // Is this legal?
  ```
  - Check compiler behavior
  - If supported: document the resulting function type
  - If not supported: explicitly state "methods cannot be referenced without calling"

- [ ] Document associated function references:
  ```ori
  let f = Point.origin;  // Reference to associated function
  f()  // Legal?
  ```

---

## 05.4 String Interpolation Semantics

**File:** `docs/ori_lang/v2026/spec/14-expressions.md` or §7.7.4

String interpolation semantics are in §7 (lexical level) but the evaluation rules should also be in §14.

- [ ] State: template string `` `...{expr}...` `` is syntactic sugar for string concatenation
- [ ] Desugaring: `` `Hello, {name}!` `` → `"Hello, " + name.to_str() + "!"`
- [ ] With format spec: `` `{n:.2f}` `` → `n.format(spec: FormatSpec { ... })`
- [ ] Evaluation order: left-to-right through template segments
- [ ] Nested templates: `` `outer {`inner {x}`}` `` — is nesting allowed?
  - Check compiler behavior

---

## 05.5 Type Narrowing

**File:** `docs/ori_lang/v2026/spec/14-expressions.md`

After a type check or pattern match, is the type refined?

- [ ] Document whether Ori has type narrowing:
  ```ori
  let x: Option<int> = ...;
  if is_some(x) then
      // Is x narrowed to Some<int> here? Or still Option<int>?
  ```
  - Check compiler behavior (type checker)
  - If narrowing exists: specify which constructs trigger it
  - If no narrowing: explicitly state it and recommend `match` instead

- [ ] Document match arm type narrowing:
  ```ori
  match opt {
      Some(x) -> x + 1,    // x: int (always narrowed in match)
      None -> 0,
  }
  ```

---

## 05.6 Completion Checklist

- [ ] Conversion table for `as` is exhaustive — every legal conversion listed
- [ ] Conversion table for `as?` is exhaustive
- [ ] `as`/`as?` tables verified against compiler behavior
- [ ] Composite literal typing rules explicit for list, map, struct, tuple
- [ ] Empty literal disambiguation documented (`{}`, `[]`)
- [ ] Method values/references: supported or not, explicitly stated
- [ ] String interpolation desugaring formally defined
- [ ] Type narrowing: present or absent, explicitly stated
- [ ] All additions use ISO normative style

**Exit Criteria:** A reader can determine from §14 alone: what type any expression has, whether a conversion is legal, and how composite literals are typed.
