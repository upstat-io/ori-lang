//! Resolution-chain following for [`Pool`].
//!
//! Separates resolve/resolve_fully/link-chase/var-lookup from the pure
//! compound-type accessors in the parent `mod.rs`. Also houses Named/Applied
//! resolution registration and newtype constructor registration.

use crate::{Idx, Tag};

use super::super::{Pool, VarState};

impl Pool {
    // Named Type Resolution Registration

    /// Register a resolution from a Named/Applied type to its concrete definition.
    ///
    /// After type registration creates a Pool Struct/Enum entry, this links the
    /// Named Idx (from `pool.named(name)`) to the concrete Struct/Enum Idx so
    /// codegen can resolve types without accessing `TypeRegistry`.
    pub fn set_resolution(&mut self, named: Idx, concrete: Idx) {
        self.resolutions.insert(named, concrete);
    }

    /// Record the `CAbiKind` of an FFI `c_*` type's distinct Named `Idx`.
    /// Called at FFI resolution alongside `set_resolution`; the kind carries
    /// the C-ABI width identity the resolution-to-`Idx::INT` discards.
    /// `ori_repr` reads it at the extern ABI boundary.
    pub fn set_cabi_kind(&mut self, named: Idx, kind: ori_ir::CAbiKind) {
        self.cabi_kinds.insert(named, kind);
    }

    /// Look up the `CAbiKind` of a type `Idx`. `None` for any non-FFI type —
    /// the carrier never leaks to non-FFI types (the width-leakage guard).
    #[must_use]
    pub fn cabi_kind(&self, idx: Idx) -> Option<ori_ir::CAbiKind> {
        self.cabi_kinds.get(&idx).copied()
    }

    /// Attach the FFI carrier — concrete resolution + `CAbiKind` width identity
    /// — to a `Named` FFI type `Idx`, as a pair. Single home for the FFI
    /// named-type registration step shared by every resolution site; the two
    /// attachments always travel together, so a half-attached state (resolution
    /// without kind) is unrepresentable through this entry point.
    pub fn attach_ffi_carrier(&mut self, named: Idx, concrete: Idx, kind: ori_ir::CAbiKind) {
        self.set_resolution(named, concrete);
        self.set_cabi_kind(named, kind);
    }

    /// Register a newtype constructor: `type N = Existing` produces an entry
    /// `N → Existing.idx`.
    ///
    /// Called from `check::registration::user_types::register_user_type` when
    /// processing `TypeDeclKind::Newtype`. The `name` is the newtype's
    /// source-level identifier; `underlying` is the resolved `Idx` of the
    /// wrapped type (primitive, container, struct, etc.).
    ///
    /// Consumers (`ori_arc::lower`) call `newtype_underlying(name)` at call
    /// sites to detect newtype constructor invocations and emit transparent
    /// `Let { Var(arg) }` instead of unresolvable `PartialApply`, preserving
    /// the layout-transparent newtype invariant (a newtype shares its
    /// underlying type's representation).
    pub fn register_newtype_ctor(&mut self, name: ori_ir::Name, underlying: Idx) {
        self.newtype_ctors.insert(name, underlying);
    }

    /// Look up a newtype constructor by name.
    ///
    /// Returns the underlying type's `Idx` if `name` is a registered newtype
    /// constructor (per `register_newtype_ctor`), otherwise `None`. This is
    /// the discriminator that `ori_arc::lower::calls::lower_call` uses to
    /// dispatch `UserId("hello")` to a transparent wrap rather than to the
    /// indirect-call path that misroutes through `lower_ident`'s
    /// `Tag::Function` arm.
    pub fn newtype_underlying(&self, name: ori_ir::Name) -> Option<Idx> {
        self.newtype_ctors.get(&name).copied()
    }

    /// True iff `name` is a registered newtype constructor.
    ///
    /// Convenience wrapper over [`Self::newtype_underlying`] for predicate use
    /// at lowering call sites (`if pool.is_newtype_ctor(name) { ... }`).
    pub fn is_newtype_ctor(&self, name: ori_ir::Name) -> bool {
        self.newtype_ctors.contains_key(&name)
    }

    /// Resolve a Named/Applied type to its concrete Struct/Enum definition.
    ///
    /// Follows resolution chains (e.g., alias -> named -> struct) with a depth
    /// limit of 16 to prevent infinite loops from cyclic references.
    ///
    /// Returns `None` if no resolution exists (e.g., generic type parameters
    /// that are only resolved during monomorphization).
    pub fn resolve(&self, idx: Idx) -> Option<Idx> {
        const MAX_DEPTH: u32 = 16;

        let mut current = idx;
        for _ in 0..MAX_DEPTH {
            match self.resolutions.get(&current) {
                Some(&resolved) => {
                    // If the resolved type points to itself, stop
                    if resolved == current {
                        return Some(resolved);
                    }
                    current = resolved;
                }
                None => {
                    // Only return Some if we followed at least one resolution
                    return if current == idx { None } else { Some(current) };
                }
            }
        }

        // Depth limit hit — return what we have so far
        tracing::warn!(
            idx = ?idx,
            depth = MAX_DEPTH,
            "Resolution chain depth limit reached"
        );
        Some(current)
    }

    /// Resolve a type index by following inference variable links first,
    /// then Named/Applied resolution chains.
    ///
    /// Unlike `resolve()`, which only follows the `resolutions` hashmap,
    /// this method also follows `VarState::Link` chains left by the unifier.
    /// After type checking, inference variables retain their `VarState::Link`
    /// targets in the Pool. This method follows those links to find the
    /// concrete type.
    ///
    /// For `Applied` types with no direct resolution (common for user-defined
    /// types like `Shape` which the type checker records as `Applied("Shape", [])`),
    /// falls back to searching for a `Named` resolution with the same name.
    ///
    /// Returns the fully-resolved type, or the input if no resolution exists.
    pub fn resolve_fully(&self, idx: Idx) -> Idx {
        // Bounds check: return early for indices outside the pool.
        if idx.raw() as usize >= self.items.len() {
            return idx;
        }
        let current = self.chase_var_links(idx);
        if let Some(resolved) = self.resolve(current) {
            return resolved;
        }
        if self.tag(current) == Tag::Applied {
            if let Some(concrete) = self.resolve_applied_via_matching_args(current) {
                return concrete;
            }
        }
        current
    }

    /// Follow `VarState::Link` chains starting at `idx` up to 16 steps.
    ///
    /// Stops at (a) the first non-`Tag::Var` item, (b) a malformed `var_id`
    /// outside `var_states`, or (c) the depth limit. Returns the resolved
    /// leaf. Bounds-check on `var_id` is retained even though the
    /// Generalized-leak path is retired (scheme bodies now hold
    /// `Tag::BoundVar` per invariant SC-1), because genuinely malformed inputs — e.g.,
    /// cross-pool `var_id` collisions caught by the pool re-intern path —
    /// can still reach this accessor.
    pub(crate) fn chase_var_links(&self, idx: Idx) -> Idx {
        let mut current = idx;
        for _ in 0..16 {
            if current.raw() as usize >= self.items.len() {
                break;
            }
            if self.tag(current) != Tag::Var {
                break;
            }
            let var_id = self.data(current);
            if (var_id as usize) >= self.var_states.len() {
                break;
            }
            match self.var_state(var_id) {
                VarState::Link { target } => current = *target,
                _ => break,
            }
        }
        current
    }

    /// Look up a concrete resolution for `current: Tag::Applied` by matching
    /// against registered `Applied`/`Named` keys whose resolved args agree
    /// with `current`'s resolved args.
    ///
    /// The type checker creates `Applied(Pair, [Var(X), Var(Y)])` during
    /// inference where `Var(X)`/`Var(Y)` are later unified via
    /// `VarState::Link`. Registrations live against `Applied(Pair, [Int, Int])` —
    /// a different `Idx`. This helper resolves both sides through links,
    /// then matches. Falls back to bare `Named` keys for non-generic
    /// aliases (e.g., `Applied("Shape", [])` resolving to a registered
    /// `Named("Shape")`).
    fn resolve_applied_via_matching_args(&self, current: Idx) -> Option<Idx> {
        let name = self.applied_name(current);
        let resolved_args: Vec<Idx> = self
            .applied_args(current)
            .iter()
            .map(|&a| self.resolve_fully(a))
            .collect();

        // Match Applied keys with the same name whose args resolve equal.
        for &key in self.resolutions.keys() {
            if self.tag(key) != Tag::Applied || self.applied_name(key) != name {
                continue;
            }
            let key_args = self.applied_args(key);
            if key_args.len() != resolved_args.len() {
                continue;
            }
            let all_match = key_args
                .iter()
                .zip(&resolved_args)
                .all(|(k, r)| self.resolve_fully(*k) == *r);
            if all_match {
                if let Some(concrete) = self.resolve(key) {
                    return Some(concrete);
                }
            }
        }

        // Non-generic fallback: Named resolution for the same name.
        for &key in self.resolutions.keys() {
            if self.tag(key) != Tag::Named || self.named_name(key) != name {
                continue;
            }
            if let Some(concrete) = self.resolve(key) {
                return Some(concrete);
            }
        }

        None
    }

    // Variable Lookup

    /// Find the pool [`Idx`] for a `Tag::Var` item whose `data` (`var_id`)
    /// matches `var_id`, or `None` if no such entry exists.
    ///
    /// This is the canonical reverse lookup from a `var_id` (the key into
    /// `var_states`) to the pool `Idx` that carries it. Consumers that
    /// need to map a `var_id` back to a pool handle — e.g., for
    /// `resolve_fully` on a scheme var's equivalence-class root —
    /// MUST use this method rather than open-coding a `FIRST_DYNAMIC..len`
    /// scan.
    ///
    /// Cost: `O(pool_len)` linear scan. This is acceptable because the
    /// method is called `O(scheme_var_count)` times per body validation,
    /// and `scheme_var_count` is typically 1–4.
    pub fn var_idx_for_id(&self, var_id: u32) -> Option<Idx> {
        let pool_len = u32::try_from(self.items.len()).unwrap_or(u32::MAX);
        (Idx::FIRST_DYNAMIC..pool_len)
            .map(Idx::from_raw)
            .find(|&idx| self.tag(idx) == Tag::Var && self.data(idx) == var_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::Name;

    /// INVARIANT: `resolve_fully` terminates on a self-similar nested `Applied`
    /// type (`Wrap<Wrap<int>>` resolving its inner `Wrap<int>`) whose registered
    /// concrete-instantiation key collides on name + arity with the inner type
    /// being resolved — the in-progress guard breaks the self-referential match
    /// candidate, and a type with no registered resolution of its own returns
    /// its own unresolved leaf.
    #[test]
    fn resolve_fully_terminates_on_self_similar_nested_applied() {
        let mut pool = Pool::new();
        let wrap = Name::from_raw(2075);
        // inner = Wrap<int>; outer = Wrap<Wrap<int>>; both share name + arity 1.
        let inner = pool.applied(wrap, &[Idx::INT]);
        let outer = pool.applied(wrap, &[inner]);
        // Register only the OUTER instantiation as a resolution key. Resolving the
        // INNER type then scans keys, finds OUTER (name+arity match), and recurses
        // on OUTER's nested arg (== inner) — the self-referential cycle.
        pool.set_resolution(outer, Idx::INT);
        // Must return (not overflow). Inner has no registered resolution of its
        // own, so the self-referential candidate is rejected and the leaf returns.
        assert_eq!(pool.resolve_fully(inner), inner);
    }

    /// Negative pin: the `resolve_applied_via_matching_args` path still resolves
    /// a generic instantiation whose query `Idx` differs structurally from the
    /// registered key but whose args resolve-equal — the cycle guard does NOT
    /// block legitimate match-based resolution. The query has NO direct
    /// resolution (so `resolve()` returns `None` and the matching path runs);
    /// the registered key carries an arg that must itself be resolved to match.
    #[test]
    fn resolve_fully_matches_applied_with_resolution_equal_args() {
        let mut pool = Pool::new();
        let wrap = Name::from_raw(2076);
        // A named alias that resolves to int — forces the matching loop to
        // resolve the key's arg before comparing.
        let alias = pool.named(Name::from_raw(2077));
        pool.set_resolution(alias, Idx::INT);
        // key = Wrap<alias> (alias resolves to int); resolves to the FLOAT marker.
        let key = pool.applied(wrap, &[alias]);
        pool.set_resolution(key, Idx::FLOAT);
        // query = Wrap<int> — structurally distinct from `key` (int vs alias),
        // has no direct resolution of its own, so it routes through matching.
        let query = pool.applied(wrap, &[Idx::INT]);
        assert_eq!(pool.resolve_fully(query), Idx::FLOAT);
    }
}
