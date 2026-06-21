//! Non-primitive `{expr:spec}` interpolation desugaring.
//!
//! When a template interpolation carries a format spec on a NON-primitive
//! expression, this routes the desugaring per spec
//! (Spec: Clause 14 string interpolation + Clause 9 Printable/Formattable):
//!
//! - Primitive `expr_ty` (int/float/bool/char/str): keep `CanExpr::FormatWith`
//!   so the `ori_format_*` runtime fast path applies in both backends.
//! - Non-primitive with an explicit `impl T: Formattable`: emit a
//!   `Formattable.format(self:, spec:)` `MethodCall` with a synthesized
//!   `FormatSpec` struct argument.
//! - Non-primitive Printable-only (blanket `impl<T: Printable> T: Formattable`):
//!   emit `FormatWith { expr: <expr.to_str()>, spec }`, re-routing through the
//!   existing `Idx::STR => "ori_format_str"` arm that applies width/align/fill/
//!   precision identically in both backends.

use ori_ir::canon::{CanExpr, CanField, CanId};
use ori_ir::format_spec::{parse_format_spec, Align, FormatType, ParsedFormatSpec, Sign};
use ori_ir::{Name, Span, TypeId};
use ori_types::{FormatSpecTypes, Idx};

use crate::lower::Lowerer;

impl Lowerer<'_> {
    /// Desugar a `{expr:spec}` interpolation. Returns the `str`-typed `CanId`
    /// to be concatenated into the template chain.
    pub(crate) fn desugar_format_with(
        &mut self,
        expr: CanId,
        expr_ty: TypeId,
        format_spec: Name,
        span: Span,
    ) -> CanId {
        // Primitives keep the FormatWith fast path verbatim.
        if is_primitive_format_ty(expr_ty) {
            return self.push(
                CanExpr::FormatWith {
                    expr,
                    spec: format_spec,
                },
                span,
                TypeId::STR,
            );
        }

        // Non-primitive: branch on explicit-Formattable-impl presence.
        let expr_idx = self.pool.resolve_fully(Idx::from_raw(expr_ty.raw()));
        if self.typed.has_formattable_impl(expr_idx) {
            self.desugar_user_format(expr, format_spec, span)
        } else {
            self.desugar_printable_format(expr, format_spec, span)
        }
    }

    /// Branch (a): explicit `impl T: Formattable` → `expr.format(spec)` with a
    /// synthesized `FormatSpec` struct argument. The receiver `expr` carries
    /// its own (non-primitive) type; the call result is `str`.
    fn desugar_user_format(&mut self, expr: CanId, format_spec: Name, span: Span) -> CanId {
        let spec_arg = self.synthesize_format_spec(format_spec, span);
        let args = self.arena.push_expr_list(&[spec_arg]);
        self.push(
            CanExpr::MethodCall {
                receiver: expr,
                method: self.fmt.format,
                args,
            },
            span,
            TypeId::STR,
        )
    }

    /// Branch (b): Printable-only → `FormatWith { expr.to_str(), spec }`.
    /// Re-routes through the `Idx::STR` runtime arm so width/align/fill/
    /// precision apply identically in both backends.
    fn desugar_printable_format(&mut self, expr: CanId, format_spec: Name, span: Span) -> CanId {
        let empty_args = self.arena.push_expr_list(&[]);
        let to_str = self.push(
            CanExpr::MethodCall {
                receiver: expr,
                method: self.name_to_str,
                args: empty_args,
            },
            span,
            TypeId::STR,
        );
        self.push(
            CanExpr::FormatWith {
                expr: to_str,
                spec: format_spec,
            },
            span,
            TypeId::STR,
        )
    }

    /// Build a `CanExpr::Struct { name: "FormatSpec", fields: [...] }` from the
    /// parsed format spec, mirroring `ori_eval`'s `build_format_spec_value` so
    /// both backends construct the identical shape. The parse here cannot fail:
    /// the type checker already validated this spec (`E2034`/`E2035`).
    fn synthesize_format_spec(&mut self, format_spec: Name, span: Span) -> CanId {
        let parsed =
            parse_format_spec(self.interner.lookup(format_spec)).unwrap_or(ParsedFormatSpec::EMPTY);

        // Field idxs are always present after a successful check (builtin
        // FormatSpec is registered in every module's pool); fall back to the
        // raw spec idx defensively if absent (synthetic/isolated modules).
        let types = self.typed.format_spec_types;

        let fill = self.opt_char_field(parsed.fill, types, span);
        let align = self.opt_align_field(parsed.align, types, span);
        let sign = self.opt_sign_field(parsed.sign, types, span);
        let width = self.opt_int_field(parsed.width, types, span);
        let precision = self.opt_int_field(parsed.precision, types, span);
        let format_type = self.opt_format_type_field(parsed.format_type, types, span);

        let fields = self.arena.push_fields(&[
            CanField {
                name: self.fmt.fill,
                value: fill,
            },
            CanField {
                name: self.fmt.align,
                value: align,
            },
            CanField {
                name: self.fmt.sign,
                value: sign,
            },
            CanField {
                name: self.fmt.width,
                value: width,
            },
            CanField {
                name: self.fmt.precision,
                value: precision,
            },
            CanField {
                name: self.fmt.format_type,
                value: format_type,
            },
        ]);

        let spec_ty = types.map_or(TypeId::ERROR, |t| TypeId::from_raw(t.spec.raw()));
        self.push(
            CanExpr::Struct {
                name: self.fmt.format_spec,
                fields,
            },
            span,
            spec_ty,
        )
    }

    /// `Option<char>` field: `Some('c')` or `None`.
    fn opt_char_field(
        &mut self,
        fill: Option<char>,
        types: Option<FormatSpecTypes>,
        span: Span,
    ) -> CanId {
        let opt_ty = opt_ty(types, |t| t.opt_char);
        match fill {
            Some(c) => {
                let inner = self.push(CanExpr::Char(c), span, TypeId::CHAR);
                self.push(CanExpr::Some(inner), span, opt_ty)
            }
            None => self.push(CanExpr::None, span, opt_ty),
        }
    }

    /// `Option<Alignment>` field: `Some(Left|Center|Right)` or `None`.
    fn opt_align_field(
        &mut self,
        align: Option<Align>,
        types: Option<FormatSpecTypes>,
        span: Span,
    ) -> CanId {
        let opt_ty = opt_ty(types, |t| t.opt_alignment);
        let variant_ty = variant_ty(types, |t| t.alignment);
        match align {
            Some(a) => {
                let variant = match a {
                    Align::Left => self.fmt.left,
                    Align::Center => self.fmt.center,
                    Align::Right => self.fmt.right,
                };
                let inner = self.push(CanExpr::Ident(variant), span, variant_ty);
                self.push(CanExpr::Some(inner), span, opt_ty)
            }
            None => self.push(CanExpr::None, span, opt_ty),
        }
    }

    /// `Option<Sign>` field: `Some(Plus|Minus|Space)` or `None`.
    fn opt_sign_field(
        &mut self,
        sign: Option<Sign>,
        types: Option<FormatSpecTypes>,
        span: Span,
    ) -> CanId {
        let opt_ty = opt_ty(types, |t| t.opt_sign);
        let variant_ty = variant_ty(types, |t| t.sign);
        match sign {
            Some(s) => {
                let variant = match s {
                    Sign::Plus => self.fmt.plus,
                    Sign::Minus => self.fmt.minus,
                    Sign::Space => self.fmt.space,
                };
                let inner = self.push(CanExpr::Ident(variant), span, variant_ty);
                self.push(CanExpr::Some(inner), span, opt_ty)
            }
            None => self.push(CanExpr::None, span, opt_ty),
        }
    }

    /// `Option<int>` field (`width` / `precision`): `Some(n)` or `None`.
    fn opt_int_field(
        &mut self,
        value: Option<usize>,
        types: Option<FormatSpecTypes>,
        span: Span,
    ) -> CanId {
        let opt_ty = opt_ty(types, |t| t.opt_int);
        match value {
            Some(w) => {
                let n = i64::try_from(w).unwrap_or(i64::MAX);
                let inner = self.push(CanExpr::Int(n), span, TypeId::INT);
                self.push(CanExpr::Some(inner), span, opt_ty)
            }
            None => self.push(CanExpr::None, span, opt_ty),
        }
    }

    /// `Option<FormatType>` field: `Some(Binary|...|Percent)` or `None`.
    fn opt_format_type_field(
        &mut self,
        ft: Option<FormatType>,
        types: Option<FormatSpecTypes>,
        span: Span,
    ) -> CanId {
        let opt_ty = opt_ty(types, |t| t.opt_format_type);
        let variant_ty = variant_ty(types, |t| t.format_type);
        match ft {
            Some(ft) => {
                let variant = match ft {
                    FormatType::Binary => self.fmt.binary,
                    FormatType::Octal => self.fmt.octal,
                    FormatType::Hex => self.fmt.hex,
                    FormatType::HexUpper => self.fmt.hex_upper,
                    FormatType::Exp => self.fmt.exp,
                    FormatType::ExpUpper => self.fmt.exp_upper,
                    FormatType::Fixed => self.fmt.fixed,
                    FormatType::Percent => self.fmt.percent,
                };
                let inner = self.push(CanExpr::Ident(variant), span, variant_ty);
                self.push(CanExpr::Some(inner), span, opt_ty)
            }
            None => self.push(CanExpr::None, span, opt_ty),
        }
    }
}

/// Whether `expr_ty` is a primitive that keeps the `ori_format_*` fast path.
/// `bool` formats through `ori_format_bool` (a `FormatWith` primitive arm).
fn is_primitive_format_ty(expr_ty: TypeId) -> bool {
    matches!(
        expr_ty,
        TypeId::INT | TypeId::FLOAT | TypeId::BOOL | TypeId::CHAR | TypeId::STR
    )
}

/// The `Option<_>` field type, or `TypeId::ERROR` when types are absent.
fn opt_ty(types: Option<FormatSpecTypes>, pick: impl Fn(&FormatSpecTypes) -> Idx) -> TypeId {
    types.map_or(TypeId::ERROR, |t| TypeId::from_raw(pick(&t).raw()))
}

/// The precise enum type (`Alignment`/`Sign`/`FormatType`) for a nullary
/// variant `Ident` node. The ARC lowerer reads this `ty` when emitting the
/// variant `Construct`, so it MUST be the enum idx, not the enclosing
/// `Option<_>` idx. Falls back to `TypeId::ERROR` when types are absent.
fn variant_ty(types: Option<FormatSpecTypes>, pick: impl Fn(&FormatSpecTypes) -> Idx) -> TypeId {
    types.map_or(TypeId::ERROR, |t| TypeId::from_raw(pick(&t).raw()))
}
