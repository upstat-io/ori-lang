//! `Channel` type definition.
//!
//! Channel is an Arc-managed shared communication primitive (`Channel<T>`).
//! All Channel methods are impure — they read or mutate shared mutable
//! state and cannot be safely reordered by the optimizer.
//!
//! `send` takes ownership of the value being sent. `recv`/`try_recv`
//! return `Option<T>`. `close` transitions the channel to a closed state.

use crate::{
    DeiPropagation, MemoryStrategy, MethodDef, MethodKind, OpDefs, Ownership, ParamDef, ReturnTag,
    TypeDef, TypeParamArity, TypeProjection, TypeTag,
};

/// `(value: T)` — element being sent.
static ELEMENT_PARAM: [ParamDef; 1] = [ParamDef {
    name: "value",
    ty: ReturnTag::ElementType,
    ownership: Ownership::Owned,
}];

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const OPT_ELEM: ReturnTag = ReturnTag::OptionOf(TypeProjection::Element);

/// Channel-specific method constructor: impure instance method.
///
/// All Channel methods have `pure: false` because they access shared
/// mutable state. Uses `backend_required: false`.
const fn chan(
    name: &'static str,
    params: &'static [ParamDef],
    returns: ReturnTag,
    receiver: Ownership,
    trait_name: Option<&'static str>,
) -> MethodDef {
    MethodDef {
        name,
        receiver,
        params,
        returns,
        trait_name,
        pure: false,
        backend_required: false,
        kind: MethodKind::Instance,
        dei_only: false,
        dei_propagation: DeiPropagation::NotApplicable,
    }
}

// All 9 methods alphabetically sorted.
static CHANNEL_METHODS: &[MethodDef] = &[
    chan("close", &[], ReturnTag::Unit, Ownership::Borrow, None),
    chan("is_closed", &[], BOOL, Ownership::Borrow, None),
    chan("is_empty", &[], BOOL, Ownership::Borrow, Some("IsEmpty")),
    chan("len", &[], INT, Ownership::Borrow, Some("Len")),
    chan("receive", &[], OPT_ELEM, Ownership::Borrow, None),
    chan("recv", &[], OPT_ELEM, Ownership::Borrow, None),
    chan(
        "send",
        &ELEMENT_PARAM,
        ReturnTag::Unit,
        Ownership::Borrow,
        None,
    ),
    chan("try_receive", &[], OPT_ELEM, Ownership::Borrow, None),
    chan("try_recv", &[], OPT_ELEM, Ownership::Borrow, None),
];

pub static CHANNEL: TypeDef = TypeDef {
    tag: TypeTag::Channel,
    name: "Channel",
    memory: MemoryStrategy::Arc,
    type_params: TypeParamArity::Fixed(1),
    methods: CHANNEL_METHODS,
    operators: OpDefs::UNSUPPORTED,
    traits: &[],
};

#[cfg(test)]
mod tests;
