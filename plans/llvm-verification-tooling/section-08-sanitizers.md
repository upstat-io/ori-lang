---
section: "08"
title: "Sanitizer Integration"
status: not-started
reviewed: false
goal: "Integrate ASan and UBSan into the LLVM pass pipeline for AOT-compiled Ori binaries, gated by ORI_SANITIZE env var, with a smoke subset for PR CI and full sweep nightly — catching memory errors and undefined behavior in generated code that verification gates and FileCheck tests cannot detect"
success_criteria:
  - "ORI_SANITIZE=address,undefined adds ASan/UBSan instrumentation passes to LLVM pipeline"
  - "Linker invocation includes -fsanitize=address,undefined when ORI_SANITIZE is set"
  - "SanitizerMode field in OptimizationConfig controls which sanitizers are active"
  - "Smoke subset (≤20 test programs) runs in ≤60s for PR CI"
  - "Full spec test sweep runs nightly with sanitizers enabled"
  - "At least one test detects a memory error that would be silent without sanitizers"
inspired_by:
  - "Rust miri — sanitizer-style runtime verification of compiled code"
  - "Zig's debug allocator — runtime memory error detection in test mode"
  - "Clang -fsanitize=address,undefined — ASan/UBSan pass pipeline integration"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "08.1"
    title: "SanitizerMode in OptimizationConfig"
    status: not-started
  - id: "08.2"
    title: "LLVM Pass Pipeline Integration"
    status: not-started
  - id: "08.3"
    title: "Linker Integration"
    status: not-started
  - id: "08.4"
    title: "Smoke Test Suite and Nightly Configuration"
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
**Goal:** Integrate ASan (AddressSanitizer) and UBSan (UndefinedBehaviorSanitizer) into the LLVM pass pipeline for AOT-compiled Ori binaries. Sanitizers instrument GENERATED code (the AOT binaries Ori produces), not the compiler itself. This catches memory errors (use-after-free, buffer overflow, double-free, stack overflow) and undefined behavior (signed overflow, null pointer dereference, misaligned access) in the code the compiler emits — bugs that static verification (FileCheck, lattice properties) cannot detect because they manifest only at runtime.

**Success Criteria:**

- [ ] `ORI_SANITIZE=address,undefined` adds sanitizer passes to LLVM pipeline — satisfies mission criterion: "Sanitizer integration"
- [ ] Linker includes `-fsanitize=...` flags — satisfies mission criterion: "Sanitizer integration"
- [ ] `SanitizerMode` in `OptimizationConfig` — satisfies mission criterion: "Sanitizer integration"
- [ ] Smoke subset ≤60s for PR CI — satisfies mission criterion: "Sanitizer integration"
- [ ] Full nightly sweep — satisfies mission criterion: "Sanitizer integration"

**Context:** The Ori compiler emits LLVM IR that is then compiled to native code. Memory bugs in the GENERATED code (not the compiler itself) include: RC operations on freed memory, use-after-free when drop ordering is wrong, buffer overflows in list operations, undefined behavior in integer operations. These bugs are invisible to the Ori type system (which trusts the compiler), invisible to the AIMS verifiers (which check IR structure, not runtime behavior), and often invisible to behavioral tests (which may not exercise the failing path). Sanitizers are the industry standard for catching these bugs — every major compiler (Rust, C/C++, Go) uses them on generated code.

**CI strategy (Codex+Gemini consensus):** Sanitizers significantly increase runtime (2-10x for ASan, 1.5-3x for UBSan). Running the full test suite with sanitizers would blow the 150-second timeout. Solution: separate CI job. Smoke subset on PRs (fast, catches obvious regressions), full sweep nightly (thorough, catches subtle issues). The smoke subset selects ≤20 programs that exercise the highest-risk codegen paths: RC, COW, closures, iterators, collections.

**Reference implementations:**
- **Clang** `-fsanitize=address,undefined`: adds sanitizer passes to the LLVM pipeline and links the sanitizer runtime library. The pass pipeline string includes `asan-module` for ASan.
- **Rust** `rustc -Z sanitizer=address`: similar integration — adds sanitizer passes to the LLVM pipeline via the pass builder options.

**Depends on:** Section 01 (verification gates define what "failure" means — sanitizer errors are blocking under verification mode).

---

## 08.1 SanitizerMode in OptimizationConfig

**File(s):** `compiler/ori_llvm/src/aot/passes/config.rs`, `compiler/oric/src/debug_flags.rs`, `compiler/oric/src/commands/build/mod.rs`

Add a `SanitizerMode` field to `OptimizationConfig` and wire it to the `ORI_SANITIZE` environment variable.

- [ ] Define `SanitizerMode` in `config.rs`:
  ```rust
  /// Sanitizer instrumentation modes for generated code.
  ///
  /// Controls which LLVM sanitizer passes are added to the optimization
  /// pipeline and which sanitizer runtime libraries are linked.
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
  }
  ```

- [ ] Add `sanitizer: SanitizerMode` field to `OptimizationConfig`:
  ```rust
  pub struct OptimizationConfig {
      // ... existing fields ...
      /// Sanitizer instrumentation for generated code.
      pub sanitizer: SanitizerMode,
  }
  ```
  Add a builder method: `.with_sanitizer(mode: SanitizerMode) -> Self`.

- [ ] Register `ORI_SANITIZE` in `debug_flags.rs`:
  ```rust
  /// Enable sanitizer instrumentation on generated AOT binaries.
  ///
  /// Value: comma-separated sanitizer names (address, undefined).
  /// Example: `ORI_SANITIZE=address,undefined ori build file.ori`
  ///
  /// Significant performance impact (2-10x slower). Not for main test suite.
  ORI_SANITIZE
  ```

- [ ] Wire `ORI_SANITIZE` through `build_optimization_config` in `oric/src/commands/build/mod.rs`:
  ```rust
  let sanitizer = std::env::var("ORI_SANITIZE")
      .ok()
      .map(|v| SanitizerMode::from_env_value(&v))
      .unwrap_or(SanitizerMode::NONE);
  let opt_config = OptimizationConfig::release()
      .with_sanitizer(sanitizer);
  ```

- [ ] Add tests:
  - `test_sanitizer_mode_parse_address_only`
  - `test_sanitizer_mode_parse_address_and_undefined`
  - `test_sanitizer_mode_parse_empty_is_none`
  - `test_sanitizer_mode_parse_unknown_warns`

- [ ] **Subsection close-out (08.1)** — MANDATORY before starting 08.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging journey for 08.1 specifically: which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where test failures gave unhelpful messages. Implement every accepted improvement NOW and commit each via SEPARATE `/commit-push` using a valid conventional-commit type.

---

## 08.2 LLVM Pass Pipeline Integration

**File(s):** `compiler/ori_llvm/src/aot/passes/mod.rs`

Add sanitizer passes to the LLVM optimization pipeline when `SanitizerMode` has any sanitizer enabled. The LLVM new pass manager supports sanitizer passes via the pipeline string.

- [ ] Modify `run_optimization_passes()` (or the pipeline string construction) to append sanitizer passes:
  ```rust
  fn build_pipeline_string(config: &OptimizationConfig) -> String {
      let mut pipeline = config.level.pipeline_string().to_string();

      // Sanitizer instrumentation passes
      if config.sanitizer.address {
          // ASan module pass: instruments memory accesses
          pipeline.push_str(",asan-module");
      }
      if config.sanitizer.undefined {
          // UBSan: instruments undefined behavior patterns
          pipeline.push_str(",ubsan");
      }

      // Existing lint pass integration from Section 01
      if config.lint_enabled {
          pipeline.push_str(",lint");
      }

      pipeline
  }
  ```

- [ ] Investigate the exact pass names for LLVM 21's new pass manager. The pass names may differ from the examples above. Check LLVM documentation and the `llvm-sys` bindings:
  - ASan: may be `asan-module` or `asan` or require separate function/module passes
  - UBSan: may be `ubsan` or require individual check passes
  - Some sanitizers require pass builder options (`PassBuilderOptions`) rather than pipeline string entries — investigate and use the correct approach

- [ ] Alternative approach if pipeline string doesn't support sanitizer passes directly: use `LLVMPassBuilderOptionsSetSanitizers()` or equivalent C API. LLVM's `PassBuilder` has sanitizer configuration that may need to be set before building the pipeline:
  ```rust
  // If pass builder options support sanitizers:
  if config.sanitizer.address {
      unsafe {
          LLVMPassBuilderOptionsSetSanitizeAddress(pass_opts, true as _);
      }
  }
  ```

- [ ] Verify that ASan instrumentation is visible in the emitted IR when `ORI_SANITIZE=address` is set. Use `ORI_DUMP_AFTER_LLVM=1 ORI_SANITIZE=address ori build test.ori` and check for ASan-related function calls (`__asan_load`, `__asan_store`, `__asan_report_*`).

- [ ] Add tests:
  - `test_pipeline_string_includes_asan_when_enabled`
  - `test_pipeline_string_includes_ubsan_when_enabled`
  - `test_pipeline_string_no_sanitizer_when_disabled`
  - `test_asan_instrumentation_visible_in_emitted_ir`

- [ ] **TPR checkpoint** — `/tpr-review` covering 08.1–08.2 implementation work

- [ ] **Subsection close-out (08.2)** — MANDATORY before starting 08.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 08.1's close-out, scoped to 08.2's debugging journey.

---

## 08.3 Linker Integration

**File(s):** `compiler/ori_llvm/src/aot/linker/gcc.rs`, `compiler/ori_llvm/src/aot/linker/mod.rs`

When sanitizers are enabled, the linker must link the sanitizer runtime libraries. On Linux (the primary target), this means passing `-fsanitize=address,undefined` to the GCC/Clang linker driver.

- [ ] Modify `GccLinker` to accept sanitizer configuration and emit the appropriate flags:
  ```rust
  impl GccLinker {
      pub fn link(
          &self,
          object_path: &Path,
          output_path: &Path,
          sanitizer: &SanitizerMode,
          // ... other params ...
      ) -> Result<(), LinkerError> {
          let mut cmd = Command::new(&self.cc_path);
          cmd.arg(object_path);
          cmd.arg("-o").arg(output_path);

          // Sanitizer runtime linkage
          if sanitizer.any_enabled() {
              let mut sanitize_flags = Vec::new();
              if sanitizer.address {
                  sanitize_flags.push("address");
              }
              if sanitizer.undefined {
                  sanitize_flags.push("undefined");
              }
              cmd.arg(format!("-fsanitize={}", sanitize_flags.join(",")));
          }

          // ... existing flags ...
      }
  }
  ```

- [ ] Thread `SanitizerMode` through the linker call chain. The `link()` function is called from `compile_and_link()` in the AOT pipeline — ensure the `OptimizationConfig`'s sanitizer mode is passed down.

- [ ] Verify that the linked binary actually runs with sanitizer runtime. A simple smoke test:
  ```bash
  ORI_SANITIZE=address ori build tests/spec/basic/hello.ori -o /tmp/hello_asan
  /tmp/hello_asan  # Should print "hello" and exit 0
  ```

- [ ] Document the system requirements: ASan/UBSan runtime libraries must be installed. On Ubuntu/Debian: `libasan`, `libubsan`. On macOS: included with Xcode. Add a clear error message if the linker fails due to missing sanitizer runtimes.

- [ ] Add tests:
  - `test_linker_emits_fsanitize_when_address_enabled`
  - `test_linker_emits_fsanitize_when_both_enabled`
  - `test_linker_no_fsanitize_when_disabled`

- [ ] **Subsection close-out (08.3)** — MANDATORY before starting 08.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 08.4 Smoke Test Suite and Nightly Configuration

**File(s):** `tests/sanitizer/` (new), `scripts/sanitizer-smoke.sh` (new), `.github/workflows/nightly.yml`

Create a curated smoke test subset for PR CI and configure full nightly runs.

- [ ] Create `tests/sanitizer/` directory with a curated set of Ori programs (≤20) that exercise the highest-risk codegen paths:
  ```
  tests/sanitizer/
    rc_basic.ori              # Basic RC inc/dec
    rc_loop.ori               # RC in loops
    cow_mutation.ori           # COW copy-on-write
    closure_capture.ori        # Closure environment RC
    iterator_for_loop.ori      # Iterator create/next/drop
    iterator_break.ori         # Early iterator termination
    collections_list.ori       # List operations
    collections_map.ori        # Map operations
    collections_set.ori        # Set operations
    string_concat.ori          # String RC
    nested_struct.ori          # Nested struct drops
    enum_variant.ori           # Enum variant drops
    option_some_none.ori       # Option RC
    result_ok_err.ori          # Result RC
    recursive_struct.ori       # Recursive data structure RC
    README.md                  # Explains the smoke subset selection criteria
  ```

- [ ] Create `scripts/sanitizer-smoke.sh`:
  ```bash
  #!/usr/bin/env bash
  # Run sanitizer smoke tests. Exit non-zero on any sanitizer error.
  # Usage: ORI_SANITIZE=address,undefined ./scripts/sanitizer-smoke.sh
  #
  # Expected runtime: ≤60s (within 150s timeout with margin).

  set -euo pipefail

  SANITIZE="${ORI_SANITIZE:-address,undefined}"
  SMOKE_DIR="tests/sanitizer"
  FAIL_COUNT=0

  export ORI_SANITIZE="$SANITIZE"

  for ori_file in "$SMOKE_DIR"/*.ori; do
      name=$(basename "$ori_file" .ori)
      echo "=== Sanitizer smoke: $name ==="

      # Compile with sanitizers
      if ! cargo run -p oric --bin ori -- build "$ori_file" -o "/tmp/ori_san_$name" 2>&1; then
          echo "FAIL: compilation failed for $name"
          FAIL_COUNT=$((FAIL_COUNT + 1))
          continue
      fi

      # Run the sanitized binary
      if ! "/tmp/ori_san_$name" 2>&1; then
          echo "FAIL: sanitizer error in $name"
          FAIL_COUNT=$((FAIL_COUNT + 1))
          continue
      fi

      echo "PASS: $name"
  done

  if [ "$FAIL_COUNT" -gt 0 ]; then
      echo "=== $FAIL_COUNT sanitizer smoke test(s) FAILED ==="
      exit 1
  fi

  echo "=== All sanitizer smoke tests PASSED ==="
  ```

- [ ] The 150-second timeout constraint applies to all tests. The smoke suite must complete within ~60s to leave margin. If it exceeds this:
  - Profile to find slow programs
  - Reduce the smoke set to the 10 most important programs
  - Do NOT raise the timeout

- [ ] Add the sanitizer smoke job to `.github/workflows/ci.yml` for PRs (if runtime permits) or to a separate nightly workflow. The Codex+Gemini consensus was: separate CI job, not the main test suite:
  ```yaml
  # In .github/workflows/nightly.yml (or ci.yml with separate job):
  sanitizer-smoke:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install sanitizer runtimes
        run: sudo apt-get install -y libasan8 libubsan1
      - name: Build
        run: cargo build --release
      - name: Sanitizer smoke
        env:
          ORI_SANITIZE: "address,undefined"
        run: timeout 150 ./scripts/sanitizer-smoke.sh
  ```

- [ ] Configure the nightly workflow to run the full spec test suite with sanitizers enabled (sharded if needed for the 150s timeout):
  ```yaml
  sanitizer-full:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        shard: [1, 2, 3, 4]
    steps:
      - name: Run shard
        env:
          ORI_SANITIZE: "address,undefined"
          TEST_SHARD: "${{ matrix.shard }}"
          TEST_TOTAL_SHARDS: "4"
        run: timeout 150 ./scripts/sanitizer-full.sh
  ```

- [ ] Add tests:
  - Run `timeout 60 ./scripts/sanitizer-smoke.sh` (with `ORI_SANITIZE=address,undefined`) and verify it completes within 60 seconds and passes.
  - If any smoke test fails, that's a pre-existing memory bug in the generated code — file via `/add-bug`.

- [ ] **Subsection close-out (08.4)** — MANDATORY before starting 08.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

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

- [ ] `SanitizerMode` type defined in `config.rs` with `address` and `undefined` fields
- [ ] `OptimizationConfig` has `sanitizer: SanitizerMode` field with builder method
- [ ] `ORI_SANITIZE` registered in `debug_flags.rs`
- [ ] `ORI_SANITIZE` wired through `build_optimization_config()`
- [ ] `SanitizerMode::from_env_value()` parses comma-separated sanitizer names
- [ ] LLVM pass pipeline includes sanitizer passes when enabled
- [ ] ASan instrumentation visible in emitted IR (`__asan_load`/`__asan_store` calls)
- [ ] GCC linker emits `-fsanitize=address,undefined` when sanitizers enabled
- [ ] Sanitized binary runs correctly for simple programs
- [ ] Clear error message when sanitizer runtime libraries are missing
- [ ] `tests/sanitizer/` contains ≤20 curated smoke test programs
- [ ] `scripts/sanitizer-smoke.sh` runs smoke suite and reports pass/fail
- [ ] Smoke suite completes within 60 seconds
- [ ] At least one test detects a memory error that would be silent without sanitizers
- [ ] Nightly CI configuration added (full sweep with sharding)
- [ ] No regressions: `timeout 150 ./test-all.sh` green (sanitizers OFF)
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 08` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference updated
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** `ORI_SANITIZE=address,undefined ori build file.ori` compiles with sanitizer instrumentation. The generated binary runs with ASan/UBSan runtime checking active. `scripts/sanitizer-smoke.sh` completes within 60 seconds and passes all ≤20 smoke tests. The nightly CI workflow runs the full spec test suite with sanitizers enabled (sharded for timeout compliance). At least one test case demonstrates that a memory error is caught by the sanitizer that would be silent without it. `timeout 150 ./test-all.sh` (without sanitizers) passes with 0 regressions.
