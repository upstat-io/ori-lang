//! Type substitution for monomorphization.
//!
//! [`substitute_in_pool`] materializes concrete monomorphization body maps
//! directly in a `Pool`, preserving the type shapes ARC lowering consumes.

mod body_type_map;
mod extract;
mod materialize;
mod named_self;

pub use body_type_map::{
    build_finalized_body_type_map, build_mono_body_type_map, extend_var_subst_with_roots,
    BodyTypeMapSink,
};
pub use extract::extract_var_from_types;
pub use materialize::register_concrete_applied_resolutions;
pub(crate) use materialize::{has_unproven_named_leaf, materialize_applied_body};
pub use named_self::{substitute_named_in_pool, substitute_self_in_pool};

use rustc_hash::FxHashMap;

use crate::{Idx, Pool, Tag, TypeFlags, VarState};

/// A substituted compound identity was not interned by the owning type phase.
///
/// Read-only consumers use [`substitute_in_existing_pool`] after type checking.
/// A missing identity is therefore an upstream materialization defect, not
/// permission for the consumer to extend the canonical type pool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissingSubstitution {
    source: Idx,
}

impl MissingSubstitution {
    /// Return the source type whose substituted compound identity is missing.
    #[must_use]
    pub const fn source(self) -> Idx {
        self.source
    }
}

trait SubstitutionOutput {
    fn pool(&self) -> &Pool;
    fn simple(&mut self, tag: Tag, data: u32) -> Option<Idx>;
    fn complex(&mut self, tag: Tag, extra: &[u32]) -> Option<Idx>;
}

struct InterningOutput<'pool> {
    pool: &'pool mut Pool,
}

impl SubstitutionOutput for InterningOutput<'_> {
    fn pool(&self) -> &Pool {
        self.pool
    }

    fn simple(&mut self, tag: Tag, data: u32) -> Option<Idx> {
        Some(self.pool.intern(tag, data))
    }

    fn complex(&mut self, tag: Tag, extra: &[u32]) -> Option<Idx> {
        Some(self.pool.intern_complex(tag, extra))
    }
}

struct ExistingOutput<'pool> {
    pool: &'pool Pool,
}

impl ExistingOutput<'_> {
    fn lookup(&self, tag: Tag, data: u32, extra: &[u32]) -> Option<Idx> {
        let hash = self.pool.merkle_hash(tag, data, extra);
        let idx = self.pool.lookup_by_hash(hash)?;
        debug_assert_eq!(
            self.pool.tag(idx),
            tag,
            "Merkle hash collision while resolving an existing substitution"
        );
        (self.pool.tag(idx) == tag).then_some(idx)
    }
}

impl SubstitutionOutput for ExistingOutput<'_> {
    fn pool(&self) -> &Pool {
        self.pool
    }

    fn simple(&mut self, tag: Tag, data: u32) -> Option<Idx> {
        self.lookup(tag, data, &[])
    }

    fn complex(&mut self, tag: Tag, extra: &[u32]) -> Option<Idx> {
        self.lookup(tag, 0, extra)
    }
}

/// Recursively substitute type variables in `ty` using `var_subst`.
///
/// The substitution map keys are `var_ids` (matching [`FunctionSig::scheme_var_ids`]).
/// Each mapped value is a concrete `Idx` (e.g., `Idx::INT` for `int`).
///
/// Returns the substituted type. If no variables in `ty` match the map,
/// returns `ty` unchanged (O(1) via the `HAS_VAR` flag fast path).
/// New composite types are interned in `pool` (deduplication is automatic).
#[expect(
    clippy::implicit_hasher,
    reason = "always called with FxHashMap internally"
)]
pub fn substitute_in_pool(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    substitute_type(&mut InterningOutput { pool }, ty, var_subst)
        .unwrap_or_else(|_| unreachable!("interning substitution cannot miss a type identity"))
}

/// Recursively substitute type variables using identities already interned in
/// `pool` by type checking.
///
/// Unlike [`substitute_in_pool`], this function cannot extend `pool`. It is the
/// downstream read-only form for ARC and other post-canonicalization consumers.
/// If a changed compound type has no existing canonical identity, the function
/// fails closed with the source coordinate that the type phase failed to
/// materialize.
#[expect(
    clippy::implicit_hasher,
    reason = "always called with FxHashMap internally"
)]
pub fn substitute_in_existing_pool(
    pool: &Pool,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
) -> Result<Idx, MissingSubstitution> {
    substitute_type(&mut ExistingOutput { pool }, ty, var_subst)
}

fn substitute_type<Output: SubstitutionOutput>(
    output: &mut Output,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
) -> Result<Idx, MissingSubstitution> {
    // INVARIANT: every substitutable variable kind participates in the fast-path gate.
    if !output
        .pool()
        .flags(ty)
        .intersects(TypeFlags::HAS_VAR | TypeFlags::HAS_BOUND_VAR | TypeFlags::HAS_RIGID_VAR)
    {
        return Ok(ty);
    }

    match output.pool().tag(ty) {
        Tag::Var => substitute_var(output, ty, var_subst),
        Tag::BoundVar | Tag::RigidVar => Ok(var_subst
            .get(&output.pool().data(ty))
            .copied()
            .unwrap_or(ty)),

        // Single-child containers
        tag @ (Tag::List
        | Tag::Option
        | Tag::Set
        | Tag::Channel
        | Tag::Range
        | Tag::Iterator
        | Tag::DoubleEndedIterator) => substitute_single_child(output, ty, tag, var_subst),

        // Two-child containers
        tag @ (Tag::Map | Tag::Result) => substitute_type_pair(output, ty, tag, var_subst),

        // Borrowed reference
        Tag::Borrowed => substitute_borrowed(output, ty, var_subst),

        // Variable-length types
        Tag::Function => substitute_function(output, ty, var_subst),
        Tag::Tuple => substitute_tuple(output, ty, var_subst),
        Tag::Applied => substitute_applied(output, ty, var_subst),
        Tag::Struct => substitute_struct(output, ty, var_subst),
        Tag::Enum => substitute_enum(output, ty, var_subst),

        // Schemes have their own bound variables; primitives and other tags
        // don't contain variables.
        _ => Ok(ty),
    }
}

/// Substitute a type variable: check `var_id`, then follow links.
///
/// `Tag::Var` leaves whose `var_state` is `Generalized` or `Rigid` fall
/// through to the bottom and return `ty` unchanged — they are orphan
/// references to scheme-bound vars that the substitution map (keyed by
/// the callee's `var_id`s) does not target. The whole-pool walk in
/// `infer::expr::calls::monomorphization::maybe_record_mono_instance`
/// routinely hits such orphans and relies on this no-op fall-through.
///
/// Post-migration, scheme bodies themselves carry `Tag::BoundVar` leaves
/// (see `substitute_bound_var`); the only legitimate `Tag::Var(Generalized)`
/// pool entries are the orphan inference residues just described.
fn substitute_var<Output: SubstitutionOutput>(
    output: &mut Output,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
) -> Result<Idx, MissingSubstitution> {
    let var_id = output.pool().data(ty);

    // Direct var_id match (scheme variable)
    if let Some(&replacement) = var_subst.get(&var_id) {
        return Ok(replacement);
    }

    // Follow link if present
    let target = match output.pool().var_state(var_id) {
        VarState::Link { target } => Some(*target),
        VarState::Unbound(_) | VarState::Rigid { .. } | VarState::Generalized(_) => None,
    };
    if let Some(target) = target {
        return substitute_type(output, target, var_subst);
    }

    Ok(ty)
}

/// Build a `var_id -> concrete` substitution for impl-level rigid generics.
/// Scans every `VarState::Rigid { name }` in the pool; when `name` matches an
/// impl binder in `name_to_concrete`, maps that rigid's `var_id` to the concrete
/// type. SSOT for the impl-rigid scan consumed by `resolve_impl_signature`
/// (signature substitution feeding mono recording) and the mono body type map.
pub fn build_impl_rigid_var_subst(
    pool: &Pool,
    name_to_concrete: &FxHashMap<ori_ir::Name, Idx>,
) -> FxHashMap<u32, Idx> {
    let mut out: FxHashMap<u32, Idx> = FxHashMap::default();
    if name_to_concrete.is_empty() {
        return out;
    }
    for var_id in 0..pool.next_var_id() {
        if let Some(VarState::Rigid { name }) = pool.var_state_checked(var_id) {
            if let Some(&concrete) = name_to_concrete.get(name) {
                out.insert(var_id, concrete);
            }
        }
    }
    out
}

/// Build the concrete body-type map for a method owned by a generic impl.
///
/// Named bindings cover declaration-level type parameters while the derived
/// rigid substitution covers canonical body types. When the caller supplies a
/// generic receiver body, the helper materializes and registers its concrete
/// layout before ARC lowering resolves field projections.
pub fn build_impl_mono_body_type_map(
    pool: &mut Pool,
    named_bindings: &[(ori_ir::Name, Idx)],
    receiver: Idx,
    receiver_body: Option<Idx>,
    concrete_receiver: Option<Idx>,
) -> FxHashMap<Idx, Idx> {
    let named: FxHashMap<_, _> = named_bindings.iter().copied().collect();
    let rigid_subst = build_impl_rigid_var_subst(pool, &named);
    let generic_body = receiver_body.or_else(|| pool.resolve(receiver));
    let concrete_receiver_body =
        if let (Some(concrete_receiver), Some(generic_body)) = (concrete_receiver, generic_body) {
            let named_body = substitute_named_in_pool(pool, generic_body, &named);
            let concrete_body = substitute_in_pool(pool, named_body, &rigid_subst);
            pool.set_resolution(concrete_receiver, concrete_body);
            Some((generic_body, concrete_body))
        } else {
            None
        };
    let named_entries: Vec<_> = named
        .iter()
        .map(|(&name, &concrete)| (pool.named(name), concrete))
        .collect();
    let mut body_type_map: FxHashMap<_, _> =
        build_finalized_body_type_map(pool, &rigid_subst, &named_entries)
            .into_iter()
            .collect();
    if let Some(concrete_receiver) = concrete_receiver {
        body_type_map.insert(receiver, concrete_receiver);
    }
    if let Some((generic_body, concrete_body)) = concrete_receiver_body {
        body_type_map.insert(generic_body, concrete_body);
    }
    body_type_map
}

/// Substitute in a single-child container (List, Option, Set, etc.).
fn substitute_single_child<Output: SubstitutionOutput>(
    output: &mut Output,
    ty: Idx,
    tag: Tag,
    var_subst: &FxHashMap<u32, Idx>,
) -> Result<Idx, MissingSubstitution> {
    let child = Idx::from_raw(output.pool().data(ty));
    let new_child = substitute_type(output, child, var_subst)?;
    if new_child == child {
        Ok(ty)
    } else {
        output
            .simple(tag, new_child.raw())
            .ok_or(MissingSubstitution { source: ty })
    }
}

/// Substitute in a Map or Result type.
fn substitute_type_pair<Output: SubstitutionOutput>(
    output: &mut Output,
    ty: Idx,
    tag: Tag,
    var_subst: &FxHashMap<u32, Idx>,
) -> Result<Idx, MissingSubstitution> {
    let (first, second) = match tag {
        Tag::Map => (output.pool().map_key(ty), output.pool().map_value(ty)),
        Tag::Result => (output.pool().result_ok(ty), output.pool().result_err(ty)),
        _ => unreachable!("pair substitution requires Map or Result"),
    };
    let new_first = substitute_type(output, first, var_subst)?;
    let new_second = substitute_type(output, second, var_subst)?;
    if new_first == first && new_second == second {
        Ok(ty)
    } else {
        output
            .complex(tag, &[new_first.raw(), new_second.raw()])
            .ok_or(MissingSubstitution { source: ty })
    }
}

/// Shared recurse-and-rebuild skeleton for mutable substitution families whose
/// child context is not the variable-id map used by [`substitute_type`].
fn substitute_child<C>(
    pool: &mut Pool,
    ty: Idx,
    child: Idx,
    context: &C,
    recurse: fn(&mut Pool, Idx, &C) -> Idx,
    ctor: fn(&mut Pool, Idx) -> Idx,
) -> Idx {
    let new_child = recurse(pool, child, context);
    if new_child == child {
        ty
    } else {
        ctor(pool, new_child)
    }
}

/// Shared two-child counterpart to [`substitute_child`].
fn substitute_pair<C>(
    pool: &mut Pool,
    ty: Idx,
    first: Idx,
    second: Idx,
    context: &C,
    recurse: fn(&mut Pool, Idx, &C) -> Idx,
    ctor: fn(&mut Pool, Idx, Idx) -> Idx,
) -> Idx {
    let new_first = recurse(pool, first, context);
    let new_second = recurse(pool, second, context);
    if new_first == first && new_second == second {
        ty
    } else {
        ctor(pool, new_first, new_second)
    }
}

/// Substitute in a Borrowed reference (inner + lifetime preserved).
fn substitute_borrowed<Output: SubstitutionOutput>(
    output: &mut Output,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
) -> Result<Idx, MissingSubstitution> {
    let inner = output.pool().borrowed_inner(ty);
    let lifetime = output.pool().borrowed_lifetime(ty);
    let new_inner = substitute_type(output, inner, var_subst)?;
    if new_inner == inner {
        Ok(ty)
    } else {
        output
            .complex(Tag::Borrowed, &[new_inner.raw(), lifetime.raw()])
            .ok_or(MissingSubstitution { source: ty })
    }
}

/// Substitute in a Function type (params + return).
fn substitute_function<Output: SubstitutionOutput>(
    output: &mut Output,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
) -> Result<Idx, MissingSubstitution> {
    let params = output.pool().function_params(ty);
    let return_type = output.pool().function_return(ty);
    let new_params: Vec<Idx> = params
        .iter()
        .map(|&param| substitute_type(output, param, var_subst))
        .collect::<Result<_, _>>()?;
    let new_return = substitute_type(output, return_type, var_subst)?;
    if new_params == params && new_return == return_type {
        return Ok(ty);
    }

    let mut extra = Vec::with_capacity(new_params.len() + 2);
    extra.push(u32::try_from(new_params.len()).unwrap_or(u32::MAX));
    extra.extend(new_params.iter().map(|param| param.raw()));
    extra.push(new_return.raw());
    output
        .complex(Tag::Function, &extra)
        .ok_or(MissingSubstitution { source: ty })
}

/// Substitute in a Tuple type (element list).
fn substitute_tuple<Output: SubstitutionOutput>(
    output: &mut Output,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
) -> Result<Idx, MissingSubstitution> {
    let elements = output.pool().tuple_elems(ty);
    let new_elements: Vec<Idx> = elements
        .iter()
        .map(|&element| substitute_type(output, element, var_subst))
        .collect::<Result<_, _>>()?;
    if new_elements == elements {
        return Ok(ty);
    }

    let mut extra = Vec::with_capacity(new_elements.len() + 1);
    extra.push(u32::try_from(new_elements.len()).unwrap_or(u32::MAX));
    extra.extend(new_elements.iter().map(|element| element.raw()));
    output
        .complex(Tag::Tuple, &extra)
        .ok_or(MissingSubstitution { source: ty })
}

/// Substitute in an Applied type (name + type args).
fn substitute_applied<Output: SubstitutionOutput>(
    output: &mut Output,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
) -> Result<Idx, MissingSubstitution> {
    let name = output.pool().applied_name(ty);
    let arguments = output.pool().applied_args(ty);
    let new_arguments: Vec<Idx> = arguments
        .iter()
        .map(|&argument| substitute_type(output, argument, var_subst))
        .collect::<Result<_, _>>()?;
    if new_arguments == arguments {
        return Ok(ty);
    }

    let name_bits = u64::from(name.raw());
    let mut extra = Vec::with_capacity(new_arguments.len() + 3);
    extra.push((name_bits & 0xFFFF_FFFF) as u32);
    extra.push((name_bits >> 32) as u32);
    extra.push(u32::try_from(new_arguments.len()).unwrap_or(u32::MAX));
    extra.extend(new_arguments.iter().map(|argument| argument.raw()));
    output
        .complex(Tag::Applied, &extra)
        .ok_or(MissingSubstitution { source: ty })
}

/// Substitute in a Struct type (field types, preserving field names).
fn substitute_struct<Output: SubstitutionOutput>(
    output: &mut Output,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
) -> Result<Idx, MissingSubstitution> {
    let name = output.pool().struct_name(ty);
    let fields = output.pool().struct_fields(ty);
    let new_fields: Vec<(ori_ir::Name, Idx)> = fields
        .iter()
        .map(|&(field_name, field_type)| {
            substitute_type(output, field_type, var_subst).map(|new_type| (field_name, new_type))
        })
        .collect::<Result<_, _>>()?;
    if new_fields == fields {
        return Ok(ty);
    }

    let name_bits = u64::from(name.raw());
    let mut extra = Vec::with_capacity(3 + new_fields.len() * 2);
    extra.push((name_bits & 0xFFFF_FFFF) as u32);
    extra.push((name_bits >> 32) as u32);
    extra.push(u32::try_from(new_fields.len()).unwrap_or(u32::MAX));
    for (field_name, field_type) in new_fields {
        extra.push(field_name.raw());
        extra.push(field_type.raw());
    }
    output
        .complex(Tag::Struct, &extra)
        .ok_or(MissingSubstitution { source: ty })
}

/// Substitute in an Enum type (variant payload types, preserving variant names).
fn substitute_enum<Output: SubstitutionOutput>(
    output: &mut Output,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
) -> Result<Idx, MissingSubstitution> {
    let name = output.pool().enum_name(ty);
    let variants = output.pool().enum_variants(ty);
    let new_variants: Vec<(ori_ir::Name, Vec<Idx>)> = variants
        .iter()
        .map(|(variant_name, payloads)| {
            let new_payloads = payloads
                .iter()
                .map(|&payload| substitute_type(output, payload, var_subst))
                .collect::<Result<_, _>>()?;
            Ok((*variant_name, new_payloads))
        })
        .collect::<Result<_, MissingSubstitution>>()?;
    if new_variants == variants {
        return Ok(ty);
    }

    let name_bits = u64::from(name.raw());
    let extra_len = 3 + new_variants
        .iter()
        .map(|(_, payloads)| 2 + payloads.len())
        .sum::<usize>();
    let mut extra = Vec::with_capacity(extra_len);
    extra.push((name_bits & 0xFFFF_FFFF) as u32);
    extra.push((name_bits >> 32) as u32);
    extra.push(u32::try_from(new_variants.len()).unwrap_or(u32::MAX));
    for (variant_name, payloads) in new_variants {
        extra.push(variant_name.raw());
        extra.push(u32::try_from(payloads.len()).unwrap_or(u32::MAX));
        extra.extend(payloads.iter().map(|payload| payload.raw()));
    }
    output
        .complex(Tag::Enum, &extra)
        .ok_or(MissingSubstitution { source: ty })
}

#[cfg(test)]
mod tests;
