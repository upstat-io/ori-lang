//! Separator handling for different packing modes.

use super::{ConstructKind, Packing};

/// What separator to use between items.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Separator {
    /// Comma separator: ", " inline, ",\n" when broken
    #[default]
    Comma,

    /// Space separator: " " inline, "\n" when broken
    Space,

    /// Pipe separator: " | " inline, "\n| " when broken (sum types)
    Pipe,
}

impl Separator {
    /// Get the inline string for this separator.
    #[inline]
    pub fn inline_str(self) -> &'static str {
        match self {
            Separator::Comma => ", ",
            Separator::Space => " ",
            Separator::Pipe => " | ",
        }
    }

    /// Get the broken prefix (what comes before item on new line).
    #[inline]
    pub fn broken_prefix(self) -> &'static str {
        match self {
            Separator::Comma | Separator::Space => "",
            Separator::Pipe => "| ",
        }
    }

    /// Get the broken suffix (what comes after item, before newline).
    #[inline]
    pub fn broken_suffix(self) -> &'static str {
        match self {
            Separator::Comma => ",",
            Separator::Space | Separator::Pipe => "",
        }
    }

    /// Check if this separator uses commas.
    #[inline]
    pub fn is_comma(self) -> bool {
        matches!(self, Separator::Comma)
    }
}

/// Determine the separator for a construct and packing mode.
///
/// Most constructs use commas. Sum types use pipes.
pub fn separator_for(construct: ConstructKind, _packing: Packing) -> Separator {
    match construct {
        // Sum variants use | separator
        ConstructKind::SumVariants => Separator::Pipe,

        // Everything else uses commas
        ConstructKind::RunTopLevel
        | ConstructKind::Try
        | ConstructKind::Match
        | ConstructKind::Recurse
        | ConstructKind::Parallel
        | ConstructKind::Spawn
        | ConstructKind::Nursery
        | ConstructKind::FunctionParams
        | ConstructKind::FunctionArgs
        | ConstructKind::GenericParams
        | ConstructKind::WhereConstraints
        | ConstructKind::Capabilities
        | ConstructKind::StructFieldsDef
        | ConstructKind::StructFieldsLiteral
        | ConstructKind::MapEntries
        | ConstructKind::TupleElements
        | ConstructKind::ImportItems
        | ConstructKind::ListSimple
        | ConstructKind::ListComplex
        | ConstructKind::RunNested
        | ConstructKind::MatchArms => Separator::Comma,
    }
}
