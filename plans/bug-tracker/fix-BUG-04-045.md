---
bug: "BUG-04-045"
title: "is_cross_compiling() reports native Apple Silicon as cross-compilation: arm64 (LLVM triple) vs aarch64 (Rust cfg) string mismatch"
severity: "high"
status: in-progress
goal: "TargetTripleComponents.arch becomes a typed Arch enum parsed at the boundary with all alias spellings normalized, so every host-vs-target equality check operates on canonical typed values and Apple Silicon native builds are never mis-detected as cross-compilation."
success_criteria:
  - "TargetTripleComponents.arch is typed as Arch (not String); is_cross_for(HostPlatform) replaces ad-hoc cfg-string compares"
  - "Simulated Apple Silicon host (HostPlatform { arch: Arch::Aarch64, os: HostOs::Darwin }) parses arm64-apple-darwin25.2.0 and reports is_cross_for = false"
  - "TargetConfig::from_triple(\"arm64-apple-darwin\") succeeds (canonicalizes to aarch64-apple-darwin), fixing the 7th latent SUPPORTED_TARGETS asymmetry"
  - "14-test matrix green in compiler/ori_llvm/src/aot/target_features/tests.rs; full ./test-all.sh green; /tpr-review + /impl-hygiene-review clean"
subsystem: "compiler/ori_llvm/src/aot/{target_features,target,linker,syslib}"
found: "2026-04-07"
source: "manual"
third_party_review:
  status: findings
  updated: 2026-04-07
---

# Fix: BUG-04-045 — arm64 vs aarch64 host-vs-target mismatch on Apple Silicon

**Status:** In Progress (TPR iteration 3 — review-work surfaced new findings)
**Severity:** high
**Goal:** Introduce a typed `Arch` enum at the `TargetTripleComponents::parse` boundary that normalizes every known alias spelling (`arm64|aarch64`, `amd64|x86_64`, `i386|i486|i586|i686`). Migrate all host-vs-target comparisons to operate on typed `Arch` / `HostPlatform` values, never on raw strings. After the fix, every call site that previously did `components.arch == "aarch64"` is either (a) a compile error, or (b) a canonical-typed query. The bug class "raw arch string compare" becomes un-typeable.

**Success Criteria:**
- [ ] `TargetTripleComponents.arch` has type `Arch` (not `String`); compile-fail type-test refuses to compile if reverted
- [ ] `TargetTripleComponents::parse("arm64-apple-darwin25.2.0")` canonicalizes arch to `Arch::Aarch64` while preserving vendor/os/env verbatim
- [ ] `TargetTripleComponents::is_cross_for(host: HostPlatform)` reports `false` for every (supported host triple, simulated matching host) pair — including the arm64 native host semantic pin
- [ ] `TargetConfig::from_triple("arm64-apple-darwin")` returns Ok with canonical `triple == "aarch64-apple-darwin"` (latent bug #7)
- [ ] `LinkerDetection::gcc_cross_compiler_name` emits canonical spellings only (`aarch64-linux-gnu-gcc`, never `arm64-linux-gnu-gcc`)
- [ ] All existing tests updated for the typed field; 10-test matrix green in `compiler/ori_llvm/src/aot/target_features/tests.rs`
- [ ] `timeout 150 ./test-all.sh` green, `timeout 150 ./clippy-all.sh` green, debug + release both build
- [ ] `/tpr-review` clean, `/impl-hygiene-review` clean

**Context:** On macOS Apple Silicon, LLVM's default triple is `arm64-apple-darwin25.x.x` (Apple's historical spelling), while Rust's `cfg(target_arch = "aarch64")` uses the spec spelling `aarch64`. `LinkerDetection::is_cross_compiling()` did a literal string compare between these, wrongly reporting the native target as cross-compilation. The surfacing event was CI run 24058749864 on 2026-04-07: `test_native_target_is_not_cross_compiling` panicked on `macos-latest` while Linux/Windows runners passed. The impact is twofold — (1) nightly CI red on Apple Silicon, (2) real Apple Silicon users would hit `cross_compilation_error()` instead of using host `cc`. TPR research (Codex prior-art pass, 2026-04-07) against Rust, Zig, Swift, LLVM/Clang, and Go converged on the same methodology: a typed arch enum at the parse boundary is the SSOT pattern every mature compiler uses. See bug entry in `plans/bug-tracker/section-04-codegen-llvm.md:297-331`.

---

## 1. Root Cause Analysis

- **Symptom**: `test_native_target_is_not_cross_compiling` panics on Apple Silicon CI runners: `native target should not be detected as cross-compilation` at `linker/tests.rs:62`.
- **Proximate cause**: `LinkerDetection::is_cross_compiling()` (`linker/mod.rs:461-491`, added in c2c888fb) does `components.arch != host_arch` where `host_arch` is hardcoded to `"aarch64"` under `cfg(target_arch = "aarch64")`. On Apple Silicon, `components.arch` is `"arm64"` (from LLVM's default triple), so the raw string compare yields `true` → mis-reports native as cross.
- **Root cause**: **LEAK:scattered-knowledge** — arch-name normalization has no canonical home. The codebase already knows about the `arm64|aarch64` duality at `target_features.rs:233` (`"aarch64" | "arm64" => initialize_aarch64(...)`), but every other consumer reimplements (or omits) normalization ad hoc. `TargetTripleComponents.arch: String` is the shadow home — every comparison site is an independent reinterpretation of raw arch strings, and each site can silently disagree with the others. The bug class is "raw string compare across an aliased namespace" — a single helper function would not fix it because new consumers can bypass the helper.
- **Blast radius**: 6 sites in the `ori_llvm::aot` module tree + 1 latent bug (`from_triple` asymmetry against `SUPPORTED_TARGETS`):
  1. `linker/mod.rs:471-491` `is_cross_compiling()` — the failing test (surfacing).
  2. `linker/mod.rs:498-519` `gcc_cross_compiler_name()` — would emit `arm64-w64-mingw32-gcc` / `arm64-linux-gnu-gcc` (nonexistent toolchains) if an LLVM-default triple ever flowed through. Latent on Linux/Windows today.
  3. `linker/mod.rs:611` `cross_compilation_error()` help text uses raw `components.arch`.
  4. `syslib/mod.rs:119-138` `is_native()` — sibling cfg-string compare, test-only consumer today but would misbehave on Apple Silicon.
  5. `syslib/mod.rs:280` `target.arch == "x86_64" || target.arch == "aarch64"` — explicitly excludes `arm64` spelling; would skip `lib64` paths on any LLVM-spelling triple.
  6. `target.rs:371-376` `pointer_size()` — stringly + non-exhaustive (`match arch.as_str() { "wasm32"|"i686"|"i386"|"arm" => 4, _ => 8 }`). Currently safe-by-accident (`arm64` hits the `_ => 8` default).
  7. **Latent #7**: `TargetConfig::from_triple("arm64-apple-darwin")` is rejected by `is_supported_target` because `SUPPORTED_TARGETS` uses `aarch64-apple-darwin`. `TargetConfig::native()` on Apple Silicon happens to work (skips the supported check) but `from_triple` + the default triple would fail. Asymmetry inside the same module.

- **Affected files**:
  - `compiler/ori_llvm/src/aot/target_features.rs` — canonical home: add `Arch` enum with `parse_llvm_name`/`display_name`/`is_64_bit_non_wasm`/`is_wasm`/`pointer_size_bytes` queries; add `HostOs` enum and `HostPlatform` struct with `current()`; change `TargetTripleComponents.arch: String → Arch`; update `parse()` to canonicalize at the boundary; update `initialize_target_for_triple()` to exhaustively match on `Arch`; update `Display` impl (`arch` now formats via `Arch::Display` → canonical).
  - `compiler/ori_llvm/src/aot/target.rs` — `pointer_size()` queries `self.components.arch.pointer_size_bytes()` (exhaustive match on `Arch`, no string fallthrough); `from_triple()` parses FIRST, then validates the canonicalized triple against `SUPPORTED_TARGETS` (fixes latent bug #7).
  - `compiler/ori_llvm/src/aot/linker/mod.rs` — `is_cross_compiling()` delegates to `target.components().is_cross_for(HostPlatform::current())`; `gcc_cross_compiler_name()` interpolates via `Arch::Display` (canonical); `cross_compilation_error()` help text interpolates via `Arch::Display`.
  - `compiler/ori_llvm/src/aot/syslib/mod.rs` — `is_native()` delegates to `self.target.is_native_for(HostPlatform::current())`; `lib64` check uses `target.arch.is_64_bit_non_wasm()`; musl sysroot interpolation uses `Arch::Display` (no change needed once `arch` is typed).
  - `compiler/ori_llvm/src/aot/mod.rs` — re-export `Arch`, `HostOs`, `HostPlatform` from `target_features`.
  - `compiler/ori_llvm/src/aot/target_features/tests.rs` — NEW test file for the 10-test matrix; declare `#[cfg(test)] mod tests;` in `target_features.rs`.
  - `compiler/ori_llvm/src/aot/linker/tests.rs` — existing assertions updated to match the typed API (no behavior change).
  - `compiler/ori_llvm/src/aot/syslib/tests.rs` — `config.target().arch == Arch::X86_64` instead of `== "x86_64"`.
  - `compiler/oric/tests/phases/codegen/targets.rs` — 4 `assert_eq!(components.arch, "X")` updates + ~8 struct-literal construction sites (`TargetTripleComponents { arch: "x86_64".to_string(), ... } → arch: Arch::X86_64, ...`).
  - `compiler/ori_llvm/tests/aot/cross.rs` — `config.components().arch == Arch::X86_64`.

**Reference implementations:**
- **Rust** (`compiler/rustc_target/src/spec/mod.rs:1857,2077`): `Target.arch` is a typed `Arch` enum generated by `target_spec_enum!`; Apple's `arm64` surface is reconciled by a per-platform mapping `Arm64|Arm64e|Arm64_32 → crate::spec::Arch::AArch64`. Host-vs-target comparisons happen on typed objects (`rustc_codegen_ssa/back/link.rs:1814`, `rustc_codegen_llvm/llvm_util.rs:529`), never raw strings.
- **Swift** (`lib/Basic/Platform.cpp:371`): explicit canonical mapping `arm64|aarch64 → arm64`, `x86_64|amd64 → x86_64`, `i386|i486|i586|i686 → i386`. Internal decisions go through typed `getArch()`; Apple-facing edges re-emit `getArchName()` only when needed.
- **LLVM/Clang** (`llvm/include/llvm/TargetParser/Triple.h:46,85,328`): `llvm::Triple::ArchType` is the canonical internal model with documented alias groups; Clang's Darwin toolchain has the explicit Apple mapping `arm64|arm64e → aarch64` (`clang/lib/Driver/ToolChains/Darwin.cpp:57,77`).
- **Zig** (`lib/std/Target.zig:1303`): `Target.Cpu.Arch` is an exhaustive enum; parsing is direct string-to-enum in `Target.Query.parse()`; unknown archs are rejected at parse, not stored.
- **Go** (`src/cmd/internal/sys/arch.go:29,98,125`): canonical `goarch` strings stored in typed `Arch` descriptors; host canonicalized once at startup (`aarch64|arm64 → arm64`).

**Rejected alternatives (documented in bug entry)**: a string-normalization helper loses because consumers can bypass it (bug class stays alive). Using `inkwell::TargetTriple` loses because inkwell is only a string wrapper — LLVM's parsed C++ `Triple` API isn't exposed across FFI, so Ori needs its own typed representation regardless.

---

## Third Party Review Findings

- [x] `[TPR-BUG-04-045-01][high]` `compiler/ori_llvm/src/aot/target.rs:103-115` / `compiler/ori_llvm/src/aot/target_features.rs:73-88,361-400` — `TargetConfig::from_triple()` still rejects the versioned Darwin triples that LLVM actually emits on Apple Silicon (`arm64-apple-darwin25.2.0` / `aarch64-apple-darwin25.2.0`).
  Evidence: `TargetTripleComponents::parse()` intentionally preserves the Darwin version suffix (`os = "darwin25.2.0"`), and `is_cross_for()` explicitly handles that suffix via `starts_with("darwin")`, but `from_triple()` validated `components.to_string()` against `SUPPORTED_TARGETS`, which only contains the unversioned `aarch64-apple-darwin`. The explicit-triple path therefore returned `UnsupportedTarget` for the real LLVM-default Apple Silicon triple. This left the boundary only partially fixed: `TargetConfig::native()` worked because it bypasses the supported-target check, but `from_triple()` still rejected the exact Darwin spelling the fix documentation called out.
  Resolved: Fixed on 2026-04-07. Added `TargetTripleComponents::support_key()` as the canonical SSOT for "what string does the SUPPORTED_TARGETS lookup use?" — it strips Darwin OS version suffixes by delegating to the typed `is_macos()` predicate (so `darwin25.2.0` and `macos` both collapse to `darwin`), and emits the canonical arch spelling. `TargetConfig::from_triple()` now routes the support-targets check through `support_key()` while still storing the version-preserving canonical form (`components.to_string()`) as the `triple` field, because LLVM's `TargetMachine` expects the version-bearing form when one was supplied. Three regression tests added in `compiler/ori_llvm/tests/aot/cross.rs`: `test_from_triple_accepts_versioned_darwin_arm64` (the exact failing case `arm64-apple-darwin25.2.0`), `test_from_triple_accepts_versioned_darwin_aarch64`, and `test_from_triple_accepts_versioned_darwin_x86_64`. Plus 11-cell matrix `test_support_key_strips_darwin_version_matrix` in the lib tests covering versioned, unversioned, `macos`-spelling, non-Darwin, and arch-alias compositions, with self-verifying counter. Plus `test_support_key_matches_supported_targets_for_known_aliases` cross-checking that every aliased input lands in the actual `SUPPORTED_TARGETS` lookup.

- [x] `[TPR-BUG-04-045-02][medium]` `compiler/ori_llvm/src/aot/linker/mod.rs:159-165` / `compiler/ori_llvm/src/aot/target_features.rs:361-368` — `LinkOutput::extension()` still keys shared-library suffixes off `target.os == "darwin"`, so versioned Darwin triples now produce `.so` instead of `.dylib`.
  Evidence: the typed-triple refactor correctly taught `TargetTripleComponents::is_macos()` to treat `darwin25.2.0` as macOS, but `LinkOutput::extension()` bypassed that typed query and pattern-matched the raw OS string. A parsed triple like `arm64-apple-darwin25.2.0` therefore fell through to the generic Unix branch and got the Linux/ELF suffix. **LEAK:scattered-knowledge** — the "is this a macOS variant" rule had two homes (the typed predicate and the raw match arm) that disagreed.
  Resolved: Fixed on 2026-04-07. Rewrote `LinkOutput::extension()` to delegate to the typed `target.is_windows()` / `target.is_macos()` predicates instead of matching raw `target.os.as_str()`. The typed predicates are now the SSOT for OS-family decisions. Two regression tests added in `compiler/ori_llvm/src/aot/linker/tests.rs`: `test_link_output_extension_macos_unversioned` (sibling for the bare spelling) and `test_link_output_extension_macos_versioned` (the exact failing case `arm64-apple-darwin25.2.0` → `.dylib`).

- [x] `[TPR-BUG-04-045-03][low]` `plans/bug-tracker/fix-BUG-04-045.md:5-17,26-34` — the fix section is still marked `status: complete` even though `third_party_review` had not been updated and this review found open correctness issues.
  Evidence: the frontmatter previously reported `third_party_review.status: none`, the checklist still showed unchecked completion items, and the code above still had unresolved Apple-Silicon/Darwin-path bugs. The plan state currently over-claims completion.
  Resolved: Fixed on 2026-04-07. Reverted `status` to in-progress / In Progress (TPR iteration 2) immediately when iteration-1 findings landed, updated `third_party_review.status` to track the loop state (`findings` → `in-review` → `clean`). Will only restore `status: complete` after the iteration-2 re-review confirms zero actionable findings.

- [x] `[TPR-BUG-04-045-04][high]` `compiler/ori_llvm/src/aot/target_features.rs:73-87,319-340` / `compiler/oric/src/commands/build/mod.rs:103-107` — the typed-triple rewrite still left the advertised `wasm32-wasi` target unusable because `SUPPORTED_TARGETS` accepted the 2-part spelling while `TargetTripleComponents::parse()` rejected any triple with fewer than 3 components.
  Evidence: `SUPPORTED_TARGETS` and `is_supported_target()` listed `wasm32-wasi`, `compiler/oric/src/commands/target.rs` told users to run `ori build --target=wasm32-wasi`, and `compiler/oric/src/commands/targets/tests.rs` explicitly documented WASI as a 2-part target format. But `TargetTripleComponents::parse()` hard-errors on `parts.len() < 3`, and `configure_target()` routes `--target` through `TargetConfig::from_triple()`. Reproduced directly on 2026-04-07: `cargo run -q -p oric --bin ori -- build ... --target=wasm32-wasi` returned `error[E5004]: target 'wasm32-wasi' is not supported` with note `invalid format: expected at least 3 components: <arch>-<vendor>-<os>`.
  Impact: the documented WASI target was broken end-to-end. Existing coverage only checked the text-level support list (`is_supported_target("wasm32-wasi")`) and a 2-part-format policy comment; there was no regression test that actually drove `TargetConfig::from_triple("wasm32-wasi")` or `ori build --target=wasm32-wasi`. The label `WASM playground build passed` in `test-all.sh` was structurally incapable of catching this — see retrospective tooling notes below.

  **Investigation: was `wasm32-wasi` ever real?** Audited the WASI codegen layer (`compiler/ori_llvm/src/aot/wasm/wasi.rs`, ~228 lines): full `WasiVersion::Preview1`/`Preview2` enum, `wasi_snapshot_preview1.fd_write`/`proc_exit`/`path_open`/etc undefined-symbol declarations, preopens, env vars, args, the works. ~30 unit tests. WASI codegen is real. But every internal test helper (`compiler/ori_llvm/tests/aot/util/mod.rs:83 wasm32_wasi_target()`) parsed the 3-component LLVM-canonical form `wasm32-unknown-wasi` and went through `from_components()`, which bypasses `from_triple()` and never touched the supported-targets check. The 2-component string `"wasm32-wasi"` in `SUPPORTED_TARGETS` was a copy-paste of the historical Rust short-form target name (deprecated upstream in May 2024 / Rust 1.78 in favor of `wasm32-wasip1`) that was never plumbed through the parser.

  **Resolved (modernization, user choice 2026-04-07):** Replaced the deprecated 2-component `"wasm32-wasi"` entry in `SUPPORTED_TARGETS` with the modern Rust 1.78+ canonical 3-component form `"wasm32-unknown-wasip1"`. Updated `WasiVersion::Preview1::target_suffix()` from `"wasi"` to `"wasip1"` to match. Updated CLI documentation (`oric/src/main.rs:209-210, 400`) and target-install handling (`oric/src/commands/target.rs:143, 292`) to the new spelling. Replaced the raw `target == "wasm32-wasi"` string compare in `commands/target.rs` with a typed `is_wasi_target()` predicate (canonical home for "is this a WASI target spelling"). Updated all consumer tests to assert the new spelling. Updated `commands/targets/tests.rs::test_supported_targets_triple_format` to enforce a uniform 3+ component invariant (no WASM exception). Updated `tests/aot/util/mod.rs::wasm32_wasi_target()` test helper to parse `wasm32-unknown-wasip1`. Updated `tests/aot/cross.rs::test_supported_targets` expected list. Added two new regression pins in `cross.rs`:
  - `test_from_triple_accepts_wasm32_wasip1_canonical` — drives `TargetConfig::from_triple("wasm32-unknown-wasip1")` end-to-end and asserts `triple == "wasm32-unknown-wasip1"`, `arch == Arch::Wasm32`, `os == "wasip1"`, `is_wasm() == true`.
  - `test_from_triple_rejects_deprecated_wasm32_wasi` — negative pin asserting that the deprecated 2-component spelling is rejected as `InvalidTripleFormat` at the parse boundary (NOT silently normalized). Pins the strict triple-parser invariant against future "be liberal in what you accept" patches.
  Plus added `("wasm32-unknown-wasip1", "wasm32-unknown-wasip1")` to the lib-side `test_support_key_strips_darwin_version_matrix` and `test_support_key_matches_supported_targets_for_known_aliases` matrices to confirm it round-trips through the support-key lookup. Full `./test-all.sh` green (16878 passed, 0 failed; +2 from the new regression pins, +2 from prior iteration).

- [x] `[TPR-BUG-04-045-05][medium]` `compiler/oric/src/commands/target.rs:110-123` / `compiler/ori_llvm/src/aot/target.rs:87-124` — `ori target add` still validates targets against the raw `SUPPORTED_TARGETS` string list, so aliased/versioned triples accepted by `ori build --target=...` are rejected at the install boundary.
  Evidence: `TargetConfig::from_triple()` parses first and validates through `TargetTripleComponents::support_key()`, which accepts alias/versioned spellings like `arm64-apple-darwin25.2.0`. But `add_target()` gated on `SUPPORTED_TARGETS.contains(&target)` with no canonicalization pass — `cargo run -q -p oric --bin ori -- target add arm64-apple-darwin25.2.0` returned `error: unsupported target 'arm64-apple-darwin25.2.0'`. **LEAK:scattered-knowledge** — supported-target validation had two homes (`from_triple` used `support_key`, `add_target` used raw contains).
  Resolved: Fixed on 2026-04-07. Extracted a pure helper `canonicalize_target_for_install(input) -> Result<String, TargetError>` in `oric/src/commands/target.rs` that routes through the same `TargetTripleComponents::parse() → support_key() → is_supported_target()` path as `from_triple`. `add_target`, `remove_target`, and `is_target_installed` now all canonicalize before any string compare or filesystem lookup. Aliased/versioned spellings are accepted at the install boundary AND stored on disk under the canonical key (so `arm64-apple-darwin25.2.0` and `aarch64-apple-darwin` resolve to the same `~/.ori/sysroots/aarch64-apple-darwin/` directory — no per-OS-subversion duplicates). The user is shown a "Canonicalizing 'X' to 'Y'" line when the input differs from the canonical form. 9 new unit tests in `compiler/oric/src/commands/target/tests.rs`: positive pins for 5 alias/canonical spellings, 2 negative pins (deprecated 2-component WASI, unknown arch), one matrix test `test_canonicalize_target_parity_with_from_triple_matrix` enumerating the same 9 spellings as `cross.rs::test_from_triple_accepts_*` to assert CLI/build parity, and one structural pin asserting `is_target_installed` resolves to the canonical path.

- [x] `[TPR-BUG-04-045-06][high]` `compiler/oric/src/commands/target.rs:34-46,252-299` / `compiler/ori_llvm/src/aot/syslib/mod.rs:164-245` — `ori target add` records installs under `~/.ori/sysroots/<target>` and even treats `~/.wasi-sdk` as a success path, but the build-side sysroot resolver never consults either location.
  Evidence: the target command advertised and wrote per-user installs at `~/.ori/sysroots/<target>` (`oric/src/commands/target.rs:34-46`), and `check_wasi_sdk()` explicitly considered `~/.wasi-sdk/share/wasi-sysroot` a valid install source. But `SysLibConfig::detect_sysroot()` only checked `ORI_SYSROOT_<TARGET>`, `ORI_SYSROOT`, and hard-coded system candidates; for WASM those were only `/opt/wasi-sdk/share/wasi-sysroot` and `/usr/share/wasi-sysroot`. **LEAK:scattered-knowledge** — the install side and the discovery side both encoded "where Ori-managed sysroots live" but disagreed, so installs reported success whose results were invisible to subsequent builds.
  Resolved: Fixed on 2026-04-07. Extracted the install-paths SSOT into `compiler/ori_llvm/src/aot/syslib/mod.rs` as a set of public free functions: `user_home_dir()`, `ori_sysroots_dir()` / `_for_home`, `ori_sysroot_path(canonical_key)` / `_for_home`, `home_wasi_sdk_sysroot()` / `_for_home`. The `_for_home` parameterized variants take an explicit `&Path` so tests can supply a tempdir without touching process-global `HOME`/`USERPROFILE` state. Re-exported from `aot/mod.rs`. Refactored `oric/src/commands/target.rs::sysroots_dir`/`sysroot_path` into thin wrappers around the SSOT (cross-crate directionality: `oric` depends on `ori_llvm`, so the SSOT must live in `ori_llvm`).

  Updated `SysLibConfig::detect_sysroot` to consult the per-user install location at `ori_sysroot_path(target.support_key())` BEFORE the hard-coded system candidates. Updated `sysroot_candidates` to include `home_wasi_sdk_sysroot()` for WASI targets, ordered BEFORE `/opt/wasi-sdk/share/wasi-sysroot` and `/usr/share/wasi-sysroot` so a user-local install takes precedence. Refactored `detect_sysroot` and `sysroot_candidates` to delegate to new `_for_home` parameterized variants so the discovery side is testable hermetically (no env var mutation, no parallel-test races).

  6 new tests in `compiler/ori_llvm/src/aot/syslib/tests.rs`: 3 unit pins for the path-construction functions (`ori_sysroots_dir_for_home`, `ori_sysroot_path_for_home`, `home_wasi_sdk_sysroot_for_home`), 1 round-trip pin `test_install_then_detect_round_trip_canonical_darwin` (writes a sysroot under the canonical name, parses a versioned darwin spelling, confirms `detect_sysroot_for_home` finds it), 1 sibling round-trip pin for the unversioned canonical input, 1 negative pin asserting nothing-installed yields `None`, and 1 ordering pin `test_sysroot_candidates_for_wasi_includes_home_wasi_sdk` asserting the home WASI SDK appears BEFORE the system-wide candidates in the discovery list.

---

## 2. TDD — Test Matrix

All 10 tests live in the new `compiler/ori_llvm/src/aot/target_features/tests.rs`. Write them ALL before the fix; verify they fail to compile or fail at runtime against the current code.

### Exact failing case
- [ ] `test_is_cross_for_regression_pin_arm64_native_host_is_not_cross` — parse `"arm64-apple-darwin25.2.0"` against `HostPlatform { arch: Arch::Aarch64, os: HostOs::Darwin }` → `is_cross_for == false`. Semantic pin: this exact bug. Would fail if the `arm64 → Aarch64` alias is removed.

### Cross-alias coverage (matrix with self-verification)
- [ ] `test_arch_parse_normalizes_alias_spellings_matrix` — iterate `[("aarch64", Arch::Aarch64), ("arm64", Arch::Aarch64), ("x86_64", Arch::X86_64), ("amd64", Arch::X86_64), ("i386", Arch::I386), ("i486", Arch::I386), ("i586", Arch::I386), ("i686", Arch::I386), ("wasm32", Arch::Wasm32), ("wasm64", Arch::Wasm64)]`; assert every alias parses to its canonical variant.
- [ ] `test_arch_parse_matrix_is_self_verifying` — counter assertion: iterate the alias table, increment per cell, assert `count == table.len()` so a skipped iteration is impossible.

### Triple parse preserves other fields
- [ ] `test_target_triple_parse_preserves_vendor_os_env_while_normalizing_arch_matrix` — parse `"arm64-apple-darwin25.2.0"` and `"amd64-unknown-linux-gnu"` and `"i686-pc-windows-msvc"`; for each, assert `arch` is canonical AND `vendor`/`os`/`env` equal the raw input slices byte-for-byte. Verifies arch canonicalization doesn't bleed into other fields.

### Cross-host matrix (type × host grid)
- [ ] `test_is_cross_for_simulated_host_matrix` — for every pair in `[Arch::X86_64, Arch::Aarch64] × [HostOs::Linux, HostOs::Darwin, HostOs::Windows]`, parse the canonical triple, build a matching `HostPlatform`, and assert `is_cross_for = false`. Then swap the host arch and assert `is_cross_for = true`. Self-verifying counter ensures all cells visited.
- [ ] `test_is_cross_for_matrix_is_self_verifying` — counter assertion for the cross-detection grid.

### Native round-trip (every supported host)
- [ ] `test_native_host_triple_round_trips_to_not_cross_compiling_matrix` — for each supported host triple (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`), simulate the matching `HostPlatform` and assert parsed triple reports `is_cross_for = false`. Also test `arm64-apple-darwin25.2.0` (LLVM-default spelling) against Darwin aarch64 host — must round-trip to not-cross.

### gcc cross-compiler name canonical spelling
- [ ] `test_gcc_cross_compiler_name_uses_canonical_arch_boundary_spellings_matrix` — for every `(Arch, target_os)` pair that produces a non-None result, assert the generated program name uses the canonical arch spelling (`aarch64-linux-gnu-gcc`, NOT `arm64-linux-gnu-gcc`). Parse `"arm64-unknown-linux-gnu"` → canonicalizes → `gcc_cross_compiler_name` must emit `aarch64-linux-gnu-gcc`.

### Sibling pin for syslib
- [ ] `test_syslib_is_native_for_simulated_host_matrix` — parse each supported triple, build the matching `HostPlatform`, assert `target.is_native_for(host) = true`; swap host arch, assert `false`.

### Negative pin (type-level, compile-fail if reverted)
- [ ] `test_target_triple_components_has_no_raw_arch_string_field` — constructs a `TargetTripleComponents`, calls `components.arch.is_64_bit_non_wasm()` and matches on `Arch::X86_64`. Only compiles if `arch: Arch`; fails to compile if reverted to `arch: String` (`String` has no `is_64_bit_non_wasm()` method and cannot be pattern-matched on `Arch::X86_64`). Stronger than a runtime pin because the bug class is "raw string compare" — a runtime test cannot detect it after the fact.

### Latent bug #7 (from_triple asymmetry)
- [ ] `test_from_triple_accepts_arm64_apple_darwin_alias` — `TargetConfig::from_triple("arm64-apple-darwin")` returns Ok and `config.triple() == "aarch64-apple-darwin"` after canonicalization. This lives in `tests/aot/cross.rs` or `phases/codegen/targets.rs` (wherever from_triple is tested), not the new matrix file.

### Verify tests fail before fix
- [ ] All new tests fail (compile errors for the type-level negative pin; runtime assertion failures for the alias matrix) against current code — confirms they exercise the bug class.

---

## 3. Implementation

**Migration order: bottom-up.** Change `TargetTripleComponents.arch: String → Arch` FIRST. The Rust compiler then refuses to compile every consumer, forcing complete migration. Top-down would create transitional state where some consumers are typed and others are stringly.

- [ ] **Step 1 — Define the typed layer in `target_features.rs`:**
  ```rust
  /// Canonical CPU architecture. Parsed from target triples with all known
  /// alias spellings normalized at the boundary (arm64 → Aarch64, amd64 → X86_64,
  /// i486/i586/i686 → I386). SSOT for arch identity — never compared via raw strings.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum Arch {
      X86_64,
      I386,
      Aarch64,
      Wasm32,
      Wasm64,
  }

  impl Arch {
      pub fn parse_llvm_name(name: &str) -> Option<Self> {
          match name {
              "x86_64" | "amd64" => Some(Self::X86_64),
              "i386" | "i486" | "i586" | "i686" => Some(Self::I386),
              "aarch64" | "arm64" => Some(Self::Aarch64),
              "wasm32" => Some(Self::Wasm32),
              "wasm64" => Some(Self::Wasm64),
              _ => None,
          }
      }

      pub fn display_name(self) -> &'static str {
          match self {
              Self::X86_64 => "x86_64",
              Self::I386 => "i386",
              Self::Aarch64 => "aarch64",
              Self::Wasm32 => "wasm32",
              Self::Wasm64 => "wasm64",
          }
      }

      pub fn pointer_size_bytes(self) -> u32 {
          match self {
              Self::X86_64 | Self::Aarch64 | Self::Wasm64 => 8,
              Self::I386 | Self::Wasm32 => 4,
          }
      }

      pub fn is_64_bit_non_wasm(self) -> bool {
          matches!(self, Self::X86_64 | Self::Aarch64)
      }

      pub fn is_wasm(self) -> bool {
          matches!(self, Self::Wasm32 | Self::Wasm64)
      }
  }

  impl fmt::Display for Arch {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
          f.write_str(self.display_name())
      }
  }

  /// Host operating system family, determined at compile time via cfg.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum HostOs {
      Linux,
      Darwin,
      Windows,
      Unknown,
  }

  impl HostOs {
      pub const fn current() -> Self {
          #[cfg(target_os = "linux")]       { Self::Linux }
          #[cfg(target_os = "macos")]       { Self::Darwin }
          #[cfg(target_os = "windows")]     { Self::Windows }
          #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
          { Self::Unknown }
      }

      pub fn display_name(self) -> &'static str {
          match self {
              Self::Linux => "linux",
              Self::Darwin => "darwin",
              Self::Windows => "windows",
              Self::Unknown => "unknown",
          }
      }
  }

  /// Host platform (arch + OS) used for cross-compilation detection.
  /// All host-vs-target comparisons flow through this typed representation.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub struct HostPlatform {
      pub arch: Arch,
      pub os: HostOs,
  }

  impl HostPlatform {
      pub const fn new(arch: Arch, os: HostOs) -> Self {
          Self { arch, os }
      }

      pub const fn current() -> Self {
          Self::new(host_arch(), HostOs::current())
      }
  }

  const fn host_arch() -> Arch {
      #[cfg(target_arch = "x86_64")]   { Arch::X86_64 }
      #[cfg(target_arch = "aarch64")]  { Arch::Aarch64 }
      #[cfg(target_arch = "x86")]      { Arch::I386 }
      // Ori compiler only builds on x86_64/aarch64/x86; unreachable fallback.
      #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86")))]
      { Arch::X86_64 }
  }
  ```

- [ ] **Step 2 — Change `TargetTripleComponents.arch: String → Arch`** and update `parse()`/`is_wasm()`/`Display`/new `is_cross_for`/`is_native_for`:
  ```rust
  pub struct TargetTripleComponents {
      pub arch: Arch,
      pub vendor: String,
      pub os: String,
      pub env: Option<String>,
  }

  impl TargetTripleComponents {
      pub fn parse(triple: &str) -> Result<Self, TargetError> {
          let parts: Vec<&str> = triple.split('-').collect();
          if parts.len() < 3 {
              return Err(TargetError::InvalidTripleFormat {
                  triple: triple.to_string(),
                  reason: "expected at least 3 components: <arch>-<vendor>-<os>".to_string(),
              });
          }
          let arch = Arch::parse_llvm_name(parts[0]).ok_or_else(|| TargetError::InvalidTripleFormat {
              triple: triple.to_string(),
              reason: format!("unknown architecture '{}'", parts[0]),
          })?;
          Ok(Self {
              arch,
              vendor: parts[1].to_string(),
              os: parts[2].to_string(),
              env: parts.get(3).map(|s| (*s).to_string()),
          })
      }

      pub fn is_wasm(&self) -> bool { self.arch.is_wasm() }

      pub fn is_cross_for(&self, host: HostPlatform) -> bool {
          if self.arch != host.arch { return true; }
          // OS check — darwin carries a version suffix from LLVM default triples.
          let host_os = host.os.display_name();
          if self.os.starts_with("darwin") { return host_os != "darwin"; }
          self.os != host_os
      }

      pub fn is_native_for(&self, host: HostPlatform) -> bool {
          !self.is_cross_for(host)
      }
      // ...is_windows/is_msvc/is_macos/is_linux/family unchanged...
  }
  ```
  `Display` impl auto-emits canonical arch because `Arch: Display`.

- [ ] **Step 3 — Exhaustive match in `initialize_target_for_triple`:**
  ```rust
  pub(crate) fn initialize_target_for_triple(components: &TargetTripleComponents)
      -> Result<(), TargetError>
  {
      match components.arch {
          Arch::X86_64 | Arch::I386 => {
              X86_TARGET_INIT.call_once(|| Target::initialize_x86(&InitializationConfig::default()));
          }
          Arch::Aarch64 => {
              AARCH64_TARGET_INIT.call_once(|| Target::initialize_aarch64(&InitializationConfig::default()));
          }
          Arch::Wasm32 | Arch::Wasm64 => {
              WASM_TARGET_INIT.call_once(|| Target::initialize_webassembly(&InitializationConfig::default()));
          }
      }
      Ok(())
  }
  ```

- [ ] **Step 4 — `TargetConfig::from_triple` parses first, canonicalizes, then validates** (fixes latent #7):
  ```rust
  pub fn from_triple(triple: &str) -> Result<Self, TargetError> {
      let components = TargetTripleComponents::parse(triple)?;
      let canonical = components.to_string(); // Display uses canonical arch
      if !is_supported_target(&canonical) {
          return Err(TargetError::UnsupportedTarget {
              triple: triple.to_string(), // report user's input in the error
              supported: SUPPORTED_TARGETS.to_vec(),
          });
      }
      initialize_target_for_triple(&components)?;
      let reloc_mode = if components.is_linux() { RelocMode::PIC } else { RelocMode::Default };
      Ok(Self {
          triple: canonical,
          components,
          cpu: "generic".to_string(),
          features: String::new(),
          opt_level: OptimizationLevel::None,
          reloc_mode,
          code_model: CodeModel::Default,
      })
  }
  ```

- [ ] **Step 5 — `pointer_size()` exhaustive on `Arch`:**
  ```rust
  pub fn pointer_size(&self) -> u32 {
      self.components.arch.pointer_size_bytes()
  }
  ```

- [ ] **Step 6 — `linker/mod.rs::is_cross_compiling`:** delegate to typed query.
  ```rust
  pub fn is_cross_compiling(target: &TargetConfig) -> bool {
      target.components().is_cross_for(HostPlatform::current())
  }
  ```

- [ ] **Step 7 — `linker/mod.rs::gcc_cross_compiler_name`:** interpolate `target.arch` via its `Display` impl (canonical). Existing `format!("{}-w64-mingw32-gcc", target.arch)` now emits canonical because `Arch: Display → canonical`. No signature change.

- [ ] **Step 8 — `linker/mod.rs::cross_compilation_error`:** same interpolation rule — `components.arch` now emits canonical.

- [ ] **Step 9 — `syslib/mod.rs::is_native`:** delegate to typed query.
  ```rust
  pub fn is_native(&self) -> bool {
      self.target.is_native_for(HostPlatform::current())
  }
  ```

- [ ] **Step 10 — `syslib/mod.rs::detect_library_paths` lib64 check:**
  ```rust
  if target.arch.is_64_bit_non_wasm() {
      paths.push(sysroot.join("lib64"));
      paths.push(sysroot.join("usr/lib64"));
  }
  ```

- [ ] **Step 11 — Re-export from `aot/mod.rs`:** add `Arch`, `HostOs`, `HostPlatform` to the `pub use target_features::{...}` block.

- [ ] **Step 12 — Update all consumer tests** to construct struct literals with `arch: Arch::X86_64` / assert `components.arch == Arch::X86_64`:
  - `compiler/oric/tests/phases/codegen/targets.rs` (4 assert_eq updates + ~8 struct literal updates)
  - `compiler/ori_llvm/tests/aot/cross.rs:226`
  - `compiler/ori_llvm/src/aot/syslib/tests.rs:24`

- [ ] **Step 13 — Create `compiler/ori_llvm/src/aot/target_features/tests.rs`** with the 10-test matrix. Add `#[cfg(test)] mod tests;` declaration in `target_features.rs`.

- [ ] **Step 14 — Add `from_triple` alias acceptance test** (latent #7) to `compiler/oric/tests/phases/codegen/targets.rs` or `tests/aot/cross.rs`.

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix (no test modifications needed)
- [ ] Matrix completeness verified — every cell in Arch-alias × host-platform × target-os grid has a test
- [ ] Debug AND release builds pass (`cargo b && cargo b --release`)
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p ori_llvm` green
- [ ] `/commit-push` — commit all changes before review
- [ ] Bug entry in `plans/bug-tracker/section-04-codegen-llvm.md:297` updated: `- [x]` with resolution details
- [ ] Fix section frontmatter `status` updated to `complete`
- [ ] Bug-tracker `00-overview.md` Quick Reference open bug count updated
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues
- [ ] `/impl-hygiene-review` passed — MUST run AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` retrospective completed — MANDATORY after both reviews are clean

**Exit Criteria:** `cargo test -p ori_llvm --lib target_features::tests` runs the 10-test matrix and all assertions pass. `test_native_target_is_not_cross_compiling` passes on simulated Apple Silicon host (via `HostPlatform::new(Arch::Aarch64, HostOs::Darwin)` against parsed `arm64-apple-darwin25.2.0`). `TargetConfig::from_triple("arm64-apple-darwin")` returns Ok with `config.triple() == "aarch64-apple-darwin"`. Full `./test-all.sh` reports zero failures. The `TargetTripleComponents.arch` field type is `Arch` — reverting to `String` would fail to compile because `test_target_triple_components_has_no_raw_arch_string_field` calls `arch.is_64_bit_non_wasm()` which does not exist on `String`.
