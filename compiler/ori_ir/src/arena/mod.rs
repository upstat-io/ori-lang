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

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::ast::{
    CallArg, Expr, ExprKind, FieldInit, GenericParam, ListElement, MapElement, MapEntry, MatchArm,
    NamedExpr, Param, Stmt, StructLitField,
};
use super::{
    BindingPatternId, ExprId, FunctionExpId, FunctionSeqId, MatchPatternId, ParsedType,
    ParsedTypeId, Span, StmtId,
};

use crate::ast::patterns::{FunctionExp, FunctionSeq};
use crate::ast::{BindingPattern, MatchPattern, TemplatePart};

mod range_builders;

/// Panic helper for capacity overflow (cold path, never inlined).
#[cold]
#[inline(never)]
pub(crate) fn panic_capacity_exceeded(value: usize, context: &str, max: u64) -> ! {
    panic!(
        "arena capacity exceeded: {context} has {value} elements (0x{value:X}), max is {max} (0x{max:X})"
    )
}

/// Panic helper for range length overflow (cold path, never inlined).
#[cold]
#[inline(never)]
pub(crate) fn panic_range_exceeded(value: usize, context: &str, max: u64) -> ! {
    panic!(
        "range length exceeded: {context} has {value} elements (0x{value:X}), max is {max} (0x{max:X})"
    )
}

/// Convert usize to u32, panicking with a clear message on overflow.
#[inline]
pub(crate) fn to_u32(value: usize, context: &str) -> u32 {
    u32::try_from(value)
        .unwrap_or_else(|_| panic_capacity_exceeded(value, context, u64::from(u32::MAX)))
}

/// Convert usize to u16, panicking with a clear message on overflow.
#[inline]
pub(crate) fn to_u16(value: usize, context: &str) -> u16 {
    u16::try_from(value)
        .unwrap_or_else(|_| panic_range_exceeded(value, context, u64::from(u16::MAX)))
}

/// Contiguous storage for all expressions in a module.
///
/// # Design
/// Per spec: "Contiguous arrays for cache locality"
/// - Struct-of-Arrays layout: kinds and spans in separate arrays
/// - Child references use `ExprId` indices
/// - Expression lists use `ExprRange` into `expr_lists`
///
/// # Struct-of-Arrays Layout
/// Expressions are stored in parallel arrays (`expr_kinds` + `expr_spans`)
/// rather than a single `Vec<Expr>`. This improves cache utilization since
/// most operations only need the kind (24 bytes) and rarely touch the span
/// (8 bytes) — keeping them separate means more kinds fit per cache line.
///
/// # Salsa Compatibility
/// Has Clone, Eq, Hash for use in query results.
#[derive(Clone, Default)]
pub struct ExprArena {
    /// Expression kinds (indexed by `ExprId`). Parallel array.
    expr_kinds: Vec<ExprKind>,

    /// Expression spans (indexed by `ExprId`). Parallel array.
    /// Parallel to `expr_kinds` — same length, same indices.
    expr_spans: Vec<Span>,

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

    /// Struct literal fields (field inits and spreads).
    struct_lit_fields: Vec<StructLitField>,

    /// List elements (values and spreads) for list literals with spread.
    list_elements: Vec<ListElement>,

    /// Map elements (entries and spreads) for map literals with spread.
    map_elements: Vec<MapElement>,

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

    /// All binding patterns (indexed by `BindingPatternId`).
    binding_patterns: Vec<BindingPattern>,

    /// All function sequences (indexed by `FunctionSeqId`).
    function_seqs: Vec<FunctionSeq>,

    /// All function expressions (indexed by `FunctionExpId`).
    function_exps: Vec<FunctionExp>,

    /// Template interpolation parts for template literals.
    template_parts: Vec<TemplatePart>,
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
            expr_kinds: Vec::with_capacity(estimated_exprs),
            expr_spans: Vec::with_capacity(estimated_exprs),
            expr_lists: Vec::with_capacity(estimated_exprs / 2),
            stmts: Vec::with_capacity(estimated_exprs / 4),
            params: Vec::with_capacity(estimated_exprs / 8),
            arms: Vec::with_capacity(estimated_exprs / 16),
            map_entries: Vec::with_capacity(estimated_exprs / 16),
            field_inits: Vec::with_capacity(estimated_exprs / 16),
            struct_lit_fields: Vec::with_capacity(estimated_exprs / 16),
            list_elements: Vec::with_capacity(estimated_exprs / 16),
            map_elements: Vec::with_capacity(estimated_exprs / 16),
            named_exprs: Vec::with_capacity(estimated_exprs / 16),
            call_args: Vec::with_capacity(estimated_exprs / 16),
            generic_params: Vec::with_capacity(estimated_exprs / 32),
            parsed_types: Vec::with_capacity(estimated_exprs / 8),
            parsed_type_lists: Vec::with_capacity(estimated_exprs / 16),
            match_patterns: Vec::with_capacity(estimated_exprs / 16),
            match_pattern_lists: Vec::with_capacity(estimated_exprs / 32),
            binding_patterns: Vec::with_capacity(estimated_exprs / 8),
            function_seqs: Vec::with_capacity(estimated_exprs / 32),
            function_exps: Vec::with_capacity(estimated_exprs / 32),
            template_parts: Vec::with_capacity(estimated_exprs / 32),
        }
    }

    /// Allocate expression, return ID.
    ///
    /// Decomposes the `Expr` into kind and span for parallel-array storage.
    #[inline]
    pub fn alloc_expr(&mut self, expr: Expr) -> ExprId {
        let id = ExprId::new(to_u32(self.expr_kinds.len(), "expressions"));
        self.expr_kinds.push(expr.kind);
        self.expr_spans.push(expr.span);
        id
    }

    /// Get expression by ID (reconstructed from parallel arrays).
    ///
    /// Returns `Expr` by value since `Expr` is `Copy` (32 bytes).
    /// For hot paths, prefer `expr_kind()` and `expr_span()` to avoid
    /// touching the span array when only the kind is needed.
    ///
    /// # Panics
    /// Panics if `id` is out of bounds.
    #[inline]
    #[track_caller]
    pub fn get_expr(&self, id: ExprId) -> Expr {
        let i = id.index();
        Expr {
            kind: self.expr_kinds[i],
            span: self.expr_spans[i],
        }
    }

    /// Get expression kind by ID (direct array access).
    ///
    /// Preferred over `get_expr()` when only the kind is needed,
    /// since it avoids touching the span array (better cache behavior).
    ///
    /// # Panics
    /// Panics if `id` is out of bounds.
    #[inline]
    #[track_caller]
    pub fn expr_kind(&self, id: ExprId) -> &ExprKind {
        &self.expr_kinds[id.index()]
    }

    /// Get expression span by ID (direct array access).
    ///
    /// # Panics
    /// Panics if `id` is out of bounds.
    #[inline]
    #[track_caller]
    pub fn expr_span(&self, id: ExprId) -> Span {
        self.expr_spans[id.index()]
    }

    /// Get number of expressions.
    #[inline]
    pub fn expr_count(&self) -> usize {
        self.expr_kinds.len()
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

    // -- Binding Pattern Storage --

    /// Allocate a binding pattern, return ID.
    #[inline]
    pub fn alloc_binding_pattern(&mut self, pattern: BindingPattern) -> BindingPatternId {
        let id = BindingPatternId::new(to_u32(self.binding_patterns.len(), "binding patterns"));
        self.binding_patterns.push(pattern);
        id
    }

    /// Get binding pattern by ID.
    ///
    /// # Panics
    /// Panics if `id` is out of bounds or invalid.
    #[inline]
    #[track_caller]
    pub fn get_binding_pattern(&self, id: BindingPatternId) -> &BindingPattern {
        &self.binding_patterns[id.index()]
    }

    // -- Function Sequence Storage --

    /// Allocate a function sequence, return ID.
    #[inline]
    pub fn alloc_function_seq(&mut self, seq: FunctionSeq) -> FunctionSeqId {
        let id = FunctionSeqId::new(to_u32(self.function_seqs.len(), "function sequences"));
        self.function_seqs.push(seq);
        id
    }

    /// Get function sequence by ID.
    ///
    /// # Panics
    /// Panics if `id` is out of bounds or invalid.
    #[inline]
    #[track_caller]
    pub fn get_function_seq(&self, id: FunctionSeqId) -> &FunctionSeq {
        &self.function_seqs[id.index()]
    }

    // -- Function Expression Storage --

    /// Allocate a function expression, return ID.
    #[inline]
    pub fn alloc_function_exp(&mut self, exp: FunctionExp) -> FunctionExpId {
        let id = FunctionExpId::new(to_u32(self.function_exps.len(), "function expressions"));
        self.function_exps.push(exp);
        id
    }

    /// Get function expression by ID.
    ///
    /// # Panics
    /// Panics if `id` is out of bounds or invalid.
    #[inline]
    #[track_caller]
    pub fn get_function_exp(&self, id: FunctionExpId) -> &FunctionExp {
        &self.function_exps[id.index()]
    }

    /// Reset arena for reuse (keeps capacity).
    pub fn reset(&mut self) {
        self.expr_kinds.clear();
        self.expr_spans.clear();
        self.expr_lists.clear();
        self.stmts.clear();
        self.params.clear();
        self.arms.clear();
        self.map_entries.clear();
        self.field_inits.clear();
        self.struct_lit_fields.clear();
        self.list_elements.clear();
        self.map_elements.clear();
        self.named_exprs.clear();
        self.call_args.clear();
        self.generic_params.clear();
        self.parsed_types.clear();
        self.parsed_type_lists.clear();
        self.match_patterns.clear();
        self.match_pattern_lists.clear();
        self.binding_patterns.clear();
        self.function_seqs.clear();
        self.function_exps.clear();
        self.template_parts.clear();
    }

    /// Check if arena is empty.
    pub fn is_empty(&self) -> bool {
        self.expr_kinds.is_empty()
    }
}

impl PartialEq for ExprArena {
    fn eq(&self, other: &Self) -> bool {
        self.expr_kinds == other.expr_kinds
            && self.expr_spans == other.expr_spans
            && self.expr_lists == other.expr_lists
            && self.stmts == other.stmts
            && self.params == other.params
            && self.arms == other.arms
            && self.map_entries == other.map_entries
            && self.field_inits == other.field_inits
            && self.struct_lit_fields == other.struct_lit_fields
            && self.list_elements == other.list_elements
            && self.map_elements == other.map_elements
            && self.named_exprs == other.named_exprs
            && self.call_args == other.call_args
            && self.generic_params == other.generic_params
            && self.parsed_types == other.parsed_types
            && self.parsed_type_lists == other.parsed_type_lists
            && self.match_patterns == other.match_patterns
            && self.match_pattern_lists == other.match_pattern_lists
            && self.binding_patterns == other.binding_patterns
            && self.function_seqs == other.function_seqs
            && self.function_exps == other.function_exps
            && self.template_parts == other.template_parts
    }
}

impl Eq for ExprArena {}

impl Hash for ExprArena {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.expr_kinds.hash(state);
        self.expr_spans.hash(state);
        self.expr_lists.hash(state);
        self.stmts.hash(state);
        self.params.hash(state);
        self.arms.hash(state);
        self.map_entries.hash(state);
        self.field_inits.hash(state);
        self.struct_lit_fields.hash(state);
        self.list_elements.hash(state);
        self.map_elements.hash(state);
        self.named_exprs.hash(state);
        self.call_args.hash(state);
        self.generic_params.hash(state);
        self.parsed_types.hash(state);
        self.parsed_type_lists.hash(state);
        self.match_patterns.hash(state);
        self.match_pattern_lists.hash(state);
        self.binding_patterns.hash(state);
        self.function_seqs.hash(state);
        self.function_exps.hash(state);
        self.template_parts.hash(state);
    }
}

impl fmt::Debug for ExprArena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExprArena {{ {} exprs, {} lists, {} stmts, {} params }}",
            self.expr_kinds.len(),
            self.expr_lists.len(),
            self.stmts.len(),
            self.params.len()
        )
    }
}

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
/// `ParseOutput.arena` is already a `SharedArena`, so cloning is O(1):
/// ```text
/// let arena = parse_result.arena.clone(); // Arc::clone, not deep copy
/// let func = FunctionValue::new(params, captures, arena);
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

impl PartialEq for SharedArena {
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

impl Eq for SharedArena {}

impl std::hash::Hash for SharedArena {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for SharedArena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SharedArena({:?})", &*self.0)
    }
}

#[cfg(test)]
mod tests;
