//! Backend-neutral runtime-call identities.

/// A runtime operation whose source-level meaning is fixed before backend selection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RuntimeCall {
    /// Create an iterator from an iterable value.
    Iter,
    /// Allocate a list builder.
    ListNew,
    /// Advance an iterator.
    IterNext,
    /// Append a value to a list builder.
    ListBuilderPush,
    /// Append one value to a persistent list with copy-on-write semantics.
    ListPush,
    /// Release iterator state.
    IterDrop,
    /// Finish a list builder.
    ListTake,
    /// Index a collection.
    Index,
    /// Produce a collection with one updated element.
    Updated,
    /// Replace one list element through the concrete `List.set` method.
    ListSet,
    /// Read a collection or string length.
    Length,
    /// Convert a value to its string form.
    ToString,
    /// Concatenate strings.
    Concat,
    /// Test whether a string contains another string.
    StringContains,
    /// Test whether a string starts with a prefix.
    StringStartsWith,
    /// Test whether a string ends with a suffix.
    StringEndsWith,
    /// Test whether a string is empty.
    StringIsEmpty,
    /// Trim leading and trailing Unicode whitespace.
    StringTrim,
    /// Convert a string to Unicode uppercase.
    StringUppercase,
    /// Convert a string to Unicode lowercase.
    StringLowercase,
    /// Split a string by a separator.
    StringSplit,
    /// Print a string.
    Print,
    /// Raise an Ori panic.
    Panic,
}

impl RuntimeCall {
    /// Return the fixed number of operands accepted by this runtime operation.
    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::Iter
            | Self::IterDrop
            | Self::ListTake
            | Self::Length
            | Self::ToString
            | Self::StringIsEmpty
            | Self::StringTrim
            | Self::StringUppercase
            | Self::StringLowercase
            | Self::Print
            | Self::Panic => 1,
            Self::ListNew
            | Self::IterNext
            | Self::Index
            | Self::ListPush
            | Self::Concat
            | Self::StringContains
            | Self::StringStartsWith
            | Self::StringEndsWith
            | Self::StringSplit => 2,
            Self::ListBuilderPush | Self::Updated | Self::ListSet => 3,
        }
    }

    pub(super) fn resolve(symbol: &str, receiver: Option<ori_registry::TypeTag>) -> Option<Self> {
        if matches!(receiver, Some(ori_registry::TypeTag::Str)) {
            let string_call = match symbol {
                "contains" => Some(Self::StringContains),
                "starts_with" => Some(Self::StringStartsWith),
                "ends_with" => Some(Self::StringEndsWith),
                "is_empty" => Some(Self::StringIsEmpty),
                "trim" => Some(Self::StringTrim),
                "to_uppercase" => Some(Self::StringUppercase),
                "to_lowercase" => Some(Self::StringLowercase),
                "split" => Some(Self::StringSplit),
                _ => None,
            };
            if string_call.is_some() {
                return string_call;
            }
        }
        if matches!(receiver, Some(ori_registry::TypeTag::List)) {
            match symbol {
                "push" => return Some(Self::ListPush),
                "set" => return Some(Self::ListSet),
                _ => {}
            }
        }
        Self::from_symbol(symbol)
    }

    fn from_symbol(symbol: &str) -> Option<Self> {
        match symbol {
            "iter" => Some(Self::Iter),
            "ori_list_new" => Some(Self::ListNew),
            "__iter_next" => Some(Self::IterNext),
            "ori_list_push" => Some(Self::ListBuilderPush),
            "ori_iter_drop" => Some(Self::IterDrop),
            "ori_list_take" => Some(Self::ListTake),
            "__index" => Some(Self::Index),
            "updated" => Some(Self::Updated),
            "len" => Some(Self::Length),
            "to_str" | "str" => Some(Self::ToString),
            "concat" => Some(Self::Concat),
            "ori_print" => Some(Self::Print),
            "ori_panic" => Some(Self::Panic),
            _ => None,
        }
    }
}
