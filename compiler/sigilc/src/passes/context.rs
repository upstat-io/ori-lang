// Pass context for Sigil compiler passes
// Holds shared state accessible to all passes
//
// Uses the Visitor trait for call graph construction - demonstrates how
// adding a new expression type only requires updating ir/visit.rs.

use crate::ir::{FuncRef, TExpr, TModule, Type, Visitor};
use std::collections::{HashMap, HashSet};

/// Configuration for debug output
#[derive(Debug, Clone, Default)]
pub struct DebugConfig {
    /// Dump the TIR after each pass
    pub dump_after_each: bool,
    /// Print timing information
    pub print_timing: bool,
    /// Print pass names as they run
    pub verbose: bool,
}

impl DebugConfig {
    pub fn quiet() -> Self {
        DebugConfig {
            dump_after_each: false,
            print_timing: false,
            verbose: false,
        }
    }

    pub fn verbose() -> Self {
        DebugConfig {
            dump_after_each: false,
            print_timing: true,
            verbose: true,
        }
    }
}

/// Visitor that collects function callees from expressions
struct CalleeCollector;

impl Visitor for CalleeCollector {
    type Result = HashSet<String>;

    fn default_result(&self) -> HashSet<String> {
        HashSet::new()
    }

    fn combine_results(&self, mut a: HashSet<String>, b: HashSet<String>) -> HashSet<String> {
        a.extend(b);
        a
    }

    // Only override visit_call to capture function references
    fn visit_call(
        &mut self,
        func: &FuncRef,
        args: &[TExpr],
        _ty: &Type,
        _span: &crate::ast::Span,
    ) -> HashSet<String> {
        let mut callees = HashSet::new();

        // Capture user function calls
        if let FuncRef::User(name) = func {
            callees.insert(name.clone());
        }

        // Also visit arguments
        for arg in args {
            callees.extend(self.visit_expr(arg));
        }

        callees
    }
}

/// Call graph representing function call relationships
#[derive(Debug, Clone, Default)]
pub struct CallGraph {
    /// Map from caller to set of callees
    edges: HashMap<String, HashSet<String>>,
    /// Set of functions reachable from main/tests
    reachable: HashSet<String>,
}

impl CallGraph {
    pub fn new() -> Self {
        CallGraph {
            edges: HashMap::new(),
            reachable: HashSet::new(),
        }
    }

    /// Build the call graph from a module
    pub fn build(module: &TModule) -> Self {
        let mut graph = CallGraph::new();
        let mut collector = CalleeCollector;

        // Add edges for each function
        for func in &module.functions {
            let callees = collector.visit_expr(&func.body);
            graph.edges.insert(func.name.clone(), callees);
        }

        // Add edges for tests
        for test in &module.tests {
            let callees = collector.visit_expr(&test.body);
            graph.edges.insert(format!("test:{}", test.name), callees);
        }

        // Compute reachable functions from main and tests
        graph.compute_reachable(module);

        graph
    }

    /// Compute which functions are reachable from main and tests
    fn compute_reachable(&mut self, module: &TModule) {
        let mut worklist = Vec::new();

        // Start from main function if it exists
        if module.find_main().is_some() {
            worklist.push("main".to_string());
        }

        // Also start from all test functions and their targets
        for test in &module.tests {
            worklist.push(format!("test:{}", test.name));
            worklist.push(test.target.clone());
        }

        // Traverse the call graph
        while let Some(func) = worklist.pop() {
            if self.reachable.contains(&func) {
                continue;
            }
            self.reachable.insert(func.clone());

            if let Some(callees) = self.edges.get(&func) {
                for callee in callees {
                    if !self.reachable.contains(callee) {
                        worklist.push(callee.clone());
                    }
                }
            }
        }
    }

    /// Check if a function is reachable
    pub fn is_reachable(&self, name: &str) -> bool {
        self.reachable.contains(name)
    }

    /// Get all callees of a function
    pub fn callees(&self, name: &str) -> Option<&HashSet<String>> {
        self.edges.get(name)
    }

    /// Get all reachable functions
    pub fn reachable_functions(&self) -> &HashSet<String> {
        &self.reachable
    }
}

/// Context passed to all compiler passes
#[derive(Debug)]
pub struct PassContext {
    /// Debug configuration
    pub debug: DebugConfig,
    /// Call graph (lazily built)
    call_graph: Option<CallGraph>,
}

impl Default for PassContext {
    fn default() -> Self {
        PassContext::new()
    }
}

impl PassContext {
    pub fn new() -> Self {
        PassContext {
            debug: DebugConfig::default(),
            call_graph: None,
        }
    }

    pub fn with_debug(debug: DebugConfig) -> Self {
        PassContext {
            debug,
            call_graph: None,
        }
    }

    /// Get or build the call graph
    pub fn call_graph(&mut self, module: &TModule) -> &CallGraph {
        self.call_graph
            .get_or_insert_with(|| CallGraph::build(module))
    }

    /// Invalidate the call graph (should be called when the module changes)
    pub fn invalidate_call_graph(&mut self) {
        self.call_graph = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_config_defaults() {
        let config = DebugConfig::default();
        assert!(!config.dump_after_each);
        assert!(!config.print_timing);
        assert!(!config.verbose);
    }

    #[test]
    fn test_debug_config_quiet() {
        let config = DebugConfig::quiet();
        assert!(!config.verbose);
    }

    #[test]
    fn test_debug_config_verbose() {
        let config = DebugConfig::verbose();
        assert!(config.verbose);
        assert!(config.print_timing);
    }

    #[test]
    fn test_call_graph_new() {
        let graph = CallGraph::new();
        assert!(graph.edges.is_empty());
        assert!(graph.reachable.is_empty());
    }

    #[test]
    fn test_callee_collector() {
        use crate::ir::{TExpr, TExprKind, Type};

        let mut collector = CalleeCollector;

        // Test simple function call
        let call = TExpr::new(
            TExprKind::Call {
                func: FuncRef::User("foo".to_string()),
                args: vec![],
            },
            Type::Int,
            0..1,
        );

        let callees = collector.visit_expr(&call);
        assert!(callees.contains("foo"));
        assert_eq!(callees.len(), 1);
    }

    #[test]
    fn test_callee_collector_nested() {
        use crate::ir::{TExpr, TExprKind, Type};

        let mut collector = CalleeCollector;

        // Test nested: foo(bar())
        let inner_call = TExpr::new(
            TExprKind::Call {
                func: FuncRef::User("bar".to_string()),
                args: vec![],
            },
            Type::Int,
            0..1,
        );

        let outer_call = TExpr::new(
            TExprKind::Call {
                func: FuncRef::User("foo".to_string()),
                args: vec![inner_call],
            },
            Type::Int,
            0..1,
        );

        let callees = collector.visit_expr(&outer_call);
        assert!(callees.contains("foo"));
        assert!(callees.contains("bar"));
        assert_eq!(callees.len(), 2);
    }
}
