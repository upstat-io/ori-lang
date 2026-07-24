//! JSONL record projection for the `emit-aims-state` producer.
//!
//! Projects the AIMS 19-property contract surface plus per-variable
//! lattice values to JSON through EXPLICIT snake-case match tables. The
//! field-name + variant-name pairing is the join surface a downstream
//! code-intelligence consumer reads — it is a stable contract, never a
//! `Debug`/`serde`-derive artifact of the lattice enums (Spec: Annex E §AIMS).

use ori_arc::aims::contract::{EffectSummary, FipContract, ReturnContract};
use ori_arc::aims::lattice::{
    AccessClass, AimsState, Cardinality, Consumption, Locality, ReuseCtorKind, ShapeClass,
    Uniqueness,
};
use ori_arc::MemoryContract;
use serde_json::{json, Value};

/// Schema version stamped on every record.
///
/// The downstream consumer validates this and fails loudly when a renamed
/// `ReturnContract`/`EffectSummary`/`FipContract` field shifts the projected
/// surface, so a drift never silently drops a property.
pub(super) const SCHEMA_VERSION: u32 = 2;

/// Repo identity matching the intel-graph `:Symbol.repo` key.
pub(super) const REPO: &str = "ori";

// Enum → snake_case match tables. Each is the explicit serialization contract
// for one lattice / contract enum; never `format!("{:?}")` and never a derived
// `Serialize`.

pub(super) fn access_name(a: AccessClass) -> &'static str {
    match a {
        AccessClass::Borrowed => "borrowed",
        AccessClass::Owned => "owned",
    }
}

pub(super) fn consumption_name(c: Consumption) -> &'static str {
    match c {
        Consumption::Dead => "dead",
        Consumption::Linear => "linear",
        Consumption::Affine => "affine",
        Consumption::Unrestricted => "unrestricted",
    }
}

pub(super) fn cardinality_name(c: Cardinality) -> &'static str {
    match c {
        Cardinality::Absent => "absent",
        Cardinality::Once => "once",
        Cardinality::Many => "many",
    }
}

pub(super) fn uniqueness_name(u: Uniqueness) -> &'static str {
    match u {
        Uniqueness::Unique => "unique",
        Uniqueness::MaybeShared => "maybe_shared",
        Uniqueness::Shared => "shared",
    }
}

pub(super) fn locality_name(l: Locality) -> &'static str {
    match l {
        Locality::BlockLocal => "block_local",
        Locality::FunctionLocal => "function_local",
        Locality::HeapEscaping => "heap_escaping",
        Locality::Unknown => "unknown",
    }
}

pub(super) fn shape_name(s: ShapeClass) -> &'static str {
    match s {
        ShapeClass::NonReusable => "non_reusable",
        ShapeClass::ReusableCtor(ReuseCtorKind::Struct) => "reusable_ctor_struct",
        ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant) => "reusable_ctor_enum_variant",
        ShapeClass::CollectionBuffer => "collection_buffer",
        ShapeClass::ContextHole => "context_hole",
    }
}

pub(super) fn fip_name(f: &FipContract) -> &'static str {
    match f {
        FipContract::Never => "never",
        FipContract::Conditional { .. } => "conditional",
        FipContract::Certified => "certified",
        FipContract::Bounded(_) => "bounded",
    }
}

/// Producer-emitted identity stamped on every record. A downstream consumer
/// bridges `(repo, qualified_name, file, line)` to its own symbol key;
/// `scip_moniker` is the same SCIP symbol string the SCIP index mints.
pub(super) struct RecordIdentity<'a> {
    pub qualified_name: &'a str,
    pub file: &'a str,
    pub line: u32,
    pub scip_moniker: &'a str,
}

/// Project one per-variable lattice value, `position`-qualified by its
/// `ArcVarId` index.
pub(super) fn lattice_value_json(position: u32, state: AimsState) -> Value {
    json!({
        "position": position,
        "access": access_name(state.access),
        "consumption": consumption_name(state.consumption),
        "cardinality": cardinality_name(state.cardinality),
        "uniqueness": uniqueness_name(state.uniqueness),
        "locality": locality_name(state.locality),
        "shape": shape_name(state.shape),
        "aims_effect_may_alloc": state.effect.may_alloc,
        "aims_effect_may_share": state.effect.may_share,
        "aims_effect_may_throw": state.effect.may_throw,
    })
}

/// Build one JSONL record: the 19-property surface + identity +
/// per-variable lattice values.
pub(super) fn build_record(
    id: &RecordIdentity,
    contract: &MemoryContract,
    fip_construct_count: u32,
    fip_consumed_count: u32,
    lattice_values: Vec<Value>,
) -> Value {
    let ret: &ReturnContract = &contract.return_info;
    let eff: &EffectSummary = &contract.effects;

    let mut record = json!({
        "schema_version": SCHEMA_VERSION,
        "repo": REPO,
        "qualified_name": id.qualified_name,
        "file": id.file,
        "line": id.line,
        "scip_moniker": id.scip_moniker,

        "aims_param_count": contract.params.len(),
        "aims_is_fbip": contract.is_fbip,

        // ReturnContract — all 6 fields.
        "aims_return_uniqueness": uniqueness_name(ret.uniqueness),
        "aims_return_preserves_freshness": ret.preserves_freshness,
        "aims_return_locality": locality_name(ret.locality),
        "aims_return_shape": shape_name(ret.shape),
        "aims_return_returns_fresh_self_alloc": ret.returns_fresh_self_alloc,
        "aims_return_returns_sharing_view": ret.returns_sharing_view,

        // EffectSummary — 6 flags.
        "aims_effect_may_allocate": eff.may_allocate,
        "aims_effect_alloc_only_on_slow_path": eff.alloc_only_on_slow_path,
        "aims_effect_may_deallocate": eff.may_deallocate,
        "aims_effect_may_share": eff.may_share,
        "aims_effect_may_throw": eff.may_throw,
        "aims_effect_has_unbounded_stack": eff.has_unbounded_stack,

        // FipContract discriminant.
        "aims_fip": fip_name(&contract.fip),

        // MemoryContract / AimsStateMap Layer-A props.
        "aims_fip_construct_count": fip_construct_count,
        "aims_fip_consumed_count": fip_consumed_count,
    });

    // Per-variable lattice values (consumed into the record array).
    record["lattice_values"] = Value::Array(lattice_values);

    // FipContract payloads — serialized, not discriminant-only.
    match &contract.fip {
        FipContract::Bounded(n) => {
            record["aims_fip_bounded"] = json!(n);
        }
        FipContract::Conditional {
            requires_unique_params,
        } => {
            record["aims_fip_conditional_requires_unique_params"] = json!(requires_unique_params);
        }
        FipContract::Never | FipContract::Certified => {}
    }

    record
}
