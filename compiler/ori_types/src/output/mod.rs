//! Type Checker Output Types.
//!
//! This module provides the output structures for the type checker.
//! All types are Salsa-compatible (`Clone, Eq, PartialEq, Hash, Debug`).
//!
//! # Key Types
//!
//! - [`TypedModule`]: Complete type information for a module
//! - [`FunctionSig`]: Function signature with parameter and return types
//! - [`TypeCheckResult`]: Wrapper with errors and guarantee
//!
//! Uses [`Idx`](crate::Idx) (pool-based) instead of `TypeId` (legacy interning).

mod derived;
mod mono;
mod producer;
mod result;
mod sig;
mod typed_module;

pub use derived::AcceptedDerivedImpl;
pub use mono::{
    ConcreteMethodMono, ConstGenericTerm, ConstValue, DeferredMonoCall, DeferredVarBinding,
    GenericArg, MonoConstBinding, MonoInstance, MonoInstanceId,
};
pub use producer::{
    imported_method_producer, imported_method_signature_hash, DerivedCallPlan, DerivedCallPosition,
    DerivedCallSelection, DerivedDirectCallSelection, MethodProducer, RegistryMethodIdentity,
    RegistryPreludeIdentity, IMPORTED_METHOD_PRODUCER_SCHEMA, REGISTRY_PRODUCER_SCHEMA,
};
pub use result::TypeCheckResult;
pub use sig::{ConstParamInfo, EffectClass, FnWhereClause, FunctionSig};
pub use typed_module::{
    AssignDesugar, ExportedTypeMetadata, FormatSpecTypes, ImplMethodId, ImplMethodRole, ImplSig,
    ImportedImplSig, IterMethodRoute, TypedModule,
};

// Test-support imports: the sibling `tests.rs` uses `use super::*;` and relies
// on these symbols being in scope through this module.
#[cfg(test)]
use crate::registry::TypeEntry;
#[cfg(test)]
use crate::{Idx, TypeCheckError};
#[cfg(test)]
use ori_ir::{Name, Span};

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
