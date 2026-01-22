// Symbol definitions for the Sigil compiler
//
// Provides detailed symbol information for various entities.

use crate::ast::TypeExpr;

use super::id::ScopeId;

/// Detailed information about a function symbol.
#[derive(Clone, Debug)]
pub struct FunctionSymbol {
    /// Type parameters (generics)
    pub type_params: Vec<String>,
    /// Parameter names
    pub param_names: Vec<String>,
    /// Parameter types
    pub param_types: Vec<TypeExpr>,
    /// Return type
    pub return_type: TypeExpr,
    /// Whether this is a test function
    pub is_test: bool,
    /// Whether this is the main function
    pub is_main: bool,
    /// The function being tested (for test functions)
    pub tests_target: Option<String>,
}

impl FunctionSymbol {
    /// Create a new function symbol.
    pub fn new(
        type_params: Vec<String>,
        param_names: Vec<String>,
        param_types: Vec<TypeExpr>,
        return_type: TypeExpr,
    ) -> Self {
        FunctionSymbol {
            type_params,
            param_names,
            param_types,
            return_type,
            is_test: false,
            is_main: false,
            tests_target: None,
        }
    }

    /// Create a test function symbol.
    pub fn test(target: String) -> Self {
        FunctionSymbol {
            type_params: vec![],
            param_names: vec![],
            param_types: vec![],
            return_type: TypeExpr::Named("void".to_string()),
            is_test: true,
            is_main: false,
            tests_target: Some(target),
        }
    }
}

/// Detailed information about a type symbol.
#[derive(Clone, Debug)]
pub struct TypeSymbol {
    /// Type parameters (generics)
    pub type_params: Vec<String>,
    /// The kind of type definition
    pub kind: TypeDefKind,
}

/// Kind of type definition.
#[derive(Clone, Debug)]
pub enum TypeDefKind {
    /// A struct type
    Struct { fields: Vec<(String, TypeExpr)> },
    /// An enum type
    Enum { variants: Vec<EnumVariant> },
    /// A type alias
    Alias { target: TypeExpr },
}

/// An enum variant.
#[derive(Clone, Debug)]
pub struct EnumVariant {
    /// Variant name
    pub name: String,
    /// Variant fields (empty for unit variants)
    pub fields: Vec<(String, TypeExpr)>,
}

/// Detailed information about a config symbol.
#[derive(Clone, Debug)]
pub struct ConfigSymbol {
    /// The type of the config variable
    pub ty: TypeExpr,
}

/// Detailed information about a local variable symbol.
#[derive(Clone, Debug)]
pub struct LocalSymbol {
    /// The type of the local variable
    pub ty: TypeExpr,
    /// Whether the variable is mutable
    pub mutable: bool,
    /// The scope in which this local is defined
    pub scope: ScopeId,
}

/// Detailed information about a trait symbol.
#[derive(Clone, Debug)]
pub struct TraitSymbol {
    /// Type parameters
    pub type_params: Vec<String>,
    /// Required methods
    pub methods: Vec<TraitMethod>,
    /// Associated types
    pub associated_types: Vec<String>,
    /// Supertraits
    pub supertraits: Vec<String>,
}

/// A method signature in a trait.
#[derive(Clone, Debug)]
pub struct TraitMethod {
    /// Method name
    pub name: String,
    /// Parameter types (excluding self)
    pub param_types: Vec<TypeExpr>,
    /// Return type
    pub return_type: TypeExpr,
    /// Whether this method has a default implementation
    pub has_default: bool,
}

/// Detailed information about a type parameter symbol.
#[derive(Clone, Debug)]
pub struct TypeParamSymbol {
    /// Trait bounds on this type parameter
    pub bounds: Vec<String>,
}

/// The kind of entity a symbol represents, with detailed information.
#[derive(Clone, Debug)]
pub enum SymbolKind {
    /// A function definition
    Function(FunctionSymbol),
    /// A type definition
    Type(TypeSymbol),
    /// A config variable
    Config(ConfigSymbol),
    /// A local variable
    Local(LocalSymbol),
    /// A trait definition
    Trait(TraitSymbol),
    /// A type parameter
    TypeParam(TypeParamSymbol),
    /// A module
    Module,
}

impl SymbolKind {
    /// Get the kind name for display.
    pub fn kind_name(&self) -> &'static str {
        match self {
            SymbolKind::Function(_) => "function",
            SymbolKind::Type(_) => "type",
            SymbolKind::Config(_) => "config",
            SymbolKind::Local(_) => "variable",
            SymbolKind::Trait(_) => "trait",
            SymbolKind::TypeParam(_) => "type parameter",
            SymbolKind::Module => "module",
        }
    }

    /// Check if this is a function symbol.
    pub fn is_function(&self) -> bool {
        matches!(self, SymbolKind::Function(_))
    }

    /// Check if this is a type symbol.
    pub fn is_type(&self) -> bool {
        matches!(self, SymbolKind::Type(_))
    }

    /// Check if this is a local variable symbol.
    pub fn is_local(&self) -> bool {
        matches!(self, SymbolKind::Local(_))
    }

    /// Get the function symbol info, if this is a function.
    pub fn as_function(&self) -> Option<&FunctionSymbol> {
        match self {
            SymbolKind::Function(f) => Some(f),
            _ => None,
        }
    }

    /// Get the type symbol info, if this is a type.
    pub fn as_type(&self) -> Option<&TypeSymbol> {
        match self {
            SymbolKind::Type(t) => Some(t),
            _ => None,
        }
    }

    /// Get the local symbol info, if this is a local.
    pub fn as_local(&self) -> Option<&LocalSymbol> {
        match self {
            SymbolKind::Local(l) => Some(l),
            _ => None,
        }
    }
}

/// A symbol with name, path, and detailed kind information.
#[derive(Clone, Debug)]
pub struct Symbol {
    /// The simple name of the symbol
    pub name: String,
    /// The module path (empty for current module)
    pub module_path: Vec<String>,
    /// Detailed information about the symbol
    pub kind: SymbolKind,
    /// The scope containing this symbol
    pub scope: ScopeId,
    /// Source location (byte offset range)
    pub span: std::ops::Range<usize>,
    /// Whether this symbol is public
    pub is_public: bool,
}

impl Symbol {
    /// Create a new symbol in the current module.
    pub fn new(name: String, kind: SymbolKind) -> Self {
        Symbol {
            name,
            module_path: vec![],
            kind,
            scope: ScopeId::ROOT,
            span: 0..0,
            is_public: false,
        }
    }

    /// Create a symbol with a module path.
    pub fn with_path(name: String, module_path: Vec<String>, kind: SymbolKind) -> Self {
        Symbol {
            name,
            module_path,
            kind,
            scope: ScopeId::ROOT,
            span: 0..0,
            is_public: false,
        }
    }

    /// Set the scope.
    pub fn in_scope(mut self, scope: ScopeId) -> Self {
        self.scope = scope;
        self
    }

    /// Set the source span.
    pub fn at_span(mut self, span: std::ops::Range<usize>) -> Self {
        self.span = span;
        self
    }

    /// Set as public.
    pub fn public(mut self) -> Self {
        self.is_public = true;
        self
    }

    /// Get the fully qualified name.
    pub fn fully_qualified(&self) -> String {
        if self.module_path.is_empty() {
            self.name.clone()
        } else {
            format!("{}::{}", self.module_path.join("::"), self.name)
        }
    }

    /// Check if this is a local (unqualified) symbol.
    pub fn is_local_path(&self) -> bool {
        self.module_path.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int_type() -> TypeExpr {
        TypeExpr::Named("int".to_string())
    }

    fn void_type() -> TypeExpr {
        TypeExpr::Named("void".to_string())
    }

    #[test]
    fn test_function_symbol() {
        let func = FunctionSymbol::new(vec![], vec!["x".to_string()], vec![int_type()], int_type());

        assert!(!func.is_test);
        assert!(!func.is_main);
        assert_eq!(func.param_names.len(), 1);
    }

    #[test]
    fn test_test_function() {
        let test = FunctionSymbol::test("add".to_string());

        assert!(test.is_test);
        assert_eq!(test.tests_target, Some("add".to_string()));
    }

    #[test]
    fn test_symbol_kind_name() {
        let func_kind =
            SymbolKind::Function(FunctionSymbol::new(vec![], vec![], vec![], void_type()));
        assert_eq!(func_kind.kind_name(), "function");

        let local_kind = SymbolKind::Local(LocalSymbol {
            ty: int_type(),
            mutable: false,
            scope: ScopeId::ROOT,
        });
        assert_eq!(local_kind.kind_name(), "variable");
    }

    #[test]
    fn test_symbol_creation() {
        let sym = Symbol::new(
            "foo".to_string(),
            SymbolKind::Function(FunctionSymbol::new(vec![], vec![], vec![], void_type())),
        );

        assert_eq!(sym.name, "foo");
        assert!(sym.is_local_path());
        assert_eq!(sym.fully_qualified(), "foo");
    }

    #[test]
    fn test_symbol_with_path() {
        let type_sym = TypeSymbol {
            type_params: vec!["K".to_string(), "V".to_string()],
            kind: TypeDefKind::Struct { fields: vec![] },
        };

        let sym = Symbol::with_path(
            "HashMap".to_string(),
            vec!["std".to_string(), "collections".to_string()],
            SymbolKind::Type(type_sym),
        );

        assert_eq!(sym.fully_qualified(), "std::collections::HashMap");
    }

    #[test]
    fn test_symbol_builder() {
        let sym = Symbol::new(
            "x".to_string(),
            SymbolKind::Local(LocalSymbol {
                ty: int_type(),
                mutable: true,
                scope: ScopeId::ROOT,
            }),
        )
        .in_scope(ScopeId::new(1))
        .at_span(10..15)
        .public();

        assert_eq!(sym.scope, ScopeId::new(1));
        assert_eq!(sym.span, 10..15);
        assert!(sym.is_public);
    }
}
