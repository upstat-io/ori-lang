# Proposal: Self-Contained Toolchain

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-03-23
**Affects:** Build system, AOT linker, installer, CI, release packaging

---

## Summary

`ori build` currently requires an external platform linker (MSVC `link.exe` on Windows, `gcc`/`cc` on Linux/macOS) to produce native executables. This proposal defines a phased plan to make the Ori toolchain self-contained — a single download that can compile `.ori` files to native executables with zero external dependencies.

---

## Motivation

### The Problem

Today, a user who downloads Ori and runs `ori build hello.ori` on a fresh machine gets:

```
error: linker 'link.exe' not found
```

On Windows, fixing this requires installing Visual Studio Build Tools (~2-4 GB, requires Microsoft account for Community edition, or paid license for enterprise). On Linux, it requires `gcc` or `build-essential`. On macOS, it requires Xcode Command Line Tools (~1.5 GB).

This is the single largest friction point in the getting-started experience. A language that promises "if your code compiles, it works" shouldn't fail at the final step because of a missing system tool.

### Who This Affects

- **New users** trying Ori for the first time — the install script downloads a 22MB zip, but compilation fails without a multi-gigabyte toolchain
- **CI/CD pipelines** that need a minimal Ori installation without full build environments
- **Educators and students** on lab machines without admin privileges to install MSVC
- **Cross-compilation** workflows where the host doesn't have the target's native linker

### Prior Art

| Language | Ships linker? | Ships CRT/SDK? | Self-contained? |
|----------|--------------|-----------------|-----------------|
| **Go** | Yes (internal linker) | Yes (pure Go runtime) | **Yes** — `go build` works on fresh machines |
| **Zig** | Yes (bundled LLD) | Yes (bundles libc headers + MinGW CRT) | **Yes** — `zig build-exe` works anywhere |
| **Rust** | Ships `rust-lld` but doesn't use it by default on MSVC | No (needs MSVC CRT + SDK) | **No** — `rustup` offers to install VS Build Tools |
| **Swift** | No | No | **No** — requires Xcode on macOS, VS on Windows |
| **Deno** | N/A (runtime, not AOT) | N/A | Yes for its domain |
| **GraalVM Native Image** | No | No | **No** — requires platform C toolchain |

**Go and Zig prove this is achievable.** Go uses a custom linker and pure-Go runtime. Zig bundles LLVM's LLD and ships MinGW/musl headers, making cross-compilation a first-class feature (`zig cc` is widely used as a C cross-compiler).

### Why Now

- Ori already depends on LLVM 21 — LLD (LLVM's linker) is part of the same project
- The Ori runtime (`libori_rt`) is small (~600KB static library) with minimal CRT dependencies
- The alpha release phase is the right time to establish the toolchain story before users build workflows around external dependencies

---

## Design

### Phase 1: Bundle LLD (Months 1-2)

**Goal:** Eliminate the need for `link.exe` / `gcc` / `ld` — Ori ships its own linker.

#### What Changes

1. **Build LLD alongside LLVM** in the CI release pipeline
   - LLD is already part of the LLVM monorepo (`llvm-project/lld/`)
   - Add `-DLLVM_ENABLE_PROJECTS="...;lld"` to the LLVM build configuration
   - Produces `lld-link` (COFF/Windows), `ld.lld` (ELF/Linux), `ld64.lld` (Mach-O/macOS)

2. **Ship the LLD binary in the release archive**
   - Windows: `ori.exe` + `lld-link.exe` (~15MB additional)
   - Linux: `ori` + `ld.lld` (~15MB additional)
   - macOS: `ori` + `ld64.lld` (~15MB additional)

3. **Prefer bundled LLD over system linker**
   - `MsvcLinker` / `GccLinker` first check for `lld-link` / `ld.lld` next to `ori` binary
   - Fall back to system linker if bundled LLD not found (backward compatible)
   - Add `--linker=system` / `--linker=bundled` flags for explicit control

4. **Update installer** to place LLD alongside `ori` in the install directory

#### What This Solves

- **Windows:** No more "install Visual Studio Build Tools" — `ori build` works out of the box *if* MSVC CRT libs are already present (they usually are from Windows SDK or VS redistributables)
- **Linux:** No more `apt install gcc` — direct ELF linking via `ld.lld`
- **macOS:** No more Xcode CLT — direct Mach-O linking via `ld64.lld`

#### What This Doesn't Solve

- Windows still needs MSVC CRT libraries (`libcmt.lib`, `vcruntime140.lib`) and Windows SDK import libraries (`kernel32.lib`, `ntdll.lib`, `ucrt.lib`) for the final link
- Linux still needs `libc.so` / `crt1.o` / `crti.o` (from `libc-dev` or equivalent)
- macOS still needs system frameworks and `libSystem.dylib`

#### Estimated Size Impact

| Platform | Current release | After Phase 1 |
|----------|----------------|---------------|
| Windows | ~22 MB | ~37 MB |
| Linux | ~25 MB | ~40 MB |
| macOS | ~24 MB | ~39 MB |

### Phase 2: Bundle Minimal Platform Libraries (Months 3-6)

**Goal:** Eliminate the need for platform SDK installations — true zero-dependency compilation.

This phase is the most complex and platform-specific.

#### Windows: Bundle MinGW CRT (Zig's Approach)

The key insight from Zig: you don't need MSVC. MinGW provides open-source implementations of the Windows CRT that are link-compatible with MSVC-generated code.

1. **Bundle MinGW CRT import libraries** (~5 MB)
   - `libkernel32.a`, `libntdll.a`, `libucrt.a`, `libmsvcrt.a`
   - These are import stubs — the actual DLLs are part of every Windows installation
   - Licensed under public domain / MIT (MinGW-w64 headers/libs)

2. **Bundle a minimal CRT startup** (~50 KB)
   - `crt2.o` (entry point), `crtbegin.o`, `crtend.o`
   - Or: generate our own entry point in LLVM IR (Ori already generates `main()` wrappers)

3. **Switch Windows linking to MinGW-compatible mode**
   - Use `ld.lld` (ELF-flavor LLD in MinGW mode) or `lld-link` with MinGW libs
   - This is exactly what Zig does — it bundles MinGW and uses LLD

**Trade-off:** MinGW-linked binaries use `msvcrt.dll` (available on all Windows) instead of the UCRT (`ucrtbase.dll`, available on Windows 10+). For Ori's alpha stage targeting modern Windows, this is acceptable.

#### Linux: Bundle musl libc (Static Linking Option)

1. **Ship prebuilt `musl-libc` static archives** (~2 MB)
   - Enables fully static Linux binaries with zero runtime dependencies
   - Add target `x86_64-unknown-linux-musl` alongside the default glibc target

2. **For glibc targets**, continue requiring system libc
   - But document clearly: `apt install libc-dev` (50 MB) vs MSVC Build Tools (4 GB)
   - Linux is already the easiest platform — most dev machines have `gcc` or `cc`

#### macOS: System Frameworks (Hardest Case)

macOS is the most constrained platform due to Apple's licensing:

1. **Xcode CLT stubs are sufficient** — `ld64.lld` can link against the `.tbd` (text-based stub) files that ship with Xcode CLT (~200 MB vs full Xcode 12 GB)
2. **Cannot redistribute Apple frameworks** — licensing prohibits bundling macOS SDK
3. **Practical approach:** Auto-detect Xcode CLT; if missing, prompt user to install via `xcode-select --install` (no Apple ID required, ~200 MB, installs from Apple CDN)

This means macOS remains the one platform where a small external install is required, similar to Go and Zig's situation.

### Phase 3: Cross-Compilation (Months 6-12)

**Goal:** `ori build --target=linux-x86_64` works from any host platform.

With LLD and platform libraries bundled, cross-compilation becomes possible:

1. **Ship all platform libraries in a single archive** (or downloadable components)
2. **Add `--target` flag** to `ori build`
3. **Generate platform-specific LLVM IR** based on target triple
4. **Link with the target's bundled libraries** using the appropriate LLD flavor

This is Zig's killer feature (`zig cc --target=aarch64-linux-musl`) and would be a significant differentiator for Ori.

---

## Alternatives Considered

### A: Auto-Install MSVC (Rust's Approach)

The Ori installer could detect missing MSVC and offer to install Visual Studio Build Tools, similar to `rustup-init.exe`.

**Rejected because:**
- Downloads 2-4 GB for a 22 MB language toolchain
- Requires Microsoft account for VS Community
- Enterprise licensing concerns
- Doesn't work on machines without admin privileges
- Doesn't solve Linux or macOS

### B: Require Docker/Containers

Ship an official Docker image with all dependencies pre-installed.

**Rejected because:**
- Terrible getting-started experience ("install Docker to run Hello World")
- Doesn't help native development workflows
- Overkill for the problem

### C: Interpreter-Only Mode (No AOT)

For getting-started, use `ori run` (interpreter) exclusively and treat `ori build` as an advanced feature requiring a toolchain.

**Rejected because:**
- `ori run` already works without a linker — this is the current status quo
- But users expect `ori build` to produce executables — that's the whole point of AOT
- Deferring AOT usability to "advanced" undermines Ori's performance story

### D: Ship LLVM's compiler-rt Instead of Platform CRT

Use LLVM's `compiler-rt` builtins library as a CRT replacement.

**Not sufficient because:**
- `compiler-rt` provides intrinsics (integer division, float conversion) but NOT the C runtime (stdio, memory allocation, thread-local storage)
- Still need platform libc or a replacement (musl, MinGW)
- But `compiler-rt` builtins SHOULD be bundled alongside LLD for completeness

---

## Implementation Plan

### Phase 1 Milestones

1. **CI: Build LLD alongside LLVM** — modify the LLVM build scripts in CI to enable `lld`
2. **Release: Include LLD in archives** — update release packaging to ship `lld-link`/`ld.lld`/`ld64.lld`
3. **Compiler: Prefer bundled LLD** — update `MsvcLinker`/`GccLinker` to search for LLD next to the `ori` binary
4. **Installer: Deploy LLD** — update `install.sh` to place LLD in the install directory
5. **Docs: Update prerequisites** — document that external linker is optional when LLD is bundled
6. **Tests: CI validation** — add a CI job that builds with bundled LLD and no system linker

### Phase 2 Milestones

1. **Research: MinGW CRT licensing** — confirm all bundled files are public domain or permissively licensed
2. **Prototype: MinGW-linked Windows binary** — prove `ori build` works with bundled MinGW + LLD
3. **Prototype: musl-linked Linux binary** — prove static Linux binaries work
4. **Package: Create platform library bundles** — automate extraction of minimal library sets
5. **Compiler: Self-contained link mode** — add `--self-contained` flag that uses bundled libraries
6. **Installer: Ship platform libraries** — update archives to include library bundles

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| LLD compatibility issues with platform-specific code | Link failures on edge cases | Fall back to system linker; file LLD bugs upstream |
| MinGW ABI incompatibility with MSVC-compiled libraries | FFI breaks for Windows libraries | Default to MinGW for pure Ori; offer `--msvc` mode for FFI-heavy projects |
| Release archive size growth (22 MB → ~45 MB) | Slower downloads | Use better compression (zstd); offer "minimal" install without LLD |
| macOS SDK licensing prevents bundling | Can't be fully self-contained on macOS | Accept this limitation; auto-prompt for Xcode CLT (200 MB, no account required) |
| Maintenance burden of bundled libraries | Stale CRT/SDK versions | Automate updates in CI; version-pin and test quarterly |

---

## Success Metrics

- **Phase 1:** `ori build hello.ori` produces a working executable on a fresh Windows machine with only Windows 10/11 installed (no VS, no MSVC, no MinGW) — using bundled LLD + system DLL import libs
- **Phase 2:** `ori build hello.ori` produces a working executable on a completely fresh Windows 10 machine — zero external dependencies
- **Phase 3:** `ori build --target=linux-x86_64 hello.ori` works from Windows/macOS

---

## References

- [Zig's approach to shipping a C/C++ toolchain](https://ziglang.org/learn/overview/#cc-compiler)
- [Go internal linker documentation](https://pkg.go.dev/cmd/link)
- [LLVM LLD documentation](https://lld.llvm.org/)
- [MinGW-w64 import library licensing](https://www.mingw-w64.org/licenses/)
- [Rust LLD tracking issue (rust-lang/rust#71520)](https://github.com/rust-lang/rust/issues/71520)
- [musl libc — static linking for Linux](https://musl.libc.org/)
