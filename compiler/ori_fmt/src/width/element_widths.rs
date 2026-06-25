//! Sequence-element width accumulation.
//!
//! Comma-separated width of element/argument sequences (expr lists, call args,
//! map entries, field inits, struct-lit fields, list/map elements, named exprs).
//! Each returns `ALWAYS_STACKED` when any element forces a stacked layout.

use ori_ir::{ExprId, StringLookup};

use super::metrics::COMMA_SEPARATOR_WIDTH;
use super::{WidthCalculator, ALWAYS_STACKED};

impl<I: StringLookup> WidthCalculator<'_, I> {
    /// Calculate width of an expression list (comma-separated).
    pub(super) fn width_of_expr_list(&mut self, exprs: &[ExprId]) -> usize {
        super::metrics::accumulate_widths(exprs, |id| self.width(*id), COMMA_SEPARATOR_WIDTH)
    }

    /// Calculate width of call arguments (name: value, ...).
    pub(super) fn width_of_call_args(&mut self, args: &[ori_ir::CallArg]) -> usize {
        if args.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, arg) in args.iter().enumerate() {
            let value_w = self.width(arg.value);
            if value_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;
            }

            if let Some(name) = arg.name {
                total += self.interner.lookup(name).len() + 2 + value_w;
            } else {
                total += value_w;
            }

            if i < args.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of map entries (key: value, ...).
    pub(super) fn width_of_map_entries(&mut self, entries: &[ori_ir::MapEntry]) -> usize {
        if entries.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, entry) in entries.iter().enumerate() {
            let key_w = self.width(entry.key);
            let value_w = self.width(entry.value);
            if key_w == ALWAYS_STACKED || value_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;
            }
            total += key_w + 2 + value_w;

            if i < entries.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of field initializers (name: value, ...).
    pub(super) fn width_of_field_inits(&mut self, fields: &[ori_ir::FieldInit]) -> usize {
        if fields.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, field) in fields.iter().enumerate() {
            let name_w = self.interner.lookup(field.name).len();

            if let Some(value) = field.value {
                let value_w = self.width(value);
                if value_w == ALWAYS_STACKED {
                    return ALWAYS_STACKED;
                }
                total += name_w + 2 + value_w;
            } else {
                total += name_w;
            }

            if i < fields.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of struct literal fields (including spreads).
    pub(super) fn width_of_struct_lit_fields(
        &mut self,
        fields: &[ori_ir::StructLitField],
    ) -> usize {
        if fields.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, field) in fields.iter().enumerate() {
            match field {
                ori_ir::StructLitField::Field(init) => {
                    let name_w = self.interner.lookup(init.name).len();
                    if let Some(value) = init.value {
                        let value_w = self.width(value);
                        if value_w == ALWAYS_STACKED {
                            return ALWAYS_STACKED;
                        }
                        total += name_w + 2 + value_w;
                    } else {
                        total += name_w;
                    }
                }
                ori_ir::StructLitField::Spread { expr, .. } => {
                    let expr_w = self.width(*expr);
                    if expr_w == ALWAYS_STACKED {
                        return ALWAYS_STACKED;
                    }
                    // "..." + expr
                    total += 3 + expr_w;
                }
            }

            if i < fields.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of list elements (including spreads).
    pub(super) fn width_of_list_elements(&mut self, elements: &[ori_ir::ListElement]) -> usize {
        if elements.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, element) in elements.iter().enumerate() {
            match element {
                ori_ir::ListElement::Expr { expr, .. } => {
                    let expr_w = self.width(*expr);
                    if expr_w == ALWAYS_STACKED {
                        return ALWAYS_STACKED;
                    }
                    total += expr_w;
                }
                ori_ir::ListElement::Spread { expr, .. } => {
                    let expr_w = self.width(*expr);
                    if expr_w == ALWAYS_STACKED {
                        return ALWAYS_STACKED;
                    }
                    // "..." + expr
                    total += 3 + expr_w;
                }
            }

            if i < elements.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of map elements (including spreads).
    pub(super) fn width_of_map_elements(&mut self, elements: &[ori_ir::MapElement]) -> usize {
        if elements.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, element) in elements.iter().enumerate() {
            match element {
                ori_ir::MapElement::Entry(entry) => {
                    let key_w = self.width(entry.key);
                    let value_w = self.width(entry.value);
                    if key_w == ALWAYS_STACKED || value_w == ALWAYS_STACKED {
                        return ALWAYS_STACKED;
                    }
                    total += key_w + 2 + value_w; // key: value
                }
                ori_ir::MapElement::Spread { expr, .. } => {
                    let expr_w = self.width(*expr);
                    if expr_w == ALWAYS_STACKED {
                        return ALWAYS_STACKED;
                    }
                    // "..." + expr
                    total += 3 + expr_w;
                }
            }

            if i < elements.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of named expressions (name: value, ...).
    pub(super) fn width_of_named_exprs(&mut self, exprs: &[ori_ir::NamedExpr]) -> usize {
        if exprs.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, expr) in exprs.iter().enumerate() {
            let name_w = self.interner.lookup(expr.name).len();
            let value_w = self.width(expr.value);
            if value_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;
            }
            total += name_w + 2 + value_w;

            if i < exprs.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }
}
