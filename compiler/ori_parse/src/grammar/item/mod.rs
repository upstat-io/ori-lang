//! Item parsing (functions, tests, imports, traits, impls).
//!
//! This module extends Parser with methods for parsing top-level items
//! like function definitions, test definitions, import statements,
//! trait definitions, and implementation blocks.
//!
//! # Module Structure
//!
//! - `use_def.rs`: Import/use statement parsing
//! - `config.rs`: Config variable parsing
//! - `function.rs`: Function and test definition parsing
//! - `trait_def.rs`: Trait definition parsing
//! - `impl_def.rs`: Impl block parsing
//! - `type_decl.rs`: Type declaration parsing (struct, enum, newtype)
//! - `extend.rs`: Extend block parsing
//! - `generics.rs`: Generic parameters, bounds, where clauses

mod use_def;
mod config;
mod function;
mod trait_def;
mod impl_def;
mod type_decl;
mod extend;
mod generics;
