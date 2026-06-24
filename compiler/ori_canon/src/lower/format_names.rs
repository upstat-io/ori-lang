//! Pre-interned names for non-primitive `{expr:spec}` format desugaring.

use ori_ir::Name;

/// Pre-interned names for synthesizing the `Formattable.format(self:, spec:)`
/// `MethodCall` and its `FormatSpec` struct argument during non-primitive
/// `{expr:spec}` desugaring (`desugar/format.rs`).
///
/// Field names + variant names mirror `ori_eval`'s `FormatNames`
/// (`interpreter/interned_names.rs`) so the synthesized `CanExpr::Struct`
/// constructs the identical `FormatSpec` shape `build_format_spec_value`
/// produces — keeping interpreter and LLVM dispatch in parity.
pub(crate) struct FormatDesugarNames {
    pub(crate) format: Name,
    pub(crate) format_spec: Name,
    // FormatSpec fields (struct field names).
    pub(crate) fill: Name,
    pub(crate) align: Name,
    pub(crate) sign: Name,
    pub(crate) width: Name,
    pub(crate) precision: Name,
    pub(crate) format_type: Name,
    // Alignment variants.
    pub(crate) left: Name,
    pub(crate) center: Name,
    pub(crate) right: Name,
    // Sign variants.
    pub(crate) plus: Name,
    pub(crate) minus: Name,
    pub(crate) space: Name,
    // FormatType variants.
    pub(crate) binary: Name,
    pub(crate) octal: Name,
    pub(crate) hex: Name,
    pub(crate) hex_upper: Name,
    pub(crate) exp: Name,
    pub(crate) exp_upper: Name,
    pub(crate) fixed: Name,
    pub(crate) percent: Name,
}

impl FormatDesugarNames {
    pub(super) fn new(interner: &ori_ir::StringInterner) -> Self {
        Self {
            format: interner.intern("format"),
            format_spec: interner.intern("FormatSpec"),
            fill: interner.intern("fill"),
            align: interner.intern("align"),
            sign: interner.intern("sign"),
            width: interner.intern("width"),
            precision: interner.intern("precision"),
            format_type: interner.intern("format_type"),
            left: interner.intern("Left"),
            center: interner.intern("Center"),
            right: interner.intern("Right"),
            plus: interner.intern("Plus"),
            minus: interner.intern("Minus"),
            space: interner.intern("Space"),
            binary: interner.intern("Binary"),
            octal: interner.intern("Octal"),
            hex: interner.intern("Hex"),
            hex_upper: interner.intern("HexUpper"),
            exp: interner.intern("Exp"),
            exp_upper: interner.intern("ExpUpper"),
            fixed: interner.intern("Fixed"),
            percent: interner.intern("Percent"),
        }
    }
}
