//! Module-Level Items
//!
//! Top-level declarations: Module, Function, `TestDef`, `TypeDecl`.

mod function;
mod imports;
mod traits;
mod types;

pub use function::{
    CapabilityRef, CfgAttr, ConstDef, ExpectedError, FileAttr, Function, Module, Param, TargetAttr,
    TestDef,
};
pub use imports::{ImportErrorKind, ImportPath, UseDef, UseItem};
pub use traits::{
    DefImplDef, ExtendDef, GenericParam, ImplAssocType, ImplDef, ImplMethod, TraitAssocType,
    TraitBound, TraitDef, TraitDefaultMethod, TraitItem, TraitMethodSig, WhereClause,
};
pub use types::{StructField, TypeDecl, TypeDeclKind, Variant, VariantField};
