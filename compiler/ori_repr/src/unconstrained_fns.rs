//! Backend-neutral unconstrained-function identities for range analysis.
//!
//! Pub top-level functions and trait impl methods may be called from external
//! code or via dynamic dispatch, so range analysis must assign Top to their
//! parameters. This module collects those identities in the format the
//! analysis-only ARC lowering uses.

/// Collect unconstrained function identities (pub or trait impl).
///
/// Returns `(Option<Idx>, Name)` pairs: `None` for pub top-level functions,
/// `Some(self_type)` for trait impl methods (disambiguation).
///
/// Tracks ordinals for same-type same-name method duplicates (e.g., two
/// `impl Index<...>` on the same type). Registers both the base qualified
/// name and ordinal-suffixed variants to match the analysis-only ARC
/// lowering format.
pub fn collect_unconstrained_fn_names(
    function_sigs: &[ori_types::FunctionSig],
    trait_impl_fn_names: &[(ori_types::Idx, ori_ir::Name)],
    interner: Option<&ori_ir::StringInterner>,
) -> Vec<(Option<ori_types::Idx>, ori_ir::Name)> {
    let mut names = Vec::new();
    for sig in function_sigs {
        if sig.is_public {
            names.push((None, sig.name));
        }
    }
    // Track ordinals for same-type same-name duplicates.
    let mut method_ordinals: rustc_hash::FxHashMap<(ori_types::Idx, ori_ir::Name), usize> =
        rustc_hash::FxHashMap::default();
    for &(self_type, name) in trait_impl_fn_names {
        names.push((Some(self_type), name));
        // Register qualified names matching the analysis-only ARC lowering format
        // Ordinal-qualified names are registered for same-type
        // same-name duplicates so `is_qualified_unconstrained()` finds them
        if let Some(interner) = interner {
            let ordinal = {
                let entry = method_ordinals.entry((self_type, name)).or_insert(0);
                let ord = *entry;
                *entry += 1;
                ord
            };
            let method_str = interner.lookup(name);
            let qualified = if ordinal == 0 {
                interner.intern(&format!("__impl_{}_{method_str}", self_type.raw()))
            } else {
                interner.intern(&format!(
                    "__impl_{}_{}_{ordinal}",
                    self_type.raw(),
                    method_str
                ))
            };
            names.push((None, qualified));
        }
    }
    names
}
