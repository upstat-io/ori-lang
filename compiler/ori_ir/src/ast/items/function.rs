//! Function and Module Types
//!
//! Function definitions, test definitions, parameters, and module structure.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use std::fmt;

use super::super::ranges::{GenericParamRange, ParamRange};
use super::super::Visibility;
use super::imports::UseDef;
use super::traits::WhereClause;
use crate::{ExprId, Name, ParsedType, Span, Spanned};

/// Parameter in a function or lambda.
///
/// Supports clause-based parameters with patterns and default values:
/// - Simple: `(x: int)` — name only
/// - Pattern: `(0: int)` — literal pattern
/// - Default: `(x: int = 42)` — default value
/// - Variadic: `(nums: ...int)` — receives zero or more values as `[T]`
///
/// # Fields
/// - `name`: Primary binding name. For simple params, this is the identifier.
///   For patterns, this may be derived from the primary binding or generated.
/// - `pattern`: Optional match pattern. If None, this is a simple name binding.
/// - `ty`: Optional type annotation.
/// - `default`: Optional default value expression.
/// - `is_variadic`: If true, this parameter accepts multiple values (`...T`).
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Param {
    /// Primary binding name (for simple params or derived from pattern).
    pub name: Name,
    /// Optional pattern for clause-based parameters (e.g., `0`, `Some(x)`).
    /// If None, this is a simple name binding.
    pub pattern: Option<super::super::patterns::MatchPattern>,
    /// The parsed type annotation. None if no type annotation.
    pub ty: Option<ParsedType>,
    /// Default value expression (e.g., `x: int = 42`).
    pub default: Option<ExprId>,
    /// If true, this is a variadic parameter (`nums: ...int`).
    /// Variadic params receive values as `[T]` inside the function.
    pub is_variadic: bool,
    pub span: Span,
}

impl Spanned for Param {
    fn span(&self) -> Span {
        self.span
    }
}

/// Capability requirement in a `uses` clause.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct CapabilityRef {
    pub name: Name,
    pub span: Span,
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
    /// Capabilities required by this function: `uses Http, FileSystem`
    pub capabilities: Vec<CapabilityRef>,
    /// Where clauses: `where T: Clone, U: Default`
    pub where_clauses: Vec<WhereClause>,
    /// Guard clause: `if condition` before `=`
    /// Example: `@abs (n: int) -> int if n < 0 = -n`
    pub guard: Option<ExprId>,
    pub body: ExprId,
    pub span: Span,
    pub visibility: Visibility,
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Function {{ name: {:?}, generics: {:?}, params: {:?}, ret: {:?}, uses: {:?}, where: {:?}, guard: {:?}, visibility: {:?} }}",
            self.name, self.generics, self.params, self.return_ty, self.capabilities, self.where_clauses, self.guard, self.visibility
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
/// ```ori
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
    /// Create from a simple message substring.
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
        self.message.is_none()
            && self.code.is_none()
            && self.line.is_none()
            && self.column.is_none()
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
            self.name,
            self.targets,
            self.skip_reason,
            self.expected_errors.len(),
            self.fail_expected
        )
    }
}

impl Spanned for TestDef {
    fn span(&self) -> Span {
        self.span
    }
}

/// Constant definition.
///
/// Syntax: `[pub] let $name = literal`
///
/// Constants are compile-time immutable bindings. The type is inferred from the literal.
/// They can be imported via `use "./module" { $const_name }`.
///
/// # Fields
///
/// - `name`: The interned name of the constant (without the `$` prefix).
/// - `value`: The initializer expression ID (must resolve to a literal).
/// - `span`: The source span covering the entire definition.
/// - `visibility`: The visibility of this constant.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ConstDef {
    /// The interned name of the constant (without the `$` prefix).
    pub name: Name,
    /// The initializer expression (must be a literal).
    /// At parse time, this points to an `ExprKind::Int`, `ExprKind::Float`,
    /// `ExprKind::String`, `ExprKind::Bool`, or similar literal node.
    pub value: ExprId,
    /// Source span covering the entire constant definition (`let $name = value`).
    pub span: Span,
    /// Visibility of this constant (`pub let $name = ...` or private).
    pub visibility: Visibility,
}

impl Spanned for ConstDef {
    fn span(&self) -> Span {
        self.span
    }
}

/// A parsed module (collection of items).
#[derive(Clone, Eq, PartialEq, Hash, Default)]
pub struct Module {
    /// Import statements
    pub imports: Vec<UseDef>,
    /// Constant definitions
    pub consts: Vec<ConstDef>,
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
    /// Default trait implementations
    pub def_impls: Vec<super::traits::DefImplDef>,
    /// Extension method blocks
    pub extends: Vec<super::traits::ExtendDef>,
}

impl Module {
    pub fn new() -> Self {
        Module {
            imports: Vec::new(),
            consts: Vec::new(),
            functions: Vec::new(),
            tests: Vec::new(),
            types: Vec::new(),
            traits: Vec::new(),
            impls: Vec::new(),
            def_impls: Vec::new(),
            extends: Vec::new(),
        }
    }

    /// Create a new module with pre-allocated capacity based on source length.
    ///
    /// Heuristic: ~1 function per 50 bytes of source code.
    /// This reduces allocation overhead during parsing.
    #[inline]
    pub fn with_capacity_hint(source_len: usize) -> Self {
        // Estimate: 1 function per ~50 bytes, minimum 8
        let func_estimate = (source_len / 50).max(8);
        // Tests are usually fewer than functions
        let test_estimate = func_estimate / 4;
        // Types, traits, impls are typically sparse
        let type_estimate = (func_estimate / 8).max(2);

        Module {
            imports: Vec::with_capacity(4),
            consts: Vec::with_capacity(2),
            functions: Vec::with_capacity(func_estimate),
            tests: Vec::with_capacity(test_estimate),
            types: Vec::with_capacity(type_estimate),
            traits: Vec::with_capacity(type_estimate),
            impls: Vec::with_capacity(type_estimate),
            def_impls: Vec::with_capacity(2),
            extends: Vec::with_capacity(2),
        }
    }
}

impl fmt::Debug for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Module {{ {} consts, {} functions, {} tests, {} types, {} traits, {} impls, {} def_impls, {} extends }}",
            self.consts.len(),
            self.functions.len(),
            self.tests.len(),
            self.types.len(),
            self.traits.len(),
            self.impls.len(),
            self.def_impls.len(),
            self.extends.len()
        )
    }
}
