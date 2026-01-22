//! Expression arena for contiguous AST storage.
//!
//! All expressions in a module are stored contiguously in the arena,
//! with children referenced by index. This provides:
//! - Cache locality during traversal
//! - Reduced memory fragmentation
//! - Trivially copyable references (ExprId)

use super::{
    Expr, ExprId, ExprRange, StmtRange, ArmRange, ParamRange,
    MapEntryRange, FieldInitRange, PatternArgsId, TypeExprId,
    expr::{Stmt, MatchArm, MapEntry, FieldInit, Param, PatternArgs, TypeExpr},
};

/// Contiguous storage for all expressions in a module.
pub struct ExprArena {
    /// All expressions.
    exprs: Vec<Expr>,

    /// Flattened expression lists (for Call args, List elements, etc.).
    expr_lists: Vec<ExprId>,

    /// Match arms.
    arms: Vec<MatchArm>,

    /// Statements.
    stmts: Vec<Stmt>,

    /// Pattern arguments.
    pattern_args: Vec<PatternArgs>,

    /// Map entries.
    map_entries: Vec<MapEntry>,

    /// Field initializers.
    field_inits: Vec<FieldInit>,

    /// Parameters.
    params: Vec<Param>,

    /// Type expressions.
    type_exprs: Vec<TypeExpr>,
}

impl ExprArena {
    /// Create a new arena with default capacity hints.
    pub fn new() -> Self {
        Self::with_capacity(4096)
    }

    /// Create an arena with specified initial expression capacity.
    pub fn with_capacity(expr_capacity: usize) -> Self {
        Self {
            exprs: Vec::with_capacity(expr_capacity),
            expr_lists: Vec::with_capacity(expr_capacity / 2),
            arms: Vec::with_capacity(256),
            stmts: Vec::with_capacity(1024),
            pattern_args: Vec::with_capacity(512),
            map_entries: Vec::with_capacity(256),
            field_inits: Vec::with_capacity(256),
            params: Vec::with_capacity(256),
            type_exprs: Vec::with_capacity(512),
        }
    }

    // ===== Expression allocation =====

    /// Allocate an expression, returning its ID.
    #[inline]
    pub fn alloc(&mut self, expr: Expr) -> ExprId {
        let id = ExprId::new(self.exprs.len() as u32);
        self.exprs.push(expr);
        id
    }

    /// Get an expression by ID.
    #[inline]
    pub fn get(&self, id: ExprId) -> &Expr {
        debug_assert!(id.is_valid(), "Attempted to get invalid ExprId");
        &self.exprs[id.index()]
    }

    /// Get a mutable expression by ID.
    #[inline]
    pub fn get_mut(&mut self, id: ExprId) -> &mut Expr {
        debug_assert!(id.is_valid(), "Attempted to get invalid ExprId");
        &mut self.exprs[id.index()]
    }

    /// Number of expressions in the arena.
    pub fn len(&self) -> usize {
        self.exprs.len()
    }

    /// Check if arena is empty.
    pub fn is_empty(&self) -> bool {
        self.exprs.is_empty()
    }

    // ===== Expression list allocation =====

    /// Allocate an expression list, returning a range.
    pub fn alloc_expr_list(&mut self, exprs: impl IntoIterator<Item = ExprId>) -> ExprRange {
        let start = self.expr_lists.len() as u32;
        self.expr_lists.extend(exprs);
        let len = (self.expr_lists.len() as u32 - start) as u16;
        ExprRange::new(start, len)
    }

    /// Get expression IDs from a range.
    #[inline]
    pub fn get_expr_list(&self, range: ExprRange) -> &[ExprId] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.expr_lists[start..end]
    }

    // ===== Match arm allocation =====

    /// Allocate match arms, returning a range.
    pub fn alloc_arms(&mut self, arms: impl IntoIterator<Item = MatchArm>) -> ArmRange {
        let start = self.arms.len() as u32;
        self.arms.extend(arms);
        let len = (self.arms.len() as u32 - start) as u16;
        ArmRange::new(start, len)
    }

    /// Get match arms from a range.
    #[inline]
    pub fn get_arms(&self, range: ArmRange) -> &[MatchArm] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.arms[start..end]
    }

    // ===== Statement allocation =====

    /// Allocate statements, returning a range.
    pub fn alloc_stmts(&mut self, stmts: impl IntoIterator<Item = Stmt>) -> StmtRange {
        let start = self.stmts.len() as u32;
        self.stmts.extend(stmts);
        let len = (self.stmts.len() as u32 - start) as u16;
        StmtRange::new(start, len)
    }

    /// Get statements from a range.
    #[inline]
    pub fn get_stmts(&self, range: StmtRange) -> &[Stmt] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.stmts[start..end]
    }

    // ===== Pattern args allocation =====

    /// Allocate pattern arguments, returning an ID.
    pub fn alloc_pattern_args(&mut self, args: PatternArgs) -> PatternArgsId {
        let id = PatternArgsId::new(self.pattern_args.len() as u32);
        self.pattern_args.push(args);
        id
    }

    /// Get pattern arguments by ID.
    #[inline]
    pub fn get_pattern_args(&self, id: PatternArgsId) -> &PatternArgs {
        debug_assert!(id.is_valid(), "Attempted to get invalid PatternArgsId");
        &self.pattern_args[id.index()]
    }

    // ===== Map entry allocation =====

    /// Allocate map entries, returning a range.
    pub fn alloc_map_entries(&mut self, entries: impl IntoIterator<Item = MapEntry>) -> MapEntryRange {
        let start = self.map_entries.len() as u32;
        self.map_entries.extend(entries);
        let len = (self.map_entries.len() as u32 - start) as u16;
        MapEntryRange::new(start, len)
    }

    /// Get map entries from a range.
    #[inline]
    pub fn get_map_entries(&self, range: MapEntryRange) -> &[MapEntry] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.map_entries[start..end]
    }

    // ===== Field init allocation =====

    /// Allocate field initializers, returning a range.
    pub fn alloc_field_inits(&mut self, inits: impl IntoIterator<Item = FieldInit>) -> FieldInitRange {
        let start = self.field_inits.len() as u32;
        self.field_inits.extend(inits);
        let len = (self.field_inits.len() as u32 - start) as u16;
        FieldInitRange::new(start, len)
    }

    /// Get field initializers from a range.
    #[inline]
    pub fn get_field_inits(&self, range: FieldInitRange) -> &[FieldInit] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.field_inits[start..end]
    }

    // ===== Parameter allocation =====

    /// Allocate parameters, returning a range.
    pub fn alloc_params(&mut self, params: impl IntoIterator<Item = Param>) -> ParamRange {
        let start = self.params.len() as u32;
        self.params.extend(params);
        let len = (self.params.len() as u32 - start) as u16;
        ParamRange::new(start, len)
    }

    /// Get parameters from a range.
    #[inline]
    pub fn get_params(&self, range: ParamRange) -> &[Param] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.params[start..end]
    }

    // ===== Type expression allocation =====

    /// Allocate a type expression, returning an ID.
    pub fn alloc_type_expr(&mut self, ty: TypeExpr) -> TypeExprId {
        let id = TypeExprId::new(self.type_exprs.len() as u32);
        self.type_exprs.push(ty);
        id
    }

    /// Get a type expression by ID.
    #[inline]
    pub fn get_type_expr(&self, id: TypeExprId) -> &TypeExpr {
        debug_assert!(id.is_valid(), "Attempted to get invalid TypeExprId");
        &self.type_exprs[id.index()]
    }

    // ===== Utility =====

    /// Reset the arena for reuse, keeping allocated capacity.
    pub fn reset(&mut self) {
        self.exprs.clear();
        self.expr_lists.clear();
        self.arms.clear();
        self.stmts.clear();
        self.pattern_args.clear();
        self.map_entries.clear();
        self.field_inits.clear();
        self.params.clear();
        self.type_exprs.clear();
    }

    /// Get memory statistics for debugging.
    pub fn stats(&self) -> ArenaStats {
        ArenaStats {
            exprs: self.exprs.len(),
            exprs_capacity: self.exprs.capacity(),
            expr_lists: self.expr_lists.len(),
            arms: self.arms.len(),
            stmts: self.stmts.len(),
            pattern_args: self.pattern_args.len(),
            map_entries: self.map_entries.len(),
            field_inits: self.field_inits.len(),
            params: self.params.len(),
            type_exprs: self.type_exprs.len(),
        }
    }
}

impl Default for ExprArena {
    fn default() -> Self {
        Self::new()
    }
}

/// Arena memory statistics.
#[derive(Clone, Debug)]
pub struct ArenaStats {
    pub exprs: usize,
    pub exprs_capacity: usize,
    pub expr_lists: usize,
    pub arms: usize,
    pub stmts: usize,
    pub pattern_args: usize,
    pub map_entries: usize,
    pub field_inits: usize,
    pub params: usize,
    pub type_exprs: usize,
}

impl ArenaStats {
    /// Estimate total memory usage in bytes.
    pub fn estimated_bytes(&self) -> usize {
        use std::mem::size_of;
        self.exprs_capacity * size_of::<Expr>()
            + self.expr_lists * size_of::<ExprId>()
            + self.arms * size_of::<MatchArm>()
            + self.stmts * size_of::<Stmt>()
            + self.pattern_args * size_of::<PatternArgs>()
            + self.map_entries * size_of::<MapEntry>()
            + self.field_inits * size_of::<FieldInit>()
            + self.params * size_of::<Param>()
            + self.type_exprs * size_of::<TypeExpr>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{ExprKind, Span};

    #[test]
    fn test_alloc_and_get() {
        let mut arena = ExprArena::new();

        let id1 = arena.alloc(Expr::new(ExprKind::Int(42), Span::new(0, 2)));
        let id2 = arena.alloc(Expr::new(ExprKind::Bool(true), Span::new(3, 7)));

        assert_eq!(arena.len(), 2);
        assert!(matches!(arena.get(id1).kind, ExprKind::Int(42)));
        assert!(matches!(arena.get(id2).kind, ExprKind::Bool(true)));
    }

    #[test]
    fn test_expr_list() {
        let mut arena = ExprArena::new();

        let e1 = arena.alloc(Expr::new(ExprKind::Int(1), Span::DUMMY));
        let e2 = arena.alloc(Expr::new(ExprKind::Int(2), Span::DUMMY));
        let e3 = arena.alloc(Expr::new(ExprKind::Int(3), Span::DUMMY));

        let range = arena.alloc_expr_list([e1, e2, e3]);

        assert_eq!(range.len, 3);
        let list = arena.get_expr_list(range);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], e1);
        assert_eq!(list[1], e2);
        assert_eq!(list[2], e3);
    }

    #[test]
    fn test_reset() {
        let mut arena = ExprArena::new();

        arena.alloc(Expr::new(ExprKind::Int(42), Span::DUMMY));
        arena.alloc(Expr::new(ExprKind::Bool(true), Span::DUMMY));

        assert_eq!(arena.len(), 2);

        arena.reset();
        assert_eq!(arena.len(), 0);
        assert!(arena.is_empty());
    }
}
