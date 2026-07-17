//! Type-checker-selected callable provenance for executable method demands.

use ori_ir::{DerivedImplId, Name};

use crate::{Idx, ImplMethodId};

/// Schema of registry coordinates embedded in cached semantic artifacts.
///
/// Increment this when `TypeTag` discriminants, builtin method-table order, or
/// prelude function-table order changes. A mismatched identity fails closed.
pub const REGISTRY_PRODUCER_SCHEMA: u8 = 1;

/// Stable, serializable projection of one versioned builtin-registry method.
///
/// `ori_registry` deliberately has zero dependencies, so its identity types do
/// not derive serde traits. The type checker freezes the same three registry-
/// owned coordinates here for cached semantic artifacts.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct RegistryMethodIdentity {
    schema: u8,
    receiver_tag: u8,
    index: u16,
    arity: u8,
}

impl RegistryMethodIdentity {
    /// Project a registry-owned method identity without retaining its spelling.
    #[must_use]
    pub fn from_registered(identity: ori_registry::RegisteredMethodId) -> Self {
        let Ok(index) = u16::try_from(identity.index()) else {
            panic!("registered method index must fit its u16 registry carrier")
        };
        let Ok(arity) = u8::try_from(identity.arity()) else {
            panic!("registered method arity must fit its u8 registry carrier")
        };
        Self {
            schema: REGISTRY_PRODUCER_SCHEMA,
            receiver_tag: identity.receiver() as u8,
            index,
            arity,
        }
    }

    /// Registry-coordinate schema carried by this identity.
    #[must_use]
    pub const fn schema(self) -> u8 {
        self.schema
    }

    /// Receiver discriminant in the versioned builtin registry.
    #[must_use]
    pub const fn receiver_tag(self) -> u8 {
        self.receiver_tag
    }

    /// Method-table position in the receiver's registry entry.
    #[must_use]
    pub const fn index(self) -> u16 {
        self.index
    }

    /// Number of source operands, including an instance receiver.
    #[must_use]
    pub const fn arity(self) -> u8 {
        self.arity
    }

    /// Resolve this projection against the current versioned registry.
    #[must_use]
    pub fn resolve(self) -> Option<ori_registry::RegisteredMethodId> {
        if self.schema != REGISTRY_PRODUCER_SCHEMA {
            return None;
        }
        let receiver = ori_registry::TypeTag::all()
            .iter()
            .copied()
            .find(|tag| *tag as u8 == self.receiver_tag)?;
        let method = ori_registry::methods_for(receiver).get(usize::from(self.index))?;
        let identity = ori_registry::find_method_id(receiver, method.name)?;
        (identity.arity() == usize::from(self.arity)).then_some(identity)
    }
}

/// Stable, serializable projection of one prelude free-function identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct RegistryPreludeIdentity {
    schema: u8,
    index: u16,
    arity: u8,
}

impl RegistryPreludeIdentity {
    /// Project a registry-owned prelude identity without retaining its spelling.
    #[must_use]
    pub fn from_registered(identity: ori_registry::RegisteredPreludeFunctionId) -> Self {
        let Ok(index) = u16::try_from(identity.index()) else {
            panic!("registered prelude index must fit its u16 registry carrier")
        };
        let Ok(arity) = u8::try_from(identity.arity()) else {
            panic!("registered prelude arity must fit its u8 registry carrier")
        };
        Self {
            schema: REGISTRY_PRODUCER_SCHEMA,
            index,
            arity,
        }
    }

    /// Registry-coordinate schema carried by this identity.
    #[must_use]
    pub const fn schema(self) -> u8 {
        self.schema
    }

    /// Prelude-table position.
    #[must_use]
    pub const fn index(self) -> u16 {
        self.index
    }

    /// Number of source operands.
    #[must_use]
    pub const fn arity(self) -> u8 {
        self.arity
    }

    /// Resolve this projection against the current versioned registry.
    #[must_use]
    pub fn resolve(self) -> Option<ori_registry::RegisteredPreludeFunctionId> {
        if self.schema != REGISTRY_PRODUCER_SCHEMA {
            return None;
        }
        let function = ori_registry::PRELUDE_FUNCTIONS.get(usize::from(self.index))?;
        let identity = ori_registry::find_prelude_function_id(function.name)?;
        (identity.arity() == usize::from(self.arity)).then_some(identity)
    }
}

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
mod tests {
    use super::{RegistryMethodIdentity, RegistryPreludeIdentity, REGISTRY_PRODUCER_SCHEMA};

    #[test]
    fn every_registry_method_identity_round_trips_without_saturation() {
        for receiver in ori_registry::TypeTag::all().iter().copied() {
            for method in ori_registry::methods_for(receiver) {
                let registered = ori_registry::find_method_id(receiver, method.name)
                    .unwrap_or_else(|| {
                        panic!(
                            "missing registered identity for {receiver:?}.{}",
                            method.name
                        )
                    });
                let projected = RegistryMethodIdentity::from_registered(registered);
                assert_eq!(projected.schema(), REGISTRY_PRODUCER_SCHEMA);
                assert_eq!(projected.resolve(), Some(registered));
            }
        }
    }

    #[test]
    fn every_prelude_identity_round_trips_without_saturation() {
        for function in ori_registry::PRELUDE_FUNCTIONS {
            let registered =
                ori_registry::find_prelude_function_id(function.name).unwrap_or_else(|| {
                    panic!("missing registered prelude identity for {}", function.name)
                });
            let projected = RegistryPreludeIdentity::from_registered(registered);
            assert_eq!(projected.schema(), REGISTRY_PRODUCER_SCHEMA);
            assert_eq!(projected.resolve(), Some(registered));
        }
    }
}
