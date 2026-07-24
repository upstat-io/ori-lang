# `ori-fuzz` — Differential Oracle Fuzzing

Coverage-guided fuzz targets for the Ori compiler. Owned by
[`plans/llvm-verification-tooling/section-10-differential-fuzzing.md`](../../plans/llvm-verification-tooling/section-10-differential-fuzzing.md);
this README is the operator-facing surface.

## Architecture — Why two execution models

Constraint #1 of §10 (verbatim from the plan):

> libFuzzer's coverage feedback (compile-time SanitizerCoverage instrumentation)
> maps to symbols in the *current process only*. Subprocess invocations of
> the eval interpreter would reduce the fuzzer to a blind random generator —
> the very thing libFuzzer exists to avoid. Therefore the eval side runs
> IN-PROCESS via the `ori_compiler` / `ori_eval` Rust API. The AOT side
> MUST stay subprocess because linking libFuzzer's instrumentation with
> the LLVM backend (which is already heavily instrumented for its own
> purposes, and which `ori_llvm` is NOT a fuzz-crate dependency by design)
> is incompatible — both libraries assume they own coverage instrumentation.
> This split is non-negotiable.

Hence `Cargo.toml` deliberately omits `ori_llvm`. The differential target
spawns the workspace `ori` binary as a subprocess, discovered via the
`OnceLock<cargo build>` pattern in [`src/aot_binary.rs`](src/aot_binary.rs)
(duplicated from `compiler_repo/compiler/ori_llvm/tests/aot/util/binary.rs`
pending the §10.C extraction-to-shared-dev-dep cleanup item).

## Targets

| Target | Phase coverage | Source |
|--------|----------------|--------|
| `ori_parse` | lex + parse | `fuzz_targets/ori_parse.rs` |
| `ori_typecheck` | lex + parse + typecheck | `fuzz_targets/ori_typecheck.rs` |
| `ori_canon` | lex + parse + typecheck + canonicalize | `fuzz_targets/ori_canon.rs` |
| `ori_differential` | full eval vs AOT differential (§10.4 — stub today) | `fuzz_targets/ori_differential.rs` |

## Running

From the wrapper root:

```
cd compiler_repo/fuzz
cargo +nightly fuzz build
cargo +nightly fuzz run ori_parse -- -max_total_time=10
```

`cargo-fuzz` and a nightly Rust toolchain are required.

## ASan matrix

Per §10.1 sub-item:

| Path | ASan | Mechanism |
|------|------|-----------|
| In-process eval target | ON (always) | cargo-fuzz default — `RUSTFLAGS="-Cinstrument-coverage -Zsanitizer=address"`. Catches memory bugs in `ori_eval` / `ori_patterns` / `ori_rt` (when used by eval as a dev-dep). |
| AOT subprocess (default) | OFF | Throughput-optimized. The generated binary uses the release `ori_rt` static lib without ASan instrumentation. |
| AOT subprocess (triage) | ON via `FUZZ_AOT_ASAN=1` | Differential target sets `ORI_SANITIZE=address` in the subprocess env; AOT compile uses `libori_rt_asan.a` (per §08, already complete). Toggle for triage runs that need ASan signal across the process boundary. |

Workspace-build hygiene: `compiler_repo/Cargo.toml` excludes the `fuzz/`
directory from the workspace, so cargo-fuzz's nightly-only sanitizer flags
cannot bleed into the main workspace build.

## Companion binaries

| Binary | Role |
|--------|------|
| `parse-only-check` | Corpus-stage filter: succeeds iff lex + parse reports no errors. |
| `parse-typecheck-check` | Corpus-stage filter: succeeds iff lex + parse + typecheck reports no errors. Used by `populate-fuzz-corpus.sh` so the seed corpus runs through the same pipeline as the `ori_typecheck` / `ori_canon` fuzz targets. |
| `seed-writer` | Seed-corpus generator (stub today; full implementation lands in §10.5). |
