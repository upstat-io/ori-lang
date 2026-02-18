use super::*;
use ori_ir::SharedInterner;
use ori_lexer::lex;
use ori_parse::{parse, ParseOutput};

fn parse_source(source: &str) -> (ParseOutput, SharedInterner) {
    let interner = SharedInterner::default();
    let tokens = lex(source, &interner);
    let result = parse(&tokens, &interner);
    (result, interner)
}

#[test]
fn test_register_module_functions() {
    let (result, interner) = parse_source(
        r"
        @add (a: int, b: int) -> int = a + b
        @main () -> void = print(msg: str(add(a: 1, b: 2)))
    ",
    );

    let arena = result.arena.clone();
    let mut env = Environment::new();
    register_module_functions(&result.module, &arena, &mut env, None);

    let add_name = interner.intern("add");
    let main_name = interner.intern("main");

    assert!(env.lookup(add_name).is_some());
    assert!(env.lookup(main_name).is_some());
}

#[test]
fn test_register_variant_constructors() {
    let (result, interner) = parse_source(
        r"
        type Status = Running | Done(result: int)
    ",
    );

    let mut env = Environment::new();
    register_variant_constructors(&result.module, &mut env);

    let running_name = interner.intern("Running");
    let done_name = interner.intern("Done");

    // Unit variant should be a Value::Variant
    let running = env.lookup(running_name);
    assert!(running.is_some());
    assert!(matches!(running.unwrap(), Value::Variant { .. }));

    // Variant with fields should be a constructor
    let done = env.lookup(done_name);
    assert!(done.is_some());
    assert!(matches!(done.unwrap(), Value::VariantConstructor { .. }));
}

#[test]
fn test_register_newtype_constructors() {
    let (result, interner) = parse_source(
        r"
        type UserId = str
    ",
    );

    let mut env = Environment::new();
    register_newtype_constructors(&result.module, &mut env);

    let userid_name = interner.intern("UserId");

    let constructor = env.lookup(userid_name);
    assert!(constructor.is_some());
    assert!(matches!(
        constructor.unwrap(),
        Value::NewtypeConstructor { .. }
    ));
}

#[test]
fn test_collect_impl_methods() {
    let (result, interner) = parse_source(
        r"
        type Point = { x: int, y: int }

        impl Point {
            @sum (self) -> int = self.x + self.y
        }
    ",
    );

    let arena = result.arena.clone();
    let mut registry = UserMethodRegistry::new();
    let captures = Arc::new(FxHashMap::default());

    collect_impl_methods(
        &result.module,
        &arena,
        &captures,
        None,
        &interner,
        &mut registry,
    );

    let point_name = interner.intern("Point");
    let sum_name = interner.intern("sum");

    assert!(registry.lookup(point_name, sum_name).is_some());
}

#[test]
fn test_collect_impl_methods_with_config() {
    let (result, interner) = parse_source(
        r"
        type Point = { x: int, y: int }

        impl Point {
            @sum (self) -> int = self.x + self.y
        }
    ",
    );

    let arena = result.arena.clone();
    let mut registry = UserMethodRegistry::new();
    let captures = Arc::new(FxHashMap::default());

    let config = MethodCollectionConfig {
        module: &result.module,
        arena: &arena,
        captures: Arc::clone(&captures),
        canon: None,
        interner: &interner,
    };
    collect_impl_methods_with_config(&config, &mut registry);

    let point_name = interner.intern("Point");
    let sum_name = interner.intern("sum");

    assert!(registry.lookup(point_name, sum_name).is_some());
}

#[test]
fn test_collect_extend_methods() {
    let (result, interner) = parse_source(
        r"
        extend [T] {
            @double (self) -> [T] = self + self
        }
    ",
    );

    let arena = result.arena.clone();
    let mut registry = UserMethodRegistry::new();
    let captures = Arc::new(FxHashMap::default());

    collect_extend_methods(&result.module, &arena, &captures, None, &mut registry);

    let list_name = interner.intern("list");
    let double_name = interner.intern("double");

    assert!(registry.lookup(list_name, double_name).is_some());
}

#[test]
fn test_collect_extend_methods_with_config() {
    let (result, interner) = parse_source(
        r"
        extend [T] {
            @double (self) -> [T] = self + self
        }
    ",
    );

    let arena = result.arena.clone();
    let mut registry = UserMethodRegistry::new();
    let captures = Arc::new(FxHashMap::default());

    let config = MethodCollectionConfig {
        module: &result.module,
        arena: &arena,
        captures: Arc::clone(&captures),
        canon: None,
        interner: &interner,
    };
    collect_extend_methods_with_config(&config, &mut registry);

    let list_name = interner.intern("list");
    let double_name = interner.intern("double");

    assert!(registry.lookup(list_name, double_name).is_some());
}

#[test]
fn test_collect_def_impl_methods() {
    let (result, interner) = parse_source(
        r"
        def impl Http {
            @get (url: str) -> str = url
            @post (url: str, body: str) -> str = body
        }
    ",
    );

    let arena = result.arena.clone();
    let mut registry = UserMethodRegistry::new();
    let captures = Arc::new(FxHashMap::default());

    collect_def_impl_methods(&result.module, &arena, &captures, None, &mut registry);

    let http_name = interner.intern("Http");
    let get_name = interner.intern("get");
    let post_name = interner.intern("post");

    // Methods should be registered under the trait name
    assert!(registry.lookup(http_name, get_name).is_some());
    assert!(registry.lookup(http_name, post_name).is_some());
}

#[test]
fn test_collect_def_impl_methods_with_config() {
    let (result, interner) = parse_source(
        r"
        pub def impl Http {
            @get (url: str) -> str = url
        }
    ",
    );

    let arena = result.arena.clone();
    let mut registry = UserMethodRegistry::new();
    let captures = Arc::new(FxHashMap::default());

    let config = MethodCollectionConfig {
        module: &result.module,
        arena: &arena,
        captures: Arc::clone(&captures),
        canon: None,
        interner: &interner,
    };
    collect_def_impl_methods_with_config(&config, &mut registry);

    let http_name = interner.intern("Http");
    let get_name = interner.intern("get");

    assert!(registry.lookup(http_name, get_name).is_some());
}
