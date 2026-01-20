# Sigil Type System

**Nominal (Opaque) + Strong + Full Inference**

---

## Core Principles

1. **Nominal typing** — Types are distinct by name, not structure
2. **Strong typing** — No implicit conversions
3. **Full inference** — Types inferred within functions
4. **Explicit signatures** — Function boundaries require types
5. **Null safety** — `?T` for optional, `Result T E` for errors

---

## Primitive Types

```
int       // 64-bit signed integer
float     // 64-bit floating point
str       // UTF-8 string
bool      // true / false
void      // no value (for side-effect functions)
```

---

## Generic Types

Generics use single uppercase letters or PascalCase names:

```
T         // generic type parameter
K, V      // key/value pair
E         // error type
Item      // descriptive generic name
```

---

## Built-in Compound Types

### Optional

```
?T                    // T or nil

?int                  // int or nil
?str                  // str or nil
?User                 // User or nil
```

### List

```
[T]                   // list of T

[int]                 // list of ints
[str]                 // list of strings
[User]                // list of Users
[[int]]               // list of list of ints
```

### Map

```
{K: V}                // map from K to V

{str: int}            // string keys, int values
{UserId: User}        // UserId keys, User values
```

### Tuple

```
(T, U)                // pair
(T, U, V)             // triple

(int, str)            // int and string pair
(User, Session, Token) // triple
```

### Result

```
Result T E            // success T or error E

Result User AuthError // User on success, AuthError on failure
Result int str        // int on success, string error message
```

---

## User-Defined Types

### Struct (Product Type)

```
type User {
    id: UserId
    email: Email
    name: str
    active: bool
    tries: int
    created: Timestamp
}
```

- All fields required by default
- Fields are ordered
- Nominal: `User != {id, email, name, active, tries, created}`

### Newtype (Opaque Alias)

```
type UserId = str
type Email = str
type Timestamp = int
type Hash = str
```

- `UserId` and `Email` are **different types** even though both wrap `str`
- Prevents mixing up IDs with emails
- Requires explicit conversion

```
// This is a TYPE ERROR:
@find_user (id: UserId) -> ?User = ...

let email: Email = "foo@bar.com"
find_user(email)  // ERROR: Email != UserId
```

### Enum (Sum Type)

```
type AuthError =
    | NotFound
    | Inactive
    | Locked { attempts: int }
    | InvalidPassword
    | Expired { at: Timestamp }
```

- Each variant is a distinct constructor
- Variants can carry data

### Generic Types

```
type Result T E =
    | Ok { value: T }
    | Err { error: E }

type Option T =
    | Some { value: T }
    | None

type List T =
    | Nil
    | Cons { head: T, tail: List T }

type Tree T =
    | Leaf { value: T }
    | Node { left: Tree T, right: Tree T }
```

---

## Function Types

### Basic Syntax

```
@name (param: Type, param: Type) -> ReturnType
```

### Function Type Annotation

```
T -> U                      // function from T to U
(T, U) -> V                 // function taking two args
T -> U -> V                 // curried function
```

### Examples

```
int -> str                  // int to string
(int, int) -> int           // two ints to int
[T] -> ?T                   // list to optional element
(T, T -> bool) -> [T]       // filter signature
```

### Higher-Order Functions

```
@map T U (list: [T], f: T -> U) -> [U] = ...

@filter T (list: [T], pred: T -> bool) -> [T] = ...

@fold T U (list: [T], init: U, f: (U, T) -> U) -> U = ...
```

---

## Type Inference

### Within Functions: Fully Inferred

```
@process (user: User) -> str = run(
    name := user.name,           // inferred: str
    upper := name.upper(),       // inferred: str
    parts := upper.split(" "),   // inferred: [str]
    parts.first() ?? "Anonymous" // inferred: str
)
```

### At Boundaries: Explicit Required

```
// REQUIRED: explicit param and return types
@find_user (id: UserId) -> ?User

// ERROR: missing types
@find_user (id) -> ...
```

### Generic Inference

```
@identity T (x: T) -> T = x

let a = identity(5)        // inferred: int
let b = identity("hello")  // inferred: str
```

---

## Type Conversions

### Explicit Conversions

```
// Primitives
int(3.14)           // -> 3
float(42)           // -> 42.0
str(123)            // -> "123"
bool(1)             // -> true

// Newtypes require explicit wrapping
UserId("abc-123")   // str -> UserId
Email("a@b.com")    // str -> Email

// Unwrapping
user_id.unwrap()    // UserId -> str
```

### No Implicit Conversions

```
let x: int = 3.14           // ERROR: float != int
let id: UserId = "abc"      // ERROR: str != UserId

// Must be explicit:
let x: int = int(3.14)      // OK
let id: UserId = UserId("abc")  // OK
```

---

## Pattern Matching with Types

### Match on Enum

```
@handle_error (err: AuthError) -> str = match(
    err,
    NotFound         : "User not found",
    Inactive         : "Account inactive",
    Locked { attempts } : "Locked after " + str(attempts),
    InvalidPassword  : "Wrong password",
    Expired { at }   : "Expired at " + str(at)
)
```

### Match on Result

```
@process (res: Result User AuthError) -> str = match(
    res,
    Ok { value }  : "Hello " + value.name,
    Err { error } : handle_error(error)
)
```

### Match on Option

```
@greet (name: ?str) -> str = match(
    name,
    Some { value } : "Hello " + value,
    None           : "Hello stranger"
)

// Or use ?? operator
@greet (name: ?str) -> str = "Hello " + (name ?? "stranger")
```

---

## Type Constraints

### Where Clauses (Future)

```
@sort T (list: [T]) -> [T]
    where T: Ord
    = ...

@print_all T (list: [T]) -> void
    where T: Show
    = ...
```

### Built-in Traits/Constraints

| Constraint | Meaning |
|------------|---------|
| `Eq` | Equality comparable |
| `Ord` | Orderable (< > <= >=) |
| `Show` | Convertible to string |
| `Hash` | Hashable (for map keys) |
| `Clone` | Copyable |

---

## Complete Example

```
// types.si

// Opaque newtypes
type UserId = str
type Email = str
type Hash = str
type Timestamp = int

// Struct
type User {
    id: UserId
    email: Email
    name: str
    hash: Hash
    active: bool
    tries: int
    created: Timestamp
}

type Session {
    id: str
    user_id: UserId
    expires: Timestamp
}

// Sum type for errors
type AuthError =
    | UserNotFound
    | UserInactive
    | AccountLocked { attempts: int }
    | InvalidPassword
    | SessionExpired

// Config
$max_attempts = 5
$session_duration = 24h

// Functions with full type signatures
@find_user (id: UserId) -> ?User =
    db.get("users", id)

@verify_password (input: str, hash: Hash) -> bool =
    crypto.verify(input, hash)

@create_session (user: User) -> Session =
    Session {
        id: crypto.random_id(),
        user_id: user.id,
        expires: now() + $session_duration
    }

@authenticate (email: Email, password: str) -> Result Session AuthError = match(
    find_by_email(email),

    None : Err(UserNotFound),

    Some { value: user } : match(
        !user.active           : Err(UserInactive),
        user.tries >= $max_attempts : Err(AccountLocked { attempts: user.tries }),
        !verify_password(password, user.hash) : run(
            user.tries := user.tries + 1,
            save(user),
            Err(InvalidPassword)
        ),
        run(
            user.tries := 0,
            save(user),
            Ok(create_session(user))
        )
    )
)

// Usage - types are inferred within function
@main () -> void = run(
    result := authenticate(Email("user@test.com"), "password123"),

    match(result,
        Ok { value: session } : print("Session: " + session.id),
        Err { error }         : print("Error: " + str(error))
    )
)
```

---

## Type System Summary

| Feature | Sigil |
|---------|---------|
| Typing | Static, Strong, Nominal |
| Inference | Full (within functions) |
| Signatures | Explicit required |
| Nullability | `?T` (Option) |
| Errors | `Result T E` |
| Generics | `T`, `K V`, etc. |
| Newtypes | Opaque (UserId ≠ str) |
| Sum types | `\| Variant` syntax |
| Product types | `type Name { fields }` |
| Conversions | Explicit only |

---

## Benefits of Strict Typing

| Benefit | How |
|---------|-----|
| **Prevents mixing up values** | `UserId` ≠ `Email` even though both are strings |
| **Self-documenting** | Types communicate intent |
| **Catches errors early** | Type mismatches caught at compile time |
| **Inference reduces boilerplate** | No redundant type annotations |
| **Explicit boundaries** | Function signatures are clear contracts |
| **Pattern matching** | Exhaustive checking ensures all cases handled |
