# Proposal: std.crypto API Design

**Status:** Approved
**Approved:** 2026-01-30
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Standard library

---

## Summary

This proposal defines the API for `std.crypto`, providing cryptographic primitives for hashing, encryption, signatures, key exchange, and secure random number generation.

---

## Motivation

Cryptography is essential for:
- Password hashing and verification
- Data encryption and decryption
- Digital signatures
- Secure token generation
- Message authentication
- Key exchange

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

The `Crypto` capability is non-suspending (CPU-bound operations complete synchronously).

### High-Level First

Provide high-level APIs for common operations, with low-level primitives available for advanced users.

### Secure Defaults

Default parameters are secure. Insecure options require explicit configuration.

### Type-Safe Key Usage

Asymmetric keys are separated by purpose (signing, encryption, key exchange) to prevent misuse at compile time.

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
@hash_hex (data: str, algorithm: HashAlgorithm = Sha256) -> str uses Crypto
```

Usage:
```ori
use std.crypto { hash, hash_hex, HashAlgorithm }

let digest = hash(data: file_contents, algorithm: Sha256)
let hex = hash_hex(data: "hello", algorithm: Blake3)
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
type SecretKey = { bytes: [byte] }  // Opaque key type, auto-zeroizes on drop

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

## Asymmetric Key Types

Keys are separated by purpose to prevent misuse:

### Signing Keys

```ori
type SigningAlgorithm = Ed25519 | Rsa2048 | Rsa4096

type SigningKeyPair = { public: SigningPublicKey, private: SigningPrivateKey }
type SigningPublicKey = { bytes: [byte], algorithm: SigningAlgorithm }
type SigningPrivateKey = { bytes: [byte], algorithm: SigningAlgorithm }  // auto-zeroizes

@generate_signing_keypair (algorithm: SigningAlgorithm = Ed25519) -> SigningKeyPair uses Crypto
```

### Encryption Keys

```ori
type EncryptionAlgorithm = Rsa2048 | Rsa4096

type EncryptionKeyPair = { public: EncryptionPublicKey, private: EncryptionPrivateKey }
type EncryptionPublicKey = { bytes: [byte], algorithm: EncryptionAlgorithm }
type EncryptionPrivateKey = { bytes: [byte], algorithm: EncryptionAlgorithm }  // auto-zeroizes

@generate_encryption_keypair (algorithm: EncryptionAlgorithm = Rsa2048) -> EncryptionKeyPair uses Crypto
```

### Key Exchange Keys

```ori
type KeyExchangeAlgorithm = X25519

type KeyExchangeKeyPair = { public: KeyExchangePublicKey, private: KeyExchangePrivateKey }
type KeyExchangePublicKey = { bytes: [byte], algorithm: KeyExchangeAlgorithm }
type KeyExchangePrivateKey = { bytes: [byte], algorithm: KeyExchangeAlgorithm }  // auto-zeroizes

@generate_key_exchange_keypair (algorithm: KeyExchangeAlgorithm = X25519) -> KeyExchangeKeyPair uses Crypto
```

---

## Public Key Encryption

For encrypting data to a recipient's public key:

```ori
@encrypt_for (recipient: EncryptionPublicKey, plaintext: [byte]) -> [byte] uses Crypto
@decrypt_with (key: EncryptionPrivateKey, ciphertext: [byte]) -> Result<[byte], CryptoError> uses Crypto
```

Usage:
```ori
use std.crypto { generate_encryption_keypair, encrypt_for, decrypt_with }

// Recipient generates keypair
let keypair = generate_encryption_keypair()

// Sender encrypts for recipient
let ciphertext = encrypt_for(recipient: keypair.public, plaintext: message)

// Recipient decrypts
let message = decrypt_with(key: keypair.private, ciphertext: ciphertext)?
```

---

## Digital Signatures

### Signing and Verification

```ori
@sign (key: SigningPrivateKey, data: [byte]) -> [byte] uses Crypto
@verify_signature (key: SigningPublicKey, data: [byte], signature: [byte]) -> bool uses Crypto
```

Usage:
```ori
use std.crypto { generate_signing_keypair, sign, verify_signature, SigningAlgorithm }

let keypair = generate_signing_keypair(algorithm: Ed25519)

// Sign data
let signature = sign(key: keypair.private, data: document)

// Verify signature
if verify_signature(key: keypair.public, data: document, signature: signature) then
    trust_document()
```

---

## Key Exchange

Diffie-Hellman key exchange for establishing shared secrets:

```ori
@derive_shared_secret (
    my_private: KeyExchangePrivateKey,
    their_public: KeyExchangePublicKey,
) -> [byte] uses Crypto
```

Usage:
```ori
use std.crypto { generate_key_exchange_keypair, derive_shared_secret }

// Alice and Bob each generate keypairs
let alice = generate_key_exchange_keypair()
let bob = generate_key_exchange_keypair()

// Exchange public keys (via network, etc.), then derive shared secret
let alice_secret = derive_shared_secret(my_private: alice.private, their_public: bob.public)
let bob_secret = derive_shared_secret(my_private: bob.private, their_public: alice.public)
// alice_secret == bob_secret (can be used as symmetric key)
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

impl SigningPublicKey {
    @to_pem (self) -> str
    @from_pem (pem: str) -> Result<SigningPublicKey, CryptoError>
    @to_bytes (self) -> [byte]
    @from_bytes (bytes: [byte], algorithm: SigningAlgorithm) -> Result<SigningPublicKey, CryptoError>
}

impl SigningPrivateKey {
    @to_pem (self) -> str
    @to_encrypted_pem (self, password: str) -> str uses Crypto
    @from_pem (pem: str) -> Result<SigningPrivateKey, CryptoError>
    @from_encrypted_pem (pem: str, password: str) -> Result<SigningPrivateKey, CryptoError> uses Crypto
}

impl EncryptionPublicKey {
    @to_pem (self) -> str
    @from_pem (pem: str) -> Result<EncryptionPublicKey, CryptoError>
    @to_bytes (self) -> [byte]
    @from_bytes (bytes: [byte], algorithm: EncryptionAlgorithm) -> Result<EncryptionPublicKey, CryptoError>
}

impl EncryptionPrivateKey {
    @to_pem (self) -> str
    @to_encrypted_pem (self, password: str) -> str uses Crypto
    @from_pem (pem: str) -> Result<EncryptionPrivateKey, CryptoError>
    @from_encrypted_pem (pem: str, password: str) -> Result<EncryptionPrivateKey, CryptoError> uses Crypto
}

impl KeyExchangePublicKey {
    @to_bytes (self) -> [byte]
    @from_bytes (bytes: [byte], algorithm: KeyExchangeAlgorithm) -> Result<KeyExchangePublicKey, CryptoError>
}

impl KeyExchangePrivateKey {
    @to_bytes (self) -> [byte]
    @from_bytes (bytes: [byte], algorithm: KeyExchangeAlgorithm) -> Result<KeyExchangePrivateKey, CryptoError>
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
    | KeyExchangeFailed  // Key exchange operation failed
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

## Memory Safety

### Automatic Key Zeroization

`SecretKey`, `SigningPrivateKey`, `EncryptionPrivateKey`, and `KeyExchangePrivateKey` automatically zero their memory when dropped. This prevents sensitive key material from lingering in memory after use.

This is automatic — no user action required. The zeroization happens even if the value is dropped due to an error or early return.

**Note**: Zeroization cannot prevent the operating system from swapping memory to disk. For high-security applications, consider memory locking (OS-specific).

---

## Algorithm Deprecation

Algorithms may be marked as deprecated when they become insecure. Using a deprecated algorithm emits a compiler warning:

```ori
let hash = hash(data: content, algorithm: Md5)  // Warning: Md5 is deprecated
```

Suppress warnings when necessary (e.g., legacy compatibility):

```ori
#allow(deprecated_algorithm)
let hash = hash(data: content, algorithm: Md5)
```

### Deprecation Schedule

| Algorithm | Status | Reason |
|-----------|--------|--------|
| SHA-256 | Current | — |
| SHA-384 | Current | — |
| SHA-512 | Current | — |
| Blake2b | Current | — |
| Blake3 | Current | — |
| Ed25519 | Current | — |
| X25519 | Current | — |
| RSA-2048 | Current | — |
| RSA-4096 | Current | — |

This table will be updated as cryptographic recommendations evolve.

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
use std.crypto { generate_signing_keypair, sign, verify_signature }

type SignedMessage = { data: [byte], signature: [byte], signer: SigningPublicKey }

@sign_message (key: SigningPrivateKey, public: SigningPublicKey, data: [byte]) -> SignedMessage uses Crypto =
    SignedMessage {
        data: data,
        signature: sign(key: key, data: data),
        signer: public,
    }

@verify_message (msg: SignedMessage) -> bool uses Crypto =
    verify_signature(key: msg.signer, data: msg.data, signature: msg.signature)
```

### Secure Channel Establishment

```ori
use std.crypto { generate_key_exchange_keypair, derive_shared_secret, encrypt, decrypt, SecretKey }

// Establish encrypted channel between two parties
@establish_channel (my_keypair: KeyExchangeKeyPair, their_public: KeyExchangePublicKey) -> SecretKey uses Crypto =
    run(
        let shared = derive_shared_secret(my_private: my_keypair.private, their_public: their_public),
        SecretKey.from_bytes(bytes: shared).unwrap_or(panic(msg: "Invalid shared secret")),
    )
```

---

## Module Structure

```ori
// std/crypto/mod.ori
pub use "./hash" { hash, hash_hex, hash_password, verify_password, HashAlgorithm }
pub use "./hmac" { hmac, verify_hmac }
pub use "./symmetric" { SecretKey, generate_key, encrypt, decrypt, encrypt_with_nonce, decrypt_with_nonce }
pub use "./signing" {
    SigningAlgorithm, SigningKeyPair, SigningPublicKey, SigningPrivateKey,
    generate_signing_keypair, sign, verify_signature,
}
pub use "./encryption" {
    EncryptionAlgorithm, EncryptionKeyPair, EncryptionPublicKey, EncryptionPrivateKey,
    generate_encryption_keypair, encrypt_for, decrypt_with,
}
pub use "./key_exchange" {
    KeyExchangeAlgorithm, KeyExchangeKeyPair, KeyExchangePublicKey, KeyExchangePrivateKey,
    generate_key_exchange_keypair, derive_shared_secret,
}
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
6. **Key type safety**: Use the correct key type for each operation (signing keys for signatures, encryption keys for encryption, etc.)

---

## Summary

| Category | Functions |
|----------|-----------|
| Password hashing | `hash_password`, `verify_password` |
| Data hashing | `hash`, `hash_hex`, `hmac`, `verify_hmac` |
| Symmetric encryption | `generate_key`, `encrypt`, `decrypt` |
| Asymmetric encryption | `generate_encryption_keypair`, `encrypt_for`, `decrypt_with` |
| Signatures | `generate_signing_keypair`, `sign`, `verify_signature` |
| Key exchange | `generate_key_exchange_keypair`, `derive_shared_secret` |
| Random | `random_bytes`, `random_int`, `random_uuid` |
| Key derivation | `derive_key`, `stretch_key` |
| Utilities | `constant_time_eq` |
