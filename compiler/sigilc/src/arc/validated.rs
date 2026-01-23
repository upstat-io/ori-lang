// ARC Validated Module
//
// This module implements the type-state pattern for mandatory ARC enforcement.
// A TModule can only be converted to ArcValidatedModule by running exhaustive
// ARC analysis, and code generation ONLY accepts ArcValidatedModule.
//
// This ensures that:
// 1. ARC analysis is mandatory - you cannot generate code without it
// 2. Adding new IR variants forces handling them for ARC (Rust compiler errors)
// 3. ARC information is pre-computed and available during codegen

use std::collections::HashMap;

use crate::ir::{LocalId, TModule, Type};

use super::traits::{ReleasePoint, RetainPoint, StorageClass, TypeClassification};

// =============================================================================
// Module-Level ARC Information
// =============================================================================

/// ARC analysis results for an entire module.
///
/// This contains pre-computed retain/release points and type classifications
/// that can be used during code generation.
#[derive(Debug, Clone)]
pub struct ModuleArcInfo {
    /// Per-function ARC information
    pub functions: HashMap<String, FunctionArcInfo>,

    /// Per-test ARC information
    pub tests: HashMap<String, FunctionArcInfo>,

    /// Type classifications cache
    pub type_classes: HashMap<String, TypeClassification>,
}

impl ModuleArcInfo {
    /// Create empty ARC info
    pub fn new() -> Self {
        ModuleArcInfo {
            functions: HashMap::new(),
            tests: HashMap::new(),
            type_classes: HashMap::new(),
        }
    }

    /// Get ARC info for a function by name
    pub fn get_function(&self, name: &str) -> Option<&FunctionArcInfo> {
        self.functions.get(name)
    }

    /// Get ARC info for a test by name
    pub fn get_test(&self, name: &str) -> Option<&FunctionArcInfo> {
        self.tests.get(name)
    }
}

impl Default for ModuleArcInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// ARC information for a single function
#[derive(Debug, Clone)]
pub struct FunctionArcInfo {
    /// Function name
    pub name: String,

    /// Points where retains are needed
    pub retains: Vec<RetainPoint>,

    /// Points where releases are needed
    pub releases: Vec<ReleasePoint>,

    /// Number of elision opportunities found
    pub elision_count: usize,

    /// Number of reference-typed locals
    pub ref_type_locals: usize,

    /// Per-local ARC classification
    pub local_arc_info: HashMap<LocalId, LocalArcInfo>,
}

impl FunctionArcInfo {
    /// Create new function ARC info
    pub fn new(name: String) -> Self {
        FunctionArcInfo {
            name,
            retains: Vec::new(),
            releases: Vec::new(),
            elision_count: 0,
            ref_type_locals: 0,
            local_arc_info: HashMap::new(),
        }
    }
}

/// ARC information for a local variable
#[derive(Debug, Clone)]
pub struct LocalArcInfo {
    /// The local's ID
    pub local_id: LocalId,

    /// The local's type
    pub ty: Type,

    /// Storage class for this local
    pub storage: StorageClass,

    /// Whether this local needs ARC management
    pub needs_arc: bool,

    /// Whether this local needs destruction at scope exit
    pub needs_destruction: bool,
}

// =============================================================================
// ARC Error Types
// =============================================================================

/// Errors that can occur during ARC validation
#[derive(Debug, Clone)]
pub enum ArcError {
    /// A type contains a cycle that would cause memory leaks
    CyclicType {
        type_name: String,
        cycle_path: Vec<String>,
    },

    /// An expression kind was not handled by the classifier
    /// This should never happen if the classifier is exhaustive
    UnhandledExpression {
        expr_kind: String,
        location: String,
    },

    /// A type was not handled by the classifier
    UnhandledType {
        type_name: String,
        location: String,
    },

    /// A pattern was not handled by the classifier
    UnhandledPattern {
        pattern_kind: String,
        location: String,
    },

    /// Internal error during analysis
    InternalError(String),
}

impl std::fmt::Display for ArcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArcError::CyclicType {
                type_name,
                cycle_path,
            } => {
                write!(
                    f,
                    "Cyclic type '{}' would cause memory leaks: {}",
                    type_name,
                    cycle_path.join(" -> ")
                )
            }
            ArcError::UnhandledExpression {
                expr_kind,
                location,
            } => {
                write!(
                    f,
                    "Expression kind '{}' not handled by ARC classifier at {}",
                    expr_kind, location
                )
            }
            ArcError::UnhandledType {
                type_name,
                location,
            } => {
                write!(
                    f,
                    "Type '{}' not handled by ARC classifier at {}",
                    type_name, location
                )
            }
            ArcError::UnhandledPattern {
                pattern_kind,
                location,
            } => {
                write!(
                    f,
                    "Pattern kind '{}' not handled by ARC classifier at {}",
                    pattern_kind, location
                )
            }
            ArcError::InternalError(msg) => {
                write!(f, "Internal ARC error: {}", msg)
            }
        }
    }
}

impl std::error::Error for ArcError {}

// =============================================================================
// ARC Validated Module (Type-State Pattern)
// =============================================================================

/// A TModule that has been validated for ARC correctness.
///
/// This type can ONLY be constructed by running ARC analysis through
/// `ArcValidatedModule::validate()`. This enforces at the type level
/// that ARC analysis has been performed.
///
/// Code generation accepts ONLY this type, not raw TModule.
#[derive(Debug)]
pub struct ArcValidatedModule {
    /// The underlying module
    module: TModule,

    /// Pre-computed ARC information
    arc_info: ModuleArcInfo,
}

impl ArcValidatedModule {
    /// Validate a TModule for ARC correctness.
    ///
    /// This is the ONLY way to construct an ArcValidatedModule.
    /// It runs exhaustive ARC analysis on the module, ensuring
    /// every expression, type, and pattern is classified.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The module contains cyclic types
    /// - Any IR variant is not handled by the classifier (should not happen)
    pub fn validate(module: TModule) -> Result<Self, ArcError> {
        use super::analyzer::ExhaustiveArcAnalyzer;

        let analyzer = ExhaustiveArcAnalyzer::new();
        let arc_info = analyzer.analyze_module(&module)?;

        Ok(ArcValidatedModule { module, arc_info })
    }

    /// Get a reference to the underlying module
    pub fn module(&self) -> &TModule {
        &self.module
    }

    /// Get a reference to the pre-computed ARC info
    pub fn arc_info(&self) -> &ModuleArcInfo {
        &self.arc_info
    }

    /// Consume the validated module to get the inner TModule and ArcInfo
    pub fn into_parts(self) -> (TModule, ModuleArcInfo) {
        (self.module, self.arc_info)
    }

    /// Get the module name
    pub fn name(&self) -> &str {
        &self.module.name
    }
}

// =============================================================================
// Convenience Types
// =============================================================================

/// Result type for ARC operations
pub type ArcResult<T> = Result<T, ArcError>;

/// Summary of ARC analysis for reporting
#[derive(Debug, Clone, Default)]
pub struct ArcSummary {
    /// Total number of retain operations needed
    pub total_retains: usize,

    /// Total number of release operations needed
    pub total_releases: usize,

    /// Total number of elision opportunities
    pub total_elisions: usize,

    /// Total number of reference-typed locals
    pub total_ref_locals: usize,

    /// Number of functions analyzed
    pub functions_analyzed: usize,

    /// Number of tests analyzed
    pub tests_analyzed: usize,
}

impl ArcSummary {
    /// Create summary from module ARC info
    pub fn from_arc_info(info: &ModuleArcInfo) -> Self {
        let mut summary = ArcSummary::default();

        for func_info in info.functions.values() {
            summary.total_retains += func_info.retains.len();
            summary.total_releases += func_info.releases.len();
            summary.total_elisions += func_info.elision_count;
            summary.total_ref_locals += func_info.ref_type_locals;
            summary.functions_analyzed += 1;
        }

        for test_info in info.tests.values() {
            summary.total_retains += test_info.retains.len();
            summary.total_releases += test_info.releases.len();
            summary.total_elisions += test_info.elision_count;
            summary.total_ref_locals += test_info.ref_type_locals;
            summary.tests_analyzed += 1;
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{LocalTable, TExpr, TExprKind, TFunction};

    fn make_simple_module() -> TModule {
        let mut module = TModule::new("test".to_string());

        // Create a simple function that returns an int
        let func = TFunction {
            name: "simple".to_string(),
            public: false,
            params: vec![],
            return_type: Type::Int,
            locals: LocalTable::new(),
            body: TExpr::new(TExprKind::Int(42), Type::Int, 0..1),
            span: 0..10,
        };

        module.functions.push(func);
        module
    }

    #[test]
    fn test_arc_validated_module_creation() {
        let module = make_simple_module();
        let result = ArcValidatedModule::validate(module);
        assert!(result.is_ok());

        let validated = result.unwrap();
        assert_eq!(validated.name(), "test");
    }

    #[test]
    fn test_arc_info_access() {
        let module = make_simple_module();
        let validated = ArcValidatedModule::validate(module).unwrap();

        let arc_info = validated.arc_info();
        assert!(arc_info.functions.contains_key("simple"));
    }

    #[test]
    fn test_arc_summary() {
        let module = make_simple_module();
        let validated = ArcValidatedModule::validate(module).unwrap();

        let summary = ArcSummary::from_arc_info(validated.arc_info());
        assert_eq!(summary.functions_analyzed, 1);
    }

    #[test]
    fn test_into_parts() {
        let module = make_simple_module();
        let validated = ArcValidatedModule::validate(module).unwrap();

        let (module, arc_info) = validated.into_parts();
        assert_eq!(module.name, "test");
        assert!(arc_info.functions.contains_key("simple"));
    }

    #[test]
    fn test_arc_error_display() {
        let err = ArcError::CyclicType {
            type_name: "Node".to_string(),
            cycle_path: vec!["Node".to_string(), "child".to_string(), "Node".to_string()],
        };
        assert!(err.to_string().contains("Node"));

        let err = ArcError::UnhandledExpression {
            expr_kind: "FooBar".to_string(),
            location: "test.si:10".to_string(),
        };
        assert!(err.to_string().contains("FooBar"));
    }

    #[test]
    fn test_module_arc_info_default() {
        let info = ModuleArcInfo::default();
        assert!(info.functions.is_empty());
        assert!(info.tests.is_empty());
    }

    #[test]
    fn test_function_arc_info_new() {
        let info = FunctionArcInfo::new("test".to_string());
        assert_eq!(info.name, "test");
        assert!(info.retains.is_empty());
        assert!(info.releases.is_empty());
    }
}
