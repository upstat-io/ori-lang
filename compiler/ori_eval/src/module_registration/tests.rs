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
        @add (a: int, b: int) -> int = a + b;
        @main () -> void = print(msg: str(add(a: 1, b: 2)));
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
fn registered_functions_bind_the_complete_same_module_namespace() {
    let (result, interner) = parse_source(
        r"
        @private_leaf (x: int) -> int = x + 1;
        @private_mid (x: int) -> int = private_leaf(x: x);
        pub @exported (x: int) -> int = private_mid(x: x);
    ",
    );
    let mut env = Environment::new();
    register_module_functions(&result.module, &result.arena, &mut env, None);
    let exported = interner.intern("exported");
    let private_leaf = interner.intern("private_leaf");
    let private_mid = interner.intern("private_mid");
    let Some(Value::Function(function)) = env.lookup(exported) else {
        panic!("missing exported function");
    };
    let mut call_env = Environment::new();

    crate::exec::call::bind_captures(&mut call_env, &function);

    assert!(matches!(
        call_env.lookup(private_leaf),
        Some(Value::Function(_))
    ));
    assert!(matches!(
        call_env.lookup(private_mid),
        Some(Value::Function(_))
    ));
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
fn module_bindings_capture_local_variants_over_prelude_name_collisions() {
    let (result, interner) = parse_source(
        r"
        type Stream = Left(v: int) | Right(v: int);
        @make_left () -> Stream = Left(v: 1);
        @make_right () -> Stream = Right(v: 2);
    ",
    );

    let alignment = interner.intern("Alignment");
    let left = interner.intern("Left");
    let right = interner.intern("Right");
    let mut env = Environment::new();
    env.define_global(left, Value::variant(alignment, left, vec![]));
    env.define_global(right, Value::variant(alignment, right, vec![]));

    register_module_bindings(&result.module, &result.arena, &mut env, None);

    for (function_name, variant_name) in [("make_left", left), ("make_right", right)] {
        let function = env
            .lookup(interner.intern(function_name))
            .unwrap_or_else(|| panic!("missing {function_name}"));
        let Value::Function(function) = function else {
            panic!("{function_name} is not a function");
        };
        assert!(matches!(
            function.get_capture(variant_name),
            Some(Value::VariantConstructor {
                variant_name: captured,
                field_count: 1,
                ..
            }) if *captured == variant_name
        ));
    }
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
            @sum (self) -> int = self.x + self.y;
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
            @sum (self) -> int = self.x + self.y;
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
            @double (self) -> [T] = self + self;
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
            @double (self) -> [T] = self + self;
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
            @get (url: str) -> str = url;
            @post (url: str, body: str) -> str = body;
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
            @get (url: str) -> str = url;
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
