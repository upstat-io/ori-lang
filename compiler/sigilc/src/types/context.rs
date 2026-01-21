// Type context for the Sigil type checker
// Holds type definitions, function signatures, and local bindings

use crate::ast::{TypeDef, TypeExpr};
use std::collections::HashMap;

/// Function signature for type checking
#[derive(Clone)]
pub struct FunctionSig {
    pub type_params: Vec<String>,
    pub params: Vec<(String, TypeExpr)>,
    pub return_type: TypeExpr,
}

/// Type checking context
pub struct TypeContext {
    /// Type definitions
    pub(super) types: HashMap<String, TypeDef>,

    /// Function signatures
    pub(super) functions: HashMap<String, FunctionSig>,

    /// Config variables
    pub(super) configs: HashMap<String, TypeExpr>,

    /// Local variable types (in current scope)
    pub(super) locals: HashMap<String, TypeExpr>,

    /// Current function's return type (for `self` calls in recurse)
    pub(super) current_return_type: Option<TypeExpr>,
}

impl TypeContext {
    pub fn new() -> Self {
        let mut ctx = TypeContext {
            types: HashMap::new(),
            functions: HashMap::new(),
            configs: HashMap::new(),
            locals: HashMap::new(),
            current_return_type: None,
        };

        // Register builtin functions
        ctx.register_builtins();
        ctx
    }

    fn register_builtins(&mut self) {
        // print: any -> void
        self.functions.insert(
            "print".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![("value".to_string(), TypeExpr::Named("any".to_string()))],
                return_type: TypeExpr::Named("void".to_string()),
            },
        );

        // str: any -> str (conversion)
        self.functions.insert(
            "str".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![("value".to_string(), TypeExpr::Named("any".to_string()))],
                return_type: TypeExpr::Named("str".to_string()),
            },
        );

        // int: any -> int (conversion)
        self.functions.insert(
            "int".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![("value".to_string(), TypeExpr::Named("any".to_string()))],
                return_type: TypeExpr::Named("int".to_string()),
            },
        );

        // float: any -> float (conversion)
        self.functions.insert(
            "float".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![("value".to_string(), TypeExpr::Named("any".to_string()))],
                return_type: TypeExpr::Named("float".to_string()),
            },
        );

        // len: any -> int (polymorphic: works on strings and lists)
        self.functions.insert(
            "len".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![("value".to_string(), TypeExpr::Named("any".to_string()))],
                return_type: TypeExpr::Named("int".to_string()),
            },
        );

        // assert: bool -> void
        self.functions.insert(
            "assert".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![("condition".to_string(), TypeExpr::Named("bool".to_string()))],
                return_type: TypeExpr::Named("void".to_string()),
            },
        );

        // assert_eq: (T, T) -> void
        self.functions.insert(
            "assert_eq".to_string(),
            FunctionSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    ("actual".to_string(), TypeExpr::Named("T".to_string())),
                    ("expected".to_string(), TypeExpr::Named("T".to_string())),
                ],
                return_type: TypeExpr::Named("void".to_string()),
            },
        );

        // assert_err: Result T E -> void
        self.functions.insert(
            "assert_err".to_string(),
            FunctionSig {
                type_params: vec!["T".to_string(), "E".to_string()],
                params: vec![(
                    "result".to_string(),
                    TypeExpr::Generic(
                        "Result".to_string(),
                        vec![
                            TypeExpr::Named("T".to_string()),
                            TypeExpr::Named("E".to_string()),
                        ],
                    ),
                )],
                return_type: TypeExpr::Named("void".to_string()),
            },
        );

        // Arithmetic operators as first-class functions
        // +: (T, T) -> T where T is numeric
        self.functions.insert(
            "+".to_string(),
            FunctionSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    ("a".to_string(), TypeExpr::Named("T".to_string())),
                    ("b".to_string(), TypeExpr::Named("T".to_string())),
                ],
                return_type: TypeExpr::Named("T".to_string()),
            },
        );

        // -: (T, T) -> T where T is numeric
        self.functions.insert(
            "-".to_string(),
            FunctionSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    ("a".to_string(), TypeExpr::Named("T".to_string())),
                    ("b".to_string(), TypeExpr::Named("T".to_string())),
                ],
                return_type: TypeExpr::Named("T".to_string()),
            },
        );

        // *: (T, T) -> T where T is numeric
        self.functions.insert(
            "*".to_string(),
            FunctionSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    ("a".to_string(), TypeExpr::Named("T".to_string())),
                    ("b".to_string(), TypeExpr::Named("T".to_string())),
                ],
                return_type: TypeExpr::Named("T".to_string()),
            },
        );

        // /: (T, T) -> T where T is numeric
        self.functions.insert(
            "/".to_string(),
            FunctionSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    ("a".to_string(), TypeExpr::Named("T".to_string())),
                    ("b".to_string(), TypeExpr::Named("T".to_string())),
                ],
                return_type: TypeExpr::Named("T".to_string()),
            },
        );

        // %: (int, int) -> int
        self.functions.insert(
            "%".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![
                    ("a".to_string(), TypeExpr::Named("int".to_string())),
                    ("b".to_string(), TypeExpr::Named("int".to_string())),
                ],
                return_type: TypeExpr::Named("int".to_string()),
            },
        );
    }

    pub fn set_current_return_type(&mut self, ty: TypeExpr) {
        self.current_return_type = Some(ty);
    }

    #[allow(dead_code)]
    pub fn clear_current_return_type(&mut self) {
        self.current_return_type = None;
    }

    pub fn define_type(&mut self, name: String, def: TypeDef) {
        self.types.insert(name, def);
    }

    pub fn define_function(&mut self, name: String, sig: FunctionSig) {
        self.functions.insert(name, sig);
    }

    pub fn define_config(&mut self, name: String, ty: TypeExpr) {
        self.configs.insert(name, ty);
    }

    pub fn define_local(&mut self, name: String, ty: TypeExpr) {
        self.locals.insert(name, ty);
    }

    pub fn lookup_type(&self, name: &str) -> Option<&TypeDef> {
        self.types.get(name)
    }

    pub fn lookup_function(&self, name: &str) -> Option<&FunctionSig> {
        self.functions.get(name)
    }

    pub fn lookup_config(&self, name: &str) -> Option<&TypeExpr> {
        self.configs.get(name)
    }

    pub fn lookup_local(&self, name: &str) -> Option<&TypeExpr> {
        self.locals.get(name)
    }

    /// Get the current function's return type (for `self` calls in recurse)
    pub fn current_return_type(&self) -> Option<TypeExpr> {
        self.current_return_type.clone()
    }

    /// Get a snapshot of current locals (for saving/restoring state)
    pub fn save_locals(&self) -> HashMap<String, TypeExpr> {
        self.locals.clone()
    }

    /// Restore locals from a saved snapshot
    pub fn restore_locals(&mut self, locals: HashMap<String, TypeExpr>) {
        self.locals = locals;
    }

    /// Create a child context that inherits all state (for block scopes)
    pub fn child(&self) -> Self {
        TypeContext {
            types: self.types.clone(),
            functions: self.functions.clone(),
            configs: self.configs.clone(),
            locals: self.locals.clone(),
            current_return_type: self.current_return_type.clone(),
        }
    }

    /// Create a child context with additional locals added via a closure
    pub fn child_with_locals<F>(&self, f: F) -> Self
    where
        F: FnOnce(&mut HashMap<String, TypeExpr>),
    {
        let mut child = self.child();
        f(&mut child.locals);
        child
    }
}
