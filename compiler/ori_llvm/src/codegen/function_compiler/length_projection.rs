//! LLVM identity projection for compiled length-only clones.

use ori_ir::{Name, StringInterner};

/// Mint the private clone identity after the compiled plan qualifies a callee.
pub(super) fn projection_name(interner: &StringInterner, callee: Name) -> Name {
    interner.intern(&format!("{}$length_only", interner.lookup(callee)))
}
