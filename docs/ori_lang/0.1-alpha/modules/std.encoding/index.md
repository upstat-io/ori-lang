# std.encoding

Data encoding and decoding.

```ori
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

```ori
@encode (data: [byte]) -> str
@encode (data: str) -> str
```

Encodes data as base64 string.

```ori
use std.encoding.base64

base64.encode("hello")  // "aGVsbG8="
base64.encode([0x00, 0x01, 0x02])  // "AAEC"
```

---

### @decode

```ori
@decode (encoded: str) -> Result<[byte], EncodingError>
```

Decodes base64 string.

```ori
use std.encoding.base64

let bytes = base64.decode("aGVsbG8=")?  // [104, 101, 108, 108, 111]
str.from_bytes(bytes)  // "hello"
```

---

### URL-safe Base64

```ori
use std.encoding.base64 { encode_url, decode_url }

encode_url(data)  // Uses -_ instead of +/
decode_url(s)?
```

---

## std.encoding.hex

### @encode

```ori
@encode (data: [byte]) -> str
```

Encodes bytes as hexadecimal.

```ori
use std.encoding.hex

hex.encode([0xde, 0xad, 0xbe, 0xef])  // "deadbeef"
```

---

### @decode

```ori
@decode (encoded: str) -> Result<[byte], EncodingError>
```

Decodes hexadecimal string.

```ori
use std.encoding.hex

let bytes = hex.decode("deadbeef")?  // [0xde, 0xad, 0xbe, 0xef]
```

---

## std.encoding.url

### @encode

```ori
@encode (s: str) -> str
```

URL-encodes a string (percent-encoding).

```ori
use std.encoding.url

url.encode("hello world")  // "hello%20world"
url.encode("a=1&b=2")      // "a%3D1%26b%3D2"
```

---

### @decode

```ori
@decode (s: str) -> Result<str, EncodingError>
```

Decodes URL-encoded string.

```ori
use std.encoding.url

url.decode("hello%20world")?  // "hello world"
```

---

### @encode_component

```ori
@encode_component (s: str) -> str
```

Encodes URI component (stricter than `encode`).

---

## Types

### EncodingError

```ori
type EncodingError =
    | InvalidCharacter(char: char, position: int)
    | InvalidLength
    | InvalidPadding
```

---

## Examples

### Basic auth header

```ori
use std.encoding.base64 { encode }

@basic_auth (username: str, password: str) -> str =
    "Basic " + encode(username + ":" + password)
```

### Hex dump

```ori
use std.encoding.hex { encode }
use std.fs { read_bytes }

@hex_dump (path: str) uses FileSystem -> Result<str, Error> = {
    let bytes = read_bytes(path)?

    Ok(encode(bytes))
}
```

### URL query string

```ori
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
