# Proposal: Standard Library Philosophy

**Status:** Approved
**Approved:** 2026-01-31
**Created:** 2026-01-30
**Affects:** Standard library, package management, project structure

---

## Summary

Ori's standard library follows a "batteries included, independently versioned" philosophy. Official `std.*` packages cover common programming tasks, are maintained by the core team with long-term commitment, use FFI to proven C libraries, and can be patched independently of compiler releases.

---

## Motivation

### The Problem with Minimal Stdlibs

Languages with minimal standard libraries (Rust's approach) push essential functionality to community packages:

- **Decision fatigue**: Which HTTP client? Which datetime library? Which JSON parser?
- **Maintenance risk**: Key packages maintained by individuals who may abandon them
- **Quality variance**: No guarantee community packages meet security/correctness standards
- **Dependency explosion**: Simple projects accumulate hundreds of transitive dependencies
- **Fragmentation**: Competing packages create incompatible ecosystems

A language without a comprehensive stdlib is "just a syntax engine."

### The Problem with Monolithic Stdlibs

Traditional monolithic stdlibs (early Java, Python) have different issues:

- **Slow iteration**: Everything ships with the compiler
- **Security response**: Patching requires full toolchain release
- **Deprecation difficulty**: Bad APIs live forever
- **Bloat**: Users pay for everything whether they use it or not

### Ori's Approach: Best of Both

Official packages with independent versioning:

- **Batteries included**: Common tasks don't require third-party packages
- **Core team maintained**: Long-term commitment, not community gamble
- **FFI-backed**: Leverage battle-tested C libraries
- **Security agility**: Patch `std.crypto` without releasing new compiler
- **No decision fatigue**: Clear canonical choices

---

## Architecture

### Core (Versioned with Compiler)

Fundamental language constructs that cannot be separated from the compiler:

```
Primitives:      int, float, bool, str, char, byte, void, Never
Special types:   Duration, Size
Collections:     [T], {K: V}, (T, U), Set<T>
Option/Result:   Option<T>, Result<T, E>, Error
Core traits:     Eq, Comparable, Clone, Hashable, Printable, Debug,
                 Iterator, Iterable, Collect, Into, Default, Index
Built-ins:       print, len, assert, panic, todo, unreachable, dbg, etc.
```

These are inseparable from the compiler and version together.

### Official Stdlib Packages (Semver Independent)

Packages maintained by the core team with long-term support commitment:

| Package | Purpose | FFI Backend |
|---------|---------|-------------|
| `std.math` | Mathematical functions, constants | libm |
| `std.fs` | File system operations | libc/POSIX, Win32 |
| `std.time` | Date, time, timezone, duration | libc, IANA tzdb |
| `std.json` | JSON parsing and serialization | yyjson |
| `std.crypto` | Cryptographic primitives | libsodium |
| `std.net` | TCP/UDP networking | libc sockets |
| `std.http` | HTTP client and server | TBD (libcurl or custom) |
| `std.regex` | Regular expressions | TBD (PCRE2 or RE2) |
| `std.compression` | Compression algorithms | zlib, zstd |
| `std.encoding` | Base64, hex, URL encoding | Pure Ori |
| `std.uuid` | UUID generation and parsing | Pure Ori + std.crypto |
| `std.log` | Structured logging | Pure Ori |
| `std.test` | Assertion methods, test doubles, mocking utilities | Pure Ori |

### Community Packages

Everything else lives in the community ecosystem:

- Domain-specific libraries (game engines, ML frameworks)
- Alternative implementations (different trade-offs)
- Experimental features
- Niche formats and protocols

Clear separation: if it's `std.*`, it's official and maintained.

---

## Versioning

### Semver for Stdlib Packages

Each `std.*` package follows semantic versioning:

```
std.crypto 1.0.0  # Initial release
std.crypto 1.0.1  # Security patch (CVE fix)
std.crypto 1.1.0  # New feature (new algorithm)
std.crypto 2.0.0  # Breaking change (rare, requires migration)
```

### Compiler Compatibility

Each compiler version specifies compatible ranges for stdlib packages:

```toml
# Compiler 0.5.0 ships with:
[stdlib.defaults]
math = "1.0"
fs = "1.2"
time = "1.1"
json = "1.0"
crypto = "1.0"
```

Users get these by default. Override only when needed:

```toml
# Project ori.toml
[dependencies]
std.crypto = "1.0.1"  # Security patch
# Everything else uses compiler defaults
```

### Security Patch Flow

```
Day 0: CVE discovered in libsodium
Day 1: libsodium team releases patch
Day 2: Ori team updates std.crypto FFI bindings
Day 2: std.crypto 1.0.1 released
Day 2: Users: `ori update std.crypto && ori build`
```

No compiler release required. No full toolchain update. Minimal disruption.

---

## The Stdlib Contract

If a package is in `std.*`:

1. **Core team maintains it** - Not one person's side project
2. **Long-term support** - Won't be abandoned
3. **Security priority** - CVEs patched promptly
4. **Quality standard** - Reviewed, tested, documented
5. **Stability commitment** - Breaking changes are rare and managed
6. **FFI correctness** - Backed by proven C libraries where appropriate

This is the promise that makes `std.*` trustworthy.

---

## What Belongs in Stdlib

### Inclusion Criteria

A package should be in `std.*` if:

1. **Nearly universal need** - Most programs need it (fs, json, http)
2. **Security sensitive** - Correctness is critical (crypto, auth)
3. **Platform abstraction** - Hides OS differences (fs, net, time)
4. **Stable domain** - Not rapidly evolving (math, encoding)
5. **Benefits from FFI** - C libraries provide correctness/performance

### Exclusion Criteria

A package should NOT be in `std.*` if:

1. **Niche use case** - Only some domains need it (game physics, ML)
2. **Rapidly evolving** - Standards/practices change fast (web frameworks)
3. **Multiple valid approaches** - No clear "right way" (ORMs, state management)
4. **Large/heavy** - Would bloat installs (GUI toolkits)

### Gray Areas

Some packages might start as community and graduate to stdlib:

```
Community proves value → Stabilizes → Core team adopts → Becomes std.*
```

This is safer than premature standardization.

---

## Default Imports

For convenience, new projects include stdlib defaults:

```toml
# Generated ori.toml for new project
[dependencies]
std.fs = "default"
std.json = "default"
std.time = "default"
# std.math, std.crypto, etc. available but not auto-imported
```

Users explicitly import what they need:

```ori
use std.fs { read, write }
use std.json { parse, stringify }
use std.crypto { hash_password }
```

No implicit imports beyond the prelude.

---

## Package Distribution

### Official Registry

Stdlib packages live in the official Ori registry:

```
registry.ori-lang.org/std/math
registry.ori-lang.org/std/fs
registry.ori-lang.org/std/crypto
```

Community packages are separate:

```
registry.ori-lang.org/packages/cool-library
```

### Bundled with Toolchain

Compiler ships with compatible stdlib versions pre-downloaded:

```
ori-0.5.0/
├── bin/oric
├── lib/
│   └── std/
│       ├── math-1.0.0/
│       ├── fs-1.2.0/
│       ├── time-1.1.0/
│       └── ...
```

Works offline. Updates pulled only when requested.

---

## Migration from Monolithic

If we started monolithic and moved to modular, migration would be painful. Starting modular from day one avoids this.

The stdlib packages are designed together and tested together, but versioned separately. Users see a cohesive stdlib; the infrastructure allows independent patches.

---

## Examples

### Typical Project

```toml
# ori.toml
[project]
name = "my-app"
version = "1.0.0"

[dependencies]
# Uses compiler defaults for all std packages
# Override specific versions only if needed:
# std.crypto = "1.0.1"  # Uncomment if patching

[dependencies.community]
some-lib = "2.3.0"
```

```ori
// main.ori
use std.fs { read }
use std.json { parse_as }
use std.http { get }

type Config = { api_url: str, timeout: Duration }

@main () -> void uses FileSystem, Http =
    {
        let config = read(path: "config.json")?
        let config = parse_as<Config>(source: config)?
        let response = get(url: config.api_url)?
        print(msg: response.body)
    }
```

No third-party packages needed for basic operations.

### Security Patch Scenario

```bash
# CVE announced for crypto vulnerability
$ ori outdated
std.crypto: 1.0.0 -> 1.0.1 (security patch available)

$ ori update std.crypto
Updated std.crypto to 1.0.1

$ ori build
# Done. No compiler update needed.
```

---

## Summary

| Aspect | Ori's Approach |
|--------|----------------|
| Core primitives | Versioned with compiler |
| Stdlib packages | Semver independent, core team maintained |
| Common tasks | Covered by std.* (batteries included) |
| Implementation | FFI to proven C libraries |
| Security patches | Independent of compiler releases |
| Community packages | Separate ecosystem, not std.* |
| Long-term commitment | If it's std.*, we maintain it |

**The principle**: A language is only as useful as its standard library. Ori provides comprehensive, trustworthy, maintainable stdlib packages so users can build real software without gambling on third-party maintenance.
