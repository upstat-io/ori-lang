use super::*;
use crate::method::MethodDef;
use crate::operator::OpDefs;
use crate::tags::{
    DeiPropagation, MemoryStrategy, MethodKind, OpStrategy, Ownership, ReturnTag, TypeParamArity,
    TypeTag,
};

static TEST_METHODS: [MethodDef; 2] = [
    MethodDef {
        name: "abs",
        receiver: Ownership::Borrow,
        params: &[],
        returns: ReturnTag::SelfType,
        trait_name: None,
        pure: true,
        backend_required: true,
        kind: MethodKind::Instance,
        dei_only: false,
        dei_propagation: DeiPropagation::NotApplicable,
    },
    MethodDef {
        name: "to_str",
        receiver: Ownership::Borrow,
        params: &[],
        returns: ReturnTag::Concrete(TypeTag::Str),
        trait_name: Some("Printable"),
        pure: true,
        backend_required: true,
        kind: MethodKind::Instance,
        dei_only: false,
        dei_propagation: DeiPropagation::NotApplicable,
    },
];

static TEST_TYPE_DEF: TypeDef = TypeDef {
    tag: TypeTag::Int,
    name: "int",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: &TEST_METHODS,
    operators: OpDefs {
        add: OpStrategy::IntInstr,
        sub: OpStrategy::IntInstr,
        mul: OpStrategy::IntInstr,
        div: OpStrategy::IntInstr,
        rem: OpStrategy::IntInstr,
        floor_div: OpStrategy::IntInstr,
        eq: OpStrategy::IntInstr,
        neq: OpStrategy::IntInstr,
        lt: OpStrategy::IntInstr,
        gt: OpStrategy::IntInstr,
        lt_eq: OpStrategy::IntInstr,
        gt_eq: OpStrategy::IntInstr,
        neg: OpStrategy::IntInstr,
        not: OpStrategy::Unsupported,
        bit_and: OpStrategy::IntInstr,
        bit_or: OpStrategy::IntInstr,
        bit_xor: OpStrategy::IntInstr,
        bit_not: OpStrategy::IntInstr,
        shl: OpStrategy::IntInstr,
        shr: OpStrategy::IntInstr,
    },
    traits: &[],
};

#[test]
fn type_def_const_constructible() {
    assert_eq!(TEST_TYPE_DEF.tag, TypeTag::Int);
    assert_eq!(TEST_TYPE_DEF.name, "int");
    assert_eq!(TEST_TYPE_DEF.memory, MemoryStrategy::Copy);
    assert_eq!(TEST_TYPE_DEF.type_params, TypeParamArity::Fixed(0));
    assert_eq!(TEST_TYPE_DEF.methods.len(), 2);
    assert_eq!(TEST_TYPE_DEF.methods[0].name, "abs");
    assert_eq!(TEST_TYPE_DEF.operators.add, OpStrategy::IntInstr);
    assert_eq!(TEST_TYPE_DEF.operators.not, OpStrategy::Unsupported);
}

#[test]
fn type_def_static_ref() {
    let td: &'static TypeDef = &TEST_TYPE_DEF;
    assert_eq!(td.tag, TypeTag::Int);
    assert_eq!(td.methods.len(), 2);
}

#[test]
fn type_def_in_static_slice() {
    static EMPTY_DEF: TypeDef = TypeDef {
        tag: TypeTag::Unit,
        name: "void",
        memory: MemoryStrategy::Copy,
        type_params: TypeParamArity::Fixed(0),
        methods: &[],
        operators: OpDefs::UNSUPPORTED,
        traits: &[],
    };
    static TYPES: &[&TypeDef] = &[&TEST_TYPE_DEF, &EMPTY_DEF];
    assert_eq!(TYPES.len(), 2);
    assert_eq!(TYPES[0].tag, TypeTag::Int);
    assert_eq!(TYPES[1].tag, TypeTag::Unit);
    assert_eq!(TYPES[1].methods.len(), 0);
}
