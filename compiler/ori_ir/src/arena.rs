//! Arena allocation for flat AST.
//!
//! Per design spec A-data-structuresmd:
//! - Contiguous storage for all expressions
//! - Cache-friendly iteration
//! - Bulk deallocation
//!
//! # Capacity Limits
//! - Max expressions: 4 billion (`u32::MAX`)
//! - Max list/range length: 65,535 (`u16::MAX`)
//!
//! These limits are enforced at runtime with clear panic messages.

// Arc is needed for SharedArena - the implementation of shared arena references
#![expect(
    clippy::disallowed_types,
    reason = "Arc is the implementation of SharedArena"
)]

use super::{
    ExprId, ExprRange, MatchPatternId, MatchPatternRange, ParsedType, ParsedTypeId,
    ParsedTypeRange, StmtId, StmtRange,
};
use crate::ast::MatchPattern;

/// Panic helper for capacity overflow (cold path, never inlined).
#[cold]
#[inline(never)]
fn panic_capacity_exceeded(value: usize, context: &str, max: u64) -> ! {
    panic!(
        "arena capacity exceeded: {context} has {value} elements (0x{value:X}), max is {max} (0x{max:X})"
    )
}

/// Panic helper for range length overflow (cold path, never inlined).
#[cold]
#[inline(never)]
fn panic_range_exceeded(value: usize, context: &str, max: u64) -> ! {
    panic!(
        "range length exceeded: {context} has {value} elements (0x{value:X}), max is {max} (0x{max:X})"
    )
}

/// Convert usize to u32, panicking with a clear message on overflow.
#[inline]
fn to_u32(value: usize, context: &str) -> u32 {
    u32::try_from(value)
        .unwrap_or_else(|_| panic_capacity_exceeded(value, context, u64::from(u32::MAX)))
}

/// Convert usize to u16, panicking with a clear message on overflow.
#[inline]
fn to_u16(value: usize, context: &str) -> u16 {
    u16::try_from(value)
        .unwrap_or_else(|_| panic_range_exceeded(value, context, u64::from(u16::MAX)))
}
use super::ast::{
    ArmRange, CallArg, CallArgRange, Expr, FieldInit, FieldInitRange, GenericParam,
    GenericParamRange, MapEntry, MapEntryRange, MatchArm, NamedExpr, NamedExprRange, Param,
    ParamRange, SeqBinding, SeqBindingRange, Stmt,
};
use std::fmt;
use std::hash::{Hash, Hasher};

/// Contiguous storage for all expressions in a module.
///
/// # Design
/// Per spec: "Contiguous arrays for cache locality"
/// - All expressions stored in flat Vec
/// - Child references use `ExprId` indices
/// - Expression lists use `ExprRange` into `expr_lists`
///
/// # Salsa Compatibility
/// Has Clone, Eq, Hash for use in query results.
#[derive(Clone, Default)]
pub struct ExprArena {
    /// All expressions (indexed by `ExprId`).
    exprs: Vec<Expr>,

    /// Flattened expression lists (for Call args, List elements, etc.).
    expr_lists: Vec<ExprId>,

    /// All statements (indexed by `StmtId`).
    stmts: Vec<Stmt>,

    /// All parameters.
    params: Vec<Param>,

    /// All match arms.
    arms: Vec<MatchArm>,

    /// All map entries.
    map_entries: Vec<MapEntry>,

    /// All field initializers.
    field_inits: Vec<FieldInit>,

    /// Sequence bindings for `function_seq` (run/try).
    seq_bindings: Vec<SeqBinding>,

    /// Named expressions for `function_exp`.
    named_exprs: Vec<NamedExpr>,

    /// Call arguments for `CallNamed`.
    call_args: Vec<CallArg>,

    /// Generic parameters for functions and types.
    generic_params: Vec<GenericParam>,

    /// All parsed types (indexed by `ParsedTypeId`).
    /// Used for arena-allocated type annotations.
    parsed_types: Vec<ParsedType>,

    /// Flattened parsed type lists (for generic type arguments).
    parsed_type_lists: Vec<ParsedTypeId>,

    /// All match patterns (indexed by `MatchPatternId`).
    /// Used for arena-allocated match patterns.
    match_patterns: Vec<MatchPattern>,

    /// Flattened match pattern lists (for pattern collections).
    match_pattern_lists: Vec<MatchPatternId>,
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
            parsed_types: Vec::with_capacity(estimated_exprs / 8),
            parsed_type_lists: Vec::with_capacity(estimated_exprs / 16),
            match_patterns: Vec::with_capacity(estimated_exprs / 16),
            match_pattern_lists: Vec::with_capacity(estimated_exprs / 32),
        }
    }

    /// Allocate expression, return ID.
    #[inline]
    pub fn alloc_expr(&mut self, expr: Expr) -> ExprId {
        let id = ExprId::new(to_u32(self.exprs.len(), "expressions"));
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

    /// Allocate expression list, return range.
    pub fn alloc_expr_list(&mut self, exprs: impl IntoIterator<Item = ExprId>) -> ExprRange {
        let start = to_u32(self.expr_lists.len(), "expression lists");
        self.expr_lists.extend(exprs);
        debug_assert!(
            self.expr_lists.len() >= start as usize,
            "arena corruption: expr_lists length {} < start {}",
            self.expr_lists.len(),
            start
        );
        let len = to_u16(self.expr_lists.len() - start as usize, "expression list");
        ExprRange::new(start, len)
    }

    /// Get expression list by range.
    #[inline]
    pub fn get_expr_list(&self, range: ExprRange) -> &[ExprId] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.expr_lists[start..end]
    }

    // -- Two-Tier Storage (ExprList) --

    /// Allocate expression list with two-tier storage.
    ///
    /// For 0-2 items, stores inline in the returned `ExprList`.
    /// For 3+ items, allocates to `expr_lists` and returns an overflow reference.
    ///
    /// # Performance
    ///
    /// ~77% of function call arguments have 0-2 items and will use inline storage,
    /// eliminating indirection and improving cache locality.
    pub fn alloc_expr_list_inline(&mut self, exprs: &[ExprId]) -> super::ExprList {
        super::ExprList::from_items(exprs, |items| {
            let start = to_u32(self.expr_lists.len(), "expression lists");
            self.expr_lists.extend_from_slice(items);
            let len = to_u16(items.len(), "expression list");
            (start, len)
        })
    }

    /// Iterate over items in an `ExprList`.
    ///
    /// Works transparently for both inline and overflow storage.
    #[inline]
    pub fn iter_expr_list(&self, list: super::ExprList) -> super::ExprListIter<'_> {
        list.iter(&self.expr_lists)
    }

    /// Get raw `expr_lists` storage (for `ExprList` iteration).
    ///
    /// This is a low-level accessor for when you need direct access to the storage.
    /// Prefer `iter_expr_list()` for most use cases.
    #[inline]
    pub fn expr_lists_storage(&self) -> &[ExprId] {
        &self.expr_lists
    }

    /// Allocate statement, return ID.
    #[inline]
    pub fn alloc_stmt(&mut self, stmt: Stmt) -> StmtId {
        let id = StmtId::new(to_u32(self.stmts.len(), "statements"));
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
        StmtRange::new(start_index, to_u16(count, "statement range"))
    }

    /// Get statements by range.
    pub fn get_stmt_range(&self, range: StmtRange) -> &[Stmt] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.stmts[start..end]
    }

    /// Allocate parameter list, return range.
    pub fn alloc_params(&mut self, params: impl IntoIterator<Item = Param>) -> ParamRange {
        let start = to_u32(self.params.len(), "parameters");
        self.params.extend(params);
        debug_assert!(
            self.params.len() >= start as usize,
            "arena corruption: params length {} < start {}",
            self.params.len(),
            start
        );
        let len = to_u16(self.params.len() - start as usize, "parameter list");
        ParamRange::new(start, len)
    }

    /// Get parameters by range.
    #[inline]
    pub fn get_params(&self, range: ParamRange) -> &[Param] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.params[start..end]
    }

    /// Get just the parameter names from a range.
    ///
    /// This is a convenience method for the common pattern of extracting
    /// parameter names from a `ParamRange` for function/method registration.
    #[inline]
    pub fn get_param_names(&self, range: ParamRange) -> Vec<super::Name> {
        self.get_params(range).iter().map(|p| p.name).collect()
    }

    /// Iterate over parameter names without allocation.
    ///
    /// Use this when you only need to iterate once and don't need a collected Vec.
    /// For cases where you need to store or pass the names, use `get_param_names()`.
    #[inline]
    pub fn param_names_iter(&self, range: ParamRange) -> impl Iterator<Item = super::Name> + '_ {
        self.get_params(range).iter().map(|p| p.name)
    }

    /// Allocate match arms, return range.
    pub fn alloc_arms(&mut self, arms: impl IntoIterator<Item = MatchArm>) -> ArmRange {
        let start = to_u32(self.arms.len(), "match arms");
        self.arms.extend(arms);
        debug_assert!(
            self.arms.len() >= start as usize,
            "arena corruption: arms length {} < start {}",
            self.arms.len(),
            start
        );
        let len = to_u16(self.arms.len() - start as usize, "match arm list");
        ArmRange::new(start, len)
    }

    /// Get match arms by range.
    #[inline]
    pub fn get_arms(&self, range: ArmRange) -> &[MatchArm] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.arms[start..end]
    }

    /// Allocate map entries, return range.
    pub fn alloc_map_entries(
        &mut self,
        entries: impl IntoIterator<Item = MapEntry>,
    ) -> MapEntryRange {
        let start = to_u32(self.map_entries.len(), "map entries");
        self.map_entries.extend(entries);
        debug_assert!(
            self.map_entries.len() >= start as usize,
            "arena corruption: map_entries length {} < start {}",
            self.map_entries.len(),
            start
        );
        let len = to_u16(self.map_entries.len() - start as usize, "map entry list");
        MapEntryRange::new(start, len)
    }

    /// Get map entries by range.
    #[inline]
    pub fn get_map_entries(&self, range: MapEntryRange) -> &[MapEntry] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.map_entries[start..end]
    }

    /// Allocate field initializers, return range.
    pub fn alloc_field_inits(
        &mut self,
        inits: impl IntoIterator<Item = FieldInit>,
    ) -> FieldInitRange {
        let start = to_u32(self.field_inits.len(), "field initializers");
        self.field_inits.extend(inits);
        debug_assert!(
            self.field_inits.len() >= start as usize,
            "arena corruption: field_inits length {} < start {}",
            self.field_inits.len(),
            start
        );
        let len = to_u16(
            self.field_inits.len() - start as usize,
            "field initializer list",
        );
        FieldInitRange::new(start, len)
    }

    /// Get field initializers by range.
    #[inline]
    pub fn get_field_inits(&self, range: FieldInitRange) -> &[FieldInit] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.field_inits[start..end]
    }

    /// Allocate sequence bindings, return range.
    pub fn alloc_seq_bindings(
        &mut self,
        bindings: impl IntoIterator<Item = SeqBinding>,
    ) -> SeqBindingRange {
        let start = to_u32(self.seq_bindings.len(), "sequence bindings");
        self.seq_bindings.extend(bindings);
        debug_assert!(
            self.seq_bindings.len() >= start as usize,
            "arena corruption: seq_bindings length {} < start {}",
            self.seq_bindings.len(),
            start
        );
        let len = to_u16(
            self.seq_bindings.len() - start as usize,
            "sequence binding list",
        );
        SeqBindingRange::new(start, len)
    }

    /// Get sequence bindings by range.
    #[inline]
    pub fn get_seq_bindings(&self, range: SeqBindingRange) -> &[SeqBinding] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.seq_bindings[start..end]
    }

    /// Allocate named expressions, return range.
    pub fn alloc_named_exprs(
        &mut self,
        exprs: impl IntoIterator<Item = NamedExpr>,
    ) -> NamedExprRange {
        let start = to_u32(self.named_exprs.len(), "named expressions");
        self.named_exprs.extend(exprs);
        debug_assert!(
            self.named_exprs.len() >= start as usize,
            "arena corruption: named_exprs length {} < start {}",
            self.named_exprs.len(),
            start
        );
        let len = to_u16(
            self.named_exprs.len() - start as usize,
            "named expression list",
        );
        NamedExprRange::new(start, len)
    }

    /// Get named expressions by range.
    #[inline]
    pub fn get_named_exprs(&self, range: NamedExprRange) -> &[NamedExpr] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.named_exprs[start..end]
    }

    /// Allocate call arguments, return range.
    pub fn alloc_call_args(&mut self, args: impl IntoIterator<Item = CallArg>) -> CallArgRange {
        let start = to_u32(self.call_args.len(), "call arguments");
        self.call_args.extend(args);
        debug_assert!(
            self.call_args.len() >= start as usize,
            "arena corruption: call_args length {} < start {}",
            self.call_args.len(),
            start
        );
        let len = to_u16(self.call_args.len() - start as usize, "call argument list");
        CallArgRange::new(start, len)
    }

    /// Get call arguments by range.
    #[inline]
    pub fn get_call_args(&self, range: CallArgRange) -> &[CallArg] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.call_args[start..end]
    }

    /// Allocate generic parameters, return range.
    pub fn alloc_generic_params(
        &mut self,
        params: impl IntoIterator<Item = GenericParam>,
    ) -> GenericParamRange {
        let start = to_u32(self.generic_params.len(), "generic parameters");
        self.generic_params.extend(params);
        debug_assert!(
            self.generic_params.len() >= start as usize,
            "arena corruption: generic_params length {} < start {}",
            self.generic_params.len(),
            start
        );
        let len = to_u16(
            self.generic_params.len() - start as usize,
            "generic parameter list",
        );
        GenericParamRange::new(start, len)
    }

    /// Get generic parameters by range.
    #[inline]
    pub fn get_generic_params(&self, range: GenericParamRange) -> &[GenericParam] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.generic_params[start..end]
    }

    // -- Parsed Type Storage --

    /// Allocate a parsed type, return ID.
    #[inline]
    pub fn alloc_parsed_type(&mut self, ty: ParsedType) -> ParsedTypeId {
        let id = ParsedTypeId::new(to_u32(self.parsed_types.len(), "parsed types"));
        self.parsed_types.push(ty);
        id
    }

    /// Get parsed type by ID.
    ///
    /// # Panics
    /// Panics if `id` is out of bounds or invalid.
    #[inline]
    #[track_caller]
    pub fn get_parsed_type(&self, id: ParsedTypeId) -> &ParsedType {
        &self.parsed_types[id.index()]
    }

    /// Allocate parsed type list, return range.
    pub fn alloc_parsed_type_list(
        &mut self,
        types: impl IntoIterator<Item = ParsedTypeId>,
    ) -> ParsedTypeRange {
        let start = to_u32(self.parsed_type_lists.len(), "parsed type lists");
        self.parsed_type_lists.extend(types);
        debug_assert!(
            self.parsed_type_lists.len() >= start as usize,
            "arena corruption: parsed_type_lists length {} < start {}",
            self.parsed_type_lists.len(),
            start
        );
        let len = to_u16(
            self.parsed_type_lists.len() - start as usize,
            "parsed type list",
        );
        ParsedTypeRange::new(start, len)
    }

    /// Get parsed type list by range.
    #[inline]
    pub fn get_parsed_type_list(&self, range: ParsedTypeRange) -> &[ParsedTypeId] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.parsed_type_lists[start..end]
    }

    // -- Match Pattern Storage --

    /// Allocate a match pattern, return ID.
    #[inline]
    pub fn alloc_match_pattern(&mut self, pattern: MatchPattern) -> MatchPatternId {
        let id = MatchPatternId::new(to_u32(self.match_patterns.len(), "match patterns"));
        self.match_patterns.push(pattern);
        id
    }

    /// Get match pattern by ID.
    ///
    /// # Panics
    /// Panics if `id` is out of bounds or invalid.
    #[inline]
    #[track_caller]
    pub fn get_match_pattern(&self, id: MatchPatternId) -> &MatchPattern {
        &self.match_patterns[id.index()]
    }

    /// Allocate match pattern list, return range.
    pub fn alloc_match_pattern_list(
        &mut self,
        patterns: impl IntoIterator<Item = MatchPatternId>,
    ) -> MatchPatternRange {
        let start = to_u32(self.match_pattern_lists.len(), "match pattern lists");
        self.match_pattern_lists.extend(patterns);
        debug_assert!(
            self.match_pattern_lists.len() >= start as usize,
            "arena corruption: match_pattern_lists length {} < start {}",
            self.match_pattern_lists.len(),
            start
        );
        let len = to_u16(
            self.match_pattern_lists.len() - start as usize,
            "match pattern list",
        );
        MatchPatternRange::new(start, len)
    }

    /// Get match pattern list by range.
    #[inline]
    pub fn get_match_pattern_list(&self, range: MatchPatternRange) -> &[MatchPatternId] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.match_pattern_lists[start..end]
    }

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
        self.parsed_types.clear();
        self.parsed_type_lists.clear();
        self.match_patterns.clear();
        self.match_pattern_lists.clear();
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
            && self.parsed_types == other.parsed_types
            && self.parsed_type_lists == other.parsed_type_lists
            && self.match_patterns == other.match_patterns
            && self.match_pattern_lists == other.match_pattern_lists
    }
}

impl Eq for ExprArena {}

impl Hash for ExprArena {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.exprs.hash(state);
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
        self.parsed_types.hash(state);
        self.parsed_type_lists.hash(state);
        self.match_patterns.hash(state);
        self.match_pattern_lists.hash(state);
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

// SharedArena

use std::sync::Arc;

/// Shared expression arena wrapper for cross-module function references.
///
/// This newtype enforces that all arena sharing goes through this type,
/// preventing accidental direct `Arc<ExprArena>` usage.
///
/// # Purpose
/// When importing functions from other modules, the function's body expression
/// references expressions in the imported module's arena. `SharedArena` allows
/// the imported function to carry its arena reference for correct evaluation.
///
/// # Thread Safety
/// Uses `Arc` internally for thread-safe reference counting.
///
/// # Usage
///
/// ```text
/// let arena = SharedArena::new(parse_result.arena);
/// let func = FunctionValue::new(params, body, captures, arena);
/// ```
#[derive(Clone)]
pub struct SharedArena(Arc<ExprArena>);

impl SharedArena {
    /// Create a new shared arena from an `ExprArena`.
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
    use crate::{ast::ExprKind, Span};

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

    // -- ExprList (Two-Tier Storage) Tests --

    #[test]
    fn test_alloc_expr_list_inline_empty() {
        let mut arena = ExprArena::new();
        let list = arena.alloc_expr_list_inline(&[]);

        assert!(list.is_empty());
        assert!(list.is_inline());
        assert_eq!(list.len(), 0);

        let items: Vec<_> = arena.iter_expr_list(list).collect();
        assert!(items.is_empty());
    }

    #[test]
    fn test_alloc_expr_list_inline_single() {
        let mut arena = ExprArena::new();
        let id1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));

        let list = arena.alloc_expr_list_inline(&[id1]);

        assert!(!list.is_empty());
        assert!(list.is_inline());
        assert_eq!(list.len(), 1);

        let items: Vec<_> = arena.iter_expr_list(list).collect();
        assert_eq!(items, vec![id1]);
    }

    #[test]
    fn test_alloc_expr_list_inline_pair() {
        let mut arena = ExprArena::new();
        let id1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let id2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(2, 3)));

        let list = arena.alloc_expr_list_inline(&[id1, id2]);

        assert!(list.is_inline());
        assert_eq!(list.len(), 2);

        let items: Vec<_> = arena.iter_expr_list(list).collect();
        assert_eq!(items, vec![id1, id2]);
    }

    #[test]
    fn test_alloc_expr_list_inline_overflow() {
        let mut arena = ExprArena::new();
        let id1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let id2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(2, 3)));
        let id3 = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::new(4, 5)));

        let list = arena.alloc_expr_list_inline(&[id1, id2, id3]);

        assert!(!list.is_inline()); // Should overflow
        assert_eq!(list.len(), 3);

        let items: Vec<_> = arena.iter_expr_list(list).collect();
        assert_eq!(items, vec![id1, id2, id3]);
    }

    #[test]
    fn test_alloc_expr_list_inline_many_items() {
        let mut arena = ExprArena::new();
        let ids: Vec<_> = (0..10)
            .map(|i| arena.alloc_expr(Expr::new(ExprKind::Int(i), Span::new(0, 1))))
            .collect();

        let list = arena.alloc_expr_list_inline(&ids);

        assert!(!list.is_inline()); // Should overflow
        assert_eq!(list.len(), 10);

        let items: Vec<_> = arena.iter_expr_list(list).collect();
        assert_eq!(items, ids);
    }

    #[test]
    fn test_expr_lists_storage_access() {
        let mut arena = ExprArena::new();
        let id1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let id2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(2, 3)));
        let id3 = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::new(4, 5)));

        // Allocate an overflow list
        let _list = arena.alloc_expr_list_inline(&[id1, id2, id3]);

        // Check that storage is accessible
        let storage = arena.expr_lists_storage();
        assert_eq!(storage.len(), 3);
        assert_eq!(storage, &[id1, id2, id3]);
    }

    #[test]
    fn test_inline_does_not_allocate_to_storage() {
        let mut arena = ExprArena::new();
        let id1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let id2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(2, 3)));

        // Inline storage should not touch expr_lists
        let _list = arena.alloc_expr_list_inline(&[id1, id2]);

        assert!(
            arena.expr_lists_storage().is_empty(),
            "inline list should not allocate to expr_lists"
        );
    }
}
