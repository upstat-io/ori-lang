//! Component structs for `TypeChecker` organization.
//!
//! These structs group related fields for better organization and testability.
//!
//! # Component Architecture
//!
//! `TypeChecker` is organized into logical components:
//! - `CheckContext`: Immutable references to arena and interner
//! - `InferenceState`: Mutable inference context, environments, and expression types
//! - `Registries`: Pattern, type operator, type, and trait registries
//! - `DiagnosticState`: Error collection and diagnostic queue
//! - `ScopeContext`: Function signatures, impl Self type, config types, capabilities

use rustc_hash::{FxHashMap, FxHashSet};

use ori_diagnostic::queue::DiagnosticQueue;
use ori_ir::{ExprArena, Name, StringInterner, TypeId};
use ori_patterns::PatternRegistry;
use ori_types::{InferenceContext, SharedTypeInterner, Type, TypeEnv};

use super::types::{FunctionType, TypeCheckError};
use crate::infer::builtin_methods::BuiltinMethodRegistry;
use crate::registry::{TraitRegistry, TypeRegistry};
use crate::shared::SharedRegistry;

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
    /// Inferred types for expressions (stored as `TypeId` for efficiency).
    pub expr_types: FxHashMap<usize, TypeId>,
}

impl InferenceState {
    /// Create a new inference state.
    pub fn new() -> Self {
        Self {
            ctx: InferenceContext::new(),
            env: TypeEnv::new(),
            base_env: None,
            expr_types: FxHashMap::default(),
        }
    }

    /// Create a new inference state with a shared type interner.
    ///
    /// Use this when you need to share the type interner with other code
    /// (e.g., for tests that need to verify `TypeId` values).
    pub fn with_type_interner(interner: SharedTypeInterner) -> Self {
        Self {
            ctx: InferenceContext::with_interner(interner.clone()),
            env: TypeEnv::with_interner(interner),
            base_env: None,
            expr_types: FxHashMap::default(),
        }
    }
}

/// Registry bundle for patterns, types, and traits.
pub struct Registries {
    /// Pattern registry for `function_exp` type checking.
    pub pattern: SharedRegistry<PatternRegistry>,
    /// Registry for user-defined types (structs, enums, aliases).
    pub types: TypeRegistry,
    /// Registry for traits and implementations.
    pub traits: TraitRegistry,
    /// Registry for built-in method type checking (ZST handlers).
    pub builtin_methods: BuiltinMethodRegistry,
}

impl Registries {
    /// Create a new registries bundle with default values.
    pub fn new() -> Self {
        Self {
            pattern: SharedRegistry::new(PatternRegistry::new()),
            types: TypeRegistry::new(),
            traits: TraitRegistry::new(),
            builtin_methods: BuiltinMethodRegistry::new(),
        }
    }

    /// Create registries with a custom pattern registry.
    pub fn with_pattern_registry(pattern: SharedRegistry<PatternRegistry>) -> Self {
        Self {
            pattern,
            types: TypeRegistry::new(),
            traits: TraitRegistry::new(),
            builtin_methods: BuiltinMethodRegistry::new(),
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
    pub function_sigs: FxHashMap<Name, FunctionType>,
    /// The Self type when inside an impl block.
    pub current_impl_self: Option<Type>,
    /// Config variable types for $name references.
    pub config_types: FxHashMap<Name, Type>,
    /// Capabilities declared by the current function (from `uses` clause).
    pub current_function_caps: FxHashSet<Name>,
    /// Capabilities currently provided by `with...in` expressions in scope.
    pub provided_caps: FxHashSet<Name>,
    /// The type of the current function being checked.
    /// Used for patterns like `recurse` that need `self` to have the enclosing function's signature.
    pub current_function_type: Option<Type>,
}

impl ScopeContext {
    /// Create a new scope context.
    pub fn new() -> Self {
        Self {
            function_sigs: FxHashMap::default(),
            current_impl_self: None,
            config_types: FxHashMap::default(),
            current_function_caps: FxHashSet::default(),
            provided_caps: FxHashSet::default(),
            current_function_type: None,
        }
    }
}
