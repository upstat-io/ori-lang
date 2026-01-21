# Types

This section defines the type system of Sigil.

## Type Syntax

```
type          = async_type
              | type_path [ type_args ]
              | list_type
              | map_type
              | tuple_type
              | function_type
              | "dyn" type .

async_type    = "async" type .

type_path     = identifier { "." identifier } .
type_args     = "<" type { "," type } ">" .
list_type     = "[" type "]" .
map_type      = "{" type ":" type "}" .
tuple_type    = "(" type { "," type } ")" | "()" .
function_type = "(" [ type { "," type } ] ")" "->" type .
```

## Primitive Types

### Integer Type

```
int
```

A 64-bit signed integer.

- Range: -9,223,372,036,854,775,808 to 9,223,372,036,854,775,807
- Default value: 0

### Floating-Point Type

```
float
```

A 64-bit IEEE 754 double-precision floating-point number.

- Default value: 0.0
- Special values: positive infinity, negative infinity, NaN

### Boolean Type

```
bool
```

A boolean value.

- Values: `true`, `false`
- Default value: `false`

### String Type

```
str
```

A UTF-8 encoded string.

- Immutable sequence of Unicode code points
- Default value: `""`

### Byte Type

```
byte
```

An 8-bit unsigned integer.

- Range: 0 to 255
- Default value: 0

### Character Type

```
char
```

A single Unicode code point.

- Size: 4 bytes (32 bits)
- Range: U+0000 to U+10FFFF (excluding surrogate code points U+D800 to U+DFFF)
- Represents a single Unicode scalar value

```sigil
'a'         // ASCII character
'λ'         // Greek letter
'🦀'        // Emoji
'\n'        // Newline
'\t'        // Tab
```

A `char` is distinct from a single-character `str`. Converting between them requires explicit conversion.

### Void Type

```
void
```

The unit type, representing the absence of a meaningful value.

- Single value: `()`
- `void` is an alias for the unit tuple type `()`

### Never Type

```
Never
```

The bottom type, representing computations that never produce a value.

- Has no values (uninhabited type)
- A function returning `Never` never returns normally
- Can be coerced to any other type

```sigil
@panic (message: str) -> Never = ...

@unreachable () -> Never = panic("unreachable code")

// Never coerces to any type
@example () -> int = if condition then 42 else panic("failed")
```

`Never` is used for:
- Functions that always panic
- Functions that loop forever
- Exhaustive match arms that are unreachable
- `Result<T, Never>` indicates infallible operations

### Duration Type

```
Duration
```

A span of time.

- Internal representation: 64-bit integer (nanoseconds)
- Constructed via duration literals: `100ms`, `30s`, `5m`, `2h`

### Size Type

```
Size
```

A byte size.

- Internal representation: 64-bit integer (bytes)
- Constructed via size literals: `1024b`, `4kb`, `10mb`, `2gb`

## Compound Types

### List Type

```
[ T ]
```

An ordered, homogeneous collection of elements of type `T`.

```sigil
[int]           // list of integers
[str]           // list of strings
[[int]]         // list of lists of integers
```

### Map Type

```
{ K : V }
```

A collection of key-value pairs where keys are of type `K` and values are of type `V`.

```sigil
{str: int}      // map from strings to integers
{int: User}     // map from integers to Users
```

Keys must implement the `Eq` and `Hashable` traits.

### Set Type

```
Set< T >
```

An unordered collection of unique elements of type `T`.

```sigil
Set<int>        // set of integers
Set<str>        // set of strings
Set<UserId>     // set of user IDs
```

Elements must implement the `Eq` and `Hashable` traits.

```sigil
let ids = Set<int>.new()
ids.insert(1)
ids.insert(2)
ids.insert(1)   // no effect, already present
ids.len()       // 2
ids.contains(1) // true
```

### Tuple Type

```
( T1 , T2 , ... )
```

A fixed-size, heterogeneous collection.

```sigil
(int, str)          // pair
(int, str, bool)    // triple
()                  // unit (zero elements)
```

The unit type `()` is the tuple with zero elements.

### Function Type

```
( T1 , T2 , ... ) -> R
```

A function that takes parameters of types `T1`, `T2`, etc., and returns a value of type `R`.

```sigil
(int) -> int            // function taking int, returning int
(int, int) -> bool      // function taking two ints, returning bool
() -> void              // function taking no args, returning void
```

### Async Type

```
async T
```

An `async` type represents a computation that may suspend and eventually produce a value of type `T`.

- `async T` values are awaitable with the `.await` postfix operator
- `.await` yields a value of type `T`

```sigil
@fetch_user (id: int) -> async Result<User, Error> = ...

let result: Result<User, Error> = fetch_user(42).await
```

### Range Type

```
Range< T >
```

A range of values from a start to an end bound.

Ranges are produced by the range operators `..` (exclusive end) and `..=` (inclusive end):

```sigil
0..10       // Range<int>: 0, 1, 2, ..., 9
0..=10      // Range<int>: 0, 1, 2, ..., 10
'a'..'z'    // Range<char>: 'a', 'b', ..., 'y'
```

Range bounds must implement the `Comparable` trait.

Ranges are iterable and commonly used with `for` and collection patterns:

```sigil
for i in 0..10 do print(str(i))

let squares = collect(.range: 1..=10, .transform: x -> x * x)
```

## Generic Types

### Generic Type Application

A generic type is instantiated by providing type arguments:

```sigil
Option<int>
Result<User, Error>
Map<str, int>
```

### Type Parameters

Type parameters are declared in angle brackets:

```sigil
type Pair<T> = { first: T, second: T }
type Result<T, E> = Ok(T) | Err(E)
```

## Built-in Generic Types

### Option Type

```sigil
type Option<T> = Some(T) | None
```

Represents an optional value.

- `Some(value)` — a value is present
- `None` — no value

### Result Type

```sigil
type Result<T, E> = Ok(T) | Err(E)
```

Represents a computation that may succeed or fail.

- `Ok(value)` — success with value
- `Err(error)` — failure with error

### Ordering Type

```sigil
type Ordering = Less | Equal | Greater
```

Represents the result of a comparison between two values.

- `Less` — the first value is less than the second
- `Equal` — the values are equal
- `Greater` — the first value is greater than the second

`Ordering` is returned by the `compare` method of the `Comparable` trait.

```sigil
compare(1, 2)   // Less
compare(2, 2)   // Equal
compare(3, 2)   // Greater
```

### Error Type

```sigil
type Error = {
    message: str,
    source: Option<Error>,
}
```

The standard error type for general error handling.

- `message` — a human-readable error description
- `source` — an optional underlying cause (for error chaining)

```sigil
let err = Error { message: "connection failed", source: None }

let wrapped = Error {
    message: "failed to fetch user",
    source: Some(original_error),
}
```

For domain-specific errors, define custom sum types. Use `Error` for general-purpose error handling or when aggregating errors from multiple sources.

### Channel Type

```sigil
Channel< T >
```

A typed, bounded channel for communication between concurrent tasks.

- All channels have a fixed buffer size (no unbounded channels)
- Send blocks when the buffer is full
- Receive blocks when the buffer is empty
- Channels are the primary mechanism for sharing data between tasks

```sigil
let ch = Channel<int>.new(buffer: 10)

ch.send(42).await       // send a value
let value = ch.receive().await  // receive a value (Option<T>)
ch.close()              // close the channel
```

See [Capabilities § Async](14-capabilities.md) for details on async operations.

## User-Defined Types

### Struct Types

A struct type is a product type with named fields:

```
struct_type   = "{" [ field { "," field } [ "," ] ] "}" .
field         = identifier ":" type .
```

```sigil
type Point = { x: int, y: int }
type User = { id: int, name: str, email: str }
```

### Sum Types

A sum type (enum) is a type with multiple variants:

```
sum_type      = variant { "|" variant } .
variant       = identifier [ variant_data ] .
variant_data  = "(" [ field { "," field } ] ")" .
```

```sigil
type Status = Pending | Running | Done | Failed
type Option<T> = Some(T) | None
type Result<T, E> = Ok(T) | Err(E)
```

### Newtype

A newtype creates a distinct type from an existing type:

```sigil
type UserId = str
type Email = str
type Timestamp = int
```

Newtypes are nominally distinct from their underlying type.

## Type Definitions

### Syntax

```
type_def      = [ "pub" ] [ derive ] "type" identifier [ generics ] [ where_clause ] "=" type_body .
derive        = "#[derive(" identifier { "," identifier } ")]" .
generics      = "<" generic_param { "," generic_param } ">" .
generic_param = identifier [ ":" bounds ] .
bounds        = type_path { "+" type_path } .
type_body     = struct_type | sum_type | type .
```

The optional `where_clause` constrains type parameters. See [Properties of Types § Type Constraints](07-properties-of-types.md#type-constraints).

### Visibility

Types are private by default. The `pub` modifier exports the type:

```sigil
pub type User = { id: int, name: str }
type InternalState = { ... }  // private
```

### Derive Attribute

The `#[derive(...)]` attribute auto-implements traits:

```sigil
#[derive(Eq, Hashable, Clone)]
type Point = { x: int, y: int }

type Cache<T> where T: Hashable = {
    items: {T: str},
}
```

Derivable traits: `Eq`, `Hashable`, `Comparable`, `Printable`, `Clone`, `Default`, `Serialize`, `Deserialize`.

## Nominal Typing

Sigil uses nominal typing for user-defined types. Two types are the same only if they have the same name.

```sigil
type Point2D = { x: int, y: int }
type Vector2D = { x: int, y: int }

// Point2D and Vector2D are distinct types
// even though they have identical structure
```

## Type Inference

Types are inferred where possible:

```sigil
let x = 42              // inferred: int
let s = "hello"         // inferred: str
let items = [1, 2, 3]   // inferred: [int]
```

Type annotations are required in:

1. Function parameter types
2. Function return types
3. Type definitions

```sigil
// Parameters and return type must be annotated
@add (a: int, b: int) -> int = a + b
```
