//! Typed failures produced while closing an executable program.

use ori_ir::Name;

/// A failure to construct a closed backend-neutral executable program.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RealizationError {
    /// The artifact schema version is not supported by this compiler.
    #[error("unsupported executable-program version {found}; expected {expected}")]
    UnsupportedVersion { found: u32, expected: u32 },
    /// The program contains more functions than its stable index can represent.
    #[error("executable program contains too many functions: {count}")]
    TooManyFunctions { count: usize },
    /// A function name is absent from the supplied immutable symbol table.
    #[error("function name {name:?} is absent from the executable symbol table")]
    UnknownFunctionName { name: Name },
    /// Two realized function bodies use the same stable name.
    #[error("duplicate realized function {name:?}")]
    DuplicateFunction { name: Name },
    /// A validated function was not assigned an executable identity.
    #[error("realized function {name:?} has no executable function identity")]
    MissingFunctionIdentity { name: Name },
    /// The selected entry point is not one of the realized functions.
    #[error("entry point {name:?} has no realized function body")]
    MissingEntryPoint { name: Name },
    /// A direct call has no realized function or runtime descriptor.
    #[error(
        "executable-program realization cannot resolve call to '{callee_symbol}' from '{caller_symbol}': no realized function body or runtime operation is registered; add a realized body or RuntimeCall mapping for '{callee_symbol}' before selecting an executable backend"
    )]
    MissingCallable {
        /// Stable caller identity.
        caller: Name,
        /// Stable callee identity.
        callee: Name,
        /// Human-readable caller spelling captured before symbol storage moves.
        caller_symbol: Box<str>,
        /// Human-readable callee spelling captured before symbol storage moves.
        callee_symbol: Box<str>,
    },
    /// A closure constructor names a runtime descriptor instead of a function body.
    #[error("function {caller:?} captures non-function target {callee:?}")]
    InvalidClosureTarget { caller: Name, callee: Name },
    /// A block index cannot be represented in the executable call-site table.
    #[error("function {function:?} has too many basic blocks")]
    TooManyBlocks { function: Name },
    /// An instruction index cannot be represented in the executable call-site table.
    #[error("a basic block in function {function:?} has too many instructions")]
    TooManyInstructions { function: Name },
}
