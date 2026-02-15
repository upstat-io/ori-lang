use super::*;

#[test]
fn test_simple_cx_types() {
    let context = Context::create();
    let scx = SimpleCx::new(&context, "test");

    assert_eq!(scx.type_i64().get_bit_width(), 64);
    assert_eq!(scx.type_i32().get_bit_width(), 32);
    assert_eq!(scx.type_i8().get_bit_width(), 8);
    assert_eq!(scx.type_i1().get_bit_width(), 1);
}
