//! Pre-interned method names for builtin method dispatch.
//!
//! `BuiltinMethodNames` is constructed once per Interpreter and enables
//! `u32 == u32` comparison instead of string matching during dispatch.

use ori_ir::{Name, StringInterner};

/// Pre-interned method names for builtin method dispatch.
///
/// Constructed once per Interpreter (like `TypeNames`, `PrintNames`, `PropNames`).
/// Each field holds the interned `Name` for a builtin method, enabling
/// `u32 == u32` comparison instead of string matching during dispatch.
///
/// This eliminates the de-interning that previously happened in
/// `dispatch_builtin_method` on every builtin method call.
#[derive(Clone, Copy)]
pub(crate) struct BuiltinMethodNames {
    // Common trait methods (used across many types)
    pub(crate) add: Name,
    pub(crate) sub: Name,
    pub(crate) mul: Name,
    pub(crate) div: Name,
    pub(crate) rem: Name,
    pub(crate) neg: Name,
    pub(crate) compare: Name,
    pub(crate) equals: Name,
    pub(crate) clone_: Name,
    pub(crate) to_str: Name,
    pub(crate) debug: Name,
    pub(crate) hash: Name,
    pub(crate) contains: Name,
    pub(crate) len: Name,
    pub(crate) length: Name,
    pub(crate) is_empty: Name,
    pub(crate) not: Name,
    pub(crate) unwrap: Name,
    pub(crate) concat: Name,

    // Numeric (int-specific)
    pub(crate) floor_div: Name,
    pub(crate) bit_and: Name,
    pub(crate) bit_or: Name,
    pub(crate) bit_xor: Name,
    pub(crate) bit_not: Name,
    pub(crate) shl: Name,
    pub(crate) shr: Name,

    // Collection (string-specific)
    pub(crate) to_uppercase: Name,
    pub(crate) to_lowercase: Name,
    pub(crate) trim: Name,
    pub(crate) starts_with: Name,
    pub(crate) ends_with: Name,
    pub(crate) escape: Name,
    pub(crate) replace: Name,
    pub(crate) repeat: Name,
    pub(crate) split: Name,
    pub(crate) substring: Name,

    // Collection (list-specific)
    pub(crate) first: Name,
    pub(crate) last: Name,
    pub(crate) pop: Name,
    pub(crate) push: Name,
    pub(crate) set: Name,
    pub(crate) slice: Name,
    pub(crate) sort: Name,
    pub(crate) sort_stable: Name,
    pub(crate) take: Name,
    pub(crate) skip: Name,
    pub(crate) drop_: Name,

    // Collection (map-specific)
    pub(crate) contains_key: Name,
    pub(crate) get: Name,
    pub(crate) keys: Name,
    pub(crate) values: Name,
    pub(crate) entries: Name,

    // Collection (map + set shared)
    pub(crate) insert: Name,
    pub(crate) remove: Name,
    pub(crate) union: Name,
    pub(crate) intersection: Name,
    pub(crate) difference: Name,
    pub(crate) to_list: Name,

    // Variant (Option-specific)
    pub(crate) unwrap_or: Name,
    pub(crate) is_some: Name,
    pub(crate) is_none: Name,
    pub(crate) ok_or: Name,

    // Variant (Result-specific)
    pub(crate) is_ok: Name,
    pub(crate) is_err: Name,
    pub(crate) unwrap_err: Name,

    // Ordering
    pub(crate) is_less: Name,
    pub(crate) is_equal: Name,
    pub(crate) is_greater: Name,
    pub(crate) is_less_or_equal: Name,
    pub(crate) is_greater_or_equal: Name,
    pub(crate) reverse: Name,
    pub(crate) then: Name,

    // Type names for associated function dispatch
    pub(crate) duration: Name,
    pub(crate) size: Name,

    // Duration/Size operator aliases
    pub(crate) subtract: Name,
    pub(crate) multiply: Name,
    pub(crate) divide: Name,
    pub(crate) remainder: Name,
    pub(crate) negate: Name,

    // Duration accessors
    pub(crate) nanoseconds: Name,
    pub(crate) microseconds: Name,
    pub(crate) milliseconds: Name,
    pub(crate) seconds: Name,
    pub(crate) minutes: Name,
    pub(crate) hours: Name,

    // Size accessors
    pub(crate) bytes: Name,
    pub(crate) kilobytes: Name,
    pub(crate) megabytes: Name,
    pub(crate) gigabytes: Name,
    pub(crate) terabytes: Name,

    // Iterator
    pub(crate) iter: Name,

    // Conversion (Into trait)
    pub(crate) into: Name,

    // Traceable trait (Error type)
    pub(crate) trace: Name,
    pub(crate) trace_entries: Name,
    pub(crate) has_trace: Name,
    pub(crate) with_trace: Name,
    pub(crate) message: Name,
}

impl BuiltinMethodNames {
    /// Pre-intern all builtin method names.
    pub(crate) fn new(interner: &StringInterner) -> Self {
        Self {
            // Common trait methods
            add: interner.intern("add"),
            sub: interner.intern("sub"),
            mul: interner.intern("mul"),
            div: interner.intern("div"),
            rem: interner.intern("rem"),
            neg: interner.intern("neg"),
            compare: interner.intern("compare"),
            equals: interner.intern("equals"),
            clone_: interner.intern("clone"),
            to_str: interner.intern("to_str"),
            debug: interner.intern("debug"),
            hash: interner.intern("hash"),
            contains: interner.intern("contains"),
            len: interner.intern("len"),
            length: interner.intern("length"),
            is_empty: interner.intern("is_empty"),
            not: interner.intern("not"),
            unwrap: interner.intern("unwrap"),
            concat: interner.intern("concat"),
            // Numeric
            floor_div: interner.intern("floor_div"),
            bit_and: interner.intern("bit_and"),
            bit_or: interner.intern("bit_or"),
            bit_xor: interner.intern("bit_xor"),
            bit_not: interner.intern("bit_not"),
            shl: interner.intern("shl"),
            shr: interner.intern("shr"),
            // String
            to_uppercase: interner.intern("to_uppercase"),
            to_lowercase: interner.intern("to_lowercase"),
            trim: interner.intern("trim"),
            starts_with: interner.intern("starts_with"),
            ends_with: interner.intern("ends_with"),
            escape: interner.intern("escape"),
            replace: interner.intern("replace"),
            repeat: interner.intern("repeat"),
            split: interner.intern("split"),
            substring: interner.intern("substring"),
            // List
            first: interner.intern("first"),
            last: interner.intern("last"),
            pop: interner.intern("pop"),
            push: interner.intern("push"),
            set: interner.intern("set"),
            slice: interner.intern("slice"),
            sort: interner.intern("sort"),
            sort_stable: interner.intern("sort_stable"),
            take: interner.intern("take"),
            skip: interner.intern("skip"),
            drop_: interner.intern("drop"),
            // Map
            contains_key: interner.intern("contains_key"),
            get: interner.intern("get"),
            keys: interner.intern("keys"),
            values: interner.intern("values"),
            entries: interner.intern("entries"),
            // Map + Set shared
            insert: interner.intern("insert"),
            remove: interner.intern("remove"),
            union: interner.intern("union"),
            intersection: interner.intern("intersection"),
            difference: interner.intern("difference"),
            to_list: interner.intern("to_list"),
            // Option
            unwrap_or: interner.intern("unwrap_or"),
            is_some: interner.intern("is_some"),
            is_none: interner.intern("is_none"),
            ok_or: interner.intern("ok_or"),
            // Result
            is_ok: interner.intern("is_ok"),
            is_err: interner.intern("is_err"),
            unwrap_err: interner.intern("unwrap_err"),
            // Ordering
            is_less: interner.intern("is_less"),
            is_equal: interner.intern("is_equal"),
            is_greater: interner.intern("is_greater"),
            is_less_or_equal: interner.intern("is_less_or_equal"),
            is_greater_or_equal: interner.intern("is_greater_or_equal"),
            reverse: interner.intern("reverse"),
            then: interner.intern("then"),
            // Type names
            duration: interner.intern("Duration"),
            size: interner.intern("Size"),
            // Duration/Size aliases
            subtract: interner.intern("subtract"),
            multiply: interner.intern("multiply"),
            divide: interner.intern("divide"),
            remainder: interner.intern("remainder"),
            negate: interner.intern("negate"),
            // Duration accessors
            nanoseconds: interner.intern("nanoseconds"),
            microseconds: interner.intern("microseconds"),
            milliseconds: interner.intern("milliseconds"),
            seconds: interner.intern("seconds"),
            minutes: interner.intern("minutes"),
            hours: interner.intern("hours"),
            // Size accessors
            bytes: interner.intern("bytes"),
            kilobytes: interner.intern("kilobytes"),
            megabytes: interner.intern("megabytes"),
            gigabytes: interner.intern("gigabytes"),
            terabytes: interner.intern("terabytes"),
            // Iterator
            iter: interner.intern("iter"),
            // Conversion
            into: interner.intern("into"),
            // Traceable
            trace: interner.intern("trace"),
            trace_entries: interner.intern("trace_entries"),
            has_trace: interner.intern("has_trace"),
            with_trace: interner.intern("with_trace"),
            message: interner.intern("message"),
        }
    }
}

/// Context for builtin method dispatch.
///
/// Bundles pre-interned method names and the string interner to keep
/// sub-dispatch function signatures at 4 parameters. The interner is
/// only used on cold paths (error messages, deep comparisons).
#[derive(Clone, Copy)]
pub(crate) struct DispatchCtx<'a> {
    pub(crate) names: &'a BuiltinMethodNames,
    pub(crate) interner: &'a StringInterner,
}
