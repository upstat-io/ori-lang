//! FNV-1a hash algorithm constants — canonical definition.
//!
//! All compiler-side consumers (`ori_eval`, `ori_llvm`, `ori_patterns`) import from here.
//! Spec: FNV-1a 64-bit — <http://www.isthe.com/chongo/tech/comp/fnv/>
//!
//! NOTE: `ori_rt` intentionally does NOT import these — it has no production
//! dependency on `ori_ir`. Its copy in `string/ops.rs` is kept in sync by a
//! dev-only conformance test (see `ori_rt` tests).

/// FNV-1a 64-bit offset basis.
pub const FNV_OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;

/// FNV-1a 64-bit prime multiplier.
pub const FNV_PRIME: u64 = 1_099_511_628_211;
