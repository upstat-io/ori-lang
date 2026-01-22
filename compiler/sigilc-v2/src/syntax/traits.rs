//! Common traits for AST nodes.
//!
//! These traits provide uniform access to common properties
//! across different AST node types.

use crate::intern::Name;
use super::Span;

/// Trait for AST nodes that have a source location span.
///
/// This enables generic functions that work with any spanned item:
/// ```ignore
/// fn report_error(node: &impl Spanned, message: &str) {
///     println!("Error at {}: {}", node.span(), message);
/// }
/// ```
pub trait Spanned {
    /// Returns the source location span of this node.
    fn span(&self) -> Span;
}

/// Trait for AST nodes that have a name.
///
/// This enables generic functions that work with any named item:
/// ```ignore
/// fn lookup<T: Named>(items: &[T], name: Name) -> Option<&T> {
///     items.iter().find(|item| item.name() == name)
/// }
/// ```
pub trait Named {
    /// Returns the name of this node.
    fn name(&self) -> Name;
}

// ===== Implementations for syntax types =====

use super::{
    Expr, Item, Token,
    items::{
        Function, TypeDef, Config, Test, Trait,
        StructField, EnumVariant, ImportItem, TypeParam, WhereClause,
        TraitItem, ImplItem,
    },
    expr::{Param, MapEntry, PatternArg, PatternArgs, MatchArm, Stmt, TypeExpr},
};

// ----- Spanned implementations -----

impl Spanned for Expr {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for Item {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for Token {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for StructField {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for EnumVariant {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for ImportItem {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for TypeParam {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for WhereClause {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for Param {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for MapEntry {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for PatternArg {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for PatternArgs {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for MatchArm {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for Stmt {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for TypeExpr {
    fn span(&self) -> Span {
        self.span
    }
}

impl Spanned for TraitItem {
    fn span(&self) -> Span {
        match self {
            TraitItem::Method { span, .. } => *span,
            TraitItem::AssociatedType { span, .. } => *span,
        }
    }
}

impl Spanned for ImplItem {
    fn span(&self) -> Span {
        match self {
            ImplItem::Method { span, .. } => *span,
            ImplItem::AssociatedType { span, .. } => *span,
        }
    }
}

// ----- Named implementations -----

impl Named for Function {
    fn name(&self) -> Name {
        self.name
    }
}

impl Named for TypeDef {
    fn name(&self) -> Name {
        self.name
    }
}

impl Named for Config {
    fn name(&self) -> Name {
        self.name
    }
}

impl Named for Test {
    fn name(&self) -> Name {
        self.name
    }
}

impl Named for Trait {
    fn name(&self) -> Name {
        self.name
    }
}

impl Named for StructField {
    fn name(&self) -> Name {
        self.name
    }
}

impl Named for EnumVariant {
    fn name(&self) -> Name {
        self.name
    }
}

impl Named for ImportItem {
    fn name(&self) -> Name {
        self.name
    }
}

impl Named for TypeParam {
    fn name(&self) -> Name {
        self.name
    }
}

impl Named for Param {
    fn name(&self) -> Name {
        self.name
    }
}

impl Named for PatternArg {
    fn name(&self) -> Name {
        self.name
    }
}

impl Named for TraitItem {
    fn name(&self) -> Name {
        match self {
            TraitItem::Method { name, .. } => *name,
            TraitItem::AssociatedType { name, .. } => *name,
        }
    }
}

impl Named for ImplItem {
    fn name(&self) -> Name {
        match self {
            ImplItem::Method { name, .. } => *name,
            ImplItem::AssociatedType { name, .. } => *name,
        }
    }
}

// ===== Utility functions using traits =====

/// Find a named item in a slice.
pub fn find_by_name<'a, T: Named>(items: &'a [T], name: Name) -> Option<&'a T> {
    items.iter().find(|item| item.name() == name)
}

/// Merge spans of two items.
pub fn merge_spans<S1: Spanned, S2: Spanned>(start: &S1, end: &S2) -> Span {
    start.span().merge(end.span())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intern::StringInterner;

    #[test]
    fn test_spanned_expr() {
        use crate::syntax::{Expr, ExprKind, Span};

        let expr = Expr::new(ExprKind::Int(42), Span::new(0, 2));
        assert_eq!(expr.span().start, 0);
        assert_eq!(expr.span().end, 2);
    }

    #[test]
    fn test_named_function() {
        use crate::syntax::items::{Function, Visibility};
        use crate::syntax::{ExprId, ParamRange, Span};

        let interner = StringInterner::new();
        let name = interner.intern("my_func");

        let func = Function {
            name,
            visibility: Visibility::Private,
            type_params: vec![],
            params: ParamRange::EMPTY,
            return_type: None,
            capabilities: vec![],
            body: ExprId::PLACEHOLDER,
            is_async: false,
            sig_span: Span::new(0, 10),
        };

        assert_eq!(func.name(), name);
    }

    #[test]
    fn test_find_by_name() {
        use crate::syntax::items::{Function, Visibility};
        use crate::syntax::{ExprId, ParamRange, Span};

        let interner = StringInterner::new();
        let name1 = interner.intern("func1");
        let name2 = interner.intern("func2");

        let funcs = vec![
            Function {
                name: name1,
                visibility: Visibility::Private,
                type_params: vec![],
                params: ParamRange::EMPTY,
                return_type: None,
                capabilities: vec![],
                body: ExprId::PLACEHOLDER,
                is_async: false,
                sig_span: Span::new(0, 10),
            },
            Function {
                name: name2,
                visibility: Visibility::Public,
                type_params: vec![],
                params: ParamRange::EMPTY,
                return_type: None,
                capabilities: vec![],
                body: ExprId::PLACEHOLDER,
                is_async: false,
                sig_span: Span::new(20, 30),
            },
        ];

        assert!(find_by_name(&funcs, name1).is_some());
        assert!(find_by_name(&funcs, name2).is_some());
        assert!(find_by_name(&funcs, interner.intern("nonexistent")).is_none());
    }
}
