//! Canonical expression arena.
//!
//! [`CanArena`] uses struct-of-arrays layout for cache locality (parallel
//! `kinds`, `spans`, `types` arrays indexed by [`CanId`]). The canonicalization
//! result types that bundle this arena live in [`super::result`].

use crate::arena::{to_u16, to_u32};
use crate::{Span, TypeId};

use super::expr::{CanExpr, CanNode};
use super::ids::{
    CanBindingPatternId, CanBindingPatternRange, CanFieldBindingRange, CanFieldRange, CanId,
    CanMapEntryRange, CanRange,
};
use super::patterns::{
    CanBindingPattern, CanFieldBinding, CanNamedExpr, CanNamedExprRange, CanParam, CanParamRange,
};
use super::support::{CanField, CanMapEntry};

/// Arena for canonical expressions.
///
/// Uses struct-of-arrays layout for cache locality, following the same
/// pattern as [`ExprArena`](crate::ExprArena).
///
/// # Index Spaces
///
/// - `kinds`/`spans`/`types`: parallel arrays indexed by [`CanId`]
/// - `expr_lists`: flat `Vec<CanId>` indexed by [`CanRange`]
/// - `map_entries`: indexed by [`CanMapEntryRange`]
/// - `fields`: indexed by [`CanFieldRange`]
#[derive(Clone, Debug)]
pub struct CanArena {
    /// Canonical expression kinds (parallel with spans and types).
    kinds: Vec<CanExpr>,
    /// Source spans for error reporting (parallel with kinds).
    spans: Vec<Span>,
    /// Resolved types from the type checker (parallel with kinds).
    types: Vec<TypeId>,
    /// Flattened expression ID lists for ranges (args, elements, stmts).
    expr_lists: Vec<CanId>,
    /// Map entries (key-value pairs).
    map_entries: Vec<CanMapEntry>,
    /// Struct field initializers (name-value pairs).
    fields: Vec<CanField>,
    /// Canonical binding patterns (indexed by `CanBindingPatternId`).
    binding_patterns: Vec<CanBindingPattern>,
    /// Flattened binding pattern ID lists (for Tuple/List sub-patterns).
    binding_pattern_lists: Vec<CanBindingPatternId>,
    /// Struct field bindings (indexed by `CanFieldBindingRange`).
    field_bindings: Vec<CanFieldBinding>,
    /// Canonical function parameters (indexed by `CanParamRange`).
    params: Vec<CanParam>,
    /// Named expressions for `FunctionExp` props (indexed by `CanNamedExprRange`).
    named_exprs: Vec<CanNamedExpr>,
}

impl CanArena {
    /// Create an empty arena.
    pub fn new() -> Self {
        Self {
            kinds: Vec::new(),
            spans: Vec::new(),
            types: Vec::new(),
            expr_lists: Vec::new(),
            map_entries: Vec::new(),
            fields: Vec::new(),
            binding_patterns: Vec::new(),
            binding_pattern_lists: Vec::new(),
            field_bindings: Vec::new(),
            params: Vec::new(),
            named_exprs: Vec::new(),
        }
    }

    /// Create an arena pre-allocated based on source length.
    ///
    /// Uses the same heuristic as `ExprArena`: ~1 expression per 20 bytes of source.
    pub fn with_capacity(source_len: usize) -> Self {
        let estimated = source_len / 20;
        Self {
            kinds: Vec::with_capacity(estimated),
            spans: Vec::with_capacity(estimated),
            types: Vec::with_capacity(estimated),
            expr_lists: Vec::with_capacity(estimated),
            map_entries: Vec::new(),
            fields: Vec::new(),
            binding_patterns: Vec::new(),
            binding_pattern_lists: Vec::new(),
            field_bindings: Vec::new(),
            params: Vec::new(),
            named_exprs: Vec::new(),
        }
    }

    /// Allocate a canonical node, returning its ID.
    pub fn push(&mut self, node: CanNode) -> CanId {
        let id = CanId::new(to_u32(self.kinds.len(), "canonical expressions"));
        self.kinds.push(node.kind);
        self.spans.push(node.span);
        self.types.push(node.ty);
        id
    }

    /// Get the expression kind for a node.
    #[inline]
    pub fn kind(&self, id: CanId) -> &CanExpr {
        &self.kinds[id.index()]
    }

    /// Get the source span for a node.
    #[inline]
    pub fn span(&self, id: CanId) -> Span {
        self.spans[id.index()]
    }

    /// Get the resolved type for a node.
    #[inline]
    pub fn ty(&self, id: CanId) -> TypeId {
        self.types[id.index()]
    }

    /// Reconstruct a full `CanNode` from parallel arrays.
    pub fn get(&self, id: CanId) -> CanNode {
        CanNode {
            kind: self.kinds[id.index()],
            span: self.spans[id.index()],
            ty: self.types[id.index()],
        }
    }

    /// Number of allocated nodes.
    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    /// Returns `true` if no nodes have been allocated.
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    /// Allocate a contiguous range of expression IDs (for args, elements, stmts).
    pub fn push_expr_list(&mut self, ids: &[CanId]) -> CanRange {
        if ids.is_empty() {
            return CanRange::EMPTY;
        }
        let start = to_u32(self.expr_lists.len(), "expression lists");
        self.expr_lists.extend_from_slice(ids);
        CanRange::new(start, to_u16(ids.len(), "expression list"))
    }

    /// Get expression IDs from a range.
    pub fn get_expr_list(&self, range: CanRange) -> &[CanId] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.expr_lists[start..end]
    }

    /// Begin building an expression list incrementally.
    pub fn start_expr_list(&self) -> u32 {
        to_u32(self.expr_lists.len(), "expression lists")
    }

    /// Push one ID to the list being built.
    pub fn push_expr_list_item(&mut self, id: CanId) {
        self.expr_lists.push(id);
    }

    /// Append all items from an existing expression list into the list being built.
    ///
    /// Use between `start_expr_list()` and `finish_expr_list()` to splice in
    /// items from an existing range without an intermediate `Vec` allocation.
    pub fn extend_expr_list(&mut self, src: CanRange) {
        if src.is_empty() {
            return;
        }
        let start = src.start as usize;
        self.expr_lists.extend_from_within(start..start + src.len());
    }

    /// Finish building an expression list.
    pub fn finish_expr_list(&self, start: u32) -> CanRange {
        let len = to_u16(self.expr_lists.len() - start as usize, "expression list");
        if len == 0 {
            CanRange::EMPTY
        } else {
            CanRange::new(start, len)
        }
    }

    /// Allocate a contiguous range of map entries.
    pub fn push_map_entries(&mut self, entries: &[CanMapEntry]) -> CanMapEntryRange {
        if entries.is_empty() {
            return CanMapEntryRange::EMPTY;
        }
        let start = to_u32(self.map_entries.len(), "map entries");
        self.map_entries.extend_from_slice(entries);
        CanMapEntryRange::new(start, to_u16(entries.len(), "map entry list"))
    }

    /// Get map entries from a range.
    pub fn get_map_entries(&self, range: CanMapEntryRange) -> &[CanMapEntry] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.map_entries[start..end]
    }

    /// Allocate a contiguous range of struct field initializers.
    pub fn push_fields(&mut self, fields: &[CanField]) -> CanFieldRange {
        if fields.is_empty() {
            return CanFieldRange::EMPTY;
        }
        let start = to_u32(self.fields.len(), "struct fields");
        self.fields.extend_from_slice(fields);
        CanFieldRange::new(start, to_u16(fields.len(), "struct field list"))
    }

    /// Get struct fields from a range.
    pub fn get_fields(&self, range: CanFieldRange) -> &[CanField] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.fields[start..end]
    }

    /// Allocate a canonical binding pattern, returning its ID.
    pub fn push_binding_pattern(&mut self, pattern: CanBindingPattern) -> CanBindingPatternId {
        let id = CanBindingPatternId::new(to_u32(self.binding_patterns.len(), "binding patterns"));
        self.binding_patterns.push(pattern);
        id
    }

    /// Get a canonical binding pattern by ID.
    pub fn get_binding_pattern(&self, id: CanBindingPatternId) -> &CanBindingPattern {
        &self.binding_patterns[id.index()]
    }

    /// Allocate a range of binding pattern IDs (for Tuple/List sub-patterns).
    pub fn push_binding_pattern_list(
        &mut self,
        ids: &[CanBindingPatternId],
    ) -> CanBindingPatternRange {
        if ids.is_empty() {
            return CanBindingPatternRange::EMPTY;
        }
        let start = to_u32(self.binding_pattern_lists.len(), "binding pattern lists");
        self.binding_pattern_lists.extend_from_slice(ids);
        CanBindingPatternRange::new(start, to_u16(ids.len(), "binding pattern list"))
    }

    /// Get binding pattern IDs from a range.
    pub fn get_binding_pattern_list(
        &self,
        range: CanBindingPatternRange,
    ) -> &[CanBindingPatternId] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.binding_pattern_lists[start..end]
    }

    /// Allocate a range of field bindings.
    pub fn push_field_bindings(&mut self, bindings: &[CanFieldBinding]) -> CanFieldBindingRange {
        if bindings.is_empty() {
            return CanFieldBindingRange::EMPTY;
        }
        let start = to_u32(self.field_bindings.len(), "field bindings");
        self.field_bindings.extend_from_slice(bindings);
        CanFieldBindingRange::new(start, to_u16(bindings.len(), "field binding list"))
    }

    /// Get field bindings from a range.
    pub fn get_field_bindings(&self, range: CanFieldBindingRange) -> &[CanFieldBinding] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.field_bindings[start..end]
    }

    /// Allocate a range of canonical parameters.
    pub fn push_params(&mut self, params: &[CanParam]) -> CanParamRange {
        if params.is_empty() {
            return CanParamRange::EMPTY;
        }
        let start = to_u32(self.params.len(), "params");
        self.params.extend_from_slice(params);
        CanParamRange::new(start, to_u16(params.len(), "param list"))
    }

    /// Get canonical parameters from a range.
    pub fn get_params(&self, range: CanParamRange) -> &[CanParam] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.params[start..end]
    }

    /// Allocate a range of named expressions.
    pub fn push_named_exprs(&mut self, exprs: &[CanNamedExpr]) -> CanNamedExprRange {
        if exprs.is_empty() {
            return CanNamedExprRange::EMPTY;
        }
        let start = to_u32(self.named_exprs.len(), "named exprs");
        self.named_exprs.extend_from_slice(exprs);
        CanNamedExprRange::new(start, to_u16(exprs.len(), "named expr list"))
    }

    /// Get named expressions from a range.
    pub fn get_named_exprs(&self, range: CanNamedExprRange) -> &[CanNamedExpr] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.named_exprs[start..end]
    }

    /// Remap all `TypeId` values in this arena using the given mapping function.
    ///
    /// Used for cross-pool type re-interning: after re-interning types from a
    /// source `Pool` into a target `Pool`, the canonical IR's `TypeId` references
    /// must be updated to use the target pool's indices.
    ///
    /// Primitives (indices 0-11) are identical across all pools and pass through
    /// unchanged. Only compound types need remapping.
    pub fn remap_types(&mut self, mut map_fn: impl FnMut(TypeId) -> TypeId) {
        for ty in &mut self.types {
            *ty = map_fn(*ty);
        }
    }
}

impl Default for CanArena {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for CanArena {
    fn eq(&self, other: &Self) -> bool {
        self.kinds == other.kinds
            && self.spans == other.spans
            && self.types == other.types
            && self.expr_lists == other.expr_lists
            && self.map_entries == other.map_entries
            && self.fields == other.fields
            && self.binding_patterns == other.binding_patterns
            && self.binding_pattern_lists == other.binding_pattern_lists
            && self.field_bindings == other.field_bindings
            && self.params == other.params
            && self.named_exprs == other.named_exprs
    }
}

impl Eq for CanArena {}
