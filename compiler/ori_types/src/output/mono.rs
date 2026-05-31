//! Monomorphization output types.
//!
//! Const-generic argument values, generic substitution arguments, discovered
//! monomorphization instances, and deferred generic-calling-generic call records.

use ori_ir::ExprId;
use ori_ir::Name;

use crate::Idx;

/// A compile-time value used as a const generic argument.
///
/// Phase 1: unused (only [`GenericArg::Type`] variants).
/// Phase 2+: const generics (`$N: int`, `$B: bool`).
/// Phase 3+: any type `with Eq, Hashable` (`$C: Color`, `$S: [int]`).
///
/// Each variant must be `Eq + Hash`, mirroring Ori's requirement that
/// const-eligible types implement `Eq + Hashable`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ConstValue {
    /// Integer constant (`$N: int → 42`).
    Int(i64),
    /// Boolean constant (`$B: bool → true`).
    Bool(bool),
    // Future phases add variants as const generic eligibility expands:
    // Str(Name), Char(char), Byte(u8),
    // Enum { type_name: Name, variant: Name },
    // List(Vec<ConstValue>), Tuple(Vec<ConstValue>),
}

/// A concrete argument to a generic parameter.
///
/// Unifies type substitution (`T → int`) and const value substitution
/// (`$N → 42`). Parallel to the function's generic parameter list.
///
/// This design matches the convergent pattern across reference compilers:
/// - Rust: `GenericArgKind::Type | Const | Lifetime`
/// - Zig: uniform `InternPool.Index` for types and comptime values
/// - Lean 4: `Expr`-based key (types and values are both expressions)
///
/// Using a single enum avoids impedance mismatches when generic parameter
/// lists contain both type and const params (e.g., `@f<T with Clone, $N: int>`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GenericArg {
    /// Type parameter substitution: `T → int`.
    Type(Idx),
    /// Const generic value substitution: `$N → 42`, `$C → Color.Red`.
    Const(ConstValue),
}

pub use ori_ir::canon::MonoInstanceId;

/// A concrete instantiation of a generic function or method discovered during type checking.
///
/// Two shapes share this struct, distinguished by `receiver_type`:
///
/// - **Top-level** generic function (`@identity<T>(x: T) -> T`): `generic_args`
///   populated; `impl_args`, `method_args` empty; `receiver_type = None`.
/// - **Method instance** (any function inside an `impl` block, including
///   associated functions without `self`, e.g. `Box::new()`): `impl_args` and
///   `method_args` carry the impl-level and method-level type arguments;
///   `receiver_type = Some(Self)` (the enclosing impl's `Self` type, even when
///   the function takes no `self` parameter); `generic_args` empty.
///
/// `receiver_type` is `Some` for ANY function inside an impl block — including
/// associated functions like `Box.new()` and `Option.new()` — and is the
/// distinguishing field for cross-container dedup (`Box<int>.new()` vs
/// `Option<int>.new()` differ only in `receiver_type`). `None` is reserved
/// EXCLUSIVELY for top-level free functions declared outside any impl block.
///
/// Construct via `new_top_level` or `new_method` — inline struct-literal
/// construction is banned (the constructors enforce the invariant `exactly
/// one of (generic_args populated) OR (impl_args + method_args populated +
/// receiver_type populated)`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MonoInstance {
    /// The generic function or method being instantiated.
    pub fn_name: Name,
    /// Concrete generic arguments for top-level functions.
    ///
    /// Empty for method instances (use `impl_args` + `method_args`).
    pub generic_args: Vec<GenericArg>,
    /// Concrete impl-level type arguments for method instances.
    ///
    /// Empty for top-level instances. Populated from the enclosing impl block's
    /// generic parameter list (e.g. `impl<T> Box<T> { ... }` instantiated at
    /// `T = int` produces `impl_args = [int]`).
    pub impl_args: Vec<GenericArg>,
    /// Concrete method-level type arguments for method instances.
    ///
    /// Empty for top-level instances. Populated from the method's own generic
    /// parameter list (e.g. `@m<U>(self) -> U` instantiated at `U = str`
    /// produces `method_args = [str]`).
    pub method_args: Vec<GenericArg>,
    /// The enclosing impl block's `Self` type, for any function inside an
    /// impl block (including associated functions without `self`).
    ///
    /// `None` ONLY for top-level free functions declared outside any impl block.
    /// Critical for cross-container dedup discrimination — `Box<int>.new()` and
    /// `Option<int>.new()` carry distinct `receiver_type` values even when their
    /// `concrete_param_types` and `concrete_return_type` post-substitution match.
    pub receiver_type: Option<Idx>,
    /// Substituted parameter types (all type variables replaced with concrete types).
    pub concrete_param_types: Vec<Idx>,
    /// Substituted return type.
    pub concrete_return_type: Idx,
    /// Maps generic `Idx` → concrete `Idx` for body expression types.
    ///
    /// Sorted by key for deterministic `Eq`/`Hash` (required by Salsa early
    /// cutoff). The ARC lowerer converts this to `FxHashMap` for O(1) lookup
    /// when lowering the shared canonical IR body into a monomorphized ARC
    /// function (matching Swift's clone-and-substitute strategy).
    pub body_type_map: Vec<(Idx, Idx)>,
}

impl MonoInstance {
    /// Construct a top-level (non-method) generic function instance.
    ///
    /// Sets `impl_args` and `method_args` to empty, `receiver_type` to `None`.
    /// Asserts the resulting instance satisfies the top-level/method invariant.
    pub fn new_top_level(
        fn_name: Name,
        generic_args: Vec<GenericArg>,
        concrete_param_types: Vec<Idx>,
        concrete_return_type: Idx,
        body_type_map: Vec<(Idx, Idx)>,
    ) -> Self {
        let inst = Self {
            fn_name,
            generic_args,
            impl_args: Vec::new(),
            method_args: Vec::new(),
            receiver_type: None,
            concrete_param_types,
            concrete_return_type,
            body_type_map,
        };
        inst.check_invariants();
        inst
    }

    /// Construct a method instance (any function inside an impl block,
    /// including associated functions without `self`).
    ///
    /// Sets `generic_args` to empty. `receiver_type` MUST be the enclosing
    /// impl block's `Self` type (the post-substitution concrete form).
    /// Asserts the resulting instance satisfies the top-level/method invariant.
    pub fn new_method(
        fn_name: Name,
        impl_args: Vec<GenericArg>,
        method_args: Vec<GenericArg>,
        receiver_type: Idx,
        concrete_param_types: Vec<Idx>,
        concrete_return_type: Idx,
        body_type_map: Vec<(Idx, Idx)>,
    ) -> Self {
        let inst = Self {
            fn_name,
            generic_args: Vec::new(),
            impl_args,
            method_args,
            receiver_type: Some(receiver_type),
            concrete_param_types,
            concrete_return_type,
            body_type_map,
        };
        inst.check_invariants();
        inst
    }

    /// Enforce the top-level / method invariant.
    ///
    /// `assert!` (not `debug_assert!`): a release-build mangling collision
    /// from a violated invariant is silent miscompilation per CLAUDE.md
    /// §Debug/Release Parity. The check is O(1) (Vec emptiness + Option tag),
    /// so the runtime cost is negligible relative to the soundness guarantee.
    fn check_invariants(&self) {
        if self.receiver_type.is_some() {
            assert!(
                self.generic_args.is_empty(),
                "MonoInstance invariant violated: method instance for \
                 {:?} must have empty generic_args; populate impl_args + \
                 method_args instead (got generic_args = {:?})",
                self.fn_name,
                self.generic_args,
            );
        } else {
            assert!(
                self.impl_args.is_empty(),
                "MonoInstance invariant violated: top-level instance for \
                 {:?} must have empty impl_args (got impl_args = {:?})",
                self.fn_name,
                self.impl_args,
            );
            assert!(
                self.method_args.is_empty(),
                "MonoInstance invariant violated: top-level instance for \
                 {:?} must have empty method_args (got method_args = {:?})",
                self.fn_name,
                self.method_args,
            );
        }
    }
}

/// How a callee's type variable binds during a generic-calling-generic call.
#[derive(Clone, Debug)]
pub enum DeferredVarBinding {
    /// Maps to the caller's scheme var at this position (deferred until the
    /// caller is instantiated). E.g., `identity`'s `T` → `apply_identity`'s position 0.
    CallerSchemeVar(usize),
    /// Already resolved to a concrete type. E.g., `make_pair`'s `B` → `int` when
    /// called as `make_pair(a: x, b: 99)` inside a generic function.
    Concrete(Idx),
}

/// A deferred monomorphization call: generic function calling another generic.
///
/// Recorded when a generic function's body calls another generic function
/// and at least one type argument is still a type variable. Resolved later
/// via [`resolve_deferred_mono_calls`] when the caller is instantiated.
///
/// # Examples
///
/// Simple chain: `apply_identity<T>(x: T) = identity(x: x)` records
/// `identity`'s `T` → `CallerSchemeVar(0)`.
///
/// Mixed: `wrap_with_int<T>(x: T) = make_pair(a: x, b: 99)` records
/// `make_pair`'s `A` → `CallerSchemeVar(0)`, `B` → `Concrete(Idx::INT)`.
#[derive(Clone, Debug)]
pub struct DeferredMonoCall {
    /// The generic function that contains the call.
    pub caller: Name,
    /// The generic function being called.
    pub callee: Name,
    /// The callee's scheme var IDs (in declaration order).
    pub callee_scheme_var_ids: Vec<u32>,
    /// Maps callee scheme var ID → binding (caller scheme var position or
    /// concrete type). This semantic mapping avoids dependence on pool
    /// union-find state.
    pub var_subst: Vec<(u32, DeferredVarBinding)>,
    /// The callee's parameter types (from its generic signature).
    pub callee_param_types: Vec<Idx>,
    /// The callee's return type (from its generic signature).
    pub callee_return_type: Idx,
    /// The AST `ExprId` of the call expression that recorded this deferred
    /// call. Used by the deferred-resolution path
    /// (`check::exports::resolve_deferred_mono_calls`) to publish a dispatch
    /// entry into `ModuleChecker.mono_dispatch_pre_dedup` once the deferred
    /// call resolves to a concrete `MonoInstance`. After dedup-remap (per
    /// the `check/mod.rs` export pipeline), the entry lands in
    /// `TypedModule.mono_dispatch_map` and flows downstream symmetrically
    /// with eager-path entries.
    pub call_expr_id: ExprId,
}
