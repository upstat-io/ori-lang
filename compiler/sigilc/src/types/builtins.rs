// Declarative builtin function registration for Sigil
// Reduces boilerplate by using macros to define builtin functions

use crate::ast::TypeExpr;
use super::registries::{FunctionRegistry, FunctionSig};

// Helper macros for type expressions
macro_rules! any {
    () => {
        TypeExpr::Named("any".to_string())
    };
}

macro_rules! void {
    () => {
        TypeExpr::Named("void".to_string())
    };
}

macro_rules! int {
    () => {
        TypeExpr::Named("int".to_string())
    };
}

macro_rules! str {
    () => {
        TypeExpr::Named("str".to_string())
    };
}

macro_rules! bool_ {
    () => {
        TypeExpr::Named("bool".to_string())
    };
}

#[allow(unused_macros)]
macro_rules! float {
    () => {
        TypeExpr::Named("float".to_string())
    };
}

macro_rules! named {
    ($n:expr) => {
        TypeExpr::Named($n.to_string())
    };
}

macro_rules! generic {
    ($n:expr, $($t:expr),*) => {
        TypeExpr::Generic($n.to_string(), vec![$($t),*])
    };
}

/// Macro for declarative builtin function registration
///
/// Syntax:
/// ```ignore
/// define_builtins! {
///     "name": (param: type, ...) -> return_type;
///     "name": <T, ...> (param: type, ...) -> return_type;
/// }
/// ```
macro_rules! define_builtins {
    (
        $(
            $name:literal : $( < $($tp:ident),* > )? ( $( $pname:ident : $pty:expr ),* $(,)? ) -> $ret:expr
        );* $(;)?
    ) => {
        /// Register all builtin functions
        pub fn register_builtins(registry: &mut FunctionRegistry) {
            $(
                registry.define(
                    $name.to_string(),
                    FunctionSig {
                        type_params: vec![$( $(stringify!($tp).to_string()),* )?],
                        type_param_bounds: vec![],
                        params: vec![$( (stringify!($pname).to_string(), $pty) ),*],
                        return_type: $ret,
                        capabilities: vec![],
                    },
                );
            )*
        }
    };
}

// Define all builtin functions declaratively
define_builtins! {
    // I/O and conversion functions
    "print": (value: any!()) -> void!();
    "str": (value: any!()) -> str!();
    "int": (value: any!()) -> int!();
    "float": (value: any!()) -> named!("float");
    "len": (value: any!()) -> int!();

    // Assertions
    "assert": (condition: bool_!()) -> void!();
    "assert_eq": <T> (actual: named!("T"), expected: named!("T")) -> void!();
    "assert_err": <T, E> (
        result: generic!("Result", named!("T"), named!("E"))
    ) -> void!();

    // Arithmetic operators as first-class functions
    "+": <T> (a: named!("T"), b: named!("T")) -> named!("T");
    "-": <T> (a: named!("T"), b: named!("T")) -> named!("T");
    "*": <T> (a: named!("T"), b: named!("T")) -> named!("T");
    "/": <T> (a: named!("T"), b: named!("T")) -> named!("T");
    "%": (a: int!(), b: int!()) -> int!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_builtins() {
        let mut registry = FunctionRegistry::new();
        register_builtins(&mut registry);

        // Check that essential builtins are registered
        assert!(registry.lookup("print").is_some());
        assert!(registry.lookup("assert").is_some());
        assert!(registry.lookup("assert_eq").is_some());
        assert!(registry.lookup("+").is_some());
        assert!(registry.lookup("-").is_some());
        assert!(registry.lookup("*").is_some());
        assert!(registry.lookup("/").is_some());
        assert!(registry.lookup("%").is_some());
    }

    #[test]
    fn test_print_signature() {
        let mut registry = FunctionRegistry::new();
        register_builtins(&mut registry);

        let print_sig = registry.lookup("print").unwrap();
        assert_eq!(print_sig.params.len(), 1);
        assert_eq!(print_sig.params[0].0, "value");
        assert_eq!(print_sig.return_type, TypeExpr::Named("void".to_string()));
    }

    #[test]
    fn test_assert_eq_is_generic() {
        let mut registry = FunctionRegistry::new();
        register_builtins(&mut registry);

        let assert_eq_sig = registry.lookup("assert_eq").unwrap();
        assert_eq!(assert_eq_sig.type_params, vec!["T".to_string()]);
        assert_eq!(assert_eq_sig.params.len(), 2);
    }

    #[test]
    fn test_modulo_not_generic() {
        let mut registry = FunctionRegistry::new();
        register_builtins(&mut registry);

        let mod_sig = registry.lookup("%").unwrap();
        assert!(mod_sig.type_params.is_empty());
        assert_eq!(mod_sig.params[0].1, TypeExpr::Named("int".to_string()));
    }
}
