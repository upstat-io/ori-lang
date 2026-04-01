//! ABI boundary classification for integer narrowing.
//!
//! At function boundaries, narrowed integers must be widened back to
//! canonical width (i64) to preserve ABI correctness. This module
//! defines the boundary classification and policy rules that Phase B
//! (local variable narrowing) and Phase C (collection element narrowing)
//! use to decide when widening is required.
//!
//! # Boundary Rules
//!
//! - **Internal calls**: Both caller and callee are in the same compilation
//!   unit. If both agree on the narrowed type (via Merkle hash), the narrow
//!   type can cross the boundary without widening.
//! - **Public API**: External callers expect canonical i64. All parameters
//!   and return values must be canonical-width at public function boundaries.
//! - **FFI**: C ABI dictates the width. The `c_int`/`c_long` types map to
//!   platform-specific widths, NOT to Ori's narrowed integers.
//! - **Trait methods**: Unknown implementors may call through vtable dispatch.
//!   Parameters and returns must be canonical-width.
//! - **Closure captures**: Capture slots are stored in the closure environment
//!   at creation time. Narrowing captured values would require the closure
//!   layout to encode per-capture widths — deferred to a future section.
//!
//! # Architecture
//!
//! `abi.rs` provides the **policy layer** — it classifies boundaries and
//! answers "is narrowing allowed here?" The actual `sext`/`trunc` insertion
//! is a codegen concern handled by the LLVM codegen integration.
//!
//! # ARC IR Limitations
//!
//! The ARC IR (`ArcFunction`) currently lacks visibility, trait-method,
//! and closure-parameter metadata. Until these are added, classification
//! functions accept explicit boolean flags. When ARC IR gains these fields,
//! the classification can be derived directly from `ArcFunction`.

use crate::repr::IntWidth;

/// Classifies the ABI boundary type a value crosses at a function edge.
///
/// Each variant encodes a different set of narrowing constraints.
/// The ordering is deliberate: more restrictive boundaries are listed
/// first, matching the priority when multiple classifications apply
/// (e.g., a public FFI function is `Ffi`, not `PublicApi`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbiBoundary {
    /// FFI call — must match C ABI (platform-specific widths).
    ///
    /// FFI boundaries are the most restrictive: the width is dictated
    /// by the C ABI, not by Ori's narrowing analysis. `c_int` may be
    /// 32-bit on most platforms but 16-bit on some embedded targets.
    Ffi,

    /// Public function — must use canonical i64.
    ///
    /// External callers (other modules, dynamic linking) expect the
    /// canonical Ori `int` representation. Narrowing would silently
    /// break the ABI contract.
    PublicApi,

    /// Trait method — must use canonical (unknown callers).
    ///
    /// Trait methods may be called through vtable dispatch by code
    /// that doesn't know the concrete type. Parameters and returns
    /// must be canonical-width to maintain the trait's ABI contract.
    TraitMethod,

    /// Closure capture — parameter types fixed at creation.
    ///
    /// Closure environments store captured values at canonical width.
    /// Narrowing captured `int` fields would require per-capture width
    /// metadata in the closure layout, which is not yet implemented.
    ClosureCapture,

    /// Internal function call — can use narrow types if both sides agree.
    ///
    /// When caller and callee are in the same compilation unit and both
    /// agree on the narrowed type (verified via Merkle hash of the
    /// `MachineRepr`), the narrow type can cross the boundary without
    /// widening. This is the only boundary that permits narrowing.
    InternalCall,
}

/// What width a value must have when crossing a given ABI boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidthRequirement {
    /// Must be i64 — no narrowing allowed at this boundary.
    ///
    /// The value must be widened (`sext`) to canonical i64 before
    /// crossing the boundary, and truncated back after if the
    /// receiving side uses a narrowed local.
    Canonical,

    /// May use narrow type if caller and callee agree on the width.
    ///
    /// Both sides must produce the same `MachineRepr` for the parameter
    /// or return type (verified via Merkle hash). If they disagree,
    /// falls back to `Canonical`.
    NarrowIfAgreed,

    /// Must match platform C ABI width (not Ori's canonical i64).
    ///
    /// For `c_int`, this is typically i32 on most platforms.
    /// The repr-opt pipeline does not narrow FFI parameters — they
    /// are always emitted at their declared C type width.
    PlatformCabi,
}

/// Cross-module narrowing agreement status.
///
/// When a function call crosses a module boundary, the caller and callee
/// may have different `MachineRepr` decisions for the same parameter type.
/// The Merkle hash of `MachineRepr` is used to detect agreement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossModuleAgreement {
    /// Both modules computed the same `MachineRepr` for this type.
    /// The Merkle hash matches — narrow type can cross the boundary.
    Agreed,

    /// Modules computed different `MachineRepr` values.
    /// The Merkle hash differs — widen to canonical at boundary.
    Disagreed,

    /// Agreement status not yet determined (conservative: treat as `Disagreed`).
    ///
    /// This is the default until cross-module Merkle hash comparison
    /// is implemented in the linker or module system.
    Unknown,
}

/// Determine the width requirement for a given ABI boundary.
///
/// This is the core policy function: given a boundary classification,
/// it returns what width constraint applies.
#[must_use]
pub fn width_requirement(boundary: AbiBoundary) -> WidthRequirement {
    match boundary {
        AbiBoundary::Ffi => WidthRequirement::PlatformCabi,
        AbiBoundary::PublicApi | AbiBoundary::TraitMethod | AbiBoundary::ClosureCapture => {
            WidthRequirement::Canonical
        }
        AbiBoundary::InternalCall => WidthRequirement::NarrowIfAgreed,
    }
}

/// Check if a function parameter can be narrowed at the given boundary.
///
/// Returns `true` only for `InternalCall` boundaries where both sides
/// can agree on the narrowed width. All other boundaries require
/// canonical or C-ABI width.
#[must_use]
pub fn can_narrow_param(boundary: AbiBoundary) -> bool {
    matches!(boundary, AbiBoundary::InternalCall)
}

/// Check if a function return value can be narrowed at the given boundary.
///
/// Same rules as parameters: only internal calls allow narrow returns.
#[must_use]
pub fn can_narrow_return(boundary: AbiBoundary) -> bool {
    matches!(boundary, AbiBoundary::InternalCall)
}

/// Whether narrowing is allowed given cross-module agreement status.
///
/// Even at an `InternalCall` boundary, narrowing is only safe when
/// both modules agree on the `MachineRepr`. `Unknown` is treated
/// conservatively as `Disagreed`.
#[must_use]
pub fn can_narrow_cross_module(agreement: CrossModuleAgreement) -> bool {
    matches!(agreement, CrossModuleAgreement::Agreed)
}

/// Metadata about a function's ABI-relevant properties.
///
/// Bundles the flags needed to classify a function's ABI boundary.
/// Replaces individual boolean parameters per the ">3 params → config
/// struct" and "no boolean flags" coding guidelines.
///
/// # Note on ARC IR
///
/// The ARC IR (`ArcFunction`) does not yet carry visibility, trait-method,
/// or closure-parameter metadata. Callers must extract these flags from
/// the type checker or other sources. When ARC IR gains these fields,
/// a `From<&ArcFunction>` impl should be added.
#[derive(Debug, Clone, Copy, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "ABI boundary flags are genuinely boolean properties — \
              a function is or isn't FFI/public/trait/closure"
)]
pub struct FunctionBoundaryInfo {
    /// Function is an FFI declaration (`extern "c"`).
    pub is_ffi: bool,
    /// Function has `pub` visibility (external callers possible).
    pub is_public: bool,
    /// Function is a trait method implementation (unknown callers via vtable).
    pub is_trait_method: bool,
    /// Function is a closure (capture slots fixed at creation).
    pub is_closure: bool,
}

/// Classify a function's ABI boundary from available metadata.
///
/// Priority order (most restrictive first):
/// 1. FFI → `Ffi`
/// 2. Public → `PublicApi`
/// 3. Trait method → `TraitMethod`
/// 4. Closure → `ClosureCapture`
/// 5. Otherwise → `InternalCall`
#[must_use]
pub fn classify_function_boundary(info: FunctionBoundaryInfo) -> AbiBoundary {
    if info.is_ffi {
        return AbiBoundary::Ffi;
    }
    if info.is_public {
        return AbiBoundary::PublicApi;
    }
    if info.is_trait_method {
        return AbiBoundary::TraitMethod;
    }
    if info.is_closure {
        return AbiBoundary::ClosureCapture;
    }
    AbiBoundary::InternalCall
}

/// Determine the effective width for a parameter at a boundary.
///
/// Given the narrowed width from range analysis and the ABI boundary,
/// returns the width that should be used at the boundary edge:
/// - `Canonical` boundaries → always `I64`
/// - `PlatformCabi` boundaries → always `I64` (Ori `int` maps to i64 in C)
/// - `NarrowIfAgreed` with `Agreed` → the narrowed width
/// - `NarrowIfAgreed` with `Disagreed`/`Unknown` → `I64`
#[must_use]
pub fn effective_boundary_width(
    narrowed_width: IntWidth,
    boundary: AbiBoundary,
    agreement: CrossModuleAgreement,
) -> IntWidth {
    match width_requirement(boundary) {
        WidthRequirement::Canonical | WidthRequirement::PlatformCabi => IntWidth::I64,
        WidthRequirement::NarrowIfAgreed => {
            if can_narrow_cross_module(agreement) {
                narrowed_width
            } else {
                IntWidth::I64
            }
        }
    }
}

/// Whether a `sext` (sign-extend) is needed when a value crosses a boundary.
///
/// Returns `true` when the narrowed width differs from the effective
/// boundary width — meaning the value must be sign-extended before
/// crossing the boundary.
#[must_use]
pub fn needs_sext_at_boundary(
    narrowed_width: IntWidth,
    boundary: AbiBoundary,
    agreement: CrossModuleAgreement,
) -> bool {
    let effective = effective_boundary_width(narrowed_width, boundary, agreement);
    narrowed_width != effective
}

/// Whether a `trunc` (truncation) is needed when a value arrives from a boundary.
///
/// Returns `true` when the incoming canonical-width value should be
/// truncated to the locally-narrowed width after crossing the boundary
/// into a function that uses narrowed locals.
#[must_use]
pub fn needs_trunc_after_boundary(
    local_narrowed_width: IntWidth,
    boundary: AbiBoundary,
    agreement: CrossModuleAgreement,
) -> bool {
    // If the boundary doesn't require canonical width, no trunc needed.
    let boundary_width = effective_boundary_width(local_narrowed_width, boundary, agreement);
    local_narrowed_width != boundary_width
}
