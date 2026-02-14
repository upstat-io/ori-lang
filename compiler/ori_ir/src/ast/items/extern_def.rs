//! Extern block and extern item definitions.
//!
//! Grammar:
//! ```ebnf
//! extern_block  = [ "pub" ] "extern" string_literal [ "from" string_literal ] "{" { extern_item } "}" .
//! extern_item   = "@" identifier extern_params "->" type [ "as" string_literal ] .
//! extern_params = "(" [ extern_param { "," extern_param } ] [ c_variadic ] ")" .
//! extern_param  = identifier ":" type .
//! c_variadic    = "," "..." .
//! ```

use crate::{Name, ParsedType, Span, Spanned, Visibility};

/// A parameter in an extern function declaration.
///
/// Simpler than regular `Param` — no patterns, defaults, or Ori variadic flags.
/// Type annotation is always required.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ExternParam {
    pub name: Name,
    pub ty: ParsedType,
    pub span: Span,
}

impl Spanned for ExternParam {
    fn span(&self) -> Span {
        self.span
    }
}

/// A single extern function declaration within an extern block.
///
/// ```ori
/// @_sin (x: float) -> float as "sin"
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ExternItem {
    /// Ori function name (after `@`).
    pub name: Name,
    /// Typed parameters.
    pub params: Vec<ExternParam>,
    /// Return type (always required).
    pub return_ty: ParsedType,
    /// Foreign function alias: `as "sin"`. Without `as`, the Ori name is used.
    pub alias: Option<Name>,
    /// True if the parameter list ends with `, ...` (C variadic).
    pub is_c_variadic: bool,
    pub span: Span,
}

impl Spanned for ExternItem {
    fn span(&self) -> Span {
        self.span
    }
}

/// An extern block declaring foreign functions.
///
/// ```ori
/// extern "c" from "m" {
///     @_sin (x: float) -> float as "sin"
/// }
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ExternBlock {
    /// Calling convention: `"c"` or `"js"`.
    pub convention: Name,
    /// Library path from the `from` clause.
    pub library: Option<Name>,
    /// Functions declared in this block.
    pub items: Vec<ExternItem>,
    pub visibility: Visibility,
    pub span: Span,
}

impl Spanned for ExternBlock {
    fn span(&self) -> Span {
        self.span
    }
}
