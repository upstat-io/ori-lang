use ori_ir::StringInterner;
use ori_patterns::ControlAction;

use crate::methods::BuiltinMethodNames;

use super::*;

#[test]
fn char_to_byte_accepts_ascii_boundary() {
    let interner = StringInterner::new();
    let names = BuiltinMethodNames::new(&interner);
    let ctx = DispatchCtx {
        names: &names,
        interner: &interner,
    };

    let Ok(result) = dispatch_char_method(Value::Char('\u{7f}'), names.to_byte, vec![], &ctx)
    else {
        panic!("the highest ASCII code point should convert to byte");
    };

    assert_eq!(result, Value::Byte(0x7f));
}

#[test]
fn char_to_byte_rejects_non_ascii_with_actionable_error() {
    let interner = StringInterner::new();
    let names = BuiltinMethodNames::new(&interner);
    let ctx = DispatchCtx {
        names: &names,
        interner: &interner,
    };

    let Err(ControlAction::Error(error)) =
        dispatch_char_method(Value::Char('\u{80}'), names.to_byte, vec![], &ctx)
    else {
        panic!("non-ASCII code points must produce a runtime error");
    };
    let error = &error.message;

    assert!(error.contains("accepts only ASCII (U+0000..U+007F)"));
    assert!(error.contains("call `.to_int()` on the character"));
    assert!(error.contains("to preserve its Unicode code point"));
    assert!(error.contains("Spec: Clause 8.11.3"));
}

#[test]
fn byte_shift_uses_one_checked_boundary_path_for_both_directions() {
    let Ok(left) = byte_shift(0b0000_0011, 2, u8::checked_shl) else {
        panic!("in-range left shift should succeed");
    };
    let Ok(right) = byte_shift(0b0000_1100, 2, u8::checked_shr) else {
        panic!("in-range right shift should succeed");
    };
    assert_eq!(left, Value::Byte(0b0000_1100));
    assert_eq!(right, Value::Byte(0b0000_0011));

    for operation in [
        u8::checked_shl as fn(u8, u32) -> Option<u8>,
        u8::checked_shr,
    ] {
        for shift in [-1, 8] {
            let Err(ControlAction::Error(error)) = byte_shift(1, shift, operation) else {
                panic!("out-of-range shift {shift} should fail");
            };
            assert_eq!(
                error.message,
                format!("shift amount {shift} out of range (0-7)")
            );
        }
    }
}
