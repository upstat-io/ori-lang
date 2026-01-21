# Notation

This section defines the notation used throughout the specification.

## Grammar Notation

The grammar is expressed in Extended Backus-Naur Form (EBNF).

### Productions

A production has the form:

```
production_name = expression .
```

The period (`.`) terminates each production.

### Expressions

| Notation | Meaning |
|----------|---------|
| `a b` | Sequence: `a` followed by `b` |
| `a \| b` | Alternation: `a` or `b` |
| `[ a ]` | Option: zero or one `a` |
| `{ a }` | Repetition: zero or more `a` |
| `( a )` | Grouping |
| `"keyword"` | Terminal: literal keyword |
| `'c'` | Terminal: literal character |
| `TERMINAL` | Terminal: named terminal symbol |

### Character Ranges

Character ranges are expressed using the notation:

```
'a' ... 'z'
```

This denotes all characters from `'a'` to `'z'` inclusive in Unicode code point order.

### Exclusion

The notation `a - b` denotes the set of elements in `a` that are not in `b`.

## Conventions

### Case Sensitivity

All terminal symbols are case-sensitive. The keyword `if` is distinct from `If` or `IF`.

### Whitespace

Unless explicitly noted, whitespace between tokens is insignificant. The grammar does not specify whitespace handling; see [Lexical Elements](03-lexical-elements.md) for details.

### Naming Conventions

In this specification:

| Name Style | Usage |
|------------|-------|
| `lower_case` | Grammar production names |
| `UPPER_CASE` | Terminal symbol names |
| `PascalCase` | Type names in examples |
| `snake_case` | Function and variable names in examples |

## Terminology

The following terms have specific meanings in this specification:

| Term | Definition |
|------|------------|
| **must** | Absolute requirement. Violation is an error. |
| **must not** | Absolute prohibition. Violation is an error. |
| **shall** | Equivalent to must. |
| **shall not** | Equivalent to must not. |
| **should** | Recommendation. Implementations are encouraged but not required. |
| **should not** | Discouraged. Implementations are advised against. |
| **may** | Optional behavior. |
| **error** | A compile-time diagnostic that prevents successful compilation. |
| **panic** | A run-time failure that halts execution. |
| **undefined** | Behavior not specified. Implementations may vary. |
| **implementation-defined** | Behavior that must be documented by implementations. |

## Normative vs Informative

Sections and text marked as **informative** provide context, rationale, or examples but do not define requirements. All other text is **normative**.

Examples are informative unless explicitly stated otherwise.

> **Note:** Text in this format is informative.

## Example Notation

Examples appear in code blocks with the language marker `sigil`:

```sigil
@add (a: int, b: int) -> int = a + b
```

Invalid examples are marked with comments:

```sigil
// ERROR: missing return type
@add (a: int, b: int) = a + b
```

## Cross-References

References to other sections use the section name in brackets:

- See [Lexical Elements](03-lexical-elements.md)
- See [Types § Primitive Types](06-types.md#primitive-types)
