use super::*;

fn h(n: u64) -> ContentHash {
    ContentHash::new(n)
}

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

#[test]
fn test_work_item() {
    let item = WorkItem::new(p("main.ori"), h(123))
        .with_dependencies(vec![p("lib.ori")])
        .with_priority(1);

    assert_eq!(item.path, p("main.ori"));
    assert_eq!(item.hash.value(), 123);
    assert_eq!(item.dependencies.len(), 1);
    assert_eq!(item.priority, 1);
}

#[test]
fn test_compilation_plan_empty() {
    let plan = CompilationPlan::new();
    assert!(plan.is_empty());
    assert!(plan.is_complete());
}

#[test]
fn test_compilation_plan_single() {
    let mut plan = CompilationPlan::new();
    plan.add_item(WorkItem::new(p("main.ori"), h(1)));

    assert_eq!(plan.len(), 1);
    assert!(!plan.is_complete());

    let item = plan.take_next().unwrap();
    assert_eq!(item.path, p("main.ori"));

    plan.complete(&p("main.ori"));
    assert!(plan.is_complete());
}

#[test]
fn test_compilation_plan_with_deps() {
    let mut plan = CompilationPlan::new();

    // Add items with dependencies
    plan.add_item(WorkItem::new(p("main.ori"), h(1)).with_dependencies(vec![p("lib.ori")]));
    plan.add_item(WorkItem::new(p("lib.ori"), h(2)));

    // lib.ori should be ready first (no deps)
    let item = plan.take_next().unwrap();
    assert_eq!(item.path, p("lib.ori"));
    plan.complete(&p("lib.ori"));

    // Now main.ori should be ready
    let item = plan.take_next().unwrap();
    assert_eq!(item.path, p("main.ori"));
    plan.complete(&p("main.ori"));

    assert!(plan.is_complete());
}

#[test]
fn test_parallel_config_auto() {
    let config = ParallelConfig::auto();
    assert!(config.effective_jobs() >= 1);
}

#[test]
fn test_parallel_config_explicit() {
    let config = ParallelConfig::new(4);
    assert_eq!(config.effective_jobs(), 4);
}

#[test]
fn test_parallel_compiler_execute() {
    let mut plan = CompilationPlan::new();
    plan.add_item(WorkItem::new(p("a.ori"), h(1)));
    plan.add_item(WorkItem::new(p("b.ori"), h(2)));

    let compiler = ParallelCompiler::new(ParallelConfig::new(1));

    let stats = compiler
        .execute(plan, |item| {
            Ok(CompileResult {
                path: item.path.clone(),
                output: PathBuf::from(format!("{}.o", item.path.display())),
                cached: false,
                time_ms: 10,
            })
        })
        .unwrap();

    assert_eq!(stats.total, 2);
    assert_eq!(stats.compiled, 2);
    assert_eq!(stats.cached, 0);
}

#[test]
fn test_parallel_compiler_with_error() {
    let mut plan = CompilationPlan::new();
    plan.add_item(WorkItem::new(p("bad.ori"), h(1)));

    let compiler = ParallelCompiler::new(ParallelConfig::new(1));

    let result = compiler.execute(plan, |item| {
        Err(CompileError {
            path: item.path.clone(),
            message: "syntax error".to_string(),
        })
    });

    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("syntax error"));
}

#[test]
#[allow(
    deprecated,
    reason = "tests compile_parallel which is deprecated in favor of execute_parallel"
)]
fn test_compile_parallel_single() {
    let mut plan = CompilationPlan::new();
    plan.add_item(WorkItem::new(p("test.ori"), h(1)));

    let results: Vec<String> = compile_parallel(&plan, 1, |item| {
        Ok(format!("compiled: {}", item.path.display()))
    })
    .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].contains("test.ori"));
}

#[test]
#[allow(
    deprecated,
    reason = "tests compile_parallel which is deprecated in favor of execute_parallel"
)]
fn test_compile_parallel_multiple() {
    let mut plan = CompilationPlan::new();
    for i in 0..10 {
        plan.add_item(WorkItem::new(p(&format!("file{i}.ori")), h(i)));
    }

    let results: Vec<usize> =
        compile_parallel(&plan, 4, |item| Ok(item.hash.value() as usize)).unwrap();

    assert_eq!(results.len(), 10);
}

#[test]
fn test_compile_error_display() {
    let err = CompileError {
        path: p("test.ori"),
        message: "undefined variable".to_string(),
    };

    let msg = err.to_string();
    assert!(msg.contains("test.ori"));
    assert!(msg.contains("undefined variable"));
}

#[test]
fn test_from_graph_three_file_dependency_order() {
    use crate::aot::incremental::deps::DependencyGraph;

    // Build a 3-file dependency graph:
    //   main.ori → lib.ori → utils.ori
    let mut graph = DependencyGraph::new();
    graph.add_file(p("utils.ori"), h(1), vec![]);
    graph.add_file(p("lib.ori"), h(2), vec![p("utils.ori")]);
    graph.add_file(p("main.ori"), h(3), vec![p("lib.ori")]);

    let files = vec![p("main.ori"), p("lib.ori"), p("utils.ori")];
    let plan = CompilationPlan::from_graph(&graph, &files);

    assert_eq!(plan.len(), 3);
    assert!(!plan.is_complete());

    // Execute the plan through ParallelCompiler to verify topological order
    let compiler = ParallelCompiler::new(ParallelConfig::new(1));
    let mut compilation_order = Vec::new();

    let stats = compiler
        .execute(plan, |item| {
            compilation_order.push(item.path.clone());
            Ok(CompileResult {
                path: item.path.clone(),
                output: PathBuf::from(format!("{}.o", item.path.display())),
                cached: false,
                time_ms: 1,
            })
        })
        .unwrap_or_else(|_| panic!("compilation should succeed"));

    assert_eq!(stats.total, 3);
    assert_eq!(stats.compiled, 3);

    // Verify topological order: utils before lib, lib before main
    let utils_pos = compilation_order
        .iter()
        .position(|p| p == &PathBuf::from("utils.ori"))
        .unwrap_or_else(|| panic!("utils.ori should be in compilation order"));
    let lib_pos = compilation_order
        .iter()
        .position(|p| p == &PathBuf::from("lib.ori"))
        .unwrap_or_else(|| panic!("lib.ori should be in compilation order"));
    let main_pos = compilation_order
        .iter()
        .position(|p| p == &PathBuf::from("main.ori"))
        .unwrap_or_else(|| panic!("main.ori should be in compilation order"));

    assert!(
        utils_pos < lib_pos,
        "utils.ori ({utils_pos}) should compile before lib.ori ({lib_pos})"
    );
    assert!(
        lib_pos < main_pos,
        "lib.ori ({lib_pos}) should compile before main.ori ({main_pos})"
    );
}

// ── execute_parallel tests ─────────────────────────────────

#[test]
fn test_execute_parallel_dependency_order() {
    let mut plan = CompilationPlan::new();
    plan.add_item(WorkItem::new(p("main.ori"), h(1)).with_dependencies(vec![p("lib.ori")]));
    plan.add_item(WorkItem::new(p("lib.ori"), h(2)));

    use std::sync::{Arc, Mutex};
    let order = Arc::new(Mutex::new(Vec::new()));
    let order_clone = Arc::clone(&order);

    let stats = execute_parallel(plan, 1, move |item| {
        order_clone
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(item.path.clone());
        Ok(CompileResult {
            path: item.path.clone(),
            output: PathBuf::from(format!("{}.o", item.path.display())),
            cached: false,
            time_ms: 1,
        })
    })
    .unwrap_or_else(|_| panic!("should succeed"));

    assert_eq!(stats.total, 2);
    let order = order.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(order[0], p("lib.ori"), "lib should compile before main");
    assert_eq!(order[1], p("main.ori"));
}

#[test]
fn test_execute_parallel_failure_cascade() {
    let mut plan = CompilationPlan::new();

    // main depends on lib, lib depends on utils
    plan.add_item(WorkItem::new(p("main.ori"), h(1)).with_dependencies(vec![p("lib.ori")]));
    plan.add_item(WorkItem::new(p("lib.ori"), h(2)).with_dependencies(vec![p("utils.ori")]));
    plan.add_item(WorkItem::new(p("utils.ori"), h(3)));

    // utils.ori fails → lib.ori and main.ori should be skipped
    let result = execute_parallel(plan, 1, |item| {
        if item.path == p("utils.ori") {
            Err(CompileError {
                path: item.path.clone(),
                message: "utils failed".to_string(),
            })
        } else {
            Ok(CompileResult {
                path: item.path.clone(),
                output: p("out.o"),
                cached: false,
                time_ms: 1,
            })
        }
    });

    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].path, p("utils.ori"));
    // lib.ori and main.ori should NOT have been attempted (cascade failure)
}

#[test]
fn test_execute_parallel_single_thread_fallback() {
    let mut plan = CompilationPlan::new();
    plan.add_item(WorkItem::new(p("a.ori"), h(1)));

    let stats = execute_parallel(plan, 1, |item| {
        Ok(CompileResult {
            path: item.path.clone(),
            output: p("a.o"),
            cached: false,
            time_ms: 5,
        })
    })
    .unwrap_or_else(|_| panic!("should succeed"));

    assert_eq!(stats.total, 1);
    assert_eq!(stats.compiled, 1);
}

#[test]
fn test_execute_parallel_empty_plan() {
    let plan = CompilationPlan::new();

    let stats = execute_parallel(plan, 4, |_item| {
        Ok(CompileResult {
            path: p("never.ori"),
            output: p("never.o"),
            cached: false,
            time_ms: 0,
        })
    })
    .unwrap_or_else(|_| panic!("empty plan should succeed"));

    assert_eq!(stats.total, 0);
}

#[test]
fn test_execute_parallel_multi_thread_same_as_sequential() {
    // Build the same plan twice, run with 1 thread and 4 threads
    fn make_plan() -> CompilationPlan {
        let mut plan = CompilationPlan::new();
        plan.add_item(WorkItem::new(PathBuf::from("a.ori"), ContentHash::new(1)));
        plan.add_item(WorkItem::new(PathBuf::from("b.ori"), ContentHash::new(2)));
        plan.add_item(WorkItem::new(PathBuf::from("c.ori"), ContentHash::new(3)));
        plan
    }

    let stats_1 = execute_parallel(make_plan(), 1, |item| {
        Ok(CompileResult {
            path: item.path.clone(),
            output: PathBuf::from(format!("{}.o", item.path.display())),
            cached: false,
            time_ms: 1,
        })
    })
    .unwrap_or_else(|_| panic!("should succeed"));

    let stats_4 = execute_parallel(make_plan(), 4, |item| {
        Ok(CompileResult {
            path: item.path.clone(),
            output: PathBuf::from(format!("{}.o", item.path.display())),
            cached: false,
            time_ms: 1,
        })
    })
    .unwrap_or_else(|_| panic!("should succeed"));

    assert_eq!(stats_1.total, stats_4.total);
    assert_eq!(stats_1.compiled, stats_4.compiled);
}

// ── mark_failed / transitive_dependents tests ────────────

#[test]
fn test_mark_failed_basic() {
    let mut plan = CompilationPlan::new();
    plan.add_item(WorkItem::new(p("a.ori"), h(1)));
    plan.add_item(WorkItem::new(p("b.ori"), h(2)).with_dependencies(vec![p("a.ori")]));

    plan.mark_failed(&p("a.ori"));

    assert!(plan.is_failed(&p("a.ori")));
    assert!(
        plan.is_failed(&p("b.ori")),
        "dependent should be cascade-failed"
    );
    assert_eq!(plan.failed_count(), 2);
    assert!(
        plan.is_complete(),
        "all items failed, plan should be complete"
    );
}

#[test]
fn test_transitive_dependents() {
    let mut plan = CompilationPlan::new();
    plan.add_item(WorkItem::new(p("a.ori"), h(1)));
    plan.add_item(WorkItem::new(p("b.ori"), h(2)).with_dependencies(vec![p("a.ori")]));
    plan.add_item(WorkItem::new(p("c.ori"), h(3)).with_dependencies(vec![p("b.ori")]));

    let deps = plan.transitive_dependents(&p("a.ori"));

    assert_eq!(deps.len(), 2);
    assert!(deps.contains(&p("b.ori")));
    assert!(deps.contains(&p("c.ori")));
}

#[test]
fn test_progress_tracking() {
    let compiler = ParallelCompiler::new(ParallelConfig::new(1));
    assert_eq!(compiler.progress(), 0);

    let mut plan = CompilationPlan::new();
    plan.add_item(WorkItem::new(p("a.ori"), h(1)));
    plan.add_item(WorkItem::new(p("b.ori"), h(2)));

    let _ = compiler.execute(plan, |item| {
        Ok(CompileResult {
            path: item.path.clone(),
            output: p("out.o"),
            cached: false,
            time_ms: 1,
        })
    });

    assert_eq!(compiler.progress(), 2);

    compiler.reset_progress();
    assert_eq!(compiler.progress(), 0);
}
