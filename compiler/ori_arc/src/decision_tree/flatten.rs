//! Flatten arena-allocated `MatchPattern`s into algorithm-internal `FlatPattern`s.
//!
//! The Maranget decision tree algorithm operates on `FlatPattern` — a self-contained
//! enum with owned `Vec`s for sub-patterns. This module bridges the gap from the
//! arena-allocated `MatchPattern` (which uses `MatchPatternId` indices into `ExprArena`)
//! to the flat representation.

use ori_ir::ast::patterns::MatchPattern;
use ori_ir::ast::ExprKind;
use ori_ir::{ExprArena, ExprId, Name, StringInterner};

use super::FlatPattern;

/// Immutable context for pattern flattening.
///
/// Groups the three constant parameters (`arena`, `pool`, `interner`) that are
/// threaded unchanged through every recursive call to `flatten`. This avoids
/// passing 5 parameters at each of the 7+ recursive call sites.
pub struct FlattenCtx<'a> {
    pub arena: &'a ExprArena,
    pub pool: &'a ori_types::Pool,
    pub interner: &'a StringInterner,
}

impl<'a> FlattenCtx<'a> {
    /// Create a new flattening context.
    pub fn new(
        arena: &'a ExprArena,
        pool: &'a ori_types::Pool,
        interner: &'a StringInterner,
    ) -> Self {
        Self {
            arena,
            pool,
            interner,
        }
    }

    /// Flatten a `MatchPattern` from the arena into a `FlatPattern`.
    ///
    /// Recursively resolves `MatchPatternId` references via the arena and
    /// converts literal `ExprId` references to concrete `FlatPattern` variants.
    ///
    /// The `interner` is needed to resolve variant names for well-known types
    /// (`Option`, `Result`) which have dedicated Pool tags rather than `Tag::Enum`.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive MatchPattern → FlatPattern lowering"
    )]
    pub fn flatten(&self, pattern: &MatchPattern, scrutinee_ty: ori_types::Idx) -> FlatPattern {
        match pattern {
            MatchPattern::Wildcard => FlatPattern::Wildcard,

            MatchPattern::Binding(name) => FlatPattern::Binding(*name),

            MatchPattern::Literal(expr_id) => self.flatten_literal(*expr_id),

            MatchPattern::Variant { name, inner } => {
                let variant_index = self.resolve_variant_index(scrutinee_ty, *name);
                let pat_ids = self.arena.get_match_pattern_list(*inner);

                // Get the field types for this variant directly from the enum definition.
                let field_types =
                    self.resolve_variant_field_types(scrutinee_ty, variant_index, *name);

                let fields: Vec<FlatPattern> = pat_ids
                    .iter()
                    .enumerate()
                    .map(|(i, &pat_id)| {
                        let inner_pat = self.arena.get_match_pattern(pat_id);
                        let field_ty = field_types.get(i).copied().unwrap_or(ori_types::Idx::UNIT);
                        self.flatten(inner_pat, field_ty)
                    })
                    .collect();

                FlatPattern::Variant {
                    variant_name: *name,
                    variant_index,
                    fields,
                }
            }

            MatchPattern::Tuple(sub_patterns) => {
                let pat_ids = self.arena.get_match_pattern_list(*sub_patterns);
                let elements: Vec<FlatPattern> = pat_ids
                    .iter()
                    .enumerate()
                    .map(|(i, &pat_id)| {
                        let inner_pat = self.arena.get_match_pattern(pat_id);
                        let elem_ty = self.resolve_tuple_elem_ty(scrutinee_ty, i);
                        self.flatten(inner_pat, elem_ty)
                    })
                    .collect();

                FlatPattern::Tuple(elements)
            }

            MatchPattern::Struct { fields, rest } => {
                // Build flat patterns for explicitly named fields
                let mut flat_fields: Vec<(Name, FlatPattern)> = fields
                    .iter()
                    .map(|(field_name, sub_pat)| {
                        let field_ty = self.resolve_struct_field_ty(scrutinee_ty, *field_name);
                        let flat = if let Some(pat_id) = sub_pat {
                            let inner_pat = self.arena.get_match_pattern(*pat_id);
                            self.flatten(inner_pat, field_ty)
                        } else {
                            // Shorthand: `{ x }` is equivalent to `{ x: x }` → binding
                            FlatPattern::Binding(*field_name)
                        };
                        (*field_name, flat)
                    })
                    .collect();

                // When rest (`..`) is present, pad missing struct fields with wildcards.
                // The decision tree requires uniform column counts across all matrix rows.
                if *rest {
                    let all_fields = self.resolve_all_struct_fields(scrutinee_ty);
                    for (fname, _fty) in &all_fields {
                        if !flat_fields.iter().any(|(n, _)| n == fname) {
                            flat_fields.push((*fname, FlatPattern::Wildcard));
                        }
                    }
                }

                // Sort by Name to match StructValue's layout order.
                // StructValue::new sorts field names, so StructField(i) indices
                // must correspond to sorted-by-Name positions.
                flat_fields.sort_by_key(|(name, _)| *name);

                FlatPattern::Struct {
                    fields: flat_fields,
                }
            }

            MatchPattern::List { elements, rest } => {
                let pat_ids = self.arena.get_match_pattern_list(*elements);
                let elem_ty = self.resolve_list_elem_ty(scrutinee_ty);
                let flat_elements: Vec<FlatPattern> = pat_ids
                    .iter()
                    .map(|&pat_id| {
                        let inner_pat = self.arena.get_match_pattern(pat_id);
                        self.flatten(inner_pat, elem_ty)
                    })
                    .collect();

                FlatPattern::List {
                    elements: flat_elements,
                    rest: *rest,
                }
            }

            MatchPattern::Range {
                start,
                end,
                inclusive,
            } => {
                let start_val = start.map(|id| self.extract_int_literal(id));
                let end_val = end.map(|id| self.extract_int_literal(id));
                FlatPattern::Range {
                    start: start_val,
                    end: end_val,
                    inclusive: *inclusive,
                }
            }

            MatchPattern::Or(sub_patterns) => {
                let pat_ids = self.arena.get_match_pattern_list(*sub_patterns);
                let alternatives: Vec<FlatPattern> = pat_ids
                    .iter()
                    .map(|&pat_id| {
                        let inner_pat = self.arena.get_match_pattern(pat_id);
                        self.flatten(inner_pat, scrutinee_ty)
                    })
                    .collect();

                FlatPattern::Or(alternatives)
            }

            MatchPattern::At { name, pattern } => {
                let inner_pat = self.arena.get_match_pattern(*pattern);
                let inner = self.flatten(inner_pat, scrutinee_ty);
                FlatPattern::At {
                    name: *name,
                    inner: Box::new(inner),
                }
            }
        }
    }

    // Literal extraction

    /// Convert a literal expression to a `FlatPattern`.
    fn flatten_literal(&self, expr_id: ExprId) -> FlatPattern {
        let expr = self.arena.get_expr(expr_id);
        match &expr.kind {
            ExprKind::Int(v) => FlatPattern::LitInt(*v),
            ExprKind::Float(bits) => FlatPattern::LitFloat(*bits),
            ExprKind::Bool(v) => FlatPattern::LitBool(*v),
            ExprKind::String(name) => FlatPattern::LitStr(*name),
            ExprKind::Char(v) => FlatPattern::LitChar(*v),
            _ => {
                tracing::debug!(
                    ?expr_id,
                    "non-literal in pattern position, treating as wildcard"
                );
                FlatPattern::Wildcard
            }
        }
    }

    /// Extract an i64 from a literal expression (for range patterns).
    ///
    /// Handles both integer and char literals — chars are converted to their
    /// Unicode code point for numeric range comparison.
    fn extract_int_literal(&self, expr_id: ExprId) -> i64 {
        let expr = self.arena.get_expr(expr_id);
        match &expr.kind {
            ExprKind::Int(v) => *v,
            ExprKind::Char(c) => i64::from(u32::from(*c)),
            _ => {
                tracing::debug!(?expr_id, "non-int/char literal in range pattern");
                0
            }
        }
    }

    // Type resolution helpers

    /// Resolve a variant name to its discriminant index.
    ///
    /// Handles three cases:
    /// - `Tag::Enum`: looks up variant by name in the enum definition
    /// - `Tag::Option`: `None` = 0, `Some` = 1
    /// - `Tag::Result`: `Ok` = 0, `Err` = 1
    #[expect(
        clippy::cast_possible_truncation,
        reason = "variant indices never exceed u32"
    )]
    fn resolve_variant_index(&self, enum_ty: ori_types::Idx, variant_name: Name) -> u32 {
        use ori_types::Tag;
        let resolved = self.pool.resolve_fully(enum_ty);
        match self.pool.tag(resolved) {
            Tag::Enum => {
                let count = self.pool.enum_variant_count(resolved);
                for i in 0..count {
                    let (vname, _) = self.pool.enum_variant(resolved, i);
                    if vname == variant_name {
                        return i as u32;
                    }
                }
            }
            Tag::Option => {
                // Convention: None = 0, Some = 1 (matches evaluator's Value::Some/None)
                return u32::from(self.interner.lookup(variant_name) == "Some");
            }
            Tag::Result => {
                // Convention: Ok = 0, Err = 1 (matches evaluator's Value::Ok/Err)
                return u32::from(self.interner.lookup(variant_name) == "Err");
            }
            _ => {}
        }
        tracing::debug!(
            ?enum_ty,
            ?resolved,
            resolved_tag = ?self.pool.tag(resolved),
            ?variant_name,
            "could not resolve variant index in flatten"
        );
        0
    }

    /// Get the field types for a specific variant of an enum, Option, or Result.
    ///
    /// - `Tag::Enum`: returns field types from the enum definition
    /// - `Tag::Option`: `Some` (index 1) has one field (the inner type), `None` (index 0) has none
    /// - `Tag::Result`: `Ok` (index 0) has one field (ok type), `Err` (index 1) has one field (err type)
    fn resolve_variant_field_types(
        &self,
        enum_ty: ori_types::Idx,
        variant_index: u32,
        variant_name: Name,
    ) -> Vec<ori_types::Idx> {
        use ori_types::Tag;
        let resolved = self.pool.resolve_fully(enum_ty);
        match self.pool.tag(resolved) {
            Tag::Enum => {
                let count = self.pool.enum_variant_count(resolved);
                if (variant_index as usize) < count {
                    let (_, field_types) = self.pool.enum_variant(resolved, variant_index as usize);
                    return field_types;
                }
            }
            Tag::Option => {
                // Some has 1 field (the inner type), None has 0 fields.
                if self.interner.lookup(variant_name) == "Some" {
                    return vec![self.pool.option_inner(resolved)];
                }
                return Vec::new();
            }
            Tag::Result => {
                // Ok has 1 field (ok type), Err has 1 field (err type).
                if self.interner.lookup(variant_name) == "Err" {
                    return vec![self.pool.result_err(resolved)];
                }
                return vec![self.pool.result_ok(resolved)];
            }
            _ => {}
        }
        Vec::new()
    }

    /// Get the type of a tuple element.
    fn resolve_tuple_elem_ty(&self, tuple_ty: ori_types::Idx, index: usize) -> ori_types::Idx {
        use ori_types::Tag;
        let resolved = self.pool.resolve_fully(tuple_ty);
        if self.pool.tag(resolved) == Tag::Tuple {
            let count = self.pool.tuple_elem_count(resolved);
            if index < count {
                return self.pool.tuple_elem(resolved, index);
            }
        }
        ori_types::Idx::UNIT
    }

    /// Get all fields of a struct type as `(name, type)` pairs.
    fn resolve_all_struct_fields(&self, struct_ty: ori_types::Idx) -> Vec<(Name, ori_types::Idx)> {
        use ori_types::Tag;
        let resolved = self.pool.resolve_fully(struct_ty);
        if self.pool.tag(resolved) == Tag::Struct {
            let count = self.pool.struct_field_count(resolved);
            (0..count)
                .map(|i| self.pool.struct_field(resolved, i))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get the type of a struct field.
    fn resolve_struct_field_ty(
        &self,
        struct_ty: ori_types::Idx,
        field_name: Name,
    ) -> ori_types::Idx {
        use ori_types::Tag;
        let resolved = self.pool.resolve_fully(struct_ty);
        if self.pool.tag(resolved) == Tag::Struct {
            let count = self.pool.struct_field_count(resolved);
            for i in 0..count {
                let (fname, fty) = self.pool.struct_field(resolved, i);
                if fname == field_name {
                    return fty;
                }
            }
        }
        ori_types::Idx::UNIT
    }

    /// Get the element type of a list.
    fn resolve_list_elem_ty(&self, list_ty: ori_types::Idx) -> ori_types::Idx {
        use ori_types::Tag;
        let resolved = self.pool.resolve_fully(list_ty);
        if self.pool.tag(resolved) == Tag::List {
            return self.pool.list_elem(resolved);
        }
        ori_types::Idx::UNIT
    }
}
