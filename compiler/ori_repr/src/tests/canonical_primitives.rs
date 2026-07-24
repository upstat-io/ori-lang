use super::*;

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
