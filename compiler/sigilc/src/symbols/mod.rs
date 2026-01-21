// Symbol infrastructure for the Sigil compiler
//
// This module provides:
// - Symbol IDs for efficient reference
// - Symbol table for storing and looking up symbols
// - Scope tree for lexical name resolution
// - Name resolver pass for two-phase resolution

pub mod id;
pub mod resolver;
pub mod scope;
pub mod symbol;
pub mod table;

// Re-export key types
pub use id::{NodeId, ScopeId, SymbolId, TypeId};
pub use resolver::{resolve, ResolvedModule, Resolver};
pub use scope::{Scope, ScopeGuard, ScopeKind, ScopeTree};
pub use symbol::{
    ConfigSymbol, EnumVariant, FunctionSymbol, LocalSymbol, Symbol, SymbolKind, TraitMethod,
    TraitSymbol, TypeDefKind, TypeParamSymbol, TypeSymbol,
};
pub use table::SymbolTable;
