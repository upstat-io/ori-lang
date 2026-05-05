//! Seed corpus writer (TODO: §10.5).
//!
//! Per §10.5 of `plans/llvm-verification-tooling/section-10-differential-fuzzing.md`,
//! this binary will walk `compiler_repo/tests/spec/` and emit a deterministic
//! seed corpus into `compiler_repo/fuzz/corpus/<target>/` so libFuzzer
//! bootstraps coverage from spec-conformant programs.
//!
//! Stub at §10.1 — no work yet.

use std::process::ExitCode;

fn main() -> ExitCode {
    // TODO: §10.5 — corpus seeding from tests/spec/.
    ExitCode::SUCCESS
}
