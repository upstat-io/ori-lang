# Proposal: Computed Map Keys

**Status:** Draft
**Created:** 2026-01-30
**Affects:** Syntax, grammar
**Depends on:** Spread operator proposal

---

## Summary

Formalize map literal key semantics: bare identifiers are literal string keys (like TypeScript/JSON), and `[expression]` syntax enables computed keys.

---

## Motivation

The current grammar says `map_entry = expression ":" expression`, implying keys are evaluated as expressions. But this conflicts with developer expectations from TypeScript/JavaScript where:

```typescript
const key = "foo";
{key: 1}    // → {"key": 1}  (literal)
{[key]: 1}  // → {"foo": 1}  (computed)
```

Given the prevalence of TypeScript developers, Ori should match this convention to avoid constant mistakes.

---

## Design

### Map Key Semantics

| Syntax | Meaning | Example |
|--------|---------|---------|
| `{foo: v}` | Literal string key `"foo"` | `{timeout: 30}` → `{"timeout": 30}` |
| `{"foo": v}` | Literal string key `"foo"` | `{"timeout": 30}` → `{"timeout": 30}` |
| `{[expr]: v}` | Computed key, evaluates `expr` | `{[key]: 30}` → `{"timeout": 30}` if `key = "timeout"` |

### Examples

```ori
// Literal keys (equivalent)
let m1 = {timeout: 30, retries: 3}
let m2 = {"timeout": 30, "retries": 3}

// Computed keys
let field = "timeout"
let m3 = {[field]: 30}  // {"timeout": 30}

// Expression as key
let prefix = "user_"
let m4 = {[prefix + "name"]: "Alice"}  // {"user_name": "Alice"}

// Mixed
let key = "dynamic"
let m5 = {static: 1, [key]: 2}  // {"static": 1, "dynamic": 2}
```

### With Spread

```ori
let defaults = {timeout: 30}
let key = "retries"

// Literal key after spread
{...defaults, verbose: true}  // {"timeout": 30, "verbose": true}

// Computed key after spread
{...defaults, [key]: 5}  // {"timeout": 30, "retries": 5}
```

---

## Grammar Changes

Update `grammar.ebnf`:

```ebnf
// Map literals with spread support
map_literal    = "{" [ map_element { "," map_element } ] "}" .
map_element    = "..." expression | map_entry .
map_entry      = map_key ":" expression .
map_key        = "[" expression "]"    /* computed key */
               | identifier            /* literal string key */
               | string_literal .      /* literal string key */
```

---

## Type Checking

- Bare identifier keys: type is `str`
- String literal keys: type is `str`
- Computed keys `[expr]`: `expr` must be of type `K` where the map is `{K: V}`

For most maps (`{str: V}`), computed keys must evaluate to `str`.

---

## Comparison with Other Languages

| Language | `{key: v}` | Computed key |
|----------|------------|--------------|
| JavaScript | literal `"key"` | `{[key]: v}` |
| TypeScript | literal `"key"` | `{[key]: v}` |
| Python | N/A (uses `{key: v}` as computed) | `{key: v}` |
| **Ori** | literal `"key"` | `{[key]: v}` |

Ori follows JavaScript/TypeScript convention, not Python.

---

## Migration

This is a **clarification**, not a breaking change. The grammar previously said `expression : expression` but all examples used string literals. This proposal formalizes the intended behavior.

Any code relying on bare identifiers being evaluated would break, but no such code exists in practice since the behavior was never documented or demonstrated.

---

## Alternatives Considered

### A. Evaluate bare identifiers as expressions

```ori
{key: 30}  // evaluates key variable
```

**Rejected:** Too surprising for TypeScript developers. High bug potential.

### B. Require quotes for all map keys

```ori
{"key": 30}  // only valid syntax
{key: 30}   // error
```

**Rejected:** More verbose than necessary. Bare identifiers as literal strings is intuitive.

### C. Use parentheses for computed keys

```ori
{(key): 30}  // computed
```

**Rejected:** Less familiar than bracket syntax. Parens already overloaded.

---

## Summary

| Syntax | Key Type | Value |
|--------|----------|-------|
| `{foo: v}` | `"foo"` (literal) | `v` |
| `{"foo": v}` | `"foo"` (literal) | `v` |
| `{[expr]: v}` | result of `expr` | `v` |

This matches TypeScript/JavaScript conventions and prevents common mistakes from developers familiar with those languages.
