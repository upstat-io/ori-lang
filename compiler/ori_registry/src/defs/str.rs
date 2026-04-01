//! `str` type definition.
//!
//! `str` is the first `MemoryStrategy::Arc` type in the registry. Unlike
//! primitives (Copy), str is reference-counted with SSO (strings ≤23 bytes
//! stored inline). All operators delegate to `ori_rt` runtime functions.

use crate::{
    defs::params::{COUNT_PARAM, INT_RANGE_PARAMS},
    MemoryStrategy, MethodDef, OpDefs, OpStrategy, Ownership, ParamDef, ReturnTag, TypeDef,
    TypeParamArity, TypeProjection, TypeTag, ONE_SELF_BORROW,
};

// Parameter arrays

/// `(substr: str)` — for `contains`, `index_of`, `last_index_of`.
static SUBSTR_PARAM: [ParamDef; 1] = [ParamDef {
    name: "substr",
    ty: ReturnTag::Concrete(TypeTag::Str),
    ownership: Ownership::Borrow,
}];

/// `(other: str)` — for `add`, `concat`.
static OTHER_STR_PARAM: [ParamDef; 1] = [ParamDef {
    name: "other",
    ty: ReturnTag::Concrete(TypeTag::Str),
    ownership: Ownership::Borrow,
}];

/// `(width: int, fill: str)` — for `pad_start`, `pad_end`.
static PAD_PARAMS: [ParamDef; 2] = [
    ParamDef {
        name: "width",
        ty: ReturnTag::Concrete(TypeTag::Int),
        ownership: Ownership::Copy,
    },
    ParamDef {
        name: "fill",
        ty: ReturnTag::Concrete(TypeTag::Str),
        ownership: Ownership::Borrow,
    },
];

/// `(pattern: str, replacement: str)` — for `replace`.
static REPLACE_PARAMS: [ParamDef; 2] = [
    ParamDef {
        name: "pattern",
        ty: ReturnTag::Concrete(TypeTag::Str),
        ownership: Ownership::Borrow,
    },
    ParamDef {
        name: "replacement",
        ty: ReturnTag::Concrete(TypeTag::Str),
        ownership: Ownership::Borrow,
    },
];

/// `(bytes: [byte])` — for `from_utf8`, `from_utf8_unchecked`.
static BYTES_PARAM: [ParamDef; 1] = [ParamDef {
    name: "bytes",
    ty: ReturnTag::List(TypeTag::Byte),
    ownership: Ownership::Owned,
}];

/// `(sep: str)` — for `split`.
static SEP_PARAM: [ParamDef; 1] = [ParamDef {
    name: "sep",
    ty: ReturnTag::Concrete(TypeTag::Str),
    ownership: Ownership::Borrow,
}];

// Helper aliases for common return tags
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const STR_TAG: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const ORD: ReturnTag = ReturnTag::Concrete(TypeTag::Ordering);
const ERROR_TAG: ReturnTag = ReturnTag::Concrete(TypeTag::Error);

// Shorthand: all str instance methods share these 5 frozen defaults
// via MethodDef::primitive(). See method/mod.rs.
// - pure: true
// - backend_required: true
// - kind: MethodKind::Instance
// - dei_only: false
// - dei_propagation: DeiPropagation::NotApplicable

static STR_METHODS: &[MethodDef] = &[
    // All methods alphabetically sorted
    MethodDef::primitive(
        "add",
        &OTHER_STR_PARAM,
        ReturnTag::SelfType,
        Some("Add"),
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "as_bytes",
        &[],
        ReturnTag::List(TypeTag::Byte),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive("byte_len", &[], INT, None, Ownership::Borrow),
    MethodDef::primitive(
        "bytes",
        &[],
        ReturnTag::List(TypeTag::Byte),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "chars",
        &[],
        ReturnTag::List(TypeTag::Char),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "clone",
        &[],
        ReturnTag::SelfType,
        Some("Clone"),
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "compare",
        &ONE_SELF_BORROW,
        ORD,
        Some("Comparable"),
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "concat",
        &OTHER_STR_PARAM,
        ReturnTag::SelfType,
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive("contains", &SUBSTR_PARAM, BOOL, None, Ownership::Borrow),
    MethodDef::primitive("debug", &[], STR_TAG, Some("Debug"), Ownership::Borrow),
    MethodDef::primitive(
        "ends_with",
        &[ParamDef {
            name: "suffix",
            ty: STR_TAG,
            ownership: Ownership::Borrow,
        }],
        BOOL,
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "equals",
        &ONE_SELF_BORROW,
        BOOL,
        Some("Eq"),
        Ownership::Borrow,
    ),
    MethodDef::primitive("escape", &[], ReturnTag::SelfType, None, Ownership::Borrow),
    // Associated functions
    MethodDef::associated_backend(
        "from_utf8",
        &BYTES_PARAM,
        ReturnTag::ResultOfProjectionFresh(TypeProjection::Fixed(TypeTag::Str)),
    ),
    MethodDef::associated_backend("from_utf8_unchecked", &BYTES_PARAM, ReturnTag::SelfType),
    MethodDef::primitive("hash", &[], INT, Some("Hashable"), Ownership::Borrow),
    MethodDef::primitive(
        "index_of",
        &SUBSTR_PARAM,
        ReturnTag::Option(TypeTag::Int),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive("into", &[], ERROR_TAG, None, Ownership::Borrow),
    MethodDef::primitive("is_empty", &[], BOOL, Some("IsEmpty"), Ownership::Borrow),
    MethodDef::primitive(
        "iter",
        &[],
        ReturnTag::DoubleEndedIterator(TypeTag::Char),
        Some("Iterable"),
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "last_index_of",
        &SUBSTR_PARAM,
        ReturnTag::Option(TypeTag::Int),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive("len", &[], INT, Some("Len"), Ownership::Borrow),
    MethodDef::primitive("length", &[], INT, Some("Len"), Ownership::Borrow),
    MethodDef::primitive(
        "lines",
        &[],
        ReturnTag::List(TypeTag::Str),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "pad_end",
        &PAD_PARAMS,
        ReturnTag::SelfType,
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "pad_start",
        &PAD_PARAMS,
        ReturnTag::SelfType,
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "parse_float",
        &[],
        ReturnTag::Option(TypeTag::Float),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "parse_int",
        &[],
        ReturnTag::Option(TypeTag::Int),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "repeat",
        &COUNT_PARAM,
        ReturnTag::SelfType,
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "replace",
        &REPLACE_PARAMS,
        ReturnTag::SelfType,
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "slice",
        &INT_RANGE_PARAMS,
        ReturnTag::SelfType,
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "split",
        &SEP_PARAM,
        ReturnTag::List(TypeTag::Str),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "starts_with",
        &[ParamDef {
            name: "prefix",
            ty: STR_TAG,
            ownership: Ownership::Borrow,
        }],
        BOOL,
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "substring",
        &INT_RANGE_PARAMS,
        ReturnTag::SelfType,
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "to_bytes",
        &[],
        ReturnTag::List(TypeTag::Byte),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "to_float",
        &[],
        ReturnTag::Option(TypeTag::Float),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "to_int",
        &[],
        ReturnTag::Option(TypeTag::Int),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "to_lowercase",
        &[],
        ReturnTag::SelfType,
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive("to_str", &[], STR_TAG, Some("Printable"), Ownership::Borrow),
    MethodDef::primitive(
        "to_uppercase",
        &[],
        ReturnTag::SelfType,
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive("trim", &[], ReturnTag::SelfType, None, Ownership::Borrow),
    MethodDef::primitive(
        "trim_end",
        &[],
        ReturnTag::SelfType,
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "trim_start",
        &[],
        ReturnTag::SelfType,
        None,
        Ownership::Borrow,
    ),
];

pub static STR: TypeDef = TypeDef {
    tag: TypeTag::Str,
    name: "str",
    memory: MemoryStrategy::Arc,
    type_params: TypeParamArity::Fixed(0),
    methods: STR_METHODS,
    operators: OpDefs {
        add: OpStrategy::RuntimeCall {
            fn_name: "ori_str_concat",
            returns_bool: false,
        },
        sub: OpStrategy::Unsupported,
        mul: OpStrategy::Unsupported,
        div: OpStrategy::Unsupported,
        rem: OpStrategy::Unsupported,
        floor_div: OpStrategy::Unsupported,
        eq: OpStrategy::RuntimeCall {
            fn_name: "ori_str_eq",
            returns_bool: true,
        },
        neq: OpStrategy::RuntimeCall {
            fn_name: "ori_str_ne",
            returns_bool: true,
        },
        lt: OpStrategy::RuntimeCall {
            fn_name: "ori_str_compare",
            returns_bool: true,
        },
        gt: OpStrategy::RuntimeCall {
            fn_name: "ori_str_compare",
            returns_bool: true,
        },
        lt_eq: OpStrategy::RuntimeCall {
            fn_name: "ori_str_compare",
            returns_bool: true,
        },
        gt_eq: OpStrategy::RuntimeCall {
            fn_name: "ori_str_compare",
            returns_bool: true,
        },
        neg: OpStrategy::Unsupported,
        not: OpStrategy::Unsupported,
        bit_and: OpStrategy::Unsupported,
        bit_or: OpStrategy::Unsupported,
        bit_xor: OpStrategy::Unsupported,
        bit_not: OpStrategy::Unsupported,
        shl: OpStrategy::Unsupported,
        shr: OpStrategy::Unsupported,
    },
    traits: &["Default"],
};
