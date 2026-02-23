//! Flat AST types using arena allocation.
//!
//! Per design spec A-data-structures.md:
//! - No `Box<Expr>`, use `ExprId(u32)` indices
//! - Contiguous arrays for cache locality
//! - All types have Salsa-required traits (Clone, Eq, Hash, Debug)
//!
//! # Module Structure
//!
//! - `expr`: Core expression types (Expr, `ExprKind`)
//! - `operators`: Binary and unary operators
//! - `stmt`: Statement types
//! - `ranges`: Arena range types for efficient iteration
//! - `collections`: Map entries, field initializers, call arguments
//! - `patterns/`: Binding patterns, match patterns, `FunctionSeq`, `FunctionExp`
//! - `items/`: Module-level items (Function, `TestDef`, imports, traits)

mod collections;
mod expr;
pub mod items;
mod operators;
pub mod patterns;
mod ranges;
mod stmt;

// Re-export core expression types
pub use expr::{Expr, ExprKind, TemplatePart};
pub use operators::{BinaryOp, UnaryOp};
pub use stmt::{Stmt, StmtKind};

// Re-export all range types
pub use ranges::{
    ArmRange, CallArgRange, FieldInitRange, GenericParamRange, ListElementRange, MapElementRange,
    MapEntryRange, NamedExprRange, ParamRange, StructLitFieldRange, TemplatePartRange,
};

// Re-export collection types
pub use collections::{CallArg, FieldInit, ListElement, MapElement, MapEntry, StructLitField};

// Re-export pattern types
pub use patterns::{
    BindingPattern, FieldBinding, FunctionExp, FunctionExpKind, FunctionSeq, MatchArm,
    MatchPattern, Mutability, NamedExpr,
};

// Re-export item types
pub use items::{
    CapabilityRef, CfgAttr, ConstDef, DefImplDef, ExpectedError, ExtendDef, ExtensionImport,
    ExtensionImportItem, ExternBlock, ExternItem, ExternParam, FileAttr, Function, GenericParam,
    ImplAssocType, ImplDef, ImplMethod, ImportErrorKind, ImportPath, Module, Param, PostContract,
    PreContract, StructField, TargetAttr, TestDef, TraitAssocType, TraitBound, TraitDef,
    TraitDefaultMethod, TraitItem, TraitMethodSig, TypeDecl, TypeDeclKind, UseDef, UseItem,
    Variant, VariantField, WhereClause,
};

/// Visibility of a declaration.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, Default)]
pub enum Visibility {
    /// Private (default visibility, accessible only within the module).
    #[default]
    Private,
    /// Public (accessible from other modules).
    Public,
}

impl Visibility {
    /// Returns true if this is public visibility.
    pub fn is_public(self) -> bool {
        matches!(self, Visibility::Public)
    }
}

#[cfg(test)]
mod tests;
