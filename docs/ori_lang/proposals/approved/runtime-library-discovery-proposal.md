# Proposal: Runtime Library Discovery

**Status:** Approved
**Created:** 2026-02-02
**Approved:** 2026-02-02
**Affects:** Compiler (AOT), build system, installation
**Depends on:** Platform FFI Proposal (approved)

---

## Summary

Define how the Ori compiler discovers `libori_rt.a` (the runtime library) during AOT compilation. Following rustc's sysroot pattern, discovery walks up from the compiler binary location rather than relying on environment variables.

---

## Motivation

AOT-compiled Ori programs must link against `libori_rt.a`, which provides:
- Memory allocation (ARC runtime)
- Panic handling
- Platform-specific entry points

The compiler must locate this library reliably across:
1. **Installed systems**: `/usr/local/bin/ori` with `/usr/local/lib/libori_rt.a`
2. **Development builds**: Cargo workspace with `target/release/libori_rt.a`
3. **Custom installations**: User-specified locations

### Design Principles

1. **Convention over configuration**: Standard layouts work without configuration
2. **No magic environment variables**: Env vars are for overrides, not primary discovery
3. **Relative to binary**: Like rustc, discover paths relative to the compiler executable
4. **Fail fast with clear errors**: List searched paths when library not found

---

## Prior Art Analysis

### Rust (rustc)

Rust's `rustc_session/src/filesearch.rs`:

```rust
// Walk up from binary: bin/rustc -> lib/rustlib/
fn default_sysroot() -> PathBuf {
    // Pop off `bin/rustc`, obtaining the suspected sysroot.
    p.pop();
    p.pop();
    // Validate by checking if rustlib directory exists
    rustlib_path.exists().then_some(p)
}
```

**Key insight**: No `RUST_SYSROOT` env var for basic operation. Uses `--sysroot` CLI flag only for overrides.

### Zig

Uses `ZIG_LIB_DIR` environment variable, but also walks from binary:

```zig
// Check relative to binary first
const self_exe_path = std.fs.selfExeDirPath();
// Then check ZIG_LIB_DIR
```

### Go

Uses `GOROOT` but defaults to compile-time path embedded in binary.

### Our Approach

Follow Rust most closely:
- Primary: walk up from binary
- CLI flag (`--runtime-path`) for overrides
- No environment variable for primary discovery

---

## Discovery Algorithm

The compiler searches in order, returning the first directory containing `libori_rt.a`:

### 1. CLI Override (highest priority)

```bash
ori build --runtime-path=/custom/path
```

If specified, use this path directly. Error if library not found there.

### 2. Installed Layout (relative to binary)

For binary at `/usr/local/bin/ori`:

```
<binary>/../lib/libori_rt.a
  → /usr/local/lib/libori_rt.a
```

This matches the standard Unix FHS layout:
- `/usr/local/bin/ori` (compiler)
- `/usr/local/lib/libori_rt.a` (runtime)

### 3. Development Layout (Cargo workspace)

When running from a Cargo build (`target/debug/ori` or `target/release/ori`):

```
<binary>/libori_rt.a
  → target/release/libori_rt.a
```

Both debug and release profiles are checked in the same directory as the compiler.

### 4. Workspace Root Target (Development Only)

If `ORI_WORKSPACE_DIR` is set (during development builds):

```
$ORI_WORKSPACE_DIR/target/release/libori_rt.a
$ORI_WORKSPACE_DIR/target/debug/libori_rt.a
```

This is primarily for the Ori compiler's own development. End users should rely on binary-relative discovery.

### Error: Library Not Found

If none succeed, emit a clear error listing all searched paths:

```
error: Ori runtime library (libori_rt.a) not found

Searched paths:
  - /home/user/.local/lib/libori_rt.a
  - /home/user/ori/target/release/libori_rt.a

To fix this:
  1. Build the runtime: cargo build -p ori_rt --release
  2. Install Ori properly: make install
  3. Specify path manually: ori build --runtime-path=/path/to/lib
```

---

## Platform-Specific Library Names

| Platform | Library Name |
|----------|--------------|
| Linux/macOS | `libori_rt.a` |
| Windows | `ori_rt.lib` |

The discovery algorithm uses the appropriate name for the current platform.

---

## Implementation

### RuntimeConfig::detect()

```rust
impl RuntimeConfig {
    pub fn detect() -> Result<Self, RuntimeNotFound> {
        let mut searched = Vec::new();
        let lib_name = Self::lib_name();

        // 1. Relative to current executable
        if let Ok(exe_path) = std::env::current_exe() {
            let exe_path = exe_path.canonicalize().unwrap_or(exe_path);

            if let Some(exe_dir) = exe_path.parent() {
                // Installed layout: bin/ori -> lib/libori_rt.a
                let lib_path = exe_dir.join("../lib");
                if Self::lib_exists(&lib_path, lib_name) {
                    return Ok(Self::new(lib_path));
                }
                searched.push(lib_path);

                // Dev layout: same directory as binary
                if Self::lib_exists(exe_dir, lib_name) {
                    return Ok(Self::new(exe_dir.to_path_buf()));
                }
                searched.push(exe_dir.to_path_buf());
            }
        }

        // 2. Ori workspace (for development builds)
        if let Ok(workspace) = std::env::var("ORI_WORKSPACE_DIR") {
            for profile in ["release", "debug"] {
                let path = PathBuf::from(&workspace).join("target").join(profile);
                if Self::lib_exists(&path, lib_name) {
                    return Ok(Self::new(path));
                }
                searched.push(path);
            }
        }

        Err(RuntimeNotFound { searched_paths: searched })
    }

    fn lib_name() -> &'static str {
        if cfg!(windows) { "ori_rt.lib" } else { "libori_rt.a" }
    }

    fn lib_exists(dir: &Path, lib_name: &str) -> bool {
        dir.join(lib_name).exists()
    }
}
```

---

## Installation Layout

### Standard Install (`make install PREFIX=/usr/local`)

```
/usr/local/
├── bin/
│   └── ori           # Compiler binary
└── lib/
    └── libori_rt.a   # Runtime library
```

### Development Build (Cargo workspace)

```
ori_lang/
├── target/
│   ├── debug/
│   │   ├── ori           # Debug compiler
│   │   └── libori_rt.a   # Debug runtime
│   └── release/
│       ├── ori           # Release compiler
│       └── libori_rt.a   # Release runtime
└── ...
```

---

## Build System Integration

### Cargo Build

The `ori_rt` crate should be built alongside the compiler. Add to workspace `Cargo.toml`:

```toml
[workspace]
members = [
    "compiler/oric",
    "runtime/ori_rt",
    # ...
]
```

When building `oric`, also build `ori_rt`:

```bash
cargo build -p oric -p ori_rt --release
```

Or configure `oric` to depend on `ori_rt` as a build dependency.

### Install Target

The `Makefile` or install script should copy both:

```makefile
install:
    install -m 755 target/release/ori $(PREFIX)/bin/
    install -m 644 target/release/libori_rt.a $(PREFIX)/lib/
```

---

## CLI Flag

Add `--runtime-path` to override discovery:

```bash
# Use custom runtime location
ori build --runtime-path=/opt/ori/lib program.ori

# Useful for cross-compilation or testing
ori build --runtime-path=./test-runtime program.ori
```

This flag takes precedence over all discovery paths.

---

## Rejected Alternatives

### Environment Variable (ORI_LIB_DIR / ORI_RT_PATH)

**Rejected.** Environment variables add configuration complexity and are easy to forget or misconfigure. Rust doesn't use one; neither should we. The `--runtime-path` CLI flag provides explicit override capability when needed.

> **Note:** `ORI_WORKSPACE_DIR` is retained for development builds only, as it aids compiler development but is not expected for end-user installations.

### Hardcoded Paths

**Rejected.** Paths like `/home/user/ori_lang/target/release` are brittle, machine-specific, and break on any system with a different layout.

### Embedded Path at Compile Time

**Rejected.** Like Go's `GOROOT`, this would require rebuilding the compiler to change the path. The binary-relative approach is more flexible.

### Search PATH for Library

**Rejected.** Searching `LD_LIBRARY_PATH` or system library paths could find the wrong version. Explicit relative paths are more predictable.

---

## Migration

No migration needed. This is the initial design for AOT compilation.

---

## Testing

1. **Installed layout**: Verify discovery works when compiler is in `/usr/local/bin/`
2. **Dev layout**: Verify discovery works when running `./target/release/ori`
3. **Missing library**: Verify clear error message with searched paths
4. **CLI override**: Verify `--runtime-path` takes precedence
5. **Cross-platform**: Verify correct library name on Windows vs Unix

---

## Summary

| Scenario | Discovery Path |
|----------|----------------|
| Installed (`/usr/local/bin/ori`) | `../lib/libori_rt.a` |
| Dev build (`target/release/ori`) | `./libori_rt.a` (same dir) |
| Workspace dev | `$ORI_WORKSPACE_DIR/target/{release,debug}/` |
| CLI override | `--runtime-path=/path` |

The key principle: **walk up from the binary, not down from environment variables.**
