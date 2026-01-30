# Proposal: std.crypto API Design

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Standard library

---

## Summary

This proposal defines the API for `std.crypto`, providing cryptographic primitives for hashing, encryption, signatures, and secure random number generation.

---

## Motivation

Cryptography is essential for:
- Password hashing and verification
- Data encryption and decryption
- Digital signatures
- Secure token generation
- Message authentication

A standard crypto library must be:
1. **Secure by default** — Hard to misuse
2. **Well-documented** — Clear guidance on appropriate use
3. **Complete** — Cover common use cases
4. **Updatable** — Algorithms can be deprecated

---

## Design Principles

### Capability-Based

All crypto operations use the `Crypto` capability to track cryptographic side effects:

```ori
@hash_password (password: str) -> str uses Crypto = ...
```

### High-Level First

Provide high-level APIs for common operations, with low-level primitives available for advanced users.

### Secure Defaults

Default parameters are secure. Insecure options require explicit configuration.

---

## Hashing API

### Password Hashing

For password storage (slow, salted):

```ori
@hash_password (password: str) -> str uses Crypto
@verify_password (password: str, hash: str) -> bool uses Crypto
```

Usage:
```ori
use std.crypto { hash_password, verify_password }

// Hash password for storage
let hash = hash_password(password: "user_password")
// "$argon2id$v=19$m=65536,t=3,p=4$..."

// Verify on login
if verify_password(password: input, hash: stored_hash) then
    grant_access()
```

Implementation uses Argon2id with secure defaults.

### Cryptographic Hashing

For data integrity (fast, deterministic):

```ori
type HashAlgorithm = Sha256 | Sha384 | Sha512 | Blake2b | Blake3

@hash (data: [byte], algorithm: HashAlgorithm = Sha256) -> [byte] uses Crypto
@hash_str (data: str, algorithm: HashAlgorithm = Sha256) -> str uses Crypto
```

Usage:
```ori
use std.crypto { hash, hash_str, HashAlgorithm }

let digest = hash(data: file_contents, algorithm: Sha256)
let hex = hash_str(data: "hello", algorithm: Blake3)
// "ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f"
```

### HMAC

For message authentication:

```ori
@hmac (key: [byte], data: [byte], algorithm: HashAlgorithm = Sha256) -> [byte] uses Crypto
@verify_hmac (key: [byte], data: [byte], mac: [byte], algorithm: HashAlgorithm = Sha256) -> bool uses Crypto
```

Usage:
```ori
use std.crypto { hmac, verify_hmac }

let mac = hmac(key: secret_key, data: message)
// Later:
if verify_hmac(key: secret_key, data: message, mac: received_mac) then
    accept_message()
```

---

## Symmetric Encryption

### High-Level API

For simple encrypt/decrypt with secure defaults:

```ori
type SecretKey = { bytes: [byte] }  // Opaque key type

@generate_key () -> SecretKey uses Crypto
@encrypt (key: SecretKey, plaintext: [byte]) -> [byte] uses Crypto
@decrypt (key: SecretKey, ciphertext: [byte]) -> Result<[byte], CryptoError> uses Crypto
```

Usage:
```ori
use std.crypto { generate_key, encrypt, decrypt }

let key = generate_key()
let ciphertext = encrypt(key: key, plaintext: secret_data)

// Later:
let plaintext = decrypt(key: key, ciphertext: ciphertext)?
```

Internally uses AES-256-GCM with random nonce (prepended to ciphertext).

### Explicit Nonce API

For when you need to control the nonce:

```ori
@encrypt_with_nonce (
    key: SecretKey,
    nonce: [byte],
    plaintext: [byte],
    aad: [byte] = [],
) -> [byte] uses Crypto

@decrypt_with_nonce (
    key: SecretKey,
    nonce: [byte],
    ciphertext: [byte],
    aad: [byte] = [],
) -> Result<[byte], CryptoError> uses Crypto
```

**Warning**: Nonce reuse is catastrophic for security. Use high-level API unless you understand the implications.

---

## Asymmetric Encryption

### Key Pairs

```ori
type KeyPair = { public: PublicKey, private: PrivateKey }
type PublicKey = { bytes: [byte], algorithm: AsymmetricAlgorithm }
type PrivateKey = { bytes: [byte], algorithm: AsymmetricAlgorithm }
type AsymmetricAlgorithm = Ed25519 | X25519 | Rsa2048 | Rsa4096

@generate_keypair (algorithm: AsymmetricAlgorithm = Ed25519) -> KeyPair uses Crypto
```

### Public Key Encryption

For encrypting data to a recipient's public key:

```ori
@encrypt_for (recipient: PublicKey, plaintext: [byte]) -> [byte] uses Crypto
@decrypt_with (key: PrivateKey, ciphertext: [byte]) -> Result<[byte], CryptoError> uses Crypto
```

Usage:
```ori
use std.crypto { generate_keypair, encrypt_for, decrypt_with }

// Recipient generates keypair
let keypair = generate_keypair()

// Sender encrypts for recipient
let ciphertext = encrypt_for(recipient: keypair.public, plaintext: message)

// Recipient decrypts
let message = decrypt_with(key: keypair.private, ciphertext: ciphertext)?
```

---

## Digital Signatures

### Signing and Verification

```ori
@sign (key: PrivateKey, data: [byte]) -> [byte] uses Crypto
@verify_signature (key: PublicKey, data: [byte], signature: [byte]) -> bool uses Crypto
```

Usage:
```ori
use std.crypto { generate_keypair, sign, verify_signature, AsymmetricAlgorithm }

let keypair = generate_keypair(algorithm: Ed25519)

// Sign data
let signature = sign(key: keypair.private, data: document)

// Verify signature
if verify_signature(key: keypair.public, data: document, signature: signature) then
    trust_document()
```

---

## Secure Random

### Random Bytes

```ori
@random_bytes (count: int) -> [byte] uses Crypto
```

Usage:
```ori
use std.crypto { random_bytes }

let token = random_bytes(count: 32)  // 256-bit random token
let nonce = random_bytes(count: 12)  // 96-bit nonce
```

### Random Values

```ori
@random_int (min: int, max: int) -> int uses Crypto
@random_uuid () -> str uses Crypto
```

Usage:
```ori
use std.crypto { random_int, random_uuid }

let code = random_int(min: 100000, max: 999999)  // 6-digit code
let id = random_uuid()  // "550e8400-e29b-41d4-a716-446655440000"
```

---

## Key Derivation

### From Password

```ori
@derive_key (
    password: str,
    salt: [byte],
    key_length: int = 32,
) -> [byte] uses Crypto
```

Usage:
```ori
use std.crypto { derive_key, random_bytes }

let salt = random_bytes(count: 16)
let key = derive_key(password: user_password, salt: salt, key_length: 32)
// Store salt with encrypted data
```

### Key Stretching

```ori
@stretch_key (
    input_key: [byte],
    info: [byte] = [],
    length: int = 32,
) -> [byte] uses Crypto
```

HKDF for deriving multiple keys from one master key.

---

## Key Serialization

### Export/Import

```ori
impl SecretKey {
    @to_bytes (self) -> [byte]
    @from_bytes (bytes: [byte]) -> Result<SecretKey, CryptoError>
}

impl PublicKey {
    @to_pem (self) -> str
    @from_pem (pem: str) -> Result<PublicKey, CryptoError>
    @to_bytes (self) -> [byte]
    @from_bytes (bytes: [byte], algorithm: AsymmetricAlgorithm) -> Result<PublicKey, CryptoError>
}

impl PrivateKey {
    @to_pem (self) -> str
    @to_encrypted_pem (self, password: str) -> str uses Crypto
    @from_pem (pem: str) -> Result<PrivateKey, CryptoError>
    @from_encrypted_pem (pem: str, password: str) -> Result<PrivateKey, CryptoError> uses Crypto
}
```

---

## Error Types

```ori
type CryptoError = {
    kind: CryptoErrorKind,
    message: str,
}

type CryptoErrorKind =
    | DecryptionFailed   // Wrong key or corrupted ciphertext
    | InvalidKey         // Malformed or wrong-algorithm key
    | InvalidSignature   // Signature verification failed
    | KeyDerivationFailed
    | RandomGenerationFailed
```

---

## Constant-Time Comparison

For preventing timing attacks:

```ori
@constant_time_eq (a: [byte], b: [byte]) -> bool uses Crypto
```

Usage:
```ori
use std.crypto { constant_time_eq }

// Compare MACs securely
if constant_time_eq(a: computed_mac, b: received_mac) then
    accept()
```

---

## Examples

### Secure Password Storage

```ori
use std.crypto { hash_password, verify_password }

@register_user (username: str, password: str) -> Result<User, Error> uses Crypto, Database =
    run(
        let hash = hash_password(password: password),
        Database.insert(user: User { username, password_hash: hash }),
    )

@login (username: str, password: str) -> Result<Session, Error> uses Crypto, Database =
    run(
        let user = Database.find_user(username: username)?,
        if verify_password(password: password, hash: user.password_hash) then
            Ok(create_session(user: user))
        else
            Err(InvalidCredentials),
    )
```

### Encrypted Configuration

```ori
use std.crypto { generate_key, encrypt, decrypt }
use std.json { parse_as, to_json_string }

@save_secrets (config: Config, key: SecretKey) -> Result<void, Error> uses Crypto, FileSystem =
    run(
        let json = to_json_string(value: config),
        let encrypted = encrypt(key: key, plaintext: json.as_bytes()),
        FileSystem.write(path: "secrets.enc", data: encrypted),
    )

@load_secrets (key: SecretKey) -> Result<Config, Error> uses Crypto, FileSystem =
    run(
        let encrypted = FileSystem.read_bytes(path: "secrets.enc")?,
        let json = decrypt(key: key, ciphertext: encrypted)?,
        parse_as<Config>(source: str.from_bytes(json)),
    )
```

### Message Signing

```ori
use std.crypto { generate_keypair, sign, verify_signature }

type SignedMessage = { data: [byte], signature: [byte], signer: PublicKey }

@sign_message (key: PrivateKey, public: PublicKey, data: [byte]) -> SignedMessage uses Crypto =
    SignedMessage {
        data: data,
        signature: sign(key: key, data: data),
        signer: public,
    }

@verify_message (msg: SignedMessage) -> bool uses Crypto =
    verify_signature(key: msg.signer, data: msg.data, signature: msg.signature)
```

---

## Module Structure

```ori
// std/crypto/mod.ori
pub use "./hash" { hash, hash_str, hash_password, verify_password, HashAlgorithm }
pub use "./hmac" { hmac, verify_hmac }
pub use "./symmetric" { SecretKey, generate_key, encrypt, decrypt, encrypt_with_nonce, decrypt_with_nonce }
pub use "./asymmetric" { KeyPair, PublicKey, PrivateKey, AsymmetricAlgorithm, generate_keypair }
pub use "./signature" { sign, verify_signature }
pub use "./random" { random_bytes, random_int, random_uuid }
pub use "./kdf" { derive_key, stretch_key }
pub use "./error" { CryptoError, CryptoErrorKind }
pub use "./util" { constant_time_eq }
```

---

## Security Warnings

### Must Document

1. **Nonce reuse**: Never reuse nonces with the same key
2. **Key storage**: Keys must be stored securely (not in source code)
3. **Algorithm selection**: Use recommended defaults unless you have specific requirements
4. **Timing attacks**: Use `constant_time_eq` for secret comparisons
5. **Key rotation**: Plan for key rotation from the start

---

## Summary

| Category | Functions |
|----------|-----------|
| Password hashing | `hash_password`, `verify_password` |
| Data hashing | `hash`, `hash_str`, `hmac`, `verify_hmac` |
| Symmetric encryption | `generate_key`, `encrypt`, `decrypt` |
| Asymmetric encryption | `generate_keypair`, `encrypt_for`, `decrypt_with` |
| Signatures | `sign`, `verify_signature` |
| Random | `random_bytes`, `random_int`, `random_uuid` |
| Key derivation | `derive_key`, `stretch_key` |
| Utilities | `constant_time_eq` |
