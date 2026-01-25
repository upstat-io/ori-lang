//! Flat AST types using arena allocation.
//!
//! Per design spec A-data-structures.md:
//! - No Box<Expr>, use ExprId(u32) indices
//! - Contiguous arrays for cache locality
//! - All types have Salsa-required traits (Clone, Eq, Hash, Debug)
//!
//! # Module Structure
//!
//! - `expr`: Core expression types (Expr, ExprKind)
//! - `operators`: Binary and unary operators
//! - `stmt`: Statement types
//! - `ranges`: Arena range types for efficient iteration
//! - `collections`: Map entries, field initializers, call arguments
//! - `patterns/`: Binding patterns, match patterns, FunctionSeq, FunctionExp
//! - `items/`: Module-level items (Function, TestDef, imports, traits)

mod expr;
mod operators;
mod stmt;
mod ranges;
mod collections;
pub mod patterns;
pub mod items;

// Re-export core expression types
pub use expr::{Expr, ExprKind};
pub use operators::{BinaryOp, UnaryOp};
pub use stmt::{Stmt, StmtKind};

// Re-export all range types
pub use ranges::{
    ParamRange, GenericParamRange, ArmRange, MapEntryRange,
    FieldInitRange, SeqBindingRange, NamedExprRange, CallArgRange,
};

// Re-export collection types
pub use collections::{MapEntry, FieldInit, CallArg};

// Re-export pattern types
pub use patterns::{
    BindingPattern, MatchPattern, MatchArm,
    SeqBinding, FunctionSeq,
    NamedExpr, FunctionExpKind, FunctionExp,
};

// Re-export item types
pub use items::{
    Param, Function, TestDef, Module,
    UseDef, UseItem, ImportPath,
    GenericParam, TraitBound, WhereClause,
    TraitDef, TraitItem, TraitMethodSig, TraitDefaultMethod, TraitAssocType,
    ImplDef, ImplMethod, ExtendDef,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Span;

    #[test]
    fn test_expr_kind_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        set.insert(ExprKind::Int(42));
        set.insert(ExprKind::Int(42));
        set.insert(ExprKind::Int(43));
        set.insert(ExprKind::Bool(true));

        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_binary_op() {
        let op = BinaryOp::Add;
        assert_eq!(op, BinaryOp::Add);
        assert_ne!(op, BinaryOp::Sub);
    }

    #[test]
    fn test_expr_spanned() {
        use crate::ir::Spanned;
        let expr = Expr::new(ExprKind::Int(42), Span::new(0, 2));
        assert_eq!(expr.span().start, 0);
        assert_eq!(expr.span().end, 2);
    }

    #[test]
    fn test_module_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        let m1 = Module::new();
        let m2 = Module::new();

        set.insert(m1);
        set.insert(m2);

        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_function_exp_kind() {
        // Note: Map, Filter, Fold, etc. are now stdlib methods
        assert_eq!(FunctionExpKind::Parallel, FunctionExpKind::Parallel);
        assert_ne!(FunctionExpKind::Parallel, FunctionExpKind::Spawn);
        assert_eq!(FunctionExpKind::Parallel.name(), "parallel");
    }
}
