//! Unit tests for the `emit-aims-state` producer.
//!
//! Covers the explicit `snake_case` enum match tables (positive mappings +
//! negative pins that a renamed variant or a `Debug`-derive substitution would
//! break), the 19-property record projection, and an end-to-end emit on a tiny
//! compiled fixture.

use ori_arc::aims::contract::FipContract;
use ori_arc::aims::lattice::{
    AccessClass, AimsState, Cardinality, Consumption, EffectClass, Locality, ReuseCtorKind,
    ShapeClass, Uniqueness,
};
use ori_arc::MemoryContract;
use serde_json::Value;

use super::record::{
    access_name, build_record, cardinality_name, consumption_name, fip_name, lattice_value_json,
    locality_name, shape_name, uniqueness_name, RecordIdentity, REPO, SCHEMA_VERSION,
};

// Enum match-table positive mappings.

#[test]
fn access_name_maps_each_variant_to_snake_case() {
    assert_eq!(access_name(AccessClass::Borrowed), "borrowed");
    assert_eq!(access_name(AccessClass::Owned), "owned");
}

#[test]
fn consumption_name_maps_each_variant_to_snake_case() {
    assert_eq!(consumption_name(Consumption::Dead), "dead");
    assert_eq!(consumption_name(Consumption::Linear), "linear");
    assert_eq!(consumption_name(Consumption::Affine), "affine");
    assert_eq!(consumption_name(Consumption::Unrestricted), "unrestricted");
}

#[test]
fn cardinality_name_maps_each_variant_to_snake_case() {
    assert_eq!(cardinality_name(Cardinality::Absent), "absent");
    assert_eq!(cardinality_name(Cardinality::Once), "once");
    assert_eq!(cardinality_name(Cardinality::Many), "many");
}

#[test]
fn uniqueness_name_maps_each_variant_to_snake_case() {
    assert_eq!(uniqueness_name(Uniqueness::Unique), "unique");
    assert_eq!(uniqueness_name(Uniqueness::MaybeShared), "maybe_shared");
    assert_eq!(uniqueness_name(Uniqueness::Shared), "shared");
}

#[test]
fn locality_name_maps_each_variant_to_snake_case() {
    assert_eq!(locality_name(Locality::BlockLocal), "block_local");
    assert_eq!(locality_name(Locality::FunctionLocal), "function_local");
    assert_eq!(locality_name(Locality::HeapEscaping), "heap_escaping");
    assert_eq!(locality_name(Locality::Unknown), "unknown");
}

#[test]
fn shape_name_maps_each_variant_to_snake_case() {
    assert_eq!(shape_name(ShapeClass::NonReusable), "non_reusable");
    assert_eq!(
        shape_name(ShapeClass::ReusableCtor(ReuseCtorKind::Struct)),
        "reusable_ctor_struct"
    );
    assert_eq!(
        shape_name(ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant)),
        "reusable_ctor_enum_variant"
    );
    assert_eq!(
        shape_name(ShapeClass::CollectionBuffer),
        "collection_buffer"
    );
    assert_eq!(shape_name(ShapeClass::ContextHole), "context_hole");
}

#[test]
fn fip_name_maps_each_discriminant_to_snake_case() {
    assert_eq!(fip_name(&FipContract::Never), "never");
    assert_eq!(
        fip_name(&FipContract::Conditional {
            requires_unique_params: vec![true]
        }),
        "conditional"
    );
    assert_eq!(fip_name(&FipContract::Certified), "certified");
    assert_eq!(fip_name(&FipContract::Bounded(2)), "bounded");
}

// Negative pins: the explicit snake_case contract is NOT the `Debug` derive.
// A rename of a variant forces an edit to the match table (caught above); a
// silent swap to `format!("{:?}")` would re-introduce the PascalCase Debug form
// these pins reject.

#[test]
fn names_are_not_debug_derive_artifacts() {
    assert_ne!(
        uniqueness_name(Uniqueness::MaybeShared),
        format!("{:?}", Uniqueness::MaybeShared)
    );
    assert_ne!(
        locality_name(Locality::BlockLocal),
        format!("{:?}", Locality::BlockLocal)
    );
    assert_ne!(
        shape_name(ShapeClass::CollectionBuffer),
        format!("{:?}", ShapeClass::CollectionBuffer)
    );
    // The Debug form of `Uniqueness::MaybeShared` is `"MaybeShared"`; the
    // canonical projection is `"maybe_shared"`.
    assert_eq!(format!("{:?}", Uniqueness::MaybeShared), "MaybeShared");
    assert_eq!(uniqueness_name(Uniqueness::MaybeShared), "maybe_shared");
}

// Lattice-value projection.

#[test]
fn lattice_value_json_carries_position_and_all_seven_dims() {
    let state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::MaybeShared,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::CollectionBuffer,
        effect: EffectClass::ALL,
    };
    let value = lattice_value_json(7, state);

    assert_eq!(value["position"], Value::from(7));
    assert_eq!(value["access"], Value::from("owned"));
    assert_eq!(value["consumption"], Value::from("linear"));
    assert_eq!(value["cardinality"], Value::from("once"));
    assert_eq!(value["uniqueness"], Value::from("maybe_shared"));
    assert_eq!(value["locality"], Value::from("function_local"));
    assert_eq!(value["shape"], Value::from("collection_buffer"));
    assert_eq!(value["aims_effect_may_alloc"], Value::from(true));
    assert_eq!(value["aims_effect_may_share"], Value::from(true));
    assert_eq!(value["aims_effect_may_throw"], Value::from(true));
}

// Record projection: 19-property surface + identity + FipContract payloads.

fn assert_nineteen_property_surface(record: &Value) {
    for key in [
        "aims_param_count",
        "aims_is_fbip",
        "aims_return_uniqueness",
        "aims_return_preserves_freshness",
        "aims_return_locality",
        "aims_return_shape",
        "aims_return_returns_fresh_self_alloc",
        "aims_return_returns_sharing_view",
        "aims_effect_may_allocate",
        "aims_effect_alloc_only_on_slow_path",
        "aims_effect_may_deallocate",
        "aims_effect_may_share",
        "aims_effect_may_throw",
        "aims_effect_has_unbounded_stack",
        "aims_fip",
        "aims_fip_construct_count",
        "aims_fip_consumed_count",
    ] {
        assert!(
            record.get(key).is_some(),
            "record missing 19-property surface key `{key}`"
        );
    }
    // Identity surface.
    for key in [
        "schema_version",
        "repo",
        "qualified_name",
        "file",
        "line",
        "scip_moniker",
        "lattice_values",
    ] {
        assert!(
            record.get(key).is_some(),
            "record missing identity key `{key}`"
        );
    }
}

#[test]
fn build_record_emits_full_surface_with_conditional_fip_array() {
    let mut contract = MemoryContract::conservative(2);
    contract.fip = FipContract::Conditional {
        requires_unique_params: vec![true, false],
    };
    let id = RecordIdentity {
        qualified_name: "build_list",
        file: "fixture.ori",
        line: 1,
        scip_moniker: "ori . . build_list().",
    };
    let record = build_record(
        &id,
        &contract,
        3,
        1,
        vec![lattice_value_json(0, AimsState::TOP)],
    );

    assert_nineteen_property_surface(&record);
    assert_eq!(record["schema_version"], Value::from(SCHEMA_VERSION));
    assert_eq!(record["repo"], Value::from(REPO));
    assert_eq!(record["aims_param_count"], Value::from(2));
    assert_eq!(record["aims_fip"], Value::from("conditional"));
    assert_eq!(record["aims_fip_construct_count"], Value::from(3));
    assert_eq!(record["aims_fip_consumed_count"], Value::from(1));
    // Conditional payload is serialized as a JSON array of bools, not
    // discriminant-only.
    assert_eq!(
        record["aims_fip_conditional_requires_unique_params"],
        Value::from(vec![true, false])
    );
    // All 6 ReturnContract fields are present.
    assert!(record.get("aims_return_returns_fresh_self_alloc").is_some());
    assert!(record.get("aims_return_returns_sharing_view").is_some());
    // Lattice values carried through.
    assert_eq!(record["lattice_values"].as_array().map(Vec::len), Some(1));
}

#[test]
fn build_record_serializes_bounded_fip_payload() {
    let mut contract = MemoryContract::conservative(0);
    contract.fip = FipContract::Bounded(4);
    let id = RecordIdentity {
        qualified_name: "f",
        file: "fixture.ori",
        line: 1,
        scip_moniker: "ori . . f().",
    };
    let record = build_record(&id, &contract, 0, 0, vec![]);

    assert_eq!(record["aims_fip"], Value::from("bounded"));
    assert_eq!(record["aims_fip_bounded"], Value::from(4));
    // A non-Conditional fip never emits the conditional array.
    assert!(record
        .get("aims_fip_conditional_requires_unique_params")
        .is_none());
}

#[test]
fn build_record_serializes_returns_sharing_view_bool() {
    let id = RecordIdentity {
        qualified_name: "sharing_view",
        file: "fixture.ori",
        line: 1,
        scip_moniker: "ori . . sharing_view().",
    };

    for expected in [false, true] {
        let mut contract = MemoryContract::conservative(0);
        contract.return_info.returns_sharing_view = expected;
        let record = build_record(&id, &contract, 0, 0, vec![]);

        assert_eq!(
            record["aims_return_returns_sharing_view"],
            Value::from(expected)
        );
    }
}

// End-to-end: emit on a tiny compiled fixture, asserting the producer drives the
// real pipeline and projects `aims_*` props + per-variable lattice records.

#[test]
fn build_records_emits_aims_state_for_compiled_fixture() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "ori_emit_aims_state_e2e_{}.ori",
        std::process::id()
    ));
    std::fs::write(&path, "@build_list (n: int) -> [int] = [n, n, n];\n")
        .unwrap_or_else(|e| panic!("write fixture: {e}"));

    let Some(path_str) = path.to_str() else {
        panic!("temp fixture path is not valid utf8");
    };
    let result = super::build_records(path_str);
    let _ = std::fs::remove_file(&path);

    let records =
        result.unwrap_or_else(|e| panic!("producer should emit records for a valid fixture: {e}"));
    assert!(
        !records.is_empty(),
        "expected at least one record for the top-level function"
    );

    let Some((_, record)) = records
        .iter()
        .find(|(_, v)| v["qualified_name"] == "build_list")
    else {
        panic!("no record emitted for build_list");
    };

    assert_nineteen_property_surface(record);
    assert_eq!(record["aims_param_count"], Value::from(1));
    assert_eq!(record["repo"], Value::from(REPO));
    assert_eq!(record["qualified_name"], Value::from("build_list"));
    // The SCIP moniker is the function's minted symbol string.
    assert!(record["scip_moniker"]
        .as_str()
        .is_some_and(|m| m.contains("build_list")));

    // The `[n, n, n]` collection-buffer result is a non-scalar variable, so at
    // least one position-qualified lattice value must be present.
    let Some(lattice) = record["lattice_values"].as_array() else {
        panic!("lattice_values must be a JSON array");
    };
    assert!(
        !lattice.is_empty(),
        "expected per-variable lattice records for the collection result"
    );
    let first = &lattice[0];
    assert!(first.get("position").is_some());
    for dim in [
        "access",
        "consumption",
        "cardinality",
        "uniqueness",
        "locality",
        "shape",
    ] {
        assert!(
            first.get(dim).and_then(Value::as_str).is_some(),
            "lattice value missing dim `{dim}`"
        );
    }

    // Shape SSOT pin: `[n, n, n]` lowers to a `CollectionBuffer` Construct, so
    // the per-variable `shape` projection MUST surface `collection_buffer` for
    // that variable — NOT the `non_reusable` the flat-lattice block-state join
    // collapses every reusable shape to. Permanent regression guard: reverting
    // the `var_shape`-sourcing re-collapses every per-variable shape to
    // `non_reusable` and this assertion fails.
    let shapes: Vec<&str> = lattice
        .iter()
        .filter_map(|lv| lv.get("shape").and_then(Value::as_str))
        .collect();
    assert!(
        shapes.contains(&"collection_buffer"),
        "expected a `collection_buffer` per-variable shape for the `[n, n, n]` \
         collection literal; got {shapes:?} (a uniform `non_reusable` set means \
         the per-variable shape SSOT `var_shape` is not being projected)"
    );
}

// Negative pin: `build_records` PROPAGATES a broken-pipeline error as `Err`
// rather than swallowing it and emitting partial / unsound JSONL (the
// LEAK:swallowed-error discipline the frontend-error gate + the ARC-problems
// gate share). A syntactically-invalid source fails the frontend; reverting the
// gate to `Ok(records)` on a broken pipeline fails this assertion.
#[test]
fn build_records_returns_err_on_broken_pipeline() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "ori_emit_aims_state_err_{}.ori",
        std::process::id()
    ));
    // Unterminated function body — the frontend reports errors.
    std::fs::write(&path, "@broken (n: int) -> int = {\n")
        .unwrap_or_else(|e| panic!("write fixture: {e}"));

    let Some(path_str) = path.to_str() else {
        panic!("temp fixture path is not valid utf8");
    };
    let result = super::build_records(path_str);
    let _ = std::fs::remove_file(&path);

    assert!(
        result.is_err(),
        "build_records must return Err on a broken pipeline, never Ok with \
         partial/unsound AIMS state"
    );
}

#[test]
fn build_records_rejects_const_errors_before_aims_lowering() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "ori_emit_aims_state_const_err_{}.ori",
        std::process::id()
    ));
    std::fs::write(&path, "$unsupported = [1];\n@valid () -> int = 0;\n")
        .unwrap_or_else(|e| panic!("write fixture: {e}"));

    let Some(path_str) = path.to_str() else {
        panic!("temp fixture path is not valid utf8");
    };
    let result = super::build_records(path_str);
    let _ = std::fs::remove_file(&path);

    let Err(error) = result else {
        panic!("E2058 must stop AIMS state emission");
    };
    assert!(error.contains("E2058"), "actual error: {error}");
    assert!(
        error.contains("composite value"),
        "the actionable constant-evaluation cause must survive: {error}"
    );
}
