# std.json

JSON encoding and decoding.

```sigil
use std.json { parse, stringify, Value }
```

**No capability required**

---

## Overview

The `std.json` module provides:

- Parsing JSON strings to typed values
- Serializing values to JSON strings
- Dynamic JSON value manipulation

---

## Functions

### @parse

```sigil
@parse<T> (json: str) -> Result<T, JsonError>
```

Parses JSON string into a typed value.

```sigil
use std.json { parse }

type User = { name: str, age: int }

let user = parse<User>("{\"name\": \"Alice\", \"age\": 30}")?
// User { name: "Alice", age: 30 }

let numbers = parse<[int]>("[1, 2, 3]")?
// [1, 2, 3]
```

**Type mapping:**

| JSON | Sigil |
|------|-------|
| `null` | `None` (in `Option<T>`) |
| `true`/`false` | `bool` |
| `number` (integer) | `int` |
| `number` (decimal) | `float` |
| `string` | `str` |
| `array` | `[T]` |
| `object` | Struct or `{str: T}` |

---

### @stringify

```sigil
@stringify<T> (value: T) -> str
@stringify<T> (value: T, pretty: bool) -> str
```

Converts a value to JSON string.

```sigil
use std.json { stringify }

type User = { name: str, age: int }
let user = User { name: "Alice", age: 30 }

stringify(user)
// {"name":"Alice","age":30}

stringify(user, pretty: true)
// {
//   "name": "Alice",
//   "age": 30
// }
```

---

### @parse_value

```sigil
@parse_value (json: str) -> Result<Value, JsonError>
```

Parses JSON into dynamic `Value` type.

```sigil
use std.json { parse_value, Value }

let v = parse_value("{\"name\": \"Alice\"}")?

match(v,
    Value.Object(map) -> map["name"],
    _ -> None,
)
```

---

## Types

### Value

```sigil
type Value =
    | Null
    | Bool(bool)
    | Int(int)
    | Float(float)
    | String(str)
    | Array([Value])
    | Object({str: Value})
```

Dynamic JSON value for untyped handling.

**Methods:**
- `as_bool() -> Option<bool>`
- `as_int() -> Option<int>`
- `as_float() -> Option<float>`
- `as_str() -> Option<str>`
- `as_array() -> Option<[Value]>`
- `as_object() -> Option<{str: Value}>`
- `get(key: str) -> Option<Value>` — For objects
- `index(i: int) -> Option<Value>` — For arrays

---

### JsonError

```sigil
type JsonError =
    | ParseError(message: str, line: int, column: int)
    | TypeError(expected: str, got: str)
    | MissingField(field: str)
```

---

## Derive Support

Types with `#[derive(Serialize, Deserialize)]` can be used with `parse` and `stringify`:

```sigil
#[derive(Serialize, Deserialize)]
type Config = {
    host: str,
    port: int,
    debug: bool,
}

let config = parse<Config>(json_str)?
let json = stringify(config)
```

### Field Attributes

```sigil
#[derive(Serialize, Deserialize)]
type User = {
    #[json(name = "user_id")]
    id: int,

    #[json(skip_if_none)]
    nickname: Option<str>,

    #[json(default = 0)]
    score: int,
}
```

---

## Examples

### Config file loading

```sigil
use std.json { parse }
use std.fs { read_file }

type Config = {
    database: DatabaseConfig,
    server: ServerConfig,
}

type DatabaseConfig = { url: str, pool_size: int }
type ServerConfig = { host: str, port: int }

@load_config (path: str) uses FileSystem -> Result<Config, Error> = run(
    let content = read_file(path)?,
    parse<Config>(content).map_err(e -> Error {
        message: "Invalid config: " + e.message,
        source: None,
    }),
)
```

### API response handling

```sigil
use std.json { parse }
use std.net.http { get }

type ApiResponse<T> = {
    success: bool,
    data: Option<T>,
    error: Option<str>,
}

type User = { id: int, name: str }

@fetch_user (id: int) -> Result<User, Error> uses Http, Async = run(
    let resp = get("https://api.example.com/users/" + str(id))?,
    let api_resp = parse<ApiResponse<User>>(resp.body)?,
    match(api_resp,
        { success: true, data: Some(user), .. } -> Ok(user),
        { error: Some(msg), .. } -> Err(Error { message: msg, source: None }),
        _ -> Err(Error { message: "Unknown error", source: None }),
    ),
)
```

### Dynamic JSON handling

```sigil
use std.json { parse_value, Value }

@extract_field (json: str, field: str) -> Option<str> = run(
    let v = parse_value(json).ok()?,
    let obj = v.as_object()?,
    let val = obj[field]?,
    val.as_str(),
)
```

---

## See Also

- [std.encoding](../std.encoding/) — Other encodings
- [std.net.http](../std.net/http.md) — HTTP with JSON
