//! Spread desugaring: `ListWithSpread`, `MapWithSpread`, `StructWithSpread`.
//!
//! Transforms spread syntax into compositions of primitive `CanExpr` nodes:
//! - `[a, ...b, c]` becomes `[a].concat(b).concat([c])`
//! - `{k: v, ...base}` becomes `{k: v}.merge(base)`
//! - `Point { ...base, x: 10 }` becomes a flat struct with all fields resolved

use ori_ir::canon::{CanExpr, CanField, CanId, CanMapEntry};
use ori_ir::{ListElementRange, MapElementRange, Name, Span, StructLitFieldRange, TypeId};
use rustc_hash::FxHashMap;

use crate::lower::Lowerer;

#[derive(Clone, Copy)]
enum StructSpreadField {
    Init {
        name: Name,
        value: Option<ori_ir::ExprId>,
        span: Span,
    },
    Spread {
        expr: ori_ir::ExprId,
        span: Span,
    },
}

impl Lowerer<'_> {
    // ListWithSpread → List + .concat()

    /// Desugar `[a, b, ...c, d, ...e]` into:
    ///
    /// ```text
    /// [a, b].concat(c).concat([d]).concat(e)
    /// ```
    ///
    /// Groups consecutive non-spread elements into `List` literals, then
    /// chains all segments left-to-right via `.concat()`.
    pub(crate) fn desugar_list_with_spread(
        &mut self,
        elements: ListElementRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let src_elements = self.src.get_list_elements(elements);

        // Copy out element data (is_spread, expr_id) to avoid borrow conflict.
        let mut element_data: Vec<(bool, ori_ir::ExprId)> = Vec::with_capacity(src_elements.len());
        for elem in src_elements {
            element_data.push(match elem {
                ori_ir::ListElement::Expr { expr, .. } => (false, *expr),
                ori_ir::ListElement::Spread { expr, .. } => (true, *expr),
            });
        }

        // Group consecutive non-spread elements into list segments.
        let mut segments: Vec<CanId> = Vec::new();
        let mut current_group: Vec<CanId> = Vec::new();

        for (is_spread, expr_id) in element_data {
            if is_spread {
                // Flush current non-spread group as a List.
                if !current_group.is_empty() {
                    let range = self.arena.push_expr_list(&current_group);
                    segments.push(self.push(CanExpr::List(range), span, ty));
                    current_group.clear();
                }
                // The spread expression itself is a segment.
                segments.push(self.lower_expr(expr_id));
            } else {
                current_group.push(self.lower_expr(expr_id));
            }
        }

        // Flush trailing non-spread group.
        if !current_group.is_empty() {
            let range = self.arena.push_expr_list(&current_group);
            segments.push(self.push(CanExpr::List(range), span, ty));
        }

        // Chain all segments via .concat().
        self.chain_method_calls(segments, self.name_concat, span, ty)
    }

    // MapWithSpread → Map + .merge()

    /// Desugar `{k1: v1, ...base, k2: v2}` into:
    ///
    /// ```text
    /// {k1: v1}.merge(base).merge({k2: v2})
    /// ```
    pub(crate) fn desugar_map_with_spread(
        &mut self,
        elements: MapElementRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        enum MapSegment {
            Entry(ori_ir::ExprId, ori_ir::ExprId),
            Spread(ori_ir::ExprId),
        }

        let src_elements = self.src.get_map_elements(elements);

        // Copy out element data to avoid borrow conflict.
        let mut element_data: Vec<MapSegment> = Vec::with_capacity(src_elements.len());
        for elem in src_elements {
            element_data.push(match elem {
                ori_ir::MapElement::Entry(entry) => MapSegment::Entry(entry.key, entry.value),
                ori_ir::MapElement::Spread { expr, .. } => MapSegment::Spread(*expr),
            });
        }

        // Group consecutive entries into map segments.
        let mut segments: Vec<CanId> = Vec::new();
        let mut current_entries: Vec<CanMapEntry> = Vec::new();

        for elem in element_data {
            match elem {
                MapSegment::Entry(key, value) => {
                    let key = self.lower_expr(key);
                    let value = self.lower_expr(value);
                    current_entries.push(CanMapEntry { key, value });
                }
                MapSegment::Spread(expr_id) => {
                    // Flush current entry group as a Map.
                    if !current_entries.is_empty() {
                        let range = self.arena.push_map_entries(&current_entries);
                        segments.push(self.push(CanExpr::Map(range), span, ty));
                        current_entries.clear();
                    }
                    segments.push(self.lower_expr(expr_id));
                }
            }
        }

        // Flush trailing entry group.
        if !current_entries.is_empty() {
            let range = self.arena.push_map_entries(&current_entries);
            segments.push(self.push(CanExpr::Map(range), span, ty));
        }

        // Chain all segments via .merge().
        self.chain_method_calls(segments, self.name_merge, span, ty)
    }

    // StructWithSpread → Struct

    /// Desugar `Point { ...base, x: 10 }` into a flat `Struct` with all fields
    /// resolved by extracting individual fields from the spread expression.
    ///
    /// Strategy:
    /// 1. Look up the struct definition to get all field names in order.
    /// 2. Walk the source fields left-to-right:
    ///    - `Field(init)` → sets that field's value
    ///    - `Spread { expr }` → for ALL fields, set value to `expr.field_name`
    /// 3. "Later wins" — explicit fields after a spread override the spread.
    /// 4. Emit a flat `CanExpr::Struct` with all fields.
    pub(crate) fn desugar_struct_with_spread(
        &mut self,
        name: Name,
        fields: StructLitFieldRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let field_data = self.collect_struct_spread_fields(fields);
        match self.resolve_struct_fields(name, ty) {
            Some(field_defs) => {
                self.desugar_resolved_struct_spread(name, &field_data, &field_defs, span, ty)
            }
            None => self.desugar_unresolved_struct_spread(name, &field_data, span, ty),
        }
    }

    fn collect_struct_spread_fields(&self, fields: StructLitFieldRange) -> Vec<StructSpreadField> {
        self.src
            .get_struct_lit_fields(fields)
            .iter()
            .map(|field| match field {
                ori_ir::StructLitField::Field(init) => StructSpreadField::Init {
                    name: init.name,
                    value: init.value,
                    span: init.span,
                },
                ori_ir::StructLitField::Spread { expr, span } => StructSpreadField::Spread {
                    expr: *expr,
                    span: *span,
                },
            })
            .collect()
    }

    fn desugar_resolved_struct_spread(
        &mut self,
        name: Name,
        fields: &[StructSpreadField],
        field_defs: &[(Name, TypeId)],
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let mut field_values = vec![None; field_defs.len()];
        let mut field_positions = FxHashMap::default();
        for (position, &(field_name, _)) in field_defs.iter().enumerate() {
            field_positions.entry(field_name).or_insert(position);
        }

        for &field in fields {
            match field {
                StructSpreadField::Init {
                    name: field_name,
                    value,
                    span: field_span,
                } => {
                    let Some(&position) = field_positions.get(&field_name) else {
                        continue;
                    };
                    let field_ty = field_defs[position].1;
                    let value = match value {
                        Some(expr) => self.lower_expr(expr),
                        None => self.push(CanExpr::Ident(field_name), field_span, field_ty),
                    };
                    field_values[position] = Some(value);
                }
                StructSpreadField::Spread {
                    expr,
                    span: spread_span,
                } => {
                    let spread = self.lower_expr(expr);
                    for (position, &(field_name, field_ty)) in field_defs.iter().enumerate() {
                        field_values[position] = Some(self.push(
                            CanExpr::Field {
                                receiver: spread,
                                field: field_name,
                            },
                            spread_span,
                            field_ty,
                        ));
                    }
                }
            }
        }

        let can_fields: Vec<CanField> = field_defs
            .iter()
            .zip(field_values)
            .map(|(&(field_name, _), value)| CanField {
                name: field_name,
                value: value.unwrap_or_else(|| self.push(CanExpr::Error, span, TypeId::ERROR)),
            })
            .collect();
        let fields = self.arena.push_fields(&can_fields);
        self.push(CanExpr::Struct { name, fields }, span, ty)
    }

    fn desugar_unresolved_struct_spread(
        &mut self,
        name: Name,
        fields: &[StructSpreadField],
        span: Span,
        ty: TypeId,
    ) -> CanId {
        // Skipping unresolved spreads avoids orphaning lowered nodes without a
        // field layout; explicit fields remain available for error recovery.
        let can_fields: Vec<CanField> = fields
            .iter()
            .filter_map(|field| match *field {
                StructSpreadField::Init { name, value, span } => {
                    let value = match value {
                        Some(expr) => self.lower_expr(expr),
                        None => self.push(CanExpr::Ident(name), span, TypeId::ERROR),
                    };
                    Some(CanField { name, value })
                }
                StructSpreadField::Spread { .. } => None,
            })
            .collect();
        let fields = self.arena.push_fields(&can_fields);
        self.push(CanExpr::Struct { name, fields }, span, ty)
    }

    // Shared Helpers

    /// Chain a list of segments via left-to-right method calls.
    ///
    /// `[a, b, c]` with method `concat` becomes: `a.concat(b).concat(c)`
    ///
    /// Returns the first segment directly if there's only one (no chaining needed).
    fn chain_method_calls(
        &mut self,
        segments: Vec<CanId>,
        method: Name,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let mut iter = segments.into_iter();
        let Some(first) = iter.next() else {
            // Empty — return an empty collection. Callers should handle
            // this case, but emit Error for safety.
            return self.push(CanExpr::Error, span, ty);
        };

        iter.fold(first, |acc, segment| {
            let args = self.arena.push_expr_list(&[segment]);
            self.push(
                CanExpr::MethodCall {
                    receiver: acc,
                    method,
                    args,
                },
                span,
                ty,
            )
        })
    }
}
