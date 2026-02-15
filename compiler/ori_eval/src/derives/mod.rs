//! Derive trait processing for the evaluator.
//!
//! Processes `#[derive(Trait)]` attributes on type declarations and
//! generates derived method entries for the `UserMethodRegistry`.
//!
//! `DefaultFieldType` lives here (not in `ori_ir`) because it is consumed
//! exclusively by the evaluator — LLVM codegen uses `const_zero` instead.

use ori_ir::{
    DerivedMethodInfo, DerivedTrait, Module, Name, ParsedType, StringInterner, StructField,
    TypeDeclKind, TypeId,
};
use rustc_hash::FxHashMap;

use crate::UserMethodRegistry;

/// The type of a field for Default trait derivation.
///
/// Captures whether a field has a known primitive default (e.g., `int` → 0)
/// or a named type whose `.default()` must be called recursively.
#[derive(Clone, Debug)]
pub enum DefaultFieldType {
    /// A primitive type with a known default value.
    Primitive(TypeId),
    /// A named type — call `Type.default()` recursively.
    Named(Name),
}

/// Registry of default field types, keyed by `(type_name, method_name)`.
///
/// Populated during `process_derives` alongside the `UserMethodRegistry`.
/// Consumed by `eval_derived_default` to construct default struct values.
#[derive(Clone, Debug, Default)]
pub struct DefaultFieldTypeRegistry {
    entries: FxHashMap<(Name, Name), Vec<DefaultFieldType>>,
}

impl DefaultFieldTypeRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register default field types for a derived method.
    pub fn register(
        &mut self,
        type_name: Name,
        method_name: Name,
        field_types: Vec<DefaultFieldType>,
    ) {
        self.entries.insert((type_name, method_name), field_types);
    }

    /// Look up default field types for a derived method.
    pub fn lookup(&self, type_name: Name, method_name: Name) -> Option<&[DefaultFieldType]> {
        self.entries
            .get(&(type_name, method_name))
            .map(Vec::as_slice)
    }

    /// Merge another registry into this one.
    pub fn merge(&mut self, other: DefaultFieldTypeRegistry) {
        self.entries.extend(other.entries);
    }
}

/// Map a `ParsedType` to a `DefaultFieldType` for Default trait derivation.
fn parsed_type_to_default_field(ty: &ParsedType, interner: &StringInterner) -> DefaultFieldType {
    match ty {
        ParsedType::Primitive(id) => DefaultFieldType::Primitive(*id),
        ParsedType::Named { name, .. } => DefaultFieldType::Named(*name),
        ParsedType::List(_) | ParsedType::FixedList { .. } => {
            DefaultFieldType::Named(interner.intern("List"))
        }
        ParsedType::Map { .. } => DefaultFieldType::Named(interner.intern("Map")),
        ParsedType::Tuple(_) => DefaultFieldType::Named(interner.intern("Tuple")),
        ParsedType::Function { .. } => DefaultFieldType::Named(interner.intern("Function")),
        ParsedType::Infer
        | ParsedType::SelfType
        | ParsedType::AssociatedType { .. }
        | ParsedType::ConstExpr(_)
        | ParsedType::TraitBounds(_) => DefaultFieldType::Named(interner.intern("Unknown")),
    }
}

/// Extract field types from struct fields for Default derivation.
fn extract_field_types(fields: &[StructField], interner: &StringInterner) -> Vec<DefaultFieldType> {
    fields
        .iter()
        .map(|f| parsed_type_to_default_field(&f.ty, interner))
        .collect()
}

/// Process all derive attributes in a module.
///
/// For each type with `#[derive(...)]` attributes, generates derived method
/// entries in the user method registry. Default field types are stored in the
/// separate `DefaultFieldTypeRegistry` (evaluator-local, not in IR).
pub fn process_derives(
    module: &Module,
    user_method_registry: &mut UserMethodRegistry,
    default_field_types: &mut DefaultFieldTypeRegistry,
    interner: &StringInterner,
) {
    for type_decl in &module.types {
        if type_decl.derives.is_empty() {
            continue;
        }

        let type_name = type_decl.name;

        // Get field names and types based on type kind
        let (field_names, field_types) = match &type_decl.kind {
            TypeDeclKind::Struct(fields) => {
                let names = fields.iter().map(|f| f.name).collect();
                let types = extract_field_types(fields, interner);
                (names, types)
            }
            TypeDeclKind::Sum(_variants) => {
                // For sum types, we'll need variant-specific handling
                // For now, use empty lists and handle variants in the evaluator
                (Vec::new(), Vec::new())
            }
            TypeDeclKind::Newtype(_) => {
                // Newtypes wrap a single value
                (Vec::new(), Vec::new())
            }
        };

        // Process each derived trait
        for derive_name in &type_decl.derives {
            let trait_name_str = interner.lookup(*derive_name);

            if let Some(trait_kind) = DerivedTrait::from_name(trait_name_str) {
                let method_name = interner.intern(trait_kind.method_name());
                let info = DerivedMethodInfo::new(trait_kind, field_names.clone());

                // Store Default field types in the evaluator-local registry
                if trait_kind == DerivedTrait::Default {
                    default_field_types.register(type_name, method_name, field_types.clone());
                }

                user_method_registry.register_derived(type_name, method_name, info);
            }
            // Unknown derive traits are ignored here (type checker may report an error)
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
