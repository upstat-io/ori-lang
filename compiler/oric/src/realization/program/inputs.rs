//! Explicit inputs the shared realization entry derives its plans from.
//!
//! Every driver supplies the same checked frontend facts plus one resolved
//! policy value; the shared entry derives the registry, representation plan,
//! user-drop bindings, roots, and entry point from them.

use ori_ir::{Name, StringInterner};
use ori_parse::ParseOutput;
use ori_types::{FunctionSig, TypeCheckResult};

/// Narrowing and verification policy resolved once by each realization driver.
#[derive(Debug, Clone, Copy)]
pub struct RealizationPolicy {
    /// Representation narrowing policy selected for this compilation.
    pub narrowing: ori_repr::NarrowingPolicy,
    /// Run the optional ARC consistency oracle while freezing the artifact.
    pub verify_arc: bool,
}

impl RealizationPolicy {
    /// Resolve narrowing and ARC-oracle gates from the process environment.
    ///
    /// Env: `ORI_NO_REPR_OPT` — disables representation narrowing; read through
    /// [`ori_repr::NarrowingPolicy::env_disabled`].
    #[must_use]
    pub fn from_env() -> Self {
        let narrowing = if ori_repr::NarrowingPolicy::env_disabled() {
            ori_repr::NarrowingPolicy::Disabled
        } else {
            ori_repr::NarrowingPolicy::Aggressive
        };
        Self::with_narrowing(narrowing)
    }

    /// Pair a driver-selected narrowing policy with the environment ARC gate.
    ///
    /// Env: `ORI_VERIFY_ARC` — enables expensive ARC correctness checks, debug-only.
    #[must_use]
    pub fn with_narrowing(narrowing: ori_repr::NarrowingPolicy) -> Self {
        Self {
            narrowing,
            verify_arc: std::env::var(crate::debug_flags::ORI_VERIFY_ARC)
                .is_ok_and(|value| value != "0"),
        }
    }
}

/// Cross-module representation metadata the local plan must agree with.
#[derive(Clone, Copy, Default)]
pub(crate) struct ImportedReprSurfaces<'a> {
    /// Exported layout metadata from every imported module.
    pub type_metadata: &'a [ori_types::ExportedTypeMetadata],
    /// Exported collection surfaces from every imported module.
    pub collection_surfaces: &'a [u64],
}

/// Checked frontend facts every realization driver supplies unchanged.
#[derive(Clone, Copy)]
pub(crate) struct CheckedModuleFacts<'a> {
    /// Parsed module whose function order pairs with `function_sigs`.
    pub parse: &'a ParseOutput,
    /// Type-checked module metadata.
    pub types: &'a TypeCheckResult,
    /// Signatures aligned with `parse.module.functions`.
    pub function_sigs: &'a [FunctionSig],
    /// Interner backing representation planning.
    pub interner: &'a StringInterner,
    /// Imported layout metadata this module links against.
    pub imported_repr: ImportedReprSurfaces<'a>,
}

impl CheckedModuleFacts<'_> {
    /// Report whether analysis covers impl methods that carry no codegen body.
    ///
    /// Narrowing is suppressed for such modules because field-range summaries
    /// from analysis-only functions can narrow structs crossing ABI boundaries.
    pub(crate) fn has_analysis_only_functions(&self) -> bool {
        self.types
            .typed
            .impl_sigs
            .iter()
            .any(|entry| !entry.sig.is_generic())
    }

    /// Return the distinguished standalone-process entry, when one is declared.
    pub(crate) fn cli_entry(&self) -> Option<Name> {
        self.parse
            .module
            .functions
            .iter()
            .zip(self.function_sigs)
            .find_map(|(function, signature)| signature.is_main.then_some(function.name))
    }
}
