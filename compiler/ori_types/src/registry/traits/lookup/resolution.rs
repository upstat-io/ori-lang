//! Candidate resolution for trait, extension, and bound-chain method lookup.

use ori_ir::Name;
use rustc_hash::FxHashSet;

use crate::Idx;

use super::{
    BoundChainLookup, ImplMethodDef, ImplSpecificity, MethodLookup, MethodLookupResult,
    TraitMethodDef, TraitRegistry,
};

impl TraitRegistry {
    /// Disambiguate 2+ trait-impl candidates providing the same method: first
    /// prune super-trait-superseded candidates (a sub-trait overriding an
    /// inherited super-trait method), then fall back to specificity ranking.
    /// Returns `Ambiguous` when neither tier narrows to a single candidate.
    /// Shared tier-2 resolution step of [`Self::lookup_method_checked`].
    pub(super) fn resolve_trait_candidates<'a>(
        &self,
        candidates: Vec<(usize, Idx, &'a ImplMethodDef, ImplSpecificity)>,
    ) -> MethodLookupResult<'a> {
        match candidates.len() {
            0 => MethodLookupResult::NotFound,
            1 => {
                let (impl_idx, trait_idx, method, _) = candidates[0];
                MethodLookupResult::Found(MethodLookup::Trait {
                    trait_idx,
                    impl_idx,
                    method,
                })
            }
            _ => {
                // Multiple candidates: first filter by super-trait relationships.
                // If trait A is a super-trait of trait B and both provide the method,
                // keep only B (the sub-trait inherits or overrides A's method).
                let trait_idxs: FxHashSet<Idx> =
                    candidates.iter().map(|candidate| candidate.1).collect();
                let mut superseded: FxHashSet<Idx> = FxHashSet::default();
                for candidate in &candidates {
                    let supers = self.all_super_traits(candidate.1);
                    for &s in &supers {
                        if trait_idxs.contains(&s) {
                            superseded.insert(s);
                        }
                    }
                }
                let candidates: Vec<_> = candidates
                    .into_iter()
                    .filter(|c| !superseded.contains(&c.1))
                    .collect();

                if candidates.len() == 1 {
                    let (impl_idx, trait_idx, method, _) = candidates[0];
                    return MethodLookupResult::Found(MethodLookup::Trait {
                        trait_idx,
                        impl_idx,
                        method,
                    });
                }

                // Then try to disambiguate by specificity.
                // Keep only the most-specific candidates.
                let max_spec = candidates
                    .iter()
                    .map(|c| c.3)
                    .max()
                    .unwrap_or(ImplSpecificity::Generic);
                let best: Vec<_> = candidates.iter().filter(|c| c.3 == max_spec).collect();

                if best.len() == 1 {
                    let (impl_idx, trait_idx, method, _) = *best[0];
                    MethodLookupResult::Found(MethodLookup::Trait {
                        trait_idx,
                        impl_idx,
                        method,
                    })
                } else {
                    let trait_candidates: Vec<(Idx, Name)> = best
                        .iter()
                        .filter_map(|&&(_, trait_idx, _, _)| {
                            let name = self.get_trait_by_idx(trait_idx)?.name;
                            Some((trait_idx, name))
                        })
                        .collect();
                    MethodLookupResult::Ambiguous {
                        candidates: trait_candidates,
                    }
                }
            }
        }
    }

    /// Final extension-impl tier of [`Self::lookup_method_checked`]: fires
    /// only after every inherent and trait provider has missed. A single
    /// extension match resolves; 2+ conflicting extensions fail closed as
    /// `Ambiguous`.
    pub(super) fn resolve_extension_candidates(
        &self,
        self_type: Idx,
        method_name: Name,
    ) -> MethodLookupResult<'_> {
        let extension_candidates: Vec<_> = self
            .impls_by_type
            .get(&self_type)
            .into_iter()
            .flat_map(|indices| indices.iter())
            .filter_map(|&impl_idx| {
                if !self.is_extension_impl(impl_idx) {
                    return None;
                }
                let method = self.impls.get(impl_idx)?.methods.get(&method_name)?;
                Some((impl_idx, method))
            })
            .collect();
        match extension_candidates.as_slice() {
            [] => MethodLookupResult::NotFound,
            [(impl_idx, method)] => MethodLookupResult::Found(MethodLookup::Extension {
                impl_idx: *impl_idx,
                method,
            }),
            candidates => MethodLookupResult::Ambiguous {
                candidates: candidates
                    .iter()
                    .map(|(impl_idx, _)| {
                        (
                            Idx::ERROR,
                            self.extension_target_name(*impl_idx).unwrap_or(method_name),
                        )
                    })
                    .collect(),
            },
        }
    }

    // Bound-Chain Dispatch

    /// Find a trait method via the bound chain of a generic type parameter.
    ///
    /// When the receiver type is `Tag::RigidVar` (e.g., `T` in `@f<T: Clone>`),
    /// `lookup_method_checked` keyed on `Idx` always misses because `RigidVar`
    /// indices are never registered as `impls_by_type` keys. This helper walks
    /// the declared bounds (`type_param_bounds_for_var`) on the rigid var,
    /// resolves each bound trait name, and looks for a method matching
    /// `method_name` in that trait's `collected_methods` (covering supertrait
    /// inheritance per `BI-1`). Returns the first match — ambiguous bounds
    /// are detected by counting candidates across all bounds.
    ///
    /// Cycle protection: trait names are deduplicated via `FxHashSet`. Depth
    /// guard is implicit in the bound list (which is finite per
    /// `FunctionSig.type_param_bounds`).
    ///
    /// `type_param_bounds_for_var` is the slice from
    /// `FunctionSig.type_param_bounds[bound_idx]` where `bound_idx` is the
    /// index of the rigid var in `FunctionSig.scheme_var_ids` / `type_params`.
    /// Callers extract this from the current function signature.
    pub fn find_trait_method_via_bound_chain(
        &self,
        method_name: Name,
        type_param_bounds_for_var: &[Name],
    ) -> BoundChainLookup<'_> {
        let mut seen_traits: FxHashSet<Idx> = FxHashSet::default();
        let mut candidates: Vec<(Idx, &TraitMethodDef)> = Vec::new();

        for &trait_name in type_param_bounds_for_var {
            let Some(trait_entry) = self.get_trait_by_name(trait_name) else {
                continue;
            };
            let trait_idx = trait_entry.idx;
            if !seen_traits.insert(trait_idx) {
                continue;
            }
            for (mname, _owner_idx, method_def) in self.collected_methods(trait_idx) {
                if mname == method_name {
                    candidates.push((trait_idx, method_def));
                    break;
                }
            }
        }

        match candidates.len() {
            0 => BoundChainLookup::NotFound,
            1 => {
                let (trait_idx, method) = candidates[0];
                BoundChainLookup::Found { trait_idx, method }
            }
            _ => {
                let trait_candidates: Vec<(Idx, Name)> = candidates
                    .iter()
                    .filter_map(|&(trait_idx, _)| {
                        let name = self.get_trait_by_idx(trait_idx)?.name;
                        Some((trait_idx, name))
                    })
                    .collect();
                BoundChainLookup::Ambiguous {
                    candidates: trait_candidates,
                }
            }
        }
    }
}
