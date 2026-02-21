# std.crypto

Cryptographic functions.

```ori
use std.crypto { sha256, sha512, hmac }
use std.crypto.cipher { encrypt, decrypt }
```

**No capability required** (pure computations)

---

## Overview

The `std.crypto` module provides:

- Cryptographic hash functions (SHA-256, SHA-512, etc.)
- HMAC message authentication
- Symmetric encryption (AES)
- Secure random bytes

---

## Submodules

| Module | Description |
|--------|-------------|
| [std.crypto.cipher](cipher.md) | Symmetric encryption |
| [std.crypto.rand](rand.md) | Cryptographically secure random |

---

## Hash Functions

### @sha256

```ori
@sha256 (data: [byte]) -> [byte]
@sha256 (data: str) -> [byte]
```

Computes SHA-256 hash (32 bytes).

```ori
use std.crypto { sha256 }
use std.encoding.hex { encode }

let hash = sha256("hello world")
encode(hash)  // "b94d27b9934d3e08a52e52d7da7dabfa..."
```

---

### @sha512

```ori
@sha512 (data: [byte]) -> [byte]
@sha512 (data: str) -> [byte]
```

Computes SHA-512 hash (64 bytes).

---

### @sha256_hex

```ori
@sha256_hex (data: str) -> str
```

Computes SHA-256 and returns hex string.

```ori
use std.crypto { sha256_hex }

sha256_hex("hello")  // "2cf24dba5fb0a30e..."
```

---

### @md5

```ori
@md5 (data: [byte]) -> [byte]
@md5 (data: str) -> [byte]
```

Computes MD5 hash (16 bytes).

> **Warning:** MD5 is cryptographically broken. Use only for checksums, not security.

---

## HMAC

### @hmac

```ori
@hmac (key: [byte], data: [byte], algorithm: HashAlgorithm) -> [byte]
```

Computes HMAC.

```ori
use std.crypto { hmac, HashAlgorithm }

let mac = hmac(key, message, HashAlgorithm.Sha256)
```

---

### @hmac_sha256

```ori
@hmac_sha256 (key: [byte], data: [byte]) -> [byte]
@hmac_sha256 (key: str, data: str) -> [byte]
```

Computes HMAC-SHA256.

```ori
use std.crypto { hmac_sha256 }
use std.encoding.hex { encode }

let mac = hmac_sha256("secret", "message")
encode(mac)  // hex string
```

---

## Types

### HashAlgorithm

```ori
type HashAlgorithm = Sha256 | Sha512 | Sha1 | Md5
```

---

## Secure Comparison

### @constant_time_eq

```ori
@constant_time_eq (a: [byte], b: [byte]) -> bool
```

Compares byte arrays in constant time (prevents timing attacks).

```ori
use std.crypto { constant_time_eq }

let expected = sha256(password + salt)
let provided = sha256(input + salt)

if constant_time_eq(expected, provided) then
    authenticate()
```

---

## Password Hashing

### @hash_password

```ori
@hash_password (password: str) -> str
```

Hashes password using secure algorithm (Argon2).

```ori
use std.crypto { hash_password, verify_password }

let hash = hash_password("user_password")
// Store hash in database

// Later, verify:
if verify_password("user_input", stored_hash) then
    login()
```

---

### @verify_password

```ori
@verify_password (password: str, hash: str) -> bool
```

Verifies password against hash.

---

## Examples

### File checksum

```ori
use std.crypto { sha256_hex }
use std.fs { read_bytes }

@checksum (path: str) uses FileSystem -> Result<str, Error> = {
    let data = read_bytes(path)?

    Ok(sha256_hex(str.from_bytes(data)))
}
```

### API signature

```ori
use std.crypto { hmac_sha256 }
use std.encoding.base64 { encode }
use std.time { now }

@sign_request (method: str, path: str, secret: str) uses Clock -> str = {
    let timestamp = str(now().unix())
    let message = method + "\n" + path + "\n" + timestamp
    let signature = hmac_sha256(secret, message)

    encode(signature)
}
```

---

## Security Notes

1. **Use sha256 or sha512** for new code, not MD5 or SHA-1
2. **Use constant_time_eq** when comparing secrets
3. **Use hash_password** for passwords, not raw SHA
4. **Generate keys** using `std.crypto.rand`, not `std.math.rand`

---

## See Also

- [std.crypto.cipher](cipher.md) — Encryption
- [std.crypto.rand](rand.md) — Secure random
- [std.encoding](../std.encoding/) — Hex/base64 encoding
