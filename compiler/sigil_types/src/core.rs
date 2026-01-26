//! Core type definitions.
//!
//! Foundational types for the Sigil type system.

use sigil_ir::{Name, StringInterner};

/// Concrete type representation.
///
/// # Salsa Compatibility
/// Has Clone, Eq, Hash for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Type {
    // ===== Primitives =====
    /// Integer type
    Int,
    /// Floating point type
    Float,
    /// Boolean type
    Bool,
    /// String type
    Str,
    /// Character type
    Char,
    /// Byte type
    Byte,
    /// Unit type ()
    Unit,
    /// Never type (diverging)
    Never,

    // ===== Special types =====
    /// Duration type (30s, 100ms)
    Duration,
    /// Size type (4kb, 10mb)
    Size,

    // ===== Compound types =====
    /// Function type: (params) -> return
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
    },

    /// Tuple type: (T, U, V)
    Tuple(Vec<Type>),

    /// List type: [T]
    List(Box<Type>),

    /// Map type: {K: V}
    Map {
        key: Box<Type>,
        value: Box<Type>,
    },

    /// Set type: Set<T>
    Set(Box<Type>),

    /// Option type: Option<T>
    Option(Box<Type>),

    /// Result type: Result<T, E>
    Result {
        ok: Box<Type>,
        err: Box<Type>,
    },

    /// Range type: Range<T>
    Range(Box<Type>),

    /// Channel type: Channel<T>
    Channel(Box<Type>),

    // ===== Named types =====
    /// User-defined type reference (non-generic or unapplied generic)
    Named(Name),

    /// Applied generic type: the base type name with concrete type arguments.
    /// For example, `Box<int>` is `Applied { name: "Box", args: [Int] }`.
    Applied {
        name: Name,
        args: Vec<Type>,
    },

    /// Generic type variable (for inference)
    Var(TypeVar),

    /// Error type (for error recovery)
    Error,

    /// Associated type projection (e.g., `Self.Item`, `T.Item`).
    /// Represents accessing an associated type on a base type.
    Projection {
        /// The base type (e.g., `Self`, or a type variable).
        base: Box<Type>,
        /// The trait that defines the associated type.
        trait_name: Name,
        /// The associated type name (e.g., `Item`).
        assoc_name: Name,
    },
}

impl Type {
    /// Check if this is a primitive type.
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            Type::Int | Type::Float | Type::Bool | Type::Str |
            Type::Char | Type::Byte | Type::Unit | Type::Never
        )
    }

    /// Check if this is the error type.
    pub fn is_error(&self) -> bool {
        matches!(self, Type::Error)
    }

    /// Check if this is a type variable.
    pub fn is_var(&self) -> bool {
        matches!(self, Type::Var(_))
    }

    /// Get inner type for Option, List, etc.
    pub fn inner(&self) -> Option<&Type> {
        match self {
            Type::List(t) | Type::Option(t) | Type::Set(t) |
            Type::Range(t) | Type::Channel(t) => Some(t),
            _ => None,
        }
    }

    /// Format type for display.
    pub fn display(&self, interner: &StringInterner) -> String {
        match self {
            Type::Int => "int".to_string(),
            Type::Float => "float".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Str => "str".to_string(),
            Type::Char => "char".to_string(),
            Type::Byte => "byte".to_string(),
            Type::Unit => "()".to_string(),
            Type::Never => "Never".to_string(),
            Type::Duration => "Duration".to_string(),
            Type::Size => "Size".to_string(),
            Type::Function { params, ret } => {
                let params_str: Vec<_> = params.iter()
                    .map(|p| p.display(interner))
                    .collect();
                format!("({}) -> {}", params_str.join(", "), ret.display(interner))
            }
            Type::Tuple(types) => {
                let types_str: Vec<_> = types.iter()
                    .map(|t| t.display(interner))
                    .collect();
                format!("({})", types_str.join(", "))
            }
            Type::List(t) => format!("[{}]", t.display(interner)),
            Type::Map { key, value } => {
                format!("{{{}: {}}}", key.display(interner), value.display(interner))
            }
            Type::Set(t) => format!("Set<{}>", t.display(interner)),
            Type::Option(t) => format!("Option<{}>", t.display(interner)),
            Type::Result { ok, err } => {
                format!("Result<{}, {}>", ok.display(interner), err.display(interner))
            }
            Type::Range(t) => format!("Range<{}>", t.display(interner)),
            Type::Channel(t) => format!("Channel<{}>", t.display(interner)),
            Type::Named(name) => interner.lookup(*name).to_string(),
            Type::Applied { name, args } => {
                let args_str: Vec<_> = args.iter()
                    .map(|a| a.display(interner))
                    .collect();
                format!("{}<{}>", interner.lookup(*name), args_str.join(", "))
            }
            Type::Var(v) => format!("?{}", v.0),
            Type::Error => "<error>".to_string(),
            Type::Projection { base, trait_name: _, assoc_name } => {
                format!("{}.{}", base.display(interner), interner.lookup(*assoc_name))
            }
        }
    }
}

/// Type variable for inference.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeVar(pub u32);

impl TypeVar {
    pub fn new(id: u32) -> Self {
        TypeVar(id)
    }
}

/// A type scheme (polymorphic type) with quantified type variables.
///
/// For example, the identity function `fn id<T>(x: T) -> T` has type scheme:
/// `TypeScheme { vars: [T], ty: T -> T }`
///
/// When used, we instantiate fresh type variables for each quantified variable.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeScheme {
    /// Quantified type variables (∀ these variables)
    pub vars: Vec<TypeVar>,
    /// The type with potentially free type variables
    pub ty: Type,
}

impl TypeScheme {
    /// Create a monomorphic scheme (no quantified variables).
    pub fn mono(ty: Type) -> Self {
        TypeScheme {
            vars: Vec::new(),
            ty,
        }
    }

    /// Create a polymorphic scheme with the given quantified variables.
    pub fn poly(vars: Vec<TypeVar>, ty: Type) -> Self {
        TypeScheme { vars, ty }
    }

    /// Check if this is a monomorphic type (no quantified variables).
    pub fn is_mono(&self) -> bool {
        self.vars.is_empty()
    }
}
