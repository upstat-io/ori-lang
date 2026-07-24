//! Typed Ori program generator (TODO: §10.2).
//!
//! Per §10.2 of `plans/llvm-verification-tooling/section-10-differential-fuzzing.md`,
//! this module hosts the registry-derived generator that produces syntactically
//! and type-correct programs constrained to the eval∩LLVM-supported subset.
//! The constraint set is materialized programmatically from `ori_registry`
//! (`find_type`, `methods_for`, `MethodDef.backend_required`) — never duplicated
//! as a hardcoded allowlist (constraint #3).
//!
//! Stub at §10.1 — no public surface yet. Generator implementation lands in §10.2.
