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

// === Merkle Hash Debug Tooling Tests ===

#[test]
fn format_hash_primitive() {
    let pool = Pool::new();
    let output = pool.format_hash(Idx::INT);
    assert!(output.contains("int"), "Should show type name: {output}");
    assert!(
        output.contains("hash=0x"),
        "Should show hash value: {output}"
    );
    assert!(output.contains("tag="), "Should show tag name: {output}");
}

#[test]
fn format_hash_container_shows_child_hash() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let output = pool.format_hash(list_int);
    assert!(output.contains("[int]"), "Should show type name");
    assert!(output.contains("child_hash=0x"), "Should show child hash");
    assert!(output.contains("(int)"), "Should show child type name");
}

#[test]
fn format_hash_function_shows_param_hashes() {
    let mut pool = Pool::new();
    let func = pool.function(&[Idx::INT, Idx::STR], Idx::BOOL);
    let output = pool.format_hash(func);
    assert!(output.contains("2 params"), "Should show param count");
    assert!(output.contains("p0_hash=0x"), "Should show param hash");
    assert!(output.contains("ret_hash=0x"), "Should show return hash");
}

#[test]
fn debug_hash_tree_shows_recursive_structure() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let map = pool.map(Idx::STR, list_int);
    let tree = pool.debug_hash_tree(map);

    // Should show the top-level map
    assert!(tree.contains("{str: [int]}"), "Should show map type");
    // Should show key and value subtrees
    assert!(tree.contains("key:"), "Should show key label");
    assert!(tree.contains("value:"), "Should show value label");
    // Should show the nested list
    assert!(tree.contains("[int]"), "Should show nested list");
    // Each line should have a hash
    let hash_count = tree.matches("hash=0x").count();
    assert!(
        hash_count >= 4,
        "Should have 4+ hashes (map, str, list, int), got {hash_count}"
    );
}

#[test]
fn debug_hash_tree_function_shows_params_and_return() {
    let mut pool = Pool::new();
    let func = pool.function(&[Idx::INT, Idx::STR], Idx::BOOL);
    let tree = pool.debug_hash_tree(func);

    assert!(tree.contains("param[0]:"), "Should show param[0]");
    assert!(tree.contains("param[1]:"), "Should show param[1]");
    assert!(tree.contains("return:"), "Should show return");
}
