//! Tempfile cleanup scaffolding (TODO: §10.5).
//!
//! Per §10.5 of `plans/llvm-verification-tooling/section-10-differential-fuzzing.md`,
//! libFuzzer's worker is long-running — generated `.ori` source files,
//! compiled subprocess binaries, and intermediate object files accumulate
//! across iterations unless explicitly released. This module provides a
//! `Drop`-based RAII guard that ensures every per-iteration `tempfile`
//! allocation is released before the next input.
//!
//! Stub at §10.1 — no public surface yet. RAII guard implementation lands in
//! §10.5.
