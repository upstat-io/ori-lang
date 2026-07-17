use ori_ir::StringInterner;

use crate::methods::BuiltinMethodNames;

use super::*;

#[test]
fn string_hash_dispatch_uses_canonical_fnv1a() {
    let interner = StringInterner::new();
    let names = BuiltinMethodNames::new(&interner);
    let ctx = DispatchCtx {
        names: &names,
        interner: &interner,
    };

    let Ok(actual) = dispatch_string_method(Value::string("a"), names.hash, vec![], &ctx) else {
        panic!("str.hash should succeed");
    };
    assert_eq!(actual, Value::int(fnv1a_hash(b"a")));
}
