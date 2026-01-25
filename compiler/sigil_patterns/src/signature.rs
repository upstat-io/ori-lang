//! Pattern signature for template caching.
//!
//! Two patterns with the same signature share the same compiled template,
//! enabling efficient code generation through template reuse.

use sigil_ir::{Name, TypeId};

/// Semantic identity of a pattern instantiation.
///
/// Patterns with the same signature can share compiled templates.
/// This enables efficient code generation by avoiding redundant compilation.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct PatternSignature {
    /// Pattern kind (e.g., interned "map", "filter").
    pub kind: Name,

    /// Input types in canonical order.
    pub input_types: Vec<TypeId>,

    /// Output type.
    pub output_type: TypeId,

    /// Transform function signature (if applicable).
    pub transform_sig: Option<FunctionSignature>,

    /// Additional type parameters.
    pub type_params: Vec<TypeId>,
}

impl PatternSignature {
    /// Create a new pattern signature.
    pub fn new(kind: Name, output_type: TypeId) -> Self {
        PatternSignature {
            kind,
            input_types: Vec::new(),
            output_type,
            transform_sig: None,
            type_params: Vec::new(),
        }
    }

    /// Add an input type.
    pub fn with_input(mut self, ty: TypeId) -> Self {
        self.input_types.push(ty);
        self
    }

    /// Add multiple input types.
    pub fn with_inputs(mut self, types: impl IntoIterator<Item = TypeId>) -> Self {
        self.input_types.extend(types);
        self
    }

    /// Set the transform function signature.
    pub fn with_transform(mut self, sig: FunctionSignature) -> Self {
        self.transform_sig = Some(sig);
        self
    }

    /// Add a type parameter.
    pub fn with_type_param(mut self, ty: TypeId) -> Self {
        self.type_params.push(ty);
        self
    }
}

/// Function signature for template caching.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FunctionSignature {
    /// Parameter types.
    pub params: Vec<TypeId>,
    /// Return type.
    pub ret: TypeId,
}

impl FunctionSignature {
    /// Create a new function signature.
    pub fn new(params: Vec<TypeId>, ret: TypeId) -> Self {
        FunctionSignature { params, ret }
    }

    /// Create a unary function signature (one parameter).
    pub fn unary(param: TypeId, ret: TypeId) -> Self {
        FunctionSignature {
            params: vec![param],
            ret,
        }
    }

    /// Create a binary function signature (two parameters).
    pub fn binary(param1: TypeId, param2: TypeId, ret: TypeId) -> Self {
        FunctionSignature {
            params: vec![param1, param2],
            ret,
        }
    }
}

/// Default value for optional pattern arguments.
#[derive(Clone, Debug)]
pub enum DefaultValue {
    /// No default (argument truly optional).
    None,
    /// Boolean default.
    Bool(bool),
    /// Integer default.
    Int(i64),
    /// String default (for lambda expressions as source).
    Str(&'static str),
}

/// Optional argument specification with default value.
#[derive(Clone, Debug)]
pub struct OptionalArg {
    /// Argument name.
    pub name: &'static str,
    /// Default value.
    pub default: DefaultValue,
}

impl OptionalArg {
    /// Create an optional boolean argument.
    pub const fn bool(name: &'static str, default: bool) -> Self {
        OptionalArg {
            name,
            default: DefaultValue::Bool(default),
        }
    }

    /// Create an optional integer argument.
    pub const fn int(name: &'static str, default: i64) -> Self {
        OptionalArg {
            name,
            default: DefaultValue::Int(default),
        }
    }

    /// Create an optional argument with no default.
    pub const fn none(name: &'static str) -> Self {
        OptionalArg {
            name,
            default: DefaultValue::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_signature_eq() {
        let name1 = Name::new(0, 1);
        let name2 = Name::new(0, 1);
        let name3 = Name::new(0, 2);

        let sig1 = PatternSignature::new(name1, TypeId::INT);
        let sig2 = PatternSignature::new(name2, TypeId::INT);
        let sig3 = PatternSignature::new(name3, TypeId::INT);

        assert_eq!(sig1, sig2);
        assert_ne!(sig1, sig3);
    }

    #[test]
    fn test_pattern_signature_builder() {
        let name = Name::new(0, 1);
        let sig = PatternSignature::new(name, TypeId::INT)
            .with_input(TypeId::BOOL)
            .with_inputs([TypeId::STR, TypeId::FLOAT])
            .with_transform(FunctionSignature::unary(TypeId::INT, TypeId::BOOL));

        assert_eq!(sig.input_types.len(), 3);
        assert!(sig.transform_sig.is_some());
    }

    #[test]
    fn test_function_signature() {
        let unary = FunctionSignature::unary(TypeId::INT, TypeId::BOOL);
        assert_eq!(unary.params.len(), 1);
        assert_eq!(unary.params[0], TypeId::INT);
        assert_eq!(unary.ret, TypeId::BOOL);

        let binary = FunctionSignature::binary(TypeId::INT, TypeId::STR, TypeId::FLOAT);
        assert_eq!(binary.params.len(), 2);
    }
}
