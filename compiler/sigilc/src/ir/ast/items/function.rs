//! Function and Module Types
//!
//! Function definitions, test definitions, parameters, and module structure.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, PartialEq, Hash, Debug for Salsa requirements.

use std::fmt;

use crate::ir::{Name, Span, TypeId, ExprId, Spanned};
use super::super::ranges::{ParamRange, GenericParamRange};
use super::traits::WhereClause;
use super::imports::UseDef;

/// Parameter in a function or lambda.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Param {
    pub name: Name,
    pub ty: Option<TypeId>,
    /// The original type annotation name (e.g., `T` in `: T`).
    /// Used to connect type annotations to generic parameters for constraint checking.
    /// None if no type annotation or if it's a primitive type.
    pub type_name: Option<Name>,
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
    pub return_ty: Option<TypeId>,
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

/// Test definition.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct TestDef {
    pub name: Name,
    pub targets: Vec<Name>,
    pub params: ParamRange,
    pub return_ty: Option<TypeId>,
    pub body: ExprId,
    pub span: Span,
    /// If set, this test is skipped with the given reason.
    pub skip_reason: Option<Name>,
    /// If set, this test expects compilation to fail with an error
    /// containing this substring.
    pub compile_fail_expected: Option<Name>,
    /// If set, this test expects runtime failure with an error
    /// containing this substring.
    pub fail_expected: Option<Name>,
}

impl fmt::Debug for TestDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TestDef {{ name: {:?}, targets: {:?}, skip: {:?}, compile_fail: {:?}, fail: {:?} }}",
            self.name, self.targets, self.skip_reason, self.compile_fail_expected, self.fail_expected
        )
    }
}

impl Spanned for TestDef {
    fn span(&self) -> Span {
        self.span
    }
}

/// A parsed module (collection of items).
#[derive(Clone, Eq, PartialEq, Hash, Default)]
pub struct Module {
    /// Import statements
    pub imports: Vec<UseDef>,
    /// Function definitions
    pub functions: Vec<Function>,
    /// Test definitions
    pub tests: Vec<TestDef>,
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
            functions: Vec::new(),
            tests: Vec::new(),
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
            "Module {{ {} functions, {} tests, {} traits, {} impls, {} extends }}",
            self.functions.len(),
            self.tests.len(),
            self.traits.len(),
            self.impls.len(),
            self.extends.len()
        )
    }
}
