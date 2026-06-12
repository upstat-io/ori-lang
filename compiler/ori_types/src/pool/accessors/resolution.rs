//! Resolution-chain following for [`Pool`].
//!
//! Separates resolve/resolve_fully/link-chase/var-lookup from the pure
//! compound-type accessors in the parent `mod.rs`. Also houses Named/Applied
//! resolution registration and newtype constructor registration.

use crate::{Idx, Tag};

use super::super::{Pool, VarState};

impl Pool {
    // === Named Type Resolution Registration ===

    /// Register a resolution from a Named/Applied type to its concrete definition.
    ///
    /// After type registration creates a Pool Struct/Enum entry, this links the
    /// Named Idx (from `pool.named(name)`) to the concrete Struct/Enum Idx so
    /// codegen can resolve types without accessing `TypeRegistry`.
    pub fn set_resolution(&mut self, named: Idx, concrete: Idx) {
        self.resolutions.insert(named, concrete);
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
    /// `Let { Var(arg) }` instead of unresolvable `PartialApply`. See
    /// for the layout-transparent invariant this preserves.
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
    fn chase_var_links(&self, idx: Idx) -> Idx {
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

    // === Variable Lookup ===

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
