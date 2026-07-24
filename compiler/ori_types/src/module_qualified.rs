//! Module-qualified type-path resolution shared by both resolution paths.
//!
//! `ParsedType::AssociatedType` carries two roles: an associated-type
//! projection (`Self.Item`, `T.Assoc`) and a module-qualified type path
//! (`geom.Point`). Only the second resolves against the alias table, and both
//! the signature/registration resolver and the in-body inference resolver reach
//! it. This module owns that discrimination so the two resolvers share one
//! answer instead of each carrying its own copy.

use ori_ir::{ExprArena, Name, ParsedType, ParsedTypeId, StringInterner};
use rustc_hash::FxHashMap;

use crate::registry::TypeRegistry;
use crate::{FunctionSig, Idx};

/// A resolved module-qualified type path: the registry's canonical identity for
/// the declaration named through some alias.
#[derive(Clone, Copy)]
pub(crate) struct QualifiedTypePath {
    /// The name of the registry's canonical entry for this declaration.
    ///
    /// A declaration reached through several module aliases keeps ONE canonical
    /// entry, so applying generic args through any alias yields one applied
    /// type rather than one per alias spelling.
    pub canonical_name: Name,
    /// The registered index of the un-applied declaration.
    pub idx: Idx,
}

/// Resolve a module-qualified type path head, or `None` when the head is not one.
///
/// Answers `Some` only for a bare `Named` base that is a registered module
/// alias and whose qualified name (`alias.Type`) resolves in the registry --
/// the key `register_module_alias` interns an imported type under. Every other
/// shape (a generic base, an unregistered alias, an unknown qualified name, a
/// `Self`/type-variable base) answers `None`, leaving the caller on the
/// associated-type projection path.
pub(crate) fn qualified_type_path(
    module_aliases: &FxHashMap<Name, Vec<FunctionSig>>,
    interner: &StringInterner,
    type_registry: &TypeRegistry,
    arena: &ExprArena,
    base: ParsedTypeId,
    assoc_name: Name,
) -> Option<QualifiedTypePath> {
    let ParsedType::Named {
        name: alias,
        type_args,
    } = arena.get_parsed_type(base)
    else {
        return None;
    };
    if !type_args.is_empty() || !module_aliases.contains_key(alias) {
        return None;
    }
    let qualified = interner.intern_owned(ori_ir::qualified_alias_name(
        interner.lookup(*alias),
        interner.lookup(assoc_name),
    ));
    let idx = type_registry.get_by_name(qualified)?.idx;
    let canonical_name = type_registry
        .get_by_idx(idx)
        .map_or(qualified, |entry| entry.name);
    Some(QualifiedTypePath {
        canonical_name,
        idx,
    })
}

/// Instantiate a resolved qualified path with its terminal segment's generic
/// arguments (`geom.Pair<int>`), or return the bare declaration when it has none.
pub(crate) fn apply_qualified(
    pool: &mut crate::Pool,
    path: QualifiedTypePath,
    args: &[Idx],
) -> Idx {
    if args.is_empty() {
        path.idx
    } else {
        pool.applied(path.canonical_name, args)
    }
}
