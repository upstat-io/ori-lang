//! Built-in type registration (Pass 0a).
//!
//! Registers compiler-provided types that user code may reference:
//! `Ordering`, `TraceEntry`, `Alignment`, `Sign`, `FormatType`, `FormatSpec`.
//!
//! Primitive types (int, str, etc.) are pre-interned in the Pool and don't need
//! registration here.

use ori_ir::{Name, Span};

use crate::{EnumVariant, FieldDef, Idx, ModuleChecker, VariantDef, VariantFields, Visibility};

/// Register built-in types that user code may reference.
///
/// Currently registers:
/// - `Ordering` enum (Less, Equal, Greater)
/// - `TraceEntry` struct (function, file, line, column) — for Traceable trait
/// - `Alignment` enum (Left, Center, Right) — for Formattable trait
/// - `Sign` enum (Plus, Minus, Space) — for Formattable trait
/// - `FormatType` enum (Binary, Octal, Hex, ...) — for Formattable trait
/// - `FormatSpec` struct (fill, align, sign, ...) — for Formattable trait
pub fn register_builtin_types(checker: &mut ModuleChecker<'_>) {
    register_ordering_type(checker);
    register_trace_entry_type(checker);
    register_alignment_type(checker);
    register_sign_type(checker);
    register_format_type_type(checker);
    register_format_spec_type(checker);
}

/// Register the `Ordering` enum (Less, Equal, Greater).
fn register_ordering_type(checker: &mut ModuleChecker<'_>) {
    let ordering_name = checker.interner().intern("Ordering");
    let less_name = checker.interner().intern("Less");
    let equal_name = checker.interner().intern("Equal");
    let greater_name = checker.interner().intern("Greater");

    let ordering_idx = Idx::ORDERING;

    let variants = vec![
        VariantDef {
            name: less_name,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        },
        VariantDef {
            name: equal_name,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        },
        VariantDef {
            name: greater_name,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        },
    ];

    // Create Pool enum entry for Ordering (used by TypeRegistry for variant definitions).
    // No set_resolution: Idx::ORDERING is a pre-interned primitive and should not have
    // a resolution entry. Variant lookup returns Idx::ORDERING directly.
    let pool_variants = vec![
        EnumVariant {
            name: less_name,
            field_types: vec![],
        },
        EnumVariant {
            name: equal_name,
            field_types: vec![],
        },
        EnumVariant {
            name: greater_name,
            field_types: vec![],
        },
    ];
    let _enum_idx = checker.pool_mut().enum_type(ordering_name, &pool_variants);

    checker.type_registry_mut().register_enum(
        ordering_name,
        ordering_idx,
        vec![], // No type params
        variants,
        Span::DUMMY,
        Visibility::Public,
    );
}

/// Register the `TraceEntry` struct for the Traceable trait.
///
/// Fields: `function: str`, `file: str`, `line: int`, `column: int`.
/// This is a compiler-provided struct, not user-defined. Registered so that
/// trait method signatures referencing `TraceEntry` resolve correctly.
fn register_trace_entry_type(checker: &mut ModuleChecker<'_>) {
    let te_name = checker.interner().intern("TraceEntry");
    let fn_name = checker.interner().intern("function");
    let file_name = checker.interner().intern("file");
    let line_name = checker.interner().intern("line");
    let column_name = checker.interner().intern("column");

    // Create named index via Pool (dynamic allocation)
    let named_idx = checker.pool_mut().named(te_name);

    // Create Pool struct entry with field name+type pairs
    let pool_fields = [
        (fn_name, Idx::STR),
        (file_name, Idx::STR),
        (line_name, Idx::INT),
        (column_name, Idx::INT),
    ];
    let struct_idx = checker.pool_mut().struct_type(te_name, &pool_fields);
    checker.pool_mut().set_resolution(named_idx, struct_idx);

    // Register in TypeRegistry for field access and type checking
    let field_defs = vec![
        FieldDef {
            name: fn_name,
            ty: Idx::STR,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
        FieldDef {
            name: file_name,
            ty: Idx::STR,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
        FieldDef {
            name: line_name,
            ty: Idx::INT,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
        FieldDef {
            name: column_name,
            ty: Idx::INT,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
    ];

    checker.type_registry_mut().register_struct(
        te_name,
        named_idx,
        vec![], // No type params
        field_defs,
        Span::DUMMY,
        Visibility::Public,
    );
}

/// Register the `Alignment` enum (Left, Center, Right) for the Formattable trait.
fn register_alignment_type(checker: &mut ModuleChecker<'_>) {
    let type_name = checker.interner().intern("Alignment");
    let left = checker.interner().intern("Left");
    let center = checker.interner().intern("Center");
    let right = checker.interner().intern("Right");

    let named_idx = checker.pool_mut().named(type_name);

    let pool_variants = vec![
        EnumVariant {
            name: left,
            field_types: vec![],
        },
        EnumVariant {
            name: center,
            field_types: vec![],
        },
        EnumVariant {
            name: right,
            field_types: vec![],
        },
    ];
    let enum_idx = checker.pool_mut().enum_type(type_name, &pool_variants);
    checker.pool_mut().set_resolution(named_idx, enum_idx);

    let variants = vec![
        VariantDef {
            name: left,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        },
        VariantDef {
            name: center,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        },
        VariantDef {
            name: right,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        },
    ];

    checker.type_registry_mut().register_enum(
        type_name,
        named_idx,
        vec![],
        variants,
        Span::DUMMY,
        Visibility::Public,
    );
}

/// Register the `Sign` enum (Plus, Minus, Space) for the Formattable trait.
fn register_sign_type(checker: &mut ModuleChecker<'_>) {
    let type_name = checker.interner().intern("Sign");
    let plus = checker.interner().intern("Plus");
    let minus = checker.interner().intern("Minus");
    let space = checker.interner().intern("Space");

    let named_idx = checker.pool_mut().named(type_name);

    let pool_variants = vec![
        EnumVariant {
            name: plus,
            field_types: vec![],
        },
        EnumVariant {
            name: minus,
            field_types: vec![],
        },
        EnumVariant {
            name: space,
            field_types: vec![],
        },
    ];
    let enum_idx = checker.pool_mut().enum_type(type_name, &pool_variants);
    checker.pool_mut().set_resolution(named_idx, enum_idx);

    let variants = vec![
        VariantDef {
            name: plus,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        },
        VariantDef {
            name: minus,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        },
        VariantDef {
            name: space,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        },
    ];

    checker.type_registry_mut().register_enum(
        type_name,
        named_idx,
        vec![],
        variants,
        Span::DUMMY,
        Visibility::Public,
    );
}

/// Register the `FormatType` enum for the Formattable trait.
///
/// Variants: `Binary`, `Octal`, `Hex`, `HexUpper`, `Exp`, `ExpUpper`, `Fixed`, `Percent`.
fn register_format_type_type(checker: &mut ModuleChecker<'_>) {
    let type_name = checker.interner().intern("FormatType");

    let variant_names: Vec<Name> = [
        "Binary", "Octal", "Hex", "HexUpper", "Exp", "ExpUpper", "Fixed", "Percent",
    ]
    .iter()
    .map(|s| checker.interner().intern(s))
    .collect();

    let named_idx = checker.pool_mut().named(type_name);

    let pool_variants: Vec<EnumVariant> = variant_names
        .iter()
        .map(|&name| EnumVariant {
            name,
            field_types: vec![],
        })
        .collect();
    let enum_idx = checker.pool_mut().enum_type(type_name, &pool_variants);
    checker.pool_mut().set_resolution(named_idx, enum_idx);

    let variants: Vec<VariantDef> = variant_names
        .iter()
        .map(|&name| VariantDef {
            name,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        })
        .collect();

    checker.type_registry_mut().register_enum(
        type_name,
        named_idx,
        vec![],
        variants,
        Span::DUMMY,
        Visibility::Public,
    );
}

/// Register the `FormatSpec` struct for the Formattable trait.
///
/// Fields: fill (Option<char>), align (Option<Alignment>), sign (Option<Sign>),
/// width (`Option<int>`), precision (`Option<int>`), `format_type` (`Option<FormatType>`).
fn register_format_spec_type(checker: &mut ModuleChecker<'_>) {
    let spec_name = checker.interner().intern("FormatSpec");
    let fill_name = checker.interner().intern("fill");
    let align_name = checker.interner().intern("align");
    let sign_name = checker.interner().intern("sign");
    let width_name = checker.interner().intern("width");
    let precision_name = checker.interner().intern("precision");
    let format_type_name = checker.interner().intern("format_type");

    // Build Option<T> types for each field
    let opt_char = checker.pool_mut().option(Idx::CHAR);
    let alignment_name = checker.interner().intern("Alignment");
    let alignment_idx = checker.pool_mut().named(alignment_name);
    let opt_alignment = checker.pool_mut().option(alignment_idx);
    let sign_type_name = checker.interner().intern("Sign");
    let sign_idx = checker.pool_mut().named(sign_type_name);
    let opt_sign = checker.pool_mut().option(sign_idx);
    let opt_int = checker.pool_mut().option(Idx::INT);
    let ft_name = checker.interner().intern("FormatType");
    let ft_idx = checker.pool_mut().named(ft_name);
    let opt_ft = checker.pool_mut().option(ft_idx);

    let named_idx = checker.pool_mut().named(spec_name);

    let pool_fields = [
        (fill_name, opt_char),
        (align_name, opt_alignment),
        (sign_name, opt_sign),
        (width_name, opt_int),
        (precision_name, opt_int),
        (format_type_name, opt_ft),
    ];
    let struct_idx = checker.pool_mut().struct_type(spec_name, &pool_fields);
    checker.pool_mut().set_resolution(named_idx, struct_idx);

    let field_defs = vec![
        FieldDef {
            name: fill_name,
            ty: opt_char,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
        FieldDef {
            name: align_name,
            ty: opt_alignment,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
        FieldDef {
            name: sign_name,
            ty: opt_sign,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
        FieldDef {
            name: width_name,
            ty: opt_int,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
        FieldDef {
            name: precision_name,
            ty: opt_int,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
        FieldDef {
            name: format_type_name,
            ty: opt_ft,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
    ];

    checker.type_registry_mut().register_struct(
        spec_name,
        named_idx,
        vec![],
        field_defs,
        Span::DUMMY,
        Visibility::Public,
    );
}
