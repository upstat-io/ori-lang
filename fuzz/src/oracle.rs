//! Differential oracle (TODO: §10.4).
//!
//! Per §10.4 of `plans/llvm-verification-tooling/section-10-differential-fuzzing.md`,
//! this module compares in-process `ori_eval` results against subprocess AOT
//! results using STRUCTURAL exit categories (`Ok`, `Panic { kind }`,
//! `CompileTimeout`, `ExecutionTimeout`, `OutOfMemory`, `BackendError { phase }`),
//! canonicalized stdout, and `ORI_CHECK_LEAKS` counts — never raw stderr strings
//! (constraint #4).
//!
//! Stub at §10.1 — no public surface yet. Oracle implementation lands in §10.4.
