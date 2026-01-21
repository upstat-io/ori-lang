// Typed expressions for Sigil TIR
// Every expression carries its resolved type

use super::patterns::TPattern;
use super::types::Type;
use crate::ast::{BinaryOp, Span, UnaryOp};

/// Local variable ID (index into function's local table)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

impl LocalId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Function reference
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FuncRef {
    /// User-defined function
    User(String),
    /// Built-in function
    Builtin(String),
    /// Operator as function (for fold/map/etc)
    Operator(BinaryOp),
}

/// Every expression carries its type
#[derive(Debug, Clone)]
pub struct TExpr {
    pub kind: TExprKind,
    pub ty: Type,
    pub span: Span,
}

impl TExpr {
    pub fn new(kind: TExprKind, ty: Type, span: Span) -> Self {
        TExpr { kind, ty, span }
    }

    /// Create a simple int literal
    pub fn int(value: i64, span: Span) -> Self {
        TExpr::new(TExprKind::Int(value), Type::Int, span)
    }

    /// Create a simple bool literal
    pub fn bool(value: bool, span: Span) -> Self {
        TExpr::new(TExprKind::Bool(value), Type::Bool, span)
    }

    /// Create a nil literal
    pub fn nil(span: Span) -> Self {
        TExpr::new(TExprKind::Nil, Type::Void, span)
    }
}

/// Typed expression kinds
#[derive(Debug, Clone)]
pub enum TExprKind {
    // Literals
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Nil,

    // Variables (resolved to IDs or strings)
    Local(LocalId),
    Param(usize), // Parameter index
    Config(String),

    // Collections
    List(Vec<TExpr>),
    MapLiteral(Vec<(TExpr, TExpr)>),
    Tuple(Vec<TExpr>),
    Struct {
        name: String,
        fields: Vec<(String, TExpr)>,
    },

    // Operations
    Binary {
        op: BinaryOp,
        left: Box<TExpr>,
        right: Box<TExpr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<TExpr>,
    },

    // Access
    Field(Box<TExpr>, String),
    Index(Box<TExpr>, Box<TExpr>),
    /// Length placeholder: # inside array index (resolved to length of collection)
    LengthOf(Box<TExpr>),

    // Calls
    Call {
        func: FuncRef,
        args: Vec<TExpr>,
    },
    MethodCall {
        receiver: Box<TExpr>,
        method: String,
        args: Vec<TExpr>,
    },

    // Lambdas (with typed params and captures)
    Lambda {
        params: Vec<(String, Type)>,
        captures: Vec<LocalId>,
        body: Box<TExpr>,
    },

    // Control flow
    If {
        cond: Box<TExpr>,
        then_branch: Box<TExpr>,
        else_branch: Box<TExpr>,
    },
    Match(Box<TMatch>),
    Block(Vec<TStmt>, Box<TExpr>),
    For {
        binding: LocalId,
        iter: Box<TExpr>,
        body: Box<TExpr>,
    },

    // Assignment (in blocks)
    Assign {
        target: LocalId,
        value: Box<TExpr>,
    },

    // Range
    Range {
        start: Box<TExpr>,
        end: Box<TExpr>,
    },

    // Patterns (high-level, lowered later by passes)
    Pattern(Box<TPattern>),

    // Result/Option constructors
    Ok(Box<TExpr>),
    Err(Box<TExpr>),
    Some(Box<TExpr>),
    None_,
    Coalesce {
        value: Box<TExpr>,
        default: Box<TExpr>,
    },
    Unwrap(Box<TExpr>),
}

/// Typed statement (in blocks)
#[derive(Debug, Clone)]
pub enum TStmt {
    /// Expression statement
    Expr(TExpr),
    /// Let binding: name := value
    Let {
        local: LocalId,
        value: TExpr,
    },
}

/// Typed match expression
#[derive(Debug, Clone)]
pub struct TMatch {
    pub scrutinee: TExpr,
    pub scrutinee_ty: Type,
    pub arms: Vec<TMatchArm>,
}

/// Typed match arm
#[derive(Debug, Clone)]
pub struct TMatchArm {
    pub pattern: TMatchPattern,
    pub body: TExpr,
}

/// Typed match pattern
#[derive(Debug, Clone)]
pub enum TMatchPattern {
    /// Wildcard: _
    Wildcard,

    /// Literal: 5, "hello", true
    Literal(TExpr),

    /// Binding: x (captures scrutinee)
    Binding(LocalId, Type),

    /// Variant: Ok { value }, Err { error }
    Variant {
        name: String,
        bindings: Vec<(String, LocalId, Type)>,
    },

    /// Condition: if expr
    Condition(TExpr),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_id() {
        let id = LocalId(5);
        assert_eq!(id.index(), 5);
    }

    #[test]
    fn test_texpr_constructors() {
        let span = 0..1;
        let int_expr = TExpr::int(42, span.clone());
        assert_eq!(int_expr.ty, Type::Int);
        assert!(matches!(int_expr.kind, TExprKind::Int(42)));
    }
}
