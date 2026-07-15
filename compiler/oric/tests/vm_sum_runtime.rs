use ori_vm::{ExecutionConfig, ExitValue};

const SUM_PROGRAM: &str = r#"
type Msg = Text(body: str) | Empty;

@message_length (message: Msg) -> int = match message {
    Text(body) -> body.length(),
    Empty -> 0,
};

@main () -> int = message_length(message: Text(body: "payload-extraction"));
"#;

const RECURSIVE_SUM_PROGRAM: &str = r"
type Chain = Nil | Cons(head: int, tail: Chain);

@sum (chain: Chain) -> int = match chain {
    Nil -> 0,
    Cons(head, tail) -> head + sum(chain: tail),
};

@main () -> int = sum(chain: Cons(head: 3, tail: Cons(head: 5, tail: Nil)));
";

#[test]
fn source_sum_projects_tag_before_string_payload() {
    assert_vm_result("vm_sum_runtime.ori", SUM_PROGRAM, &ExitValue::Int(18));
}

#[test]
fn source_recursive_sum_projects_payloads_after_tag() {
    assert_vm_result(
        "vm_recursive_sum_runtime.ori",
        RECURSIVE_SUM_PROGRAM,
        &ExitValue::Int(8),
    );
}

fn assert_vm_result(path: &str, source: &str, expected: &ExitValue) {
    let executable = oric::test_support::compile_to_executable(
        path,
        source,
        ori_repr::NarrowingPolicy::Aggressive,
    )
    .unwrap_or_else(|error| panic!("sum fixture should realize: {error}"));

    for typed_primitives in [false, true] {
        let bytecode =
            ori_vm::compile_with_options(&executable, ori_vm::CompileOptions { typed_primitives })
                .unwrap_or_else(|error| panic!("sum fixture should lower: {error}"));
        let verified = ori_vm::verify(bytecode)
            .unwrap_or_else(|error| panic!("sum fixture bytecode should verify: {error}"));
        let report = ori_vm::execute_report(&verified, ExecutionConfig::default());
        let value = report.result.unwrap_or_else(|error| {
            panic!("sum fixture should execute with typed_primitives={typed_primitives}: {error}")
        });

        assert_eq!(&value, expected);
        assert_eq!(report.metrics.exit_live_heap_objects, 0);
        assert_eq!(report.metrics.final_live_heap_objects, 0);
        assert_eq!(report.metrics.final_value_arena_entries, 0);
    }
}
