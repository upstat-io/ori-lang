# Proposal: std.crypto FFI Implementation (Native)

**Status:** Approved
**Approved:** 2026-01-30
**Created:** 2026-01-30
**Affects:** Standard library
**Depends on:** Platform FFI proposal, std.crypto API proposal

---

## Summary

This proposal adds native FFI implementation details to the approved `std.crypto` API. Cryptographic operations are backed by **libsodium** for modern algorithms and **OpenSSL** for RSA algorithms.

**Scope:** Native platforms only (Linux, macOS, Windows). WASM support via `crypto.subtle` will be addressed in a separate proposal.

---

## FFI Implementation Decision

### Why libsodium?

| Library | Security | API | Algorithms | Maintenance |
|---------|----------|-----|------------|-------------|
| **libsodium** | Audited, misuse-resistant | Simple | Modern (Curve25519, ChaCha20, etc.) | Active |
| OpenSSL | CVE history | Complex | Comprehensive | Active |
| LibreSSL | Security-focused | Complex | Comprehensive | Active |
| ring (Rust) | Audited | Rust-only | Modern | Active |
| BoringSSL | Google-audited | Complex | Comprehensive | Active |

**Primary backend: libsodium**
- Designed to be hard to misuse (secure defaults)
- Modern algorithms (Ed25519, X25519, XSalsa20-Poly1305, Argon2)
- Simple, consistent API
- Cross-platform (including iOS, Android)
- Extensively audited
- MIT license

**Secondary backend: OpenSSL** (for RSA only)
- Required for RSA-2048/4096 signing and encryption
- Widely available on all platforms

### Algorithm Mapping

| Ori API | Backend | Function | Algorithm |
|---------|---------|----------|-----------|
| `hash_password` | libsodium | `crypto_pwhash_str` | Argon2id |
| `verify_password` | libsodium | `crypto_pwhash_str_verify` | Argon2id |
| `hash` (SHA-256) | libsodium | `crypto_hash_sha256` | SHA-256 |
| `hash` (SHA-512) | libsodium | `crypto_hash_sha512` | SHA-512 |
| `hash` (Blake2b) | libsodium | `crypto_generichash` | BLAKE2b |
| `hmac` (SHA-256) | libsodium | `crypto_auth_hmacsha256` | HMAC-SHA-256 |
| `hmac` (SHA-512) | libsodium | `crypto_auth_hmacsha512` | HMAC-SHA-512 |
| `hmac` (Blake2b) | libsodium | `crypto_generichash` (keyed) | BLAKE2b-MAC |
| `generate_key` | libsodium | `crypto_secretbox_keygen` | Random 256-bit |
| `encrypt` | libsodium | `crypto_secretbox_easy` | XSalsa20-Poly1305 |
| `decrypt` | libsodium | `crypto_secretbox_open_easy` | XSalsa20-Poly1305 |
| `encrypt_with_nonce` | libsodium | `crypto_aead_xchacha20poly1305_ietf_encrypt` | XChaCha20-Poly1305 |
| `decrypt_with_nonce` | libsodium | `crypto_aead_xchacha20poly1305_ietf_decrypt` | XChaCha20-Poly1305 |
| `generate_signing_keypair` (Ed25519) | libsodium | `crypto_sign_keypair` | Ed25519 |
| `generate_signing_keypair` (RSA) | OpenSSL | `RSA_generate_key_ex` | RSA |
| `sign` (Ed25519) | libsodium | `crypto_sign_detached` | Ed25519 |
| `sign` (RSA) | OpenSSL | `RSA_sign` | RSA-PKCS1 |
| `verify_signature` (Ed25519) | libsodium | `crypto_sign_verify_detached` | Ed25519 |
| `verify_signature` (RSA) | OpenSSL | `RSA_verify` | RSA-PKCS1 |
| `generate_encryption_keypair` | OpenSSL | `RSA_generate_key_ex` | RSA |
| `encrypt_for` | libsodium | `crypto_box_seal` | X25519+XSalsa20-Poly1305 |
| `decrypt_with` | libsodium | `crypto_box_seal_open` | X25519+XSalsa20-Poly1305 |
| `generate_key_exchange_keypair` | libsodium | `crypto_kx_keypair` | X25519 |
| `derive_shared_secret` | libsodium | `crypto_scalarmult` | X25519 |
| `random_bytes` | libsodium | `randombytes_buf` | ChaCha20-based CSPRNG |
| `random_int` | libsodium | `randombytes_uniform` | Uniform distribution |
| `derive_key` | libsodium | `crypto_pwhash` | Argon2id |
| `stretch_key` | libsodium | `crypto_kdf_derive_from_key` | BLAKE2b-KDF |
| `constant_time_eq` | libsodium | `sodium_memcmp` | Constant-time compare |

---

## Language Extensions

### Array Initialization Syntax

This proposal requires the `[value; count]` syntax for creating fixed-size arrays:

```ori
let buffer = [0 as byte; 32]  // Creates [0, 0, ..., 0] with 32 elements
```

This syntax should be added to the language spec.

### Zeroization Attribute

Types marked with `#zeroize` automatically have their memory cleared when they go out of scope:

```ori
#zeroize
type SecretKey = { bytes: [byte] }
```

The compiler inserts `sodium_memzero` calls at appropriate drop points. This provides secure memory cleanup without requiring a full Drop trait.

---

## External Declarations

### libsodium FFI

```ori
// std/crypto/ffi.ori (internal)

extern "c" from "sodium" {
    // Initialization
    @_sodium_init () -> int as "sodium_init"

    // Memory utilities
    @_sodium_memzero (pnt: [byte], len: int) -> void as "sodium_memzero"
    @_sodium_memcmp (b1: [byte], b2: [byte], len: int) -> int as "sodium_memcmp"

    // Random
    @_randombytes_buf (buf: [byte], size: int) -> void as "randombytes_buf"
    @_randombytes_uniform (upper_bound: int) -> int as "randombytes_uniform"

    // Password hashing (Argon2id)
    @_crypto_pwhash_str (
        out: [byte],
        passwd: str,
        passwdlen: int,
        opslimit: int,
        memlimit: int
    ) -> int as "crypto_pwhash_str"

    @_crypto_pwhash_str_verify (
        str: [byte],
        passwd: str,
        passwdlen: int
    ) -> int as "crypto_pwhash_str_verify"

    @_crypto_pwhash (
        out: [byte],
        outlen: int,
        passwd: str,
        passwdlen: int,
        salt: [byte],
        opslimit: int,
        memlimit: int,
        alg: int
    ) -> int as "crypto_pwhash"

    // Generic hashing (BLAKE2b)
    @_crypto_generichash (
        out: [byte],
        outlen: int,
        in_: [byte],
        inlen: int,
        key: [byte],
        keylen: int
    ) -> int as "crypto_generichash"

    // SHA-256
    @_crypto_hash_sha256 (out: [byte], in_: [byte], inlen: int) -> int as "crypto_hash_sha256"

    // SHA-512
    @_crypto_hash_sha512 (out: [byte], in_: [byte], inlen: int) -> int as "crypto_hash_sha512"

    // HMAC-SHA-256
    @_crypto_auth_hmacsha256 (out: [byte], in_: [byte], inlen: int, k: [byte]) -> int as "crypto_auth_hmacsha256"
    @_crypto_auth_hmacsha256_verify (h: [byte], in_: [byte], inlen: int, k: [byte]) -> int as "crypto_auth_hmacsha256_verify"

    // HMAC-SHA-512
    @_crypto_auth_hmacsha512 (out: [byte], in_: [byte], inlen: int, k: [byte]) -> int as "crypto_auth_hmacsha512"
    @_crypto_auth_hmacsha512_verify (h: [byte], in_: [byte], inlen: int, k: [byte]) -> int as "crypto_auth_hmacsha512_verify"

    // Symmetric encryption (XSalsa20-Poly1305)
    @_crypto_secretbox_keygen (k: [byte]) -> void as "crypto_secretbox_keygen"
    @_crypto_secretbox_easy (
        c: [byte],
        m: [byte],
        mlen: int,
        n: [byte],
        k: [byte]
    ) -> int as "crypto_secretbox_easy"
    @_crypto_secretbox_open_easy (
        m: [byte],
        c: [byte],
        clen: int,
        n: [byte],
        k: [byte]
    ) -> int as "crypto_secretbox_open_easy"

    // AEAD (XChaCha20-Poly1305) - for explicit nonce API
    @_crypto_aead_xchacha20poly1305_ietf_encrypt (
        c: [byte],
        clen_p: CPtr,
        m: [byte],
        mlen: int,
        ad: [byte],
        adlen: int,
        nsec: CPtr,
        npub: [byte],
        k: [byte]
    ) -> int as "crypto_aead_xchacha20poly1305_ietf_encrypt"

    @_crypto_aead_xchacha20poly1305_ietf_decrypt (
        m: [byte],
        mlen_p: CPtr,
        nsec: CPtr,
        c: [byte],
        clen: int,
        ad: [byte],
        adlen: int,
        npub: [byte],
        k: [byte]
    ) -> int as "crypto_aead_xchacha20poly1305_ietf_decrypt"

    // Ed25519 signatures
    @_crypto_sign_keypair (pk: [byte], sk: [byte]) -> int as "crypto_sign_keypair"
    @_crypto_sign_detached (
        sig: [byte],
        siglen_p: CPtr,
        m: [byte],
        mlen: int,
        sk: [byte]
    ) -> int as "crypto_sign_detached"
    @_crypto_sign_verify_detached (
        sig: [byte],
        m: [byte],
        mlen: int,
        pk: [byte]
    ) -> int as "crypto_sign_verify_detached"

    // X25519 key exchange
    @_crypto_kx_keypair (pk: [byte], sk: [byte]) -> int as "crypto_kx_keypair"
    @_crypto_kx_client_session_keys (
        rx: [byte],
        tx: [byte],
        client_pk: [byte],
        client_sk: [byte],
        server_pk: [byte]
    ) -> int as "crypto_kx_client_session_keys"
    @_crypto_kx_server_session_keys (
        rx: [byte],
        tx: [byte],
        server_pk: [byte],
        server_sk: [byte],
        client_pk: [byte]
    ) -> int as "crypto_kx_server_session_keys"

    // Scalar multiplication (raw X25519)
    @_crypto_scalarmult (q: [byte], n: [byte], p: [byte]) -> int as "crypto_scalarmult"
    @_crypto_scalarmult_base (q: [byte], n: [byte]) -> int as "crypto_scalarmult_base"

    // Key derivation (BLAKE2b-KDF)
    @_crypto_kdf_derive_from_key (
        subkey: [byte],
        subkey_len: int,
        subkey_id: int,
        ctx: [byte],
        key: [byte]
    ) -> int as "crypto_kdf_derive_from_key"

    // Public key encryption (sealed boxes)
    @_crypto_box_keypair (pk: [byte], sk: [byte]) -> int as "crypto_box_keypair"
    @_crypto_box_seal (c: [byte], m: [byte], mlen: int, pk: [byte]) -> int as "crypto_box_seal"
    @_crypto_box_seal_open (
        m: [byte],
        c: [byte],
        clen: int,
        pk: [byte],
        sk: [byte]
    ) -> int as "crypto_box_seal_open"
}

// Constants
let $crypto_secretbox_KEYBYTES: int = 32
let $crypto_secretbox_NONCEBYTES: int = 24
let $crypto_secretbox_MACBYTES: int = 16
let $crypto_aead_xchacha20poly1305_ietf_KEYBYTES: int = 32
let $crypto_aead_xchacha20poly1305_ietf_NPUBBYTES: int = 24
let $crypto_aead_xchacha20poly1305_ietf_ABYTES: int = 16
let $crypto_sign_PUBLICKEYBYTES: int = 32
let $crypto_sign_SECRETKEYBYTES: int = 64
let $crypto_sign_BYTES: int = 64
let $crypto_kx_PUBLICKEYBYTES: int = 32
let $crypto_kx_SECRETKEYBYTES: int = 32
let $crypto_kx_SESSIONKEYBYTES: int = 32
let $crypto_box_PUBLICKEYBYTES: int = 32
let $crypto_box_SECRETKEYBYTES: int = 32
let $crypto_box_SEALBYTES: int = 48
let $crypto_pwhash_STRBYTES: int = 128
let $crypto_pwhash_SALTBYTES: int = 16
let $crypto_pwhash_OPSLIMIT_INTERACTIVE: int = 2
let $crypto_pwhash_MEMLIMIT_INTERACTIVE: int = 67108864  // 64 MB
let $crypto_pwhash_OPSLIMIT_MODERATE: int = 3
let $crypto_pwhash_MEMLIMIT_MODERATE: int = 268435456  // 256 MB
let $crypto_pwhash_ALG_ARGON2ID13: int = 2
let $crypto_hash_sha256_BYTES: int = 32
let $crypto_hash_sha512_BYTES: int = 64
let $crypto_generichash_BYTES: int = 32
let $crypto_kdf_KEYBYTES: int = 32
let $crypto_kdf_CONTEXTBYTES: int = 8
```

### OpenSSL FFI (for RSA)

```ori
// std/crypto/ffi_openssl.ori (internal)

extern "c" from "crypto" {
    // BIGNUM for RSA exponent
    @_BN_new () -> CPtr as "BN_new"
    @_BN_free (bn: CPtr) -> void as "BN_free"
    @_BN_set_word (bn: CPtr, w: int) -> int as "BN_set_word"

    // RSA key management
    @_RSA_new () -> CPtr as "RSA_new"
    @_RSA_free (rsa: CPtr) -> void as "RSA_free"
    @_RSA_generate_key_ex (rsa: CPtr, bits: int, e: CPtr, cb: CPtr) -> int as "RSA_generate_key_ex"
    @_RSA_size (rsa: CPtr) -> int as "RSA_size"

    // RSA signing (using EVP for modern OpenSSL)
    @_EVP_PKEY_new () -> CPtr as "EVP_PKEY_new"
    @_EVP_PKEY_free (pkey: CPtr) -> void as "EVP_PKEY_free"
    @_EVP_PKEY_assign_RSA (pkey: CPtr, rsa: CPtr) -> int as "EVP_PKEY_assign_RSA"

    @_EVP_MD_CTX_new () -> CPtr as "EVP_MD_CTX_new"
    @_EVP_MD_CTX_free (ctx: CPtr) -> void as "EVP_MD_CTX_free"
    @_EVP_sha256 () -> CPtr as "EVP_sha256"

    @_EVP_DigestSignInit (ctx: CPtr, pctx: CPtr, type_: CPtr, e: CPtr, pkey: CPtr) -> int as "EVP_DigestSignInit"
    @_EVP_DigestSignUpdate (ctx: CPtr, d: [byte], cnt: int) -> int as "EVP_DigestSignUpdate"
    @_EVP_DigestSignFinal (ctx: CPtr, sig: [byte], siglen: CPtr) -> int as "EVP_DigestSignFinal"

    @_EVP_DigestVerifyInit (ctx: CPtr, pctx: CPtr, type_: CPtr, e: CPtr, pkey: CPtr) -> int as "EVP_DigestVerifyInit"
    @_EVP_DigestVerifyUpdate (ctx: CPtr, d: [byte], cnt: int) -> int as "EVP_DigestVerifyUpdate"
    @_EVP_DigestVerifyFinal (ctx: CPtr, sig: [byte], siglen: int) -> int as "EVP_DigestVerifyFinal"

    // RSA encryption/decryption
    @_RSA_public_encrypt (flen: int, from: [byte], to: [byte], rsa: CPtr, padding: int) -> int as "RSA_public_encrypt"
    @_RSA_private_decrypt (flen: int, from: [byte], to: [byte], rsa: CPtr, padding: int) -> int as "RSA_private_decrypt"

    // Key serialization (DER format)
    @_i2d_RSAPublicKey (rsa: CPtr, pp: CPtr) -> int as "i2d_RSAPublicKey"
    @_d2i_RSAPublicKey (rsa: CPtr, pp: CPtr, len: int) -> CPtr as "d2i_RSAPublicKey"
    @_i2d_RSAPrivateKey (rsa: CPtr, pp: CPtr) -> int as "i2d_RSAPrivateKey"
    @_d2i_RSAPrivateKey (rsa: CPtr, pp: CPtr, len: int) -> CPtr as "d2i_RSAPrivateKey"

    // PEM encoding
    @_PEM_write_bio_RSAPublicKey (bp: CPtr, x: CPtr) -> int as "PEM_write_bio_RSAPublicKey"
    @_PEM_read_bio_RSAPublicKey (bp: CPtr, x: CPtr, cb: CPtr, u: CPtr) -> CPtr as "PEM_read_bio_RSAPublicKey"
    @_PEM_write_bio_RSAPrivateKey (bp: CPtr, x: CPtr, enc: CPtr, kstr: CPtr, klen: int, cb: CPtr, u: CPtr) -> int as "PEM_write_bio_RSAPrivateKey"
    @_PEM_read_bio_RSAPrivateKey (bp: CPtr, x: CPtr, cb: CPtr, u: CPtr) -> CPtr as "PEM_read_bio_RSAPrivateKey"

    // BIO for memory operations
    @_BIO_new (type_: CPtr) -> CPtr as "BIO_new"
    @_BIO_free (a: CPtr) -> int as "BIO_free"
    @_BIO_s_mem () -> CPtr as "BIO_s_mem"
    @_BIO_read (b: CPtr, data: [byte], dlen: int) -> int as "BIO_read"
    @_BIO_write (b: CPtr, data: [byte], dlen: int) -> int as "BIO_write"
}

// Constants
let $RSA_PKCS1_PADDING: int = 1
let $RSA_PKCS1_OAEP_PADDING: int = 4
```

---

## Implementation Mapping

### Library Initialization

```ori
// std/crypto/init.ori
use "./ffi" { _sodium_init }

// Module-level initialization
let $initialized: bool = {
    let result = _sodium_init()
    if result < 0 then panic(msg: "libsodium initialization failed")
    true
}
```

### Password Hashing

```ori
// std/crypto/password.ori
use "./ffi" { ... }

pub @hash_password (password: str) -> str uses Crypto =
    {
        let out = [0 as byte; $crypto_pwhash_STRBYTES]
        let result = _crypto_pwhash_str(
            out: out
            passwd: password
            passwdlen: len(collection: password)
            opslimit: $crypto_pwhash_OPSLIMIT_INTERACTIVE
            memlimit: $crypto_pwhash_MEMLIMIT_INTERACTIVE
        )
        if result != 0 then panic(msg: "password hashing failed")
        // Find null terminator and convert to string
        str.from_bytes(bytes: out.take_while(b -> b != 0))
    }

pub @verify_password (password: str, hash: str) -> bool uses Crypto =
    {
        let hash_bytes = hash.as_bytes()
        // Pad to STRBYTES if needed
        let padded = [0 as byte; $crypto_pwhash_STRBYTES]
        copy_bytes(src: hash_bytes, dst: padded)
        let result = _crypto_pwhash_str_verify(
            str: padded
            passwd: password
            passwdlen: len(collection: password)
        )
        result == 0
    }
```

### Cryptographic Hashing

```ori
// std/crypto/hash.ori
use "./ffi" { ... }

// Updated: Removed SHA-384 and BLAKE3
type HashAlgorithm = Sha256 | Sha512 | Blake2b

pub @hash (data: [byte], algorithm: HashAlgorithm = Sha256) -> [byte] uses Crypto =
    match algorithm {
        Sha256 -> {
            let out = [0 as byte; $crypto_hash_sha256_BYTES]
            _crypto_hash_sha256(out: out, in_: data, inlen: len(collection: data))
            out
        }
        Sha512 -> {
            let out = [0 as byte; $crypto_hash_sha512_BYTES]
            _crypto_hash_sha512(out: out, in_: data, inlen: len(collection: data))
            out
        }
        Blake2b -> {
            let out = [0 as byte; $crypto_generichash_BYTES]
            _crypto_generichash(
                out: out
                outlen: $crypto_generichash_BYTES
                in_: data
                inlen: len(collection: data)
                key: []
                keylen: 0
            )
            out
        }
    }

pub @hash_hex (data: str, algorithm: HashAlgorithm = Sha256) -> str uses Crypto =
    {
        let bytes = hash(data: data.as_bytes(), algorithm: algorithm)
        bytes_to_hex(bytes: bytes)
    }
```

### HMAC

```ori
// std/crypto/hmac.ori
use "./ffi" { ... }

pub @hmac (key: [byte], data: [byte], algorithm: HashAlgorithm = Sha256) -> [byte] uses Crypto =
    match algorithm {
        Sha256 -> {
            let out = [0 as byte; 32]
            let key_padded = pad_or_hash_key(key: key, size: 32)
            _crypto_auth_hmacsha256(out: out, in_: data, inlen: len(collection: data), k: key_padded)
            out
        }
        Sha512 -> {
            let out = [0 as byte; 64]
            let key_padded = pad_or_hash_key(key: key, size: 64)
            _crypto_auth_hmacsha512(out: out, in_: data, inlen: len(collection: data), k: key_padded)
            out
        }
        Blake2b -> {
            // BLAKE2b with key is built-in
            let out = [0 as byte; $crypto_generichash_BYTES]
            _crypto_generichash(
                out: out
                outlen: $crypto_generichash_BYTES
                in_: data
                inlen: len(collection: data)
                key: key
                keylen: len(collection: key)
            )
            out
        }
    }

pub @verify_hmac (key: [byte], data: [byte], mac: [byte], algorithm: HashAlgorithm = Sha256) -> bool uses Crypto =
    constant_time_eq(a: hmac(key: key, data: data, algorithm: algorithm), b: mac)
```

### Symmetric Encryption

```ori
// std/crypto/symmetric.ori
use "./ffi" { ... }

#zeroize
type SecretKey = { bytes: [byte] }

pub @generate_key () -> SecretKey uses Crypto =
    {
        let key = [0 as byte; $crypto_secretbox_KEYBYTES]
        _crypto_secretbox_keygen(k: key)
        SecretKey { bytes: key }
    }

pub @encrypt (key: SecretKey, plaintext: [byte]) -> [byte] uses Crypto =
    {
        // Generate random nonce
        let nonce = [0 as byte; $crypto_secretbox_NONCEBYTES]
        _randombytes_buf(buf: nonce, size: $crypto_secretbox_NONCEBYTES)

        // Encrypt
        let ciphertext_len = len(collection: plaintext) + $crypto_secretbox_MACBYTES
        let ciphertext = [0 as byte; ciphertext_len]
        _crypto_secretbox_easy(
            c: ciphertext
            m: plaintext
            mlen: len(collection: plaintext)
            n: nonce
            k: key.bytes
        )

        // Prepend nonce to ciphertext
        [...nonce, ...ciphertext]
    }

pub @decrypt (key: SecretKey, ciphertext: [byte]) -> Result<[byte], CryptoError> uses Crypto =
    {
        if len(collection: ciphertext) < $crypto_secretbox_NONCEBYTES + $crypto_secretbox_MACBYTES then
            Err(CryptoError { kind: DecryptionFailed, message: "ciphertext too short" })
        else
            {
                // Extract nonce
                let nonce = ciphertext[0..$crypto_secretbox_NONCEBYTES]
                let actual_ciphertext = ciphertext[$crypto_secretbox_NONCEBYTES..]

                // Decrypt
                let plaintext_len = len(collection: actual_ciphertext) - $crypto_secretbox_MACBYTES
                let plaintext = [0 as byte; plaintext_len]
                let result = _crypto_secretbox_open_easy(
                    m: plaintext
                    c: actual_ciphertext
                    clen: len(collection: actual_ciphertext)
                    n: nonce
                    k: key.bytes
                )

                if result != 0 then
                    Err(CryptoError { kind: DecryptionFailed, message: "decryption failed" })
                else
                    Ok(plaintext)
            }
    }

// Explicit nonce API (XChaCha20-Poly1305)
pub @encrypt_with_nonce (
    key: SecretKey,
    nonce: [byte],
    plaintext: [byte],
    aad: [byte] = []
) -> [byte] uses Crypto =
    {
        if len(collection: nonce) != $crypto_aead_xchacha20poly1305_ietf_NPUBBYTES then
            panic(msg: "nonce must be 24 bytes")
        let ciphertext_len = len(collection: plaintext) + $crypto_aead_xchacha20poly1305_ietf_ABYTES
        let ciphertext = [0 as byte; ciphertext_len]
        _crypto_aead_xchacha20poly1305_ietf_encrypt(
            c: ciphertext
            clen_p: CPtr.null()
            m: plaintext
            mlen: len(collection: plaintext)
            ad: aad
            adlen: len(collection: aad)
            nsec: CPtr.null()
            npub: nonce
            k: key.bytes
        )
        ciphertext
    }

pub @decrypt_with_nonce (
    key: SecretKey,
    nonce: [byte],
    ciphertext: [byte],
    aad: [byte] = []
) -> Result<[byte], CryptoError> uses Crypto =
    {
        if len(collection: nonce) != $crypto_aead_xchacha20poly1305_ietf_NPUBBYTES then
            Err(CryptoError { kind: InvalidKey, message: "nonce must be 24 bytes" })
        else if len(collection: ciphertext) < $crypto_aead_xchacha20poly1305_ietf_ABYTES then
            Err(CryptoError { kind: DecryptionFailed, message: "ciphertext too short" })
        else
            {
                let plaintext_len = len(collection: ciphertext) - $crypto_aead_xchacha20poly1305_ietf_ABYTES
                let plaintext = [0 as byte; plaintext_len]
                let result = _crypto_aead_xchacha20poly1305_ietf_decrypt(
                    m: plaintext
                    mlen_p: CPtr.null()
                    nsec: CPtr.null()
                    c: ciphertext
                    clen: len(collection: ciphertext)
                    ad: aad
                    adlen: len(collection: aad)
                    npub: nonce
                    k: key.bytes
                )
                if result != 0 then
                    Err(CryptoError { kind: DecryptionFailed, message: "decryption failed or authentication failed" })
                else
                    Ok(plaintext)
            }
    }
```

### Digital Signatures

```ori
// std/crypto/signing.ori
use "./ffi" { ... }
use "./ffi_openssl" { ... }

// Ed25519 keys
#zeroize
type SigningPrivateKey = { bytes: [byte], algorithm: SigningAlgorithm }
type SigningPublicKey = { bytes: [byte], algorithm: SigningAlgorithm }
type SigningKeyPair = { public: SigningPublicKey, private: SigningPrivateKey }

type SigningAlgorithm = Ed25519 | Rsa2048 | Rsa4096

pub @generate_signing_keypair (algorithm: SigningAlgorithm = Ed25519) -> SigningKeyPair uses Crypto =
    match algorithm {
        Ed25519 -> {
            let pk = [0 as byte; $crypto_sign_PUBLICKEYBYTES]
            let sk = [0 as byte; $crypto_sign_SECRETKEYBYTES]
            _crypto_sign_keypair(pk: pk, sk: sk)
            SigningKeyPair {
                public: SigningPublicKey { bytes: pk, algorithm: Ed25519 }
                private: SigningPrivateKey { bytes: sk, algorithm: Ed25519 }
            }
        }
        Rsa2048 -> generate_rsa_signing_keypair(bits: 2048)
        Rsa4096 -> generate_rsa_signing_keypair(bits: 4096)
    }

@generate_rsa_signing_keypair (bits: int) -> SigningKeyPair uses Crypto, FFI =
    {
        let rsa = _RSA_new()
        let e = _BN_new()
        _BN_set_word(bn: e, w: 65537),  // Standard RSA exponent
        let result = _RSA_generate_key_ex(rsa: rsa, bits: bits, e: e, cb: CPtr.null())
        _BN_free(bn: e)
        if result != 1 then panic(msg: "RSA key generation failed")

        // Export keys to DER format
        let pk_bytes = export_rsa_public_key(rsa: rsa)
        let sk_bytes = export_rsa_private_key(rsa: rsa)
        _RSA_free(rsa: rsa)

        let algorithm = if bits == 2048 then Rsa2048 else Rsa4096
        SigningKeyPair {
            public: SigningPublicKey { bytes: pk_bytes, algorithm: algorithm }
            private: SigningPrivateKey { bytes: sk_bytes, algorithm: algorithm }
        }
    }

pub @sign (key: SigningPrivateKey, data: [byte]) -> [byte] uses Crypto =
    match key.algorithm {
        Ed25519 -> {
            let sig = [0 as byte; $crypto_sign_BYTES]
            _crypto_sign_detached(
                sig: sig
                siglen_p: CPtr.null()
                m: data
                mlen: len(collection: data)
                sk: key.bytes
            )
            sig
        }
        Rsa2048 -> sign_rsa(key: key, data: data)
        Rsa4096 -> sign_rsa(key: key, data: data)
    }

@sign_rsa (key: SigningPrivateKey, data: [byte]) -> [byte] uses Crypto, FFI =
    {
        let rsa = import_rsa_private_key(bytes: key.bytes)
        let pkey = _EVP_PKEY_new()
        _EVP_PKEY_assign_RSA(pkey: pkey, rsa: rsa)

        let ctx = _EVP_MD_CTX_new()
        _EVP_DigestSignInit(ctx: ctx, pctx: CPtr.null(), type_: _EVP_sha256(), e: CPtr.null(), pkey: pkey)
        _EVP_DigestSignUpdate(ctx: ctx, d: data, cnt: len(collection: data))

        // Get signature length first
        let sig_len = [0 as byte; 8],  // size_t
        _EVP_DigestSignFinal(ctx: ctx, sig: [], siglen: sig_len.as_ptr())
        let actual_len = bytes_to_int(bytes: sig_len)

        // Get actual signature
        let sig = [0 as byte; actual_len]
        _EVP_DigestSignFinal(ctx: ctx, sig: sig, siglen: sig_len.as_ptr())

        _EVP_MD_CTX_free(ctx: ctx)
        _EVP_PKEY_free(pkey: pkey)
        sig
    }

pub @verify_signature (key: SigningPublicKey, data: [byte], signature: [byte]) -> bool uses Crypto =
    match key.algorithm {
        Ed25519 -> {
            let result = _crypto_sign_verify_detached(
                sig: signature
                m: data
                mlen: len(collection: data)
                pk: key.bytes
            )
            result == 0
        }
        Rsa2048 -> verify_rsa(key: key, data: data, signature: signature)
        Rsa4096 -> verify_rsa(key: key, data: data, signature: signature)
    }

@verify_rsa (key: SigningPublicKey, data: [byte], signature: [byte]) -> bool uses Crypto, FFI =
    {
        let rsa = import_rsa_public_key(bytes: key.bytes)
        let pkey = _EVP_PKEY_new()
        _EVP_PKEY_assign_RSA(pkey: pkey, rsa: rsa)

        let ctx = _EVP_MD_CTX_new()
        _EVP_DigestVerifyInit(ctx: ctx, pctx: CPtr.null(), type_: _EVP_sha256(), e: CPtr.null(), pkey: pkey)
        _EVP_DigestVerifyUpdate(ctx: ctx, d: data, cnt: len(collection: data))
        let result = _EVP_DigestVerifyFinal(ctx: ctx, sig: signature, siglen: len(collection: signature))

        _EVP_MD_CTX_free(ctx: ctx)
        _EVP_PKEY_free(pkey: pkey)
        result == 1
    }
```

### Public Key Encryption

```ori
// std/crypto/encryption.ori
use "./ffi" { ... }
use "./ffi_openssl" { ... }

type EncryptionAlgorithm = Rsa2048 | Rsa4096

#zeroize
type EncryptionPrivateKey = { bytes: [byte], algorithm: EncryptionAlgorithm }
type EncryptionPublicKey = { bytes: [byte], algorithm: EncryptionAlgorithm }
type EncryptionKeyPair = { public: EncryptionPublicKey, private: EncryptionPrivateKey }

pub @generate_encryption_keypair (algorithm: EncryptionAlgorithm = Rsa2048) -> EncryptionKeyPair uses Crypto =
    {
        let bits = match algorithm { Rsa2048 -> 2048, Rsa4096 -> 4096}
        let rsa = _RSA_new()
        let e = _BN_new()
        _BN_set_word(bn: e, w: 65537)
        let result = _RSA_generate_key_ex(rsa: rsa, bits: bits, e: e, cb: CPtr.null())
        _BN_free(bn: e)
        if result != 1 then panic(msg: "RSA key generation failed")

        let pk_bytes = export_rsa_public_key(rsa: rsa)
        let sk_bytes = export_rsa_private_key(rsa: rsa)
        _RSA_free(rsa: rsa)

        EncryptionKeyPair {
            public: EncryptionPublicKey { bytes: pk_bytes, algorithm: algorithm }
            private: EncryptionPrivateKey { bytes: sk_bytes, algorithm: algorithm }
        }
    }

pub @encrypt_for (recipient: EncryptionPublicKey, plaintext: [byte]) -> [byte] uses Crypto =
    {
        let rsa = import_rsa_public_key(bytes: recipient.bytes)
        let rsa_size = _RSA_size(rsa: rsa)
        let ciphertext = [0 as byte; rsa_size]

        let result = _RSA_public_encrypt(
            flen: len(collection: plaintext)
            from: plaintext
            to: ciphertext
            rsa: rsa
            padding: $RSA_PKCS1_OAEP_PADDING
        )
        _RSA_free(rsa: rsa)

        if result < 0 then panic(msg: "encryption failed")
        ciphertext[0..result]
    }

pub @decrypt_with (key: EncryptionPrivateKey, ciphertext: [byte]) -> Result<[byte], CryptoError> uses Crypto =
    {
        let rsa = import_rsa_private_key(bytes: key.bytes)
        let rsa_size = _RSA_size(rsa: rsa)
        let plaintext = [0 as byte; rsa_size]

        let result = _RSA_private_decrypt(
            flen: len(collection: ciphertext)
            from: ciphertext
            to: plaintext
            rsa: rsa
            padding: $RSA_PKCS1_OAEP_PADDING
        )
        _RSA_free(rsa: rsa)

        if result < 0 then
            Err(CryptoError { kind: DecryptionFailed, message: "decryption failed" })
        else
            Ok(plaintext[0..result])
    }
```

### Key Exchange

```ori
// std/crypto/key_exchange.ori
use "./ffi" { ... }

type KeyExchangeAlgorithm = X25519

#zeroize
type KeyExchangePrivateKey = { bytes: [byte], algorithm: KeyExchangeAlgorithm }
type KeyExchangePublicKey = { bytes: [byte], algorithm: KeyExchangeAlgorithm }
type KeyExchangeKeyPair = { public: KeyExchangePublicKey, private: KeyExchangePrivateKey }

pub @generate_key_exchange_keypair (algorithm: KeyExchangeAlgorithm = X25519) -> KeyExchangeKeyPair uses Crypto =
    match algorithm {
        X25519 -> {
            let pk = [0 as byte; $crypto_kx_PUBLICKEYBYTES]
            let sk = [0 as byte; $crypto_kx_SECRETKEYBYTES]
            _crypto_kx_keypair(pk: pk, sk: sk)
            KeyExchangeKeyPair {
                public: KeyExchangePublicKey { bytes: pk, algorithm: X25519 }
                private: KeyExchangePrivateKey { bytes: sk, algorithm: X25519 }
            }
        }
    }

pub @derive_shared_secret (
    my_private: KeyExchangePrivateKey,
    their_public: KeyExchangePublicKey
) -> [byte] uses Crypto =
    {
        // Use raw X25519 for simple shared secret
        let shared = [0 as byte; 32]
        let result = _crypto_scalarmult(q: shared, n: my_private.bytes, p: their_public.bytes)
        if result != 0 then panic(msg: "key exchange failed")
        shared
    }
```

### Random Generation

```ori
// std/crypto/random.ori
use "./ffi" { _randombytes_buf, _randombytes_uniform }

pub @random_bytes (count: int) -> [byte] uses Crypto =
    {
        let buf = [0 as byte; count]
        _randombytes_buf(buf: buf, size: count)
        buf
    }

pub @random_int (min: int, max: int) -> int uses Crypto =
    {
        if min >= max then panic(msg: "min must be less than max")
        let range = max - min
        let r = _randombytes_uniform(upper_bound: range)
        min + r
    }

pub @random_uuid () -> str uses Crypto =
    {
        let bytes = random_bytes(count: 16)
        // Set version (4) and variant (RFC 4122)
        let bytes = [
            ...bytes[0..6]
            (bytes[6] & 0x0F) | 0x40
            bytes[7]
            (bytes[8] & 0x3F) | 0x80
            ...bytes[9..]
        ]
        format_uuid(bytes: bytes)
    }
```

### Key Derivation

```ori
// std/crypto/kdf.ori
use "./ffi" { ... }

pub @derive_key (
    password: str,
    salt: [byte],
    key_length: int = 32
) -> [byte] uses Crypto =
    {
        if len(collection: salt) < $crypto_pwhash_SALTBYTES then
            panic(msg: `salt must be at least {$crypto_pwhash_SALTBYTES} bytes`)
        let out = [0 as byte; key_length]
        let result = _crypto_pwhash(
            out: out
            outlen: key_length
            passwd: password
            passwdlen: len(collection: password)
            salt: salt
            opslimit: $crypto_pwhash_OPSLIMIT_INTERACTIVE
            memlimit: $crypto_pwhash_MEMLIMIT_INTERACTIVE
            alg: $crypto_pwhash_ALG_ARGON2ID13
        )
        if result != 0 then panic(msg: "key derivation failed")
        out
    }

pub @stretch_key (
    input_key: [byte],
    info: [byte] = [],
    length: int = 32
) -> [byte] uses Crypto =
    {
        if len(collection: input_key) != $crypto_kdf_KEYBYTES then
            panic(msg: `input key must be {$crypto_kdf_KEYBYTES} bytes`)
        // Context must be exactly 8 bytes
        let ctx = if len(collection: info) >= $crypto_kdf_CONTEXTBYTES then
            info[0..$crypto_kdf_CONTEXTBYTES]
        else
            [...info, ...[0 as byte; $crypto_kdf_CONTEXTBYTES - len(collection: info)]]
        let out = [0 as byte; length]
        let result = _crypto_kdf_derive_from_key(
            subkey: out
            subkey_len: length
            subkey_id: 1,  // Fixed subkey ID
            ctx: ctx
            key: input_key
        )
        if result != 0 then panic(msg: "key stretching failed")
        out
    }
```

### Constant-Time Comparison

```ori
// std/crypto/util.ori
use "./ffi" { _sodium_memcmp }

pub @constant_time_eq (a: [byte], b: [byte]) -> bool uses Crypto =
    if len(collection: a) != len(collection: b) then
        false
    else
        _sodium_memcmp(b1: a, b2: b, len: len(collection: a)) == 0
```

---

## Pure Ori Components

These don't need FFI:

| Component | Implementation |
|-----------|----------------|
| Type definitions | Pure Ori types |
| Error types | Pure Ori enums |
| Key serialization helpers | Pure Ori byte manipulation |
| UUID formatting | Pure Ori string formatting |
| Hex encoding | Pure Ori byte-to-hex |

```ori
// Hex encoding (pure Ori)
@bytes_to_hex (bytes: [byte]) -> str =
    {
        let hex_chars = "0123456789abcdef"
        bytes
            .flat_map(b -> [hex_chars[(b >> 4) as int], hex_chars[(b & 0x0F) as int]])
            .collect()
    }

// UUID formatting (pure Ori)
@format_uuid (bytes: [byte]) -> str =
    {
        let hex = bytes_to_hex(bytes: bytes)
        `{hex[0..8]}-{hex[8..12]}-{hex[12..16]}-{hex[16..20]}-{hex[20..32]}`
    }
```

---

## Build Configuration

```toml
# ori.toml
[native]
libraries = ["sodium"]

# For RSA support (required for full crypto API)
[native.with_rsa]
libraries = ["sodium", "crypto"]  # crypto = OpenSSL/LibreSSL
```

### Library Installation

**Ubuntu/Debian:**
```bash
sudo apt install libsodium-dev libssl-dev
```

**macOS:**
```bash
brew install libsodium openssl
```

**Windows:**
```powershell
vcpkg install libsodium openssl
```

---

## Algorithm Availability Summary

| Algorithm | libsodium | OpenSSL | Used For |
|-----------|-----------|---------|----------|
| Argon2id | ✓ | | Password hashing |
| SHA-256 | ✓ | ✓ | Data hashing |
| SHA-512 | ✓ | ✓ | Data hashing |
| BLAKE2b | ✓ | | Data hashing, HMAC |
| Ed25519 | ✓ | ✓ | Signatures |
| X25519 | ✓ | ✓ | Key exchange |
| RSA-2048/4096 | | ✓ | Signatures, encryption |
| XSalsa20-Poly1305 | ✓ | | Symmetric encryption |
| XChaCha20-Poly1305 | ✓ | | Symmetric encryption (explicit nonce) |

---

## Design Decisions

### Why libsodium primary, OpenSSL secondary?

libsodium provides misuse-resistant APIs for modern algorithms. RSA requires OpenSSL because libsodium intentionally doesn't include legacy algorithms. This split gives us the best of both worlds: modern crypto with safe defaults, plus RSA for compatibility.

### Why `#zeroize` attribute?

Ori doesn't have a Drop trait. Adding one would be a significant language change. The `#zeroize` attribute achieves the same goal (secure memory cleanup) with minimal language impact. The compiler inserts `sodium_memzero` calls automatically.

### Why XSalsa20-Poly1305 for symmetric encryption?

libsodium's default authenticated encryption. Equally secure as AES-GCM, faster in software implementations, and has a larger nonce (24 bytes vs 12 bytes) making random nonce collisions astronomically unlikely.

### Why remove SHA-384 and BLAKE3?

Both require additional libraries not included in libsodium. Rather than runtime panics, we only expose algorithms that work. SHA-256, SHA-512, and BLAKE2b cover virtually all use cases.

### Why native-only for now?

WASM requires different FFI bindings (`crypto.subtle`). That's a separate proposal to keep this one focused and implementable.
