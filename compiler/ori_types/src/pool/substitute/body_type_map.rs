//! Monomorphization body-type-map construction.
//!
//! The `(source_idx → substituted_idx)` map the ARC lowerer consumes to emit
//! type-specific retain/release/drop: [`build_mono_body_type_map`] (the
//! two-pass pool scan + scheme-var pre-intern), [`finalize_body_type_map`]
//! (the deterministic sort+dedup), [`build_finalized_body_type_map`] (the
//! `build → extend named → finalize` bookend SSOT), and
//! [`extend_var_subst_with_roots`] (the union-find-root substitution fix-up).

use rustc_hash::FxHashMap;

use super::substitute_in_pool;
use crate::{Idx, Pool, Tag, TypeFlags};

/// Sink for [`build_mono_body_type_map`] entries — abstracts over the
/// accumulator shape used by each monomorphization call site. `Vec<(Idx, Idx)>`
/// callers (typeck-side: `maybe_record_mono_instance`, `build_mono_instance`)
/// post-process with `sort_by_key` + `dedup_by_key` to land a deterministic
/// Salsa-friendly `MonoInstance.body_type_map`. The `FxHashMap<Idx, Idx>`
/// caller (`oric::test::runner::llvm_backend::run_file_llvm` imported-mono
/// construction) keys insertions directly because it feeds the LLVM-side
/// `MonoFunction.body_type_map` which already requires hash lookup.
pub trait BodyTypeMapSink {
    /// Record a `(source_idx → substituted_idx)` entry.
    fn record(&mut self, key: Idx, value: Idx);
}

impl BodyTypeMapSink for Vec<(Idx, Idx)> {
    fn record(&mut self, key: Idx, value: Idx) {
        self.push((key, value));
    }
}

impl<S: std::hash::BuildHasher> BodyTypeMapSink for std::collections::HashMap<Idx, Idx, S> {
    fn record(&mut self, key: Idx, value: Idx) {
        self.insert(key, value);
    }
}

/// Build a monomorphization body-type map in `sink` for the mono instance
/// identified by `var_subst`.
///
/// Two-pass structure mirrors the three shipped call sites (typeck
/// `maybe_record_mono_instance`, typeck `build_mono_instance`, test-runner
/// imported-mono construction) — the single SSOT for that map construction.
///
/// Pass 1 — existing-pool scan:
///   For every dynamic `Idx` (skip pre-interned primitives) whose
///   `TypeFlags::HAS_VAR | HAS_BOUND_VAR` is set, substitute via `var_subst`
///   and record when the result differs from the source. This covers
///   `sig`/`expr_types` positions carrying generic `Tag::Var` leaves from
///   pre-generalize inference AND normalization-rewritten
///   `Tag::BoundVar` leaves.
///
/// Pass 2 — scheme-var `BoundVar` pre-intern:
///   For every `(var_id, concrete_idx)` in `var_subst`, pre-intern
///   `Tag::BoundVar(var_id)` and record `(bound_idx → concrete_idx)` in
///   the sink. The end-of-body normalization pass
///   ([`crate::infer::InferEngine::normalize_body_generalized_to_bound_var`])
///   rewrites `Tag::Var(Generalized)` leaves to `Tag::BoundVar`
///   AFTER mono instances are recorded; without the
///   pre-intern, post-normalization `sig`/`expr_types` referencing
///   `Tag::BoundVar` would not be substituted at ARC lowering, leading to
///   `Tag::BoundVar` reaching codegen's `TypeInfoStore` as a defensive ICE
///   signal per AIMS Invariant 2.
///
/// Callers are responsible for any per-site post-processing: `Vec` sinks
/// reach the Salsa-deterministic shape via [`finalize_body_type_map`] (the
/// SSOT for that sort+dedup step); Applied-type resolution registration; etc.
#[expect(
    clippy::implicit_hasher,
    reason = "var_subst is consistently FxHashMap<u32, Idx> across the whole ori_types \
              crate (matches `substitute_in_pool`'s signature); generalizing would force \
              BuildHasher plumbing through every caller for no measurable benefit."
)]
pub fn build_mono_body_type_map<Sink: BodyTypeMapSink>(
    pool: &mut Pool,
    var_subst: &FxHashMap<u32, Idx>,
    sink: &mut Sink,
) {
    let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
    // HAS_RIGID_VAR included so a `var_subst` carrying impl-level rigid var_ids
    // (generic-impl methods, `impl<T> Box<T>`) records COMPOSITE entries for
    // rigid-containing body types (e.g. a `Pair<B, A>` ctor inside `swap`), not
    // just leaf rigids. The function path passes no rigid var_ids, so
    // rigid-containing types substitute to themselves and are not recorded.
    let mask = TypeFlags::HAS_VAR | TypeFlags::HAS_BOUND_VAR | TypeFlags::HAS_RIGID_VAR;
    for raw in Idx::FIRST_DYNAMIC..pool_len {
        let idx = Idx::from_raw(raw);
        if pool.flags(idx).intersects(mask) {
            let substituted = substitute_in_pool(pool, idx, var_subst);
            if substituted != idx {
                sink.record(idx, substituted);
            }
        }
    }
    for (&var_id, &concrete_idx) in var_subst {
        let bound_idx = pool.bound_var(var_id);
        if bound_idx != concrete_idx {
            sink.record(bound_idx, concrete_idx);
        }
    }
}

/// Canonicalize a `Vec`-sink `body_type_map` into the deterministic
/// Salsa-friendly shape: sort by source-key raw `Idx`, then dedup by the same
/// key. SSOT for the post-`build_mono_body_type_map` finalization step shared
/// across every `Vec` call site (eager typeck `maybe_record_mono_instance`,
/// deferred typeck `build_mono_instance` + `refresh_method_mono_body_type_maps`)
/// — the deterministic-shape invariant has exactly one home so Salsa's `Eq`
/// early cutoff sees a stable key order.
pub fn finalize_body_type_map(body_type_map: &mut Vec<(Idx, Idx)>) {
    body_type_map.sort_by_key(|(k, _)| k.raw());
    body_type_map.dedup_by_key(|(k, _)| k.raw());
}

/// Build + extend + finalize bookend: [`build_mono_body_type_map`] into a fresh
/// `Vec`, `extend_from_slice` the caller's already-resolved `(key, val)` named
/// entries, then [`finalize_body_type_map`]. Returns the finalized map.
///
/// SSOT for the `build -> (extend named) -> finalize` skeleton shared by the
/// three `Vec`-sink monomorphization sites (`build_mono_instance`,
/// `refresh_method_mono_body_type_maps`, `build_and_register_body_type_map`).
/// Each site keeps its own `var_subst` prep and its own
/// `register_concrete_applied_resolutions` tail SITE-LOCAL.
///
/// INVARIANT: pure build+extend+finalize — NEVER registers Applied->concrete
/// resolutions (`register_concrete_applied_resolutions` stays at the registering
/// call sites; `build_mono_instance` must NOT register).
pub fn build_finalized_body_type_map(
    pool: &mut Pool,
    var_subst: &FxHashMap<u32, Idx>,
    extra_named: &[(Idx, Idx)],
) -> Vec<(Idx, Idx)> {
    let mut map: Vec<(Idx, Idx)> = Vec::new();
    build_mono_body_type_map(pool, var_subst, &mut map);
    map.extend_from_slice(extra_named);
    finalize_body_type_map(&mut map);
    map
}

/// Extend `var_subst` with `{union_find_root_var_id → concrete}` entries for
/// every scheme var whose equivalence-class root differs from the scheme
/// var's own `var_id`.
///
/// Rank-weighted union-find can make a fresh instantiation
/// var the root of a scheme var's equivalence class. In that case,
/// `substitute_var` finds the scheme var's `var_id` via
/// direct map lookup, but walking a body type that carries the ROOT's
/// `Tag::Var(root_var_id)` leaf falls through to `VarState::Unbound` (the
/// root has no `Link` to follow) and returns unchanged. Adding the root's
/// `var_id` to the map ensures pool-walk visits of the root `Tag::Var`
/// entry find the concrete type through a direct hit.
///
/// **Semantics: preserve-existing** — mirrors the idempotent-set pattern of
/// `build_exempt_var_ids` in `check/validators/mod.rs`. Caller-supplied
/// map entries (declared scheme-var → concrete) are authoritative and MUST
/// NOT be overwritten; the helper only ADDS root-var entries that were not
/// already keys.
///
/// **Invoked by every monomorphization path** to maintain the SSOT invariant:
/// - Eager typeck: `infer::expr::calls::monomorphization::maybe_record_mono_instance`
/// - Deferred typeck: `check::exports::resolve_deferred_mono_calls`
/// - JIT imported-mono: `oric::test::runner::imported_mono`
///
/// Idempotent and side-effect-free on `pool` (read-only queries).
#[expect(
    clippy::implicit_hasher,
    reason = "var_subst is consistently FxHashMap<u32, Idx> across the whole ori_types crate \
              (matches substitute_in_pool's signature); generalizing would force BuildHasher \
              plumbing through every caller for no measurable benefit."
)]
pub fn extend_var_subst_with_roots(
    pool: &Pool,
    scheme_var_ids: &[u32],
    var_subst: &mut FxHashMap<u32, Idx>,
) {
    let mut extensions: Vec<(u32, Idx)> = Vec::new();
    for &sv_id in scheme_var_ids {
        let Some(concrete) = var_subst.get(&sv_id).copied() else {
            continue;
        };
        let Some(sv_idx) = pool.var_idx_for_id(sv_id) else {
            continue;
        };
        let root = pool.resolve_fully(sv_idx);
        if pool.tag(root) == Tag::Var {
            let root_vid = pool.data(root);
            if root_vid != sv_id {
                extensions.push((root_vid, concrete));
            }
        }
    }
    for (vid, concrete) in extensions {
        var_subst.entry(vid).or_insert(concrete);
    }
}
