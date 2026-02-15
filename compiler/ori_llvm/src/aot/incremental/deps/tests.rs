use super::*;

fn h(n: u64) -> ContentHash {
    ContentHash::new(n)
}

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

#[test]
fn test_add_and_get_imports() {
    let mut graph = DependencyGraph::new();

    graph.add_file(p("main.ori"), h(1), vec![p("lib.ori"), p("utils.ori")]);

    let imports = graph.get_imports(Path::new("main.ori")).unwrap();
    assert_eq!(imports.len(), 2);
    assert!(imports.contains(&p("lib.ori")));
    assert!(imports.contains(&p("utils.ori")));
}

#[test]
fn test_get_dependents() {
    let mut graph = DependencyGraph::new();

    graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
    graph.add_file(p("tests.ori"), h(2), vec![p("lib.ori")]);
    graph.add_file(p("lib.ori"), h(3), vec![]);

    let dependents = graph.get_dependents(Path::new("lib.ori")).unwrap();
    assert_eq!(dependents.len(), 2);
    assert!(dependents.contains(&p("main.ori")));
    assert!(dependents.contains(&p("tests.ori")));
}

#[test]
fn test_transitive_dependencies() {
    let mut graph = DependencyGraph::new();

    // main -> lib -> utils -> core
    graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
    graph.add_file(p("lib.ori"), h(2), vec![p("utils.ori")]);
    graph.add_file(p("utils.ori"), h(3), vec![p("core.ori")]);
    graph.add_file(p("core.ori"), h(4), vec![]);

    let deps = graph.transitive_dependencies(Path::new("main.ori"));
    assert_eq!(deps.len(), 3);
    assert!(deps.contains(&p("lib.ori")));
    assert!(deps.contains(&p("utils.ori")));
    assert!(deps.contains(&p("core.ori")));
}

#[test]
fn test_transitive_dependents() {
    let mut graph = DependencyGraph::new();

    // main -> lib -> utils
    // test -> lib
    graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
    graph.add_file(p("test.ori"), h(2), vec![p("lib.ori")]);
    graph.add_file(p("lib.ori"), h(3), vec![p("utils.ori")]);
    graph.add_file(p("utils.ori"), h(4), vec![]);

    // Changes to utils should trigger recompilation of lib, main, test
    let deps = graph.transitive_dependents(Path::new("utils.ori"));
    assert_eq!(deps.len(), 3);
    assert!(deps.contains(&p("lib.ori")));
    assert!(deps.contains(&p("main.ori")));
    assert!(deps.contains(&p("test.ori")));
}

#[test]
fn test_topological_order() {
    let mut graph = DependencyGraph::new();

    // main -> lib -> utils
    graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
    graph.add_file(p("lib.ori"), h(2), vec![p("utils.ori")]);
    graph.add_file(p("utils.ori"), h(3), vec![]);

    let order = graph.topological_order().unwrap();
    assert_eq!(order.len(), 3);

    // utils must come before lib, lib must come before main
    let utils_pos = order.iter().position(|x| x == &p("utils.ori")).unwrap();
    let lib_pos = order.iter().position(|x| x == &p("lib.ori")).unwrap();
    let main_pos = order.iter().position(|x| x == &p("main.ori")).unwrap();

    assert!(utils_pos < lib_pos);
    assert!(lib_pos < main_pos);
}

#[test]
fn test_cycle_detection() {
    let mut graph = DependencyGraph::new();

    // a -> b -> c -> a (cycle)
    graph.add_file(p("a.ori"), h(1), vec![p("b.ori")]);
    graph.add_file(p("b.ori"), h(2), vec![p("c.ori")]);
    graph.add_file(p("c.ori"), h(3), vec![p("a.ori")]);

    assert!(graph.topological_order().is_none());
}

#[test]
fn test_files_to_recompile() {
    let mut graph = DependencyGraph::new();

    // main -> lib -> utils
    graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
    graph.add_file(p("lib.ori"), h(2), vec![p("utils.ori")]);
    graph.add_file(p("utils.ori"), h(3), vec![]);

    // Changing utils should require recompiling utils, lib, main
    let to_recompile = graph.files_to_recompile(&[p("utils.ori")]);
    assert_eq!(to_recompile.len(), 3);
    assert!(to_recompile.contains(&p("utils.ori")));
    assert!(to_recompile.contains(&p("lib.ori")));
    assert!(to_recompile.contains(&p("main.ori")));
}

#[test]
fn test_remove_file() {
    let mut graph = DependencyGraph::new();

    graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
    graph.add_file(p("lib.ori"), h(2), vec![]);

    graph.remove_file(Path::new("main.ori"));

    assert!(!graph.contains(Path::new("main.ori")));
    assert!(graph
        .get_dependents(Path::new("lib.ori"))
        .unwrap()
        .is_empty());
}

#[test]
fn test_update_imports() {
    let mut graph = DependencyGraph::new();

    // Initially main imports lib
    graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
    graph.add_file(p("lib.ori"), h(2), vec![]);
    graph.add_file(p("utils.ori"), h(3), vec![]);

    assert!(graph
        .get_dependents(Path::new("lib.ori"))
        .unwrap()
        .contains(&p("main.ori")));

    // Update: main now imports utils instead
    graph.add_file(p("main.ori"), h(1), vec![p("utils.ori")]);

    // lib should no longer have main as dependent
    assert!(!graph
        .get_dependents(Path::new("lib.ori"))
        .unwrap()
        .contains(&p("main.ori")));
    // utils should now have main as dependent
    assert!(graph
        .get_dependents(Path::new("utils.ori"))
        .unwrap()
        .contains(&p("main.ori")));
}

#[test]
fn test_dependency_tracker() {
    let tracker = DependencyTracker::new(PathBuf::from("/tmp/cache"));

    assert_eq!(tracker.cache_dir(), Path::new("/tmp/cache"));
    assert!(tracker.graph().is_empty());
}

#[test]
fn test_dependency_error_display() {
    let err = DependencyError::CyclicDependency {
        cycle: vec![p("a.ori"), p("b.ori"), p("a.ori")],
    };
    let msg = err.to_string();
    assert!(msg.contains("circular dependency"));
    assert!(msg.contains("a.ori"));
    assert!(msg.contains("b.ori"));

    let err = DependencyError::IoError {
        path: p("/test.ori"),
        message: "not found".to_string(),
    };
    assert!(err.to_string().contains("/test.ori"));
}

#[test]
fn test_graph_len_and_empty() {
    let mut graph = DependencyGraph::new();
    assert!(graph.is_empty());
    assert_eq!(graph.len(), 0);

    graph.add_file(p("a.ori"), h(1), vec![]);
    assert!(!graph.is_empty());
    assert_eq!(graph.len(), 1);
}
