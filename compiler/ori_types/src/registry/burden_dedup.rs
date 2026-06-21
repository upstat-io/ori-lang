//! Structural-burden-signature dedup for user-defined and monomorphized
//! `UserBurdenSpec` entries.
//!
//! `BurdenSignature(u64)` is a deterministic structural hash over a
//! `UserBurdenSpec`. Two structurally equal specs hash to the same signature
//! (modulo hash collisions); two specs with different structural shape
//! ideally hash to different signatures. The signature is the reverse-index
//! key on [`crate::TypeRegistry::burden_by_signature`], enabling
//! `register_user_burden` to short-circuit insertion when an equivalent
//! spec already lives in the registry.
//!
//! Spec: Annex E §AIMS — burden specs are typed pre-pass input feeding the
//! lattice-driven analysis. Dedup keeps `TypeRegistry::burden` sub-linear
//! across the stdlib generic monomorphization surface (e.g., the 6×6
//! `Result<T, E>` cross-product collapses heavily once every `T`/`E` whose
//! own burden is empty (Value-trait) shares one entry under a future
//! lookup-through-graph refinement).
//!
//! Algorithm: FNV-1a 64-bit over a fixed-order field walk
//! (deterministic across runs). The walker is recursion-safe: a
//! `FxHashSet<Idx>` tracks `field_type` references already visited so
//! `Tree<T>`-shaped specs terminate in bounded time. On revisit, a fixed
//! sentinel ([`RECURSIVE_PLACEHOLDER`]) is folded into the hash instead
//! of recursing.
//!
//! Soundness gate (Debug/Release parity): the signature is a HASH, not a
//! unique identifier. Collisions must be tolerated identically in debug
//! and release builds. On a `register_user_burden` collision (matching
//! signature but structurally different specs), the registry rejects the
//! dedup and inserts the incoming spec under a NEW slot. The pre-existing
//! slot keeps its prior occupant. Collision tolerance is the canonical
//! path in both builds — NO `debug_assert!` may fire on collision mismatch.

use rustc_hash::FxHashSet;

use crate::registry::burden::{
    UserBorrowedField, UserBurdenSpec, UserOwnedField, UserTransferRule, UserVariantBurden,
};
use crate::Idx;

/// Sentinel hash mixed in for any `Idx` already visited during a recursive
/// signature walk. The exact constant does not matter for correctness — it
/// only matters that it differs from any plausible recursive child hash
/// and is itself fixed across runs.
pub(crate) const RECURSIVE_PLACEHOLDER: u64 = 0xCAFE_BABE_DEAD_BEEF;

/// FNV-1a 64-bit basis constant.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a 64-bit prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

/// Structural hash of a [`UserBurdenSpec`].
///
/// Two specs with structurally identical shape produce equal signatures
/// (modulo collisions). Used as the reverse-index key on
/// [`crate::TypeRegistry::burden_by_signature`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BurdenSignature(pub u64);

impl BurdenSignature {
    /// Compute the signature for a `UserBurdenSpec` using FNV-1a 64-bit
    /// over a fixed-order field walk.
    ///
    /// The walker is recursion-safe: child `Idx` references inside the
    /// spec are folded as their raw 32-bit values, and any recursive
    /// look-through to a child spec (via `fetch_child`) carries a `seen`
    /// set that substitutes [`RECURSIVE_PLACEHOLDER`] on revisit.
    ///
    /// Callers that want a single-level hash (no graph chase) pass a
    /// `fetch_child` returning `None` for every `Idx`. The dedup path in
    /// `TypeRegistry::register_user_burden` uses
    /// [`Self::compute_singlelevel`] for that case.
    #[must_use]
    pub fn compute<F>(spec: &UserBurdenSpec, fetch_child: &F) -> Self
    where
        F: Fn(Idx) -> Option<UserBurdenSpec>,
    {
        let mut h = FnvHasher::new();
        let mut seen = FxHashSet::default();
        fold_spec(&mut h, spec, &mut seen, fetch_child);
        Self(h.finish())
    }

    /// Convenience: compute the signature WITHOUT chasing child `Idx`
    /// references through any registry — child types contribute only
    /// their raw `Idx`. This is the path used by
    /// `TypeRegistry::register_user_burden` because composition produces
    /// flat specs (each child carries its own `Idx`, and codegen looks
    /// each one up separately at the consumption site).
    ///
    /// Single-level hashing is sound for dedup because:
    ///
    /// 1. Pool entries are structurally interned per `TYPES:TI-2` — two
    ///    structurally-distinct types (`Option<int>` vs `Option<str>`,
    ///    `Result<Option<int>, str>` vs `Result<Option<str>, str>`) have
    ///    distinct raw `Idx` values. Hashing the raw `Idx` discriminates
    ///    nested structural difference at every depth without recursive
    ///    walks — the pool's structural identity is load-bearing.
    /// 2. Signature is an index, not a unique identifier. False
    ///    positives (signature collision between structurally-distinct
    ///    specs) are rejected at `register_user_burden` by the
    ///    structural-equality check (`existing_spec == &spec`); the
    ///    incoming spec lands at a fresh slot, the prior owner keeps its
    ///    signature slot. Correctness floor is `PartialEq`, not the
    ///    hash function.
    ///
    /// Recursive variant (`compute` with a non-trivial `fetch_child`)
    /// is reserved for future call sites that want a hash distinguishing
    /// two outer types sharing identical child `Idx`s — not the dedup
    /// path today.
    #[must_use]
    pub fn compute_singlelevel(spec: &UserBurdenSpec) -> Self {
        Self::compute(spec, &|_| None)
    }
}

// === Hash folding ===

fn fold_spec<F>(
    h: &mut FnvHasher,
    spec: &UserBurdenSpec,
    seen: &mut FxHashSet<Idx>,
    fetch_child: &F,
) where
    F: Fn(Idx) -> Option<UserBurdenSpec>,
{
    h.write_u8(u8::from(spec.self_heap_alloc));

    h.write_usize(spec.owned_fields.len());
    for f in &spec.owned_fields {
        fold_owned_field(h, f, seen, fetch_child);
    }

    h.write_usize(spec.borrowed_fields.len());
    for f in &spec.borrowed_fields {
        fold_borrowed_field(h, f, seen, fetch_child);
    }

    h.write_usize(spec.variant_burdens.len());
    for v in &spec.variant_burdens {
        fold_variant_burden(h, v, seen, fetch_child);
    }

    match spec.element_burden {
        Some(idx) => {
            h.write_u8(1);
            fold_child_idx(h, idx, seen, fetch_child);
        }
        None => h.write_u8(0),
    }

    // compiled_drop / user_drop — folded as 0/1 presence markers plus
    // the raw FnSym value. FnSym wraps NonZeroU32, so 0 unambiguously
    // means "absent".
    match spec.compiled_drop {
        Some(sym) => {
            h.write_u8(1);
            h.write_u32(sym.get().get());
        }
        None => h.write_u8(0),
    }
    match spec.user_drop {
        Some(sym) => {
            h.write_u8(1);
            h.write_u32(sym.get().get());
        }
        None => h.write_u8(0),
    }
}

fn fold_owned_field<F>(
    h: &mut FnvHasher,
    f: &UserOwnedField,
    seen: &mut FxHashSet<Idx>,
    fetch_child: &F,
) where
    F: Fn(Idx) -> Option<UserBurdenSpec>,
{
    h.write_usize(f.field_path.len());
    for idx in &f.field_path {
        h.write_u32(*idx);
    }
    fold_child_idx(h, f.field_type, seen, fetch_child);
}

fn fold_borrowed_field<F>(
    h: &mut FnvHasher,
    f: &UserBorrowedField,
    seen: &mut FxHashSet<Idx>,
    fetch_child: &F,
) where
    F: Fn(Idx) -> Option<UserBurdenSpec>,
{
    h.write_usize(f.field_path.len());
    for idx in &f.field_path {
        h.write_u32(*idx);
    }
    fold_child_idx(h, f.field_type, seen, fetch_child);
}

fn fold_transfer_rule<F>(
    h: &mut FnvHasher,
    r: &UserTransferRule,
    seen: &mut FxHashSet<Idx>,
    fetch_child: &F,
) where
    F: Fn(Idx) -> Option<UserBurdenSpec>,
{
    h.write_usize(r.source_field_path.len());
    for idx in &r.source_field_path {
        h.write_u32(*idx);
    }
    h.write_u16(r.binding_index);
    fold_child_idx(h, r.field_type, seen, fetch_child);
    // TransferKind is a small enum — fold its discriminant via `as u8`.
    h.write_u8(r.transfer_kind as u8);
}

fn fold_variant_burden<F>(
    h: &mut FnvHasher,
    v: &UserVariantBurden,
    seen: &mut FxHashSet<Idx>,
    fetch_child: &F,
) where
    F: Fn(Idx) -> Option<UserBurdenSpec>,
{
    h.write_u32(v.variant_id.get().get());
    h.write_usize(v.transfers_on_match.len());
    for r in &v.transfers_on_match {
        fold_transfer_rule(h, r, seen, fetch_child);
    }
    h.write_usize(v.retained_owned.len());
    for f in &v.retained_owned {
        fold_owned_field(h, f, seen, fetch_child);
    }
}

/// Recursion-safe child fold: always writes the child's raw `Idx` so two
/// types with distinct identity hash differently, and additionally folds
/// the child's recursive signature on first visit. Revisits short-circuit
/// to [`RECURSIVE_PLACEHOLDER`] so `Tree<T>` and other cyclic shapes
/// terminate.
fn fold_child_idx<F>(h: &mut FnvHasher, idx: Idx, seen: &mut FxHashSet<Idx>, fetch_child: &F)
where
    F: Fn(Idx) -> Option<UserBurdenSpec>,
{
    h.write_u32(idx.raw());
    if seen.contains(&idx) {
        h.write_u64(RECURSIVE_PLACEHOLDER);
        return;
    }
    if let Some(child) = fetch_child(idx) {
        seen.insert(idx);
        fold_spec(h, &child, seen, fetch_child);
        seen.remove(&idx);
    }
    // No registered child spec — the raw `Idx` alone is the full
    // signature contribution at this position.
}

// === FNV-1a 64-bit hasher ===
//
// The compiler crate's workspace lints deny `unwrap_used` / `expect_used`,
// so we avoid relying on `std::hash::Hasher` blanket methods that may
// panic on misuse and roll a small, deterministic FNV-1a directly.

struct FnvHasher(u64);

impl FnvHasher {
    fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write_u8(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    #[inline]
    fn write_u16(&mut self, value: u16) {
        for b in value.to_le_bytes() {
            self.write_u8(b);
        }
    }

    #[inline]
    fn write_u32(&mut self, value: u32) {
        for b in value.to_le_bytes() {
            self.write_u8(b);
        }
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        for b in value.to_le_bytes() {
            self.write_u8(b);
        }
    }

    #[inline]
    fn write_usize(&mut self, value: usize) {
        // Fold as u64 for cross-target determinism.
        self.write_u64(value as u64);
    }
}

#[cfg(test)]
mod tests;
