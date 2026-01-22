//! High-level Intermediate Representation (HIR)
//!
//! This module provides name resolution and scope management for the type checker.
//! It bridges the gap between the parsed AST and the typed IR.

mod scope;
mod resolver;
mod registry;

pub use scope::{ScopeId, Scopes, Binding, LocalVar};
pub use resolver::{Resolver, ResolvedName, ResolutionError, BuiltinKind};
pub use registry::{
    FunctionSig, TypeDef as HirTypeDef, ConfigDef, TraitDef, ImplDef,
    DefinitionRegistry,
};
