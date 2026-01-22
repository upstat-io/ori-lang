//! Parameter specifications for patterns.
//!
//! Declarative parameter definitions allow patterns to specify their
//! requirements once, enabling automatic validation, documentation
//! generation, and IDE support.

/// Type constraint for pattern parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeConstraint {
    /// Any type is allowed.
    Any,
    /// Must be a specific primitive type.
    Primitive(&'static str),
    /// Must be a list.
    List,
    /// Must be a list of elements matching a constraint.
    ListOf(Box<TypeConstraint>),
    /// Must be iterable (list, range, or iterator).
    Iterable,
    /// Must be a function.
    Function,
    /// Must be a function with specific arity.
    FunctionArity(usize),
    /// Must be numeric (int or float).
    Numeric,
    /// Must be a boolean.
    Boolean,
    /// Must be a duration.
    Duration,
    /// Must match the type of another parameter.
    SameAs(&'static str),
    /// Must be the element type of another list parameter.
    ElementOf(&'static str),
    /// Must be a fold function: (Acc, T) -> Acc.
    FoldFunction(&'static str, &'static str),
    /// Custom constraint with description.
    Custom(&'static str),
}

impl TypeConstraint {
    /// Get a human-readable description of this constraint.
    pub fn description(&self) -> String {
        match self {
            TypeConstraint::Any => "any type".to_string(),
            TypeConstraint::Primitive(name) => name.to_string(),
            TypeConstraint::List => "list".to_string(),
            TypeConstraint::ListOf(inner) => format!("[{}]", inner.description()),
            TypeConstraint::Iterable => "iterable (list or range)".to_string(),
            TypeConstraint::Function => "function".to_string(),
            TypeConstraint::FunctionArity(n) => format!("function with {} parameter(s)", n),
            TypeConstraint::Numeric => "numeric (int or float)".to_string(),
            TypeConstraint::Boolean => "bool".to_string(),
            TypeConstraint::Duration => "duration".to_string(),
            TypeConstraint::SameAs(param) => format!("same type as .{}", param),
            TypeConstraint::ElementOf(param) => format!("element type of .{}", param),
            TypeConstraint::FoldFunction(acc, elem) => {
                format!("function ({}, {}) -> {}", acc, elem, acc)
            }
            TypeConstraint::Custom(desc) => desc.to_string(),
        }
    }
}

/// Specification for a pattern parameter.
#[derive(Clone, Debug)]
pub struct ParamSpec {
    /// Parameter name (without the leading dot).
    pub name: &'static str,
    /// Whether this parameter is required.
    pub required: bool,
    /// Human-readable description.
    pub description: &'static str,
    /// Type constraint for validation.
    pub constraint: TypeConstraint,
    /// Default value expression (if optional).
    pub default: Option<&'static str>,
}

impl ParamSpec {
    /// Create a required parameter with any type.
    pub const fn required(name: &'static str, description: &'static str) -> Self {
        ParamSpec {
            name,
            required: true,
            description,
            constraint: TypeConstraint::Any,
            default: None,
        }
    }

    /// Create a required parameter with a type constraint.
    pub const fn required_with(
        name: &'static str,
        description: &'static str,
        constraint: TypeConstraint,
    ) -> Self {
        ParamSpec {
            name,
            required: true,
            description,
            constraint,
            default: None,
        }
    }

    /// Create an optional parameter with any type.
    pub const fn optional(name: &'static str, description: &'static str) -> Self {
        ParamSpec {
            name,
            required: false,
            description,
            constraint: TypeConstraint::Any,
            default: None,
        }
    }

    /// Create an optional parameter with a type constraint.
    pub const fn optional_with(
        name: &'static str,
        description: &'static str,
        constraint: TypeConstraint,
    ) -> Self {
        ParamSpec {
            name,
            required: false,
            description,
            constraint,
            default: None,
        }
    }

    /// Create an optional parameter with a default value.
    pub const fn optional_default(
        name: &'static str,
        description: &'static str,
        default: &'static str,
    ) -> Self {
        ParamSpec {
            name,
            required: false,
            description,
            constraint: TypeConstraint::Any,
            default: Some(default),
        }
    }

    /// Create a boolean flag parameter (defaults to false).
    pub const fn flag(name: &'static str, description: &'static str) -> Self {
        ParamSpec {
            name,
            required: false,
            description,
            constraint: TypeConstraint::Boolean,
            default: Some("false"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_spec_required() {
        let spec = ParamSpec::required("over", "collection to iterate");
        assert!(spec.required);
        assert_eq!(spec.name, "over");
        assert!(spec.default.is_none());
    }

    #[test]
    fn test_param_spec_optional_with_default() {
        let spec = ParamSpec::optional_default("timeout", "operation timeout", "5s");
        assert!(!spec.required);
        assert_eq!(spec.default, Some("5s"));
    }

    #[test]
    fn test_type_constraint_description() {
        assert_eq!(TypeConstraint::Any.description(), "any type");
        assert_eq!(TypeConstraint::Numeric.description(), "numeric (int or float)");
        assert_eq!(
            TypeConstraint::FunctionArity(2).description(),
            "function with 2 parameter(s)"
        );
    }
}
