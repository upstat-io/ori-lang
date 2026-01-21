# std.encoding

Data encoding and decoding.

```sigil
use std.encoding.base64 { encode, decode }
use std.encoding.hex { encode, decode }
```

**No capability required**

---

## Overview

The `std.encoding` module provides:

- Base64 encoding/decoding
- Hexadecimal encoding/decoding
- URL encoding/decoding

---

## Submodules

| Module | Description |
|--------|-------------|
| [base64](base64.md) | Base64 encoding |
| [hex](hex.md) | Hexadecimal encoding |
| [url](url.md) | URL percent-encoding |

---

## std.encoding.base64

### @encode

```sigil
@encode (data: [byte]) -> str
@encode (data: str) -> str
```

Encodes data as base64 string.

```sigil
use std.encoding.base64

base64.encode("hello")  // "aGVsbG8="
base64.encode([0x00, 0x01, 0x02])  // "AAEC"
```

---

### @decode

```sigil
@decode (encoded: str) -> Result<[byte], EncodingError>
```

Decodes base64 string.

```sigil
use std.encoding.base64

let bytes = base64.decode("aGVsbG8=")?  // [104, 101, 108, 108, 111]
str.from_bytes(bytes)  // "hello"
```

---

### URL-safe Base64

```sigil
use std.encoding.base64 { encode_url, decode_url }

encode_url(data)  // Uses -_ instead of +/
decode_url(s)?
```

---

## std.encoding.hex

### @encode

```sigil
@encode (data: [byte]) -> str
```

Encodes bytes as hexadecimal.

```sigil
use std.encoding.hex

hex.encode([0xde, 0xad, 0xbe, 0xef])  // "deadbeef"
```

---

### @decode

```sigil
@decode (encoded: str) -> Result<[byte], EncodingError>
```

Decodes hexadecimal string.

```sigil
use std.encoding.hex

let bytes = hex.decode("deadbeef")?  // [0xde, 0xad, 0xbe, 0xef]
```

---

## std.encoding.url

### @encode

```sigil
@encode (s: str) -> str
```

URL-encodes a string (percent-encoding).

```sigil
use std.encoding.url

url.encode("hello world")  // "hello%20world"
url.encode("a=1&b=2")      // "a%3D1%26b%3D2"
```

---

### @decode

```sigil
@decode (s: str) -> Result<str, EncodingError>
```

Decodes URL-encoded string.

```sigil
use std.encoding.url

url.decode("hello%20world")?  // "hello world"
```

---

### @encode_component

```sigil
@encode_component (s: str) -> str
```

Encodes URI component (stricter than `encode`).

---

## Types

### EncodingError

```sigil
type EncodingError =
    | InvalidCharacter(char: char, position: int)
    | InvalidLength
    | InvalidPadding
```

---

## Examples

### Basic auth header

```sigil
use std.encoding.base64 { encode }

@basic_auth (username: str, password: str) -> str =
    "Basic " + encode(username + ":" + password)
```

### Hex dump

```sigil
use std.encoding.hex { encode }
use std.fs { read_bytes }

@hex_dump (path: str) uses FileSystem -> Result<str, Error> = run(
    let bytes = read_bytes(path)?,
    Ok(encode(bytes)),
)
```

### URL query string

```sigil
use std.encoding.url { encode }
use std.text { join }

@build_query (params: {str: str}) -> str =
    params.entries()
    | map(_, (k, v) -> encode(k) + "=" + encode(v))
    | join("&")
```

---

## See Also

- [std.json](../std.json/) — JSON encoding
- [std.crypto](../std.crypto/) — Cryptographic operations
