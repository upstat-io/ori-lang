//! Arena allocation for flat AST.
//!
//! Per design spec A-data-structuresmd:
//! - Contiguous storage for all expressions
//! - Cache-friendly iteration
//! - Bulk deallocation

// Arc is needed for SharedArena - the implementation of shared arena references
#![expect(clippy::disallowed_types, reason = "Arc is the implementation of SharedArena")]

use super::{ExprId, ExprRange, StmtId, StmtRange};
use super::ast::{
    Expr, Stmt, Param, ParamRange,
    MatchArm, MapEntry, FieldInit,
    ArmRange, MapEntryRange, FieldInitRange,
    SeqBinding, SeqBindingRange,
    NamedExpr, NamedExprRange,
    CallArg, CallArgRange,
    GenericParam, GenericParamRange,
};
use std::fmt;
use std::hash::{Hash, Hasher};

/// Contiguous storage for all expressions in a module.
///
/// # Design
/// Per spec: "Contiguous arrays for cache locality"
/// - All expressions stored in flat Vec
/// - Child references use ExprId indices
/// - Expression lists use ExprRange into expr_lists
///
/// # Salsa Compatibility
/// Has Clone, Eq, Hash for use in query results.
#[derive(Clone, Default)]
pub struct ExprArena {
    /// All expressions (indexed by ExprId).
    exprs: Vec<Expr>,

    /// Flattened expression lists (for Call args, List elements, etc.).
    expr_lists: Vec<ExprId>,

    /// All statements (indexed by StmtId).
    stmts: Vec<Stmt>,

    /// All parameters.
    params: Vec<Param>,

    /// All match arms.
    arms: Vec<MatchArm>,

    /// All map entries.
    map_entries: Vec<MapEntry>,

    /// All field initializers.
    field_inits: Vec<FieldInit>,

    /// Sequence bindings for function_seq (run/try).
    seq_bindings: Vec<SeqBinding>,

    /// Named expressions for function_exp.
    named_exprs: Vec<NamedExpr>,

    /// Call arguments for CallNamed.
    call_args: Vec<CallArg>,

    /// Generic parameters for functions and types.
    generic_params: Vec<GenericParam>,
}

impl ExprArena {
    /// Create a new empty arena.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with estimated capacity based on source size.
    /// Heuristic: ~1 expression per 20 bytes of source.
    pub fn with_capacity(source_len: usize) -> Self {
        let estimated_exprs = source_len / 20;
        ExprArena {
            exprs: Vec::with_capacity(estimated_exprs),
            expr_lists: Vec::with_capacity(estimated_exprs / 2),
            stmts: Vec::with_capacity(estimated_exprs / 4),
            params: Vec::with_capacity(estimated_exprs / 8),
            arms: Vec::with_capacity(estimated_exprs / 16),
            map_entries: Vec::with_capacity(estimated_exprs / 16),
            field_inits: Vec::with_capacity(estimated_exprs / 16),
            seq_bindings: Vec::with_capacity(estimated_exprs / 16),
            named_exprs: Vec::with_capacity(estimated_exprs / 16),
            call_args: Vec::with_capacity(estimated_exprs / 16),
            generic_params: Vec::with_capacity(estimated_exprs / 32),
        }
    }

    // ===== Expression allocation =====

    /// Allocate expression, return ID.
    #[inline]
    pub fn alloc_expr(&mut self, expr: Expr) -> ExprId {
        let id = ExprId::new(self.exprs.len() as u32);
        self.exprs.push(expr);
        id
    }

    /// Get expression by ID.
    ///
    /// # Panics
    /// Panics if `id` is out of bounds.
    #[inline]
    #[track_caller]
    pub fn get_expr(&self, id: ExprId) -> &Expr {
        &self.exprs[id.index()]
    }

    /// Get mutable expression by ID.
    ///
    /// # Panics
    /// Panics if `id` is out of bounds.
    #[inline]
    #[track_caller]
    pub fn get_expr_mut(&mut self, id: ExprId) -> &mut Expr {
        &mut self.exprs[id.index()]
    }

    /// Get number of expressions.
    #[inline]
    pub fn expr_count(&self) -> usize {
        self.exprs.len()
    }

    // ===== Expression list allocation =====

    /// Allocate expression list, return range.
    pub fn alloc_expr_list(&mut self, exprs: impl IntoIterator<Item = ExprId>) -> ExprRange {
        let start = self.expr_lists.len() as u32;
        self.expr_lists.extend(exprs);
        let len = (self.expr_lists.len() as u32 - start) as u16;
        ExprRange::new(start, len)
    }

    /// Get expression list by range.
    #[inline]
    pub fn get_expr_list(&self, range: ExprRange) -> &[ExprId] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.expr_lists[start..end]
    }

    // ===== Statement allocation =====

    /// Allocate statement, return ID.
    #[inline]
    pub fn alloc_stmt(&mut self, stmt: Stmt) -> StmtId {
        let id = StmtId::new(self.stmts.len() as u32);
        self.stmts.push(stmt);
        id
    }

    /// Get statement by ID.
    ///
    /// # Panics
    /// Panics if `id` is out of bounds.
    #[inline]
    #[track_caller]
    pub fn get_stmt(&self, id: StmtId) -> &Stmt {
        &self.stmts[id.index()]
    }

    /// Allocate statement list, return range.
    pub fn alloc_stmt_range(&mut self, start_index: u32, count: usize) -> StmtRange {
        StmtRange::new(start_index, count as u16)
    }

    /// Get statements by range.
    pub fn get_stmt_range(&self, range: StmtRange) -> &[Stmt] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.stmts[start..end]
    }

    // ===== Parameter allocation =====

    /// Allocate parameter list, return range.
    pub fn alloc_params(&mut self, params: impl IntoIterator<Item = Param>) -> ParamRange {
        let start = self.params.len() as u32;
        self.params.extend(params);
        let len = (self.params.len() as u32 - start) as u16;
        ParamRange::new(start, len)
    }

    /// Get parameters by range.
    #[inline]
    pub fn get_params(&self, range: ParamRange) -> &[Param] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.params[start..end]
    }

    // ===== Match arm allocation =====

    /// Allocate match arms, return range.
    pub fn alloc_arms(&mut self, arms: impl IntoIterator<Item = MatchArm>) -> ArmRange {
        let start = self.arms.len() as u32;
        self.arms.extend(arms);
        let len = (self.arms.len() as u32 - start) as u16;
        ArmRange::new(start, len)
    }

    /// Get match arms by range.
    #[inline]
    pub fn get_arms(&self, range: ArmRange) -> &[MatchArm] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.arms[start..end]
    }

    // ===== Map entry allocation =====

    /// Allocate map entries, return range.
    pub fn alloc_map_entries(&mut self, entries: impl IntoIterator<Item = MapEntry>) -> MapEntryRange {
        let start = self.map_entries.len() as u32;
        self.map_entries.extend(entries);
        let len = (self.map_entries.len() as u32 - start) as u16;
        MapEntryRange::new(start, len)
    }

    /// Get map entries by range.
    #[inline]
    pub fn get_map_entries(&self, range: MapEntryRange) -> &[MapEntry] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.map_entries[start..end]
    }

    // ===== Field init allocation =====

    /// Allocate field initializers, return range.
    pub fn alloc_field_inits(&mut self, inits: impl IntoIterator<Item = FieldInit>) -> FieldInitRange {
        let start = self.field_inits.len() as u32;
        self.field_inits.extend(inits);
        let len = (self.field_inits.len() as u32 - start) as u16;
        FieldInitRange::new(start, len)
    }

    /// Get field initializers by range.
    #[inline]
    pub fn get_field_inits(&self, range: FieldInitRange) -> &[FieldInit] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.field_inits[start..end]
    }

    // ===== Sequence binding allocation (function_seq) =====

    /// Allocate sequence bindings, return range.
    pub fn alloc_seq_bindings(&mut self, bindings: impl IntoIterator<Item = SeqBinding>) -> SeqBindingRange {
        let start = self.seq_bindings.len() as u32;
        self.seq_bindings.extend(bindings);
        let len = (self.seq_bindings.len() as u32 - start) as u16;
        SeqBindingRange::new(start, len)
    }

    /// Get sequence bindings by range.
    #[inline]
    pub fn get_seq_bindings(&self, range: SeqBindingRange) -> &[SeqBinding] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.seq_bindings[start..end]
    }

    // ===== Named expression allocation (function_exp) =====

    /// Allocate named expressions, return range.
    pub fn alloc_named_exprs(&mut self, exprs: impl IntoIterator<Item = NamedExpr>) -> NamedExprRange {
        let start = self.named_exprs.len() as u32;
        self.named_exprs.extend(exprs);
        let len = (self.named_exprs.len() as u32 - start) as u16;
        NamedExprRange::new(start, len)
    }

    /// Get named expressions by range.
    #[inline]
    pub fn get_named_exprs(&self, range: NamedExprRange) -> &[NamedExpr] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.named_exprs[start..end]
    }

    // ===== Call argument allocation =====

    /// Allocate call arguments, return range.
    pub fn alloc_call_args(&mut self, args: impl IntoIterator<Item = CallArg>) -> CallArgRange {
        let start = self.call_args.len() as u32;
        self.call_args.extend(args);
        let len = (self.call_args.len() as u32 - start) as u16;
        CallArgRange::new(start, len)
    }

    /// Get call arguments by range.
    #[inline]
    pub fn get_call_args(&self, range: CallArgRange) -> &[CallArg] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.call_args[start..end]
    }

    // ===== Generic parameter allocation =====

    /// Allocate generic parameters, return range.
    pub fn alloc_generic_params(&mut self, params: impl IntoIterator<Item = GenericParam>) -> GenericParamRange {
        let start = self.generic_params.len() as u32;
        self.generic_params.extend(params);
        let len = (self.generic_params.len() as u32 - start) as u16;
        GenericParamRange::new(start, len)
    }

    /// Get generic parameters by range.
    #[inline]
    pub fn get_generic_params(&self, range: GenericParamRange) -> &[GenericParam] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.generic_params[start..end]
    }

    // ===== Utility =====

    /// Reset arena for reuse (keeps capacity).
    pub fn reset(&mut self) {
        self.exprs.clear();
        self.expr_lists.clear();
        self.stmts.clear();
        self.params.clear();
        self.arms.clear();
        self.map_entries.clear();
        self.field_inits.clear();
        self.seq_bindings.clear();
        self.named_exprs.clear();
        self.call_args.clear();
        self.generic_params.clear();
    }

    /// Check if arena is empty.
    pub fn is_empty(&self) -> bool {
        self.exprs.is_empty()
    }
}

impl PartialEq for ExprArena {
    fn eq(&self, other: &Self) -> bool {
        self.exprs == other.exprs
            && self.expr_lists == other.expr_lists
            && self.stmts == other.stmts
            && self.params == other.params
            && self.arms == other.arms
            && self.map_entries == other.map_entries
            && self.field_inits == other.field_inits
            && self.seq_bindings == other.seq_bindings
            && self.named_exprs == other.named_exprs
            && self.call_args == other.call_args
            && self.generic_params == other.generic_params
    }
}

impl Eq for ExprArena {}

impl Hash for ExprArena {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.exprs.len().hash(state);
        for expr in &self.exprs {
            expr.hash(state);
        }
        self.expr_lists.hash(state);
        self.stmts.hash(state);
        self.params.hash(state);
        self.arms.hash(state);
        self.map_entries.hash(state);
        self.field_inits.hash(state);
        self.seq_bindings.hash(state);
        self.named_exprs.hash(state);
        self.call_args.hash(state);
        self.generic_params.hash(state);
    }
}

impl fmt::Debug for ExprArena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExprArena {{ {} exprs, {} lists, {} stmts, {} params }}",
            self.exprs.len(),
            self.expr_lists.len(),
            self.stmts.len(),
            self.params.len()
        )
    }
}

// =============================================================================
// SharedArena
// =============================================================================

use std::sync::Arc;

/// Shared expression arena wrapper for cross-module function references.
///
/// This newtype enforces that all arena sharing goes through this type,
/// preventing accidental direct `Arc<ExprArena>` usage.
///
/// # Purpose
/// When importing functions from other modules, the function's body expression
/// references expressions in the imported module's arena. SharedArena allows
/// the imported function to carry its arena reference for correct evaluation.
///
/// # Thread Safety
/// Uses `Arc` internally for thread-safe reference counting.
///
/// # Usage
/// ```ignore
/// let imported_arena = SharedArena::new(parse_result.arena);
/// let func = FunctionValue::from_import(params, body, captures, imported_arena);
/// ```
#[derive(Clone)]
pub struct SharedArena(Arc<ExprArena>);

impl SharedArena {
    /// Create a new shared arena from an ExprArena.
    pub fn new(arena: ExprArena) -> Self {
        SharedArena(Arc::new(arena))
    }
}

impl std::ops::Deref for SharedArena {
    type Target = ExprArena;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Debug for SharedArena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SharedArena({:?})", &*self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Span, ast::ExprKind};

    #[test]
    fn test_alloc_expr() {
        let mut arena = ExprArena::new();

        let id1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let id2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(2, 3)));

        assert_eq!(id1.index(), 0);
        assert_eq!(id2.index(), 1);
        assert_eq!(arena.expr_count(), 2);

        assert!(matches!(arena.get_expr(id1).kind, ExprKind::Int(1)));
        assert!(matches!(arena.get_expr(id2).kind, ExprKind::Int(2)));
    }

    #[test]
    fn test_alloc_expr_list() {
        let mut arena = ExprArena::new();

        let id1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let id2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(2, 3)));
        let id3 = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::new(4, 5)));

        let range = arena.alloc_expr_list([id1, id2, id3]);

        assert_eq!(range.len(), 3);
        let list = arena.get_expr_list(range);
        assert_eq!(list, &[id1, id2, id3]);
    }

    #[test]
    fn test_arena_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        let mut arena1 = ExprArena::new();
        arena1.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(0, 2)));

        let mut arena2 = ExprArena::new();
        arena2.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(0, 2)));

        let arena3 = ExprArena::new();

        set.insert(arena1);
        set.insert(arena2);
        set.insert(arena3);

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_arena_reset() {
        let mut arena = ExprArena::new();

        arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        assert_eq!(arena.expr_count(), 1);

        arena.reset();
        assert!(arena.is_empty());
        assert_eq!(arena.expr_count(), 0);
    }
}
