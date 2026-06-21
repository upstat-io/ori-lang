//! Generic burden composition at monomorphization.
//!
//! Composes a heap-backed `UserBurdenSpec` (in `ori_types`) from a pure-const
//! `BuiltinBurdenSpec` template (in `ori_registry::burden::table::BURDEN_TABLE`)
//! plus a vector of concrete type arguments. One call per first-instantiation
//! of a fully-resolved generic `Idx`; downstream codegen reads the registered
//! spec via `TypeRegistry::burden(idx)` without re-deriving.
//!
//! Spec: Annex E §AIMS — burden specs are typed pre-pass input feeding the
//! lattice-driven analysis. Composition runs at type-instantiation time so
//! Phase 5 trivial-emission consumes ready burden specs without dispatching on
//! type parameters at codegen time.
//!
//! Placeholder substitution:
//! - `TYPE_PARAM_T` (`u32::MAX`) → `type_args[0]`
//! - `TYPE_PARAM_E` (`u32::MAX - 1`) → `type_args[1]`
//!
//! Templates today use at most two type parameters (Option<T>, Result<T, E>,
//! [T], {K: V}, Set<T>). If a template ever introduces a third parameter, the
//! `placeholder_to_arg` mapping below SHALL be extended; the function silently
//! preserves unknown placeholders as `Idx::ERROR` to surface the gap loudly.
//!
//! Soundness gate: composition runs at type-instantiation time only — never
//! lazily at codegen. Lazy composition would force Phase 5 to emit indirect
//! dispatch on each burden walk. The `debug_assert!` in
//! `ori_arc::lower::burden_lookup` (the lookup hook) is the contractual end of this
//! invariant: when the lookup site exists, it consumes the registered spec
//! and asserts presence per the burden schema.

pub mod closure;
pub mod scc;

use ori_registry::burden::table::{TYPE_PARAM_E, TYPE_PARAM_T};
use ori_registry::burden::{
    BuiltinBurdenSpec, OwnedField as BuiltinOwnedField, TransferRule as BuiltinTransferRule,
    TypeId as BurdenTypeId, VariantBurden as BuiltinVariantBurden,
};

use crate::registry::burden::{
    UserBorrowedField, UserBurdenSpec, UserOwnedField, UserTransferRule, UserVariantBurden,
};
use crate::Idx;

/// Compose a monomorphized `UserBurdenSpec` from a template + concrete args.
///
/// `template` is a pure-const builtin template from `BURDEN_TABLE`
/// (e.g., the Option<T>, Result<T, E>, [T] entries). `type_args` are the
/// concrete `Idx` values supplied at monomorphization. The returned
/// `UserBurdenSpec` is heap-backed (`Vec`-backed) and carries concrete `Idx`
/// values throughout — no placeholders remain.
///
/// Recursive composition is implicit, not explicit: each substituted
/// `field_type: Idx` references a concrete pool entry whose burden spec is
/// looked up SEPARATELY by codegen at consumption time via
/// `TypeRegistry::burden(field_type)`. The composition does not in-line-expand
/// nested generics; it produces a flat spec keyed on the OUTER monomorphized
/// type, and the type graph is walked at codegen.
///
/// Nested-generic shapes (e.g. `Option<Result<int, str>>`) are fully supported
/// through this pattern — the outer `Option`'s composed spec carries the inner
/// `Result<int, str>`'s monomorphized `Idx` at its `Some` arm's `field_type`
/// slot, and codegen resolves the inner burden via a SEPARATE
/// `TypeRegistry::burden(inner_idx)` lookup. Pool interning (`TYPES:TI-2`)
/// guarantees the inner `Idx` distinguishes one nested shape from another;
/// see the `compose_option_with_result_payload_carries_inner_result_idx`
/// matrix test in `burden_compose/tests.rs`.
///
/// `pool` and `registry` parameters are accepted for forward compatibility
/// with future composition rules that may want to consult the resolved type
/// structure of a substituted argument for non-structural concerns —
/// e.g., transfer-kind inheritance from a user-type argument's own variant
/// burdens. Structural recursion through nested generics is NOT one such
/// concern; it falls out of pool-interned-`Idx` propagation as described
/// above. The shipped algorithm therefore does not consult either parameter.
pub fn compose_user_burden(
    template: &BuiltinBurdenSpec,
    type_args: &[Idx],
    _pool: &crate::Pool,
    _registry: &crate::TypeRegistry,
) -> UserBurdenSpec {
    UserBurdenSpec {
        self_heap_alloc: template.self_heap_alloc,
        owned_fields: template
            .owned_fields
            .iter()
            .map(|f| substitute_owned_field(f, type_args))
            .collect(),
        // Borrowed-field tier is target-only (Tag::Borrowed not yet constructible)
        // per `burden_compute.rs` precedent — composition mirrors that empty
        // discipline until borrow construction ships.
        borrowed_fields: Vec::<UserBorrowedField>::new(),
        variant_burdens: template
            .variant_burdens
            .iter()
            .map(|v| substitute_variant_burden(v, type_args))
            .collect(),
        element_burden: template
            .element_burden
            .map(|placeholder| substitute_type_id(placeholder, type_args)),
        compiled_drop: template.compiled_drop,
        user_drop: template.user_drop,
    }
}

/// Substitute a single placeholder `BurdenTypeId` to the concrete `Idx`
/// supplied at monomorphization. Non-placeholder ids reaching this function
/// indicate a template that mixes concrete `BurdenTypeId`s with placeholders
/// — not a shape the shipped templates use — and we surface them as
/// `Idx::ERROR` so the lookup-site `debug_assert!` fires loudly.
fn substitute_type_id(placeholder: BurdenTypeId, type_args: &[Idx]) -> Idx {
    if placeholder == TYPE_PARAM_T {
        // First type param. Missing arg → ERROR (composition site bug).
        type_args.first().copied().unwrap_or(Idx::ERROR)
    } else if placeholder == TYPE_PARAM_E {
        // Second type param. Missing arg → ERROR.
        type_args.get(1).copied().unwrap_or(Idx::ERROR)
    } else {
        // Concrete BurdenTypeId reaching composition is out of scope for the
        // shipped templates. Surface as ERROR so the lookup-site assertion
        // catches it instead of silently miscompiling.
        Idx::ERROR
    }
}

fn substitute_owned_field(field: &BuiltinOwnedField, type_args: &[Idx]) -> UserOwnedField {
    UserOwnedField {
        field_path: field.field_path.to_vec(),
        field_type: substitute_type_id(field.field_type, type_args),
    }
}

fn substitute_transfer_rule(rule: &BuiltinTransferRule, type_args: &[Idx]) -> UserTransferRule {
    UserTransferRule {
        source_field_path: rule.source_field_path.to_vec(),
        binding_index: rule.binding_index,
        field_type: substitute_type_id(rule.field_type, type_args),
        transfer_kind: rule.transfer_kind,
    }
}

fn substitute_variant_burden(
    variant: &BuiltinVariantBurden,
    type_args: &[Idx],
) -> UserVariantBurden {
    UserVariantBurden {
        variant_id: variant.variant_id,
        transfers_on_match: variant
            .transfers_on_match
            .iter()
            .map(|r| substitute_transfer_rule(r, type_args))
            .collect(),
        retained_owned: variant
            .retained_owned
            .iter()
            .map(|f| substitute_owned_field(f, type_args))
            .collect(),
    }
}

#[cfg(test)]
mod tests;
