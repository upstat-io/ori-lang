# Proposal: std.regex FFI Backend Selection

**Status:** Draft
**Created:** 2026-01-31
**Affects:** Standard library, FFI
**Related:** stdlib-philosophy-proposal.md (approved), platform-ffi-proposal.md (approved)

---

## Summary

This proposal selects the FFI backend for `std.regex` — Ori's regular expression library.

---

## Problem Statement

The stdlib-philosophy-proposal lists `std.regex` with "TBD (PCRE2 or RE2)" as the FFI backend. We need to decide:

1. Which regex engine to use
2. What syntax/features to support
3. How to handle WASM

---

## Candidates

### PCRE2

Perl-Compatible Regular Expressions, version 2.

**Pros:**
- Feature-rich (lookahead, lookbehind, backreferences, Unicode)
- Widely used and well-documented
- JIT compilation for performance

**Cons:**
- Complex feature set may encourage slow patterns
- Potential for catastrophic backtracking (ReDoS)
- Larger binary size

### RE2

Google's regular expression library.

**Pros:**
- Guaranteed linear time (no catastrophic backtracking)
- Good Unicode support
- Predictable performance

**Cons:**
- No backreferences
- No lookahead/lookbehind
- May surprise users expecting PCRE features

### rust-regex (via C API)

Rust's regex crate with potential C bindings.

**Pros:**
- Memory-safe
- Good performance
- Balances features and safety

**Cons:**
- Requires building C bindings
- Rust runtime dependency

### Pure Ori

Implement regex engine in Ori.

**Pros:**
- Full control
- Consistent cross-platform behavior

**Cons:**
- Massive undertaking
- Performance unlikely to match C implementations

---

## Feature Comparison

| Feature | PCRE2 | RE2 | rust-regex |
|---------|-------|-----|------------|
| Backreferences | Yes | No | No |
| Lookahead | Yes | No | No |
| Lookbehind | Yes | No | No |
| Unicode | Yes | Yes | Yes |
| Named groups | Yes | Yes | Yes |
| Linear time | No | Yes | Yes |
| JIT | Yes | No | No |

---

## WASM Considerations

For WASM targets:

1. **JavaScript RegExp**: Use browser's regex via `extern "js"`
2. **Compile engine to WASM**: Use same engine, compiled to WASM
3. **Pure Ori fallback**: Limited feature set for WASM

---

## Open Questions

1. Should Ori guarantee linear-time regex (preventing ReDoS)?
2. Is the lack of backreferences acceptable?
3. Should syntax be Perl-compatible or RE2-compatible?
4. What's the priority: features vs. safety?

---

## Recommendation

**Tentative: RE2**

Rationale:
- Linear time guarantee aligns with Ori's "if it compiles, it works" philosophy
- Predictable performance is more important than exotic features
- Users needing backreferences can use community packages

This is subject to community feedback.

---

## References

- PCRE2: https://www.pcre.org/
- RE2: https://github.com/google/re2
- rust-regex: https://docs.rs/regex/
- ReDoS: https://owasp.org/www-community/attacks/Regular_expression_Denial_of_Service_-_ReDoS
