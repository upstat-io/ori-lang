use super::*;

#[test]
fn format_primitives() {
    let pool = Pool::new();

    assert_eq!(pool.format_type(Idx::INT), "int");
    assert_eq!(pool.format_type(Idx::FLOAT), "float");
    assert_eq!(pool.format_type(Idx::BOOL), "bool");
    assert_eq!(pool.format_type(Idx::STR), "str");
    assert_eq!(pool.format_type(Idx::CHAR), "char");
    assert_eq!(pool.format_type(Idx::UNIT), "()");
    assert_eq!(pool.format_type(Idx::NEVER), "never");
    assert_eq!(pool.format_type(Idx::ERROR), "<error>");
}

#[test]
fn format_containers() {
    let mut pool = Pool::new();

    let list_int = pool.list(Idx::INT);
    assert_eq!(pool.format_type(list_int), "[int]");

    let opt_str = pool.option(Idx::STR);
    assert_eq!(pool.format_type(opt_str), "str?");

    let set_bool = pool.set(Idx::BOOL);
    assert_eq!(pool.format_type(set_bool), "{bool}");
}

#[test]
fn format_two_child() {
    let mut pool = Pool::new();

    let map_ty = pool.map(Idx::STR, Idx::INT);
    assert_eq!(pool.format_type(map_ty), "{str: int}");

    let result_ty = pool.result(Idx::INT, Idx::STR);
    assert_eq!(pool.format_type(result_ty), "result<int, str>");
}

#[test]
fn format_function() {
    let mut pool = Pool::new();

    let fn_ty = pool.function(&[Idx::INT, Idx::STR], Idx::BOOL);
    assert_eq!(pool.format_type(fn_ty), "(int, str) -> bool");

    let nullary = pool.function0(Idx::UNIT);
    assert_eq!(pool.format_type(nullary), "() -> ()");
}

#[test]
fn format_tuple() {
    let mut pool = Pool::new();

    let tuple = pool.tuple(&[Idx::INT, Idx::STR, Idx::BOOL]);
    assert_eq!(pool.format_type(tuple), "(int, str, bool)");
}

#[test]
fn format_nested() {
    let mut pool = Pool::new();

    // [[int]]
    let inner = pool.list(Idx::INT);
    let outer = pool.list(inner);
    assert_eq!(pool.format_type(outer), "[[int]]");

    // (int, [str])?
    let list_str = pool.list(Idx::STR);
    let tuple = pool.tuple(&[Idx::INT, list_str]);
    let opt = pool.option(tuple);
    assert_eq!(pool.format_type(opt), "(int, [str])?");
}

#[test]
fn format_fresh_var() {
    let mut pool = Pool::new();

    let var = pool.fresh_var();
    let formatted = pool.format_type(var);
    assert!(formatted.starts_with("$t"));
}

#[test]
fn format_named_resolved() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();

    let name = interner.intern("Point");
    let named = pool.named(name);
    assert_eq!(pool.format_type_resolved(named, &interner), "Point");
}

#[test]
fn format_named_in_container_resolved() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();

    let name = interner.intern("Point");
    let named = pool.named(name);
    let list = pool.list(named);
    assert_eq!(pool.format_type_resolved(list, &interner), "[Point]");

    let opt = pool.option(named);
    assert_eq!(pool.format_type_resolved(opt, &interner), "Point?");
}

#[test]
fn format_named_without_interner_shows_raw() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();

    let name = interner.intern("Point");
    let named = pool.named(name);
    // Without interner, shows raw index
    assert!(pool.format_type(named).starts_with("Named#"));
}
