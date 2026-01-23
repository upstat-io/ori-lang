// ARC Insertion Pass for Sigil compiler
//
// This pass inserts retain/release operations for ARC memory management.
// It runs after pattern lowering and before code generation.
//
// The pass:
// 1. Classifies all types as value or reference types
// 2. Tracks scope boundaries for local variables
// 3. Identifies retain points (new references created)
// 4. Identifies release points (references going out of scope)
// 5. Applies elision optimizations where safe
//
// ## Exhaustive Classification
//
// This pass uses the ExhaustiveArcAnalyzer which implements classifier traits
// with one method per IR variant. Adding a new TExprKind, Type, TPattern, or
// TMatchPattern variant will cause a Rust compile error until the variant is
// handled in the analyzer. This makes it impossible to forget ARC handling.

use crate::arc::{
    classify_type_exhaustive, DefaultRefCountAnalyzer, ExhaustiveArcAnalyzer, RefCountAnalyzer,
    ReleasePoint, RetainPoint,
};
use crate::ir::{TFunction, TModule, Type};
use crate::passes::{Pass, PassContext, PassError, PassResult};

/// ARC insertion pass
///
/// Analyzes the TIR and prepares metadata for ARC code generation.
/// The actual retain/release calls are emitted during codegen.
///
/// ## Exhaustive Classification
///
/// This pass uses `ExhaustiveArcAnalyzer` which enforces handling of all
/// IR variants through exhaustive pattern matching. Adding a new variant
/// to TExprKind, Type, TPattern, or TMatchPattern will cause a Rust
/// compile error until the corresponding classifier method is implemented.
#[derive(Default)]
pub struct ArcInsertionPass {
    /// The exhaustive analyzer for ARC classification
    analyzer: ExhaustiveArcAnalyzer,
}

impl ArcInsertionPass {
    pub fn new() -> Self {
        ArcInsertionPass {
            analyzer: ExhaustiveArcAnalyzer::new(),
        }
    }

    /// Analyze a function for ARC requirements using the exhaustive analyzer
    fn analyze_function(&self, func: &TFunction) -> PassFunctionArcInfo {
        // Use the legacy analyzer for retain/release points
        // The exhaustive analyzer is used for type classification
        let legacy_analyzer = DefaultRefCountAnalyzer::new();
        let retains = legacy_analyzer.retains_needed(func);
        let releases = legacy_analyzer.releases_needed(func);
        let elisions = legacy_analyzer.elision_opportunities(func);

        // Count reference-typed locals using the exhaustive type classifier
        // This ensures that adding new Type variants will cause a compile error
        // until they are properly handled
        let ref_type_count = func
            .locals
            .iter()
            .filter(|(_, info)| {
                let type_info = classify_type_exhaustive(&self.analyzer, &info.ty);
                type_info.needs_arc
            })
            .count();

        PassFunctionArcInfo {
            function_name: func.name.clone(),
            retains,
            releases,
            elision_count: elisions.len(),
            ref_type_locals: ref_type_count,
        }
    }
}

/// ARC information for a function (pass-local version)
#[derive(Debug)]
struct PassFunctionArcInfo {
    function_name: String,
    retains: Vec<RetainPoint>,
    releases: Vec<ReleasePoint>,
    elision_count: usize,
    ref_type_locals: usize,
}

impl Pass for ArcInsertionPass {
    fn name(&self) -> &'static str {
        "arc_insertion"
    }

    fn required(&self) -> bool {
        true
    }

    fn requires(&self) -> &[&'static str] {
        &["pattern_lowering"]
    }

    fn run(&self, ir: &mut TModule, ctx: &mut PassContext) -> Result<PassResult, PassError> {
        let mut total_retains = 0;
        let mut total_releases = 0;
        let mut total_elisions = 0;

        // Analyze each function
        for func in &ir.functions {
            let info = self.analyze_function(func);

            if ctx.debug.verbose {
                eprintln!(
                    "[arc] Function '{}': {} retains, {} releases, {} elisions, {} ref-typed locals",
                    info.function_name,
                    info.retains.len(),
                    info.releases.len(),
                    info.elision_count,
                    info.ref_type_locals
                );
            }

            total_retains += info.retains.len();
            total_releases += info.releases.len();
            total_elisions += info.elision_count;
        }

        // Analyze tests too
        for test in &ir.tests {
            // Create a pseudo-function for analysis
            let test_func = TFunction {
                name: test.name.clone(),
                public: false,
                params: vec![],
                return_type: Type::Void,
                locals: test.locals.clone(),
                body: test.body.clone(),
                span: test.span.clone(),
            };
            let info = self.analyze_function(&test_func);
            total_retains += info.retains.len();
            total_releases += info.releases.len();
        }

        if ctx.debug.verbose {
            eprintln!(
                "[arc] Module total: {} retains, {} releases, {} elisions",
                total_retains, total_releases, total_elisions
            );
        }

        // The pass analyzes but doesn't transform the IR directly
        // ARC operations are emitted during code generation based on type classification
        Ok(PassResult::unchanged())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{LocalTable, TExpr, TExprKind, TFunction, TModule, Type};
    use crate::passes::DebugConfig;

    fn make_test_module() -> TModule {
        let mut module = TModule::new("test".to_string());

        // Create a simple function with a string local
        let mut locals = LocalTable::new();
        locals.add("s".to_string(), Type::Str, false, false);

        let func = TFunction {
            name: "test_func".to_string(),
            public: false,
            params: vec![],
            return_type: Type::Void,
            locals,
            body: TExpr::new(TExprKind::String("hello".to_string()), Type::Str, 0..1),
            span: 0..10,
        };

        module.functions.push(func);
        module
    }

    #[test]
    fn test_arc_pass_runs() {
        let pass = ArcInsertionPass::new();
        let mut module = make_test_module();
        let mut ctx = PassContext::with_debug(DebugConfig::default());

        let result = pass.run(&mut module, &mut ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn test_arc_pass_name() {
        let pass = ArcInsertionPass::new();
        assert_eq!(pass.name(), "arc_insertion");
    }

    #[test]
    fn test_arc_pass_required() {
        let pass = ArcInsertionPass::new();
        assert!(pass.required());
    }

    #[test]
    fn test_arc_pass_requires_pattern_lowering() {
        let pass = ArcInsertionPass::new();
        assert!(pass.requires().contains(&"pattern_lowering"));
    }
}
