---
section: "09"
title: "Alive2 Formal Verification"
status: in-progress
reviewed: true
goal: "Integrate Alive2's alive-tv translation validator to formally verify that LLVM optimization passes preserve the semantics of Ori's emitted IR — running a curated subset of pure/arithmetic functions on every CI nightly build and a full sweep weekly"
success_criteria:
  - "alive-tv binary is built from a pinned Alive2 commit and available in CI via cached artifact"
  - "diagnostics/alive2-verify.sh script verifies pre-opt vs post-opt IR refinement for a given .ori file"
  - "Curated test corpus of >=15 pure/arithmetic-heavy Ori functions passes alive-tv refinement checking"
  - "False positive suppression list handles known-benign counterexamples across 9 categories (runtime-call, memory-model, loop-bound, invoke, inter-procedural, closure-indirect-call, iterator-runtime, cow-alias, overflow-panic)"
  - "Nightly CI job (in nightly-verification.yml) runs alive-tv on the curated corpus with zero unresolved refinement failures"
  - "Weekly CI job runs alive-tv on all compiler/ori_llvm/tests/codegen/ IR with timeout per function"
  - "Machine-readable results in build/alive2-results/ with defined JSON schema consumed by Sections 11 and 12"
inspired_by:
  - "Alive2 alive-tv (~/projects/reference_repos/verification_tools/alive2/tools/alive-tv.cpp) — standalone translation validation comparing two LLVM IR files"
  - "Alive2 opt plugin (~/projects/reference_repos/verification_tools/alive2/tv/tv.cpp) — per-pass translation validation integrated into LLVM opt pipeline"
  - "Rust LLVM CI (rust-lang/rust .github/workflows/) — nightly verification jobs separate from PR CI"
depends_on: ["07"]
third_party_review:
  status: resolved
  updated: 2026-04-13
sections:
  - id: "09.1"
    title: "Build Alive2 from Pinned Source with Z3 Dependencies"
    status: complete
  - id: "09.2"
    title: "IR Capture Pipeline in oric (Phase-Pure)"
    status: in-progress
  - id: "09.3"
    title: "Diagnostic Script, Function Selection, and Inlining Survival"
    status: complete
  - id: "09.4"
    title: "False Positive Management (9 Categories)"
    status: in-progress
  - id: "09.5"
    title: "CI Integration via nightly-verification.yml"
    status: complete
  - id: "09.6"
    title: "Machine-Readable Artifact Contract for Sections 11/12"
    status: complete
  - id: "09.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "09.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 09: Alive2 Formal Verification

**Status:** Not Started
**Goal:** Integrate Alive2's `alive-tv` translation validator to formally verify that LLVM optimization passes preserve the semantics of Ori's emitted IR. Unlike behavioral testing (which checks "correct output for specific inputs"), Alive2 provides mathematical proof that the optimized IR is a valid refinement of the unoptimized IR — catching miscompiles that no finite set of test inputs can find. The integration targets a curated subset of pure/arithmetic functions (where Alive2 excels) and explicitly excludes RC operations, runtime calls, closures with indirect calls, COW branching, iterator loop nests, and checked-overflow panic blocks (where Alive2's limitations produce false positives).

**Success Criteria:**

- [ ] `alive-tv` binary built from pinned commit, available in CI — satisfies mission criterion: "Alive2 refinement checking curated subset"
- [ ] >=15 curated pure/arithmetic functions pass alive-tv — satisfies mission criterion: "nightly alive-tv green"
- [ ] False positive suppression (9 categories) prevents spurious failures — satisfies mission criterion: "zero unresolved failures"
- [ ] Nightly CI runs curated corpus; weekly CI runs full `compiler/ori_llvm/tests/codegen/` — satisfies mission criterion: "CI fully integrated"
- [ ] Machine-readable `build/alive2-results/results.json` consumed by Sections 11 and 12 — satisfies mission criterion: "CI artifact contract"

**Context:** Alive2 is the standard tool for formal verification of LLVM transformations. It uses the Z3 SMT solver to prove that for ALL possible inputs, the post-optimization IR produces the same observable behavior as the pre-optimization IR (or strictly more defined behavior — "refinement"). This is strictly stronger than testing: a test checks one input, Alive2 proves all inputs. However, Alive2 has significant limitations: it does not support inter-procedural transformations, loops are unrolled to a configurable bound (~256 max), exception handling (`invoke`/`landingpad`) is not modeled, function calls are approximated conservatively, indirect calls (closures) produce false positives, and memory operations can produce false positives. For Ori, this means Alive2 is best applied to pure/arithmetic functions — NOT to RC operations, runtime calls, closures, COW branches, iterators, or programs with complex control flow. The curated subset is guided by the FileCheck test corpus from Section 07, which identifies which functions have clean, self-contained LLVM IR.

**Scope boundary — LTO:** The standard AOT pipeline (`verify_optimize_emit` in `compiler/ori_llvm/src/aot/object.rs:294`) runs `run_optimization_passes` on a per-module basis. The LTO paths (`prelink_and_emit_bitcode` at `passes/mod.rs:371` and `run_lto_pipeline` at `passes/mod.rs:414`) involve cross-module merging and separate pipeline strings (`thinlto-pre-link<O2>`, `lto<O2>`). Alive2 operates on per-function refinement within a single module — it cannot verify cross-module LTO transformations (this is an Alive2 limitation, not an Ori limitation). **LTO is explicitly out of scope for Section 09.** If LTO verification becomes needed, it belongs in a dedicated section with a different tool (e.g., comparing pre-LTO-merge per-module IR against post-LTO output, which requires IR extraction from bitcode). This scope-out is anchored by the `inter-procedural` suppression category — any LTO-related false positive encountered during weekly sweeps is suppressed under this category with a `<!-- blocked-by: future-lto-verification -->` note.

**Reference implementations:**
- **Alive2** `tools/alive-tv.cpp`: Standalone translation validator. Takes two LLVM IR files (or one file with `-passes=` flag to optimize), compares function pairs for refinement. Uses Z3 for SMT solving. Supports `--src-fn`/`--tgt-fn` for per-function verification.
- **Alive2** `tv/tv.cpp`: LLVM opt plugin that intercepts every optimization pass and verifies each transform in-flight. Higher coverage but much slower — suitable for weekly sweeps.
- **Alive2** `README.md`: "Alive2 does not support inter-procedural transformations. Alive2 may produce spurious counterexamples if run with such passes."

**Depends on:** Section 07 (FileCheck test corpus provides the input selection guide — functions with clean IR patterns are the best candidates for Alive2 verification).

**Rule integration:**
- **Phase purity** (compiler.md, impl-hygiene.md): IO lives in `oric` only. IR capture (`ORI_DUMP_PREOPT_LLVM`, `ORI_DUMP_POSTOPT_LLVM`) is file IO, so it belongs in `oric` (specifically `compiler/oric/src/commands/build/single.rs` and `compiler/oric/src/commands/build/multi_emission.rs`), NOT in `ori_llvm`.
- **File size limit** (CLAUDE.md): `passes/mod.rs` is 438 lines — at the 500-line limit. No capture logic goes there. Capture logic goes in a new `compiler/oric/src/commands/build/ir_capture.rs` helper.
- **SSOT for flags** (impl-hygiene.md): All new env vars (`ORI_DUMP_PREOPT_LLVM`, `ORI_DUMP_POSTOPT_LLVM`, `ORI_ALIVE2_CAPTURE`) registered in `compiler/oric/src/debug_flags.rs` as the single source of truth.
- **Cross-platform** (impl-hygiene.md): Use `std::env::temp_dir()` not `/tmp` in scripts and tests.

---

## 09.1 Build Alive2 from Pinned Source with Z3 Dependencies

**File(s):** `scripts/build-alive2.sh`, `.github/workflows/nightly-verification.yml` (nightly job dependency)

Alive2 tracks LLVM top-of-tree and may not build against a specific LLVM release. The build script pins a known-good Alive2 commit that builds cleanly against LLVM 21 (our required version per `.cargo/config.toml` and `.claude/rules/llvm.md`). The pinned commit must be updated when the LLVM version changes.

- [x] Create `scripts/build-alive2.sh` with commit pinning:
  - Define version constants at the top of the script:
    ```bash
    ALIVE2_REPO="https://github.com/AliveToolkit/alive2.git"
    # Pinned commit: must build against LLVM 21. Update when LLVM version changes.
    ALIVE2_COMMIT="<sha>"  # TODO: determine at implementation time by testing HEAD against LLVM 21
    EXPECTED_LLVM_MAJOR=21
    ```
  - Validate system LLVM version matches expected before building:
    ```bash
    ACTUAL_LLVM=$(llvm-config --version | cut -d. -f1)
    if [ "$ACTUAL_LLVM" != "$EXPECTED_LLVM_MAJOR" ]; then
      echo "ERROR: Alive2 pinned to LLVM $EXPECTED_LLVM_MAJOR but found LLVM $ACTUAL_LLVM" >&2
      exit 1
    fi
    ```
  - Check for Z3 installation (headers + library): `pkg-config --exists z3` or check `/usr/include/z3.h`
  - Check for re2c: `which re2c`
  - Clone or update Alive2 from `~/projects/reference_repos/verification_tools/alive2/` (local) or GitHub (CI), then checkout pinned commit
  - Build Alive2 with cmake+ninja against the system LLVM 21 with the **`BUILD_TV`** flag (without this, the `alive-tv` target is never added to the cmake build). Use CMake's native `FindZ3` for cross-platform Z3 discovery — do NOT hardcode platform-specific paths:
    ```bash
    # Dynamic Z3 + LLVM discovery — works on Ubuntu, Debian, macOS (Homebrew), etc.
    # CMake's FindZ3 module (Alive2 uses find_package(Z3 4.8.5 REQUIRED))
    # handles platform-specific library paths when CMAKE_PREFIX_PATH is set.
    # Use modern cmake -B/-S for explicit out-of-source build.
    # The CI cache path (build/alive2/alive-tv) must match this layout.
    cmake -B build -S . -GNinja \
      -DCMAKE_BUILD_TYPE=Release \
      -DCMAKE_PREFIX_PATH="$(llvm-config --prefix)" \
      -DBUILD_TV=ON
    ninja -C build alive-tv
    # Binary is at: build/alive-tv (relative to Alive2 checkout root)
    ```
    Note: Do NOT pass `-DZ3_INCLUDE_DIR` or `-DZ3_LIBRARIES` with hardcoded paths — this breaks macOS and non-Debian Linux. CMake's `FindZ3` discovers Z3 automatically. If Z3 is in a non-standard location, pass its prefix via `CMAKE_PREFIX_PATH` alongside LLVM's.
    **WHERE:** `scripts/build-alive2.sh` (new file)
  - Output `alive-tv` binary path to stdout for downstream consumption
  - Support `--cached` flag that skips rebuild if `alive-tv` binary exists and is newer than source

- [x] Document Z3 installation requirements in the script's `--help` output:
  ```
  # Ubuntu/Debian: sudo apt-get install libz3-dev re2c
  # macOS: brew install z3 re2c
  ```

- [x] Add CI caching for the `alive-tv` binary. Cache key includes both LLVM version and the pinned Alive2 commit hash to invalidate on either change:
  ```yaml
  - uses: actions/cache@v4
    with:
      path: build/alive2/alive-tv
      key: alive2-llvm${{ env.LLVM_MAJOR }}-${{ env.ALIVE2_COMMIT }}-${{ hashFiles('scripts/build-alive2.sh') }}
  ```
  **WHERE:** `.github/workflows/nightly-verification.yml` (Alive2 job)

- [x] Verify the built `alive-tv` works by running it on a trivial identity function:
  ```bash
  tmpfile="$(mktemp --suffix=.ll)"
  echo 'define i64 @f(i64 %x) { ret i64 %x }' > "$tmpfile"
  ./alive-tv "$tmpfile" --passes=instcombine
  # Should print: "Transformation seems to be correct!"
  rm -f "$tmpfile"
  ```
  Use `mktemp` (not hardcoded `/tmp`) per cross-platform rules.

- [x] **Subsection close-out (09.1)** — MANDATORY before starting 09.2:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 09.1: no tooling gaps. Build script authoring task — the script itself is the tooling improvement. Friction points (LLVM compat bisection, shallow clone handling, missing re2c) all resolved inline.
  - [x] **Repo hygiene check** — clean (2026-04-13).

---

## 09.2 IR Capture Pipeline in oric (Phase-Pure)

**File(s):** `compiler/oric/src/debug_flags.rs`, `compiler/oric/src/commands/build/ir_capture.rs` (new), `compiler/oric/src/commands/build/single.rs`, `compiler/oric/src/commands/build/multi_emission.rs`

Alive2's `alive-tv` needs two IR files: the pre-optimization LLVM IR and the post-optimization LLVM IR. Ori must capture these at the right pipeline boundary.

**Phase purity constraint (compiler.md, impl-hygiene.md):** IO lives in `oric` only — core crates (`ori_llvm`) are pure. IR capture writes files, so it MUST live in `oric`, not `ori_llvm`. The capture calls `module.print_to_file()` from the `oric` build pipeline where the module is already accessible.

**Capture timing constraint:** `ORI_DUMP_PREOPT_LLVM` must capture AFTER `module.verify()` succeeds (Step 1 in `verify_optimize_emit` at `compiler/ori_llvm/src/aot/object.rs:306-309`) but BEFORE `run_optimization_passes` (Step 2 at `object.rs:312`). Alive2 requires valid IR — capturing before verification would produce potentially malformed IR that alive-tv cannot parse. This is different from `ORI_DUMP_AFTER_LLVM` which intentionally dumps BEFORE verification (at `codegen_pipeline.rs:481-488`) so broken IR is visible for debugging.

**Distinction from existing flags:**
- `ORI_DUMP_AFTER_LLVM` (existing): Dumps annotated IR to stderr for human diagnostic, runs before verification, includes Ori-aware annotations. Purpose: human debugging.
- `ORI_DUMP_PREOPT_LLVM` (new): Dumps raw `.ll` file after verification, before optimization. Purpose: machine-readable input for alive-tv.
- `ORI_DUMP_POSTOPT_LLVM` (new): Dumps raw `.ll` file after optimization, before object emission. Purpose: machine-readable input for alive-tv.

**File size constraint:** `passes/mod.rs` is 438 lines (approaching 500-line limit). No capture logic goes there. All capture logic lives in a new `ir_capture.rs` helper in `oric`.

- [x] Register three new flags in `compiler/oric/src/debug_flags.rs` (the single source of truth for all debug env vars):
  ```rust
  /// Dump raw LLVM IR to a .preopt.ll file after verification, before optimization.
  ///
  /// Produces machine-readable IR suitable for alive-tv input.
  /// Distinct from `ORI_DUMP_AFTER_LLVM` which dumps annotated IR to stderr
  /// for human debugging (and before verification).
  /// Usage: `ORI_DUMP_PREOPT_LLVM=1 ori build file.ori`
  ORI_DUMP_PREOPT_LLVM

  /// Dump raw LLVM IR to a .postopt.ll file after optimization, before emission.
  ///
  /// Produces machine-readable IR suitable for alive-tv input.
  /// Usage: `ORI_DUMP_POSTOPT_LLVM=1 ori build file.ori`
  ORI_DUMP_POSTOPT_LLVM

  /// Enable Alive2 IR capture: dumps both pre-opt and post-opt IR to build/alive2-results/.
  ///
  /// Convenience flag that enables both `ORI_DUMP_PREOPT_LLVM` and `ORI_DUMP_POSTOPT_LLVM`
  /// and places output in a structured directory.
  /// Usage: `ORI_ALIVE2_CAPTURE=1 ori build file.ori`
  ORI_ALIVE2_CAPTURE
  ```
  **WHERE:** `compiler/oric/src/debug_flags.rs` — add to the `flags!` block after the existing verification flags (after line 165).

- [x] Create `compiler/oric/src/commands/build/ir_capture.rs` helper module (keeps capture logic out of single.rs and passes/mod.rs):
  ```rust
  //! IR capture for Alive2 translation validation.
  //!
  //! Captures pre-opt and post-opt LLVM IR to files for alive-tv consumption.
  //! Lives in oric (not ori_llvm) per phase purity: IO is oric-only.

  use std::path::{Path, PathBuf};

  /// Determines if any IR capture is requested.
  pub fn capture_requested() -> bool {
      crate::dbg_set!(crate::debug_flags::ORI_ALIVE2_CAPTURE)
          || crate::dbg_set!(crate::debug_flags::ORI_DUMP_PREOPT_LLVM)
          || crate::dbg_set!(crate::debug_flags::ORI_DUMP_POSTOPT_LLVM)
  }

  /// Determines if pre-opt capture is requested.
  pub fn preopt_requested() -> bool {
      crate::dbg_set!(crate::debug_flags::ORI_ALIVE2_CAPTURE)
          || crate::dbg_set!(crate::debug_flags::ORI_DUMP_PREOPT_LLVM)
  }

  /// Determines if post-opt capture is requested.
  pub fn postopt_requested() -> bool {
      crate::dbg_set!(crate::debug_flags::ORI_ALIVE2_CAPTURE)
          || crate::dbg_set!(crate::debug_flags::ORI_DUMP_POSTOPT_LLVM)
  }

  /// Compute output path for a capture file.
  pub fn capture_path(source_path: &str, suffix: &str) -> PathBuf {
      if crate::dbg_set!(crate::debug_flags::ORI_ALIVE2_CAPTURE) {
          // Structured directory for Alive2 — encode repo-relative path
          // to guarantee uniqueness (multiple test files share basenames
          // like traits.ori, collections.ori across different directories)
          let dir = Path::new("build/alive2-results");
          std::fs::create_dir_all(dir).ok();
          // Sanitize repo-relative path: strip extension safely, then replace / with _
          // e.g., "tests/spec/traits/iterator/map.ori" -> "tests_spec_traits_iterator_map"
          // Use Path::with_extension("") to strip only the trailing extension — do NOT
          // use .replace(".ori", "") which corrupts paths containing ".ori" in directory
          // names (e.g., "tests/spec/ori_rc/basic.ori" would lose "ori_" → collision)
          let without_ext = Path::new(source_path).with_extension("");
          let sanitized = without_ext.to_str().unwrap_or(source_path)
              .trim_start_matches("./")
              .replace('/', "_");
          dir.join(format!("{sanitized}{suffix}"))
      } else {
          // Alongside the source file
          Path::new(source_path).with_extension(suffix.trim_start_matches('.'))
      }
  }
  ```
  **WHERE:** `compiler/oric/src/commands/build/ir_capture.rs` (new file, ~50 lines)

- [x] Refactor `ObjectEmitter::verify_optimize_emit` in `compiler/ori_llvm/src/aot/object.rs` to accept optional capture hooks. This keeps the pipeline orchestration (verify -> optimize -> sanitizer -> audit -> emit) as the sole SSOT in `ori_llvm`, while letting `oric` inject IO at the right points via closures:
  ```rust
  /// Optional hooks for IR capture at pipeline boundaries.
  /// Closures perform IO in the caller's crate (oric), not in ori_llvm.
  pub struct CaptureHooks<'a> {
      /// Called after module verification succeeds, before optimization.
      pub pre_opt: Option<Box<dyn FnOnce(&Module<'_>) -> Result<(), String> + 'a>>,
      /// Called after optimization completes, before object emission.
      pub post_opt: Option<Box<dyn FnOnce(&Module<'_>) -> Result<(), String> + 'a>>,
  }

  impl Default for CaptureHooks<'_> {
      fn default() -> Self { Self { pre_opt: None, post_opt: None } }
  }
  ```
  Then modify `verify_optimize_emit` to call hooks at the correct pipeline points — between verify and optimize (pre_opt), and between optimize and emit (post_opt). Non-capture builds pass `CaptureHooks::default()` (zero overhead).
  **WHERE:** `compiler/ori_llvm/src/aot/object.rs` — refactor `verify_optimize_emit` (~15 lines added). This preserves the existing sanitizer check, audit histogram, and Clang delegation inside the canonical pipeline owner.

- [x] Wire capture hooks from `oric` into the refactored `verify_optimize_emit`. In `single.rs`, construct `CaptureHooks` from the `ir_capture` helper:
  ```rust
  let hooks = if ir_capture::capture_requested() {
      CaptureHooks {
          pre_opt: if ir_capture::preopt_requested() {
              let path = ir_capture::capture_path(source, ".preopt.ll");
              Some(Box::new(move |m: &Module| m.print_to_file(&path).map_err(|e| e.to_string())))
          } else { None },
          post_opt: if ir_capture::postopt_requested() {
              let path = ir_capture::capture_path(source, ".postopt.ll");
              Some(Box::new(move |m: &Module| m.print_to_file(&path).map_err(|e| e.to_string())))
          } else { None },
      }
  } else { CaptureHooks::default() };
  emitter.verify_optimize_emit(&module, &opt_config, &obj_path, format, hooks)?;
  ```
  **WHERE:** `compiler/oric/src/commands/build/single.rs` — at the existing `verify_optimize_emit` call site.

- [x] Apply the same hook wiring to `compiler/oric/src/commands/build/multi_emission.rs`. Note: the multi-file path calls `emit_module_artifact(ctx, llvm_module, module_name)` which does not currently receive the source file path. Thread `source_path` from `compile_single_module(...)` (one layer up) into `emit_module_artifact` so that `capture_path()` can encode the repo-relative path for collision-free filenames. Without this, the multi-file capture cannot produce unique artifact names per the iteration-3 invariant.
  **WHERE:** `compiler/oric/src/commands/build/multi_emission.rs` — at the `verify_optimize_emit` call site and the `emit_module_artifact` signature.

- [x] Re-export `CaptureHooks` from `ori_llvm::aot` (via `compiler/ori_llvm/src/aot/mod.rs`) so `oric` can import it without reaching into `ori_llvm::aot::object` directly. This follows the existing pattern where `ObjectEmitter` and `OutputFormat` are already re-exported from `ori_llvm::aot`.
  **WHERE:** `compiler/ori_llvm/src/aot/mod.rs` — add `pub use object::CaptureHooks;`

- [x] Update all other callers of `verify_optimize_emit` (if any) to pass `CaptureHooks::default()`. Grep for `verify_optimize_emit` across the codebase to find all call sites.

- [x] Verify that `module.print_to_file()` preserves all function definitions (standard LLVM behavior, but confirm with a test that a multi-function module round-trips correctly). Alive2 compares functions by name — both pre-opt and post-opt IR must contain the same `@_ori_*` function names.

- [ ] **TPR checkpoint** — `/tpr-review` covering 09.1-09.2 implementation work

- [x] **Subsection close-out (09.2)** — MANDATORY before starting 09.3:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 09.2: no tooling gaps. Friction: `check-debug-flags.sh` correctly caught undocumented flags — fixed inline. `ModuleCompileContext` didn't have `source_path` — threaded through. No new diagnostic tooling needed.
  - [x] **Repo hygiene check** — clean (2026-04-13).

---

## 09.3 Diagnostic Script, Function Selection, and Inlining Survival

**File(s):** `diagnostics/alive2-verify.sh`, `tests/alive2/curated-corpus.txt`

Build the diagnostic script that orchestrates alive-tv verification and curate the initial function corpus. A critical concern: pure/arithmetic functions are the most likely to be fully inlined at `-O2`, causing alive-tv to silently verify nothing (missing functions are ignored). The corpus must include an inlining survival filter.

- [x] Create `diagnostics/alive2-verify.sh` following existing diagnostic conventions (`--help`, `--no-color`, `--verbose`, `--json`, exit codes 0/1/2):
  ```bash
  # Usage: diagnostics/alive2-verify.sh [OPTIONS] <file.ori | --corpus>
  #
  # Options:
  #   --corpus           Run against curated corpus (tests/alive2/curated-corpus.txt)
  #   --function NAME    Verify only the named function
  #   --timeout SECS     Per-function Z3 timeout (default: 60)
  #   --passes PASSES    LLVM passes to verify (default: O2)
  #   --verbose          Show alive-tv output for passing functions
  #   --json             Machine-readable output to build/alive2-results/results.json
  #   --suppress FILE    False positive suppression file (default: tests/alive2/suppressed.json)
  #   --strict           Ignore all suppressions (for deep manual verification)
  #   --check-survival   Verify that target functions survive optimization (not inlined away)
  #   --all-codegen      Run against all .ori files in compiler/ori_llvm/tests/codegen/
  #                      (discovers files via find, filters to .ori extension, used by weekly CI sweep)
  ```
  **WHERE:** `diagnostics/alive2-verify.sh` (new file)

- [x] Implement the verification pipeline in the script:
  1. Build the `.ori` file with `ORI_ALIVE2_CAPTURE=1 cargo run -- build <file>`
  2. Extract function names from the pre-opt IR: `grep '^define' preopt.ll | sed 's/.*@\([^ (]*\).*/\1/'`
  3. Extract function names from the post-opt IR (same command on postopt.ll)
  4. **Inlining survival filter**: For each target function, verify it exists in BOTH pre-opt and post-opt IR. If a function was inlined away (present in pre-opt but absent in post-opt), report it as `inlined` (not `verified` and not `failed`). This prevents alive-tv from silently skipping functions and reporting a false "all clear":
     ```bash
     preopt_funcs=$(grep '^define' "$preopt" | extract_names)
     postopt_funcs=$(grep '^define' "$postopt" | extract_names)
     for func in $target_funcs; do
       if ! echo "$postopt_funcs" | grep -qF "$func"; then
         report "inlined" "$func" "function was inlined away at $OPT_LEVEL — not verifiable by alive-tv"
         continue
       fi
       # ... run alive-tv
     done
     ```
  5. For each surviving function (or `--function` target):
     - Run `alive-tv preopt.ll postopt.ll --src-fn func --tgt-fn func --smt-to=<timeout>` (note: alive-tv expects function names WITHOUT the leading `@` — the `@` is LLVM IR syntax, not part of the name; strip it when extracting from `grep '^define'` output)
     - Parse output for "correct", "incorrect", "timeout", "unknown"
     - Check against suppression list for known false positives
  6. Report summary: N verified, M timeouts, K suppressed, L failures, P inlined

- [x] Curate the initial function corpus (`tests/alive2/curated-corpus.txt`). Selection criteria:
  - **Include**: Pure arithmetic functions, simple control flow (no loops or loops with small bounds), no runtime calls (`_ori_rc_inc`, `_ori_rc_dec`, `_ori_alloc`, `_ori_panic`), no exception handling (`invoke`/`landingpad`), no indirect calls (closures)
  - **Exclude**: Functions with `call void @_ori_rc_*` (RC operations), functions with `invoke` (exception handling), functions calling external runtime (`_ori_*`), functions with large loop nests (>256 iterations), functions with `va_arg` or variadics, functions with `_ori_is_unique` calls (COW branches), functions with checked-overflow intrinsics that branch to panic blocks
  - **Survival requirement**: Each corpus entry must **naturally** survive `-O2` optimization (not be fully inlined away). Verify with `--check-survival` during corpus creation. If a function is inlined at `-O2`, **replace it** with one that survives — do NOT attempt to inject `noinline` attributes, as there is no clean transport mechanism from the corpus file to the compiler's codegen path (CaptureHooks are module-level IO hooks, not per-function attribute injectors). The `--check-survival` filter is the enforcement mechanism: any entry that fails survival is flagged as an error during `--corpus` runs, forcing the maintainer to find a replacement.
  - **Source**: Start from `compiler/ori_llvm/tests/codegen/` (Section 07's FileCheck tests) — functions that have clean CHECK patterns are good Alive2 candidates. Also include pure functions from `tests/spec/` that compile to small LLVM IR. Prefer functions with `external` linkage or `@main`-like entry points that the optimizer cannot fully inline.
  - Format: one line per entry: `<ori_file_path> <function_name>`
  **WHERE:** `tests/alive2/curated-corpus.txt` (new file)

- [x] Create `tests/alive2/` directory with the corpus file and a README explaining the selection criteria, survival requirements, and how to add new entries.

- [x] Add the script to `diagnostics/self-test.sh` with a minimal positive test (one known-good pure function that survives optimization).

- [x] **Subsection close-out (09.3)** — MANDATORY before starting 09.4:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 09.3: no tooling gaps. Script follows diagnostic conventions. Corpus verified end-to-end (8/8 functions pass). Self-test integration gated on alive-tv availability.
  - [x] **Repo hygiene check** — clean (2026-04-13).

---

## 09.4 False Positive Management (9 Categories)

**File(s):** `tests/alive2/suppressed.json`, `diagnostics/alive2-verify.sh` (suppression logic)

Alive2 will produce false positives for Ori programs because it conservatively approximates function calls and does not model Ori's runtime semantics. A structured suppression system prevents false positives from blocking CI while keeping a clear audit trail. The category list is comprehensive based on analysis of Ori's codegen patterns.

- [x] Define the suppression file format (`tests/alive2/suppressed.json`):
  ```json
  [
    {
      "function": "_ori_main",
      "file": "compiler/ori_llvm/tests/codegen/rc/basic_rc.ori",
      "reason": "Contains _ori_rc_inc/_ori_rc_dec runtime calls that Alive2 cannot model",
      "category": "runtime-call",
      "added": "2026-04-10",
      "alive2_output_hash": "abc123..."
    }
  ]
  ```
  **WHERE:** `tests/alive2/suppressed.json` (new file)

- [x] Implement suppression matching in `alive2-verify.sh`:
  - Before reporting a failure, check if the function+file pair is in the suppression list
  - If suppressed, report as "suppressed" (not "passed" and not "failed")
  - If the alive2 output hash differs from the recorded hash, report as "suppression-stale" — the failure mode changed, requiring re-investigation
  - `--strict` flag ignores all suppressions (for manual deep verification)

- [x] Define 9 suppression categories covering all Ori codegen patterns that produce Alive2 false positives:
  1. **`runtime-call`** — function calls Ori runtime (`_ori_*`) which Alive2 cannot model (e.g., `_ori_rc_inc`, `_ori_rc_dec`, `_ori_alloc`, `_ori_string_concat`)
  2. **`memory-model`** — Alive2's memory model disagrees with Ori's ARC semantics (e.g., uniqueness-based optimizations that Alive2 cannot prove sound)
  3. **`loop-bound`** — loop exceeds Alive2's unroll limit (~256 iterations)
  4. **`invoke`** — exception handling paths (`invoke`/`landingpad`) not modeled by Alive2
  5. **`inter-procedural`** — cross-function transformations; Alive2 README explicitly warns about this. Also covers LTO-related false positives (`<!-- blocked-by: future-lto-verification -->`)
  6. **`closure-indirect-call`** — closures compile to indirect `call` through function pointers; Alive2 conservatively assumes any side effect for indirect calls, producing false counterexamples
  7. **`iterator-runtime`** — iterator functions compile to complex loop nests with runtime dispatch (variant-based `next()` calls); the loop structure plus indirect dispatch exceeds Alive2's analysis capability
  8. **`cow-alias`** — COW branching involves `_ori_is_unique` runtime calls that check reference count; Alive2 cannot model the aliasing semantics, so COW code paths produce false positives about potentially-aliased memory
  9. **`overflow-panic`** — checked arithmetic intrinsics (`llvm.sadd.with.overflow.*`) branch to panic blocks on overflow; Alive2 may report these as refinement failures because the panic path has different defined behavior than the overflow-is-UB model

- [x] Add a `--review-suppressions` flag that checks whether suppressions are still needed — rerun alive-tv on each suppressed entry and report which ones now pass (can be removed from the suppression list).

- [ ] **TPR checkpoint** — `/tpr-review` covering 09.3-09.4 implementation work

- [x] **Subsection close-out (09.4)** — MANDATORY before starting 09.5:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 09.4: no tooling gaps. Suppression system is JSON-based with python3 parsing. `--review-suppressions` validates staleness. Currently 0 suppressions — categories documented for when weekly sweeps encounter false positives.
  - [x] **Repo hygiene check** — clean (2026-04-13).

---

## 09.5 CI Integration via nightly-verification.yml

**File(s):** `.github/workflows/nightly-verification.yml` (existing workflow — extend, do not create new)

Wire alive-tv into CI with tiered execution: nightly runs the curated corpus (fast, high-value), weekly runs the full FileCheck test set (slow, comprehensive). The existing `nightly-verification.yml` already handles sanitizer smoke and full sweep jobs (added in Section 08). Alive2 jobs are added to this existing workflow.

- [x] Update `nightly-verification.yml`'s `pull_request.paths` to include Alive2-owned files so that changes to Alive2 tooling trigger PR validation:
  ```yaml
  paths:
    # ... existing paths (compiler/**, library/**, tests/sanitizer/**, etc.)
    - scripts/build-alive2.sh
    - diagnostics/alive2-verify.sh
    - tests/alive2/**
  ```
  **WHERE:** `.github/workflows/nightly-verification.yml` — in the existing `on.pull_request.paths` block.

- [x] Add nightly CI job `alive2-curated` to the existing `.github/workflows/nightly-verification.yml`:
  ```yaml
  alive2-curated:
    name: Alive2 Curated Corpus (Nightly)
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v4
      - name: Install dependencies
        run: sudo apt-get install -y libz3-dev re2c
      - name: Install LLVM 21
        run: # ... same as existing CI LLVM install steps
      - name: Extract Alive2 pin for cache key
        run: |
          # SSOT: the pinned commit lives in scripts/build-alive2.sh.
          # Export to GITHUB_ENV so the cache key can reference it.
          ALIVE2_COMMIT=$(grep '^ALIVE2_COMMIT=' scripts/build-alive2.sh | cut -d'"' -f2)
          LLVM_MAJOR=$(llvm-config --version | cut -d. -f1)
          echo "ALIVE2_COMMIT=$ALIVE2_COMMIT" >> "$GITHUB_ENV"
          echo "LLVM_MAJOR=$LLVM_MAJOR" >> "$GITHUB_ENV"
      - name: Cache alive-tv binary
        uses: actions/cache@v4
        with:
          path: build/alive2/alive-tv
          key: alive2-llvm${{ env.LLVM_MAJOR }}-${{ env.ALIVE2_COMMIT }}-${{ hashFiles('scripts/build-alive2.sh') }}
      - name: Build alive-tv
        run: ./scripts/build-alive2.sh --cached
      - name: Build Ori compiler
        run: cargo build
      - name: Run Alive2 on curated corpus
        run: diagnostics/alive2-verify.sh --corpus --json --timeout 60 --check-survival
      - name: Upload results
        uses: actions/upload-artifact@v4
        if: always()
        with:
          name: alive2-curated-results
          path: build/alive2-results/
          retention-days: 30
  ```
  **WHERE:** `.github/workflows/nightly-verification.yml` — add after the existing `sanitizer-full` job.

- [x] Define the weekly CI job `alive2-full-sweep` in `nightly-verification.yml` as a **provisional** job. Section 11 (CI Integration) may later consolidate weekly jobs into a dedicated `weekly-verification.yml` workflow — at that point, the job moves from `nightly-verification.yml` to the new file. Section 09 defines the job's content and interface; Section 11 owns the final workflow topology. Add a cross-reference: `<!-- plan-sync: Section 11.3 may relocate this job to weekly-verification.yml -->`:
  ```yaml
  alive2-full-sweep:
    name: Alive2 Full Sweep (Weekly)
    runs-on: ubuntu-latest
    timeout-minutes: 120
    if: github.event_name == 'schedule' && github.event.schedule == '0 4 * * 0'
    needs: alive2-curated
    steps:
      # ... same setup as curated
      - name: Run Alive2 on all codegen tests
        run: |
          diagnostics/alive2-verify.sh --all-codegen --json --timeout 120 \
            --suppress tests/alive2/suppressed.json --check-survival
      - name: Upload results
        uses: actions/upload-artifact@v4
        if: always()
        with:
          name: alive2-full-results
          path: build/alive2-results/
          retention-days: 90
  ```
  **WHERE:** `.github/workflows/nightly-verification.yml` — add after `alive2-curated`.

  Note: The weekly gate uses `schedule == '0 4 * * 0'` (Sunday 4 AM UTC). The existing nightly runs at `cron: '30 2 * * *'` (2:30 AM UTC daily). A separate weekly cron entry is needed in the `on.schedule` block:
  ```yaml
  on:
    schedule:
      - cron: '30 2 * * *'  # existing nightly
      - cron: '0 4 * * 0'   # weekly (Sunday 4 AM UTC)
  ```

- [x] Configure Z3 timeout appropriately:
  - Nightly (curated): 60 seconds per function — pre-selected to be fast
  - Weekly (full sweep): 120 seconds per function — allows more complex functions
  - Functions that timeout are reported but do not fail the job (they indicate candidates for the suppression list or corpus exclusion)

- [x] **Subsection close-out (09.5)** — MANDATORY before starting 09.6:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 09.5: no tooling gaps. CI jobs use shared cache key pattern from alive2-build. Nightly=60s timeout, weekly=120s. Section 11 cross-ref added for future job relocation.
  - [x] **Repo hygiene check** — clean (2026-04-13).

---

## 09.6 Machine-Readable Artifact Contract for Sections 11/12

**File(s):** `diagnostics/alive2-verify.sh` (JSON output), `tests/alive2/results-schema.json`

Sections 11 (CI Integration) and 12 (Regression Dashboard) explicitly consume artifacts from `build/alive2-results/`. This subsection defines the machine-readable output format that serves as the contract between Section 09 (producer) and Sections 11/12 (consumers).

- [x] Define the output schema (`tests/alive2/results-schema.json`):
  ```json
  {
    "$schema": "http://json-schema.org/draft-07/schema#",
    "type": "object",
    "required": ["version", "timestamp", "llvm_version", "alive2_commit", "mode", "summary", "functions"],
    "properties": {
      "version": { "type": "integer", "const": 1 },
      "timestamp": { "type": "string", "format": "date-time" },
      "llvm_version": { "type": "integer" },
      "alive2_commit": { "type": "string" },
      "mode": { "enum": ["curated", "full-sweep", "single-file"] },
      "summary": {
        "type": "object",
        "required": ["verified", "failed", "timeout", "suppressed", "inlined", "total"],
        "properties": {
          "verified": { "type": "integer" },
          "failed": { "type": "integer" },
          "timeout": { "type": "integer" },
          "suppressed": { "type": "integer" },
          "inlined": { "type": "integer" },
          "total": { "type": "integer" }
        }
      },
      "functions": {
        "type": "array",
        "items": {
          "type": "object",
          "required": ["file", "function", "status"],
          "properties": {
            "file": { "type": "string" },
            "function": { "type": "string" },
            "status": { "enum": ["verified", "failed", "timeout", "suppressed", "suppression-stale", "inlined"] },
            "suppression_category": { "type": "string" },
            "duration_seconds": { "type": "number" },
            "alive2_output": { "type": "string" }
          }
        }
      }
    }
  }
  ```
  **WHERE:** `tests/alive2/results-schema.json` (new file)

- [x] Implement `--json` output in `diagnostics/alive2-verify.sh` that writes to `build/alive2-results/results.json` conforming to the schema above. The script creates the `build/alive2-results/` directory and writes:
  - `results.json` — the structured results file
  - `*.preopt.ll` and `*.postopt.ll` — the captured IR pairs (for debugging failed verifications)

- [x] Add a validation step that checks `results.json` against the schema (using `python3 -c 'import jsonschema; ...'` or a simple jq-based structural check).

- [x] Document the contract in `tests/alive2/README.md`:
  - Schema version and compatibility guarantees
  - Directory layout: `build/alive2-results/{results.json, *.preopt.ll, *.postopt.ll}`
  - How Section 11 consumes: artifact upload in CI, nightly/weekly comparison
  - How Section 12 consumes: trend tracking across runs, regression detection

- [x] **Subsection close-out (09.6)** — MANDATORY before starting 09.R:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 09.6: no tooling gaps. JSON output verified end-to-end with python3. Schema in results-schema.json. Contract documented in README.
  - [x] **Repo hygiene check** — clean (2026-04-13).

---

## 09.R Third Party Review Findings

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

- [x] `[TPR-09-001-codex][high]` `section-09-alive2.md:242` — LEAK: Keep `verify_optimize_emit` as the only optimization pipeline owner.
  Resolved: Fixed on 2026-04-13. Rewrote 09.2 to use `CaptureHooks` callback approach — pipeline orchestration stays in `ori_llvm`, IO hooks injected from `oric`.
- [x] `[TPR-09-002-codex][medium]` `section-09-alive2.md:329` — GAP: Pass bare function names to alive-tv (without `@`).
  Resolved: Fixed on 2026-04-13. Updated 09.3 alive-tv invocation to strip leading `@` with explicit note.
- [x] `[TPR-09-003-codex][medium]` `section-09-alive2.md:223` — DRIFT: Unify capture dir and results dir.
  Resolved: Fixed on 2026-04-13. Unified to `build/alive2-results/` throughout (09.2, 09.5, 09.6).
- [x] `[TPR-09-004-codex][medium]` `section-09-alive2.md:295` — GAP: Define `--all-codegen` mode before weekly job uses it.
  Resolved: Fixed on 2026-04-13. Added `--all-codegen` to 09.3's CLI interface with discovery rules.
- [x] `[TPR-09-005-codex][medium]` `section-11-ci-integration.md:127` — DRIFT: Reconcile Section 11 vs 09 workflow topology.
  Resolved: Fixed on 2026-04-13. Added cross-reference note in 09.5 that weekly job is provisional; Section 11 owns final topology.
- [x] `[TPR-09-006-codex][medium]` `section-09-alive2.md:128` — DRIFT: Export pinned Alive2 commit into CI env.
  Resolved: Fixed on 2026-04-13. Added "Extract Alive2 pin for cache key" CI step that reads ALIVE2_COMMIT from build script and writes to GITHUB_ENV.
- [x] `[TPR-09-001-gemini][high]` `section-09-alive2.md:340` — Replace `optnone` with `noinline` for survival injection.
  Resolved: Fixed on 2026-04-13. Changed both corpus metadata and verification script to use `noinline`, with explicit warning against `optnone`.
- [x] `[TPR-09-002-gemini][high]` `section-09-alive2.md:242` — Do not duplicate verify_optimize_emit pipeline in oric.
  Resolved: Fixed on 2026-04-13. Same fix as [TPR-09-001-codex] — CaptureHooks approach keeps pipeline in ori_llvm (agreement on architecture).
- [x] `[TPR-09-003-gemini][high]` `section-09-alive2.md:114` — Use dynamic Z3 path instead of hardcoded Ubuntu path.
  Resolved: Fixed on 2026-04-13. Replaced hardcoded path with `pkg-config` + CMake `FindZ3` discovery.
- [x] `[TPR-09-004-gemini][medium]` `section-09-alive2.md:447` — Delegate weekly CI job definition to Section 11.
  Resolved: Fixed on 2026-04-13. Same fix as [TPR-09-005-codex] — weekly job provisional in 09, Section 11 owns topology.
- [x] `[TPR-09-005-gemini][medium]` `section-09-alive2.md:427` — Export ALIVE2_COMMIT to GITHUB_ENV for cache key.
  Resolved: Fixed on 2026-04-13. Same fix as [TPR-09-006-codex] — CI step extracts from build script.

**Iteration 2 findings (all resolved):**

- [x] `[TPR-09-007-codex][high]` `section-09-alive2.md:242` — 09.2 body still had pipeline-duplication code; CaptureHooks only in 09.R/09.N.
  Resolved: Fixed on 2026-04-13 (iter 2). Replaced old split-pipeline code block with CaptureHooks implementation in 09.2 body.
- [x] `[TPR-09-008-codex][medium]` `section-09-alive2.md:342` — noinline injection must happen at compile time, not post-capture.
  Resolved: Fixed on 2026-04-13 (iter 2). Updated corpus metadata and survival docs to specify CaptureHooks pre_opt path for noinline injection.
- [x] `[TPR-09-009-codex][medium]` `section-09-alive2.md:109` — Hardcoded Z3 paths still in cmake command despite claimed fix.
  Resolved: Fixed on 2026-04-13 (iter 2). Removed -DZ3_INCLUDE_DIR and -DZ3_LIBRARIES from cmake snippet; uses FindZ3 + llvm-config --prefix.
- [x] `[TPR-09-010-codex][low]` `section-09-alive2.md:139` — mktemp smoke-test variable not assigned.
  Resolved: Fixed on 2026-04-13 (iter 2). Assigned mktemp output to $tmpfile variable.
- [x] `[TPR-09-007-gemini][high]` `section-09-alive2.md:113` — Same as TPR-09-009-codex (hardcoded Z3 paths).
  Resolved: Fixed on 2026-04-13 (iter 2). Same fix as [TPR-09-009-codex].
- [x] `[TPR-09-008-gemini][high]` `section-09-alive2.md:242` — Same as TPR-09-007-codex (pipeline duplication in body).
  Resolved: Fixed on 2026-04-13 (iter 2). Same fix as [TPR-09-007-codex].

**Iteration 3 findings (all resolved):**

- [x] `[TPR-09-011-codex][medium]` `section-09-alive2.md:349` — No transport mechanism for `[noinline]` corpus entries to reach compiler hooks.
  Resolved: Fixed on 2026-04-13 (iter 3). Removed noinline corpus metadata entirely; corpus entries must naturally survive -O2. --check-survival is the enforcement mechanism. Simplest correct design — no phantom transport needed.
- [x] `[TPR-09-012-codex][medium]` `section-09-alive2.md:233` — Duplicate basenames in build/alive2-results/ from different directories.
  Resolved: Fixed on 2026-04-13 (iter 3). Changed capture_path() to encode repo-relative path (/ -> _) for guaranteed uniqueness.

**Iteration 4 findings (all resolved):**

- [x] `[TPR-09-013-codex][medium]` `section-09-alive2.md:286` — Multi-file capture needs source_path threading + CaptureHooks re-export.
  Resolved: Fixed on 2026-04-13 (iter 4). Added source_path threading to emit_module_artifact and CaptureHooks re-export via ori_llvm::aot.
- [x] `[TPR-09-014-codex][medium]` `section-09-alive2.md:426` — Nightly workflow missing Alive2 path filters for PR triggers.
  Resolved: Fixed on 2026-04-13 (iter 4). Added PR path filter update task for build-alive2.sh, alive2-verify.sh, tests/alive2/**.
- [x] `[TPR-09-013-gemini][high]` `section-09-alive2.md:210` — .replace(".ori","") corrupts paths with .ori in directory names.
  Resolved: Fixed on 2026-04-13 (iter 4). Changed to Path::with_extension("") for safe suffix-only stripping.
- [x] `[TPR-09-014-gemini][medium]` `section-09-alive2.md:115` — cmake missing explicit build directory (in-source build fails).
  Resolved: Fixed on 2026-04-13 (iter 4). Changed to `cmake -B build -S .` with `ninja -C build` for modern out-of-source build.

---

## 09.N Completion Checklist

- [ ] `alive-tv` binary builds reproducibly via `scripts/build-alive2.sh` from pinned commit
- [ ] `scripts/build-alive2.sh` validates LLVM version matches pinned expectation (LLVM 21)
- [ ] `ORI_DUMP_PREOPT_LLVM`, `ORI_DUMP_POSTOPT_LLVM`, and `ORI_ALIVE2_CAPTURE` registered in `compiler/oric/src/debug_flags.rs`
- [ ] IR capture uses `CaptureHooks` callback approach — pipeline orchestration stays in `verify_optimize_emit` (ori_llvm SSOT), IO hooks injected from `oric`
- [ ] `ir_capture.rs` helper in `oric` constructs hook closures (NOT pipeline duplication)
- [ ] `ORI_ALIVE2_CAPTURE=1` produces `.preopt.ll` (after verify, before opt) and `.postopt.ll` (after opt, before emit) in `build/alive2-results/`
- [ ] Z3 discovery uses `pkg-config` / CMake `FindZ3` — no hardcoded platform-specific paths
- [ ] Function names passed to alive-tv WITHOUT `@` prefix (stripped during extraction)
- [ ] `diagnostics/alive2-verify.sh` passes `--help` and follows diagnostic conventions
- [ ] `--check-survival` flag detects functions inlined away by optimization
- [ ] `--all-codegen` flag discovers and verifies all `.ori` files in `compiler/ori_llvm/tests/codegen/`
- [ ] Corpus entries all naturally survive `-O2` (no noinline injection — replaced with survival-mandatory policy)
- [ ] Capture filenames encode repo-relative paths to prevent basename collisions across test directories
- [ ] Curated corpus in `tests/alive2/curated-corpus.txt` with >=15 functions, all surviving `-O2`
- [ ] All curated corpus functions pass alive-tv refinement checking
- [ ] Suppression file `tests/alive2/suppressed.json` documents all false positives with 9 categories
- [ ] `--review-suppressions` identifies stale suppressions
- [ ] Machine-readable `build/alive2-results/results.json` conforms to `tests/alive2/results-schema.json`
- [ ] Schema version field enables forward-compatible consumption by Sections 11/12
- [ ] CI step extracts `ALIVE2_COMMIT` from `scripts/build-alive2.sh` and exports to `$GITHUB_ENV` for cache key
- [ ] Nightly CI job (`alive2-curated` in `nightly-verification.yml`) runs curated corpus with zero failures
- [ ] Weekly CI job (`alive2-full-sweep`) defined — provisionally in `nightly-verification.yml`, Section 11 owns final topology
- [ ] Script added to `diagnostics/self-test.sh`
- [ ] No existing tests regressed: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 09` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` -> `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for this section
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed -- AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** -- verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** `diagnostics/alive2-verify.sh --corpus` passes with zero unresolved failures on the curated corpus of >=15 pure/arithmetic functions that survive `-O2` optimization. All false positives are documented in the suppression file with 9 categories and rationale. Nightly CI runs the curated corpus; weekly CI runs the full `compiler/ori_llvm/tests/codegen/` set — both within the existing `nightly-verification.yml` workflow. The `alive-tv` binary is built from a pinned commit and cached in CI. Pre-opt IR is captured AFTER module verification (producing valid IR for alive-tv) and post-opt IR is captured AFTER optimization (before emission). All capture logic lives in `oric` per phase purity. Machine-readable results in `build/alive2-results/results.json` conform to a versioned schema consumed by Sections 11 and 12. No capture logic was added to `passes/mod.rs` (438 lines, approaching limit).
