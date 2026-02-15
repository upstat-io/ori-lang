use super::*;
use crate::aot::incremental::hash::hash_string;

fn deps(name: &str, callees: &[&str], sig: &str) -> FunctionDeps {
    FunctionDeps {
        name: name.to_string(),
        callees: callees.iter().map(|s| (*s).to_string()).collect(),
        signature_hash: hash_string(sig),
        content_hash: hash_string(&format!("{name}_content")),
    }
}

#[test]
fn body_only_change_skips_callers() {
    let mut graph = FunctionDependencyGraph::new();

    // main calls helper; helper calls utils
    graph.add_function(deps("main", &["helper"], "main_sig"));
    graph.add_function(deps("helper", &["utils"], "helper_sig"));
    graph.add_function(deps("utils", &[], "utils_sig"));

    // Change helper's body but NOT its signature
    let old_sigs: FxHashMap<String, ContentHash> = [
        ("main".to_string(), hash_string("main_sig")),
        ("helper".to_string(), hash_string("helper_sig")), // Same sig!
        ("utils".to_string(), hash_string("utils_sig")),
    ]
    .into_iter()
    .collect();

    let recompile = graph.functions_to_recompile(&["helper".to_string()], &old_sigs);

    // Only helper should be recompiled (body-only change)
    assert!(recompile.contains("helper"));
    assert!(
        !recompile.contains("main"),
        "main should not be recompiled for body-only change"
    );
    assert!(!recompile.contains("utils"));
}

#[test]
fn signature_change_propagates_to_callers() {
    let mut graph = FunctionDependencyGraph::new();

    // main calls helper; helper calls utils
    graph.add_function(deps("main", &["helper"], "main_sig"));
    graph.add_function(deps("helper", &["utils"], "helper_sig_v2")); // Changed sig
    graph.add_function(deps("utils", &[], "utils_sig"));

    let old_sigs: FxHashMap<String, ContentHash> = [
        ("main".to_string(), hash_string("main_sig")),
        ("helper".to_string(), hash_string("helper_sig_v1")), // Different from current
        ("utils".to_string(), hash_string("utils_sig")),
    ]
    .into_iter()
    .collect();

    let recompile = graph.functions_to_recompile(&["helper".to_string()], &old_sigs);

    // helper AND main should be recompiled (signature change propagates)
    assert!(recompile.contains("helper"));
    assert!(
        recompile.contains("main"),
        "main should be recompiled when helper's signature changes"
    );
    assert!(!recompile.contains("utils"));
}

#[test]
fn transitive_signature_propagation() {
    let mut graph = FunctionDependencyGraph::new();

    // a calls b, b calls c, c calls d
    graph.add_function(deps("a", &["b"], "a_sig"));
    graph.add_function(deps("b", &["c"], "b_sig"));
    graph.add_function(deps("c", &["d"], "c_sig_changed"));
    graph.add_function(deps("d", &[], "d_sig"));

    let old_sigs: FxHashMap<String, ContentHash> = [
        ("a".to_string(), hash_string("a_sig")),
        ("b".to_string(), hash_string("b_sig")),
        ("c".to_string(), hash_string("c_sig_original")), // Different
        ("d".to_string(), hash_string("d_sig")),
    ]
    .into_iter()
    .collect();

    let recompile = graph.functions_to_recompile(&["c".to_string()], &old_sigs);

    // c's signature changed, so c, b (caller), and a (transitive caller) recompile
    assert!(recompile.contains("c"));
    assert!(recompile.contains("b"));
    assert!(recompile.contains("a"));
    assert!(!recompile.contains("d")); // d is a callee, not a caller
}

#[test]
fn new_function_treated_as_signature_change() {
    let mut graph = FunctionDependencyGraph::new();

    graph.add_function(deps("main", &["new_fn"], "main_sig"));
    graph.add_function(deps("new_fn", &[], "new_fn_sig"));

    // new_fn not in old_sigs — treated as new (signature change)
    let old_sigs: FxHashMap<String, ContentHash> = [("main".to_string(), hash_string("main_sig"))]
        .into_iter()
        .collect();

    let recompile = graph.functions_to_recompile(&["new_fn".to_string()], &old_sigs);

    assert!(recompile.contains("new_fn"));
    assert!(
        recompile.contains("main"),
        "caller should recompile for new function"
    );
}

#[test]
fn empty_changed_set() {
    let mut graph = FunctionDependencyGraph::new();
    graph.add_function(deps("main", &[], "main_sig"));

    let old_sigs = FxHashMap::default();
    let recompile = graph.functions_to_recompile(&[], &old_sigs);

    assert!(recompile.is_empty());
}

#[test]
fn graph_size_queries() {
    let mut graph = FunctionDependencyGraph::new();
    assert!(graph.is_empty());
    assert_eq!(graph.len(), 0);

    graph.add_function(deps("a", &[], "sig"));
    assert!(!graph.is_empty());
    assert_eq!(graph.len(), 1);
}

#[test]
fn callers_of_query() {
    let mut graph = FunctionDependencyGraph::new();
    graph.add_function(deps("a", &["c"], "a_sig"));
    graph.add_function(deps("b", &["c"], "b_sig"));
    graph.add_function(deps("c", &[], "c_sig"));

    let callers = graph.callers_of("c");
    assert!(callers.is_some());
    let callers = callers.unwrap_or_else(|| panic!("callers should exist"));
    assert!(callers.contains("a"));
    assert!(callers.contains("b"));
    assert_eq!(callers.len(), 2);
}
