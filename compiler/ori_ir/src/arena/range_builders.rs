//! Range builder methods for [`ExprArena`].
//!
//! Provides batch push, incremental build, and get accessors for all range types
//! (expression lists, map entries, fields, call args, patterns, etc.).

use crate::ast::{
    CallArg, CallArgRange, FieldInit, FieldInitRange, GenericParam, GenericParamRange, ListElement,
    ListElementRange, MapElement, MapElementRange, MapEntry, MapEntryRange, MatchArm, NamedExpr,
    NamedExprRange, Param, ParamRange, Stmt, StructLitField, StructLitFieldRange,
    TemplatePartRange,
};
use crate::{
    ArmRange, ExprId, ExprRange, MatchPatternId, MatchPatternRange, ParsedTypeId, ParsedTypeRange,
    StmtRange,
};

use crate::ast::TemplatePart;

use super::{to_u16, to_u32, ExprArena};

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
    // -- Expression List Ranges --

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

    // -- Statement Ranges --

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

    // -- Parameter Ranges --

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

    // -- Map Entry Ranges --

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

    // -- Field Init Ranges --

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

    // -- Struct Literal Field Ranges --

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

    // -- List Element Ranges --

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

    // -- Map Element Ranges --

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

    // -- Named Expression Ranges --

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

    // -- Call Argument Ranges --

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

    // -- Generic Parameter Ranges --

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

    // -- Parsed Type List Ranges --

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

    // -- Match Pattern List Ranges --

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

    // -- Template Part Ranges --

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
        stmts,
        Stmt,
        StmtRange,
        start_stmts,
        push_stmt,
        finish_stmts,
        "statement list"
    );
}
