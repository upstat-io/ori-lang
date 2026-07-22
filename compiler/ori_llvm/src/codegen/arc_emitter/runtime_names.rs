use ori_ir::{Name, StringInterner};

/// Return ABI used by a binary string runtime helper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StringRuntimeReturnAbi {
    /// The caller provides storage for an `OriStr` result.
    StringSret,
    /// The helper returns its Boolean result directly.
    BoolDirect,
}

/// Pre-interned list runtime symbols used by call emission.
#[derive(Clone, Copy, Debug)]
pub(super) struct ListRtNames {
    /// Appends one element to a list builder.
    pub(super) push: Name,
    /// Allocates a list builder.
    pub(super) new: Name,
    /// Finalizes a list builder into a list value.
    pub(super) take: Name,
    /// Releases an unfinished list builder.
    pub(super) free: Name,
    /// Produces the suffix excluded by a list-rest pattern.
    pub(super) slice_drop: Name,
}

impl ListRtNames {
    /// Interns every list symbol once for identity-based dispatch.
    pub(super) fn from_interner(interner: &StringInterner) -> Self {
        Self {
            push: interner.intern("ori_list_push"),
            new: interner.intern("ori_list_new"),
            take: interner.intern("ori_list_take"),
            free: interner.intern("ori_list_free"),
            slice_drop: interner.intern("ori_list_slice_drop"),
        }
    }
}

/// Pre-interned format runtime symbols and their typed targets.
#[derive(Clone, Copy, Debug)]
pub(super) struct FormatRtNames {
    int: Name,
    float: Name,
    str: Name,
    bool: Name,
    char: Name,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FormatRuntimeTarget {
    /// Signed integer formatting.
    Int,
    /// Floating-point formatting.
    Float,
    /// String formatting.
    Str,
    /// Boolean formatting.
    Bool,
    /// Character formatting.
    Char,
}

impl FormatRuntimeTarget {
    /// Returns the runtime symbol implementing this typed formatting target.
    pub(super) const fn symbol(self) -> &'static str {
        match self {
            Self::Int => "ori_format_int",
            Self::Float => "ori_format_float",
            Self::Str => "ori_format_str",
            Self::Bool => "ori_format_bool",
            Self::Char => "ori_format_char",
        }
    }

    /// Reports whether the helper consumes its formatted value through a pointer.
    pub(super) const fn value_needs_pointer(self) -> bool {
        matches!(self, Self::Str)
    }
}

impl FormatRtNames {
    /// Interns every formatting symbol once for identity-based dispatch.
    pub(super) fn from_interner(interner: &StringInterner) -> Self {
        Self {
            int: interner.intern("ori_format_int"),
            float: interner.intern("ori_format_float"),
            str: interner.intern("ori_format_str"),
            bool: interner.intern("ori_format_bool"),
            char: interner.intern("ori_format_char"),
        }
    }

    /// Resolve a callee name to its runtime target and value ABI.
    pub(super) fn lookup(&self, callee: Name) -> Option<FormatRuntimeTarget> {
        if callee == self.int {
            Some(FormatRuntimeTarget::Int)
        } else if callee == self.float {
            Some(FormatRuntimeTarget::Float)
        } else if callee == self.str {
            Some(FormatRuntimeTarget::Str)
        } else if callee == self.bool {
            Some(FormatRuntimeTarget::Bool)
        } else if callee == self.char {
            Some(FormatRuntimeTarget::Char)
        } else {
            None
        }
    }
}
