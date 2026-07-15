use ori_vm::{ExecutionConfig, ExitValue};

const USER_OPERATOR_PROGRAM: &str = r"
trait Add<Rhs = Self> {
    type Output

    @add (self, rhs: Rhs) -> Self.Output
}

type Point = { x: int, y: int }

impl Point: Add {
    type Output = Point;

    @add (self, rhs: Point) -> Point = Point {
        x: self.x + rhs.x,
        y: self.y + rhs.y,
    }
}

@main () -> int = {
    let left = Point { x: 1, y: 2 };
    let right = Point { x: 3, y: 4 };
    let sum = left + right;
    sum.x
}
";

#[test]
fn resolved_user_operator_uses_the_normal_callable_path_before_vm_lowering() {
    let executable = oric::test_support::compile_to_executable(
        "vm_operator_trait_runtime.ori",
        USER_OPERATOR_PROGRAM,
        ori_repr::NarrowingPolicy::Aggressive,
    )
    .unwrap_or_else(|error| panic!("user operator fixture should realize: {error}"));

    for typed_primitives in [false, true] {
        let bytecode =
            ori_vm::compile_with_options(&executable, ori_vm::CompileOptions { typed_primitives })
                .unwrap_or_else(|error| panic!("user operator fixture should lower: {error}"));
        let verified = ori_vm::verify(bytecode)
            .unwrap_or_else(|error| panic!("user operator bytecode should verify: {error}"));
        let report = ori_vm::execute_report(&verified, ExecutionConfig::default());
        let value = report.result.unwrap_or_else(|error| {
            panic!("user operator fixture should execute in both modes: {error}")
        });

        assert_eq!(value, ExitValue::Int(4));
        assert_eq!(report.metrics.final_live_heap_objects, 0);
        assert_eq!(report.metrics.final_value_arena_entries, 0);
    }
}
