//! Serializable projections of versioned builtin-registry identities.

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
