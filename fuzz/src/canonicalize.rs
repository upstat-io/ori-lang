//! Stdout and exit-category canonicalization (TODO: §10.4).
//!
//! Per §10.4 of `plans/llvm-verification-tooling/section-10-differential-fuzzing.md`,
//! this module normalizes raw stdout before equality comparison: NaN
//! representation, pointer-print output, hash-set / hash-map iteration order,
//! `Duration` formatting, and any other format-divergent surface (constraint #5).
//!
//! Stub at §10.1 — no public surface yet. Canonicalization implementation lands
//! in §10.4.
