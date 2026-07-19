//! Flatten arena-allocated `MatchPattern`s into algorithm-internal `FlatPattern`s.
//!
//! The Maranget decision tree algorithm operates on `FlatPattern` — a self-contained
//! enum with owned `Vec`s for sub-patterns. This module bridges the gap from the
//! arena-allocated `MatchPattern` (which uses `MatchPatternId` indices into `ExprArena`)
//! to the flat representation.

use ori_ir::ast::patterns::MatchPattern;
use ori_ir::ast::ExprKind;
use ori_ir::canon::tree::FlatPattern;
use ori_ir::{ExprArena, ExprId, Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

/// Immutable context for pattern flattening.
///
/// Groups the three constant parameters (`arena`, `pool`, `interner`) that are
/// threaded unchanged through every recursive conversion. This avoids
/// passing 5 parameters at each of the 7+ recursive call sites.
pub(in crate::patterns) struct FlattenCtx<'a> {
    arena: &'a ExprArena,
    pool: &'a ori_types::Pool,
    interner: &'a StringInterner,
}

impl<'a> FlattenCtx<'a> {
    /// Create a new flattening context.
    #[must_use]
    pub(in crate::patterns) fn new(
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

    /// Convert a `MatchPattern` from the arena into a `FlatPattern`.
    ///
    /// Recursively resolves `MatchPatternId` references via the arena and
    /// converts literal `ExprId` references to concrete `FlatPattern` variants.
    ///
    /// The `interner` is needed to resolve variant names for well-known types
    /// (`Option`, `Result`) which have dedicated Pool tags rather than `Tag::Enum`.
    pub(in crate::patterns) fn to_flat_pattern(
        &self,
        pattern: &MatchPattern,
        scrutinee_ty: ori_types::Idx,
    ) -> FlatPattern {
        match pattern {
            MatchPattern::Wildcard => FlatPattern::Wildcard,

            MatchPattern::Binding(name) => FlatPattern::Binding(*name),

            MatchPattern::Literal(expr_id) => self.to_flat_literal(*expr_id),

            MatchPattern::Variant { name, inner } => {
                self.to_flat_variant(scrutinee_ty, *name, *inner)
            }

            MatchPattern::Tuple(patterns) => self.to_flat_tuple(scrutinee_ty, *patterns),

            MatchPattern::Struct { fields, rest } => {
                self.to_flat_struct(scrutinee_ty, fields, *rest)
            }

            MatchPattern::List { elements, rest } => {
                self.to_flat_list(scrutinee_ty, *elements, *rest)
            }

            MatchPattern::Range {
                start,
                end,
                inclusive,
            } => self.to_flat_range(*start, *end, *inclusive),

            MatchPattern::Or(patterns) => self.to_flat_or(scrutinee_ty, *patterns),

            MatchPattern::At { name, pattern } => self.to_flat_at(scrutinee_ty, *name, *pattern),
        }
    }

    fn to_flat_range(
        &self,
        start: Option<ExprId>,
        end: Option<ExprId>,
        inclusive: bool,
    ) -> FlatPattern {
        FlatPattern::Range {
            start: start.map(|id| self.extract_int_literal(id)),
            end: end.map(|id| self.extract_int_literal(id)),
            inclusive,
        }
    }

    fn to_flat_at(
        &self,
        scrutinee_ty: ori_types::Idx,
        name: Name,
        pattern: ori_ir::MatchPatternId,
    ) -> FlatPattern {
        let inner_pat = self.arena.get_match_pattern(pattern);
        let inner = self.to_flat_pattern(inner_pat, scrutinee_ty);
        FlatPattern::At {
            name,
            inner: Box::new(inner),
        }
    }

    fn to_flat_variant(
        &self,
        scrutinee_ty: ori_types::Idx,
        name: Name,
        patterns: ori_ir::MatchPatternRange,
    ) -> FlatPattern {
        let (variant_index, field_types) = self.resolve_variant(scrutinee_ty, name);
        let fields = self
            .arena
            .get_match_pattern_list(patterns)
            .iter()
            .enumerate()
            .map(|(index, &pattern)| {
                let pattern = self.arena.get_match_pattern(pattern);
                let field_ty = field_types
                    .get(index)
                    .copied()
                    .unwrap_or(ori_types::Idx::UNIT);
                self.to_flat_pattern(pattern, field_ty)
            })
            .collect();
        FlatPattern::Variant {
            variant_name: name,
            variant_index,
            fields,
        }
    }

    fn to_flat_tuple(
        &self,
        scrutinee_ty: ori_types::Idx,
        patterns: ori_ir::MatchPatternRange,
    ) -> FlatPattern {
        let elements = self
            .arena
            .get_match_pattern_list(patterns)
            .iter()
            .enumerate()
            .map(|(index, &pattern)| {
                self.to_flat_pattern(
                    self.arena.get_match_pattern(pattern),
                    self.resolve_tuple_elem_ty(scrutinee_ty, index),
                )
            })
            .collect();
        FlatPattern::Tuple(elements)
    }

    fn to_flat_struct(
        &self,
        scrutinee_ty: ori_types::Idx,
        fields: &[(Name, Option<ori_ir::MatchPatternId>)],
        has_rest: bool,
    ) -> FlatPattern {
        let field_types = self.resolve_struct_field_types(scrutinee_ty);
        let mut flat_fields: Vec<(Name, FlatPattern)> = fields
            .iter()
            .map(|(field_name, pattern)| {
                let field_ty = field_types
                    .as_ref()
                    .and_then(|types| types.get(field_name))
                    .copied()
                    .unwrap_or(ori_types::Idx::UNIT);

                let pattern = pattern.map_or(FlatPattern::Binding(*field_name), |pattern| {
                    self.to_flat_pattern(self.arena.get_match_pattern(pattern), field_ty)
                });
                (*field_name, pattern)
            })
            .collect();

        if has_rest {
            if let Some(field_types) = field_types {
                let explicit_fields: FxHashSet<Name> =
                    flat_fields.iter().map(|(name, _)| *name).collect();
                flat_fields.extend(
                    field_types
                        .into_iter()
                        .filter(|(name, _)| !explicit_fields.contains(name))
                        .map(|(name, _)| (name, FlatPattern::Wildcard)),
                );
            }
        }

        // Stable field ordering aligns matrix columns while named paths let
        // each consumer choose its physical layout.
        flat_fields.sort_by_key(|(name, _)| *name);
        FlatPattern::Struct {
            fields: flat_fields,
        }
    }

    fn to_flat_list(
        &self,
        scrutinee_ty: ori_types::Idx,
        patterns: ori_ir::MatchPatternRange,
        rest: Option<Name>,
    ) -> FlatPattern {
        let elem_ty = self.resolve_list_elem_ty(scrutinee_ty);
        let elements = self
            .arena
            .get_match_pattern_list(patterns)
            .iter()
            .map(|&pattern| self.to_flat_pattern(self.arena.get_match_pattern(pattern), elem_ty))
            .collect();
        FlatPattern::List { elements, rest }
    }

    fn to_flat_or(
        &self,
        scrutinee_ty: ori_types::Idx,
        patterns: ori_ir::MatchPatternRange,
    ) -> FlatPattern {
        FlatPattern::Or(
            self.arena
                .get_match_pattern_list(patterns)
                .iter()
                .map(|&pattern| {
                    self.to_flat_pattern(self.arena.get_match_pattern(pattern), scrutinee_ty)
                })
                .collect(),
        )
    }

    // Literal extraction

    /// Convert a literal expression to a `FlatPattern`.
    fn to_flat_literal(&self, expr_id: ExprId) -> FlatPattern {
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

    /// Resolve a variant name to its discriminant index and field types.
    ///
    /// Handles four cases:
    /// - `Tag::Enum`: looks up variant by name in the enum definition
    /// - `Tag::Option`: `Some` = 0, `None` = 1
    /// - `Tag::Result`: `Ok` = 0, `Err` = 1
    /// - `Tag::Ordering`: `Less` = 0, `Equal` = 1, `Greater` = 2
    fn resolve_variant(
        &self,
        enum_ty: ori_types::Idx,
        variant_name: Name,
    ) -> (u32, Vec<ori_types::Idx>) {
        use ori_types::Tag;
        let resolved = self.pool.resolve_fully(enum_ty);
        let variant = self.interner.lookup(variant_name);
        match self.pool.tag(resolved) {
            Tag::Enum => {
                return self
                    .pool
                    .enum_variants(resolved)
                    .into_iter()
                    .enumerate()
                    .find_map(|(index, (name, fields))| {
                        (name == variant_name).then(|| {
                            let index = u32::try_from(index)
                                .unwrap_or_else(|_| unreachable!("variant index fits in u32"));
                            (index, fields)
                        })
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "typed variant `{variant}` must belong to resolved enum type {resolved:?}"
                        )
                    });
            }
            Tag::Option => match variant {
                "Some" => {
                    return (
                        ori_ir::OPTION_VARIANT_SOME,
                        vec![self.pool.option_inner(resolved)],
                    )
                }
                "None" => return (ori_ir::OPTION_VARIANT_NONE, Vec::new()),
                _ => {}
            },
            Tag::Result => match variant {
                "Ok" => {
                    return (
                        ori_ir::RESULT_VARIANT_OK,
                        vec![self.pool.result_ok(resolved)],
                    )
                }
                "Err" => {
                    return (
                        ori_ir::RESULT_VARIANT_ERR,
                        vec![self.pool.result_err(resolved)],
                    )
                }
                _ => {}
            },
            Tag::Ordering => match variant {
                "Less" => return (0, Vec::new()),
                "Equal" => return (1, Vec::new()),
                "Greater" => return (2, Vec::new()),
                _ => {}
            },
            _ => {}
        }
        panic!(
            "typed variant `{variant}` must belong to resolved scrutinee type {resolved:?} ({:?}) before canonical pattern lowering",
            self.pool.tag(resolved),
        )
    }

    /// Get the type of a tuple element.
    fn resolve_tuple_elem_ty(&self, tuple_ty: ori_types::Idx, index: usize) -> ori_types::Idx {
        use ori_types::Tag;
        let resolved = self.pool.resolve_fully(tuple_ty);
        if self.pool.tag(resolved) == Tag::Tuple && index < self.pool.tuple_elem_count(resolved) {
            self.pool.tuple_elem(resolved, index)
        } else {
            ori_types::Idx::UNIT
        }
    }

    /// Get every struct field type by name in one pool traversal.
    fn resolve_struct_field_types(
        &self,
        struct_ty: ori_types::Idx,
    ) -> Option<FxHashMap<Name, ori_types::Idx>> {
        use ori_types::Tag;
        let resolved = self.pool.resolve_fully(struct_ty);
        (self.pool.tag(resolved) == Tag::Struct)
            .then(|| self.pool.struct_fields(resolved).into_iter().collect())
    }

    /// Get the element type of a list.
    fn resolve_list_elem_ty(&self, list_ty: ori_types::Idx) -> ori_types::Idx {
        use ori_types::Tag;
        let resolved = self.pool.resolve_fully(list_ty);
        if self.pool.tag(resolved) == Tag::List {
            self.pool.list_elem(resolved)
        } else {
            ori_types::Idx::UNIT
        }
    }
}
