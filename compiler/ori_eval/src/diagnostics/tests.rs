use super::*;
use ori_ir::StringInterner;

// CallStack basic operations

#[test]
fn empty_stack() {
    let stack = CallStack::new(Some(100));
    assert!(stack.is_empty());
    assert_eq!(stack.depth(), 0);
}

#[test]
fn push_and_pop() {
    let interner = StringInterner::new();
    let name = interner.intern("foo");
    let mut stack = CallStack::new(Some(100));
    stack
        .push(CallFrame {
            name,
            call_span: None,
        })
        .expect("push should succeed");
    assert_eq!(stack.depth(), 1);
    assert!(!stack.is_empty());
    stack.pop();
    assert!(stack.is_empty());
}

#[test]
fn depth_limit_enforced() {
    let interner = StringInterner::new();
    let name = interner.intern("recurse");
    let mut stack = CallStack::new(Some(3));
    for _ in 0..3 {
        stack
            .push(CallFrame {
                name,
                call_span: None,
            })
            .expect("push within limit");
    }
    assert_eq!(stack.depth(), 3);
    let result = stack.push(CallFrame {
        name,
        call_span: None,
    });
    assert!(result.is_err());
    let err = result.expect_err("push should fail at max depth");
    assert_eq!(
        err.kind,
        ori_patterns::EvalErrorKind::StackOverflow { depth: 3 }
    );
    // Depth unchanged after failed push
    assert_eq!(stack.depth(), 3);
}

#[test]
fn unlimited_depth() {
    let interner = StringInterner::new();
    let name = interner.intern("deep");
    let mut stack = CallStack::new(None);
    for _ in 0..1000 {
        stack
            .push(CallFrame {
                name,
                call_span: None,
            })
            .expect("unlimited should never fail");
    }
    assert_eq!(stack.depth(), 1000);
}

// Backtrace capture

#[test]
fn capture_empty_stack() {
    let interner = StringInterner::new();
    let stack = CallStack::new(None);
    let bt = stack.capture(&interner);
    assert!(bt.is_empty());
}

#[test]
fn capture_preserves_order() {
    let interner = StringInterner::new();
    let foo = interner.intern("foo");
    let bar = interner.intern("bar");
    let baz = interner.intern("baz");

    let mut stack = CallStack::new(None);
    stack
        .push(CallFrame {
            name: foo,
            call_span: None,
        })
        .expect("ok");
    stack
        .push(CallFrame {
            name: bar,
            call_span: Some(Span::new(10, 20)),
        })
        .expect("ok");
    stack
        .push(CallFrame {
            name: baz,
            call_span: Some(Span::new(30, 40)),
        })
        .expect("ok");

    let bt = stack.capture(&interner);
    assert_eq!(bt.len(), 3);
    // Most recent call first
    assert_eq!(bt.frames()[0].name, "baz");
    assert_eq!(bt.frames()[1].name, "bar");
    assert_eq!(bt.frames()[2].name, "foo");
}

#[test]
fn attach_backtrace_to_error() {
    let interner = StringInterner::new();
    let name = interner.intern("failing_func");
    let mut stack = CallStack::new(None);
    stack
        .push(CallFrame {
            name,
            call_span: None,
        })
        .expect("ok");

    let err = ori_patterns::division_by_zero();
    let err = stack.attach_backtrace(err, &interner);
    assert!(err.backtrace.is_some());
    assert_eq!(
        err.backtrace.as_ref().map(ori_patterns::EvalBacktrace::len),
        Some(1)
    );
}

// Clone-per-child model

#[test]
fn clone_preserves_frames() {
    let interner = StringInterner::new();
    let name = interner.intern("parent");
    let mut stack = CallStack::new(Some(10));
    stack
        .push(CallFrame {
            name,
            call_span: None,
        })
        .expect("ok");

    let child = stack.clone();
    assert_eq!(child.depth(), 1);
    // Modifying child doesn't affect parent
    let mut child = child;
    let child_name = interner.intern("child");
    child
        .push(CallFrame {
            name: child_name,
            call_span: None,
        })
        .expect("ok");
    assert_eq!(child.depth(), 2);
    assert_eq!(stack.depth(), 1); // Parent unchanged
}

// EvalCounters

#[test]
fn counters_default_zero() {
    let c = EvalCounters::default();
    assert_eq!(c.expressions_evaluated, 0);
    assert_eq!(c.function_calls, 0);
}

#[test]
fn counters_increment() {
    let mut c = EvalCounters::default();
    c.count_expression();
    c.count_expression();
    c.count_function_call();
    assert_eq!(c.expressions_evaluated, 2);
    assert_eq!(c.function_calls, 1);
}

#[test]
fn counters_report_format() {
    let c = EvalCounters {
        expressions_evaluated: 100,
        function_calls: 10,
        method_calls: 5,
        pattern_matches: 3,
    };
    let report = c.report();
    assert!(report.contains("100"));
    assert!(report.contains("10"));
    assert!(report.contains("Evaluation profile"));
}

#[test]
fn counters_merge() {
    let mut parent = EvalCounters {
        expressions_evaluated: 10,
        function_calls: 2,
        method_calls: 1,
        pattern_matches: 0,
    };
    let child = EvalCounters {
        expressions_evaluated: 5,
        function_calls: 3,
        method_calls: 0,
        pattern_matches: 4,
    };
    parent.merge(&child);
    assert_eq!(parent.expressions_evaluated, 15);
    assert_eq!(parent.function_calls, 5);
    assert_eq!(parent.method_calls, 1);
    assert_eq!(parent.pattern_matches, 4);
}
