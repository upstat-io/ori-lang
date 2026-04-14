---
section: "10"
title: "Differential Oracle Fuzzing"
status: not-started
reviewed: false
goal: "Build a coverage-guided fuzzer that generates random typed Ori programs, executes them through both the interpreter (eval) and LLVM backend, and compares stdout output plus ORI_CHECK_LEAKS results — accumulating ≥24h of clean fuzzing with zero unresolved divergences"
success_criteria:
  - "fuzz/ directory exists in workspace with fuzz_targets/ori_differential.rs, fuzz_targets/ori_parse.rs, fuzz_targets/ori_typecheck.rs"
  - "Typed Ori program generator produces syntactically and type-correct programs constrained to the eval∩LLVM-supported subset"
  - "Differential fuzzer compares eval vs LLVM stdout output and exit codes"
  - "ORI_CHECK_LEAKS=1 comparison catches memory leak divergences between backends"
  - "Seed corpus from existing test files bootstraps coverage"
  - "≥24h cumulative fuzzing with zero unresolved divergences"
inspired_by:
  - "cargo-fuzz (~/projects/reference_repos/verification_tools/cargo-fuzz/) — Rust wrapper around libFuzzer for coverage-guided fuzzing"
  - "libfuzzer-sys (~/projects/reference_repos/verification_tools/libfuzzer/) — Rust bindings for LLVM's libFuzzer"
  - "Csmith (random C program generator) — structured random program generation for differential testing"
  - "CReduce + llvm-reduce — test case reduction for found divergences"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "10.1"
    title: "Fuzz Directory and Cargo-Fuzz Setup"
    status: not-started
  - id: "10.2"
    title: "Typed Ori Program Generator"
    status: not-started
  - id: "10.3"
    title: "Parse and Typecheck Fuzz Targets"
    status: not-started
  - id: "10.4"
    title: "Differential Oracle Fuzz Target"
    status: not-started
  - id: "10.5"
    title: "Seed Corpus and Fuzzing Campaigns"
    status: not-started
  - id: "10.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "10.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 10: Differential Oracle Fuzzing

**Status:** Not Started
**Goal:** Build coverage-guided fuzzers that test the Ori compiler at three levels: parse (crash-finding), typecheck (crash-finding), and differential oracle (eval vs LLVM semantic equivalence). The differential oracle is the world-class component — Ori's dual-layer architecture (interpreter + LLVM backend sharing the same `CanExpr` IR) enables differential testing that most compilers cannot do. A random Ori program that produces different stdout output from the interpreter vs LLVM backend is a compiler bug. Combined with `ORI_CHECK_LEAKS=1`, this also catches memory leak divergences.

**Success Criteria:**

- [ ] Three fuzz targets operational — satisfies mission criterion: "Differential oracle fuzzing"
- [ ] Program generator constrained to eval-LLVM intersection — satisfies mission criterion: "no false divergences from backend-specific features"
- [ ] Seed corpus bootstraps coverage — satisfies mission criterion: "effective fuzzing from day one"
- [ ] ≥24h clean fuzzing — satisfies mission criterion: "24h+ cumulative fuzzing with zero unresolved divergences"

**Context:** Most compilers can only fuzz for crashes — they have one backend, so there is no oracle for "correct output." Ori is different: the interpreter (ori_eval) and LLVM backend (ori_llvm) both consume `CanExpr` from the type checker, but execute it through completely different code paths. If both paths produce the same output for all inputs, confidence is high. If they diverge, one has a bug. This is the exact pattern used by Csmith (which found hundreds of GCC/LLVM miscompiles by comparing multiple C compilers) — but Ori has the advantage of two backends in one compiler, eliminating external dependency drift.

**CRITICAL CONSTRAINT**: The program generator must be constrained to the intersection of eval-supported and LLVM-supported features. The `ori_registry` `backend_required` field marks which methods have LLVM implementations — methods where `backend_required: false` (e.g., some Duration/Size factory methods, associated functions) exist only in eval/typeck and would cause false divergences if generated. Similarly, Range methods and some Error methods may lack LLVM coverage. The generator must consult `ori_registry` or use a hardcoded allowlist derived from it.

**Reference implementations:**
- **cargo-fuzz** `~/projects/reference_repos/verification_tools/cargo-fuzz/`: Rust CLI tool wrapping LLVM's libFuzzer. Creates `fuzz/` directory with `Cargo.toml` and `fuzz_targets/`. Requires nightly Rust for `-Zsanitizer` flags.
- **libfuzzer-sys** `~/projects/reference_repos/verification_tools/libfuzzer/`: Rust crate providing `fuzz_target!` macro that interfaces with libFuzzer's coverage feedback.
- **Csmith**: Random C program generator — Ori's generator follows the same philosophy (generate typed, well-formed programs) but for a simpler language.

**Depends on:** Section 01 (verifier gates active during fuzzing catches more issues — a divergence that also triggers a verifier failure is easier to diagnose).

---

## 10.1 Fuzz Directory and Cargo-Fuzz Setup

**File(s):** `fuzz/Cargo.toml`, `fuzz/fuzz_targets/`, `Cargo.toml` (workspace)

Set up the fuzzing infrastructure using cargo-fuzz conventions. The fuzz crate depends on compiler internals and must use nightly Rust for sanitizer instrumentation.

- [ ] Create `fuzz/Cargo.toml`:
  ```toml
  [package]
  name = "ori-fuzz"
  version = "0.0.0"
  publish = false
  edition = "2024"

  [package.metadata]
  cargo-fuzz = true

  [dependencies]
  libfuzzer-sys = "0.4"
  arbitrary = { version = "1", features = ["derive"] }
  # Compiler crates for parse/typecheck/eval/codegen
  ori_lexer = { path = "../compiler/ori_lexer" }
  ori_parse = { path = "../compiler/ori_parse" }
  ori_types = { path = "../compiler/ori_types" }
  ori_eval = { path = "../compiler/ori_eval" }
  ori_ir = { path = "../compiler/ori_ir" }

  # NOTE: ori_llvm is NOT a direct dependency — the differential
  # target compiles and runs the AOT binary as a subprocess to
  # avoid linking libFuzzer instrumentation with LLVM codegen.
  # This is the standard pattern for differential fuzzing with
  # cargo-fuzz.

  [[bin]]
  name = "ori_parse"
  path = "fuzz_targets/ori_parse.rs"
  doc = false

  [[bin]]
  name = "ori_typecheck"
  path = "fuzz_targets/ori_typecheck.rs"
  doc = false

  [[bin]]
  name = "ori_differential"
  path = "fuzz_targets/ori_differential.rs"
  doc = false
  ```

- [ ] Add `fuzz/` to workspace exclude list (cargo-fuzz convention — fuzz crates use nightly features not compatible with stable workspace builds):
  ```toml
  # In root Cargo.toml:
  [workspace]
  exclude = ["fuzz"]
  ```

- [ ] Create stub fuzz targets that compile but do minimal work (to verify the setup):
  - `fuzz/fuzz_targets/ori_parse.rs` — accepts arbitrary bytes, calls lexer+parser
  - `fuzz/fuzz_targets/ori_typecheck.rs` — accepts arbitrary bytes, calls parse+typecheck
  - `fuzz/fuzz_targets/ori_differential.rs` — accepts structured input, generates program, runs both backends

- [ ] Verify the setup with: `cd fuzz && cargo +nightly fuzz run ori_parse -- -max_total_time=10`

- [ ] **Subsection close-out (10.1)** — MANDATORY before starting 10.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging journey for 10.1 specifically: nightly Rust setup issues, cargo-fuzz version compatibility, workspace exclude behavior. Implement every accepted improvement NOW and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(fuzz): ...`).
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 10.2 Typed Ori Program Generator

**File(s):** `fuzz/src/gen.rs`, `fuzz/src/lib.rs`

The core of the differential fuzzer: a structured program generator that produces syntactically valid, type-correct Ori programs. The generator uses `arbitrary::Unstructured` to make fuzzer-guided random choices, producing programs that exercise different code paths without wasting time on parse/type errors.

- [ ] Define the generation strategy. The generator builds an AST-like structure and serializes to Ori source text:
  ```rust
  pub struct ProgramGenerator<'a> {
      u: &'a mut arbitrary::Unstructured<'a>,
      /// Variable environment: name -> type
      vars: Vec<(String, OriType)>,
      /// Depth counter to prevent unbounded recursion
      depth: usize,
      max_depth: usize,
  }

  #[derive(Clone, Debug)]
  pub enum OriType {
      Int, Float, Bool, Str, Void,
      List(Box<OriType>),
      Option(Box<OriType>),
      Tuple(Vec<OriType>),
  }
  ```

- [ ] Implement expression generation with type-directed choices:
  - **Literals**: `int` (random i64), `float` (random f64, excluding NaN/Inf edge cases initially), `bool`, `str` (random ASCII to avoid UTF-8 edge cases)
  - **Arithmetic**: `+`, `-`, `*`, `/`, `%` on `int`/`float` — avoid division by zero (guard with `if`)
  - **Comparisons**: `==`, `!=`, `<`, `>`, `<=`, `>=` on `int`/`float`/`str`
  - **Boolean ops**: `&&`, `||`, `!` on `bool`
  - **Let bindings**: `let x = expr` — adds to variable environment
  - **If-then-else**: `if cond then expr1 else expr2` — both branches same type
  - **Function definitions**: `@f (x: int) -> int = body` — simple single-argument functions
  - **Function calls**: call previously defined functions
  - **Lists**: `[1, 2, 3]` with `.len()`, `.is_empty()`, indexing
  - **Match**: `match expr { pattern -> body }` on simple types
  - **String operations**: `.len()`, `.is_empty()`, `+` concatenation
  - **Print**: `print(msg: to_str)` — the observable output compared between backends

- [ ] **CRITICAL**: Constrain to eval-LLVM intersection. The generator must NOT produce:
  - Methods where `ori_registry` `backend_required: false` — these lack LLVM implementations
  - Range methods that are eval-only (check registry)
  - Error construction/methods without LLVM coverage
  - `Duration`/`Size` factory associated functions (`from_nanoseconds`, etc.) — check registry
  - Capabilities (`uses Http`, etc.) — not available in AOT
  - `extern` FFI calls
  - `Suspend`/`parallel`/`spawn`/`nursery` — concurrency not in LLVM backend
  - Floating-point operations that produce NaN (comparison semantics may differ)
  Maintain a hardcoded allowlist of types and methods derived from `ori_registry` where `backend_required: true`, updated when the registry changes.

- [ ] Add depth limiting (max_depth = 8-10) and size limiting (max program ~200 lines) to prevent timeouts in the compiler.

- [ ] Add tests for the generator itself:
  - `test_generator_produces_valid_syntax` — parse the output, verify no errors
  - `test_generator_produces_typeable_programs` — typecheck the output, verify no errors (note: this may have a nonzero failure rate since the generator is best-effort; that is acceptable — the fuzzer discards programs that fail parsing/typechecking)
  - `test_generator_respects_depth_limit`
  - `test_generator_avoids_backend_only_features`

- [ ] **TPR checkpoint** — `/tpr-review` covering 10.1–10.2 implementation work

- [ ] **Subsection close-out (10.2)** — MANDATORY before starting 10.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 10.1's close-out, scoped to 10.2's debugging journey. Commit improvements separately using a valid conventional-commit type.
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 10.3 Parse and Typecheck Fuzz Targets

**File(s):** `fuzz/fuzz_targets/ori_parse.rs`, `fuzz/fuzz_targets/ori_typecheck.rs`

These are the simpler fuzz targets that find crashes (panics, OOM, infinite loops) in the parser and type checker. They accept raw bytes as input, maximizing coverage through libFuzzer's mutation engine.

- [ ] Implement the parse fuzz target:
  ```rust
  #![no_main]
  use libfuzzer_sys::fuzz_target;

  fuzz_target!(|data: &[u8]| {
      // Try to interpret as UTF-8; skip if invalid
      let source = match std::str::from_utf8(data) {
          Ok(s) => s,
          Err(_) => return,
      };

      // Parser should NEVER panic on any input — errors are returned
      let _ = ori_parse::parse_source(source);
  });
  ```
  The key property: the parser must handle ALL inputs without panicking. Parse errors are expected and returned as `Result` — panics are bugs.

- [ ] Implement the typecheck fuzz target:
  ```rust
  #![no_main]
  use libfuzzer_sys::fuzz_target;

  fuzz_target!(|data: &[u8]| {
      let source = match std::str::from_utf8(data) {
          Ok(s) => s,
          Err(_) => return,
      };

      // Parse first — skip if parse fails (not interesting for typeck fuzzing)
      let ast = match ori_parse::parse_source(source) {
          Ok(ast) => ast,
          Err(_) => return,
      };

      // Type checker should NEVER panic on any parseable input
      let _ = ori_types::check_program(&ast);
  });
  ```
  Note: the exact API for `ori_parse::parse_source` and `ori_types::check_program` will need to be adapted to the actual public API. These are illustrative — the implementation must use the real entry points (likely via the Salsa database).

- [ ] Add `catch_unwind` around the target body to distinguish panics from other failures — libFuzzer treats panics as crashes, which is what we want, but wrapping provides better error messages in the fuzzing log.

- [ ] Create seed corpus directories:
  - `fuzz/corpus/ori_parse/` — populated from `tests/spec/**/*.ori` (valid programs)
  - `fuzz/corpus/ori_typecheck/` — populated from same source, filtered to parseable files

- [ ] **Subsection close-out (10.3)** — MANDATORY before starting 10.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 10.4 Differential Oracle Fuzz Target

**File(s):** `fuzz/fuzz_targets/ori_differential.rs`

The flagship fuzz target: generates a random typed Ori program, executes it through both backends, and compares results. This is where Ori's dual-layer architecture creates verification capabilities that single-backend compilers cannot achieve.

- [ ] Implement the differential fuzz target:
  ```rust
  #![no_main]
  use libfuzzer_sys::fuzz_target;
  use arbitrary::Arbitrary;
  use ori_fuzz::gen::ProgramGenerator;

  #[derive(Debug, Arbitrary)]
  struct FuzzInput {
      seed: Vec<u8>,
  }

  fuzz_target!(|input: FuzzInput| {
      let mut u = arbitrary::Unstructured::new(&input.seed);
      let mut gen = ProgramGenerator::new(&mut u, /* max_depth */ 8);

      // 1. Generate a typed Ori program
      let source = match gen.generate_program() {
          Ok(s) => s,
          Err(_) => return, // Generator ran out of entropy
      };

      // 2. Write to temp file
      let tmp = tempfile::NamedTempFile::with_suffix(".ori").unwrap();
      std::fs::write(tmp.path(), &source).unwrap();

      // 3. Locate the ori binary via ORI_BIN env var (set during setup —
      //    e.g. `export ORI_BIN=$(cargo bin-path ori)` or point to a
      //    pre-built binary). env!("CARGO_BIN_EXE_ori") does NOT work
      //    from fuzz crates since ori is not a [[bin]] dependency.
      let ori_bin = std::env::var("ORI_BIN")
          .expect("ORI_BIN env var must be set to the ori binary path");

      // 4. Run through interpreter (eval)
      let eval_result = std::process::Command::new(&ori_bin)
          .args(["run", tmp.path().to_str().unwrap()])
          .env("ORI_CHECK_LEAKS", "1")
          .output();

      // 5. Run through LLVM backend (AOT compile + execute)
      let build_dir = tempfile::tempdir().unwrap();
      let binary_path = build_dir.path().join("fuzz_prog");
      let llvm_compile = std::process::Command::new(&ori_bin)
          .args(["build", tmp.path().to_str().unwrap(),
                 "-o", binary_path.to_str().unwrap()])
          .output();

      let llvm_result = if let Ok(compile_out) = llvm_compile {
          if compile_out.status.success() {
              std::process::Command::new(&binary_path)
                  .env("ORI_CHECK_LEAKS", "1")
                  .output()
                  .ok()
          } else {
              // LLVM compile failures are actionable artifacts — log and track them.
              // They indicate generator constraint gaps or genuine backend bugs.
              // Only programs rejected before typecheck may be silently skipped.
              eprintln!(
                  "LLVM_COMPILE_FAIL: {}\nstderr: {}",
                  tmp.path().display(),
                  String::from_utf8_lossy(&compile_out.stderr),
              );
              return;
          }
      } else {
          return;
      };

      // 6. Compare results
      let eval_out = match eval_result {
          Ok(r) => r,
          Err(_) => return,
      };
      let llvm_out = match llvm_result {
          Some(r) => r,
          None => return,
      };

      // Both must produce the same stdout
      assert_eq!(
          eval_out.stdout, llvm_out.stdout,
          "STDOUT DIVERGENCE!\nSource:\n{source}\n\
           Eval: {:?}\nLLVM: {:?}",
          String::from_utf8_lossy(&eval_out.stdout),
          String::from_utf8_lossy(&llvm_out.stdout),
      );

      // Both must produce the same exit code
      assert_eq!(
          eval_out.status.code(), llvm_out.status.code(),
          "EXIT CODE DIVERGENCE!\nSource:\n{source}\n\
           Eval: {:?}\nLLVM: {:?}",
          eval_out.status, llvm_out.status,
      );

      // Check for leak divergences in stderr
      // ORI_CHECK_LEAKS=1 output goes to stderr
      let eval_leaks = extract_leak_count(&eval_out.stderr);
      let llvm_leaks = extract_leak_count(&llvm_out.stderr);
      assert_eq!(
          eval_leaks, llvm_leaks,
          "LEAK DIVERGENCE!\nSource:\n{source}\n\
           Eval leaks: {eval_leaks}\nLLVM leaks: {llvm_leaks}",
      );
  });
  ```

- [ ] Implement `extract_leak_count(stderr: &[u8]) -> Option<usize>` that parses the `ORI_CHECK_LEAKS` output format from stderr. If leak reporting is not present (program exited before leak check), return `None` — divergences where one side reports leaks and the other does not are also flagged.

- [ ] Handle timeout for generated programs. Set a 5-second timeout on both eval and AOT execution:
  ```rust
  use std::time::Duration;
  // Use Command::timeout() or a wrapper that kills after 5s
  ```
  Programs that timeout on either backend are skipped (not divergences — the generator may produce infinite loops).

- [ ] Handle panic output. Ori programs may `panic()` — this is valid behavior. The divergence check must account for:
  - Both panic with the same message: OK (not a divergence)
  - One panics, the other does not: DIVERGENCE (critical bug)
  - Both panic with different messages: possible divergence (worth investigating)

- [ ] **TPR checkpoint** — `/tpr-review` covering 10.3–10.4 implementation work

- [ ] **Subsection close-out (10.4)** — MANDATORY before starting 10.5:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 10.5 Seed Corpus and Fuzzing Campaigns

**File(s):** `fuzz/corpus/`, `scripts/populate-fuzz-corpus.sh`, `.github/workflows/ci.yml` (weekly job)

Bootstrap the fuzzer with existing test programs and run initial fuzzing campaigns to validate the infrastructure.

- [ ] Create `scripts/populate-fuzz-corpus.sh` that copies existing test files into the corpus directories:
  ```bash
  # Parse corpus: all .ori files
  find tests/spec/ -name '*.ori' -exec cp {} fuzz/corpus/ori_parse/ \;
  # Typecheck corpus: same
  find tests/spec/ -name '*.ori' -exec cp {} fuzz/corpus/ori_typecheck/ \;
  # Differential corpus: only files that compile successfully with both backends
  # (requires building each — run as a batch job)
  ```

- [ ] For the differential fuzzer, also generate seed inputs from the program generator. Run the generator with deterministic seeds (0-999) and save the generated programs as corpus entries. This gives libFuzzer a diverse starting corpus of well-typed programs.

- [ ] Run initial fuzzing campaigns locally to validate:
  - `cd fuzz && cargo +nightly fuzz run ori_parse -- -max_total_time=3600` (1 hour)
  - `cd fuzz && cargo +nightly fuzz run ori_typecheck -- -max_total_time=3600`
  - `cd fuzz && cargo +nightly fuzz run ori_differential -- -max_total_time=3600`
  - Any crashes found during these initial campaigns are bugs — file via `/add-bug` and fix before proceeding.

- [ ] Add CI job for weekly fuzzing (fuzzing is too expensive for every-commit or nightly):
  ```yaml
  fuzz:
    name: Fuzzing (Weekly)
    runs-on: ubuntu-latest
    timeout-minutes: 180  # 3 hours total
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - name: Install cargo-fuzz
        run: cargo +nightly install cargo-fuzz
      - name: Install LLVM 21
        run: # ... standard LLVM install
      - name: Build compiler
        run: cargo build
      - name: Populate seed corpus
        run: ./scripts/populate-fuzz-corpus.sh
      - name: Fuzz parse (30 min)
        run: cd fuzz && cargo +nightly fuzz run ori_parse -- -max_total_time=1800
      - name: Fuzz typecheck (30 min)
        run: cd fuzz && cargo +nightly fuzz run ori_typecheck -- -max_total_time=1800
      - name: Fuzz differential (1 hour)
        run: cd fuzz && cargo +nightly fuzz run ori_differential -- -max_total_time=3600
      - name: Upload crash artifacts
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: fuzz-crashes
          path: fuzz/artifacts/
  ```

- [ ] Document the expected fuzzing campaign durations and coverage goals:
  - **Target**: ≥24h cumulative fuzzing across all three targets with zero unresolved divergences
  - **Divergence triage protocol**: when a divergence is found, (1) save the minimal reproducing input, (2) determine which backend is wrong (usually by manual inspection or by checking against the spec), (3) file via `/add-bug`, (4) fix before counting the fuzzing time as "clean"

- [ ] **Subsection close-out (10.5)** — MANDATORY before starting 10.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 10.R Third Party Review Findings

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

## 10.N Completion Checklist

- [ ] `fuzz/` directory exists with `Cargo.toml` and three fuzz targets
- [ ] `fuzz/` excluded from workspace in root `Cargo.toml`
- [ ] Typed program generator produces valid, typeable Ori programs
- [ ] Generator constrained to eval-LLVM intersection (no `backend_required: false` methods)
- [ ] `ori_parse` fuzz target compiles and runs with cargo-fuzz
- [ ] `ori_typecheck` fuzz target compiles and runs with cargo-fuzz
- [ ] `ori_differential` fuzz target compiles, generates programs, and compares backends
- [ ] Seed corpus populated from existing test files
- [ ] `ORI_CHECK_LEAKS=1` comparison integrated into differential target
- [ ] Timeout handling prevents infinite-loop programs from hanging the fuzzer
- [ ] Panic divergence detection distinguishes expected panics from backend bugs
- [ ] ≥24h cumulative fuzzing with zero unresolved divergences across all three targets
- [ ] All crashes found during initial campaigns filed and fixed
- [ ] Weekly CI job runs all three fuzz targets
- [ ] Crash artifacts uploaded on CI failure
- [ ] No existing tests regressed: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 10` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for this section
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** All three fuzz targets (`ori_parse`, `ori_typecheck`, `ori_differential`) compile and run under `cargo +nightly fuzz run`. The program generator produces valid typed Ori programs constrained to the eval-LLVM feature intersection. The differential fuzzer compares stdout, exit code, and leak count between eval and LLVM backends. ≥24h cumulative clean fuzzing with zero unresolved divergences. Weekly CI job runs all three targets with crash artifact upload. All bugs discovered during fuzzing campaigns are filed and fixed.
