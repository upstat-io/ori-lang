// Module and function definitions for Sigil TIR
// Contains TModule, TFunction, LocalTable, and related types

use super::expr::TExpr;
use super::types::Type;
use crate::ast::Span;

/// A typed Sigil module
#[derive(Debug, Clone)]
pub struct TModule {
    pub name: String,
    pub types: Vec<TTypeDef>,
    pub configs: Vec<TConfig>,
    pub functions: Vec<TFunction>,
    pub tests: Vec<TTest>,
    pub imports: Vec<TImport>,
}

impl TModule {
    pub fn new(name: String) -> Self {
        TModule {
            name,
            types: Vec::new(),
            configs: Vec::new(),
            functions: Vec::new(),
            tests: Vec::new(),
            imports: Vec::new(),
        }
    }

    /// Find a function by name
    pub fn find_function(&self, name: &str) -> Option<&TFunction> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// Find the main function
    pub fn find_main(&self) -> Option<&TFunction> {
        self.find_function("main")
    }

    /// Check if this module has a main function
    pub fn has_main(&self) -> bool {
        self.find_main().is_some()
    }
}

/// Typed type definition
#[derive(Debug, Clone)]
pub struct TTypeDef {
    pub name: String,
    pub public: bool,
    pub params: Vec<String>, // Generic type parameters
    pub kind: TTypeDefKind,
    pub span: Span,
}

/// Kind of type definition
#[derive(Debug, Clone)]
pub enum TTypeDefKind {
    /// Type alias: type UserId = str
    Alias(Type),

    /// Struct: type User { id: UserId, name: str }
    Struct(Vec<TField>),

    /// Enum: type Error = NotFound | Invalid { msg: str }
    Enum(Vec<TVariant>),
}

/// Struct field definition
#[derive(Debug, Clone)]
pub struct TField {
    pub name: String,
    pub ty: Type,
}

/// Enum variant definition
#[derive(Debug, Clone)]
pub struct TVariant {
    pub name: String,
    pub fields: Vec<TField>, // Empty for unit variants
}

/// Typed config variable
#[derive(Debug, Clone)]
pub struct TConfig {
    pub name: String,
    pub ty: Type,
    pub value: TExpr,
    pub span: Span,
}

/// Typed function definition
#[derive(Debug, Clone)]
pub struct TFunction {
    pub name: String,
    pub public: bool,
    pub params: Vec<TParam>,
    pub return_type: Type,
    pub locals: LocalTable, // All locals in function (including params)
    pub body: TExpr,
    pub span: Span,
}

impl TFunction {
    /// Check if this function takes no arguments (nullary)
    pub fn is_nullary(&self) -> bool {
        self.params.is_empty()
    }

    /// Get the function signature as a Type
    pub fn signature(&self) -> Type {
        Type::Function {
            params: self.params.iter().map(|p| p.ty.clone()).collect(),
            ret: Box::new(self.return_type.clone()),
        }
    }
}

/// Function parameter
#[derive(Debug, Clone)]
pub struct TParam {
    pub name: String,
    pub ty: Type,
}

/// Local variable table for a function
#[derive(Debug, Clone, Default)]
pub struct LocalTable {
    pub entries: Vec<LocalInfo>,
}

impl LocalTable {
    pub fn new() -> Self {
        LocalTable {
            entries: Vec::new(),
        }
    }

    /// Add a new local variable and return its ID
    pub fn add(
        &mut self,
        name: String,
        ty: Type,
        is_param: bool,
        mutable: bool,
    ) -> super::expr::LocalId {
        let id = super::expr::LocalId(self.entries.len() as u32);
        self.entries.push(LocalInfo {
            name,
            ty,
            is_param,
            mutable,
        });
        id
    }

    /// Get local info by ID
    pub fn get(&self, id: super::expr::LocalId) -> Option<&LocalInfo> {
        self.entries.get(id.index())
    }

    /// Get a local by name
    pub fn find(&self, name: &str) -> Option<(super::expr::LocalId, &LocalInfo)> {
        self.entries
            .iter()
            .enumerate()
            .find(|(_, info)| info.name == name)
            .map(|(i, info)| (super::expr::LocalId(i as u32), info))
    }

    /// Number of locals
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterator over all locals
    pub fn iter(&self) -> impl Iterator<Item = (super::expr::LocalId, &LocalInfo)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, info)| (super::expr::LocalId(i as u32), info))
    }
}

/// Information about a local variable
#[derive(Debug, Clone)]
pub struct LocalInfo {
    pub name: String,
    pub ty: Type,
    pub is_param: bool,
    pub mutable: bool,
}

/// Typed test definition
#[derive(Debug, Clone)]
pub struct TTest {
    pub name: String,
    pub target: String, // The function being tested
    pub locals: LocalTable,
    pub body: TExpr,
    pub span: Span,
}

/// Typed import definition
#[derive(Debug, Clone)]
pub struct TImport {
    pub path: Vec<String>,
    pub items: Vec<TImportItem>,
    pub span: Span,
}

/// Individual import item
#[derive(Debug, Clone)]
pub struct TImportItem {
    pub name: String,
    pub alias: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_table() {
        let mut table = LocalTable::new();
        let id1 = table.add("x".to_string(), Type::Int, false, false);
        let id2 = table.add("y".to_string(), Type::Bool, false, false);

        assert_eq!(id1.index(), 0);
        assert_eq!(id2.index(), 1);
        assert_eq!(table.len(), 2);

        let info = table.get(id1).unwrap();
        assert_eq!(info.name, "x");
        assert_eq!(info.ty, Type::Int);
    }

    #[test]
    fn test_tfunction_signature() {
        let func = TFunction {
            name: "add".to_string(),
            public: false,
            params: vec![
                TParam {
                    name: "a".to_string(),
                    ty: Type::Int,
                },
                TParam {
                    name: "b".to_string(),
                    ty: Type::Int,
                },
            ],
            return_type: Type::Int,
            locals: LocalTable::new(),
            body: TExpr::int(0, 0..1),
            span: 0..1,
        };

        let sig = func.signature();
        assert!(
            matches!(sig, Type::Function { params, ret } if params.len() == 2 && *ret == Type::Int)
        );
    }
}
