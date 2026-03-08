//! Ori Registry — Pure-Data Type Behavioral Specifications
//!
//! This crate is the single source of truth for every builtin type's
//! methods, operators, ownership semantics, and memory strategy. It has
//! **zero dependencies** and contains **no logic** — only `const` data
//! definitions and simple lookup functions.
//!
//! # Query Model
//!
//! All queries follow the pattern: tag → `TypeDef` → field/method.
//!
//! ```
//! use ori_registry::{find_type, find_method, TypeTag, Ownership};
//!
//! // Look up a type
//! let str_def = find_type(TypeTag::Str).unwrap();
//! assert_eq!(str_def.name, "str");
//!
//! // Look up a method
//! let length = find_method(TypeTag::Str, "length").unwrap();
//! assert_eq!(length.receiver, Ownership::Borrow);
//!
//! // Operator strategies are accessed via TypeDef.operators
//! let add_strategy = str_def.operators.add;
//! ```
//!
//! # Purity Contract
//!
//! This crate has **zero runtime dependencies**. It contains only:
//! - `const` data definitions
//! - `const fn` constructors and lookups
//! - Enums and structs that derive standard traits
//!
//! No IO, no allocation, no side effects, no `unsafe`.
//! All data is `const`-constructible and baked into `.rodata`.
//!
//! # Design Invariants
//!
//! 1. **Zero dependencies** — Cargo.toml has no `[dependencies]`
//! 2. **No logic** — only `const fn` constructors and linear-scan lookups
//! 3. **All data `const`** — baked into `.rodata`, no heap allocation
//! 4. **Deterministic** — safe for Salsa memoization
//! 5. **Structural enforcement** — adding a field to [`TypeDef`] is a
//!    compile error in every type definition, enforcing full coverage
//!
//! # Extensibility
//!
//! New fields on [`TypeDef`] or [`MethodDef`] are **required by default**.
//! Use `Option<NewField>` only for fields that genuinely don't apply to
//! some types.
//!
//! ## Adding a new field
//!
//! 1. Add the field to the struct (`TypeDef`, `MethodDef`, or `OpDefs`)
//! 2. Compilation fails in every `defs/*.rs` file — fill in each one
//! 3. Update consuming phases (`ori_types`, `ori_eval`, `ori_arc`, `ori_llvm`)
//! 4. Add tests verifying the new field's behavior
//! 5. Update the plan section that documents the struct

pub mod defs;
mod method;
mod operator;
mod query;
mod tags;
mod type_def;

pub use defs::BUILTIN_TYPES;
pub use method::{
    MethodDef, ParamDef, ONE_SELF_BORROW, ONE_SELF_COPY, ONE_SELF_OWNED, TWO_SELF_COPY,
};
pub use operator::OpDefs;
pub use query::{
    borrowing_method_names, borrowing_methods, dei_only_methods, find_method, find_type,
    find_type_by_name, has_method, is_dei_only, iterator_method_names, legacy_type_name,
    method_names_for, methods_for,
};
pub use tags::{
    DeiPropagation, MemoryStrategy, MethodKind, OpStrategy, Ownership, ReturnTag, TypeParamArity,
    TypeProjection, TypeTag,
};
pub use type_def::{TypeDef, VariantSpec};

#[cfg(test)]
mod tests;
