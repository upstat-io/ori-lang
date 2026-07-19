//! Type-checker-selected callable provenance for executable method demands.

mod imported;
mod registry;

pub use imported::{
    imported_method_producer, imported_method_signature_hash, IMPORTED_METHOD_PRODUCER_SCHEMA,
};
pub use registry::{RegistryMethodIdentity, RegistryPreludeIdentity, REGISTRY_PRODUCER_SCHEMA};

use ori_ir::{DerivedImplId, Name};

use crate::{Idx, ImplMethodId};

pub use ori_ir::canon::MethodProducerId;

/// Exact semantic producer selected by type checking for a direct method call.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum MethodProducer {
    /// One exact method in the versioned builtin registry.
    Registry(RegistryMethodIdentity),
    /// One exact free function in the versioned prelude registry.
    Prelude(RegistryPreludeIdentity),
    /// A local source/default impl body.
    Impl(ImplMethodId),
    /// A compiler-generated accepted derive body.
    Derived(DerivedImplId),
    /// A producer-module method carried through its stable exported boundary.
    Imported {
        /// Exact link symbol exported by the producer module.
        symbol: Box<str>,
        /// Stable producer signature hash.
        signature_hash: u64,
    },
}

/// Structural location of one nested call in a generated derived body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum DerivedCallPosition {
    /// Direct delegation by a newtype derive.
    Newtype,
    /// Product field in declaration order.
    Field(u32),
    /// Sum payload field, keyed by declaration-order variant and field.
    VariantField { variant: u32, field: u32 },
    /// Enum declaration ordinal comparison in a generated body.
    Discriminant,
    /// Product-field accumulator combine in declaration order.
    FieldCombine(u32),
    /// Enum declaration ordinal folded into an accumulator.
    DiscriminantCombine,
    /// Sum payload accumulator combine in declaration order.
    VariantFieldCombine { variant: u32, field: u32 },
}

/// One frozen nested-call selection in a concrete derived specialization.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct DerivedCallSelection {
    /// Structural body position, independent of SSA destination numbering.
    pub position: DerivedCallPosition,
    /// Concrete nested receiver in the module's retained type pool.
    pub receiver_type: Idx,
    /// Exact required trait identity.
    pub trait_type: Idx,
    /// Exact required method spelling retained only for consistency validation.
    pub method_name: Name,
    /// Whether the selected method consumes an explicit receiver operand.
    pub has_self: bool,
    /// Checker-selected executable producer.
    pub producer: MethodProducer,
}

/// One frozen generated free-function call.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct DerivedDirectCallSelection {
    /// Structural body position, independent of SSA destination numbering.
    pub position: DerivedCallPosition,
    /// Exact source-level callable name retained for consistency validation.
    pub function_name: Name,
    /// Checker-selected executable producer.
    pub producer: MethodProducer,
}

/// Frozen nested-call plan for one accepted derive specialization.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct DerivedCallPlan {
    /// Accepted outer generated body.
    pub derived: DerivedImplId,
    /// Concrete impl-binder substitutions in declaration order.
    ///
    /// Empty for a non-generic owner. This is the specialization key; callers
    /// do not key plans by a resolved layout or a nominal spelling.
    pub binder_substitutions: Vec<Idx>,
    /// Ordered structural calls emitted by that body.
    pub calls: Vec<DerivedCallSelection>,
    /// Ordered compiler-generated free-function calls emitted by the body.
    pub direct_calls: Vec<DerivedDirectCallSelection>,
}

#[cfg(test)]
mod tests;
