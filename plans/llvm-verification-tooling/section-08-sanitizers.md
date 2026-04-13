---
section: "08"
title: "Sanitizer Integration"
status: not-started
reviewed: false
goal: "Integrate ASan and UBSan into AOT-compiled Ori binaries via Clang driver delegation, with ASan-instrumented ori_rt, gated by ORI_SANITIZE env var, with a smoke subset for PR CI and full sweep nightly — catching memory errors and undefined behavior in generated code AND runtime library code that verification gates and FileCheck tests cannot detect"
success_criteria:
  - "ORI_SANITIZE=address,undefined causes Ori compiler to delegate compilation through Clang with -fsanitize=address,undefined"
  - "ori_rt is recompiled with -Zsanitizer=address when ORI_SANITIZE includes address (nightly Rust required)"
  - "SanitizerMode field in OptimizationConfig controls which sanitizers are active"
  - "Linker invocation includes -fsanitize=address,undefined via the existing LinkInput.extra_args + GccLinker API"
  - "Smoke subset (<=20 test programs) runs in <=60s for PR CI in a dedicated workflow"
  - "Full spec test sweep runs nightly with sanitizers enabled in a NEW nightly-verification.yml"
  - "At least one test detects a memory error that would be silent without sanitizers (semantic pin)"
  - "At least one test confirms a clean program does NOT trigger false positives (negative pin)"
inspired_by:
  - "Rust -Zsanitizer=address — nightly-only sanitizer support, recompiles std with sanitizer"
  - "Zig's debug allocator — runtime memory error detection in test mode"
  - "Clang -fsanitize=address,undefined — Clang as compilation driver with sanitizer flags"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "08.0"
    title: "Prerequisite: Linker Module Split"
    status: not-started
  - id: "08.1"
    title: "SanitizerMode Type and Env Var Wiring"
    status: not-started
  - id: "08.2"
    title: "Clang-Delegated Sanitizer Pass Integration"
    status: not-started
  - id: "08.3"
    title: "Linker Integration via LinkInput and GccLinker"
    status: not-started
  - id: "08.4"
    title: "ori_rt ASan Instrumentation"
    status: not-started
  - id: "08.5"
    title: "Smoke Test Suite and CI Configuration"
    status: not-started
  - id: "08.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "08.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Sanitizer Integration

**Status:** Not Started
**Goal:** Integrate ASan (AddressSanitizer) and UBSan (UndefinedBehaviorSanitizer) into AOT-compiled Ori binaries. Sanitizers instrument GENERATED code (the AOT binaries Ori produces) AND the runtime library (`ori_rt`), not the compiler itself. This catches memory errors (use-after-free, buffer overflow, double-free, stack overflow) and undefined behavior (signed overflow, null pointer dereference, misaligned access) in both the code the compiler emits and the runtime library that manages RC operations, allocations, and container operations.

**Success Criteria:**

- [ ] `ORI_SANITIZE=address,undefined` activates sanitizer instrumentation on generated code — satisfies mission criterion: "Sanitizer integration"
- [ ] ori_rt is ASan-instrumented when `ORI_SANITIZE` includes `address` — satisfies mission criterion: "Sanitizer integration catches RC memory bugs"
- [ ] Linker invocation includes `-fsanitize=...` flags via `LinkInput` — satisfies mission criterion: "Sanitizer integration"
- [ ] `SanitizerMode` in `OptimizationConfig` — satisfies mission criterion: "Sanitizer integration"
- [ ] Smoke subset <=60s for PR CI in dedicated workflow — satisfies mission criterion: "Sanitizer integration"
- [ ] Full nightly sweep in new `nightly-verification.yml` — satisfies mission criterion: "Sanitizer integration"
- [ ] At least one semantic pin detects a memory error silent without sanitizers — satisfies mission criterion: "Sanitizer integration"
- [ ] At least one negative pin confirms clean code does not false-positive — satisfies mission criterion: "Sanitizer integration"

**Context:** The Ori compiler emits LLVM IR that is then compiled to native code. Memory bugs in the GENERATED code include: RC operations on freed memory, use-after-free when drop ordering is wrong, buffer overflows in list operations, undefined behavior in integer operations. These bugs are invisible to the Ori type system (which trusts the compiler), invisible to the AIMS verifiers (which check IR structure, not runtime behavior), and often invisible to behavioral tests (which may not exercise the failing path).

**CRITICAL: ori_rt coverage is essential.** Without ASan-instrumented `ori_rt`, most memory errors are invisible to the sanitizer. `ori_rt` contains `ori_rc_inc`/`ori_rc_dec` (RC ops), `ori_alloc`/`ori_free` (allocation), and all string/list/map/set buffer management. If only Ori-generated LLVM IR is sanitized but `ori_rt` is not, memory bugs in RC and container code remain silent. This is the PRIMARY goal of sanitizer integration — catching RC memory bugs — and it FAILS without instrumented `ori_rt`.

**CRITICAL: LLVM 21 C API has NO sanitizer pass configuration.** `llvm-sys = "211.0.1"` (LLVM 21) does not expose `LLVMPassBuilderOptionsSetSanitizeAddress()` or any sanitizer-specific function in the C API. Appending `asan-module` or `ubsan` to the pipeline string passed to `LLVMRunPasses` is undocumented and likely broken for LLVM 21. The validated approach is to use **Clang as the compilation driver**: emit LLVM IR or bitcode from Ori's codegen, then invoke `clang` with `-fsanitize=address,undefined` to compile it to a sanitized object file. This is how Rust (`-Zsanitizer=address`) works — it delegates to LLVM's pass builder via the C++ API, but since we only have the C API, Clang delegation is the correct portable approach.

**CI strategy (Codex+Gemini consensus):** Sanitizers significantly increase runtime (2-10x for ASan, 1.5-3x for UBSan). Running the full test suite with sanitizers would blow the 150-second timeout. Solution: separate CI job. Smoke subset on PRs (fast, catches obvious regressions), full sweep nightly (thorough, catches subtle issues). The smoke subset selects <=20 programs that exercise the highest-risk codegen paths: RC, COW, closures, iterators, collections. Nightly goes in a NEW `nightly-verification.yml` workflow (existing `nightly.yml` is release automation — dev-to-master PRs — and must not be modified).

**Reference implementations:**
- **Rust** `-Zsanitizer=address`: recompiles std with sanitizer flags (nightly only), delegates to LLVM via C++ `PassBuilder::registerPipelineStartEPCallback`. The C API does not expose this — Rust uses custom C++ shim code.
- **Clang** `-fsanitize=address,undefined`: Clang adds sanitizer passes via its own C++ `PassBuilder` integration. When used as a compiler driver on LLVM IR/bitcode, it adds the passes transparently.

**Depends on:** Section 01 (verification gates define what "failure" means — sanitizer errors are blocking under verification mode).

---

## 08.0 Prerequisite: Linker Module Split

**File(s):** `compiler/ori_llvm/src/aot/linker/mod.rs`

**Why:** `linker/mod.rs` is 648 lines — 148 over the 500-line limit. Adding sanitizer support to the linker without splitting first would push it further over. The `LinkerDetection` struct and its methods (~250 lines, starting around line 390) are an independent concern from the core linker types and `LinkerImpl` dispatch.

- [ ] Extract `LinkerDetection` and all its methods (`detect`, `detect_for_target`, `is_available`, `is_available_for_target`, `is_cross_compiling`, `gcc_cross_compiler_name`, `cross_compilation_error`, etc.) to a new `compiler/ori_llvm/src/aot/linker/detect.rs`
- [ ] Update `mod.rs` to re-export: `mod detect; pub use detect::LinkerDetection;`
- [ ] Verify `linker/mod.rs` is now under 500 lines
- [ ] Verify `linker/detect.rs` is under 500 lines
- [ ] `timeout 150 cargo test -p ori_llvm` green after split
- [ ] `timeout 150 ./clippy-all.sh` green after split

- [ ] **Subsection close-out (08.0)** — MANDATORY before starting 08.1:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`

---

## 08.1 SanitizerMode Type and Env Var Wiring

**File(s):** `compiler/ori_llvm/src/aot/passes/config.rs`, `compiler/oric/src/debug_flags.rs`, `compiler/oric/src/commands/build/mod.rs`

Add a `SanitizerMode` field to `OptimizationConfig` and wire it to the `ORI_SANITIZE` environment variable. The env var uses the existing `dbg_set!` pattern from `debug_flags.rs` but with a value-bearing variant (comma-separated list, not boolean).

**Design decision — env var format:** Use a single `ORI_SANITIZE` env var with comma-separated sanitizer names (e.g., `ORI_SANITIZE=address,undefined`). This matches Clang's `-fsanitize=address,undefined` pattern. The individual-boolean pattern (`ORI_SANITIZE_ADDRESS=1`, `ORI_SANITIZE_UNDEFINED=1`) would match the existing `debug_flags.rs` pattern but diverges from the Clang convention that users will already know. Consistency with Clang is more valuable here because the sanitizer flags are ultimately passed through to Clang.

- [ ] Define `SanitizerMode` in `config.rs`:
  ```rust
  /// Sanitizer instrumentation modes for generated code.
  ///
  /// Controls which sanitizer passes are applied (via Clang delegation)
  /// and which sanitizer runtime libraries are linked.
  #[derive(Debug, Clone, PartialEq, Eq, Default)]
  pub struct SanitizerMode {
      /// AddressSanitizer: use-after-free, buffer overflow, stack overflow.
      pub address: bool,
      /// UndefinedBehaviorSanitizer: signed overflow, null deref, misaligned access.
      pub undefined: bool,
  }

  impl SanitizerMode {
      /// No sanitizers (default).
      pub const NONE: Self = Self {
          address: false,
          undefined: false,
      };

      /// Parse from ORI_SANITIZE env var value.
      /// Format: comma-separated list of "address", "undefined".
      /// Example: "address,undefined" or "address" or "undefined".
      pub fn from_env_value(value: &str) -> Self {
          let mut mode = Self::NONE;
          for part in value.split(',') {
              match part.trim() {
                  "address" => mode.address = true,
                  "undefined" => mode.undefined = true,
                  other => {
                      tracing::warn!(
                          sanitizer = other,
                          "unknown sanitizer in ORI_SANITIZE, ignoring"
                      );
                  }
              }
          }
          mode
      }

      /// Whether any sanitizer is enabled.
      pub fn any_enabled(&self) -> bool {
          self.address || self.undefined
      }

      /// Return the Clang-compatible `-fsanitize=...` flag value.
      /// Returns `None` if no sanitizers are enabled.
      pub fn clang_flag_value(&self) -> Option<String> {
          let mut parts = Vec::new();
          if self.address {
              parts.push("address");
          }
          if self.undefined {
              parts.push("undefined");
          }
          if parts.is_empty() {
              None
          } else {
              Some(parts.join(","))
          }
      }
  }
  ```

- [ ] Add `sanitizer: SanitizerMode` field to `OptimizationConfig`:
  ```rust
  pub struct OptimizationConfig {
      // ... existing fields ...
      /// Sanitizer instrumentation for generated code.
      /// When enabled, the AOT pipeline delegates to Clang for sanitizer pass insertion.
      pub sanitizer: SanitizerMode,
  }
  ```
  Add a builder method: `.with_sanitizer(mode: SanitizerMode) -> Self`.
  Update `OptimizationConfig::new()` to initialize `sanitizer: SanitizerMode::NONE`.
  Update `Default for OptimizationConfig` to include `sanitizer: SanitizerMode::NONE`.

- [ ] Register `ORI_SANITIZE` in `debug_flags.rs`:
  ```rust
  /// Enable sanitizer instrumentation on generated AOT binaries.
  ///
  /// Value: comma-separated sanitizer names (address, undefined).
  /// Example: `ORI_SANITIZE=address,undefined ori build file.ori`
  ///
  /// Requires Clang on PATH (used as compilation driver for sanitizer passes).
  /// For full coverage, also recompiles ori_rt with sanitizer flags (nightly Rust).
  /// Significant performance impact (2-10x slower). Not for main test suite.
  ORI_SANITIZE
  ```

- [ ] Wire `ORI_SANITIZE` through `build_optimization_config` in `oric/src/commands/build/mod.rs`. **NOTE:** This is the SINGLE canonical location for reading `ORI_SANITIZE` — the function is called by both `single.rs` and `multi.rs`:
  ```rust
  fn build_optimization_config(options: &BuildOptions) -> ori_llvm::aot::OptimizationConfig {
      // ... existing level, lto, verify_each, lint_enabled code ...

      let sanitizer = std::env::var("ORI_SANITIZE")
          .ok()
          .filter(|v| v != "0")
          .map(|v| SanitizerMode::from_env_value(&v))
          .unwrap_or(SanitizerMode::NONE);

      OptimizationConfig::new(level)
          .with_lto(lto)
          .with_verify_each(verify_each)
          .with_lint(lint_enabled)
          .with_sanitizer(sanitizer)
  }
  ```

- [ ] Add tests in `config.rs`'s sibling `tests.rs` (following the existing pattern):
  - `sanitizer_mode_from_env_address_only` — `"address"` -> address=true, undefined=false
  - `sanitizer_mode_from_env_address_and_undefined` — `"address,undefined"` -> both true
  - `sanitizer_mode_from_env_undefined_only` — `"undefined"` -> address=false, undefined=true
  - `sanitizer_mode_from_env_empty_is_none` — `""` -> both false
  - `sanitizer_mode_from_env_whitespace_tolerant` — `" address , undefined "` -> both true
  - `sanitizer_mode_from_env_unknown_ignored` — `"address,foo"` -> address=true, undefined=false (warning logged)
  - `sanitizer_mode_clang_flag_value_both` — `.clang_flag_value()` returns `Some("address,undefined")`
  - `sanitizer_mode_clang_flag_value_none` — `NONE.clang_flag_value()` returns `None`
  - `sanitizer_mode_any_enabled_true` — address=true -> `.any_enabled()` true
  - `sanitizer_mode_any_enabled_false` — `NONE` -> `.any_enabled()` false
  - `optimization_config_default_has_no_sanitizer` — `OptimizationConfig::default().sanitizer == SanitizerMode::NONE`

- [ ] **Subsection close-out (08.1)** — MANDATORY before starting 08.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] `timeout 150 ./test-all.sh` green (no regressions)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 08.2 Clang-Delegated Sanitizer Pass Integration

**File(s):** `compiler/ori_llvm/src/aot/passes/mod.rs`, `compiler/ori_llvm/src/aot/passes/sanitizer.rs` (new)

**Why Clang delegation instead of pipeline string manipulation:** The LLVM 21 C API (`llvm-sys = "211.0.1"`) does not expose any sanitizer-related `PassBuilderOptions` functions. There is no `LLVMPassBuilderOptionsSetSanitizeAddress`, no `LLVMPassBuilderOptionsSetSanitizeUndefined`, and no documented way to add sanitizer passes via the `LLVMRunPasses` pipeline string. The LLVM C++ API supports sanitizers via `PassBuilder::registerPipelineStartEPCallback`, but this is not available through the C API.

**The validated approach:** When sanitizers are enabled, instead of passing the module through `LLVMRunPasses` for sanitizer instrumentation, we:
1. Run our normal optimization pipeline via `LLVMRunPasses` as usual (O0/O2/O3 etc.)
2. Emit the optimized LLVM IR to a temporary `.ll` file
3. Invoke `clang` on that `.ll` file with `-fsanitize=address,undefined -c -o output.o` to produce a sanitizer-instrumented object file
4. The Clang invocation adds the sanitizer passes transparently and links the sanitizer runtime stubs

This is the same strategy used by Rust's `-Zsanitizer` flag (which uses a C++ shim to access `PassBuilder` internals we cannot reach from the C API) — we just use Clang as the external driver instead of building our own C++ shim.

- [ ] Create `compiler/ori_llvm/src/aot/passes/sanitizer.rs` with a `clang_compile_with_sanitizers` function:
  ```rust
  //! Clang-delegated sanitizer pass integration.
  //!
  //! The LLVM C API (llvm-sys 211.0.1) does not expose sanitizer pass
  //! configuration. We delegate to Clang as a compilation driver, which
  //! adds sanitizer instrumentation transparently.

  use std::path::Path;
  use std::process::Command;

  use super::OptimizationError;
  use super::config::SanitizerMode;

  /// Compile LLVM IR to an object file with sanitizer instrumentation via Clang.
  ///
  /// # Arguments
  /// * `ir_path` - Path to the LLVM IR file (.ll)
  /// * `output_path` - Path for the output object file (.o)
  /// * `sanitizer` - Which sanitizers to enable
  /// * `opt_level` - Optimization level string (e.g., "-O2")
  ///
  /// # Errors
  /// Returns `PassesFailed` if Clang is not found or compilation fails.
  pub fn clang_compile_with_sanitizers(
      ir_path: &Path,
      output_path: &Path,
      sanitizer: &SanitizerMode,
      opt_level: &str,
  ) -> Result<(), OptimizationError> {
      let fsanitize_value = sanitizer.clang_flag_value()
          .expect("clang_compile_with_sanitizers called with no sanitizers enabled");

      let mut cmd = Command::new("clang");
      cmd.arg(ir_path)
          .arg("-c")
          .arg("-o").arg(output_path)
          .arg(format!("-fsanitize={fsanitize_value}"))
          .arg(opt_level);

      let output = cmd.output().map_err(|e| {
          if e.kind() == std::io::ErrorKind::NotFound {
              OptimizationError::PassesFailed {
                  message: "Clang not found on PATH. Clang is required for sanitizer \
                            instrumentation (ORI_SANITIZE). Install clang or disable \
                            sanitizers.".to_string(),
              }
          } else {
              OptimizationError::PassesFailed {
                  message: format!("failed to run clang for sanitizer instrumentation: {e}"),
              }
          }
      })?;

      if !output.status.success() {
          let stderr = String::from_utf8_lossy(&output.stderr);
          return Err(OptimizationError::PassesFailed {
              message: format!(
                  "clang sanitizer compilation failed (exit {}): {}",
                  output.status.code().unwrap_or(-1),
                  stderr,
              ),
          });
      }

      Ok(())
  }

  /// Check that Clang is available on PATH.
  ///
  /// Call this early when ORI_SANITIZE is set to fail fast with a clear error
  /// before doing expensive compilation work.
  pub fn check_clang_available() -> Result<(), OptimizationError> {
      match Command::new("clang").arg("--version").output() {
          Ok(output) if output.status.success() => Ok(()),
          Ok(_) => Err(OptimizationError::PassesFailed {
              message: "clang --version failed. Clang is required for sanitizer \
                        instrumentation (ORI_SANITIZE).".to_string(),
          }),
          Err(_) => Err(OptimizationError::PassesFailed {
              message: "Clang not found on PATH. Clang is required for sanitizer \
                        instrumentation (ORI_SANITIZE). Install clang or disable \
                        sanitizers.".to_string(),
          }),
      }
  }
  ```

- [ ] Add `mod sanitizer;` to `passes/mod.rs` and `pub use sanitizer::{clang_compile_with_sanitizers, check_clang_available};`

- [ ] Modify the AOT pipeline call sites (`single.rs` and `multi.rs` in `commands/build/`) to use the Clang delegation path when sanitizers are enabled:
  - After the normal LLVM optimization pipeline runs, if `config.sanitizer.any_enabled()`:
    1. Emit LLVM IR to a temp file via `module.print_to_file()`
    2. Call `clang_compile_with_sanitizers()` to produce the object file
    3. Skip the normal `module.write_to_file()` object emission (Clang already produced the .o)
  - If sanitizers are NOT enabled, the existing `optimize_module()` + `emit_object()` path is unchanged

- [ ] Add early Clang availability check: at the start of `build_file_single` / `build_file_multi`, if `opt_config.sanitizer.any_enabled()`, call `check_clang_available()` and fail fast with a clear error

- [ ] Verify that ASan instrumentation is visible in the emitted IR when `ORI_SANITIZE=address` is set. Use `ORI_DUMP_AFTER_LLVM=1 ORI_SANITIZE=address ori build test.ori` and check for ASan-related function calls (`__asan_load`, `__asan_store`, `__asan_report_*`) in the final binary (via `nm` or `objdump`)

- [ ] Add tests:
  - `clang_available_on_ci` — verify `check_clang_available()` succeeds in test environment
  - `sanitizer_mode_produces_clang_flag` — verify `SanitizerMode { address: true, undefined: true }.clang_flag_value()` == `Some("address,undefined")`
  - `clang_compile_sanitized_object_has_asan_symbols` — compile a trivial C program via `clang_compile_with_sanitizers`, verify the object contains `__asan_` symbols
  - `normal_pipeline_unchanged_when_sanitizers_disabled` — verify the optimization path is identical when `SanitizerMode::NONE`

- [ ] **TPR checkpoint** — `/tpr-review` covering 08.0-08.2 implementation work

- [ ] **Subsection close-out (08.2)** — MANDATORY before starting 08.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] `timeout 150 ./test-all.sh` green (sanitizers OFF)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 08.3 Linker Integration via LinkInput and GccLinker

**File(s):** `compiler/ori_llvm/src/aot/linker/mod.rs`, `compiler/ori_llvm/src/aot/linker/driver.rs`, `compiler/oric/src/commands/build/mod.rs`

When sanitizers are enabled, the linker must link the sanitizer runtime libraries. The correct approach is to pass `-fsanitize=address,undefined` to the GCC/Clang linker driver. This is done via `LinkInput.extra_args` (the existing mechanism for additional linker flags), NOT by modifying the `GccLinker` API signature.

**Actual API (verified):**
- `LinkInput` struct has `extra_args: Vec<String>` field
- `LinkerDriver::link(&self, input: &LinkInput)` is the entry point
- `LinkerDriver::configure_linker(linker: &mut LinkerImpl, input: &LinkInput)` iterates `input.extra_args` and calls `linker.add_arg(arg)` for each
- `GccLinker.add_arg(arg: &str)` appends to the `Command`
- `link_and_finish()` in `build/mod.rs` constructs `LinkInput` and passes it to `driver.link()`

**Design:** Add `sanitizer: SanitizerMode` as a typed field on `LinkInput` (not just string args) for type safety and discoverability. `LinkerDriver::configure_linker` reads it and adds the appropriate `-fsanitize=...` arg. This keeps the sanitizer intent typed rather than buried in opaque string args.

- [ ] Add `sanitizer: SanitizerMode` field to `LinkInput`:
  ```rust
  /// Input configuration for the linker.
  #[derive(Debug, Clone, Default)]
  pub struct LinkInput {
      // ... existing fields ...
      /// Sanitizer mode. When enabled, adds `-fsanitize=...` to the linker command.
      pub sanitizer: SanitizerMode,
  }
  ```
  Import `SanitizerMode` from `crate::aot::passes::config::SanitizerMode` (or re-export at the `aot` level).

- [ ] Modify `LinkerDriver::configure_linker()` in `driver.rs` to add sanitizer flags BEFORE extra_args (sanitizer flags must come before general args for some linkers):
  ```rust
  // Sanitizer runtime linkage
  if let Some(fsanitize) = input.sanitizer.clang_flag_value() {
      linker.add_arg(&format!("-fsanitize={fsanitize}"));
  }

  // Add extra arguments (existing code, unchanged)
  for arg in &input.extra_args {
      linker.add_arg(arg);
  }
  ```

- [ ] Thread `SanitizerMode` from `OptimizationConfig` to `LinkInput` in `link_and_finish()` (`build/mod.rs`). Add a `sanitizer` parameter to `link_and_finish`:
  ```rust
  fn link_and_finish(
      object_files: Vec<PathBuf>,
      output_path: &Path,
      target: &ori_llvm::aot::TargetConfig,
      options: &BuildOptions,
      sanitizer: &SanitizerMode,
      start: std::time::Instant,
  ) {
      // ...existing code...
      let mut link_input = LinkInput {
          objects: object_files,
          output: output_path.to_path_buf(),
          output_kind,
          sanitizer: sanitizer.clone(),
          // ... rest unchanged ...
      };
      // ...existing code...
  }
  ```
  Update all call sites in `single.rs` and `multi.rs` to pass the sanitizer from `opt_config.sanitizer`.

- [ ] When sanitizers are enabled and the linker is GCC-flavor on macOS: add a note that macOS requires Clang (not bare `ld`) and that `-fsanitize` is a Clang driver flag, not a raw linker flag. The existing `GccLinker` already uses `clang` on macOS, so this should work. Add a warning if MSVC or WASM linker is selected with sanitizers enabled (sanitizers are Linux/macOS only for now).

- [ ] Verify that the linked binary actually runs with sanitizer runtime. Smoke test:
  ```bash
  ORI_SANITIZE=address ori build tests/spec/basic/hello.ori -o /tmp/hello_asan
  /tmp/hello_asan  # Should print "hello" and exit 0
  ```

- [ ] Document the system requirements: ASan/UBSan runtime libraries must be installed. On Ubuntu/Debian: `sudo apt-get install libasan8 libubsan1`. On macOS: included with Xcode Command Line Tools. Add a clear error message if the linker fails due to missing sanitizer runtimes — detect the `cannot find -lasan` pattern in linker stderr and emit a diagnostic with install instructions.

- [ ] Add tests:
  - `link_input_default_has_no_sanitizer` — `LinkInput::default().sanitizer == SanitizerMode::NONE`
  - `configure_linker_adds_fsanitize_when_address_enabled` — mock/spy on the `LinkerImpl` args; verify `-fsanitize=address` is present
  - `configure_linker_adds_fsanitize_when_both_enabled` — verify `-fsanitize=address,undefined` is present
  - `configure_linker_no_fsanitize_when_disabled` — verify no `-fsanitize` arg
  - `sanitizer_flag_before_extra_args` — verify ordering: `-fsanitize=...` appears before any `extra_args` entries

- [ ] **Subsection close-out (08.3)** — MANDATORY before starting 08.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] `timeout 150 ./test-all.sh` green (sanitizers OFF)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 08.4 ori_rt ASan Instrumentation

**File(s):** `compiler/ori_rt/Cargo.toml`, `compiler/ori_rt/build.rs` (if needed), `compiler/oric/src/aot/runtime.rs` (or wherever runtime discovery lives), `scripts/build-rt-asan.sh` (new)

**Why this is mandatory:** `ori_rt` is a Rust crate compiled to a static library (`libori_rt.a`) that gets linked into every AOT binary. It contains:
- `ori_rc_inc` / `ori_rc_dec` — RC operations (the primary source of memory bugs)
- `ori_alloc` / `ori_free` — memory allocation/deallocation
- String, list, map, set buffer management code

If only Ori-generated LLVM IR is sanitized but `ori_rt` is compiled normally, then ASan cannot see memory operations inside `ori_rt`. A use-after-free inside `ori_rc_dec` would be invisible. The plan's PRIMARY goal ("catching RC memory bugs") FAILS without ASan-instrumented `ori_rt`.

**Approach:** When `ORI_SANITIZE` includes `address`, provide an ASan-instrumented variant of `libori_rt.a`. This requires nightly Rust (`-Zsanitizer=address` is unstable). Two options:

1. **Runtime recompilation (preferred):** `scripts/build-rt-asan.sh` recompiles `ori_rt` with `RUSTFLAGS="-Zsanitizer=address"` using nightly Rust, producing `libori_rt_asan.a` alongside `libori_rt.a`. The build command detects `ORI_SANITIZE` and links the asan variant instead.
2. **Pre-built variant in CI:** CI nightly job builds both variants and caches them.

- [ ] Create `scripts/build-rt-asan.sh`:
  ```bash
  #!/usr/bin/env bash
  # Build ori_rt with AddressSanitizer instrumentation.
  # Requires nightly Rust toolchain.
  #
  # Output: target/debug/libori_rt_asan.a (or target/release/libori_rt_asan.a)
  #
  # Usage: ./scripts/build-rt-asan.sh [--release]

  set -euo pipefail

  PROFILE="debug"
  PROFILE_DIR="debug"
  if [[ "${1:-}" == "--release" ]]; then
      PROFILE="release"
      PROFILE_DIR="release"
  fi

  # Check for nightly
  if ! rustup run nightly rustc --version &>/dev/null; then
      echo "ERROR: nightly Rust required for sanitizer-instrumented ori_rt"
      echo "Install with: rustup toolchain install nightly"
      exit 1
  fi

  echo "Building ori_rt with ASan instrumentation (nightly, $PROFILE)..."
  RUSTFLAGS="-Zsanitizer=address" \
      cargo +nightly build -p ori_rt \
      $([ "$PROFILE" = "release" ] && echo "--release") \
      --target-dir target/sanitizer

  # Copy the static library to a distinguishable name
  SRC="target/sanitizer/$PROFILE_DIR/libori_rt.a"
  DEST="target/$PROFILE_DIR/libori_rt_asan.a"

  if [ ! -f "$SRC" ]; then
      echo "ERROR: Expected $SRC not found"
      exit 1
  fi

  cp "$SRC" "$DEST"
  echo "ASan-instrumented runtime: $DEST"
  ```

- [ ] Modify the runtime discovery logic to look for `libori_rt_asan.a` when `ORI_SANITIZE` includes `address`. If the asan variant is not found, emit a clear warning:
  ```
  warning: ORI_SANITIZE=address is set but libori_rt_asan.a was not found.
  ori_rt will be linked WITHOUT sanitizer instrumentation.
  Memory bugs in RC operations and containers may not be detected.
  Run `./scripts/build-rt-asan.sh` to build the ASan-instrumented runtime.
  ```
  This is a WARNING, not an error — partial sanitizer coverage (generated code only) is still better than none.

- [ ] Add a `--sanitize-rt` flag to the build script or detect automatically: if `ORI_SANITIZE` includes `address` AND nightly Rust is available, automatically run the rt rebuild. If nightly is NOT available, warn and continue with uninstrumented rt.

- [ ] Add tests:
  - `build_rt_asan_script_produces_library` — run the script and verify the output file exists (requires nightly; `#[ignore]` if nightly not available, with plan item for CI enforcement)
  - `runtime_discovery_prefers_asan_variant_when_sanitize_set` — mock the discovery to verify preference logic
  - `runtime_discovery_warns_when_asan_variant_missing` — verify the warning message

- [ ] **Subsection close-out (08.4)** — MANDATORY before starting 08.5:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] `timeout 150 ./test-all.sh` green (sanitizers OFF)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 08.5 Smoke Test Suite and CI Configuration

**File(s):** `tests/sanitizer/` (new), `scripts/sanitizer-smoke.sh` (new), `.github/workflows/nightly-verification.yml` (new)

Create a curated smoke test subset for PR CI and configure full nightly runs. **Do NOT modify `.github/workflows/nightly.yml`** — it is release automation (dev-to-master PR creation).

### Smoke Test Programs

- [ ] Create `tests/sanitizer/` directory with <=20 Ori programs exercising highest-risk codegen paths. Selection criteria: each program must exercise at least one memory-management code path that sanitizers can detect bugs in.

  **Matrix coverage — program_type x memory_pattern:**

  | Program | Primary Risk | Memory Pattern |
  |---------|-------------|----------------|
  | `rc_basic.ori` | RC inc/dec balance | Allocation + use + drop |
  | `rc_loop.ori` | RC in loop back-edges | Repeated inc/dec in loop body |
  | `cow_mutation.ori` | COW uniqueness check | Shared -> unique -> mutate |
  | `closure_capture.ori` | Closure environment RC | Capture by value, env lifetime |
  | `iterator_for_loop.ori` | Iterator create/next/drop | Full iteration lifecycle |
  | `iterator_break.ori` | Early iterator termination | Partial iteration + cleanup |
  | `list_operations.ori` | List buffer management | Push, pop, index, slice |
  | `map_operations.ori` | Map hash table | Insert, lookup, remove |
  | `set_operations.ori` | Set operations | Insert, contains, remove |
  | `string_concat.ori` | String RC and buffer | Concat, interpolation, slice |
  | `nested_struct_drop.ori` | Nested struct drops | Deep struct with RC fields |
  | `enum_variant_drop.ori` | Sum type variant drops | Variant with RC payload |
  | `option_some_none.ori` | Option RC handling | Some(rc_value), None paths |
  | `result_ok_err.ori` | Result RC handling | Ok(rc_value), Err paths |
  | `recursive_data.ori` | Recursive structure RC | Linked list or tree |

  Each program must:
  - Have at least one `assert_eq` to verify correctness (not just "doesn't crash")
  - Exercise a distinct memory pattern (no two programs testing the same thing)
  - Complete in <4 seconds with sanitizers enabled (60s budget / 15 programs)

- [ ] **Semantic pin test** — at least one program must detect a memory error WITH sanitizers that is silent WITHOUT:
  Create `tests/sanitizer/semantic_pin_asan.ori` — a program that deliberately triggers a detectable memory pattern (e.g., accessing a buffer after it should be freed, or a known edge case in RC drop ordering). This program should:
  - Exit cleanly (0) when compiled WITHOUT sanitizers
  - Exit with ASan error when compiled WITH sanitizers
  - Include a comment explaining what memory error it detects and why

  If no existing codegen bug creates such a scenario, construct one using `unsafe` FFI or by testing a known-fragile pattern (e.g., double-drop through aliased RC, use-after-cow-mutation).

- [ ] **Negative pin test** — at least one program must confirm sanitizers do NOT false-positive:
  `tests/sanitizer/negative_pin_clean.ori` — a complex but memory-correct program (nested structs, closures, iterators, COW) that must pass cleanly with ALL sanitizers enabled.

### Smoke Script

- [ ] Create `scripts/sanitizer-smoke.sh`:
  ```bash
  #!/usr/bin/env bash
  # Run sanitizer smoke tests. Exit non-zero on any sanitizer error.
  # Usage: ORI_SANITIZE=address,undefined ./scripts/sanitizer-smoke.sh
  #
  # Expected runtime: <=60s (within 150s timeout with margin).

  set -euo pipefail

  SANITIZE="${ORI_SANITIZE:-address,undefined}"
  SMOKE_DIR="tests/sanitizer"
  FAIL_COUNT=0
  PASS_COUNT=0
  SKIP_COUNT=0

  if [ ! -d "$SMOKE_DIR" ]; then
      echo "ERROR: $SMOKE_DIR not found"
      exit 1
  fi

  # Check Clang availability
  if ! command -v clang &>/dev/null; then
      echo "ERROR: Clang not found on PATH (required for sanitizer compilation)"
      exit 1
  fi

  export ORI_SANITIZE="$SANITIZE"

  for ori_file in "$SMOKE_DIR"/*.ori; do
      [ -f "$ori_file" ] || continue
      name=$(basename "$ori_file" .ori)
      echo -n "  $name ... "

      TMPDIR=$(mktemp -d)
      trap "rm -rf $TMPDIR" EXIT

      # Compile with sanitizers
      if ! cargo run -p oric --bin ori -- build "$ori_file" -o "$TMPDIR/san_$name" 2>"$TMPDIR/compile.log"; then
          echo "FAIL (compilation)"
          cat "$TMPDIR/compile.log" >&2
          FAIL_COUNT=$((FAIL_COUNT + 1))
          continue
      fi

      # Run the sanitized binary
      if ! "$TMPDIR/san_$name" 2>"$TMPDIR/run.log"; then
          echo "FAIL (runtime/sanitizer)"
          cat "$TMPDIR/run.log" >&2
          FAIL_COUNT=$((FAIL_COUNT + 1))
          continue
      fi

      echo "PASS"
      PASS_COUNT=$((PASS_COUNT + 1))
  done

  echo ""
  echo "=== Sanitizer smoke: $PASS_COUNT passed, $FAIL_COUNT failed ==="

  if [ "$FAIL_COUNT" -gt 0 ]; then
      echo "ERROR: $FAIL_COUNT sanitizer smoke test(s) FAILED"
      echo "If failures are pre-existing memory bugs in generated code, file via /add-bug."
      exit 1
  fi
  ```

- [ ] The 150-second timeout constraint applies to all tests. The smoke suite must complete within ~60s to leave margin. If it exceeds this:
  - Profile to find slow programs
  - Reduce the smoke set to the 10 most important programs (prioritize RC and container tests)
  - Do NOT raise the timeout

### CI Workflow

- [ ] Create `.github/workflows/nightly-verification.yml` — a NEW workflow for sanitizer full sweep and other verification jobs. **Do NOT modify `nightly.yml`** (release automation):
  ```yaml
  name: Nightly Verification

  on:
    schedule:
      - cron: '30 2 * * *'  # 2:30 AM UTC (after nightly release PR at midnight)
    workflow_dispatch:

  jobs:
    sanitizer-smoke:
      name: Sanitizer Smoke
      runs-on: ubuntu-latest
      timeout-minutes: 10
      steps:
        - uses: actions/checkout@v4
        - name: Install dependencies
          run: |
            sudo apt-get update
            sudo apt-get install -y libasan8 libubsan1 clang
        - name: Install Rust nightly (for ori_rt ASan)
          run: rustup toolchain install nightly
        - name: Build compiler
          run: cargo build --release
        - name: Build ASan-instrumented ori_rt
          run: ./scripts/build-rt-asan.sh --release
        - name: Sanitizer smoke
          env:
            ORI_SANITIZE: "address,undefined"
          run: timeout 150 ./scripts/sanitizer-smoke.sh

    sanitizer-full:
      name: Sanitizer Full Sweep
      runs-on: ubuntu-latest
      timeout-minutes: 30
      needs: sanitizer-smoke  # Only run full sweep if smoke passes
      strategy:
        matrix:
          shard: [1, 2, 3, 4]
      steps:
        - uses: actions/checkout@v4
        - name: Install dependencies
          run: |
            sudo apt-get update
            sudo apt-get install -y libasan8 libubsan1 clang
        - name: Install Rust nightly
          run: rustup toolchain install nightly
        - name: Build compiler
          run: cargo build --release
        - name: Build ASan-instrumented ori_rt
          run: ./scripts/build-rt-asan.sh --release
        - name: Run shard
          env:
            ORI_SANITIZE: "address,undefined"
            TEST_SHARD: "${{ matrix.shard }}"
            TEST_TOTAL_SHARDS: "4"
          run: ./scripts/sanitizer-full.sh
  ```

- [ ] Create `scripts/sanitizer-full.sh` — runs the full spec test suite with sanitizers, with shard support:
  ```bash
  #!/usr/bin/env bash
  # Run the full spec test suite with sanitizers enabled (sharded).
  # Expected to be called from CI with TEST_SHARD and TEST_TOTAL_SHARDS env vars.
  set -euo pipefail

  SHARD="${TEST_SHARD:-1}"
  TOTAL="${TEST_TOTAL_SHARDS:-1}"

  echo "=== Sanitizer full sweep: shard $SHARD of $TOTAL ==="

  # Collect all spec test files
  mapfile -t ALL_TESTS < <(find tests/spec -name '*.ori' | sort)
  TOTAL_TESTS=${#ALL_TESTS[@]}

  # Calculate shard boundaries
  PER_SHARD=$(( (TOTAL_TESTS + TOTAL - 1) / TOTAL ))
  START=$(( (SHARD - 1) * PER_SHARD ))
  END=$(( START + PER_SHARD ))
  [ "$END" -gt "$TOTAL_TESTS" ] && END="$TOTAL_TESTS"

  echo "Running tests $START to $END of $TOTAL_TESTS"

  FAIL_COUNT=0
  for (( i=START; i<END; i++ )); do
      test_file="${ALL_TESTS[$i]}"
      name=$(basename "$test_file" .ori)

      if ! cargo run -p oric --bin ori -- build "$test_file" -o "/tmp/san_full_$name" 2>/dev/null; then
          # Compilation failure is expected for compile_fail tests — skip
          continue
      fi

      if ! "/tmp/san_full_$name" 2>/dev/null; then
          echo "FAIL: $test_file"
          FAIL_COUNT=$((FAIL_COUNT + 1))
      fi
  done

  if [ "$FAIL_COUNT" -gt 0 ]; then
      echo "=== $FAIL_COUNT sanitizer failure(s) in shard $SHARD ==="
      exit 1
  fi

  echo "=== Shard $SHARD complete: all passed ==="
  ```

- [ ] **Shard timing validation:** After initial implementation, run one shard locally to measure timing. If any shard exceeds 10 minutes, increase shard count. Do NOT use `timeout 150` for the full sweep — it runs in CI with a 30-minute job timeout, not the local test timeout.

- [ ] **Matrix testing requirement** — the smoke suite covers this matrix:
  | Sanitizer | Opt Level | Program Type |
  |-----------|-----------|--------------|
  | ASan | O0 (debug) | RC basic, RC loop, COW, closures, iterators, collections, nested structs, enums, option, result |
  | ASan+UBSan | O2 (release) | Same programs |

  Run the smoke suite at both O0 and O2 to catch optimization-level-dependent sanitizer issues. The CI workflow should run with `--release` (O2); local development uses debug (O0).

- [ ] Add tests:
  - `sanitizer_smoke_script_exits_zero_on_clean_programs` — run the smoke script on a trivially-correct program set and verify exit 0
  - If any smoke test fails, that is a pre-existing memory bug in the generated code — file via `/add-bug` immediately. Do NOT mark the smoke test as "expected failure."

- [ ] **Subsection close-out (08.5)** — MANDATORY before starting 08.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] `timeout 150 ./test-all.sh` green (sanitizers OFF — no regressions to normal builds)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 08.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- all items in this block are marked resolved
- `third_party_review.status` becomes `resolved` or `none`
-->

- None.

---

## 08.N Completion Checklist

**Prerequisite:**
- [ ] `linker/mod.rs` split: `LinkerDetection` extracted to `linker/detect.rs`, `mod.rs` under 500 lines

**SanitizerMode type:**
- [ ] `SanitizerMode` type defined in `config.rs` with `address` and `undefined` fields
- [ ] `SanitizerMode::from_env_value()` parses comma-separated sanitizer names
- [ ] `SanitizerMode::clang_flag_value()` produces Clang-compatible flag string
- [ ] `OptimizationConfig` has `sanitizer: SanitizerMode` field with builder method
- [ ] `ORI_SANITIZE` registered in `debug_flags.rs` with documentation

**Env var wiring:**
- [ ] `ORI_SANITIZE` wired through `build_optimization_config()` (single canonical location)
- [ ] Both `single.rs` and `multi.rs` get sanitizer mode through `build_optimization_config()` (no duplication)

**Clang delegation:**
- [ ] `passes/sanitizer.rs` implements `clang_compile_with_sanitizers()`
- [ ] `check_clang_available()` fails fast when Clang is missing
- [ ] AOT pipeline delegates to Clang when sanitizers enabled (emit .ll, clang -fsanitize, produce .o)
- [ ] Normal optimization pipeline unchanged when sanitizers disabled

**Linker integration:**
- [ ] `LinkInput` has typed `sanitizer: SanitizerMode` field
- [ ] `LinkerDriver::configure_linker()` adds `-fsanitize=...` when sanitizers enabled
- [ ] Sanitized binary runs correctly for simple programs
- [ ] Clear error message when sanitizer runtime libraries are missing (detects `cannot find -lasan` pattern)

**ori_rt instrumentation:**
- [ ] `scripts/build-rt-asan.sh` produces `libori_rt_asan.a` with nightly Rust
- [ ] Runtime discovery prefers `libori_rt_asan.a` when `ORI_SANITIZE` includes `address`
- [ ] Clear warning when asan variant is missing (partial coverage still works)

**Smoke tests:**
- [ ] `tests/sanitizer/` contains <=20 curated smoke test programs
- [ ] Every smoke program has at least one `assert_eq` (no assertion-free programs)
- [ ] Semantic pin: at least one test detects a memory error silent without sanitizers
- [ ] Negative pin: at least one test confirms clean code does not false-positive with sanitizers
- [ ] `scripts/sanitizer-smoke.sh` runs smoke suite and reports pass/fail
- [ ] Smoke suite completes within 60 seconds

**CI:**
- [ ] `.github/workflows/nightly-verification.yml` created (NEW file, NOT modifying `nightly.yml`)
- [ ] Nightly verification runs sanitizer-smoke then sanitizer-full (4 shards)
- [ ] `nightly.yml` is UNCHANGED (release automation only)

**Standard gates:**
- [ ] No regressions: `timeout 150 ./test-all.sh` green (sanitizers OFF)
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 08` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` -> `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference updated
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** `ORI_SANITIZE=address,undefined ori build file.ori` compiles with Clang-delegated sanitizer instrumentation. The generated binary runs with ASan/UBSan runtime checking active. When `libori_rt_asan.a` is available, the runtime library is also sanitized — providing full coverage of both generated code and RC/container operations. `scripts/sanitizer-smoke.sh` completes within 60 seconds and passes all <=20 smoke tests including at least one semantic pin and one negative pin. `.github/workflows/nightly-verification.yml` runs the full spec test suite with sanitizers enabled (sharded). `timeout 150 ./test-all.sh` (without sanitizers) passes with 0 regressions.
