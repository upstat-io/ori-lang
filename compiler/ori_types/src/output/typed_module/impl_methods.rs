//! Resolved local and imported implementation-method identities.

use ori_ir::{ExprId, Name};
use ori_registry::burden::FnSym;

use crate::{FunctionSig, Idx};

/// One impl method's owning receiver type, method name, and resolved signature.
///
/// Accumulated in `ModuleChecker::impl_sigs` during `check_impl_bodies`
/// (`register_impl_sig`) and threaded — unchanged — through `ori_canon`
/// desugaring (method-call param resolution), `ori_llvm` monomorphization
/// (`collect_mono_functions`), and codegen impl-method compilation
/// (`compile_impls`). `receiver` is the owning impl block's receiver type
/// (e.g. `Box<T>` for `impl<T> Box<T>`); codegen keys mono-collection
/// dispatch on it rather than on `sig.param_types.first()`, which is the
/// first VALUE param — not the receiver — for a no-`self` associated function.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct ImplMethodId {
    /// Position of the owning impl block in `Module::impls`.
    impl_index: usize,
    /// Canonical source body. The impl index distinguishes one inherited
    /// default body instantiated for multiple impl blocks.
    body: ExprId,
}

impl ImplMethodId {
    /// Construct the stable identity assigned by the type checker.
    #[must_use]
    pub const fn new(impl_index: usize, body: ExprId) -> Self {
        Self { impl_index, body }
    }

    /// Owning impl block's module-local position.
    #[must_use]
    pub const fn impl_index(self) -> usize {
        self.impl_index
    }

    /// Canonical source/default body identity.
    #[must_use]
    pub const fn body(self) -> ExprId {
        self.body
    }
}

/// Frontend-owned semantic role of an impl method.
///
/// Downstream consumers match this value, never the source spelling of the
/// trait or method. `UserDrop` is minted only while the type checker has the
/// exact resolved `Drop` trait identity in hand.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ImplMethodRole {
    /// Ordinary inherent or trait method.
    Ordinary,
    /// Body implementing one logical user-defined destruction operation.
    UserDrop { logical: FnSym },
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImplSig {
    /// Exact source/default method identity.
    pub id: ImplMethodId,
    /// The impl block's receiver type.
    pub receiver: Idx,
    /// Exact trait implemented by the owning block; `None` for inherent impls.
    pub trait_type: Option<Idx>,
    /// The method's name.
    pub name: Name,
    /// Semantic role assigned by the type-checker authority.
    pub role: ImplMethodRole,
    /// The method's resolved signature.
    pub sig: FunctionSig,
}

/// One producer-owned impl method template imported into this module.
///
/// Every type coordinate is re-created in the importing module's pool. The
/// exact producer remains stable across module-local `ExprId` and impl-index
/// spaces through [`crate::MethodProducer::Imported`].
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImportedImplSig {
    /// Stable producer-module identity for this exact method body.
    pub producer: crate::MethodProducer,
    /// Generic receiver pattern in the importing module's pool.
    pub receiver: Idx,
    /// Exact implemented trait in the importing module's pool.
    pub trait_type: Option<Idx>,
    /// Source method name.
    pub name: Name,
    /// Whether the source signature includes `self` as its first parameter.
    pub has_self: bool,
    /// Importer-pool method signature used to close concrete mono demands.
    pub sig: FunctionSig,
}
