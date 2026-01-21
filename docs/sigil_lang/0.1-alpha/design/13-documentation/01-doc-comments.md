# Documentation Comments

This document covers Sigil's documentation comment syntax.

---

## Design Principle

**Only document what types can't express.**

Types already communicate parameter types, return types, and error possibilities. Documentation should add information that types don't capture.

---

## Comment Types

### Regular Comments

```sigil
// This is a regular comment
// Not processed by documentation tools
```

### Doc Comments

Doc comments use special markers after `//`:

| Syntax | Purpose |
|--------|---------|
| `// #` | Main description |
| `// @param <name>` | Parameter documentation |
| `// @field <name>` | Struct field documentation |
| `// !` | Error condition |
| `// >` | Example |

---

## Main Description

Use `// #` for the primary description:

```sigil
// #Fetches user data from the database
@fetch_user (id: UserId) -> Result<User, Error> = ...
```

### Multi-line Descriptions

Consecutive `// #` lines merge:

```sigil
// #Fetches data with exponential backoff retry.
// #Retries up to 3 times on transient errors.
// #Returns cached data if available and fresh.
@fetch_data (url: str) -> Result<Data, Error> = ...
```

---

## Parameter Documentation

Use `// @param` only when the type doesn't fully explain the parameter:

```sigil
// #Authenticates user and returns session
// @param username case-insensitive
// @param password minimum 8 characters
@authenticate (username: str, password: str) -> Result<Session, AuthError> = ...
```

### When to Use

- Constraints not captured by types (min length, format requirements)
- Non-obvious behavior (case sensitivity, normalization)
- Units when not clear from context

### When NOT to Use

```sigil
// Bad: obvious from types
// @param a the first integer to add
// @param b the second integer to add
// @returns the sum of a and b
@add (a: int, b: int) -> int = a + b

// Good: no doc needed, signature is clear
@add (a: int, b: int) -> int = a + b
```

---

## Field Documentation

Use `// @field` for struct fields needing explanation:

```sigil
// #User account with authentication credentials
// @field email must be valid email format
// @field role determines access permissions
type User = {
    id: int,
    name: str,
    email: str,
    role: Role
}
```

---

## Error Documentation

Use `// !` to document error conditions:

```sigil
// #Authenticates user and returns session
// !InvalidCredentials: wrong username or password
// !AccountLocked: too many failed attempts
// !SessionExpired: previous session timed out
@authenticate (username: str, password: str) -> Result<Session, AuthError> = ...
```

### Format

```
// !ErrorName: description
```

The error name should match a variant in the error type.

---

## Examples

Use `// >` for examples:

```sigil
// #Parses a duration string into milliseconds
// >parse_duration("5s") -> 5000
// >parse_duration("100ms") -> 100
// >parse_duration("2m") -> 120000
@parse_duration (s: str) -> Result<int, ParseError> = ...
```

### Example Format

```
// >expression -> result
```

Examples should be valid Sigil expressions that could be run in a REPL.

---

## Config Documentation

```sigil
// #Maximum retry attempts for network requests
$max_retries = 3

// #Base URL for API endpoints
// #Must not include trailing slash
$api_base = "https://api.example.com"

// #Request timeout duration
$timeout = 30s
```

---

## Type Documentation

```sigil
// #Represents a point in 2D space
type Point = { x: int, y: int }

// #Authentication error variants
// #Returned by authenticate() and related functions
type AuthError =
    | NotFound
    | Unauthorized
    | Locked(attempts: int)
    | Expired
```

---

## Format Rules

### Marker Placement

The marker immediately follows `// ` with no extra space:

```sigil
// #Correct format
// # Wrong format (space after #)

// @param id correct
// @param  id wrong (extra space)

// !NotFound: correct
// ! NotFound: wrong (space after !)

// >expr -> result correct
// > expr -> result wrong (space after >)
```

The formatter enforces this automatically.

---

## LSP Integration

Doc comments appear in hover information:

```
@authenticate (username: str, password: str) -> Result<Session, AuthError>

Authenticates user and returns session

Parameters:
  username: str — case-insensitive
  password: str — minimum 8 characters

Errors:
  InvalidCredentials — wrong username or password
  AccountLocked — too many failed attempts

Example:
  authenticate("alice", "password123") -> Ok(Session{...})
```

---

## Comparison with Other Languages

### TypeScript/JSDoc (Verbose)

```typescript
/**
 * Fetches user data from the database.
 *
 * @param id - The user ID to fetch
 * @param timeout - Timeout in milliseconds
 * @returns A Promise containing the User or an error
 * @throws NotFound if the user doesn't exist
 * @example
 * const user = await fetchUser("123", 5000);
 */
```

### Sigil (Concise)

```sigil
// #Fetches user from database
// !NotFound: user doesn't exist
// >fetch_user("123", 5000) -> Ok(User{...})
@fetch_user (id: str, timeout: int) -> Result<User, DbError> = ...
```

**15+ lines → 4 lines.** Types document the rest.

---

## Best Practices

### Skip Obvious Documentation

```sigil
// Bad: adds no information
// #Adds two numbers
@add (a: int, b: int) -> int = a + b

// Good: no comment needed
@add (a: int, b: int) -> int = a + b
```

### Document Non-Obvious Behavior

```sigil
// Good: explains surprising behavior
// #Saturates at int max instead of overflowing
@add_saturating (a: int, b: int) -> int = ...

// Good: documents important constraint
// #Thread-safe; can be called from multiple goroutines
@get_cached (key: str) -> Option<Data> = ...
```

### Document All Error Conditions

```sigil
// Good: comprehensive error docs
// !NotFound: user doesn't exist
// !Unauthorized: invalid credentials
// !RateLimited: too many requests
@get_user (id: str) -> Result<User, ApiError> = ...
```

### Keep Examples Simple

```sigil
// Good: clear, runnable examples
// >parse("42") -> Ok(42)
// >parse("abc") -> Err(ParseError)

// Bad: complex setup
// >config = Config.default(); parse_with_config("42", config) -> ...
```

---

## Why Markdown-Inspired

The syntax draws from Markdown conventions:

- `#` — heading (main description)
- `>` — blockquote (example/output)
- `!` — callout/warning (errors)
- `@` — ties into Sigil's sigil convention

This reads naturally as markdown, even without tooling.

---

## See Also

- [Basic Syntax](../02-syntax/01-basic-syntax.md)
- [Error Handling](../05-error-handling/index.md)
- [LSP](../12-tooling/05-lsp.md)
