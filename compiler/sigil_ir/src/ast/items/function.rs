//! Function and Module Types
//!
//! Function definitions, test definitions, parameters, and module structure.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use std::fmt;

use crate::{Name, Span, ExprId, Spanned, ParsedType};
use super::super::ranges::{ParamRange, GenericParamRange};
use super::traits::WhereClause;
use super::imports::UseDef;

/// Parameter in a function or lambda.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Param {
    pub name: Name,
    /// The parsed type annotation. None if no type annotation.
    pub ty: Option<ParsedType>,
    pub span: Span,
}

impl Spanned for Param {
    fn span(&self) -> Span {
        self.span
    }
}

/// Function definition.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Function {
    pub name: Name,
    /// Generic parameters: `<T, U: Bound>`
    pub generics: GenericParamRange,
    pub params: ParamRange,
    /// The parsed return type. None if no return type annotation.
    pub return_ty: Option<ParsedType>,
    /// Where clauses: `where T: Clone, U: Default`
    pub where_clauses: Vec<WhereClause>,
    pub body: ExprId,
    pub span: Span,
    pub is_public: bool,
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Function {{ name: {:?}, generics: {:?}, params: {:?}, ret: {:?}, where: {:?}, public: {} }}",
            self.name, self.generics, self.params, self.return_ty, self.where_clauses, self.is_public
        )
    }
}

impl Spanned for Function {
    fn span(&self) -> Span {
        self.span
    }
}

/// Rich specification for expected compilation errors.
///
/// Supports matching on:
/// - Error message substring
/// - Error code (e.g., "E2001")
/// - Source position (line/column)
///
/// # Example
///
/// ```sigil
/// #[compile_fail(message: "type mismatch", code: "E2001", line: 5)]
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct ExpectedError {
    /// Substring to match in error message.
    pub message: Option<Name>,
    /// Error code to match (e.g., "E2001").
    pub code: Option<Name>,
    /// Expected line number (1-based).
    pub line: Option<u32>,
    /// Expected column number (1-based).
    pub column: Option<u32>,
}

impl ExpectedError {
    /// Create from a simple message substring (legacy format).
    pub fn from_message(message: Name) -> Self {
        ExpectedError {
            message: Some(message),
            code: None,
            line: None,
            column: None,
        }
    }

    /// Check if this specification is empty (no requirements).
    pub fn is_empty(&self) -> bool {
        self.message.is_none() && self.code.is_none()
            && self.line.is_none() && self.column.is_none()
    }
}

/// Test definition.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct TestDef {
    pub name: Name,
    pub targets: Vec<Name>,
    pub params: ParamRange,
    /// The parsed return type. None if no return type annotation.
    pub return_ty: Option<ParsedType>,
    pub body: ExprId,
    pub span: Span,
    /// If set, this test is skipped with the given reason.
    pub skip_reason: Option<Name>,
    /// Expected compilation errors (multiple allowed).
    /// If non-empty, this is a `compile_fail` test.
    pub expected_errors: Vec<ExpectedError>,
    /// If set, this test expects runtime failure with an error
    /// containing this substring.
    pub fail_expected: Option<Name>,
}

impl TestDef {
    /// Check if this is a `compile_fail` test.
    pub fn is_compile_fail(&self) -> bool {
        !self.expected_errors.is_empty()
    }
}

impl fmt::Debug for TestDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TestDef {{ name: {:?}, targets: {:?}, skip: {:?}, expected_errors: {}, fail: {:?} }}",
            self.name, self.targets, self.skip_reason,
            self.expected_errors.len(), self.fail_expected
        )
    }
}

impl Spanned for TestDef {
    fn span(&self) -> Span {
        self.span
    }
}

/// Config variable definition.
///
/// Syntax: `[pub] $name = literal`
///
/// Config variables are compile-time constants. The type is inferred from the literal.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ConfigDef {
    pub name: Name,
    /// The initializer expression (must be a literal).
    pub value: ExprId,
    pub span: Span,
    pub is_public: bool,
}

impl Spanned for ConfigDef {
    fn span(&self) -> Span {
        self.span
    }
}

/// A parsed module (collection of items).
#[derive(Clone, Eq, PartialEq, Hash, Default)]
pub struct Module {
    /// Import statements
    pub imports: Vec<UseDef>,
    /// Config variable definitions
    pub configs: Vec<ConfigDef>,
    /// Function definitions
    pub functions: Vec<Function>,
    /// Test definitions
    pub tests: Vec<TestDef>,
    /// Type declarations (structs, sum types, newtypes)
    pub types: Vec<super::types::TypeDecl>,
    /// Trait definitions
    pub traits: Vec<super::traits::TraitDef>,
    /// Implementation blocks
    pub impls: Vec<super::traits::ImplDef>,
    /// Extension method blocks
    pub extends: Vec<super::traits::ExtendDef>,
}

impl Module {
    pub fn new() -> Self {
        Module {
            imports: Vec::new(),
            configs: Vec::new(),
            functions: Vec::new(),
            tests: Vec::new(),
            types: Vec::new(),
            traits: Vec::new(),
            impls: Vec::new(),
            extends: Vec::new(),
        }
    }
}

impl fmt::Debug for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Module {{ {} configs, {} functions, {} tests, {} types, {} traits, {} impls, {} extends }}",
            self.configs.len(),
            self.functions.len(),
            self.tests.len(),
            self.types.len(),
            self.traits.len(),
            self.impls.len(),
            self.extends.len()
        )
    }
}
