# Proposal: std.http FFI Backend Selection

**Status:** Draft
**Created:** 2026-01-31
**Affects:** Standard library, FFI
**Related:** stdlib-philosophy-proposal.md (approved), platform-ffi-proposal.md (approved)

---

## Summary

This proposal selects the FFI backend for `std.http` — Ori's HTTP client and server library.

---

## Problem Statement

The stdlib-philosophy-proposal lists `std.http` with "TBD (libcurl or custom)" as the FFI backend. We need to decide:

1. Which C library to use for native HTTP
2. How to handle WASM (no direct socket access)
3. Whether to use a single backend or platform-specific backends

---

## Candidates

### libcurl

The ubiquitous HTTP library.

**Pros:**
- Extremely mature and battle-tested
- Supports HTTP/1.1, HTTP/2, HTTP/3
- Handles TLS, cookies, redirects, authentication
- Available everywhere

**Cons:**
- Large dependency (many features we won't use)
- Complex configuration
- Callback-heavy API

### hyper-c (hyper's C API)

Rust's hyper HTTP library with C bindings.

**Pros:**
- Modern, async-first design
- Memory-safe core (Rust)
- Actively maintained

**Cons:**
- Requires Rust runtime
- Less mature C API
- Smaller ecosystem

### Custom Implementation

Build HTTP on top of `std.net` sockets.

**Pros:**
- Full control over API design
- Minimal dependencies
- Consistent with Ori philosophy

**Cons:**
- Significant implementation effort
- Security risk (TLS, parsing)
- HTTP/2 and HTTP/3 are complex

---

## WASM Considerations

Browser WASM cannot use raw sockets. Options:

1. **fetch API**: Use JavaScript's `fetch()` via `extern "js"`
2. **No HTTP in WASM**: Error when importing std.http in WASM target
3. **HTTP proxy**: Require user-provided HTTP capability impl

---

## Open Questions

1. HTTP/2 and HTTP/3 support: required or optional?
2. WebSocket support: separate module or part of std.http?
3. Streaming bodies: how to expose without full async?
4. TLS configuration: how much control to expose?

---

## Recommendation

TBD pending community input and use case analysis.

---

## References

- libcurl: https://curl.se/libcurl/
- hyper: https://hyper.rs/
- stdlib-philosophy-proposal.md
