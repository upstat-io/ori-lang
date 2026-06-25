//! Direct-append builder methods for [`ExprArena`].
//!
//! `start_*/push_*/finish_*` method triples append items directly into arena
//! buffers without an intermediate `Vec` allocation, in contrast to the
//! `alloc_*` range builders in the sibling `range_builders` module:
//!   1. `let start = arena.start_*();`   — snapshot current buffer length
//!   2. `arena.push_*(item);`            — append directly (no intermediate Vec)
//!   3. `let range = arena.finish_*();`  — seal the range from start to current length

use crate::ast::{
    AccessStep, CallArg, GenericParam, ListElement, MapElement, NamedExpr, Param, Stmt,
    StructLitField, TemplatePart,
};
use crate::{
    AccessStepRange, ArmRange, CallArgRange, GenericParamRange, ListElementRange, MapElementRange,
    MatchArm, MatchPatternId, MatchPatternRange, NamedExprRange, ParamRange, ParsedTypeId,
    ParsedTypeRange, StmtRange, StructLitFieldRange, TemplatePartRange,
};

use super::{to_u16, to_u32, ExprArena};

/// Generate `start_*/push_*/finish_*` method triples for direct arena append.
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

    define_direct_append!(
        access_steps,
        AccessStep,
        AccessStepRange,
        start_access_steps,
        push_access_step,
        finish_access_steps,
        "access step list"
    );
}
