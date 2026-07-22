//! Tests for `ori_repr` types.

use crate::canonical::{canonical, canonical_cached};
use crate::enum_repr::{EnumTag, VariantRepr};
use crate::escape::EscapeInfo;
use crate::plan::{DecisionReason, DecisionSource, NarrowingPolicy, RcStrategy, ReprDecision};
use crate::range::ValueRange;
use crate::repr::{FloatWidth, IntWidth, MachineRepr};
use crate::struct_repr::{ClosureRepr, FatRepr, FieldRepr, RcRepr, StructRepr, TupleRepr};
use crate::ReprAttribute;
use crate::ReprPlan;
use ori_arc::ir::{
    AllocationSiteId, ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership,
    ValueRepr, YieldAllocationExecution, YieldAllocationFact, YieldAllocationLocality, YieldExtent,
};
use ori_arc::ArcBlockId;
use ori_ir::Name;
use ori_types::{ExportedTypeMetadata, Idx, Pool};
use rustc_hash::FxHashMap;

// IntWidth / FloatWidth

#[test]
fn int_width_sizes() {
    assert_eq!(IntWidth::I8.size_bytes(), 1);
    assert_eq!(IntWidth::I16.size_bytes(), 2);
    assert_eq!(IntWidth::I32.size_bytes(), 4);
    assert_eq!(IntWidth::I64.size_bytes(), 8);
}

#[test]
fn int_width_alignment_matches_size() {
    for width in [IntWidth::I8, IntWidth::I16, IntWidth::I32, IntWidth::I64] {
        assert_eq!(width.alignment(), width.size_bytes());
    }
}

#[test]
fn float_width_sizes() {
    assert_eq!(FloatWidth::F32.size_bytes(), 4);
    assert_eq!(FloatWidth::F64.size_bytes(), 8);
}

#[test]
fn float_width_alignment_matches_size() {
    for width in [FloatWidth::F32, FloatWidth::F64] {
        assert_eq!(width.alignment(), width.size_bytes());
    }
}

// MachineRepr

#[test]
fn machine_repr_int_canonical() {
    let repr = MachineRepr::Int {
        width: IntWidth::I64,
        signed: true,
    };
    assert_eq!(
        repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        }
    );
}

#[test]
fn machine_repr_clone_eq() {
    let repr = MachineRepr::Float {
        width: FloatWidth::F64,
    };
    let cloned = repr.clone();
    assert_eq!(repr, cloned);
}

#[test]
fn machine_repr_stack_promoted() {
    let inner = MachineRepr::Int {
        width: IntWidth::I32,
        signed: true,
    };
    let promoted = MachineRepr::StackPromoted {
        inner: Box::new(inner.clone()),
        had_rc: true,
    };
    if let MachineRepr::StackPromoted { inner: i, had_rc } = &promoted {
        assert_eq!(i.as_ref(), &inner);
        assert!(had_rc);
    } else {
        panic!("expected StackPromoted");
    }
}

// FatRepr

#[test]
fn fat_repr_str_vs_collection() {
    let str_repr = FatRepr::Str;
    let col_repr = FatRepr::Collection {
        element_repr: Box::new(MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }),
    };
    assert_ne!(str_repr, col_repr);
}

// ClosureRepr

#[test]
fn closure_repr_preserves_parameter_and_return_shapes() {
    let closure = ClosureRepr {
        params: vec![MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }],
        ret: Box::new(MachineRepr::Bool),
    };
    assert_eq!(closure.params.len(), 1);
    assert_eq!(*closure.ret, MachineRepr::Bool);
}

// StructRepr / FieldRepr

#[test]
fn struct_repr_empty() {
    let s = StructRepr {
        fields: vec![],
        size: 0,
        align: 1,
        trivial: true,
    };
    assert!(s.trivial);
    assert!(s.fields.is_empty());
}

#[test]
fn field_repr_preserves_original_index() {
    let field = FieldRepr {
        name: Name::new(0, 42),
        original_index: 3,
        offset: 16,
        repr: MachineRepr::Bool,
    };
    assert_eq!(field.original_index, 3);
    assert_eq!(field.offset, 16);
}

// TupleRepr

#[test]
fn tuple_repr_two_elements() {
    let t = TupleRepr {
        elements: vec![
            FieldRepr {
                name: Name::new(0, 0),
                original_index: 0,
                offset: 0,
                repr: MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                },
            },
            FieldRepr {
                name: Name::new(0, 1),
                original_index: 1,
                offset: 8,
                repr: MachineRepr::Bool,
            },
        ],
        size: 16,
        align: 8,
        trivial: true,
    };
    assert_eq!(t.elements.len(), 2);
    assert!(t.trivial);
}

// RcRepr

#[test]
fn rc_repr_default_canonical() {
    let rc = RcRepr {
        rc_width: IntWidth::I64,
        atomic: true,
        inner: Box::new(MachineRepr::Struct(StructRepr {
            fields: vec![],
            size: 0,
            align: 1,
            trivial: true,
        })),
        stack_promotable: false,
    };
    assert!(rc.atomic);
    assert!(!rc.stack_promotable);
    assert_eq!(rc.rc_width, IntWidth::I64);
}

// EnumRepr / EnumTag / VariantRepr

#[test]
fn enum_tag_explicit() {
    let tag = EnumTag::Explicit {
        width: IntWidth::I64,
    };
    assert_eq!(
        tag,
        EnumTag::Explicit {
            width: IntWidth::I64
        }
    );
}

#[test]
fn enum_tag_niche() {
    let tag = EnumTag::Niche {
        field_index: 0,
        niche_value: 0,
        niche_variant_idx: 0,
    };
    if let EnumTag::Niche {
        field_index,
        niche_value,
        niche_variant_idx,
    } = tag
    {
        assert_eq!(field_index, 0);
        assert_eq!(niche_value, 0);
        assert_eq!(niche_variant_idx, 0);
    } else {
        panic!("expected Niche");
    }
}

#[test]
fn variant_repr_unit_is_not_pointer() {
    let v = VariantRepr {
        name: Name::new(0, 10),
        fields: vec![],
        size: 0,
        alignment: 1,
    };
    assert!(!v.is_pointer());
}

#[test]
fn variant_repr_single_fat_pointer_is_pointer() {
    let v = VariantRepr {
        name: Name::new(0, 11),
        fields: vec![MachineRepr::FatPointer(FatRepr::Str)],
        size: 24,
        alignment: 8,
    };
    assert!(v.is_pointer());
}

#[test]
fn variant_repr_single_opaque_is_pointer() {
    let v = VariantRepr {
        name: Name::new(0, 12),
        fields: vec![MachineRepr::OpaquePtr],
        size: 8,
        alignment: 8,
    };
    assert!(v.is_pointer());
}

#[test]
fn variant_repr_scalar_not_pointer() {
    let v = VariantRepr {
        name: Name::new(0, 13),
        fields: vec![MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }],
        size: 8,
        alignment: 8,
    };
    assert!(!v.is_pointer());
}

#[test]
fn variant_repr_two_fields_not_pointer() {
    let v = VariantRepr {
        name: Name::new(0, 14),
        fields: vec![MachineRepr::OpaquePtr, MachineRepr::Bool],
        size: 16,
        alignment: 8,
    };
    assert!(!v.is_pointer());
}

// Range and escape metadata

#[test]
fn value_range_is_interval_lattice() {
    // ValueRange is a 3-variant enum (Bottom, Bounded, Top).
    // Verify semantic contract only — layout is not part of the API.
    assert_eq!(ValueRange::default(), ValueRange::Top);
    assert_eq!(
        ValueRange::Bounded { lo: 0, hi: 10 }.join(ValueRange::Bounded { lo: 5, hi: 20 }),
        ValueRange::Bounded { lo: 0, hi: 20 }
    );
    assert_eq!(
        ValueRange::Bounded { lo: 0, hi: 10 }.meet(ValueRange::Bounded { lo: 5, hi: 20 }),
        ValueRange::Bounded { lo: 5, hi: 10 }
    );
}

fn yield_fact(
    site: u32,
    builder: u32,
    result: u32,
    elem_size: u64,
    extent: YieldExtent,
    locality: YieldAllocationLocality,
) -> YieldAllocationFact {
    YieldAllocationFact {
        site: AllocationSiteId::new(site),
        builder: ArcVarId::new(builder),
        result: ArcVarId::new(result),
        elem_ty: ori_types::Idx::BOOL,
        elem_size,
        extent,
        locality,
        execution: YieldAllocationExecution::SingleExecution,
    }
}

#[test]
fn escape_info_only_admits_aims_proven_local_identities() {
    let local = yield_fact(
        0,
        1,
        2,
        1,
        YieldExtent::StaticExact(4),
        YieldAllocationLocality::Local,
    );
    let escaping = yield_fact(
        1,
        3,
        4,
        1,
        YieldExtent::StaticExact(4),
        YieldAllocationLocality::Escaping,
    );
    let unknown = yield_fact(
        2,
        5,
        6,
        1,
        YieldExtent::StaticExact(4),
        YieldAllocationLocality::Unknown,
    );
    let info = EscapeInfo::from_yield_allocations(&[local, escaping, unknown]);

    assert!(!info.escapes(local.builder));
    assert!(!info.escapes(local.result));
    assert!(info.escapes(escaping.builder));
    assert!(info.escapes(escaping.result));
    assert!(info.escapes(unknown.builder));
    assert!(info.escapes(unknown.result));
    assert!(info.escapes(ArcVarId::new(99)));
}

#[test]
fn yield_allocation_selection_is_exact_bounded_and_fail_closed() {
    let function = Name::new(0, 91);
    let local = yield_fact(
        0,
        1,
        2,
        8,
        YieldExtent::StaticExact(32),
        YieldAllocationLocality::Local,
    );

    let oversized = yield_fact(
        1,
        3,
        4,
        8,
        YieldExtent::StaticExact(513),
        YieldAllocationLocality::Local,
    );

    let dynamic = yield_fact(
        2,
        5,
        6,
        8,
        YieldExtent::RuntimeExact(ArcVarId::new(7)),
        YieldAllocationLocality::Local,
    );

    let escaping = yield_fact(
        3,
        8,
        9,
        8,
        YieldExtent::StaticExact(32),
        YieldAllocationLocality::Escaping,
    );

    let unknown = yield_fact(
        5,
        12,
        13,
        8,
        YieldExtent::StaticExact(32),
        YieldAllocationLocality::Unknown,
    );

    let at_limit = yield_fact(
        6,
        14,
        15,
        8,
        YieldExtent::StaticExact(512),
        YieldAllocationLocality::Local,
    );

    let overflow = yield_fact(
        7,
        16,
        17,
        u64::MAX,
        YieldExtent::StaticExact(2),
        YieldAllocationLocality::Local,
    );

    let mut repeated = yield_fact(
        4,
        10,
        11,
        8,
        YieldExtent::StaticExact(32),
        YieldAllocationLocality::Local,
    );
    repeated.execution = YieldAllocationExecution::RepeatedOrUnknown;
    let mut facts = FxHashMap::default();
    facts.insert(
        function,
        vec![
            local, oversized, dynamic, escaping, unknown, at_limit, overflow, repeated,
        ],
    );
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.freeze_yield_allocations(&facts);

    let Some(local_decision) = plan.yield_allocation_for_builder(function, local.builder) else {
        panic!("local allocation decision");
    };

    assert_eq!(
        local_decision.mechanism,
        crate::CompiledAllocationMechanism::StackSlot
    );

    let Some(at_limit_decision) = plan.yield_allocation_for_builder(function, at_limit.builder)
    else {
        panic!("at-limit allocation decision");
    };

    assert_eq!(
        at_limit_decision.mechanism,
        crate::CompiledAllocationMechanism::StackSlot
    );

    for fact in [oversized, dynamic, escaping, unknown, overflow, repeated] {
        let Some(decision) = plan.yield_allocation_for_result(function, fact.result) else {
            panic!("managed allocation decision");
        };

        assert_eq!(
            decision.mechanism,
            crate::CompiledAllocationMechanism::RuntimeHeap
        );
    }
}

#[test]
fn yield_header_elision_requires_exact_runtime_call_targets() {
    let function_name = Name::new(0, 95);
    let observer_name = Name::new(0, 96);
    let result = ArcVarId::new(0);
    let observed = ArcVarId::new(1);
    let fact = yield_fact(
        0,
        2,
        result.raw(),
        1,
        YieldExtent::StaticExact(4),
        YieldAllocationLocality::Local,
    );
    let function = ArcFunction {
        name: function_name,
        return_type: ori_types::Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Apply {
                dst: observed,
                ty: ori_types::Idx::INT,
                func: observer_name,
                args: vec![result],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: observed },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::BOOL, ori_types::Idx::INT],
        var_reprs: vec![ValueRepr::RcPointer, ValueRepr::Scalar],
        spans: vec![vec![None]],
        yield_allocations: vec![fact],
        ..ArcFunction::default()
    };
    let facts = FxHashMap::from_iter([(function_name, vec![fact])]);
    let pool = ori_types::Pool::new();

    let mut runtime_plan = ReprPlan::new(NarrowingPolicy::Disabled);
    runtime_plan.freeze_yield_allocations(&facts);
    runtime_plan.close_yield_runtime_header_requirements(
        std::slice::from_ref(&function),
        &pool,
        |_, dst| (dst == observed).then_some(crate::plan::YieldLineageRuntimeCall::BorrowedRead),
    );
    let Some(runtime_decision) = runtime_plan.yield_allocation_for_result(function_name, result)
    else {
        panic!("runtime-target yield decision");
    };
    assert!(!runtime_decision.requires_runtime_header);

    let mut exact_plan = ReprPlan::new(NarrowingPolicy::Disabled);
    exact_plan.freeze_yield_allocations(&facts);
    exact_plan.close_yield_runtime_header_requirements(&[function], &pool, |_, _| None);
    let Some(exact_decision) = exact_plan.yield_allocation_for_result(function_name, result) else {
        panic!("exact-target yield decision");
    };
    assert!(
        exact_decision.requires_runtime_header,
        "same-spelled local/imported callables must fail closed to headerful storage"
    );
}

// Semantic Pin Tests

/// Semantic pin: canonical int MUST be I64 signed.
/// This test fails if the default is changed.
#[test]
fn semantic_pin_canonical_int_is_i64_signed() {
    let repr = MachineRepr::Int {
        width: IntWidth::I64,
        signed: true,
    };
    assert_eq!(
        repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "canonical int must be I64 signed — changing this breaks semantic equivalence"
    );
}

/// Semantic pin: canonical float MUST be F64.
#[test]
fn semantic_pin_canonical_float_is_f64() {
    let repr = MachineRepr::Float {
        width: FloatWidth::F64,
    };
    assert_eq!(
        repr,
        MachineRepr::Float {
            width: FloatWidth::F64
        },
        "canonical float must be F64 — changing this breaks semantic equivalence"
    );
}

// Canonical Mapping Tests

/// Test canonical mapping for all 12 primitive types.
#[test]
fn canonical_primitives() {
    let pool = Pool::new();

    assert_eq!(
        canonical(&pool, Idx::INT),
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        }
    );
    assert_eq!(
        canonical(&pool, Idx::FLOAT),
        MachineRepr::Float {
            width: FloatWidth::F64
        }
    );
    assert_eq!(canonical(&pool, Idx::BOOL), MachineRepr::Bool);
    assert_eq!(
        canonical(&pool, Idx::STR),
        MachineRepr::FatPointer(FatRepr::Str)
    );
    assert_eq!(canonical(&pool, Idx::CHAR), MachineRepr::Char);
    assert_eq!(canonical(&pool, Idx::BYTE), MachineRepr::Byte);
    assert_eq!(canonical(&pool, Idx::UNIT), MachineRepr::Unit);
    assert_eq!(canonical(&pool, Idx::NEVER), MachineRepr::Never);
    assert_eq!(canonical(&pool, Idx::DURATION), MachineRepr::Duration);
    assert_eq!(canonical(&pool, Idx::SIZE), MachineRepr::Size);
    assert_eq!(canonical(&pool, Idx::ORDERING), MachineRepr::Ordering);
}

/// Semantic pin: canonical(Int) MUST return I64 signed.
#[test]
fn semantic_pin_canonical_int_mapping() {
    let pool = Pool::new();
    assert_eq!(
        canonical(&pool, Idx::INT),
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "canonical(Int) must be I64 signed — changing breaks semantic equivalence"
    );
}

/// Semantic pin: canonical(Float) MUST return F64.
#[test]
fn semantic_pin_canonical_float_mapping() {
    let pool = Pool::new();
    assert_eq!(
        canonical(&pool, Idx::FLOAT),
        MachineRepr::Float {
            width: FloatWidth::F64
        },
        "canonical(Float) must be F64 — changing breaks semantic equivalence"
    );
}

/// Test canonical mapping for List<int> — fat pointer with element repr.
#[test]
fn canonical_list_int() {
    let mut pool = Pool::new();
    let list_idx = pool.list(Idx::INT);
    let repr = canonical(&pool, list_idx);
    assert_eq!(
        repr,
        MachineRepr::FatPointer(FatRepr::Collection {
            element_repr: Box::new(MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            })
        })
    );
}

/// Test canonical mapping for Set<str> — fat pointer.
#[test]
fn canonical_set_str() {
    let mut pool = Pool::new();
    let set_idx = pool.set(Idx::STR);
    let repr = canonical(&pool, set_idx);
    assert_eq!(
        repr,
        MachineRepr::FatPointer(FatRepr::Collection {
            element_repr: Box::new(MachineRepr::FatPointer(FatRepr::Str))
        })
    );
}

/// Test canonical mapping for Map<str, int> — retains both key and value reprs.
#[test]
fn canonical_map() {
    let mut pool = Pool::new();
    let map_idx = pool.map(Idx::STR, Idx::INT);
    let repr = canonical(&pool, map_idx);
    assert_eq!(
        repr,
        MachineRepr::FatPointer(FatRepr::Map {
            key_repr: Box::new(MachineRepr::FatPointer(FatRepr::Str)),
            value_repr: Box::new(MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }),
        })
    );
}

/// Test canonical mapping for Range — always {i64, i64, i64, i64}.
#[test]
fn canonical_range() {
    let mut pool = Pool::new();
    let range_idx = pool.range(Idx::INT);
    assert_eq!(canonical(&pool, range_idx), MachineRepr::Range);
}

/// Test canonical mapping for Iterator — opaque pointer.
#[test]
fn canonical_iterator() {
    let mut pool = Pool::new();
    let iter_idx = pool.iterator(Idx::INT);
    assert_eq!(canonical(&pool, iter_idx), MachineRepr::UnmanagedPtr);
}

/// Test canonical mapping for Channel — opaque pointer.
#[test]
fn canonical_channel() {
    let mut pool = Pool::new();
    let chan_idx = pool.channel(Idx::INT);
    assert_eq!(canonical(&pool, chan_idx), MachineRepr::OpaquePtr);
}

/// Test canonical mapping for Option<int> — 2-variant enum.
#[test]
fn canonical_option_int() {
    let mut pool = Pool::new();
    let opt_idx = pool.option(Idx::INT);
    let repr = canonical(&pool, opt_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 2, "Option should have 2 variants");
        // Option uses I64 tag (not narrowed) for ori_rt runtime compatibility
        assert_eq!(
            e.tag,
            EnumTag::Explicit {
                width: IntWidth::I64
            }
        );
        // Some variant (index 0) has one field = Int
        assert_eq!(e.variants[0].fields.len(), 1);
        assert_eq!(
            e.variants[0].fields[0],
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        // None variant (index 1) has no fields
        assert!(e.variants[1].fields.is_empty());
    } else {
        panic!("expected Enum for Option<int>, got {repr:?}");
    }
}

/// Test canonical mapping for Result<int, str> — 2-variant enum.
#[test]
fn canonical_result() {
    let mut pool = Pool::new();
    let result_idx = pool.result(Idx::INT, Idx::STR);
    let repr = canonical(&pool, result_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 2, "Result should have 2 variants");
        // Ok variant has Int
        assert_eq!(
            e.variants[0].fields[0],
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        // Err variant has Str
        assert_eq!(
            e.variants[1].fields[0],
            MachineRepr::FatPointer(FatRepr::Str)
        );
    } else {
        panic!("expected Enum for Result<int, str>, got {repr:?}");
    }
}

/// Test canonical mapping for Function (int) -> bool.
#[test]
fn canonical_function() {
    let mut pool = Pool::new();
    let fn_idx = pool.function1(Idx::INT, Idx::BOOL);
    let repr = canonical(&pool, fn_idx);
    if let MachineRepr::Closure(ref c) = repr {
        assert_eq!(c.params.len(), 1);
        assert_eq!(
            c.params[0],
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        assert_eq!(*c.ret, MachineRepr::Bool);
    } else {
        panic!("expected Closure for function, got {repr:?}");
    }
}

/// Test canonical mapping for Tuple (int, bool).
#[test]
fn canonical_tuple() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.elements.len(), 2);
        assert_eq!(
            t.elements[0].repr,
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        assert_eq!(t.elements[1].repr, MachineRepr::Bool);
        assert!(t.trivial, "tuple of int and bool should be trivial");
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// Test canonical mapping for Tuple with non-trivial element.
#[test]
fn canonical_tuple_nontrivial() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::STR);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert!(!t.trivial, "(int, str) should NOT be trivial");
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// Test canonical mapping for Struct with named fields.
#[test]
fn canonical_struct() {
    let mut pool = Pool::new();
    let name_x = Name::new(0, 100);
    let name_y = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_x, Idx::INT), (name_y, Idx::FLOAT)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(s.fields.len(), 2);
        assert_eq!(s.fields[0].name, name_x);
        assert_eq!(s.fields[1].name, name_y);
        assert_eq!(s.fields[0].original_index, 0);
        assert_eq!(s.fields[1].original_index, 1);
        assert!(s.trivial, "struct of int and float should be trivial");
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// Test canonical mapping for Enum with variants.
#[test]
fn canonical_enum() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 300);
    let a_name = Name::new(0, 301);
    let b_name = Name::new(0, 302);
    let enum_idx = pool.enum_type(
        enum_name,
        &[
            EnumVariant {
                name: a_name,
                field_types: vec![],
            },
            EnumVariant {
                name: b_name,
                field_types: vec![Idx::INT],
            },
        ],
    );
    let repr = canonical(&pool, enum_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 2);
        assert_eq!(
            e.tag,
            EnumTag::Explicit {
                width: IntWidth::I8
            }
        );
        assert!(e.variants[0].fields.is_empty());
        assert_eq!(e.variants[1].fields.len(), 1);
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

/// Test that unresolved Var returns None (not panic).
/// canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_var() {
    let mut pool = Pool::new();
    let var_idx = pool.fresh_var();
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, var_idx, &mut cache).is_none(),
        "Var must return None, not panic"
    );
}

/// Test that `BoundVar` returns None — constructs a real `BoundVar` via `pool.intern`.
/// canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_bound_var() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let bound_var_idx = pool.intern(Tag::BoundVar, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, bound_var_idx, &mut cache).is_none(),
        "BoundVar must return None, not panic"
    );
}

/// Test that `RigidVar` returns None.
/// canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_rigid_var() {
    let mut pool = Pool::new();
    let rigid = pool.rigid_var(Name::new(0, 999));
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, rigid, &mut cache).is_none(),
        "RigidVar must return None, not panic"
    );
}

/// Test that Error type returns None (should not reach codegen).
/// canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_error() {
    let pool = Pool::new();
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, Idx::ERROR, &mut cache).is_none(),
        "Error must return None, not panic"
    );
}

/// Scheme type returns None (should never reach codegen).
/// canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_scheme() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let scheme_idx = pool.scheme(&[0], Idx::INT);
    assert_eq!(pool.tag(scheme_idx), Tag::Scheme);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, scheme_idx, &mut cache).is_none(),
        "Scheme must return None, not panic"
    );
}

/// Infer type returns None (should never reach codegen).
/// canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_infer() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let infer_idx = pool.intern(Tag::Infer, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, infer_idx, &mut cache).is_none(),
        "Infer must return None, not panic"
    );
}

/// Named→Int resolves to same canonical as Int directly.
#[test]
fn canonical_named_resolves_to_int() {
    let mut pool = Pool::new();
    let named_idx = pool.named(Name::new(0, 42));
    pool.set_resolution(named_idx, Idx::INT);

    let repr = canonical(&pool, named_idx);
    assert_eq!(
        repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "Named→Int must resolve to same repr as Int"
    );
}

/// Alias chain A = B = int resolves to Int.
#[test]
fn canonical_alias_chain_resolves() {
    let mut pool = Pool::new();
    // A is a Named type
    let a_idx = pool.named(Name::new(0, 100));
    // B is another Named type
    let b_idx = pool.named(Name::new(0, 200));
    // A → B → Int
    pool.set_resolution(a_idx, b_idx);
    pool.set_resolution(b_idx, Idx::INT);

    let repr = canonical(&pool, a_idx);
    assert_eq!(
        repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "Named chain A→B→Int must resolve to Int"
    );
}

// ABI Layout Tests

/// Semantic pin: (int, bool) must be 16 bytes with ABI padding, not 9.
/// int (8 bytes) at offset 0, bool (1 byte) at offset 8, 7 bytes trailing
/// padding to reach struct alignment of 8.
#[test]
fn canonical_tuple_abi_size() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.size, 16, "(int, bool) must be 16 bytes with ABI padding");
        assert_eq!(t.align, 8, "(int, bool) alignment must be 8");
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// (bool, int) must also be 16 bytes: bool at offset 0, 7 bytes padding,
/// int at offset 8, total 16.
#[test]
fn canonical_tuple_abi_size_reversed() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::BOOL, Idx::INT);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.size, 16, "(bool, int) must be 16 bytes with ABI padding");
        assert_eq!(t.align, 8);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// (bool, bool) is 2 bytes with alignment 1 — no padding needed.
#[test]
fn canonical_tuple_no_padding_needed() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::BOOL, Idx::BOOL);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.size, 2, "(bool, bool) is 2 bytes — no padding");
        assert_eq!(t.align, 1);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// Struct {x: int, y: float} — both 8-byte aligned, no internal padding,
/// total 16 bytes.
#[test]
fn canonical_struct_abi_size() {
    let mut pool = Pool::new();
    let name_x = Name::new(0, 100);
    let name_y = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_x, Idx::INT), (name_y, Idx::FLOAT)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(s.size, 16, "struct(int, float) must be 16 bytes");
        assert_eq!(s.align, 8);
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// Struct {a: bool, b: int} — bool at 0, 7 bytes padding, int at 8, total 16.
#[test]
fn canonical_struct_abi_padding() {
    let mut pool = Pool::new();
    let name_a = Name::new(0, 100);
    let name_b = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_a, Idx::BOOL), (name_b, Idx::INT)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(
            s.size, 16,
            "struct(bool, int) must be 16 bytes with ABI padding"
        );
        assert_eq!(s.align, 8);
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// Map<str, int> semantic pin: must retain both key and value representations.
#[test]
fn canonical_map_retains_value_repr() {
    let mut pool = Pool::new();
    let map_idx = pool.map(Idx::STR, Idx::INT);
    let repr = canonical(&pool, map_idx);
    if let MachineRepr::FatPointer(FatRepr::Map {
        ref key_repr,
        ref value_repr,
    }) = repr
    {
        assert_eq!(**key_repr, MachineRepr::FatPointer(FatRepr::Str));
        assert_eq!(
            **value_repr,
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
    } else {
        panic!("expected FatPointer(Map), got {repr:?}");
    }
}

// Cycle Detection for Recursive Types

/// Recursive enum `type Tree = Leaf(int) | Node(Tree, Tree)` must not
/// stack overflow. Recursive positions yield `RcPointer`.
#[test]
fn canonical_recursive_enum_no_stack_overflow() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let tree_name = Name::new(0, 400);
    let leaf_name = Name::new(0, 401);
    let node_name = Name::new(0, 402);

    // Forward reference for Tree
    let tree_named = pool.named(tree_name);

    let tree_enum = pool.enum_type(
        tree_name,
        &[
            EnumVariant {
                name: leaf_name,
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: node_name,
                field_types: vec![tree_named, tree_named],
            },
        ],
    );

    // Link Named → Enum
    pool.set_resolution(tree_named, tree_enum);

    // Must not infinite loop
    let repr = canonical(&pool, tree_enum);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 2);
        // Leaf variant: one Int field
        assert_eq!(e.variants[0].fields.len(), 1);
        assert_eq!(
            e.variants[0].fields[0],
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        // Node variant: two RcPointer fields (recursive positions)
        assert_eq!(e.variants[1].fields.len(), 2);
        assert!(
            matches!(e.variants[1].fields[0], MachineRepr::RcPointer(_)),
            "recursive field must be RcPointer, got {:?}",
            e.variants[1].fields[0]
        );
        assert!(
            matches!(e.variants[1].fields[1], MachineRepr::RcPointer(_)),
            "recursive field must be RcPointer, got {:?}",
            e.variants[1].fields[1]
        );
    } else {
        panic!("expected Enum for recursive Tree, got {repr:?}");
    }
}

/// Semantic pin: recursive type MUST return `RcPointer` at recursive position,
/// not `OpaquePtr` or infinite recursion.
#[test]
fn semantic_pin_recursive_field_is_rc_pointer() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let list_name = Name::new(0, 500);
    let nil_name = Name::new(0, 501);
    let cons_name = Name::new(0, 502);
    let list_named = pool.named(list_name);

    // type IntList = Nil | Cons(int, IntList)
    let list_enum = pool.enum_type(
        list_name,
        &[
            EnumVariant {
                name: nil_name,
                field_types: vec![],
            },
            EnumVariant {
                name: cons_name,
                field_types: vec![Idx::INT, list_named],
            },
        ],
    );
    pool.set_resolution(list_named, list_enum);

    let repr = canonical(&pool, list_enum);
    if let MachineRepr::Enum(ref e) = repr {
        // Cons variant's second field is the recursive ref
        let cons = &e.variants[1];
        assert_eq!(cons.fields.len(), 2);
        if let MachineRepr::RcPointer(ref rc) = cons.fields[1] {
            assert_eq!(rc.rc_width, IntWidth::I64);
            assert!(rc.atomic);
            assert!(!rc.stack_promotable);
        } else {
            panic!(
                "recursive field must be RcPointer, got {:?}",
                cons.fields[1]
            );
        }
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

/// Mutual recursion canonical-consistency test.
///
/// `type A = WrapA { b: B }`
/// `type B = WrapB { a: A }`
///
/// `canonical(A)` and `canonical(B)` must each produce consistent representations:
/// mutual recursive fields are `RcPointer` and cached representations are stable.
#[test]
fn canonical_mutual_recursion_consistent() {
    let mut pool = Pool::new();

    // Forward references
    let a_name = Name::new(0, 600);
    let b_name = Name::new(0, 601);
    let a_named = pool.named(a_name);
    let b_named = pool.named(b_name);

    let b_field_name = Name::new(0, 602);
    let a_field_name = Name::new(0, 603);

    // type A = struct { b: B }
    let a_struct = pool.struct_type(a_name, &[(b_field_name, b_named)]);
    // type B = struct { a: A }
    let b_struct = pool.struct_type(b_name, &[(a_field_name, a_named)]);

    pool.set_resolution(a_named, a_struct);
    pool.set_resolution(b_named, b_struct);

    // Compute both via shared cache (simulating populate_canonical)
    let mut cache = rustc_hash::FxHashMap::default();
    let Some(a_repr) = crate::canonical::canonical_cached(&pool, a_struct, &mut cache) else {
        panic!("A should canonicalize");
    };
    let Some(b_repr) = crate::canonical::canonical_cached(&pool, b_struct, &mut cache) else {
        panic!("B should canonicalize");
    };

    // Both should be Struct types
    let MachineRepr::Struct(ref a_s) = a_repr else {
        panic!("expected Struct for A, got {a_repr:?}");
    };
    let MachineRepr::Struct(ref b_s) = b_repr else {
        panic!("expected Struct for B, got {b_repr:?}");
    };

    // A has one field (b), B has one field (a)
    assert_eq!(a_s.fields.len(), 1, "A should have 1 field");
    assert_eq!(b_s.fields.len(), 1, "B should have 1 field");

    // A's B field = full B struct (B is first-visited from A, not a cycle).
    // B's A field = RcPointer (A was being visited when B encountered it).
    assert!(
        matches!(a_s.fields[0].repr, MachineRepr::Struct(_)),
        "A's B field should be full Struct (first visit), got {:?}",
        a_s.fields[0].repr
    );
    assert!(
        matches!(b_s.fields[0].repr, MachineRepr::RcPointer(_)),
        "B's A field should be RcPointer (back-edge), got {:?}",
        b_s.fields[0].repr
    );

    // Key consistency check: B nested inside A must equal standalone B.
    // With the shared cache, both resolve to the same representation.
    let b_inside_a = &a_s.fields[0].repr;
    assert_eq!(
        b_inside_a, &b_repr,
        "B nested inside A must equal standalone B (cache consistency)"
    );

    // Semantic pin: calling canonical_cached again returns the same result (cache hit)
    let Some(a_repr2) = crate::canonical::canonical_cached(&pool, a_struct, &mut cache) else {
        panic!("cached A should canonicalize");
    };
    assert_eq!(a_repr, a_repr2, "cached result must be stable");
}

/// A struct containing an Error-typed field returns `None` because the child
/// type cannot be canonicalized.
#[test]
fn canonical_returns_none_for_struct_with_error_child() {
    let mut pool = Pool::new();
    let field_name = Name::new(0, 42);
    // Create a struct with one field of type Error
    let struct_idx = pool.struct_type(Name::new(0, 100), &[(field_name, Idx::ERROR)]);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, struct_idx, &mut cache).is_none(),
        "struct with Error child must return None, not panic"
    );
}

/// semantic pin: an Option wrapping an Error type returns None.
#[test]
fn canonical_returns_none_for_option_of_error() {
    let mut pool = Pool::new();
    let option_idx = pool.option(Idx::ERROR);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, option_idx, &mut cache).is_none(),
        "Option<Error> must return None, not panic"
    );
}

/// semantic pin: a list of Error-typed elements returns None.
#[test]
fn canonical_returns_none_for_list_of_error() {
    let mut pool = Pool::new();
    let list_idx = pool.list(Idx::ERROR);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, list_idx, &mut cache).is_none(),
        "[Error] must return None, not panic"
    );
}

/// semantic pin: `populate_canonical` does not panic on pools
/// that contain Error and type-variable types.
#[test]
fn populate_canonical_no_panics_with_error_types() {
    use crate::plan::NarrowingPolicy;

    let mut pool = Pool::new();
    // Add some valid types
    let _list_int = pool.list(Idx::INT);
    let _option_str = pool.option(Idx::STR);
    // Add some invalid types that should be silently skipped
    let _list_error = pool.list(Idx::ERROR);
    let _var = pool.fresh_var();

    // Invalid types are skipped without requiring panic recovery.
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);

    // Valid types should be populated
    assert!(
        plan.get_repr(Idx::INT).is_some(),
        "Int should have a canonical repr"
    );
}

/// Non-recursive type appearing multiple times is NOT treated as a cycle.
#[test]
fn canonical_non_recursive_repeated_type() {
    let mut pool = Pool::new();
    // (int, int) — int appears twice but is not recursive
    let tuple_idx = pool.pair(Idx::INT, Idx::INT);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.elements.len(), 2);
        // Both should be Int, NOT RcPointer
        assert!(
            matches!(t.elements[0].repr, MachineRepr::Int { .. }),
            "repeated non-recursive type must not be RcPointer"
        );
        assert!(
            matches!(t.elements[1].repr, MachineRepr::Int { .. }),
            "repeated non-recursive type must not be RcPointer"
        );
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

// Unit/Never Zero-Size in Aggregates

/// Semantic pin: ((), bool) size = 1 — Unit contributes 0 bytes in aggregates.
#[test]
fn canonical_tuple_unit_zero_sized() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::UNIT, Idx::BOOL);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(
            t.size, 1,
            "(unit, bool) must be 1 byte — Unit is zero-sized"
        );
        assert_eq!(t.align, 1);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// (bool, (), int) size = 16 — Unit in the middle contributes 0 bytes.
#[test]
fn canonical_tuple_unit_middle() {
    let mut pool = Pool::new();
    let tuple_idx = pool.triple(Idx::BOOL, Idx::UNIT, Idx::INT);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        // bool(1) + padding(7) + int(8) = 16. Unit adds 0.
        assert_eq!(
            t.size, 16,
            "(bool, unit, int) must be 16 — Unit contributes 0"
        );
        assert_eq!(t.align, 8);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

/// Struct with Unit field doesn't inflate size.
#[test]
fn canonical_struct_unit_field() {
    let mut pool = Pool::new();
    let name_a = Name::new(0, 100);
    let name_b = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_a, Idx::BOOL), (name_b, Idx::UNIT)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(s.size, 1, "struct(bool, unit) must be 1 byte");
        assert_eq!(s.align, 1);
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// `Option<()>` — i64 tag + 0 payload. Size = 8 (i64 tag, not narrowed for Option).
#[test]
fn canonical_option_unit_zero_payload() {
    let mut pool = Pool::new();
    let opt_idx = pool.option(Idx::UNIT);
    let repr = canonical(&pool, opt_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(
            e.size, 8,
            "Option<()> must be 8 bytes — i64 tag (not narrowed for runtime compat)"
        );
    } else {
        panic!("expected Enum for Option<()>, got {repr:?}");
    }
}

/// Never-typed field contributes 0 bytes in aggregates.
#[test]
fn canonical_tuple_never_zero_sized() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::NEVER);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(
            t.size, 8,
            "(int, Never) must be 8 bytes — Never is zero-sized"
        );
        assert_eq!(t.align, 8);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

// Recursive Triviality for Compound Types

/// Struct containing a trivial tuple `(int, bool)` must itself be trivial.
#[test]
fn trivial_struct_containing_trivial_tuple() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    let name_t = Name::new(0, 100);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_t, tuple_idx)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert!(
            s.trivial,
            "struct containing (int, bool) must be trivial — all scalars"
        );
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// Struct containing a non-trivial tuple `(int, str)` must be non-trivial.
#[test]
fn nontrivial_struct_containing_nontrivial_tuple() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::STR);
    let name_t = Name::new(0, 100);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_t, tuple_idx)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert!(
            !s.trivial,
            "struct containing (int, str) must NOT be trivial — str has RC"
        );
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// All-unit enum (like `type Color = Red | Green | Blue`) is trivial.
#[test]
fn trivial_all_unit_enum() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 300);
    let enum_idx = pool.enum_type(
        enum_name,
        &[
            EnumVariant {
                name: Name::new(0, 301),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 302),
                field_types: vec![],
            },
        ],
    );
    let repr = canonical(&pool, enum_idx);
    if let MachineRepr::Enum(ref e) = repr {
        let all_trivial = e.variants.iter().all(|v| {
            v.fields.iter().all(|f| {
                !matches!(
                    f,
                    MachineRepr::FatPointer(_)
                        | MachineRepr::RcPointer(_)
                        | MachineRepr::Closure(_)
                        | MachineRepr::OpaquePtr
                )
            })
        });
        assert!(all_trivial, "all-unit enum must be trivial");
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

/// Enum with scalar payloads `Shape = Circle(float) | Rect(float, float)` is trivial.
#[test]
fn trivial_scalar_payload_enum() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 300);
    let enum_idx = pool.enum_type(
        enum_name,
        &[
            EnumVariant {
                name: Name::new(0, 301),
                field_types: vec![Idx::FLOAT],
            },
            EnumVariant {
                name: Name::new(0, 302),
                field_types: vec![Idx::FLOAT, Idx::FLOAT],
            },
        ],
    );
    // Wrap in a struct to test nested triviality
    let name_s = Name::new(0, 100);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_s, enum_idx)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert!(
            s.trivial,
            "struct containing all-scalar enum must be trivial"
        );
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

// ReprDecision Tracking

#[test]
fn repr_plan_set_get_round_trip() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let decision = ReprDecision {
        source: DecisionSource::Canonical,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        },
        reason: DecisionReason::Canonical,
    };
    plan.set_repr(Idx::INT, decision);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        })
    );
}

#[test]
fn repr_plan_override_returns_second_decision() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let d1 = ReprDecision {
        source: DecisionSource::Canonical,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        },
        reason: DecisionReason::Canonical,
    };
    let d2 = ReprDecision {
        source: DecisionSource::IntegerNarrowing,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I32,
            signed: true,
        },
        reason: DecisionReason::RangeFits {
            range: ValueRange::Bounded { lo: 0, hi: 1000 },
            min_width: IntWidth::I32,
        },
    };
    plan.set_repr(Idx::INT, d1);
    plan.set_repr(Idx::INT, d2);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I32,
            signed: true,
        })
    );
}

#[test]
fn repr_plan_audit_trail_preserves_both_decisions() {
    use ori_types::Pool;

    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let d1 = ReprDecision {
        source: DecisionSource::Canonical,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        },
        reason: DecisionReason::Canonical,
    };
    let d2 = ReprDecision {
        source: DecisionSource::IntegerNarrowing,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I32,
            signed: true,
        },
        reason: DecisionReason::RangeFits {
            range: ValueRange::Bounded { lo: 0, hi: 1000 },
            min_width: IntWidth::I32,
        },
    };
    plan.set_repr(Idx::INT, d1);
    plan.set_repr(Idx::INT, d2);
    let audit = plan.dump_audit(&pool);
    assert!(!audit.is_empty());
    // Both entries should be present in order
    assert!(audit.contains("Canonical"), "audit must contain Canonical");
    assert!(
        audit.contains("IntegerNarrowing"),
        "audit must contain IntegerNarrowing"
    );
    // Canonical should appear before IntegerNarrowing (insertion order).
    // Both positions are present, so `Option<usize>` ordering is safe.
    assert!(
        audit.find("Canonical") < audit.find("IntegerNarrowing"),
        "Canonical must appear before IntegerNarrowing in audit trail"
    );
}

#[test]
fn repr_plan_get_unknown_idx_returns_none() {
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(plan.get_repr(Idx::INT).is_none());
}

#[test]
fn repr_plan_var_range_no_recorded_ranges_returns_default() {
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let func = Name::new(0, 1);
    let var = ori_arc::ArcVarId::new(0);
    let range = plan.var_range(func, var);
    assert_eq!(range, ValueRange::default());
}

#[test]
fn repr_plan_set_var_ranges_round_trip_isolated() {
    use rustc_hash::FxHashMap;

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let func_a = Name::new(0, 1);
    let func_b = Name::new(0, 2);
    let var_0 = ori_arc::ArcVarId::new(0);
    let var_1 = ori_arc::ArcVarId::new(1);

    let range_0_100 = ValueRange::Bounded { lo: 0, hi: 100 };
    let range_neg = ValueRange::Bounded { lo: -50, hi: 50 };

    let mut ranges_a = FxHashMap::default();
    ranges_a.insert(var_0, range_0_100);
    plan.set_var_ranges(func_a, ranges_a);

    let mut ranges_b = FxHashMap::default();
    ranges_b.insert(var_1, range_neg);
    plan.set_var_ranges(func_b, ranges_b);

    // func_a has var_0 but not var_1
    assert_eq!(plan.var_range(func_a, var_0), range_0_100);
    assert_eq!(plan.var_range(func_a, var_1), ValueRange::Top);

    // func_b has var_1 but not var_0
    assert_eq!(plan.var_range(func_b, var_1), range_neg);
    assert_eq!(plan.var_range(func_b, var_0), ValueRange::Top);
}

#[test]
fn repr_plan_dump_audit_contains_tag_and_source() {
    use ori_types::Pool;

    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.set_repr(
        Idx::INT,
        ReprDecision {
            source: DecisionSource::Triviality,
            type_idx: Idx::INT,
            repr: MachineRepr::Int {
                width: IntWidth::I64,
                signed: true,
            },
            reason: DecisionReason::TransitivelyTrivial,
        },
    );
    let audit = plan.dump_audit(&pool);
    assert!(!audit.is_empty());
    assert!(audit.contains("int"), "audit must contain type tag 'int'");
    assert!(
        audit.contains("Triviality"),
        "audit must contain source 'Triviality'"
    );
}

// Query Interface Default Values

#[test]
fn int_width_default_returns_i64() {
    // int_width() defaults to I64 when no decision recorded.
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert_eq!(plan.int_width(Idx::INT), IntWidth::I64);
}

#[test]
fn float_width_default_returns_f64() {
    // float_width() defaults to F64 when no decision recorded.
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert_eq!(plan.float_width(Idx::FLOAT), FloatWidth::F64);
}

#[test]
fn is_trivial_default_returns_false() {
    // is_trivial() defaults to false when no decision recorded.
    // Safe default — never elides RC it shouldn't.
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(
        !plan.is_trivial(Idx::INT),
        "safe default must be non-trivial"
    );
}

#[test]
fn escapes_default_returns_true() {
    // escapes() defaults to true when no escape info recorded.
    // Safe default — never stack-promotes when unsure.
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(
        plan.escapes(Name::new(0, 0), ori_arc::ArcVarId::new(0)),
        "safe default must assume escapes"
    );
}

// RC strategy

#[test]
fn rc_strategy_default_is_atomic_i64() {
    // Semantic pin: rc_strategy() must return Atomic { I64 } when no decision
    // has been recorded. This is the documented contract — the repr-opt
    // infrastructure alone causes zero behavioral change.
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert_eq!(
        plan.rc_strategy(Idx::INT),
        RcStrategy::Atomic {
            width: IntWidth::I64,
        },
    );
}

#[test]
fn rc_strategy_default_for_canonical_opaque_ptr() {
    // After populate_canonical(), Iterator/Channel are stored as
    // OpaquePtr. rc_strategy() must still return Atomic { I64 } (the safe
    // default) — NOT RcStrategy::None.
    let mut pool = Pool::new();
    let iter_idx = pool.iterator(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    // Simulate populate_canonical() storing an UnmanagedPtr for an iterator type.
    plan.set_repr(
        iter_idx,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: iter_idx,
            repr: MachineRepr::UnmanagedPtr,
            reason: DecisionReason::Canonical,
        },
    );
    // Must still report Atomic { I64 } — no /decision has been made.
    assert_eq!(
        plan.rc_strategy(iter_idx),
        RcStrategy::Atomic {
            width: IntWidth::I64,
        },
    );
}

#[test]
fn set_rc_strategy_preserves_original_repr() {
    // set_rc_strategy() must NOT overwrite the type's
    // MachineRepr. The original layout must be preserved for codegen.
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let original_repr = MachineRepr::Struct(StructRepr {
        fields: vec![FieldRepr {
            repr: MachineRepr::Int {
                width: IntWidth::I64,
                signed: true,
            },
            original_index: 0,
            offset: 0,
            name: Name::new(0, 1),
        }],
        size: 8,
        align: 8,
        trivial: false,
    });
    plan.set_repr(
        Idx::INT,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: Idx::INT,
            repr: original_repr.clone(),
            reason: DecisionReason::Canonical,
        },
    );
    // Set RC strategy — must NOT destroy the struct layout.
    plan.set_rc_strategy(Idx::INT, RcStrategy::None, DecisionSource::Triviality);
    // The repr must still be the original struct, not OpaquePtr.
    assert_eq!(plan.get_repr(Idx::INT), Some(&original_repr));
}

#[test]
fn set_rc_strategy_write_read_round_trip() {
    // After set_rc_strategy(idx, RcStrategy::None, ...),
    // rc_strategy(idx) returns RcStrategy::None.
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.set_rc_strategy(Idx::INT, RcStrategy::None, DecisionSource::Triviality);
    assert_eq!(plan.rc_strategy(Idx::INT), RcStrategy::None);
}

#[test]
fn set_rc_strategy_non_atomic_round_trip() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let strategy = RcStrategy::NonAtomic {
        width: IntWidth::I16,
    };
    plan.set_rc_strategy(Idx::INT, strategy, DecisionSource::ThreadLocal);
    assert_eq!(plan.rc_strategy(Idx::INT), strategy);
}

#[test]
fn set_rc_strategy_atomic_narrow_round_trip() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let strategy = RcStrategy::Atomic {
        width: IntWidth::I8,
    };
    plan.set_rc_strategy(Idx::INT, strategy, DecisionSource::ArcHeader);
    assert_eq!(plan.rc_strategy(Idx::INT), strategy);
}

#[test]
fn set_rc_strategy_records_audit_entry() {
    use ori_types::Pool;

    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.set_rc_strategy(Idx::INT, RcStrategy::None, DecisionSource::Triviality);
    let audit = plan.dump_audit(&pool);
    assert!(
        audit.contains("Triviality"),
        "audit must contain the RC strategy decision source"
    );
}

// Pipeline Integration Tests

#[test]
fn compute_repr_plan_populates_primitives() {
    // compute_repr_plan() populates canonical representations
    // for all 11 non-error primitive types.
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    // All 11 non-error primitives should have canonical entries.
    assert!(plan.get_repr(Idx::INT).is_some(), "Int must be populated");
    assert!(
        plan.get_repr(Idx::FLOAT).is_some(),
        "Float must be populated"
    );
    assert!(plan.get_repr(Idx::BOOL).is_some(), "Bool must be populated");
    assert!(plan.get_repr(Idx::STR).is_some(), "Str must be populated");
    assert!(plan.get_repr(Idx::CHAR).is_some(), "Char must be populated");
    assert!(plan.get_repr(Idx::BYTE).is_some(), "Byte must be populated");
    assert!(plan.get_repr(Idx::UNIT).is_some(), "Unit must be populated");
    assert!(
        plan.get_repr(Idx::NEVER).is_some(),
        "Never must be populated"
    );
    assert!(
        plan.get_repr(Idx::DURATION).is_some(),
        "Duration must be populated"
    );
    assert!(plan.get_repr(Idx::SIZE).is_some(), "Size must be populated");
    assert!(
        plan.get_repr(Idx::ORDERING).is_some(),
        "Ordering must be populated"
    );
    // Error type IS populated as Unit (trivial sentinel).
    assert!(
        plan.get_repr(Idx::ERROR).is_some(),
        "Error must be populated as Unit"
    );
}

#[test]
fn compute_repr_plan_disabled_policy_preserves_canonical_repr() {
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Disabled, &[]);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        })
    );
    assert_eq!(plan.narrowing_policy(), NarrowingPolicy::Disabled);
}

#[test]
fn compute_repr_plan_aggressive_policy_preserves_canonical_repr() {
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert_eq!(plan.narrowing_policy(), NarrowingPolicy::Aggressive);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        })
    );
}

#[test]
fn compute_repr_plan_canonical_int_semantic_pin() {
    // Canonical Int storage is signed I64.
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }),
        "canonical int must be i64 signed — semantic pin"
    );
}

#[test]
fn compute_repr_plan_zero_behavioral_change_with_disabled() {
    // Identical canonical representations regardless of policy.
    let pool = ori_types::Pool::new();
    let plan_aggressive = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    let plan_disabled = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Disabled, &[]);
    // Both should produce the same canonical repr for every primitive
    // (including ERROR, which is canonicalized as Unit).
    for raw in 0..Idx::PRIMITIVE_COUNT {
        let idx = Idx::from_raw(raw);
        assert_eq!(
            plan_aggressive.get_repr(idx),
            plan_disabled.get_repr(idx),
            "canonical repr for primitive {raw} must match regardless of policy"
        );
    }
}

// ORI_NO_REPR_OPT env var value parsing

#[test]
fn is_env_truthy_accepts_1() {
    assert!(crate::plan::query::is_env_truthy("1"));
}

#[test]
fn is_env_truthy_accepts_true_lowercase() {
    assert!(crate::plan::query::is_env_truthy("true"));
}

#[test]
fn is_env_truthy_accepts_true_uppercase() {
    assert!(crate::plan::query::is_env_truthy("TRUE"));
}

#[test]
fn is_env_truthy_accepts_true_mixed_case() {
    assert!(crate::plan::query::is_env_truthy("True"));
}

#[test]
fn is_env_truthy_accepts_yes_lowercase() {
    assert!(crate::plan::query::is_env_truthy("yes"));
}

#[test]
fn is_env_truthy_accepts_yes_uppercase() {
    assert!(crate::plan::query::is_env_truthy("YES"));
}

#[test]
fn is_env_truthy_rejects_0() {
    assert!(!crate::plan::query::is_env_truthy("0"));
}

#[test]
fn is_env_truthy_rejects_false() {
    assert!(!crate::plan::query::is_env_truthy("false"));
}

#[test]
fn is_env_truthy_rejects_no() {
    assert!(!crate::plan::query::is_env_truthy("no"));
}

#[test]
fn is_env_truthy_rejects_empty() {
    assert!(!crate::plan::query::is_env_truthy(""));
}

#[test]
fn is_env_truthy_rejects_arbitrary() {
    assert!(!crate::plan::query::is_env_truthy("banana"));
}

/// `NarrowingPolicy::env_disabled()` uses strict value parsing rather than
/// treating mere presence as truthy.
#[test]
fn env_disabled_rejects_falsey_values() {
    // Direct parsing avoids racy mutation of process-wide environment state.
    assert!(
        !crate::plan::query::is_env_truthy("0"),
        "0 must not enable --no-repr-opt"
    );
    assert!(
        !crate::plan::query::is_env_truthy("false"),
        "false must not enable --no-repr-opt"
    );
    assert!(
        !crate::plan::query::is_env_truthy(""),
        "empty must not enable --no-repr-opt"
    );
    assert!(
        crate::plan::query::is_env_truthy("1"),
        "1 must enable --no-repr-opt"
    );
    assert!(
        crate::plan::query::is_env_truthy("true"),
        "true must enable --no-repr-opt"
    );
}

// #repr Attribute Integration tests

#[test]
fn repr_c_stored_and_retrieved() {
    // #repr("c") on a struct → ReprAttribute::C in repr_attrs.
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(100), &[]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::C),
        "#repr(\"c\") must be stored as ReprAttribute::C"
    );
}

#[test]
fn repr_packed_stored_and_retrieved() {
    // #repr("packed") → ReprAttribute::Packed stored and retrieved.
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(101), &[]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::Packed)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(plan.repr_attr(struct_idx), Some(&ReprAttribute::Packed),);
}

#[test]
fn repr_transparent_stored_and_retrieved() {
    // #repr("transparent") on a single-field struct → ReprAttribute::Transparent.
    let mut pool = ori_types::Pool::new();
    let field = (ori_ir::Name::from_raw(200), Idx::INT);
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(102), &[field]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::Transparent)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::Transparent),
    );
}

#[test]
fn repr_aligned_stored_and_retrieved() {
    // #repr("aligned", 8) → ReprAttribute::Aligned(8).
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(103), &[]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::Aligned(8))];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(plan.repr_attr(struct_idx), Some(&ReprAttribute::Aligned(8)),);
}

#[test]
fn no_repr_returns_none() {
    // Struct with no #repr → repr_attrs has no entry.
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(104), &[]);
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert_eq!(
        plan.repr_attr(struct_idx),
        None,
        "no #repr → None from query"
    );
}

#[test]
fn repr_c_semantic_pin() {
    // `populate_canonical` stores `#repr("c")` as `ReprAttribute::C`.
    let mut pool = ori_types::Pool::new();
    let name = ori_ir::Name::from_raw(105);
    let f1 = (ori_ir::Name::from_raw(201), Idx::INT);
    let f2 = (ori_ir::Name::from_raw(202), Idx::FLOAT);
    let struct_idx = pool.struct_type(name, &[f1, f2]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(plan.repr_attr(struct_idx), Some(&ReprAttribute::C));
    // Other types should still have None.
    assert_eq!(plan.repr_attr(Idx::INT), None);
}

#[test]
fn repr_c_aligned_stored_and_retrieved() {
    // CAligned(16) from merged c + aligned → ReprAttribute::CAligned(16).
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(106), &[]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::CAligned(16))];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::CAligned(16)),
        "#repr(\"c\") + #repr(\"aligned\", 16) must be stored as ReprAttribute::CAligned(16)"
    );
}

#[test]
fn repr_convert_c_aligned_roundtrip() {
    // Semantic pin: CAligned survives the ReprAttrKind → ReprAttribute conversion.
    let kind = ori_ir::ReprAttrKind::CAligned(32);
    let attr = crate::pipeline::convert_repr_attr_kind(&kind);
    assert_eq!(attr, ReprAttribute::CAligned(32));
}

// Named-type Idx storage contract

#[test]
fn repr_attr_stored_via_named_idx() {
    // The live pipeline stores #repr attrs keyed by the Named Idx
    // from TypeEntry, not by a concrete struct_type Idx. This test pins that
    // the storage and retrieval contract works with Named Idx values, matching
    // the production codegen_pipeline path.
    let mut pool = ori_types::Pool::new();

    // Create a Named Idx (as the type registry and codegen pipeline would produce).
    let named_idx = pool.named(ori_ir::Name::from_raw(500));

    // Store #repr("c") under the Named Idx.
    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    // Verify retrieval works via the same Named Idx.
    assert_eq!(
        plan.repr_attr(named_idx),
        Some(&ReprAttribute::C),
        "Named Idx must store and retrieve #repr attrs — this is the production path"
    );

    // Also verify that a different Named Idx returns None.
    let other_named = pool.named(ori_ir::Name::from_raw(501));
    assert_eq!(
        plan.repr_attr(other_named),
        None,
        "repr_attr on unrelated Named Idx must return None"
    );
}

#[test]
fn repr_attr_named_vs_struct_idx_independent() {
    // semantic pin: Named Idx and struct_type Idx for the same name
    // are DIFFERENT pool entries. A #repr stored on one must NOT be visible on
    // the other. This verifies the storage contract uses exact Idx equality.
    let mut pool = ori_types::Pool::new();
    let name = ori_ir::Name::from_raw(600);

    let named_idx = pool.named(name);
    let struct_idx = pool.struct_type(name, &[]);

    // Store #repr("packed") on the Named Idx only.
    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::Packed)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(named_idx),
        Some(&ReprAttribute::Packed),
        "Named Idx should have the attr"
    );
    assert_eq!(
        plan.repr_attr(struct_idx),
        None,
        "struct_type Idx should NOT have the attr — different pool entry"
    );
}

// Canonical Representation Tests

/// Named→Struct resolution includes the target struct's field layout.
#[test]
fn canonical_named_resolves_to_struct() {
    let mut pool = Pool::new();
    let name_x = Name::new(0, 100);
    let name_y = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_x, Idx::INT), (name_y, Idx::FLOAT)]);

    let named_idx = pool.named(Name::new(0, 42));
    pool.set_resolution(named_idx, struct_idx);

    let repr = canonical(&pool, named_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(s.fields.len(), 2, "Named→Struct must resolve to 2 fields");
        assert_eq!(s.fields[0].name, name_x);
        assert_eq!(s.fields[1].name, name_y);
        assert!(s.trivial, "struct of (int, float) must be trivial");
        assert_eq!(s.size, 16);
    } else {
        panic!("expected Struct for Named→Struct, got {repr:?}");
    }
}

/// Struct containing an all-unit enum must be
/// trivial. This exercises `is_trivial_repr()` on the `MachineRepr::Enum`
/// path through a wrapper aggregate. A regression in the enum triviality
/// branch would make the struct non-trivial, failing this test.
#[test]
fn trivial_struct_containing_all_unit_enum() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 300);
    let enum_idx = pool.enum_type(
        enum_name,
        &[
            EnumVariant {
                name: Name::new(0, 301),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 302),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 303),
                field_types: vec![],
            },
        ],
    );
    // Wrap in struct to exercise nested triviality
    let name_e = Name::new(0, 100);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_e, enum_idx)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert!(s.trivial, "struct containing all-unit enum must be trivial");
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

/// `SelfType` returns None (should never reach codegen).
/// canonical must be fallible, not panic-driven.
#[test]
fn canonical_returns_none_for_self_type() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let self_idx = pool.intern(Tag::SelfType, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, self_idx, &mut cache).is_none(),
        "SelfType must return None, not panic"
    );
}

#[test]
fn canonical_fat_pointer_variants_cover_str_list_and_map() {
    let mut pool = Pool::new();

    let str_repr = canonical(&pool, Idx::STR);
    assert!(
        matches!(str_repr, MachineRepr::FatPointer(FatRepr::Str)),
        "str must be FatPointer(Str)"
    );

    let list_idx = pool.list(Idx::INT);
    let list_repr = canonical(&pool, list_idx);
    assert!(
        matches!(
            list_repr,
            MachineRepr::FatPointer(FatRepr::Collection { .. })
        ),
        "list must be FatPointer(Collection)"
    );

    let map_idx = pool.map(Idx::STR, Idx::INT);
    let map_repr = canonical(&pool, map_idx);
    assert!(
        matches!(map_repr, MachineRepr::FatPointer(FatRepr::Map { .. })),
        "map must be FatPointer(Map)"
    );
}

#[test]
fn canonical_container_representations() {
    let mut pool = Pool::new();

    let list_idx = pool.list(Idx::INT);
    assert!(
        matches!(
            canonical(&pool, list_idx),
            MachineRepr::FatPointer(FatRepr::Collection { .. })
        ),
        "List canonical"
    );
    let opt_idx = pool.option(Idx::INT);
    assert!(
        matches!(canonical(&pool, opt_idx), MachineRepr::Enum(_)),
        "Option canonical"
    );
    let set_idx = pool.set(Idx::STR);
    assert!(
        matches!(
            canonical(&pool, set_idx),
            MachineRepr::FatPointer(FatRepr::Collection { .. })
        ),
        "Set canonical"
    );
    let chan_idx = pool.channel(Idx::INT);
    assert_eq!(
        canonical(&pool, chan_idx),
        MachineRepr::OpaquePtr,
        "Channel canonical"
    );
    let range_idx = pool.range(Idx::INT);
    assert_eq!(
        canonical(&pool, range_idx),
        MachineRepr::Range,
        "Range canonical"
    );
    let iter_idx = pool.iterator(Idx::INT);
    assert_eq!(
        canonical(&pool, iter_idx),
        MachineRepr::UnmanagedPtr,
        "Iterator canonical"
    );
    let deiter_idx = pool.double_ended_iterator(Idx::INT);
    assert_eq!(
        canonical(&pool, deiter_idx),
        MachineRepr::UnmanagedPtr,
        "DoubleEndedIterator"
    );

    let map_idx = pool.map(Idx::STR, Idx::INT);
    assert!(
        matches!(
            canonical(&pool, map_idx),
            MachineRepr::FatPointer(FatRepr::Map { .. })
        ),
        "Map canonical"
    );
    let result_idx = pool.result(Idx::INT, Idx::STR);
    assert!(
        matches!(canonical(&pool, result_idx), MachineRepr::Enum(_)),
        "Result canonical"
    );
}

#[test]
fn canonical_complex_and_resolved_representations() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();

    let fn_idx = pool.function1(Idx::INT, Idx::BOOL);
    assert!(
        matches!(canonical(&pool, fn_idx), MachineRepr::Closure(_)),
        "Function canonical"
    );
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    assert!(
        matches!(canonical(&pool, tuple_idx), MachineRepr::Tuple(_)),
        "Tuple canonical"
    );
    let struct_name = Name::new(0, 500);
    let struct_idx = pool.struct_type(struct_name, &[(Name::new(0, 501), Idx::INT)]);
    assert!(
        matches!(canonical(&pool, struct_idx), MachineRepr::Struct(_)),
        "Struct canonical"
    );
    let enum_idx = pool.enum_type(
        Name::new(0, 600),
        &[
            EnumVariant {
                name: Name::new(0, 601),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 602),
                field_types: vec![Idx::INT],
            },
        ],
    );
    assert!(
        matches!(canonical(&pool, enum_idx), MachineRepr::Enum(_)),
        "Enum canonical"
    );

    let named_idx = pool.named(Name::new(0, 700));
    pool.set_resolution(named_idx, struct_idx);
    assert!(
        matches!(canonical(&pool, named_idx), MachineRepr::Struct(_)),
        "Named→Struct"
    );
    let applied_idx = pool.applied(Name::new(0, 800), &[Idx::INT]);
    pool.set_resolution(applied_idx, struct_idx);
    assert!(
        matches!(canonical(&pool, applied_idx), MachineRepr::Struct(_)),
        "Applied→Struct"
    );
    let alias_named = pool.named(Name::new(0, 900));
    pool.set_resolution(alias_named, Idx::INT);
    assert_eq!(
        canonical(&pool, alias_named),
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "Alias→Int canonical"
    );
}

#[test]
fn canonical_zst_aggregate_layouts() {
    let mut pool = Pool::new();

    let opt_unit = pool.option(Idx::UNIT);
    if let MachineRepr::Enum(ref e) = canonical(&pool, opt_unit) {
        assert_eq!(
            e.size, 8,
            "Option<()> = 8 bytes (i64 tag, not narrowed for runtime compat)"
        );
    } else {
        panic!("Option<()> must be Enum");
    }

    let tup_unit_bool = pool.pair(Idx::UNIT, Idx::BOOL);
    if let MachineRepr::Tuple(ref t) = canonical(&pool, tup_unit_bool) {
        assert_eq!(
            t.size, 1,
            "((), bool) = 1 byte — Unit zero-sized in aggregates"
        );
    } else {
        panic!("((), bool) must be Tuple");
    }

    let result_unit_int = pool.result(Idx::UNIT, Idx::INT);
    if let MachineRepr::Enum(ref e) = canonical(&pool, result_unit_int) {
        assert_eq!(e.size, 16, "Result<(), int> = 16 bytes");
    } else {
        panic!("Result<(), int> must be Enum");
    }

    let struct_unit_idx = pool.struct_type(
        Name::new(0, 1000),
        &[
            (Name::new(0, 1001), Idx::UNIT),
            (Name::new(0, 1002), Idx::INT),
        ],
    );
    if let MachineRepr::Struct(ref s) = canonical(&pool, struct_unit_idx) {
        assert_eq!(s.size, 8, "Struct(unit, int) = 8 bytes — Unit zero-sized");
    } else {
        panic!("Struct with Unit field must be Struct");
    }
}

/// `Borrowed` returns None — reserved type, not a codegen type.
#[test]
fn canonical_returns_none_for_borrowed() {
    use ori_types::{LifetimeId, Tag};

    let mut pool = Pool::new();
    let borrowed_idx = pool.borrowed(Idx::INT, LifetimeId::from_raw(1));
    assert_eq!(pool.tag(borrowed_idx), Tag::Borrowed);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, borrowed_idx, &mut cache).is_none(),
        "Borrowed must return None, not panic"
    );
}

/// `Projection` returns None — type-checker artifact.
#[test]
fn canonical_returns_none_for_projection() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let proj_idx = pool.intern(Tag::Projection, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, proj_idx, &mut cache).is_none(),
        "Projection must return None, not panic"
    );
}

/// `ModuleNs` returns None — module namespace, not a runtime type.
#[test]
fn canonical_returns_none_for_module_ns() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let ns_idx = pool.intern(Tag::ModuleNs, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, ns_idx, &mut cache).is_none(),
        "ModuleNs must return None, not panic"
    );
}

#[test]
fn canonical_repr_type_kind_matrix() {
    let mut pool = Pool::new();
    assert_primitive_canonical_reprs(&pool);
    assert_container_canonical_reprs(&mut pool);
    assert_complex_canonical_reprs(&mut pool);
    assert_non_codegen_reprs(&mut pool);
}

fn assert_primitive_canonical_reprs(pool: &Pool) {
    let cases = [
        (
            Idx::INT,
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true,
            },
            "Int canonical repr",
        ),
        (
            Idx::FLOAT,
            MachineRepr::Float {
                width: FloatWidth::F64,
            },
            "Float canonical repr",
        ),
        (Idx::BOOL, MachineRepr::Bool, "Bool canonical repr"),
        (
            Idx::STR,
            MachineRepr::FatPointer(FatRepr::Str),
            "Str canonical repr",
        ),
        (Idx::CHAR, MachineRepr::Char, "Char canonical repr"),
        (Idx::BYTE, MachineRepr::Byte, "Byte canonical repr"),
        (Idx::UNIT, MachineRepr::Unit, "Unit canonical repr"),
        (Idx::NEVER, MachineRepr::Never, "Never canonical repr"),
        (
            Idx::DURATION,
            MachineRepr::Duration,
            "Duration canonical repr",
        ),
        (Idx::SIZE, MachineRepr::Size, "Size canonical repr"),
        (
            Idx::ORDERING,
            MachineRepr::Ordering,
            "Ordering canonical repr",
        ),
    ];
    for (idx, expected, message) in cases {
        assert_eq!(canonical(pool, idx), expected, "{message}");
    }
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(pool, Idx::ERROR, &mut cache).is_none(),
        "Error has no canonical repr"
    );
}

fn assert_container_canonical_reprs(pool: &mut Pool) {
    use ori_types::LifetimeId;

    let list = pool.list(Idx::INT);
    assert!(matches!(
        canonical(pool, list),
        MachineRepr::FatPointer(FatRepr::Collection { .. })
    ));
    let option = pool.option(Idx::INT);
    assert!(matches!(canonical(pool, option), MachineRepr::Enum(_)));
    let set = pool.set(Idx::STR);
    assert!(matches!(
        canonical(pool, set),
        MachineRepr::FatPointer(FatRepr::Collection { .. })
    ));
    let channel = pool.channel(Idx::INT);
    assert_eq!(canonical(pool, channel), MachineRepr::OpaquePtr);
    let range = pool.range(Idx::INT);
    assert_eq!(canonical(pool, range), MachineRepr::Range);
    let iterator = pool.iterator(Idx::INT);
    assert_eq!(canonical(pool, iterator), MachineRepr::UnmanagedPtr);
    let double_ended = pool.double_ended_iterator(Idx::INT);
    assert_eq!(canonical(pool, double_ended), MachineRepr::UnmanagedPtr);
    let map = pool.map(Idx::STR, Idx::INT);
    assert!(matches!(
        canonical(pool, map),
        MachineRepr::FatPointer(FatRepr::Map { .. })
    ));
    let result = pool.result(Idx::INT, Idx::STR);
    assert!(matches!(canonical(pool, result), MachineRepr::Enum(_)));
    let borrowed = pool.borrowed(Idx::INT, LifetimeId::from_raw(1));
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(canonical_cached(pool, borrowed, &mut cache).is_none());
}

fn assert_complex_canonical_reprs(pool: &mut Pool) {
    use ori_types::EnumVariant;

    let function = pool.function1(Idx::INT, Idx::BOOL);
    assert!(matches!(canonical(pool, function), MachineRepr::Closure(_)));
    let tuple = pool.pair(Idx::INT, Idx::BOOL);
    assert!(matches!(canonical(pool, tuple), MachineRepr::Tuple(_)));
    let struct_name = Name::new(0, 500);
    let struct_idx = pool.struct_type(struct_name, &[(Name::new(0, 501), Idx::INT)]);
    assert!(matches!(
        canonical(pool, struct_idx),
        MachineRepr::Struct(_)
    ));
    let enum_idx = pool.enum_type(
        Name::new(0, 600),
        &[
            EnumVariant {
                name: Name::new(0, 601),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 602),
                field_types: vec![Idx::INT],
            },
        ],
    );
    assert!(matches!(canonical(pool, enum_idx), MachineRepr::Enum(_)));

    let named = pool.named(Name::new(0, 700));
    pool.set_resolution(named, struct_idx);
    assert!(matches!(canonical(pool, named), MachineRepr::Struct(_)));
    let applied = pool.applied(Name::new(0, 800), &[Idx::INT]);
    pool.set_resolution(applied, Idx::INT);
    assert_eq!(
        canonical(pool, applied),
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }
    );
    let alias = pool.named(Name::new(0, 900));
    pool.set_resolution(alias, Idx::FLOAT);
    assert_eq!(
        canonical(pool, alias),
        MachineRepr::Float {
            width: FloatWidth::F64,
        }
    );
}

fn assert_non_codegen_reprs(pool: &mut Pool) {
    use ori_types::Tag;

    let mut indices = vec![pool.fresh_var()];
    indices.push(pool.intern(Tag::BoundVar, 0));
    indices.push(pool.rigid_var(Name::new(0, 999)));
    indices.push(pool.scheme(&[0], Idx::INT));
    indices.push(pool.intern(Tag::Projection, 0));
    indices.push(pool.intern(Tag::ModuleNs, 0));
    indices.push(pool.intern(Tag::Infer, 0));
    indices.push(pool.intern(Tag::SelfType, 0));

    let mut cache = rustc_hash::FxHashMap::default();
    for idx in indices {
        assert!(canonical_cached(pool, idx, &mut cache).is_none());
    }
}

// analyze_triviality() validation pass produces zero mismatches

#[test]
fn analyze_triviality_validation_zero_mismatches() {
    use ori_types::{EnumVariant, Idx, Pool};

    let mut pool = Pool::new();

    // Build diverse types: trivial + non-trivial + compound
    let opt_int = pool.option(Idx::INT);
    let tuple_trivial = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let sn = Name::from_raw(8000);
    let f1 = Name::from_raw(8001);
    let f2 = Name::from_raw(8002);
    let struct_trivial = pool.struct_type(sn, &[(f1, Idx::INT), (f2, Idx::FLOAT)]);
    let result_nontrivial = pool.result(Idx::INT, Idx::STR);
    let enum_trivial = pool.enum_type(
        Name::from_raw(8010),
        &[
            EnumVariant {
                name: Name::from_raw(8011),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::from_raw(8012),
                field_types: vec![Idx::INT],
            },
        ],
    );

    // Iterator handles are box-allocated without an RC header but remain
    // non-trivial because scope exit requires `ori_iter_drop`.
    let iter_int = pool.iterator(Idx::INT);
    let deiter_int = pool.double_ended_iterator(Idx::INT);

    // compute_repr_plan canonicalizes all reachable types and runs
    // analyze_triviality() internally. The debug_assert! in the pass
    // fires on any mismatch between classify_triviality() and is_trivial_repr().
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);

    // Verify the plan's is_trivial() queries match expectations.
    assert!(plan.is_trivial(opt_int), "Option<int> should be trivial");
    assert!(
        plan.is_trivial(tuple_trivial),
        "(int, float) should be trivial"
    );
    assert!(
        plan.is_trivial(struct_trivial),
        "struct {{int, float}} should be trivial"
    );
    assert!(
        !plan.is_trivial(result_nontrivial),
        "Result<int, str> should be non-trivial"
    );
    // Both triviality classifiers must preserve iterator drop obligations.
    assert!(
        !plan.is_trivial(iter_int),
        "Iterator<int> is non-trivial — needs ori_iter_drop at scope exit"
    );
    assert!(
        !plan.is_trivial(deiter_int),
        "DoubleEndedIterator<int> is non-trivial — needs ori_iter_drop at scope exit"
    );
    assert!(
        plan.is_trivial(enum_trivial),
        "enum {{unit, int}} should be trivial"
    );
}

// Idx::ERROR must be trivial in ReprPlan (parity with classify_triviality)

#[test]
fn repr_plan_error_type_is_trivial() {
    // semantic pin: ReprPlan::is_trivial(Idx::ERROR) must return true,
    // matching classify_triviality(Idx::ERROR) which returns Triviality::Trivial.
    // ERROR is a sentinel type that should never trigger RC operations.
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert!(
        plan.is_trivial(Idx::ERROR),
        "Idx::ERROR must be trivial — matches classify_triviality() and ArcClassifier"
    );
}

#[test]
fn repr_plan_error_type_has_canonical_repr() {
    // ERROR must have a canonical repr so is_trivial() doesn't
    // fall through to the None->false default.
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert!(
        plan.get_repr(Idx::ERROR).is_some(),
        "Idx::ERROR must have a canonical representation"
    );
}

#[test]
fn repr_plan_error_triviality_matches_classify_triviality() {
    // parity test: ReprPlan and classify_triviality() must agree
    // for the ERROR sentinel.
    use ori_types::triviality::{classify_triviality, Triviality};

    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);

    let plan_trivial = plan.is_trivial(Idx::ERROR);
    let classify_trivial = classify_triviality(Idx::ERROR, &pool) == Triviality::Trivial;
    assert_eq!(
        plan_trivial, classify_trivial,
        "ReprPlan::is_trivial(ERROR) = {plan_trivial}, classify_triviality(ERROR) = {classify_trivial} — must agree"
    );
}

// Named->resolved idx metadata propagation
//
// When a Named type has a resolution chain to a concrete struct/tuple,
// repr_attrs and pub_type_indices must propagate to the resolved idx.
// Without this, narrowing and codegen bypass the exemption.

#[test]
fn repr_attr_propagates_to_resolved_struct_idx() {
    // A Named type with #repr("c") that resolves to a concrete
    // struct idx must have the attr visible on BOTH the named AND resolved idx.
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 700);
    let field_x = Name::new(0, 701);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    // Named idx should always have the attr.
    assert_eq!(
        plan.repr_attr(named_idx),
        Some(&ReprAttribute::C),
        "Named idx must retain its #repr(\"c\") attr"
    );

    // Resolved struct idx must ALSO have the attr — this is the production
    // codegen path (TypeLayoutResolver resolves through pool.resolve_fully()).
    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::C),
        "resolved struct idx must inherit #repr(\"c\") from named idx \
         via resolution chain — codegen uses the resolved idx"
    );
}

#[test]
fn repr_packed_propagates_to_resolved_struct_idx() {
    // #repr("packed") must also propagate through resolution.
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 710);
    let field_x = Name::new(0, 711);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::Packed)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::Packed),
        "resolved struct idx must inherit #repr(\"packed\")"
    );
}

#[test]
fn repr_c_aligned_propagates_to_resolved_struct_idx() {
    // #repr("c", aligned N) must also propagate.
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 720);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[]);
    pool.set_resolution(named_idx, struct_idx);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::CAligned(16))];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::CAligned(16)),
        "resolved struct idx must inherit #repr(\"c\", aligned 16)"
    );
}

#[test]
fn repr_transparent_propagates_to_resolved_struct_idx() {
    // #repr("transparent") must also propagate.
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 730);
    let field_x = Name::new(0, 731);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::Transparent)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::Transparent),
        "resolved struct idx must inherit #repr(\"transparent\")"
    );
}

#[test]
fn pub_type_propagates_to_resolved_struct_idx() {
    // A Named type marked `pub` that resolves to a concrete struct
    // must have the pub flag on the resolved idx too.
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 740);
    let field_x = Name::new(0, 741);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &[],
        &[],
        &[],
        false,
    );

    assert!(
        plan.is_public_type(named_idx),
        "Named idx must remain public"
    );
    assert!(
        plan.is_public_type(struct_idx),
        "resolved struct idx must inherit pub status from named idx"
    );
}

#[test]
fn repr_attr_no_resolution_no_propagation() {
    // Equal names without `set_resolution()` remain independent identities.
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 750);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[]);
    // No set_resolution! These are independent pool entries.

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(named_idx),
        Some(&ReprAttribute::C),
        "Named idx should have the attr"
    );
    assert_eq!(
        plan.repr_attr(struct_idx),
        None,
        "Without resolution chain, struct_type idx must NOT inherit the attr"
    );
}

#[test]
fn repr_c_resolved_idx_not_narrowed_semantic_pin() {
    // SEMANTIC PIN: A Named type with #repr("c") resolving to a
    // concrete struct — after narrowing, the resolved struct idx must NOT be
    // narrowed. This test ONLY passes with metadata propagation.
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 760);
    let field_x = Name::new(0, 761);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let mut plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    // Manually add field ranges to both indices (simulating range analysis output).
    plan.join_field_range(named_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });
    plan.join_field_range(struct_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    // Run narrowing again with the field ranges present.
    narrow_struct_fields(&mut plan, &pool);

    // The resolved struct idx must NOT be narrowed — it has #repr("c").
    match plan.get_repr(struct_idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(
                s.fields[0].repr,
                MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                },
                "semantic pin: #repr(\"c\") resolved struct idx must NOT be narrowed"
            );
        }
        other => panic!("expected Struct repr for resolved idx, got {other:?}"),
    }
}

#[test]
fn pub_resolved_idx_not_narrowed_semantic_pin() {
    // SEMANTIC PIN: A Named type marked `pub` resolving to a
    // concrete struct — after narrowing, the resolved struct idx must NOT be
    // narrowed. This test ONLY passes with pub_type propagation.
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 770);
    let field_x = Name::new(0, 771);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let mut plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &[],
        &[],
        &[],
        false,
    );

    // Manually add field ranges.
    plan.join_field_range(named_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });
    plan.join_field_range(struct_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    match plan.get_repr(struct_idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(
                s.fields[0].repr,
                MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                },
                "semantic pin: pub resolved struct idx must NOT be narrowed"
            );
        }
        other => panic!("expected Struct repr for resolved idx, got {other:?}"),
    }
}

// Applied → concrete Struct resolutions must inherit repr/pub
// exemptions from the parent Named type.
//
// IMPORTANT: Pool deduplicates struct types with identical (name, fields).
// Tests use DIFFERENT field types for base vs monomorphized structs to ensure
// distinct pool indices (the same shape produced when type parameters
// are substituted with concrete types).

#[test]
fn repr_attr_propagates_through_applied_to_concrete_struct() {
    // A Named type with #repr("c") whose Applied instantiation
    // resolves to a monomorphized concrete struct — the concrete struct idx
    // must also have the repr attr.
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 900);
    let field_a = Name::new(0, 901);
    let field_b = Name::new(0, 902);

    // Named type declaration: `type Pair<A, B> = { a: A, b: B }`
    let named_idx = pool.named(type_name);
    // Base struct (generic body — uses FLOAT to distinguish from mono)
    let base_struct_idx =
        pool.struct_type(type_name, &[(field_a, Idx::FLOAT), (field_b, Idx::FLOAT)]);
    pool.set_resolution(named_idx, base_struct_idx);

    // Monomorphized: Applied(Pair, [Int, Str]) → concrete Struct with INT/STR fields
    let applied_idx = pool.applied(type_name, &[Idx::INT, Idx::STR]);
    let mono_struct_idx = pool.struct_type(type_name, &[(field_a, Idx::INT), (field_b, Idx::STR)]);
    pool.set_resolution(applied_idx, mono_struct_idx);

    // Ensure distinct indices (Pool may dedup identical content).
    assert_ne!(
        base_struct_idx, mono_struct_idx,
        "test setup: base and mono structs must be distinct pool entries"
    );

    // #repr("c") on the Named idx (as the parser would emit)
    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    // Named and base struct should have the attr (path)
    assert_eq!(plan.repr_attr(named_idx), Some(&ReprAttribute::C));
    assert_eq!(plan.repr_attr(base_struct_idx), Some(&ReprAttribute::C));

    // The monomorphized concrete struct MUST also have it.
    assert_eq!(
        plan.repr_attr(mono_struct_idx),
        Some(&ReprAttribute::C),
        "monomorphized concrete struct must inherit #repr(\"c\") from Named parent"
    );
}

#[test]
fn pub_type_propagates_through_applied_to_concrete_struct() {
    // A Named type marked `pub` whose Applied instantiation
    // resolves to a monomorphized concrete struct — the concrete struct idx
    // must also be marked public.
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 910);
    let field_x = Name::new(0, 911);

    let named_idx = pool.named(type_name);
    // Base struct with FLOAT field (distinct from INT mono)
    let base_struct_idx = pool.struct_type(type_name, &[(field_x, Idx::FLOAT)]);
    pool.set_resolution(named_idx, base_struct_idx);

    // Monomorphized instantiation with INT field
    let applied_idx = pool.applied(type_name, &[Idx::INT]);
    let mono_struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(applied_idx, mono_struct_idx);

    assert_ne!(base_struct_idx, mono_struct_idx);

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &[],
        &[],
        &[],
        false,
    );

    assert!(plan.is_public_type(named_idx));
    assert!(plan.is_public_type(base_struct_idx));

    // monomorphized concrete struct must also be public.
    assert!(
        plan.is_public_type(mono_struct_idx),
        "monomorphized concrete struct must inherit pub from Named parent"
    );
}

#[test]
fn repr_c_applied_concrete_struct_not_narrowed_semantic_pin() {
    // SEMANTIC PIN: A #repr("c") Named type with a monomorphized
    // Applied → concrete Struct — narrowing must be blocked on the monomorphized
    // struct. This test ONLY passes with Applied-path propagation.
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 920);
    let field_x = Name::new(0, 921);

    let named_idx = pool.named(type_name);
    let base_struct_idx = pool.struct_type(type_name, &[(field_x, Idx::FLOAT)]);
    pool.set_resolution(named_idx, base_struct_idx);

    // Mono struct with INT field — the one that would be narrowed
    let applied_idx = pool.applied(type_name, &[Idx::INT]);
    let mono_struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(applied_idx, mono_struct_idx);

    assert_ne!(base_struct_idx, mono_struct_idx);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let mut plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    // Simulate bounded field ranges from range analysis.
    plan.join_field_range(mono_struct_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    // Monomorphized struct must NOT be narrowed — parent has #repr("c").
    match plan.get_repr(mono_struct_idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(
                s.fields[0].repr,
                MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                },
                "semantic pin: #repr(\"c\") monomorphized struct must NOT be narrowed"
            );
        }
        other => panic!("expected Struct repr for mono struct idx, got {other:?}"),
    }
}

#[test]
fn pub_applied_concrete_struct_not_narrowed_semantic_pin() {
    // SEMANTIC PIN: A `pub` Named type with a monomorphized
    // Applied → concrete Struct — narrowing must be blocked on the monomorphized
    // struct. This test ONLY passes with Applied-path pub propagation.
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 930);
    let field_x = Name::new(0, 931);

    let named_idx = pool.named(type_name);
    let base_struct_idx = pool.struct_type(type_name, &[(field_x, Idx::FLOAT)]);
    pool.set_resolution(named_idx, base_struct_idx);

    let applied_idx = pool.applied(type_name, &[Idx::INT]);
    let mono_struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(applied_idx, mono_struct_idx);

    assert_ne!(base_struct_idx, mono_struct_idx);

    let mut plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &[],
        &[],
        &[],
        false,
    );

    plan.join_field_range(mono_struct_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    match plan.get_repr(mono_struct_idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(
                s.fields[0].repr,
                MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                },
                "semantic pin: pub monomorphized struct must NOT be narrowed"
            );
        }
        other => panic!("expected Struct repr for mono struct idx, got {other:?}"),
    }
}

#[test]
fn applied_without_resolution_no_propagation() {
    // negative test: An Applied idx without set_resolution() should
    // NOT propagate repr attrs to an unrelated struct that happens to share a
    // name but has distinct field types (and thus a distinct pool Idx).
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 940);
    let field_x = Name::new(0, 941);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    // Applied type with NO resolution link
    let _applied_idx = pool.applied(type_name, &[Idx::STR]);
    // Unrelated struct with DIFFERENT fields (STR, not INT) — distinct pool Idx
    let unrelated_struct = pool.struct_type(type_name, &[(field_x, Idx::STR)]);
    // No set_resolution! These are independent pool entries.

    assert_ne!(
        struct_idx, unrelated_struct,
        "test setup: must be distinct indices"
    );

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    // Named and its resolved struct have the attr.
    assert_eq!(plan.repr_attr(named_idx), Some(&ReprAttribute::C));
    assert_eq!(plan.repr_attr(struct_idx), Some(&ReprAttribute::C));

    // Unlinked struct must NOT have the attr — no resolution chain connects it.
    assert_eq!(
        plan.repr_attr(unrelated_struct),
        None,
        "Struct without resolution chain must NOT inherit attr"
    );
}

#[test]
fn multiple_applied_instantiations_all_protected() {
    // Multiple monomorphized instantiations of the same pub #repr("c")
    // type must ALL be protected. Tests Pair<int, int> and Pair<int, str>.
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 950);
    let field_a = Name::new(0, 951);
    let field_b = Name::new(0, 952);

    let named_idx = pool.named(type_name);
    // Base struct with FLOAT fields (generic body, distinct from monos)
    let base_struct = pool.struct_type(type_name, &[(field_a, Idx::FLOAT), (field_b, Idx::FLOAT)]);
    pool.set_resolution(named_idx, base_struct);

    let applied_1 = pool.applied(type_name, &[Idx::INT, Idx::INT]);
    let mono_1 = pool.struct_type(type_name, &[(field_a, Idx::INT), (field_b, Idx::INT)]);
    pool.set_resolution(applied_1, mono_1);

    let applied_2 = pool.applied(type_name, &[Idx::INT, Idx::STR]);
    let mono_2 = pool.struct_type(type_name, &[(field_a, Idx::INT), (field_b, Idx::STR)]);
    pool.set_resolution(applied_2, mono_2);

    assert_ne!(base_struct, mono_1);
    assert_ne!(base_struct, mono_2);
    assert_ne!(mono_1, mono_2);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &repr_attrs,
        None,
        &[named_idx],
        &[],
        &[],
        &[],
        false,
    );

    // Both monomorphized structs must have repr and pub
    assert_eq!(plan.repr_attr(mono_1), Some(&ReprAttribute::C));
    assert_eq!(plan.repr_attr(mono_2), Some(&ReprAttribute::C));
    assert!(plan.is_public_type(mono_1));
    assert!(plan.is_public_type(mono_2));
}

// Imported type metadata tests

#[test]
fn imported_pub_type_seeded_via_metadata() {
    // A type present in the pool (as if imported via type descriptors)
    // with ExportedTypeMetadata marking it `is_public: true` must be protected
    // from narrowing, even though it is NOT in the local module's pub_type_indices.
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 900);
    let field_x = Name::new(0, 901);

    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    let struct_hash = pool.hash(struct_idx);

    // Simulate imported metadata — the type is public in its originating module.
    let imported_meta = vec![ExportedTypeMetadata {
        merkle_hash: struct_hash,
        repr: None,
        is_public: true,
    }];

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        // The fixture declares no local representation attributes.
        &[],
        None,
        // The fixture declares no local public types.
        &[],
        &imported_meta,
        &[],
        &[],
        false,
    );

    assert!(
        plan.is_public_type(struct_idx),
        "imported pub type must be seeded as public via metadata"
    );
}

#[test]
fn imported_repr_c_type_seeded_via_metadata() {
    // A type present in the pool with ExportedTypeMetadata
    // carrying `repr: Some(ReprAttrKind::C)` must have its repr attr seeded
    // in the plan, even without local repr_attrs.
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 910);
    let field_x = Name::new(0, 911);

    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    let struct_hash = pool.hash(struct_idx);

    let imported_meta = vec![ExportedTypeMetadata {
        merkle_hash: struct_hash,
        repr: Some(ori_ir::ReprAttrKind::C),
        is_public: false,
    }];

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        // The fixture declares no local representation attributes.
        &[],
        None,
        // The fixture declares no local public types.
        &[],
        &imported_meta,
        &[],
        &[],
        false,
    );

    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::C),
        "imported #repr(\"c\") type must have repr attr seeded via metadata"
    );
}

#[test]
fn imported_pub_type_not_narrowed_semantic_pin() {
    // Semantic pin: An imported pub struct with bounded fields
    // must NOT be narrowed. This test ONLY passes with imported metadata seeding.
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 920);
    let field_x = Name::new(0, 921);

    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    let struct_hash = pool.hash(struct_idx);

    let imported_meta = vec![ExportedTypeMetadata {
        merkle_hash: struct_hash,
        repr: None,
        is_public: true,
    }];

    let mut plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        // Only imported metadata contributes public types.
        &[],
        &imported_meta,
        &[],
        &[],
        false,
    );

    // Add field range that would normally trigger narrowing.
    plan.join_field_range(struct_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    // Run narrowing.
    narrow_struct_fields(&mut plan, &pool);

    // The struct must NOT be narrowed — it's public via imported metadata.
    match plan.get_repr(struct_idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(
                s.fields[0].repr,
                MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                },
                "semantic pin: imported pub struct must NOT be narrowed"
            );
        }
        other => panic!("expected Struct repr, got {other:?}"),
    }
}

#[test]
fn imported_repr_c_type_not_narrowed_semantic_pin() {
    // Semantic pin: An imported #repr("c") struct with bounded
    // fields must NOT be narrowed. This test ONLY passes with imported metadata.
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 930);
    let field_x = Name::new(0, 931);

    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    let struct_hash = pool.hash(struct_idx);

    let imported_meta = vec![ExportedTypeMetadata {
        merkle_hash: struct_hash,
        repr: Some(ori_ir::ReprAttrKind::C),
        is_public: false,
    }];

    let mut plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &imported_meta,
        &[],
        &[],
        false,
    );

    // Add field range that would normally trigger narrowing.
    plan.join_field_range(struct_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    // Run narrowing.
    narrow_struct_fields(&mut plan, &pool);

    // The struct must NOT be narrowed — it has #repr("c") via imported metadata.
    match plan.get_repr(struct_idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(
                s.fields[0].repr,
                MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                },
                "semantic pin: imported #repr(\"c\") struct must NOT be narrowed"
            );
        }
        other => panic!("expected Struct repr, got {other:?}"),
    }
}

#[test]
fn no_imported_metadata_allows_narrowing() {
    // Negative test: Without imported metadata, a struct with
    // bounded fields is narrowed. This preserves the semantic pins for
    // testing the right thing — they would fail without the metadata.
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 940);
    let field_x = Name::new(0, 941);

    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);

    let mut plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        // The fixture declares no public types.
        &[],
        // The fixture supplies no imported metadata.
        &[],
        &[],
        &[],
        false,
    );

    // Add field range that triggers narrowing.
    plan.join_field_range(struct_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    // Run narrowing.
    narrow_struct_fields(&mut plan, &pool);

    // Without any protection, the struct IS narrowed to i8.
    match plan.get_repr(struct_idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(
                s.fields[0].repr,
                MachineRepr::Int {
                    width: IntWidth::I8,
                    signed: true,
                },
                "Negative test: unprotected struct must be narrowed to i8"
            );
        }
        other => panic!("expected Struct repr, got {other:?}"),
    }
}

#[test]
fn direct_test_body_constructs_join_field_ranges_before_narrowing() {
    use ori_arc::ir::{
        ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue, ValueRepr,
    };
    use ori_arc::{ArcBlockId, ArcVarId};

    fn test_body(name: Name, struct_name: Name, struct_idx: Idx, x: i64, y: i64) -> ArcFunction {
        let x_var = ArcVarId::new(0);
        let y_var = ArcVarId::new(1);
        let struct_var = ArcVarId::new(2);
        ArcFunction {
            name,
            return_type: struct_idx,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Let {
                        dst: x_var,
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(x)),
                    },
                    ArcInstr::Let {
                        dst: y_var,
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(y)),
                    },
                    ArcInstr::Construct {
                        dst: struct_var,
                        ty: struct_idx,
                        ctor: CtorKind::Struct(struct_name),
                        args: vec![x_var, y_var],
                    },
                ],
                terminator: ArcTerminator::Return { value: struct_var },
            }],
            entry: ArcBlockId::new(0),
            var_types: vec![Idx::INT, Idx::INT, struct_idx],
            var_reprs: vec![ValueRepr::Scalar, ValueRepr::Scalar, ValueRepr::Scalar],
            spans: vec![vec![None; 3]],
            ..ArcFunction::default()
        }
    }

    let mut pool = Pool::new();
    let struct_name = Name::new(0, 960);
    let field_x = Name::new(0, 961);
    let field_y = Name::new(0, 962);
    let struct_idx = pool.struct_type(struct_name, &[(field_x, Idx::INT), (field_y, Idx::INT)]);
    let bodies = [
        test_body(Name::new(0, 963), struct_name, struct_idx, 1, 2),
        test_body(Name::new(0, 964), struct_name, struct_idx, 100, 20),
        test_body(Name::new(0, 965), struct_name, struct_idx, 999, 200),
    ];

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &bodies,
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &[],
        &[],
        &[],
        false,
    );

    let Some(MachineRepr::Struct(repr)) = plan.get_repr(struct_idx) else {
        panic!("direct test-body struct must have a representation");
    };
    for field_name in [field_x, field_y] {
        let field = repr
            .fields
            .iter()
            .find(|field| field.name == field_name)
            .unwrap_or_else(|| panic!("direct test-body struct field must be retained"));
        assert_eq!(
            field.repr,
            MachineRepr::Int {
                width: IntWidth::I16,
                signed: true,
            },
            "all direct test-body construction sites must contribute before narrowing"
        );
    }
}

#[test]
fn imported_metadata_hash_not_in_pool_ignored() {
    // Edge case: Imported metadata with a hash that doesn't
    // exist in the local pool is silently ignored (no panic, no effect).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();

    // A real local struct so the plan has a queryable decision.
    let type_name = interner.intern("Local");
    let field_r = interner.intern("r");
    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_r, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let imported_meta = vec![ExportedTypeMetadata {
        merkle_hash: 0xDEAD_BEEF_CAFE_1234,
        repr: Some(ori_ir::ReprAttrKind::C),
        is_public: true,
    }];

    let plan_with_bogus_meta = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &imported_meta,
        &[],
        &[],
        false,
    );
    let plan_without_meta = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &[],
        &[],
        &[],
        false,
    );

    // The unknown imported hash has NO effect: the local struct's
    // representation decision is identical with and without it.
    assert_eq!(
        plan_with_bogus_meta.get_repr(struct_idx),
        plan_without_meta.get_repr(struct_idx),
        "Imported metadata with a hash absent from the local pool must not change layout"
    );
}

// Cross-module collection surface protection

/// Imported collection surface hash does NOT suppress element narrowing.
/// Imported surfaces are for transitive forwarding metadata
/// (A→B→C), not for narrowing suppression. Private `[int]` in the importing
/// module can narrow independently of imported public `[int]` APIs.
#[test]
fn imported_collection_surface_does_not_suppress_narrowing() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();

    // Build a narrowable struct so the plan has something to process.
    let type_name = interner.intern("Wrapper");
    let field_xs = interner.intern("xs");
    let list_int = pool.list(Idx::INT);
    let _struct_idx = pool.struct_type(type_name, &[(field_xs, list_int)]);

    // Get the merkle hash of List<int> — simulates what the exporting module
    // would have computed via generate_exported_collection_surfaces().
    let list_int_hash = pool.hash(list_int);

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        // The fixture declares no local public types.
        &[],
        // The fixture supplies no imported type metadata.
        &[],
        // The imported collection surface is the only public input.
        &[list_int_hash],
        &[],
        false,
    );

    // Imported surfaces should NOT mark the type as public.
    // They're for transitive forwarding, not narrowing suppression.
    assert!(
        !plan.is_public_type(list_int),
        "Imported collection surface should NOT suppress narrowing"
    );
}

/// Imported collection surface hash that doesn't match any local pool type
/// is safely ignored (no panic).
#[test]
fn imported_collection_surface_unknown_hash_no_panic() {
    let mut pool = Pool::new();
    let bogus_hash = 0xDEAD_BEEF_CAFE_BABE;

    // A real collection type so the plan has a queryable decision.
    let list_int = pool.list(Idx::INT);

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &[],
        &[bogus_hash],
        &[],
        false,
    );

    // The unknown collection-surface hash is silently skipped — it marks
    // no local type public (an unmatched hash has no resolution target).
    assert!(
        !plan.is_public_type(list_int),
        "Unknown collection-surface hash must not mark any local type public"
    );
}

/// Empty imported collection surfaces behave identically to no surfaces.
#[test]
fn imported_collection_surface_empty_is_noop() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let type_name = interner.intern("Pixel");
    let field_r = interner.intern("r");
    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_r, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let plan_without = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &[],
        // The fixture supplies no collection surfaces.
        &[],
        &[],
        false,
    );

    let plan_with_empty = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &[],
        // The collection-surface input is explicitly empty.
        &[],
        &[],
        false,
    );

    // Both plans should treat the struct identically.
    assert_eq!(
        plan_without.get_repr(struct_idx),
        plan_with_empty.get_repr(struct_idx),
        "Empty collection surfaces should not change narrowing behavior"
    );
}

/// Multiple imported collection surface hashes are resolved without panic.
/// After imported surfaces don't mark types as public (they're
/// for forwarding metadata only), but the resolution still succeeds.
#[test]
fn imported_collection_surfaces_multiple_hashes_no_panic() {
    let mut pool = Pool::new();

    let list_int = pool.list(Idx::INT);
    let set_int = pool.set(Idx::INT);

    let list_hash = pool.hash(list_int);
    let set_hash = pool.hash(set_int);

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &[],
        &[list_hash, set_hash],
        &[],
        false,
    );

    assert!(
        !plan.is_public_type(list_int),
        "Imported surface should NOT mark List<int> as public"
    );
    assert!(
        !plan.is_public_type(set_int),
        "Imported surface should NOT mark Set<int> as public"
    );
}

/// Imported collection surfaces do NOT suppress element narrowing for the
/// importing module's private `[int]` usage: imported surfaces are not added
/// to `pub_type_indices`, so only same-module public functions suppress
/// narrowing.
#[test]
fn imported_collection_surface_allows_private_narrowing() {
    let mut pool = Pool::new();

    // One `[int]` Idx — shared by both imported public API and private local usage.
    let list_int = pool.list(Idx::INT);
    let list_hash = pool.hash(list_int);

    // Plan WITH imported surface: simulates a module importing a public [int] API.
    let plan_with_import = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        // The fixture declares no local public types.
        &[],
        &[],
        // The imported `[int]` surface is the only public input.
        &[list_hash],
        &[],
        false,
    );

    // Plan WITHOUT imported surface: private-only usage.
    let plan_without_import = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &[],
        // The fixture supplies no imported surfaces.
        &[],
        &[],
        false,
    );

    // With import: the shared Idx is NOT marked public. Imported surfaces
    // don't suppress narrowing — they're for forwarding metadata only.
    assert!(
        !plan_with_import.is_public_type(list_int),
        "Imported surface should NOT mark [int] as public"
    );

    // Without import: same — private [int] is not public.
    assert!(
        !plan_without_import.is_public_type(list_int),
        "Without imports, private [int] is not marked public"
    );

    // Imported surfaces do not affect narrowing of a private `[int]`.
}

/// Same-module public functions suppress narrowing independently of imports.
#[test]
fn local_public_function_still_suppresses_narrowing() {
    let mut pool = Pool::new();

    let list_int = pool.list(Idx::INT);

    // Plan with local pub type: simulates `pub @f(xs: [int])` in this module.
    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        // The local public signature exposes `[int]`.
        &[list_int],
        &[],
        // The fixture supplies no imported surfaces.
        &[],
        &[],
        false,
    );

    // Local public function's collection type MUST suppress narrowing.
    assert!(
        plan.is_public_type(list_int),
        "Local public function should suppress [int] narrowing"
    );
}

// Single-variant enum (tagless)

/// Test that canonical mapping for a single-variant enum produces `EnumTag::None`.
#[test]
fn canonical_single_variant_enum_is_tagless() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 400);
    let variant_name = Name::new(0, 401);
    let enum_idx = pool.enum_type(
        enum_name,
        &[EnumVariant {
            name: variant_name,
            field_types: vec![Idx::INT],
        }],
    );
    let repr = canonical(&pool, enum_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 1);
        assert_eq!(
            e.tag,
            EnumTag::None,
            "single-variant enum should be tagless"
        );
        assert!(e.tag.is_tagless());
        assert!(!e.tag.needs_tag_field());
        assert_eq!(e.tag.payload_gep_index(), 0);
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

/// Test that a single-variant unit enum produces `EnumTag::None` with minimal size.
#[test]
fn canonical_single_variant_unit_enum_is_tagless() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 410);
    let variant_name = Name::new(0, 411);
    let enum_idx = pool.enum_type(
        enum_name,
        &[EnumVariant {
            name: variant_name,
            field_types: vec![],
        }],
    );
    let repr = canonical(&pool, enum_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.tag, EnumTag::None);
        // Unit newtype: size 1, align 1
        assert_eq!(e.size, 1);
        assert_eq!(e.align, 1);
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

// ReprPlan::enum_repr_with_fallback (SSOT ladder)

/// A plan-miss on an enum-shaped type recomputes the canonical repr
/// instead of answering `None` — pins the single ladder every emission
/// surface (ABI sizing, type-info layout, ARC emission) consults, so a
/// variable-residue Option cannot be niche-encoded by one surface and
/// tag-sized by another.
#[test]
fn enum_repr_with_fallback_plan_miss_recomputes_canonical_option() {
    use ori_types::Pool;

    let mut pool = Pool::new();
    let opt_str = pool.option(ori_types::Idx::STR);

    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(plan.enum_repr(opt_str).is_none());

    // The ladder falls back to the canonical computation.
    let via_ladder = plan
        .enum_repr_with_fallback(&pool, opt_str)
        .unwrap_or_else(|| panic!("fallback must recompute canonical for Option<str>"));
    let canonical = crate::canonical_enum_for_type(&pool, opt_str)
        .unwrap_or_else(|| panic!("canonical_enum_for_type must cover Option<str>"));
    assert_eq!(*via_ladder, canonical);
}

/// Non-enum-shaped types stay `None` through the ladder (no spurious
/// canonical recomputation for scalars).
#[test]
fn enum_repr_with_fallback_non_enum_type_returns_none() {
    use ori_types::Pool;

    let pool = Pool::new();
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(plan
        .enum_repr_with_fallback(&pool, ori_types::Idx::INT)
        .is_none());
}
