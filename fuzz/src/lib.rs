//! Differential oracle fuzzing infrastructure (`ori-fuzz`).
//!
//! Per the §10 plan (`plans/llvm-verification-tooling/section-10-differential-fuzzing.md`),
//! this crate hosts the coverage-guided fuzz targets for the Ori compiler.
//!
//! Architectural split (constraint #1 of §10):
//!
//! - **In-process eval pipeline.** `ori_lexer` → `ori_parse` → `ori_types` →
//!   `ori_canon` → `ori_eval` are linked directly so libFuzzer's
//!   `SanitizerCoverage` instrumentation drives mutation against real
//!   coverage feedback.
//! - **Subprocess AOT compilation.** `ori_llvm` is INTENTIONALLY absent from
//!   `[dependencies]` — its internal LLVM instrumentation is incompatible
//!   with libFuzzer instrumentation in the same binary. The differential
//!   target spawns the workspace `ori` binary as a subprocess (rebuilt once
//!   per process via the `OnceLock<cargo build>` pattern in `aot_binary`).
//!
//! Module roles (each named after the §10.X subsection that fills it in):
//!
//! - `r#gen` — typed Ori program generator (filled in §10.2). Module file is
//!   `src/gen.rs` per the §10 plan; the `r#gen` raw identifier is required
//!   because `gen` is a reserved keyword in edition 2024.
//! - [`oracle`] — differential oracle comparing eval vs AOT (filled in §10.4).
//! - [`canonicalize`] — stdout / exit-category canonicalization (filled in §10.4).
//! - [`aot_binary`] — `OnceLock<cargo build>` AOT-binary discovery (filled in
//!   §10.1; duplicated from `compiler_repo/compiler/ori_llvm/tests/aot/util/binary.rs`
//!   pending the §10.C extraction-to-shared-dev-dep cleanup item).
//! - [`cleanup`] — `Drop` scaffolding for tempfile lifecycle (filled in §10.5).

pub mod aot_binary;
pub mod canonicalize;
pub mod cleanup;
pub mod r#gen;
pub mod oracle;
