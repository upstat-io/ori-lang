---
section: "04"
title: "Blocks and Scope (§11)"
status: not-started
goal: "Expand §11 with universe scope, module scope, label scope, and Self/self scope rules"
inspired_by:
  - "Go spec 'Blocks' — defines universe/package/file/function/block levels"
depends_on: ["03"]
sections:
  - id: "04.1"
    title: "Scope Hierarchy"
    status: not-started
  - id: "04.2"
    title: "Label Scopes"
    status: not-started
  - id: "04.3"
    title: "Type Parameter and Self Scope"
    status: not-started
  - id: "04.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Blocks and Scope (§11)

**Status:** Not Started
**Goal:** §11 covers lexical scoping, shadowing, and lambda capture well, but doesn't define the scope hierarchy (universe, module, function, block) or formalize label scopes and type parameter scopes. Target ~440 lines (from 240).

**Context:** Go defines 5 scope levels (universe, package, file, function, block) and specifies exactly which declarations are visible at each level. Ori's §11 explains scoping by example but doesn't formalize the hierarchy. This matters for: predeclared identifier resolution, module-level vs function-level rules, label scope, and `Self` in impl blocks.

**Reference implementations:**
- **Go** `ref/spec#Blocks`: Universe block, package block, file block, function body, nested blocks

---

## 04.1 Scope Hierarchy

**File:** `docs/ori_lang/v2026/spec/11-blocks-and-scope.md`

- [ ] Define the scope hierarchy:

  1. **Universe scope** — predeclared identifiers (types, traits, built-in functions, constructors). Encloses all modules.
  2. **Module scope** — top-level declarations within a module: functions, types, traits, impls, constants. Visible throughout the module (order-independent).
  3. **Function scope** — parameters and body. Parameters visible in body.
  4. **Block scope** — bindings within `{ ... }`. Visible from declaration to end of block.
  5. **Pattern scope** — bindings introduced by pattern matching (match arms, for loops, destructuring).

- [ ] Name resolution order: search innermost scope outward
  - Block → function → module → imports → universe
- [ ] State: imports are logically at module scope level
- [ ] State: `with...in` introduces capability bindings at block scope

---

## 04.2 Label Scopes

**File:** `docs/ori_lang/v2026/spec/11-blocks-and-scope.md`

Currently labels are only described in §16 (control flow). Scope rules belong in §11.

- [ ] Define label scope:
  - Labels (`loop:name`, `for:name`) are scoped to the enclosing function body
  - A label is visible within its labeled statement's body
  - Label names shall be unique within the enclosing function (no shadowing)
  - Labels do not conflict with variable names (separate namespace)
- [ ] Cross-reference to §16.3 for label usage rules

---

## 04.3 Type Parameter and Self Scope

**File:** `docs/ori_lang/v2026/spec/11-blocks-and-scope.md`

- [ ] Define type parameter scope:
  - Type parameters declared in `@f<T>` are visible in: parameter types, return type, where clause, and body
  - Type parameters declared in `type T<A>` are visible in: field types and where clause
  - Type parameters declared in `trait T<A>` are visible in: method signatures, associated types, where clause, and all impls of the trait
  - Type parameters declared in `impl<T>` are visible in: target type, trait name, method bodies

- [ ] Define `Self` scope:
  - `Self` is visible within `impl` blocks and `trait` definitions
  - In `impl Type { }`: `Self` = `Type`
  - In `trait Foo { }`: `Self` = the implementing type (abstract)
  - `Self` is NOT visible in standalone functions, module scope, or `extend` blocks

- [ ] Define `self` scope:
  - `self` is a parameter name bound in methods (functions with `self` parameter)
  - Immutable (like all parameters)
  - Type is `Self`

---

## 04.4 Completion Checklist

- [ ] 5-level scope hierarchy formalized (universe, module, function, block, pattern)
- [ ] Name resolution order explicitly stated
- [ ] Label scoping rules defined
- [ ] Type parameter scope rules cover all contexts (function, type, trait, impl)
- [ ] `Self` and `self` scope precisely defined
- [ ] All additions use ISO normative style

**Exit Criteria:** A reader can determine the scope of any identifier in any position from §11 alone, including edge cases like labels, type parameters, and `Self`.
