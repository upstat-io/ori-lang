// ARC Memory Management for Sigil
//
// This module implements pure Automatic Reference Counting (ARC) with
// compile-time cycle prevention for the Sigil language.
//
// ## Architecture
//
// The module is organized into several sub-modules:
//
// - `traits`: Core trait definitions following SOLID principles
// - `ids`: Newtype IDs for type safety
// - `analysis`: Compile-time type analysis (classification, cycle detection, size calculation)
// - `codegen`: Code generation (insertion, elision, scope tracking, emission)
// - `runtime`: C runtime library generation
// - `debug`: Leak detection and allocation tracking
// - `classifier`: Exhaustive classifier traits for mandatory ARC enforcement
// - `analyzer`: Exhaustive ARC analyzer implementing all classifiers
// - `validated`: ArcValidatedModule newtype for type-state enforcement
//
// ## Key Design Decisions
//
// 1. **Value vs Reference Types**: Types <= 32 bytes are value types (copied),
//    larger types are reference-counted.
//
// 2. **Compile-time Cycle Prevention**: Instead of weak references, we detect
//    cyclic type definitions at compile time and reject them.
//
// 3. **Scope-based Destruction**: Locals are released in reverse creation order
//    when exiting a scope.
//
// 4. **Elision Optimization**: Reference counting is elided when safe (unique
//    ownership, immediate consumption, moves).
//
// 5. **Mandatory Enforcement**: The type-state pattern (ArcValidatedModule) and
//    exhaustive classifier traits make it impossible to forget ARC handling
//    when adding new IR variants.

pub mod ids;
pub mod traits;

pub mod analysis;
pub mod analyzer;
pub mod classifier;
pub mod codegen;
pub mod debug;
pub mod runtime;
pub mod validated;

// Re-export commonly used types
pub use ids::{LocalId, ScopeId, TypeClassId, TypeId};
pub use traits::{
    // Type classification
    StorageClass,
    TypeClassification,
    TypeClassifier,
    // Cycle detection
    CycleCheckResult,
    CycleDetector,
    CycleInfo,
    TypeNode,
    TypeReference,
    TypeReferenceGraph,
    // Reference counting
    ElisionOpportunity,
    ElisionReason,
    RefCountAnalyzer,
    ReleasePoint,
    ReleaseReason,
    RetainPoint,
    RetainReason,
    // Emission
    ArcConfig,
    ArcEmitter,
    // Debug
    AllocationEntry,
    AllocationTracker,
};

// Re-export concrete implementations
pub use analysis::{DefaultCycleDetector, DefaultTypeClassifier, TypeSizeCalculator};
pub use codegen::{DefaultRefCountAnalyzer, ScopeTracker};
pub use debug::DefaultAllocationTracker;
pub use runtime::DefaultArcEmitter;

// Re-export exhaustive classifier types
pub use classifier::{
    // Traits
    ArcExprClassifier,
    ArcMatchPatternClassifier,
    ArcPatternClassifier,
    ArcTypeClassifier,
    // Dispatch functions (the enforcement points)
    classify_expr,
    classify_match_pattern,
    classify_pattern,
    classify_type as classify_type_exhaustive,
    // Info types
    ChildVisit,
    ExprArcInfo,
    MatchPatternArcInfo,
    PatternArcInfo,
    TypeArcInfo,
};

// Re-export analyzer
pub use analyzer::ExhaustiveArcAnalyzer;

// Re-export validated module types
pub use validated::{
    ArcError,
    ArcResult,
    ArcSummary,
    ArcValidatedModule,
    FunctionArcInfo,
    LocalArcInfo,
    ModuleArcInfo,
};

use crate::ir::{TModule, TTypeDef, Type};

// =============================================================================
// Convenience Functions
// =============================================================================

/// Check a type definition for cycles
///
/// This is the main entry point for cycle detection during type checking.
pub fn check_type_cycles(type_def: &TTypeDef, module: &TModule) -> CycleCheckResult {
    let detector = DefaultCycleDetector::new(module);
    detector.check_type(type_def)
}

/// Classify a type for ARC purposes
///
/// Returns information about how the type should be stored and managed.
pub fn classify_type(ty: &Type) -> TypeClassification {
    let classifier = DefaultTypeClassifier::new();
    classifier.classify(ty)
}

/// Check if a type is a value type (no ARC needed)
pub fn is_value_type(ty: &Type) -> bool {
    classify_type(ty).is_value()
}

/// Get the size of a type in bytes
pub fn size_of_type(ty: &Type) -> usize {
    TypeSizeCalculator::new().size_of(ty)
}

/// Create an ARC emitter with default configuration
pub fn create_emitter() -> DefaultArcEmitter {
    DefaultArcEmitter::new()
}

/// Create an ARC emitter with custom configuration
pub fn create_emitter_with_config(config: ArcConfig) -> DefaultArcEmitter {
    DefaultArcEmitter::with_config(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_primitives() {
        assert!(is_value_type(&Type::Int));
        assert!(is_value_type(&Type::Float));
        assert!(is_value_type(&Type::Bool));
    }

    #[test]
    fn test_classify_reference_types() {
        assert!(!is_value_type(&Type::List(Box::new(Type::Int))));
        assert!(!is_value_type(&Type::Map(Box::new(Type::Str), Box::new(Type::Int))));
    }

    #[test]
    fn test_size_of_primitives() {
        assert_eq!(size_of_type(&Type::Int), 8);
        assert_eq!(size_of_type(&Type::Float), 8);
        assert_eq!(size_of_type(&Type::Bool), 1);
    }
}
