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
    ArmRange, CallArg, CallArgRange, CheckExpr, CheckRange, Expr, ExprKind, FieldInit,
    FieldInitRange, GenericParam, GenericParamRange, ListElement, ListElementRange, MapElement,
    MapElementRange, MapEntry, MapEntryRange, MatchArm, NamedExpr, NamedExprRange, Param,
    ParamRange, SeqBinding, SeqBindingRange, Stmt, StructLitField, StructLitFieldRange,
    TemplatePartRange,
};
use super::{
    BindingPatternId, ExprId, ExprRange, FunctionExpId, FunctionSeqId, MatchPatternId,
    MatchPatternRange, ParsedType, ParsedTypeId, ParsedTypeRange, Span, StmtId, StmtRange,
};

use crate::ast::patterns::{FunctionExp, FunctionSeq};
use crate::ast::TemplatePart;
use crate::ast::{BindingPattern, MatchPattern};

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

    /// Sequence bindings for `function_seq` (run/try).
    seq_bindings: Vec<SeqBinding>,

    /// Check expressions for `run()` pre/post checks.
    checks: Vec<CheckExpr>,

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

/// Generate `start_*/push_*/finish_*` method triples for direct arena append.
///
/// Instead of `series() → Vec<T> → arena.alloc_*(vec)`, callers use:
///   1. `let start = arena.start_*();`   — snapshot current buffer length
///   2. `arena.push_*(item);`            — append directly (no intermediate Vec)
///   3. `let range = arena.finish_*();`  — seal the range from start to current length
///
/// This eliminates one Vec allocation + copy per parsed list.
macro_rules! define_direct_append {
    ($field:ident, $item_ty:ty, $range_ty:ty,
     $start_fn:ident, $push_fn:ident, $finish_fn:ident, $ctx:literal) => {
        #[doc = concat!("Mark the start of a direct-append sequence into `", stringify!($field), "`.")]
        #[inline]
        pub fn $start_fn(&self) -> u32 {
            to_u32(self.$field.len(), $ctx)
        }

        #[doc = concat!("Push a single item into `", stringify!($field), "` (direct append).")]
        #[inline]
        pub fn $push_fn(&mut self, item: $item_ty) {
            self.$field.push(item);
        }

        #[doc = concat!("Finish a direct-append sequence into `", stringify!($field), "`, returning the range.")]
        pub fn $finish_fn(&mut self, start: u32) -> $range_ty {
            let len = to_u16(self.$field.len() - start as usize, $ctx);
            <$range_ty>::new(start, len)
        }
    };
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
            seq_bindings: Vec::with_capacity(estimated_exprs / 16),
            checks: Vec::with_capacity(estimated_exprs / 32),
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

    /// Allocate expression list, return range.
    #[inline]
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

    /// Allocate expression list from a slice, always storing in `expr_lists`.
    ///
    /// Returns an `ExprRange` pointing into the arena's `expr_lists` storage.
    #[inline]
    pub fn alloc_expr_list_inline(&mut self, exprs: &[ExprId]) -> ExprRange {
        let start = to_u32(self.expr_lists.len(), "expression lists");
        self.expr_lists.extend_from_slice(exprs);
        let len = to_u16(exprs.len(), "expression list");
        ExprRange::new(start, len)
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

    /// Allocate struct literal fields (for spread syntax), return range.
    pub fn alloc_struct_lit_fields(
        &mut self,
        fields: impl IntoIterator<Item = StructLitField>,
    ) -> StructLitFieldRange {
        let start = to_u32(self.struct_lit_fields.len(), "struct literal fields");
        self.struct_lit_fields.extend(fields);
        debug_assert!(
            self.struct_lit_fields.len() >= start as usize,
            "arena corruption: struct_lit_fields length {} < start {}",
            self.struct_lit_fields.len(),
            start
        );
        let len = to_u16(
            self.struct_lit_fields.len() - start as usize,
            "struct literal field list",
        );
        StructLitFieldRange::new(start, len)
    }

    /// Get struct literal fields by range.
    #[inline]
    pub fn get_struct_lit_fields(&self, range: StructLitFieldRange) -> &[StructLitField] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.struct_lit_fields[start..end]
    }

    /// Allocate list elements (for spread syntax), return range.
    pub fn alloc_list_elements(
        &mut self,
        elements: impl IntoIterator<Item = ListElement>,
    ) -> ListElementRange {
        let start = to_u32(self.list_elements.len(), "list elements");
        self.list_elements.extend(elements);
        debug_assert!(
            self.list_elements.len() >= start as usize,
            "arena corruption: list_elements length {} < start {}",
            self.list_elements.len(),
            start
        );
        let len = to_u16(
            self.list_elements.len() - start as usize,
            "list element list",
        );
        ListElementRange::new(start, len)
    }

    /// Get list elements by range.
    #[inline]
    pub fn get_list_elements(&self, range: ListElementRange) -> &[ListElement] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.list_elements[start..end]
    }

    /// Allocate map elements (for spread syntax), return range.
    pub fn alloc_map_elements(
        &mut self,
        elements: impl IntoIterator<Item = MapElement>,
    ) -> MapElementRange {
        let start = to_u32(self.map_elements.len(), "map elements");
        self.map_elements.extend(elements);
        debug_assert!(
            self.map_elements.len() >= start as usize,
            "arena corruption: map_elements length {} < start {}",
            self.map_elements.len(),
            start
        );
        let len = to_u16(self.map_elements.len() - start as usize, "map element list");
        MapElementRange::new(start, len)
    }

    /// Get map elements by range.
    #[inline]
    pub fn get_map_elements(&self, range: MapElementRange) -> &[MapElement] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.map_elements[start..end]
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

    /// Allocate check expressions, return range.
    pub fn alloc_checks(&mut self, checks: impl IntoIterator<Item = CheckExpr>) -> CheckRange {
        let start = to_u32(self.checks.len(), "check expressions");
        self.checks.extend(checks);
        debug_assert!(
            self.checks.len() >= start as usize,
            "arena corruption: checks length {} < start {}",
            self.checks.len(),
            start
        );
        let len = to_u16(self.checks.len() - start as usize, "check expression list");
        CheckRange::new(start, len)
    }

    /// Get check expressions by range.
    #[inline]
    pub fn get_checks(&self, range: CheckRange) -> &[CheckExpr] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.checks[start..end]
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

    // -- Template Part Storage --

    /// Allocate template parts from an iterator.
    pub fn alloc_template_parts(
        &mut self,
        parts: impl IntoIterator<Item = TemplatePart>,
    ) -> TemplatePartRange {
        let start = to_u32(self.template_parts.len(), "template parts");
        self.template_parts.extend(parts);
        let len = to_u16(
            self.template_parts.len() - start as usize,
            "template part list",
        );
        TemplatePartRange::new(start, len)
    }

    /// Get template parts by range.
    #[inline]
    pub fn get_template_parts(&self, range: TemplatePartRange) -> &[TemplatePart] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.template_parts[start..end]
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
        self.seq_bindings.clear();
        self.checks.clear();
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

    // -- Direct Append API --
    //
    // These method triples allow callers to push items directly into arena
    // buffers without an intermediate Vec allocation. Use pattern:
    //   let start = arena.start_params();
    //   arena.push_param(item);
    //   let range = arena.finish_params(start);

    define_direct_append!(
        params,
        Param,
        ParamRange,
        start_params,
        push_param,
        finish_params,
        "parameter list"
    );

    define_direct_append!(
        arms,
        MatchArm,
        ArmRange,
        start_arms,
        push_arm,
        finish_arms,
        "match arm list"
    );

    define_direct_append!(
        call_args,
        CallArg,
        CallArgRange,
        start_call_args,
        push_call_arg,
        finish_call_args,
        "call argument list"
    );

    define_direct_append!(
        generic_params,
        GenericParam,
        GenericParamRange,
        start_generic_params,
        push_generic_param,
        finish_generic_params,
        "generic parameter list"
    );

    define_direct_append!(
        struct_lit_fields,
        StructLitField,
        StructLitFieldRange,
        start_struct_lit_fields,
        push_struct_lit_field,
        finish_struct_lit_fields,
        "struct literal field list"
    );

    define_direct_append!(
        list_elements,
        ListElement,
        ListElementRange,
        start_list_elements,
        push_list_element,
        finish_list_elements,
        "list element list"
    );

    define_direct_append!(
        map_elements,
        MapElement,
        MapElementRange,
        start_map_elements,
        push_map_element,
        finish_map_elements,
        "map element list"
    );

    define_direct_append!(
        named_exprs,
        NamedExpr,
        NamedExprRange,
        start_named_exprs,
        push_named_expr,
        finish_named_exprs,
        "named expression list"
    );

    define_direct_append!(
        parsed_type_lists,
        ParsedTypeId,
        ParsedTypeRange,
        start_parsed_type_list,
        push_parsed_type,
        finish_parsed_type_list,
        "parsed type list"
    );

    define_direct_append!(
        match_pattern_lists,
        MatchPatternId,
        MatchPatternRange,
        start_match_pattern_list,
        push_match_pattern,
        finish_match_pattern_list,
        "match pattern list"
    );

    define_direct_append!(
        template_parts,
        TemplatePart,
        TemplatePartRange,
        start_template_parts,
        push_template_part,
        finish_template_parts,
        "template part list"
    );

    define_direct_append!(
        checks,
        CheckExpr,
        CheckRange,
        start_checks,
        push_check,
        finish_checks,
        "check expression list"
    );
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
            && self.seq_bindings == other.seq_bindings
            && self.checks == other.checks
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
        self.seq_bindings.hash(state);
        self.checks.hash(state);
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
