# Proposal: std.json API Design

**Status:** Approved
**Approved:** 2026-01-30
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Standard library

---

## Summary

This proposal defines the API for `std.json`, providing JSON parsing, serialization, and manipulation capabilities.

---

## Motivation

JSON is ubiquitous for data interchange. A standard JSON library should provide:

1. Parse JSON strings to Ori values
2. Serialize Ori values to JSON strings
3. Work with arbitrary JSON (when schema unknown)
4. Derive serialization for user types
5. Handle errors gracefully

---

## Core Types

### JsonValue

Represents any JSON value:

```ori
type JsonValue =
    | Null
    | Bool(bool)
    | Number(float)
    | String(str)
    | Array([JsonValue])
    | Object({str: JsonValue})
```

### Json Trait

For types that can be serialized to/from JSON:

```ori
trait Json {
    @to_json (self) -> JsonValue
    @from_json (json: JsonValue) -> Result<Self, JsonError>
}
```

### JsonError

```ori
type JsonError = {
    kind: JsonErrorKind,
    message: str,
    path: str,        // JSON path where error occurred (e.g., ".users[0].name")
    position: int,    // Character position in source (for parse errors)
}

type JsonErrorKind =
    | ParseError
    | TypeError       // Expected different JSON type
    | MissingField    // Required field not present
    | UnknownField    // Field not in target type (strict mode)
    | ValueError      // Value out of range or invalid
```

---

## Parsing API

### parse

Parse a JSON string:

```ori
@parse (source: str) -> Result<JsonValue, JsonError>
```

Usage:
```ori
use std.json { parse }

let json = parse(source: `{"name": "Alice", "age": 30}`)?
// json: JsonValue.Object({"name": String("Alice"), "age": Number(30)})
```

### parse_as

Parse directly to a typed value:

```ori
@parse_as<T: Json> (source: str) -> Result<T, JsonError>
```

Usage:
```ori
use std.json { parse_as }

type User = { name: str, age: int }
#derive(Json)

let user = parse_as<User>(source: `{"name": "Alice", "age": 30}`)?
// user: User { name: "Alice", age: 30 }
```

---

## Serialization API

### stringify

Convert JsonValue to string:

```ori
@stringify (value: JsonValue) -> str
@stringify_pretty (value: JsonValue, indent: int = 2) -> str
```

Usage:
```ori
use std.json { stringify, stringify_pretty }

let json = JsonValue.Object({
    "name": JsonValue.String("Alice"),
    "active": JsonValue.Bool(true),
})

stringify(value: json)
// `{"name":"Alice","active":true}`

stringify_pretty(value: json, indent: 2)
// `{
//   "name": "Alice",
//   "active": true
// }`
```

### to_json_string

Serialize typed value directly:

```ori
@to_json_string<T: Json> (value: T) -> str
@to_json_string_pretty<T: Json> (value: T, indent: int = 2) -> str
```

Usage:
```ori
use std.json { to_json_string }

let user = User { name: "Alice", age: 30 }
to_json_string(value: user)
// `{"name":"Alice","age":30}`
```

---

## JsonValue Methods

### Accessors

```ori
impl JsonValue {
    // Type checks
    @is_null (self) -> bool
    @is_bool (self) -> bool
    @is_number (self) -> bool
    @is_string (self) -> bool
    @is_array (self) -> bool
    @is_object (self) -> bool

    // Safe extraction (returns Option)
    @as_bool (self) -> Option<bool>
    @as_number (self) -> Option<float>
    @as_int (self) -> Option<int>
    @as_string (self) -> Option<str>
    @as_array (self) -> Option<[JsonValue]>
    @as_object (self) -> Option<{str: JsonValue}>

    // Indexing
    @get (self, key: str) -> Option<JsonValue>      // For objects
    @get_index (self, index: int) -> Option<JsonValue>  // For arrays
}
```

#### `as_int` Semantics

`as_int()` returns `Some(n)` only if the number:
- Has no fractional part (e.g., `1.0` → `Some(1)`, `1.5` → `None`)
- Fits within int range (-2^63 to 2^63-1)

For explicit truncation, use `as_number().map(n -> int(n))`.

Usage:
```ori
let json = parse(source: data)?

// Safe access with Option chaining
let name = json.get(key: "user")
    .and_then(u -> u.get(key: "name"))
    .and_then(n -> n.as_string())
    .unwrap_or(default: "Unknown")

// Array access
let first_item = json.get(key: "items")
    .and_then(arr -> arr.get_index(index: 0))
```

### Path Access

Access nested values with dot notation:

```ori
impl JsonValue {
    @at (self, path: str) -> Option<JsonValue>
}
```

Usage:
```ori
let json = parse(source: complex_data)?

// Path syntax: "field.subfield[0].name"
json.at(path: "users[0].address.city")
    .and_then(v -> v.as_string())
// Some("New York")
```

---

## Derive Macro

### Basic Derivation

```ori
#derive(Json)
type User = {
    name: str,
    age: int,
    email: Option<str>,
}

// Generates:
impl Json for User {
    @to_json (self) -> JsonValue = JsonValue.Object({
        "name": JsonValue.String(self.name),
        "age": JsonValue.Number(float(self.age)),
        "email": match(self.email,
            Some(e) -> JsonValue.String(e),
            None -> JsonValue.Null,
        ),
    })

    @from_json (json: JsonValue) -> Result<User, JsonError> = ...
}
```

### Field Attributes

When deriving `Json`, fields may be annotated with `#json(...)` to customize serialization behavior. These attributes are specific to the `Json` derive macro, not a general language feature.

```ori
#derive(Json)
type ApiResponse = {
    #json(rename: "user_id")
    id: int,

    #json(skip)
    internal_data: str,

    #json(default: "unknown")
    source: str,

    #json(flatten)
    metadata: Metadata,
}
```

| Attribute | Description |
|-----------|-------------|
| `rename: "name"` | Use different JSON field name |
| `skip` | Don't include in JSON |
| `default: value` | Use default if field missing |
| `flatten` | Merge nested object into parent. **Compile error if field names conflict.** |

#### Flatten Conflict Example

```ori
#derive(Json)
type Parent = {
    name: str,
    #json(flatten)
    child: Child,
}

#derive(Json)
type Child = {
    name: str,  // ERROR: field "name" conflicts with Parent.name
}
```

### Enum Serialization

```ori
#derive(Json)
type Status = Active | Inactive | Pending(str)

// Serializes as:
// Active -> "Active"
// Inactive -> "Inactive"
// Pending("reason") -> {"Pending": "reason"}
```

With explicit representation:

```ori
#derive(Json)
#json(tag: "type", content: "data")
type Event =
    | Click { x: int, y: int }
    | Scroll { delta: int }

// Serializes as:
// Click { x: 10, y: 20 } -> {"type": "Click", "data": {"x": 10, "y": 20}}
```

---

## Standard Type Implementations

### Primitives

| Type | JSON Representation |
|------|---------------------|
| `bool` | `true` / `false` |
| `int` | Number |
| `float` | Number |
| `str` | String |

### Precision Note

JSON numbers are IEEE 754 doubles. Integers larger than 2^53 (9,007,199,254,740,992) may lose precision during round-trip serialization. For applications requiring larger integers:

- Use string representation in the JSON
- Provide custom `Json` implementation

A future proposal may add explicit `BigInt` support.

### Collections

| Type | JSON Representation |
|------|---------------------|
| `[T]` | Array |
| `{str: V}` | Object |
| `Set<T>` | Array (order undefined) |
| `Option<T>` | `null` or value |
| `(A, B)` | Array `[a, b]` |

### Built-in Type Extensions

`Duration` and `Size` (built-in types) have `Json` implementations provided by `std.json`. These are automatically available when `std.json` is imported.

| Type | JSON Format |
|------|-------------|
| `Duration` | ISO 8601 duration string (`"PT1H30M"`) |
| `Size` | Integer bytes (`1048576` for 1mb) |

```ori
// Duration serializes to ISO 8601 duration
Duration.from_seconds(3661).to_json()
// "PT1H1M1S"

// Non-string map keys serialize as strings
let map: {int: str} = {1: "a", 2: "b"}
map.to_json()
// {"1": "a", "2": "b"}
```

---

## Streaming API

For large JSON documents:

```ori
type JsonParser = { ... }

impl JsonParser {
    @new (source: str) -> JsonParser
}

impl Iterator for JsonParser {
    type Item = JsonEvent
    @next (self) -> (Option<JsonEvent>, JsonParser)
}

impl Iterable for JsonParser {
    type Item = JsonEvent
    @iter (self) -> JsonParser = self
}

type JsonEvent =
    | StartObject
    | EndObject
    | StartArray
    | EndArray
    | Key(str)
    | Value(JsonValue)
```

Usage:
```ori
use std.json { JsonParser }

let parser = JsonParser.new(source: large_json)
for event in parser do
    match(event,
        StartObject -> ...,
        Key(k) -> ...,
        Value(v) -> ...,
        _ -> (),
    )
```

---

## Examples

### Parse and Access

```ori
use std.json { parse }

@get_user_names (json_str: str) -> Result<[str], JsonError> = run(
    let json = parse(source: json_str)?,
    let users = json.get(key: "users")
        .and_then(u -> u.as_array())
        .ok_or(JsonError { kind: TypeError, message: "expected users array", path: "", position: 0 })?,
    Ok(users
        .map(u -> u.get(key: "name").and_then(n -> n.as_string()).unwrap_or(""))
        .collect()),
)
```

### Typed Deserialization

```ori
use std.json { parse_as }

#derive(Json)
type Config = {
    host: str,
    port: int,
    #json(default: false)
    debug: bool,
}

@load_config (path: str) -> Result<Config, Error> uses FileSystem = run(
    let content = FileSystem.read(path)?,
    parse_as<Config>(source: content).map_err(e -> Error::from(e)),
)
```

### Building JSON

```ori
use std.json { JsonValue, stringify }

@build_response (user: User, items: [Item]) -> str = run(
    let json = JsonValue.Object({
        "success": JsonValue.Bool(true),
        "user": user.to_json(),
        "items": JsonValue.Array(items.map(i -> i.to_json()).collect()),
        "count": JsonValue.Number(float(items.len())),
    }),
    stringify(value: json),
)
```

---

## Error Handling

### Parse Errors

```ori
let result = parse(source: `{"invalid": }`)
// Err(JsonError {
//     kind: ParseError,
//     message: "unexpected character '}'",
//     path: "",
//     position: 12,
// })
```

### Type Errors

```ori
#derive(Json)
type Person = { name: str, age: int }

let result = parse_as<Person>(source: `{"name": "Alice", "age": "thirty"}`)
// Err(JsonError {
//     kind: TypeError,
//     message: "expected number, got string",
//     path: ".age",
//     position: 0,
// })
```

---

## Module Structure

```ori
// std/json/mod.ori
pub use "./value" { JsonValue }
pub use "./error" { JsonError, JsonErrorKind }
pub use "./trait" { Json }
pub use "./parse" { parse, parse_as }
pub use "./stringify" { stringify, stringify_pretty, to_json_string, to_json_string_pretty }
pub use "./stream" { JsonParser, JsonEvent }
```

---

## Summary

| Function | Description |
|----------|-------------|
| `parse(source)` | Parse to `JsonValue` |
| `parse_as<T>(source)` | Parse to typed value |
| `stringify(value)` | Compact JSON string |
| `stringify_pretty(value, indent)` | Formatted JSON string |
| `to_json_string(value)` | Serialize typed value |
| `JsonValue.get(key)` | Object field access |
| `JsonValue.at(path)` | Path-based access |
| `#derive(Json)` | Auto-implement for types |
