//! Range builder methods for [`ExprArena`].
//!
//! Provides batch push, incremental build, and get accessors for all range types
//! (expression lists, map entries, fields, call args, patterns, etc.).

use crate::ast::{
    AccessStep, AccessStepRange, CallArg, CallArgRange, FieldInit, FieldInitRange, GenericParam,
    GenericParamRange, ListElement, ListElementRange, MapElement, MapElementRange, MapEntry,
    MapEntryRange, MatchArm, NamedExpr, NamedExprRange, Param, ParamRange, Stmt, StructLitField,
    StructLitFieldRange, TemplatePartRange,
};
use crate::{
    ArmRange, ExprId, ExprRange, MatchPatternId, MatchPatternRange, ParsedTypeId, ParsedTypeRange,
    StmtRange,
};

use crate::ast::TemplatePart;

use super::{to_u16, to_u32, ExprArena};

impl ExprArena {
    /// Append `items` to `storage` and return the covering range built via
    /// `ctor`. Every `alloc_*` range builder in this file shares this exact
    /// append + overflow-check + range-construction skeleton; only the
    /// backing storage, the range constructor, and the two labels
    /// (for the `to_u32`/`to_u16` overflow diagnostics) differ per call.
    #[inline]
    fn alloc_range<T, R>(
        storage: &mut Vec<T>,
        items: impl IntoIterator<Item = T>,
        ctor: impl FnOnce(u32, u16) -> R,
        storage_label: &str,
        list_label: &str,
    ) -> R {
        let start = to_u32(storage.len(), storage_label);
        storage.extend(items);
        debug_assert!(
            storage.len() >= start as usize,
            "arena corruption: {storage_label} length {} < start {start}",
            storage.len(),
        );
        let len = to_u16(storage.len() - start as usize, list_label);
        ctor(start, len)
    }

    /// Slice `storage` by a `(start, len)` range. Every `get_*` range
    /// accessor in this file shares this exact index computation.
    #[inline]
    fn slice_range<T>(storage: &[T], start: u32, len: u16) -> &[T] {
        let start = start as usize;
        let end = start + len as usize;
        &storage[start..end]
    }

    // -- Expression List Ranges --

    /// Allocate expression list, return range.
    #[inline]
    pub fn alloc_expr_list(&mut self, exprs: impl IntoIterator<Item = ExprId>) -> ExprRange {
        Self::alloc_range(
            &mut self.expr_lists,
            exprs,
            ExprRange::new,
            "expression lists",
            "expression list",
        )
    }

    /// Get expression list by range.
    #[inline]
    pub fn get_expr_list(&self, range: ExprRange) -> &[ExprId] {
        Self::slice_range(&self.expr_lists, range.start, range.len)
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

    // -- Statement Ranges --

    /// Allocate statement list, return range.
    pub fn alloc_stmt_range(&mut self, start_index: u32, count: usize) -> StmtRange {
        StmtRange::new(start_index, to_u16(count, "statement range"))
    }

    /// Get statements by range.
    pub fn get_stmt_range(&self, range: StmtRange) -> &[Stmt] {
        Self::slice_range(&self.stmts, range.start, range.len)
    }

    // -- Parameter Ranges --

    /// Allocate parameter list, return range.
    pub fn alloc_params(&mut self, params: impl IntoIterator<Item = Param>) -> ParamRange {
        Self::alloc_range(
            &mut self.params,
            params,
            ParamRange::new,
            "parameters",
            "parameter list",
        )
    }

    /// Get parameters by range.
    #[inline]
    pub fn get_params(&self, range: ParamRange) -> &[Param] {
        Self::slice_range(&self.params, range.start, range.len)
    }

    /// Get just the parameter names from a range.
    ///
    /// This is a convenience method for the common pattern of extracting
    /// parameter names from a `ParamRange` for function/method registration.
    #[inline]
    pub fn get_param_names(&self, range: ParamRange) -> Vec<crate::Name> {
        self.get_params(range).iter().map(|p| p.name).collect()
    }

    /// Iterate over parameter names without allocation.
    ///
    /// Use this when you only need to iterate once and don't need a collected Vec.
    /// For cases where you need to store or pass the names, use `get_param_names()`.
    #[inline]
    pub fn param_names_iter(&self, range: ParamRange) -> impl Iterator<Item = crate::Name> + '_ {
        self.get_params(range).iter().map(|p| p.name)
    }

    // -- Match Arm Ranges --

    /// Allocate match arms, return range.
    pub fn alloc_arms(&mut self, arms: impl IntoIterator<Item = MatchArm>) -> ArmRange {
        Self::alloc_range(
            &mut self.arms,
            arms,
            ArmRange::new,
            "match arms",
            "match arm list",
        )
    }

    /// Get match arms by range.
    #[inline]
    pub fn get_arms(&self, range: ArmRange) -> &[MatchArm] {
        Self::slice_range(&self.arms, range.start, range.len)
    }

    // -- Map Entry Ranges --

    /// Allocate map entries, return range.
    pub fn alloc_map_entries(
        &mut self,
        entries: impl IntoIterator<Item = MapEntry>,
    ) -> MapEntryRange {
        Self::alloc_range(
            &mut self.map_entries,
            entries,
            MapEntryRange::new,
            "map entries",
            "map entry list",
        )
    }

    /// Get map entries by range.
    #[inline]
    pub fn get_map_entries(&self, range: MapEntryRange) -> &[MapEntry] {
        Self::slice_range(&self.map_entries, range.start, range.len)
    }

    // -- Field Init Ranges --

    /// Allocate field initializers, return range.
    pub fn alloc_field_inits(
        &mut self,
        inits: impl IntoIterator<Item = FieldInit>,
    ) -> FieldInitRange {
        Self::alloc_range(
            &mut self.field_inits,
            inits,
            FieldInitRange::new,
            "field initializers",
            "field initializer list",
        )
    }

    /// Get field initializers by range.
    #[inline]
    pub fn get_field_inits(&self, range: FieldInitRange) -> &[FieldInit] {
        Self::slice_range(&self.field_inits, range.start, range.len)
    }

    // -- Struct Literal Field Ranges --

    /// Allocate struct literal fields (for spread syntax), return range.
    pub fn alloc_struct_lit_fields(
        &mut self,
        fields: impl IntoIterator<Item = StructLitField>,
    ) -> StructLitFieldRange {
        Self::alloc_range(
            &mut self.struct_lit_fields,
            fields,
            StructLitFieldRange::new,
            "struct literal fields",
            "struct literal field list",
        )
    }

    /// Get struct literal fields by range.
    #[inline]
    pub fn get_struct_lit_fields(&self, range: StructLitFieldRange) -> &[StructLitField] {
        Self::slice_range(&self.struct_lit_fields, range.start, range.len)
    }

    // -- List Element Ranges --

    /// Allocate list elements (for spread syntax), return range.
    pub fn alloc_list_elements(
        &mut self,
        elements: impl IntoIterator<Item = ListElement>,
    ) -> ListElementRange {
        Self::alloc_range(
            &mut self.list_elements,
            elements,
            ListElementRange::new,
            "list elements",
            "list element list",
        )
    }

    /// Get list elements by range.
    #[inline]
    pub fn get_list_elements(&self, range: ListElementRange) -> &[ListElement] {
        Self::slice_range(&self.list_elements, range.start, range.len)
    }

    // -- Map Element Ranges --

    /// Allocate map elements (for spread syntax), return range.
    pub fn alloc_map_elements(
        &mut self,
        elements: impl IntoIterator<Item = MapElement>,
    ) -> MapElementRange {
        Self::alloc_range(
            &mut self.map_elements,
            elements,
            MapElementRange::new,
            "map elements",
            "map element list",
        )
    }

    /// Get map elements by range.
    #[inline]
    pub fn get_map_elements(&self, range: MapElementRange) -> &[MapElement] {
        Self::slice_range(&self.map_elements, range.start, range.len)
    }

    // -- Named Expression Ranges --

    /// Allocate named expressions, return range.
    pub fn alloc_named_exprs(
        &mut self,
        exprs: impl IntoIterator<Item = NamedExpr>,
    ) -> NamedExprRange {
        Self::alloc_range(
            &mut self.named_exprs,
            exprs,
            NamedExprRange::new,
            "named expressions",
            "named expression list",
        )
    }

    /// Get named expressions by range.
    #[inline]
    pub fn get_named_exprs(&self, range: NamedExprRange) -> &[NamedExpr] {
        Self::slice_range(&self.named_exprs, range.start, range.len)
    }

    // -- Call Argument Ranges --

    /// Allocate call arguments, return range.
    pub fn alloc_call_args(&mut self, args: impl IntoIterator<Item = CallArg>) -> CallArgRange {
        Self::alloc_range(
            &mut self.call_args,
            args,
            CallArgRange::new,
            "call arguments",
            "call argument list",
        )
    }

    /// Get call arguments by range.
    #[inline]
    pub fn get_call_args(&self, range: CallArgRange) -> &[CallArg] {
        Self::slice_range(&self.call_args, range.start, range.len)
    }

    // -- Generic Parameter Ranges --

    /// Allocate generic parameters, return range.
    pub fn alloc_generic_params(
        &mut self,
        params: impl IntoIterator<Item = GenericParam>,
    ) -> GenericParamRange {
        Self::alloc_range(
            &mut self.generic_params,
            params,
            GenericParamRange::new,
            "generic parameters",
            "generic parameter list",
        )
    }

    /// Get generic parameters by range.
    #[inline]
    pub fn get_generic_params(&self, range: GenericParamRange) -> &[GenericParam] {
        Self::slice_range(&self.generic_params, range.start, range.len)
    }

    // -- Parsed Type List Ranges --

    /// Allocate parsed type list, return range.
    pub fn alloc_parsed_type_list(
        &mut self,
        types: impl IntoIterator<Item = ParsedTypeId>,
    ) -> ParsedTypeRange {
        Self::alloc_range(
            &mut self.parsed_type_lists,
            types,
            ParsedTypeRange::new,
            "parsed type lists",
            "parsed type list",
        )
    }

    /// Get parsed type list by range.
    #[inline]
    pub fn get_parsed_type_list(&self, range: ParsedTypeRange) -> &[ParsedTypeId] {
        Self::slice_range(&self.parsed_type_lists, range.start, range.len)
    }

    // -- Match Pattern List Ranges --

    /// Allocate match pattern list, return range.
    pub fn alloc_match_pattern_list(
        &mut self,
        patterns: impl IntoIterator<Item = MatchPatternId>,
    ) -> MatchPatternRange {
        Self::alloc_range(
            &mut self.match_pattern_lists,
            patterns,
            MatchPatternRange::new,
            "match pattern lists",
            "match pattern list",
        )
    }

    /// Get match pattern list by range.
    #[inline]
    pub fn get_match_pattern_list(&self, range: MatchPatternRange) -> &[MatchPatternId] {
        Self::slice_range(&self.match_pattern_lists, range.start, range.len)
    }

    // -- Template Part Ranges --

    /// Allocate template parts from an iterator.
    pub fn alloc_template_parts(
        &mut self,
        parts: impl IntoIterator<Item = TemplatePart>,
    ) -> TemplatePartRange {
        Self::alloc_range(
            &mut self.template_parts,
            parts,
            TemplatePartRange::new,
            "template parts",
            "template part list",
        )
    }

    /// Get template parts by range.
    #[inline]
    pub fn get_template_parts(&self, range: TemplatePartRange) -> &[TemplatePart] {
        Self::slice_range(&self.template_parts, range.start, range.len)
    }

    // -- Access Step Ranges --

    /// Allocate access steps for an assignment-target chain, return range.
    pub fn alloc_access_steps(
        &mut self,
        steps: impl IntoIterator<Item = AccessStep>,
    ) -> AccessStepRange {
        Self::alloc_range(
            &mut self.access_steps,
            steps,
            AccessStepRange::new,
            "access steps",
            "access step list",
        )
    }

    /// Get access steps by range.
    #[inline]
    pub fn get_access_steps(&self, range: AccessStepRange) -> &[AccessStep] {
        Self::slice_range(&self.access_steps, range.start, range.len)
    }
}
