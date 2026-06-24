//! Ordering value type: [`OrderingValue`].

/// Ordering value representing comparison results.
///
/// This is a first-class representation of the `Ordering` type, avoiding
/// the overhead of `Value::Variant` for this frequently-used type.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum OrderingValue {
    /// Left operand is less than right.
    Less,
    /// Operands are equal.
    Equal,
    /// Left operand is greater than right.
    Greater,
}

impl OrderingValue {
    /// Create from the raw i8 tag value.
    ///
    /// Uses the same convention as `ori_ir::builtin_constants::ordering`:
    /// - 0 = Less
    /// - 1 = Equal
    /// - 2 = Greater
    #[must_use]
    pub const fn from_tag(tag: i8) -> Option<Self> {
        match tag {
            0 => Some(Self::Less),
            1 => Some(Self::Equal),
            2 => Some(Self::Greater),
            _ => None,
        }
    }

    /// Get the raw i8 tag value.
    #[must_use]
    pub const fn to_tag(self) -> i8 {
        match self {
            Self::Less => 0,
            Self::Equal => 1,
            Self::Greater => 2,
        }
    }

    /// Get the display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Less => "Less",
            Self::Equal => "Equal",
            Self::Greater => "Greater",
        }
    }
}
