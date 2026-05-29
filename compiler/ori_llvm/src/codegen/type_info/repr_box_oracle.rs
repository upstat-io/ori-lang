//! Boxing oracle (SSOT): decides, order-independently, whether a struct field,
//! tuple element, or enum/Option/Result payload position is heap-boxed as an
//! 8-byte RC pointer.
//!
//! A position holding type `T` inside owner `O` is boxed iff its inline storage
//! would embed an infinitely-sized type: `T`'s containment closure reaches `O`
//! (direct / indirect / mutual recursion through this position) OR `T` is itself
//! recursive (reaches itself through ≥1 containment edge). The self-recursion
//! arm is keyed only on `T`, so every structurally-equal pool `Idx` for a
//! recursive type answers identically — the type checker mints distinct `Idx`
//! values for the same `Option<Node>`, and owner-reachability alone would box
//! the in-cycle wrapper while inlining a standalone one, producing two
//! contradictory layouts for one type.
//!
//! This single oracle drives the layout resolver (box-vs-inline LLVM type) AND
//! the ARC emitter's Construct / Project / drop boxing, so the two never
//! disagree. Consumed by `layout_resolver.rs`, `enum_layout.rs`, and
//! `arc_emitter::context::is_boxed_enum_field`.

use ori_types::{Idx, Pool, Tag};
use rustc_hash::FxHashSet;

/// Whether the position inside `owner_type` that holds `field_type` is a boxed
/// recursive back-edge (lowers to `ptr`). Only Struct/Enum positions box;
/// wrappers (Option/Result/Tuple/collection) store inline and box their own
/// inner Struct/Enum positions when those wrappers are laid out.
pub(crate) fn position_is_rc_boxed(pool: &Pool, owner_type: Idx, field_type: Idx) -> bool {
    let owner = pool.resolve_fully(owner_type);
    let field = pool.resolve_fully(field_type);
    if !matches!(pool.tag(field), Tag::Struct | Tag::Enum) {
        return false;
    }
    let mut visited = FxHashSet::default();
    if reaches_owner(pool, owner, field, &mut visited) {
        return true;
    }
    is_recursive(pool, field)
}

/// Whether the payload type held by an `Option`/`Result` data variant is a
/// boxed recursive Struct/Enum back-edge. `owner_type` is the wrapper, the
/// payload is its inner / ok / err type.
pub(crate) fn option_payload_is_rc_boxed(pool: &Pool, owner_type: Idx) -> bool {
    let resolved = pool.resolve_fully(owner_type);
    position_is_rc_boxed(pool, resolved, pool.option_inner(resolved))
}

/// Whether the Ok payload of a `Result` at `owner_type` is a boxed recursive
/// back-edge.
pub(crate) fn result_ok_is_rc_boxed(pool: &Pool, owner_type: Idx) -> bool {
    let resolved = pool.resolve_fully(owner_type);
    position_is_rc_boxed(pool, resolved, pool.result_ok(resolved))
}

/// Whether the Err payload of a `Result` at `owner_type` is a boxed recursive
/// back-edge.
pub(crate) fn result_err_is_rc_boxed(pool: &Pool, owner_type: Idx) -> bool {
    let resolved = pool.resolve_fully(owner_type);
    position_is_rc_boxed(pool, resolved, pool.result_err(resolved))
}

/// Whether a Some/Ok/Err payload of type `payload_ty` is boxed — i.e. it is a
/// recursive Struct/Enum back-edge. Used by consumers (e.g. derive eq/compare/
/// hash) that hold only the payload type, not the wrapper Idx. Equivalent to
/// `position_is_rc_boxed(wrapper, payload_ty)` for the recursion-cycle case.
pub(crate) fn payload_type_is_rc_boxed(pool: &Pool, payload_ty: Idx) -> bool {
    let resolved = pool.resolve_fully(payload_ty);
    matches!(pool.tag(resolved), Tag::Struct | Tag::Enum) && is_recursive(pool, resolved)
}

/// Whether `ty` (a Struct/Enum) is recursive — its containment closure reaches
/// itself through ≥1 edge. Order-independent: keyed only on `ty`.
fn is_recursive(pool: &Pool, ty: Idx) -> bool {
    let target = pool.resolve_fully(ty);
    let mut visited = FxHashSet::default();
    children_reach(pool, target, target, &mut visited)
}

/// Transitive-containment reachability: does the inline-storage closure of
/// `current` reach `owner`? Follows struct fields, enum variant fields, tuple
/// elements, Option inner, Result ok/err. Collection element types and
/// `Tag::Function` are NOT followed — those positions are already behind their
/// own heap indirection (fat-pointer buffer / closure env), so a recursive type
/// reached only through them is not inline-stored at the position under test.
fn reaches_owner(pool: &Pool, owner: Idx, current: Idx, visited: &mut FxHashSet<Idx>) -> bool {
    let resolved = pool.resolve_fully(current);
    if resolved == owner {
        return true;
    }
    if !visited.insert(resolved) {
        return false;
    }
    walk_children(
        pool,
        resolved,
        |child, v| reaches_owner(pool, owner, child, v),
        visited,
    )
}

/// Like [`reaches_owner`] but expands the FIRST node into its children before
/// any `== owner` test, so `owner`-reaches-`owner` is detected only via a real
/// containment cycle, never trivially at depth 0.
fn children_reach(pool: &Pool, owner: Idx, current: Idx, visited: &mut FxHashSet<Idx>) -> bool {
    let resolved = pool.resolve_fully(current);
    if !visited.insert(resolved) {
        return false;
    }
    walk_children(
        pool,
        resolved,
        |child, v| reaches_owner(pool, owner, child, v),
        visited,
    )
}

/// Apply `f` to each inline-storage child of the resolved aggregate `resolved`.
fn walk_children(
    pool: &Pool,
    resolved: Idx,
    mut f: impl FnMut(Idx, &mut FxHashSet<Idx>) -> bool,
    visited: &mut FxHashSet<Idx>,
) -> bool {
    match pool.tag(resolved) {
        Tag::Struct => pool
            .struct_fields(resolved)
            .into_iter()
            .any(|(_, fi)| f(fi, visited)),
        Tag::Enum => pool
            .enum_variants(resolved)
            .into_iter()
            .flat_map(|(_, fields)| fields)
            .any(|fi| f(fi, visited)),
        Tag::Tuple => pool
            .tuple_elems(resolved)
            .into_iter()
            .any(|ei| f(ei, visited)),
        Tag::Option => f(pool.option_inner(resolved), visited),
        Tag::Result => {
            f(pool.result_ok(resolved), visited) || f(pool.result_err(resolved), visited)
        }
        _ => false,
    }
}
