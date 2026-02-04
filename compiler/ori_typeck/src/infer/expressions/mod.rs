//! Expression type inference for literals, identifiers, and operators.
//!
//! # Specification
//!
//! - Type rules: `docs/ori_lang/0.1-alpha/spec/operator-rules.md`
//! - Prose: `docs/ori_lang/0.1-alpha/spec/09-expressions.md`
//!
//! This module is organized into focused submodules:
//! - `identifiers`: Identifier and function reference inference
//! - `operators`: Binary and unary operation type checking
//! - `lambdas`: Lambda expression inference
//! - `collections`: List, tuple, map, and range inference
//! - `structs`: Struct literal and field access helpers
//! - `access`: Field and index access inference
//! - `variants`: Result/Option variant constructor inference

mod access;
mod collections;
mod identifiers;
mod lambdas;
mod operators;
mod structs;
mod variants;

pub use access::{infer_field, infer_index};
pub use collections::{
    infer_list, infer_list_with_spread, infer_map, infer_map_with_spread, infer_range, infer_tuple,
};
pub use identifiers::{infer_function_ref, infer_ident};
pub use lambdas::infer_lambda;
pub use operators::{infer_binary, infer_unary};
pub use structs::{infer_struct, infer_struct_with_spread};
pub use variants::{infer_err, infer_none, infer_ok, infer_some};

use ori_ir::Name;
use ori_types::{Type, TypeFolder};
use std::collections::HashMap;

/// Substitute type parameter names with their corresponding type variables.
///
/// Uses `TypeFolder` to recursively transform Named types to their replacements.
pub(crate) fn substitute_type_params(ty: &Type, params: &HashMap<Name, Type>) -> Type {
    struct ParamSubstitutor<'a> {
        params: &'a HashMap<Name, Type>,
    }

    impl TypeFolder for ParamSubstitutor<'_> {
        fn fold_named(&mut self, name: Name) -> Type {
            if let Some(replacement) = self.params.get(&name) {
                replacement.clone()
            } else {
                Type::Named(name)
            }
        }
    }

    let mut substitutor = ParamSubstitutor { params };
    substitutor.fold(ty)
}
