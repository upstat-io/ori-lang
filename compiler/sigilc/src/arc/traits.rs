// Core trait definitions for ARC memory management
//
// These traits follow SOLID principles, particularly Interface Segregation:
// - TypeClassifier: For classifying types as value/reference
// - CycleDetector: For detecting cyclic type references at compile time
// - RefCountAnalyzer: For analyzing retain/release insertion points
// - ArcEmitter: For emitting ARC runtime code

use crate::ir::{TFunction, TModule, TTypeDef, Type};

use super::ids::{LocalId, ScopeId, TypeId};

// =============================================================================
// Type Classification
// =============================================================================

/// How a type is stored in memory
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageClass {
    /// Stored inline on the stack (primitives, small structs)
    /// Copied on assignment, no reference counting needed
    Value,

    /// Stored on the heap with reference counting
    /// Reference counted on assignment
    Reference,

    /// Contains both value and reference components
    /// E.g., a small struct with a reference field
    Hybrid,
}

/// Complete classification of a type for ARC purposes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeClassification {
    /// How this type is stored
    pub storage: StorageClass,

    /// Size of this type in bytes (for Value types, this is the inline size)
    pub size_bytes: usize,

    /// Whether this type contains any reference-counted fields
    pub contains_references: bool,

    /// Whether this type requires destruction (releasing references)
    pub requires_destruction: bool,
}

impl TypeClassification {
    /// Create a new value type classification
    pub fn value(size_bytes: usize) -> Self {
        TypeClassification {
            storage: StorageClass::Value,
            size_bytes,
            contains_references: false,
            requires_destruction: false,
        }
    }

    /// Create a new reference type classification
    pub fn reference(size_bytes: usize) -> Self {
        TypeClassification {
            storage: StorageClass::Reference,
            size_bytes,
            contains_references: true,
            requires_destruction: true,
        }
    }

    /// Create a hybrid type classification
    pub fn hybrid(size_bytes: usize) -> Self {
        TypeClassification {
            storage: StorageClass::Hybrid,
            size_bytes,
            contains_references: true,
            requires_destruction: true,
        }
    }

    /// Check if this is a value type
    pub fn is_value(&self) -> bool {
        self.storage == StorageClass::Value
    }

    /// Check if this is a reference type
    pub fn is_reference(&self) -> bool {
        self.storage == StorageClass::Reference
    }
}

/// Trait for classifying types as value or reference types
pub trait TypeClassifier: Send + Sync {
    /// Classify a type for ARC purposes
    fn classify(&self, ty: &Type) -> TypeClassification;

    /// Quick check if a type is a value type (no ARC needed)
    fn is_value_type(&self, ty: &Type) -> bool {
        self.classify(ty).is_value()
    }

    /// Get the size of a type in bytes
    fn size_of(&self, ty: &Type) -> usize {
        self.classify(ty).size_bytes
    }

    /// Check if a type requires destruction
    fn requires_destruction(&self, ty: &Type) -> bool {
        self.classify(ty).requires_destruction
    }
}

// =============================================================================
// Cycle Detection
// =============================================================================

/// Result of checking a type definition for cycles
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CycleCheckResult {
    /// No cycles detected
    Acyclic,

    /// Type directly contains itself
    /// e.g., `type Node = { next: Node }`
    DirectCycle {
        /// Path of fields leading to the cycle
        field_path: Vec<String>,
    },

    /// Type indirectly contains itself through other types
    /// e.g., `type A = { b: B }` and `type B = { a: A }`
    IndirectCycle {
        /// Path of type names forming the cycle
        type_path: Vec<String>,
    },
}

impl CycleCheckResult {
    /// Check if the result represents a cycle
    pub fn is_cyclic(&self) -> bool {
        !matches!(self, CycleCheckResult::Acyclic)
    }

    /// Get a human-readable description of the cycle
    pub fn describe(&self) -> Option<String> {
        match self {
            CycleCheckResult::Acyclic => None,
            CycleCheckResult::DirectCycle { field_path } => {
                Some(format!("type contains itself through: {}", field_path.join(" -> ")))
            }
            CycleCheckResult::IndirectCycle { type_path } => {
                Some(format!("cyclic type reference: {}", type_path.join(" -> ")))
            }
        }
    }
}

/// Information about a detected cycle
#[derive(Debug, Clone)]
pub struct CycleInfo {
    /// Types involved in the cycle (in order)
    pub types: Vec<String>,

    /// Field names connecting the types
    pub fields: Vec<String>,

    /// Whether this is a direct (single-type) cycle
    pub is_direct: bool,
}

/// Graph node for type reference analysis
#[derive(Debug, Clone)]
pub struct TypeNode {
    /// Name of the type
    pub name: String,

    /// Fields that reference other types
    pub references: Vec<TypeReference>,
}

/// A reference from one type to another through a field
#[derive(Debug, Clone)]
pub struct TypeReference {
    /// Name of the field
    pub field_name: String,

    /// Name of the referenced type
    pub target_type: String,

    /// Whether this is a direct reference (not through Option, List, etc.)
    pub is_direct: bool,
}

/// Graph of type references for cycle detection
#[derive(Debug, Clone, Default)]
pub struct TypeReferenceGraph {
    /// All type nodes in the graph
    pub nodes: Vec<TypeNode>,

    /// Edges: (from_type, to_type, field_name)
    pub edges: Vec<(String, String, String)>,
}

impl TypeReferenceGraph {
    /// Create a new empty graph
    pub fn new() -> Self {
        TypeReferenceGraph::default()
    }

    /// Add a type node to the graph
    pub fn add_node(&mut self, node: TypeNode) {
        // Also add edges for each reference
        for reference in &node.references {
            if reference.is_direct {
                self.edges.push((
                    node.name.clone(),
                    reference.target_type.clone(),
                    reference.field_name.clone(),
                ));
            }
        }
        self.nodes.push(node);
    }

    /// Find all cycles in the graph using Tarjan's SCC algorithm
    pub fn find_cycles(&self) -> Vec<CycleInfo> {
        // This will be implemented in cycle_detection.rs
        Vec::new()
    }
}

/// Trait for detecting cycles in type definitions
pub trait CycleDetector: Send + Sync {
    /// Check a single type definition for cycles
    fn check_type(&self, type_def: &TTypeDef) -> CycleCheckResult;

    /// Build a type reference graph for an entire module
    fn build_type_graph(&self, module: &TModule) -> TypeReferenceGraph;

    /// Find all cycles in a module
    fn find_all_cycles(&self, module: &TModule) -> Vec<CycleInfo> {
        let graph = self.build_type_graph(module);
        graph.find_cycles()
    }
}

// =============================================================================
// Reference Count Analysis
// =============================================================================

/// A point where a retain (increment reference count) is needed
#[derive(Debug, Clone)]
pub struct RetainPoint {
    /// Scope where the retain occurs
    pub scope_id: ScopeId,

    /// The local variable being retained
    pub local_id: LocalId,

    /// Type of the value being retained
    pub ty: Type,

    /// Why this retain is needed
    pub reason: RetainReason,
}

/// Reason for a retain operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetainReason {
    /// Value is being stored in a new binding
    Binding,

    /// Value is being passed to a function
    FunctionArg,

    /// Value is being returned from a function
    Return,

    /// Value is being stored in a collection
    CollectionInsert,

    /// Value is being captured by a closure
    ClosureCapture,
}

/// A point where a release (decrement reference count) is needed
#[derive(Debug, Clone)]
pub struct ReleasePoint {
    /// Scope where the release occurs
    pub scope_id: ScopeId,

    /// The local variable being released
    pub local_id: LocalId,

    /// Type of the value being released
    pub ty: Type,

    /// Why this release is needed
    pub reason: ReleaseReason,

    /// Order of this release within the scope (lower = released earlier)
    /// Releases happen in reverse creation order
    pub order: u32,
}

/// Reason for a release operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseReason {
    /// Variable going out of scope
    ScopeExit,

    /// Variable being reassigned
    Reassignment,

    /// Early return from function
    EarlyReturn,

    /// Break/continue leaving scope
    ControlFlow,
}

/// An opportunity to elide (skip) reference counting
#[derive(Debug, Clone)]
pub struct ElisionOpportunity {
    /// The local variable that could skip refcounting
    pub local_id: LocalId,

    /// Type of the value
    pub ty: Type,

    /// Why this elision is valid
    pub reason: ElisionReason,

    /// Estimated benefit (e.g., number of operations avoided)
    pub benefit: u32,
}

/// Reason why refcount elision is possible
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElisionReason {
    /// Only one reference exists (unique ownership)
    UniqueOwnership,

    /// Value is immediately consumed after creation
    ImmediateConsumption,

    /// Value is moved, not copied
    Move,

    /// Copy-on-write optimization is possible
    CopyOnWrite,
}

/// Trait for analyzing reference counting requirements
pub trait RefCountAnalyzer: Send + Sync {
    /// Find all points where retains are needed in a function
    fn retains_needed(&self, func: &TFunction) -> Vec<RetainPoint>;

    /// Find all points where releases are needed in a function
    fn releases_needed(&self, func: &TFunction) -> Vec<ReleasePoint>;

    /// Find opportunities to elide reference counting
    fn elision_opportunities(&self, func: &TFunction) -> Vec<ElisionOpportunity>;
}

// =============================================================================
// ARC Code Emission
// =============================================================================

/// Configuration for ARC runtime generation
#[derive(Debug, Clone)]
pub struct ArcConfig {
    /// Enable thread-safe reference counting
    pub thread_safe: bool,

    /// Enable debug tracking (leak detection)
    pub debug_tracking: bool,

    /// Threshold for SSO (Small String Optimization) in bytes
    pub sso_threshold: usize,

    /// Value type size threshold in bytes (types <= this are copied)
    pub value_type_threshold: usize,
}

impl Default for ArcConfig {
    fn default() -> Self {
        ArcConfig {
            thread_safe: false,
            debug_tracking: false,
            sso_threshold: 22,
            value_type_threshold: 32,
        }
    }
}

impl ArcConfig {
    /// Create config for thread-safe ARC
    pub fn thread_safe() -> Self {
        ArcConfig {
            thread_safe: true,
            ..Default::default()
        }
    }

    /// Create config with debug tracking enabled
    pub fn debug() -> Self {
        ArcConfig {
            debug_tracking: true,
            ..Default::default()
        }
    }
}

/// Trait for emitting ARC runtime code
pub trait ArcEmitter: Send + Sync {
    /// Emit the runtime header file (sigil_arc.h)
    fn emit_runtime_header(&self, config: &ArcConfig) -> String;

    /// Emit the runtime implementation file (sigil_arc.c)
    fn emit_runtime_impl(&self, config: &ArcConfig) -> String;

    /// Emit a retain call for a specific type
    fn emit_retain(&self, ty: &Type, var: &str) -> String;

    /// Emit a release call for a specific type
    fn emit_release(&self, ty: &Type, var: &str) -> String;

    /// Emit scope cleanup code (release multiple locals)
    fn emit_scope_cleanup(&self, releases: &[ReleasePoint]) -> String;

    /// Emit allocation code for a type
    fn emit_alloc(&self, ty: &Type, var: &str) -> String;

    /// Emit deallocation code for a type
    fn emit_dealloc(&self, ty: &Type, var: &str) -> String;
}

// =============================================================================
// Debug Support
// =============================================================================

/// Allocation tracking entry for debugging
#[derive(Debug, Clone)]
pub struct AllocationEntry {
    /// Type ID of the allocated object
    pub type_id: TypeId,

    /// Type name for display
    pub type_name: String,

    /// Current reference count
    pub refcount: usize,

    /// Source location where allocated
    pub source_location: Option<String>,
}

/// Trait for debug allocation tracking
pub trait AllocationTracker: Send + Sync {
    /// Record a new allocation
    fn record_alloc(&self, entry: AllocationEntry);

    /// Record a release
    fn record_release(&self, type_id: TypeId) -> bool;

    /// Get all current allocations (potential leaks)
    fn current_allocations(&self) -> Vec<AllocationEntry>;

    /// Generate a leak report
    fn leak_report(&self) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_classification() {
        let value = TypeClassification::value(8);
        assert!(value.is_value());
        assert!(!value.contains_references);

        let reference = TypeClassification::reference(24);
        assert!(reference.is_reference());
        assert!(reference.contains_references);
    }

    #[test]
    fn test_cycle_check_result() {
        assert!(!CycleCheckResult::Acyclic.is_cyclic());

        let direct = CycleCheckResult::DirectCycle {
            field_path: vec!["next".to_string()],
        };
        assert!(direct.is_cyclic());
        assert!(direct.describe().unwrap().contains("next"));

        let indirect = CycleCheckResult::IndirectCycle {
            type_path: vec!["A".to_string(), "B".to_string(), "A".to_string()],
        };
        assert!(indirect.is_cyclic());
        assert!(indirect.describe().unwrap().contains("A -> B"));
    }

    #[test]
    fn test_arc_config_defaults() {
        let config = ArcConfig::default();
        assert!(!config.thread_safe);
        assert!(!config.debug_tracking);
        assert_eq!(config.value_type_threshold, 32);
    }

    #[test]
    fn test_type_reference_graph() {
        let mut graph = TypeReferenceGraph::new();
        graph.add_node(TypeNode {
            name: "Node".to_string(),
            references: vec![TypeReference {
                field_name: "child".to_string(),
                target_type: "Node".to_string(),
                is_direct: true,
            }],
        });

        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0], ("Node".to_string(), "Node".to_string(), "child".to_string()));
    }
}
