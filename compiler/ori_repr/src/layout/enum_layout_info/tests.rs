//! Tests for `EnumLayoutInfo` and `compute_enum_layout_info`.

use ori_ir::Name;

use crate::enum_repr::{EnumRepr, EnumTag, VariantRepr};
use crate::layout::enum_layout_info::compute_enum_layout_info;
use crate::repr::{IntWidth, MachineRepr};

// Helper: a variant with the given field reprs. `size` is the slot-packed
// payload size, computed the same way the real canonical pass does.
fn variant(fields: Vec<MachineRepr>) -> VariantRepr {
    let size: u32 = fields
        .iter()
        .map(|f| crate::layout::field_size(f).div_ceil(8).saturating_mul(8))
        .sum();
    VariantRepr {
        name: Name::from_raw(0),
        fields,
        size,
        alignment: if size > 0 { 8 } else { 1 },
    }
}

fn int_field() -> MachineRepr {
    MachineRepr::Int {
        width: IntWidth::I64,
        signed: true,
    }
}

// --- compute_enum_layout_info: the four EnumTag variants ---

#[test]
fn all_unit_enum_has_tag_no_payload() {
    // 4-variant all-unit enum: explicit i8 tag, no payload, abi_size = 1.
    let repr = EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I8,
        },
        variants: vec![
            variant(vec![]),
            variant(vec![]),
            variant(vec![]),
            variant(vec![]),
        ],
        size: 1,
        align: 1,
    };
    let info = compute_enum_layout_info(&repr, false);
    assert_eq!(info.tag_gep_index, Some(0));
    assert_eq!(
        info.payload_gep_index, None,
        "all-unit enum has no payload field"
    );
    assert_eq!(info.abi_size, 1);
    assert_eq!(info.payload_slot_count, 0);
    assert_eq!(info.variant_count, 4);
    assert!(info.has_explicit_tag());
}

#[test]
fn option_int_explicit_tag_at_zero_payload_at_one() {
    // Option<int> explicit: tag at GEP 0, payload at GEP 1, abi_size = 16.
    let repr = EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I8,
        },
        variants: vec![variant(vec![]), variant(vec![int_field()])],
        size: 16,
        align: 8,
    };
    let info = compute_enum_layout_info(&repr, true);
    assert_eq!(info.tag_gep_index, Some(0));
    assert_eq!(info.payload_gep_index, Some(1));
    assert_eq!(info.abi_size, 16);
    assert_eq!(info.payload_slot_count, 1);
    assert!(info.is_runtime_abi);
}

#[test]
fn option_bool_niche_no_tag_gep_payload_at_zero() {
    // Option<bool> niche: no tag field, payload occupies the struct, abi_size = 1.
    let repr = EnumRepr {
        tag: EnumTag::Niche {
            field_index: 0,
            niche_value: 2,
            niche_variant_idx: 0,
        },
        variants: vec![variant(vec![]), variant(vec![MachineRepr::Bool])],
        size: 1,
        align: 1,
    };
    let info = compute_enum_layout_info(&repr, true);
    assert_eq!(
        info.tag_gep_index, None,
        "niche enums have no separate tag field"
    );
    assert_eq!(info.payload_gep_index, Some(0));
    assert_eq!(info.abi_size, 1);
    assert!(info.has_niche());
}

#[test]
fn tagged_ptr_enum_has_no_struct_geps() {
    // Tagged-pointer enum: tag in pointer bits, no struct layout, abi_size = 8.
    let repr = EnumRepr {
        tag: EnumTag::TaggedPtr,
        variants: vec![variant(vec![]), variant(vec![MachineRepr::OpaquePtr])],
        size: 8,
        align: 8,
    };
    let info = compute_enum_layout_info(&repr, false);
    assert_eq!(info.tag_gep_index, None);
    assert_eq!(
        info.payload_gep_index, None,
        "tagged-pointer enums have no struct GEPs"
    );
    assert_eq!(info.abi_size, 8);
    assert!(info.is_tagged_ptr());
}

#[test]
fn single_variant_newtype_is_tagless() {
    // Single-variant newtype: no tag, payload IS the inner type, abi_size = inner size.
    let repr = EnumRepr {
        tag: EnumTag::None,
        variants: vec![variant(vec![int_field()])],
        size: 8,
        align: 8,
    };
    let info = compute_enum_layout_info(&repr, false);
    assert_eq!(info.tag_gep_index, None);
    assert_eq!(info.payload_gep_index, Some(0));
    assert_eq!(info.abi_size, 8, "newtype abi_size is the inner type size");
    assert!(info.is_tagless());
}

// --- field_is_tag ---

#[test]
fn field_is_tag_true_only_for_explicit_field_zero() {
    let explicit = compute_enum_layout_info(
        &EnumRepr {
            tag: EnumTag::Explicit {
                width: IntWidth::I8,
            },
            variants: vec![variant(vec![]), variant(vec![int_field()])],
            size: 16,
            align: 8,
        },
        false,
    );
    assert!(explicit.field_is_tag(0), "explicit tag is field 0");
    assert!(!explicit.field_is_tag(1), "field 1 is payload, not tag");

    for tag in [
        EnumTag::Niche {
            field_index: 0,
            niche_value: 2,
            niche_variant_idx: 0,
        },
        EnumTag::TaggedPtr,
        EnumTag::None,
    ] {
        let info = compute_enum_layout_info(
            &EnumRepr {
                tag,
                variants: vec![variant(vec![MachineRepr::Bool])],
                size: 1,
                align: 1,
            },
            false,
        );
        assert!(!info.field_is_tag(0), "{tag:?} has no tag field at index 0");
    }
}

// --- payload_slot_offset: prefix-sum of slot_padded_size over payload fields ---

#[test]
fn payload_slot_offset_prefix_sums_eight_byte_slots() {
    // Three 8-byte fields (24-byte payload) → offsets 0, 8, 16.
    let info = compute_enum_layout_info(
        &EnumRepr {
            tag: EnumTag::None,
            variants: vec![variant(vec![int_field(), int_field(), int_field()])],
            size: 24,
            align: 8,
        },
        false,
    );
    assert_eq!(info.payload_slot_offset(0), 0);
    assert_eq!(info.payload_slot_offset(1), 8);
    assert_eq!(info.payload_slot_offset(2), 16);
    assert_eq!(info.payload_field_offsets, vec![0, 8, 16]);
    assert_eq!(
        info.payload_slot_offset(99),
        0,
        "out-of-range index returns 0"
    );
}

#[test]
fn payload_slot_offset_first_field_always_zero() {
    // A single field of any sub-slot size sits at offset 0.
    for fields in [vec![MachineRepr::Bool], vec![int_field()]] {
        let info = compute_enum_layout_info(
            &EnumRepr {
                tag: EnumTag::None,
                variants: vec![variant(fields)],
                size: 8,
                align: 8,
            },
            false,
        );
        assert_eq!(info.payload_slot_offset(0), 0);
    }
}

#[test]
fn payload_slot_offset_sub_slot_field_rounds_next_to_eight() {
    // [bool (1B), int (8B)] → bool rounds to one slot, int sits at offset 8.
    let info = compute_enum_layout_info(
        &EnumRepr {
            tag: EnumTag::None,
            variants: vec![variant(vec![MachineRepr::Bool, int_field()])],
            size: 16,
            align: 8,
        },
        false,
    );
    assert_eq!(info.payload_slot_offset(0), 0);
    assert_eq!(info.payload_slot_offset(1), 8);
}

// --- negative pin: Option<[int]> does NOT niche (empty list = null data ptr) ---

#[test]
fn option_list_does_not_use_niche() {
    // A [int] payload's data ptr can be null, so null is not a free niche;
    // niche analysis keeps Option<[int]> explicit-tagged. The layout info must
    // faithfully report Explicit, never Niche.
    let repr = EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I8,
        },
        // Some([int]) payload modeled as a 24-byte fat pointer's three slots.
        variants: vec![
            variant(vec![]),
            variant(vec![int_field(), int_field(), int_field()]),
        ],
        size: 32,
        align: 8,
    };
    let info = compute_enum_layout_info(&repr, true);
    assert!(!info.has_niche(), "Option<[int]> must not be niche-encoded");
    assert!(info.has_explicit_tag());
    assert_eq!(info.tag_gep_index, Some(0));
}
