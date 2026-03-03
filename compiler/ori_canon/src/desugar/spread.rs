//! Spread desugaring: `ListWithSpread`, `MapWithSpread`, `StructWithSpread`.
//!
//! Transforms spread syntax into compositions of primitive `CanExpr` nodes:
//! - `[a, ...b, c]` becomes `[a].concat(b).concat([c])`
//! - `{k: v, ...base}` becomes `{k: v}.merge(base)`
//! - `Point { ...base, x: 10 }` becomes a flat struct with all fields resolved

use ori_ir::canon::{CanExpr, CanField, CanId, CanMapEntry};
use ori_ir::{ListElementRange, MapElementRange, Name, Span, StructLitFieldRange, TypeId};

use crate::lower::Lowerer;

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
    #[expect(
        clippy::too_many_lines,
        reason = "multi-step struct spread desugaring with field override resolution"
    )]
    pub(crate) fn desugar_struct_with_spread(
        &mut self,
        name: Name,
        fields: StructLitFieldRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        enum FieldData {
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

        let src_fields = self.src.get_struct_lit_fields(fields);

        // Copy out field data to avoid borrow conflict.
        let mut field_data: Vec<FieldData> = Vec::with_capacity(src_fields.len());
        for f in src_fields {
            field_data.push(match f {
                ori_ir::StructLitField::Field(init) => FieldData::Init {
                    name: init.name,
                    value: init.value,
                    span: init.span,
                },
                ori_ir::StructLitField::Spread { expr, span } => FieldData::Spread {
                    expr: *expr,
                    span: *span,
                },
            });
        }

        // Look up the struct definition for field ordering.
        let struct_field_names = self.resolve_struct_fields(name);

        if let Some(field_names) = struct_field_names {
            // We know the struct layout — build a fully resolved field list.
            let mut field_values: Vec<Option<CanId>> = vec![None; field_names.len()];

            for field in &field_data {
                match field {
                    FieldData::Init {
                        name: field_name,
                        value,
                        span: field_span,
                    } => {
                        let field_name = *field_name;
                        let field_span = *field_span;
                        if let Some(pos) = field_names.iter().position(|n| *n == field_name) {
                            let val = match value {
                                Some(expr_id) => self.lower_expr(*expr_id),
                                None => {
                                    self.push(CanExpr::Ident(field_name), field_span, TypeId::ERROR)
                                }
                            };
                            field_values[pos] = Some(val);
                        }
                    }
                    FieldData::Spread {
                        expr: spread_expr,
                        span: spread_span,
                    } => {
                        let spread = self.lower_expr(*spread_expr);
                        for (i, field_name) in field_names.iter().enumerate() {
                            let field_access = self.push(
                                CanExpr::Field {
                                    receiver: spread,
                                    field: *field_name,
                                },
                                *spread_span,
                                TypeId::ERROR,
                            );
                            field_values[i] = Some(field_access);
                        }
                    }
                }
            }

            // Build the canonical fields.
            let can_fields: Vec<CanField> = field_names
                .iter()
                .zip(field_values)
                .map(|(fname, value)| {
                    let value = value.unwrap_or_else(|| {
                        // Missing field — emit Error (type checker should catch this).
                        self.push(CanExpr::Error, span, TypeId::ERROR)
                    });
                    CanField {
                        name: *fname,
                        value,
                    }
                })
                .collect();

            let fields_range = self.arena.push_fields(&can_fields);
            self.push(
                CanExpr::Struct {
                    name,
                    fields: fields_range,
                },
                span,
                ty,
            )
        } else {
            // Struct definition not found — fall back to lowering fields in order.
            // This handles error recovery gracefully.
            let mut can_fields = Vec::new();
            for field in &field_data {
                match field {
                    FieldData::Init {
                        name: field_name,
                        value,
                        span: field_span,
                    } => {
                        let field_name = *field_name;
                        let field_span = *field_span;
                        let val = match value {
                            Some(expr_id) => self.lower_expr(*expr_id),
                            None => {
                                self.push(CanExpr::Ident(field_name), field_span, TypeId::ERROR)
                            }
                        };
                        can_fields.push(CanField {
                            name: field_name,
                            value: val,
                        });
                    }
                    FieldData::Spread { .. } => {
                        // Struct definition not found — skip lowering the spread
                        // expression to avoid allocating orphaned nodes in the arena.
                    }
                }
            }
            let fields_range = self.arena.push_fields(&can_fields);
            self.push(
                CanExpr::Struct {
                    name,
                    fields: fields_range,
                },
                span,
                ty,
            )
        }
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

    /// Look up struct field names in order from the type registry.
    fn resolve_struct_fields(&self, name: Name) -> Option<Vec<Name>> {
        let type_entry = self.typed.type_def(name)?;
        match &type_entry.kind {
            ori_types::TypeKind::Struct(def) => Some(def.fields.iter().map(|f| f.name).collect()),
            _ => None,
        }
    }
}
