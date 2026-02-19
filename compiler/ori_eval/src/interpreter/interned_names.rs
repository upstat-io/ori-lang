//! Pre-interned name structs for hot-path dispatch.
//!
//! These structs hold `Name` values interned once at `Interpreter` construction
//! so that hot-path operations (method dispatch, print routing, operator lookup,
//! format spec construction) use `Name` comparison (`u32 == u32`) instead of
//! repeated hash lookups via `interner.intern("...")`.
//!
//! Extracted from `interpreter/mod.rs` to keep that file under the 500-line limit.

use ori_ir::{Name, StringInterner};

/// Pre-interned print method names for print dispatch in `eval_method_call`.
///
/// These names are interned once at Interpreter construction so that
/// `eval_method_call` can check for print methods via `Name` comparison
/// (a single `u32 == u32` check) instead of string lookup.
#[derive(Clone, Copy)]
pub(crate) struct PrintNames {
    pub(crate) print: Name,
    pub(crate) println: Name,
    pub(crate) builtin_print: Name,
    pub(crate) builtin_println: Name,
}

impl PrintNames {
    /// Pre-intern all print method names.
    pub(crate) fn new(interner: &StringInterner) -> Self {
        Self {
            print: interner.intern("print"),
            println: interner.intern("println"),
            builtin_print: interner.intern("__builtin_print"),
            builtin_println: interner.intern("__builtin_println"),
        }
    }
}

/// Pre-interned type names for hot-path method dispatch.
///
/// These names are interned once at Interpreter construction to avoid
/// repeated hash lookups in `get_value_type_name()`, which is called
/// on every method dispatch.
#[derive(Clone, Copy)]
pub(crate) struct TypeNames {
    pub(crate) range: Name,
    pub(crate) int: Name,
    pub(crate) float: Name,
    pub(crate) bool_: Name,
    pub(crate) str_: Name,
    pub(crate) char_: Name,
    pub(crate) byte: Name,
    pub(crate) void: Name,
    pub(crate) duration: Name,
    pub(crate) size: Name,
    pub(crate) ordering: Name,
    pub(crate) list: Name,
    pub(crate) map: Name,
    pub(crate) set: Name,
    pub(crate) tuple: Name,
    pub(crate) option: Name,
    pub(crate) result: Name,
    pub(crate) function: Name,
    pub(crate) function_val: Name,
    pub(crate) iterator: Name,
    pub(crate) module: Name,
    pub(crate) error: Name,
}

impl TypeNames {
    /// Pre-intern all primitive type names.
    pub(crate) fn new(interner: &StringInterner) -> Self {
        Self {
            range: interner.intern("range"),
            int: interner.intern("int"),
            float: interner.intern("float"),
            bool_: interner.intern("bool"),
            str_: interner.intern("str"),
            char_: interner.intern("char"),
            byte: interner.intern("byte"),
            void: interner.intern("void"),
            duration: interner.intern("Duration"),
            size: interner.intern("Size"),
            ordering: interner.intern("Ordering"),
            list: interner.intern("list"),
            map: interner.intern("map"),
            set: interner.intern("Set"),
            tuple: interner.intern("tuple"),
            option: interner.intern("Option"),
            result: interner.intern("Result"),
            function: interner.intern("function"),
            function_val: interner.intern("function_val"),
            iterator: interner.intern("Iterator"),
            module: interner.intern("module"),
            error: interner.intern("error"),
        }
    }
}

/// Pre-interned property names for `FunctionExp` prop dispatch.
///
/// These names are interned once at Interpreter construction so that
/// `find_prop_value` and `find_prop_can_id` can compare `Name` values
/// directly (single `u32 == u32`) instead of string lookup per prop.
#[derive(Clone, Copy)]
pub(crate) struct PropNames {
    pub(crate) msg: Name,
    pub(crate) operation: Name,
    pub(crate) tasks: Name,
    pub(crate) acquire: Name,
    pub(crate) action: Name,
    pub(crate) release: Name,
    pub(crate) expr: Name,
    pub(crate) condition: Name,
    pub(crate) base: Name,
    pub(crate) step: Name,
    pub(crate) memo: Name,
}

impl PropNames {
    /// Pre-intern all `FunctionExp` property names.
    pub(crate) fn new(interner: &StringInterner) -> Self {
        Self {
            msg: interner.intern("msg"),
            operation: interner.intern("operation"),
            tasks: interner.intern("tasks"),
            acquire: interner.intern("acquire"),
            action: interner.intern("action"),
            release: interner.intern("release"),
            expr: interner.intern("expr"),
            condition: interner.intern("condition"),
            base: interner.intern("base"),
            step: interner.intern("step"),
            memo: interner.intern("memo"),
        }
    }
}

/// Pre-interned operator trait method names for user-defined operator dispatch.
///
/// These names are interned once at Interpreter construction so that
/// `eval_can_binary` and `eval_can_unary` can dispatch user-defined operator
/// trait methods via `Name` comparison instead of re-interning on every call.
#[derive(Clone, Copy)]
pub(crate) struct OpNames {
    pub(crate) add: Name,
    pub(crate) subtract: Name,
    pub(crate) multiply: Name,
    pub(crate) divide: Name,
    pub(crate) floor_divide: Name,
    pub(crate) remainder: Name,
    pub(crate) bit_and: Name,
    pub(crate) bit_or: Name,
    pub(crate) bit_xor: Name,
    pub(crate) shift_left: Name,
    pub(crate) shift_right: Name,
    pub(crate) negate: Name,
    pub(crate) not: Name,
    pub(crate) bit_not: Name,
    pub(crate) index: Name,
}

impl OpNames {
    /// Pre-intern all operator trait method names.
    pub(crate) fn new(interner: &StringInterner) -> Self {
        Self {
            add: interner.intern("add"),
            subtract: interner.intern("subtract"),
            multiply: interner.intern("multiply"),
            divide: interner.intern("divide"),
            floor_divide: interner.intern("floor_divide"),
            remainder: interner.intern("remainder"),
            bit_and: interner.intern("bit_and"),
            bit_or: interner.intern("bit_or"),
            bit_xor: interner.intern("bit_xor"),
            shift_left: interner.intern("shift_left"),
            shift_right: interner.intern("shift_right"),
            negate: interner.intern("negate"),
            not: interner.intern("not"),
            bit_not: interner.intern("bit_not"),
            index: interner.intern("index"),
        }
    }
}

/// Pre-interned names for `FormatSpec` value construction.
///
/// These names are interned once at Interpreter construction so that
/// `build_format_spec_value` avoids repeated hash lookups when constructing
/// the Ori-side `FormatSpec` struct for user `Formattable::format()` calls.
#[derive(Clone, Copy)]
pub(crate) struct FormatNames {
    pub(crate) format_spec: Name,
    pub(crate) fill: Name,
    pub(crate) align: Name,
    pub(crate) sign: Name,
    pub(crate) width: Name,
    pub(crate) precision: Name,
    pub(crate) format_type: Name,
    pub(crate) alignment: Name,
    pub(crate) left: Name,
    pub(crate) center: Name,
    pub(crate) right: Name,
    pub(crate) sign_type: Name,
    pub(crate) plus: Name,
    pub(crate) minus: Name,
    pub(crate) space: Name,
    pub(crate) ft_type: Name,
    pub(crate) binary: Name,
    pub(crate) octal: Name,
    pub(crate) hex: Name,
    pub(crate) hex_upper: Name,
    pub(crate) exp: Name,
    pub(crate) exp_upper: Name,
    pub(crate) fixed: Name,
    pub(crate) percent: Name,
    pub(crate) format: Name,
}

impl FormatNames {
    /// Pre-intern all format-related names.
    pub(crate) fn new(interner: &StringInterner) -> Self {
        Self {
            format_spec: interner.intern("FormatSpec"),
            fill: interner.intern("fill"),
            align: interner.intern("align"),
            sign: interner.intern("sign"),
            width: interner.intern("width"),
            precision: interner.intern("precision"),
            format_type: interner.intern("format_type"),
            alignment: interner.intern("Alignment"),
            left: interner.intern("Left"),
            center: interner.intern("Center"),
            right: interner.intern("Right"),
            sign_type: interner.intern("Sign"),
            plus: interner.intern("Plus"),
            minus: interner.intern("Minus"),
            space: interner.intern("Space"),
            ft_type: interner.intern("FormatType"),
            binary: interner.intern("Binary"),
            octal: interner.intern("Octal"),
            hex: interner.intern("Hex"),
            hex_upper: interner.intern("HexUpper"),
            exp: interner.intern("Exp"),
            exp_upper: interner.intern("ExpUpper"),
            fixed: interner.intern("Fixed"),
            percent: interner.intern("Percent"),
            format: interner.intern("format"),
        }
    }
}
