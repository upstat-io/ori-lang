# Documentation

This section covers Sigil's documentation comment syntax.

---

## Documents

| Document | Description |
|----------|-------------|
| [Doc Comments](01-doc-comments.md) | Markdown-inspired documentation syntax |

---

## Overview

Sigil uses Markdown-inspired doc comments:

```sigil
// #Authenticates user and returns session token
// @param username case-insensitive
// @param password minimum 8 characters
// !InvalidCredentials: wrong username or password
// !AccountLocked: too many failed attempts
// >auth("user", "pass") -> Ok(Session{...})
@authenticate (username: str, password: str) -> Result<Session, AuthError> = ...
```

### Doc Comment Syntax

| Syntax | Purpose | Example |
|--------|---------|---------|
| `// #` | Main description | `// #Fetches user data` |
| `// @param` | Parameter doc | `// @param id must be positive` |
| `// @field` | Struct field doc | `// @field email must be valid` |
| `// !` | Error condition | `// !NotFound: user doesn't exist` |
| `// >` | Example | `// >parse("5s") -> 5000` |

### Key Principle

**Only document what types can't express:**

```sigil
// Bad - repeats what's obvious from signature
// #Adds two integers
// @param a the first integer
// @param b the second integer
// @returns the sum
@add (a: int, b: int) -> int = a + b

// Good - no comment needed, signature is clear
@add (a: int, b: int) -> int = a + b

// Good - adds non-obvious info
// #Saturates at int max instead of overflowing
@add_saturating (a: int, b: int) -> int = ...
```

---

## See Also

- [Main Index](../00-index.md)
- [Basic Syntax](../02-syntax/01-basic-syntax.md)
