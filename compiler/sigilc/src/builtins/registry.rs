// Builtin Registry for Sigil
// Single source of truth for builtin functions and methods

use crate::ast::TypeExpr;

/// Information about a builtin function
#[derive(Clone, Debug)]
pub struct BuiltinFunction {
    pub name: &'static str,
    pub description: &'static str,
    pub param_types: Vec<(&'static str, TypeExpr)>,
    pub return_type: TypeExpr,
    pub is_generic: bool,
}

/// Information about a builtin method
#[derive(Clone, Debug)]
pub struct BuiltinMethod {
    pub receiver_type: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub param_types: Vec<(&'static str, TypeExpr)>,
    pub return_type: TypeExpr,
}

/// Registry of all builtin functions and methods
pub struct BuiltinRegistry {
    functions: Vec<BuiltinFunction>,
    methods: Vec<BuiltinMethod>,
}

impl BuiltinRegistry {
    /// Create a new registry with all builtins
    pub fn new() -> Self {
        let mut registry = BuiltinRegistry {
            functions: Vec::new(),
            methods: Vec::new(),
        };
        registry.register_functions();
        registry.register_methods();
        registry
    }

    fn register_functions(&mut self) {
        // print: any -> void
        self.functions.push(BuiltinFunction {
            name: "print",
            description: "Print a value to stdout",
            param_types: vec![("value", TypeExpr::Named("any".to_string()))],
            return_type: TypeExpr::Named("void".to_string()),
            is_generic: false,
        });

        // str: any -> str
        self.functions.push(BuiltinFunction {
            name: "str",
            description: "Convert a value to string",
            param_types: vec![("value", TypeExpr::Named("any".to_string()))],
            return_type: TypeExpr::Named("str".to_string()),
            is_generic: false,
        });

        // int: any -> int
        self.functions.push(BuiltinFunction {
            name: "int",
            description: "Convert a value to int",
            param_types: vec![("value", TypeExpr::Named("any".to_string()))],
            return_type: TypeExpr::Named("int".to_string()),
            is_generic: false,
        });

        // float: any -> float
        self.functions.push(BuiltinFunction {
            name: "float",
            description: "Convert a value to float",
            param_types: vec![("value", TypeExpr::Named("any".to_string()))],
            return_type: TypeExpr::Named("float".to_string()),
            is_generic: false,
        });

        // len: any -> int
        self.functions.push(BuiltinFunction {
            name: "len",
            description: "Get length of a string or list",
            param_types: vec![("value", TypeExpr::Named("any".to_string()))],
            return_type: TypeExpr::Named("int".to_string()),
            is_generic: false,
        });

        // assert: bool -> void
        self.functions.push(BuiltinFunction {
            name: "assert",
            description: "Assert a condition is true",
            param_types: vec![("condition", TypeExpr::Named("bool".to_string()))],
            return_type: TypeExpr::Named("void".to_string()),
            is_generic: false,
        });

        // assert_eq: (T, T) -> void
        self.functions.push(BuiltinFunction {
            name: "assert_eq",
            description: "Assert two values are equal",
            param_types: vec![
                ("actual", TypeExpr::Named("T".to_string())),
                ("expected", TypeExpr::Named("T".to_string())),
            ],
            return_type: TypeExpr::Named("void".to_string()),
            is_generic: true,
        });

        // assert_err: Result T E -> void
        self.functions.push(BuiltinFunction {
            name: "assert_err",
            description: "Assert a result is an error",
            param_types: vec![(
                "result",
                TypeExpr::Generic(
                    "Result".to_string(),
                    vec![
                        TypeExpr::Named("T".to_string()),
                        TypeExpr::Named("E".to_string()),
                    ],
                ),
            )],
            return_type: TypeExpr::Named("void".to_string()),
            is_generic: true,
        });
    }

    fn register_methods(&mut self) {
        // String methods
        self.methods.push(BuiltinMethod {
            receiver_type: "str",
            name: "len",
            description: "Get string length",
            param_types: vec![],
            return_type: TypeExpr::Named("int".to_string()),
        });

        self.methods.push(BuiltinMethod {
            receiver_type: "str",
            name: "upper",
            description: "Convert to uppercase",
            param_types: vec![],
            return_type: TypeExpr::Named("str".to_string()),
        });

        self.methods.push(BuiltinMethod {
            receiver_type: "str",
            name: "lower",
            description: "Convert to lowercase",
            param_types: vec![],
            return_type: TypeExpr::Named("str".to_string()),
        });

        self.methods.push(BuiltinMethod {
            receiver_type: "str",
            name: "trim",
            description: "Trim whitespace",
            param_types: vec![],
            return_type: TypeExpr::Named("str".to_string()),
        });

        self.methods.push(BuiltinMethod {
            receiver_type: "str",
            name: "split",
            description: "Split string by separator",
            param_types: vec![("separator", TypeExpr::Named("str".to_string()))],
            return_type: TypeExpr::List(Box::new(TypeExpr::Named("str".to_string()))),
        });

        self.methods.push(BuiltinMethod {
            receiver_type: "str",
            name: "slice",
            description: "Get substring from start to end",
            param_types: vec![
                ("start", TypeExpr::Named("int".to_string())),
                ("end", TypeExpr::Named("int".to_string())),
            ],
            return_type: TypeExpr::Named("str".to_string()),
        });

        self.methods.push(BuiltinMethod {
            receiver_type: "str",
            name: "contains",
            description: "Check if string contains substring",
            param_types: vec![("substring", TypeExpr::Named("str".to_string()))],
            return_type: TypeExpr::Named("bool".to_string()),
        });

        // List methods (generic over T)
        self.methods.push(BuiltinMethod {
            receiver_type: "[T]",
            name: "len",
            description: "Get list length",
            param_types: vec![],
            return_type: TypeExpr::Named("int".to_string()),
        });

        self.methods.push(BuiltinMethod {
            receiver_type: "[T]",
            name: "first",
            description: "Get first element as optional",
            param_types: vec![],
            return_type: TypeExpr::Optional(Box::new(TypeExpr::Named("T".to_string()))),
        });

        self.methods.push(BuiltinMethod {
            receiver_type: "[T]",
            name: "last",
            description: "Get last element as optional",
            param_types: vec![],
            return_type: TypeExpr::Optional(Box::new(TypeExpr::Named("T".to_string()))),
        });

        self.methods.push(BuiltinMethod {
            receiver_type: "[T]",
            name: "push",
            description: "Return new list with element appended",
            param_types: vec![("element", TypeExpr::Named("T".to_string()))],
            return_type: TypeExpr::List(Box::new(TypeExpr::Named("T".to_string()))),
        });

        self.methods.push(BuiltinMethod {
            receiver_type: "[T]",
            name: "pop",
            description: "Return new list without last element",
            param_types: vec![],
            return_type: TypeExpr::List(Box::new(TypeExpr::Named("T".to_string()))),
        });

        self.methods.push(BuiltinMethod {
            receiver_type: "[T]",
            name: "slice",
            description: "Get sublist from start to end",
            param_types: vec![
                ("start", TypeExpr::Named("int".to_string())),
                ("end", TypeExpr::Named("int".to_string())),
            ],
            return_type: TypeExpr::List(Box::new(TypeExpr::Named("T".to_string()))),
        });

        self.methods.push(BuiltinMethod {
            receiver_type: "[T]",
            name: "join",
            description: "Join list elements with separator",
            param_types: vec![("separator", TypeExpr::Named("str".to_string()))],
            return_type: TypeExpr::Named("str".to_string()),
        });
    }

    /// Get all builtin function names
    pub fn function_names(&self) -> Vec<&str> {
        self.functions.iter().map(|f| f.name).collect()
    }

    /// Get a builtin function by name
    pub fn get_function(&self, name: &str) -> Option<&BuiltinFunction> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// Get all methods for a given receiver type
    pub fn get_methods(&self, receiver_type: &str) -> Vec<&BuiltinMethod> {
        self.methods
            .iter()
            .filter(|m| m.receiver_type == receiver_type)
            .collect()
    }

    /// Get a specific method by receiver type and name
    pub fn get_method(&self, receiver_type: &str, name: &str) -> Option<&BuiltinMethod> {
        self.methods
            .iter()
            .find(|m| m.receiver_type == receiver_type && m.name == name)
    }

    /// Check if a name is a builtin function
    pub fn is_builtin_function(&self, name: &str) -> bool {
        self.functions.iter().any(|f| f.name == name)
    }
}

impl Default for BuiltinRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_print() {
        let registry = BuiltinRegistry::new();
        assert!(registry.is_builtin_function("print"));
    }

    #[test]
    fn test_registry_has_assert() {
        let registry = BuiltinRegistry::new();
        let func = registry.get_function("assert").unwrap();
        assert_eq!(func.name, "assert");
    }

    #[test]
    fn test_string_methods() {
        let registry = BuiltinRegistry::new();
        let methods = registry.get_methods("str");
        assert!(methods.len() >= 5);
    }

    #[test]
    fn test_list_methods() {
        let registry = BuiltinRegistry::new();
        let methods = registry.get_methods("[T]");
        assert!(methods.len() >= 5);
    }

    #[test]
    fn test_function_names() {
        let registry = BuiltinRegistry::new();
        let names = registry.function_names();
        assert!(names.contains(&"print"));
        assert!(names.contains(&"len"));
        assert!(names.contains(&"assert_eq"));
    }
}
