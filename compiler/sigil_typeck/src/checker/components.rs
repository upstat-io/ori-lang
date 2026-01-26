//! Component structs for TypeChecker organization.
//!
//! These structs group related fields for better organization and testability.
//!
//! # Component Architecture
//!
//! TypeChecker is organized into logical components:
//! - `CheckContext`: Immutable references to arena and interner
//! - `InferenceState`: Mutable inference context, environments, and expression types
//! - `Registries`: Pattern, type operator, type, and trait registries
//! - `DiagnosticState`: Error collection and diagnostic queue
//! - `ScopeContext`: Function signatures, impl Self type, config types, capabilities

use std::collections::{HashMap, HashSet};

use sigil_diagnostic::queue::DiagnosticQueue;
use sigil_ir::{ExprArena, Name, StringInterner};
use sigil_types::{InferenceContext, Type, TypeEnv};
use sigil_patterns::PatternRegistry;

use crate::operators::TypeOperatorRegistry;
use crate::registry::{TraitRegistry, TypeRegistry};
use crate::shared::SharedRegistry;
use super::types::{FunctionType, TypeCheckError};

/// Context references for type checking (external, immutable references).
pub struct CheckContext<'a> {
    /// Expression arena for looking up expressions.
    pub arena: &'a ExprArena,
    /// String interner for looking up names.
    pub interner: &'a StringInterner,
}

impl<'a> CheckContext<'a> {
    /// Create a new check context.
    pub fn new(arena: &'a ExprArena, interner: &'a StringInterner) -> Self {
        Self { arena, interner }
    }
}

/// Type inference state (mutable inference context and environments).
#[derive(Default)]
pub struct InferenceState {
    /// Inference context for fresh type variables and unification.
    pub ctx: InferenceContext,
    /// Current type environment (variable bindings).
    pub env: TypeEnv,
    /// Frozen base environment for child scope creation.
    pub base_env: Option<TypeEnv>,
    /// Inferred types for expressions.
    pub expr_types: HashMap<usize, Type>,
}

impl InferenceState {
    /// Create a new inference state.
    pub fn new() -> Self {
        Self {
            ctx: InferenceContext::new(),
            env: TypeEnv::new(),
            base_env: None,
            expr_types: HashMap::new(),
        }
    }
}

/// Registry bundle for patterns, types, and traits.
pub struct Registries {
    /// Pattern registry for `function_exp` type checking.
    pub pattern: SharedRegistry<PatternRegistry>,
    /// Type operator registry for binary operation type checking.
    pub type_op: TypeOperatorRegistry,
    /// Registry for user-defined types (structs, enums, aliases).
    pub types: TypeRegistry,
    /// Registry for traits and implementations.
    pub traits: TraitRegistry,
}

impl Registries {
    /// Create a new registries bundle with default values.
    pub fn new() -> Self {
        Self {
            pattern: SharedRegistry::new(PatternRegistry::new()),
            type_op: TypeOperatorRegistry::new(),
            types: TypeRegistry::new(),
            traits: TraitRegistry::new(),
        }
    }

    /// Create registries with a custom pattern registry.
    pub fn with_pattern_registry(pattern: SharedRegistry<PatternRegistry>) -> Self {
        Self {
            pattern,
            type_op: TypeOperatorRegistry::new(),
            types: TypeRegistry::new(),
            traits: TraitRegistry::new(),
        }
    }
}

impl Default for Registries {
    fn default() -> Self {
        Self::new()
    }
}

/// Diagnostic collection state.
pub struct DiagnosticState {
    /// Collected type check errors.
    pub errors: Vec<TypeCheckError>,
    /// Diagnostic queue for deduplication and error limits.
    pub queue: Option<DiagnosticQueue>,
    /// Source code for line/column computation.
    pub source: Option<String>,
}

impl DiagnosticState {
    /// Create diagnostic state without source.
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            queue: None,
            source: None,
        }
    }

    /// Create diagnostic state with source and queue.
    pub fn with_source(source: String, queue: DiagnosticQueue) -> Self {
        Self {
            errors: Vec::new(),
            queue: Some(queue),
            source: Some(source),
        }
    }
}

impl Default for DiagnosticState {
    fn default() -> Self {
        Self::new()
    }
}

/// Function and scope context state.
#[derive(Default)]
pub struct ScopeContext {
    /// Function signatures for constraint checking during calls.
    pub function_sigs: HashMap<Name, FunctionType>,
    /// The Self type when inside an impl block.
    pub current_impl_self: Option<Type>,
    /// Config variable types for $name references.
    pub config_types: HashMap<Name, Type>,
    /// Capabilities declared by the current function (from `uses` clause).
    pub current_function_caps: HashSet<Name>,
    /// Capabilities currently provided by `with...in` expressions in scope.
    pub provided_caps: HashSet<Name>,
}

impl ScopeContext {
    /// Create a new scope context.
    pub fn new() -> Self {
        Self {
            function_sigs: HashMap::new(),
            current_impl_self: None,
            config_types: HashMap::new(),
            current_function_caps: HashSet::new(),
            provided_caps: HashSet::new(),
        }
    }
}
