//! Monomorphization output types.
//!
//! Const-generic argument values, generic substitution arguments, discovered
//! monomorphization instances, and deferred generic-calling-generic call records.

use ori_ir::ExprId;
use ori_ir::Name;

use crate::{Idx, ImplMethodId, MethodProducer};

pub use ori_ir::canon::GenericConstValue as ConstValue;
pub use ori_ir::canon::MonoConstBinding;

/// Solved or caller-symbolic const term observed during call inference.
///
/// Only `Value` may enter a concrete [`MonoInstance`]. `CallerParam` is kept
/// distinct so a generic caller can defer substitution without fabricating a
/// value or collapsing the const axis into a type `Idx`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ConstGenericTerm {
    /// Concrete value solved at this call site.
    Value(ConstValue),
    /// Const parameter owned by the enclosing generic caller.
    CallerParam(Name),
}

/// A concrete argument to a generic parameter.
///
/// Unifies type substitution (`T → int`) and const value substitution
/// (`$N → 42`). Parallel to the function's generic parameter list.
///
/// Reference compilers likewise use one generic-argument representation:
/// Rust uses `GenericArgKind::Type | Const | Lifetime`, Zig uses a uniform
/// `InternPool.Index`, and Lean 4 uses expression-based keys.
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

/// Exact body producer that owns a deferred generic free-function call.
///
/// A source impl method is qualified by [`ImplMethodId`], not just its spelling:
/// unrelated impls may declare same-named methods whose nested calls must
/// specialize independently. Other [`MethodProducer`] variants do not identify
/// locally checked source bodies and therefore cannot own deferred records.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeferredMonoCaller {
    /// A top-level function or test body.
    TopLevel(Name),
    /// One exact locally checked source/default impl method body.
    ImplMethod { name: Name, id: ImplMethodId },
}

impl DeferredMonoCaller {
    /// Source spelling retained for diagnostics and tracing only.
    #[must_use]
    pub const fn name(self) -> Name {
        match self {
            Self::TopLevel(name) | Self::ImplMethod { name, .. } => name,
        }
    }
}

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
    /// Ordered concrete provider types for value capabilities.
    ///
    /// This lane is distinct from source generics: it participates in
    /// specialization identity and mangling, but never in source-level generic
    /// arity. Provider values of the same ordered types intentionally share
    /// one instance and are passed separately at each call site.
    pub capability_args: Vec<Idx>,
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
    /// Named const bindings for body evaluation/lowering.
    ///
    /// The values are the same `GenericArg::Const` lane used by identity and
    /// mangling; names come from declaration metadata. Concrete instances may
    /// not contain symbolic caller parameters.
    pub const_bindings: Vec<MonoConstBinding>,
    /// The enclosing impl block's `Self` type, for any function inside an
    /// impl block (including associated functions without `self`).
    ///
    /// `None` ONLY for top-level free functions declared outside any impl block.
    /// Critical for cross-container dedup discrimination — `Box<int>.new()` and
    /// `Option<int>.new()` carry distinct `receiver_type` values even when their
    /// `concrete_param_types` and `concrete_return_type` post-substitution match.
    pub receiver_type: Option<Idx>,
    /// Exact checker-selected producer for a method specialization.
    ///
    /// `None` is valid only for a top-level free-function instance.
    pub method_producer: Option<MethodProducer>,
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

/// Concrete receiver/signature coordinates for one method specialization.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConcreteMethodMono {
    pub receiver_type: Idx,
    pub param_types: Vec<Idx>,
    pub return_type: Idx,
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
            capability_args: Vec::new(),
            impl_args: Vec::new(),
            method_args: Vec::new(),
            const_bindings: Vec::new(),
            receiver_type: None,
            method_producer: None,
            concrete_param_types,
            concrete_return_type,
            body_type_map,
        };
        inst.check_invariants();
        inst
    }

    /// Construct a top-level specialization with ordered capability-provider
    /// types in addition to any ordinary generic arguments.
    pub fn new_top_level_with_capabilities(
        fn_name: Name,
        generic_args: Vec<GenericArg>,
        capability_args: Vec<Idx>,
        concrete_param_types: Vec<Idx>,
        concrete_return_type: Idx,
        body_type_map: Vec<(Idx, Idx)>,
    ) -> Self {
        let inst = Self {
            fn_name,
            generic_args,
            capability_args,
            impl_args: Vec::new(),
            method_args: Vec::new(),
            const_bindings: Vec::new(),
            receiver_type: None,
            method_producer: None,
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
        method_producer: MethodProducer,
        impl_args: Vec<GenericArg>,
        method_args: Vec<GenericArg>,
        concrete: ConcreteMethodMono,
    ) -> Self {
        Self::new_method_with_const_bindings(
            fn_name,
            method_producer,
            impl_args,
            method_args,
            Vec::new(),
            concrete,
        )
    }

    /// Construct a method instance with named const values for body lowering.
    pub fn new_method_with_const_bindings(
        fn_name: Name,
        method_producer: MethodProducer,
        impl_args: Vec<GenericArg>,
        method_args: Vec<GenericArg>,
        const_bindings: Vec<MonoConstBinding>,
        concrete: ConcreteMethodMono,
    ) -> Self {
        let inst = Self {
            fn_name,
            generic_args: Vec::new(),
            capability_args: Vec::new(),
            impl_args,
            method_args,
            const_bindings,
            receiver_type: Some(concrete.receiver_type),
            method_producer: Some(method_producer),
            concrete_param_types: concrete.param_types,
            concrete_return_type: concrete.return_type,
            body_type_map: concrete.body_type_map,
        };
        inst.check_invariants();
        inst
    }

    /// Enforce the top-level / method invariant.
    ///
    /// `assert!` (not `debug_assert!`): a release-build mangling collision
    /// from a violated invariant is silent miscompilation, so the soundness
    /// invariant must hold in release builds too. The check is linear only in
    /// the method's generic-argument count, negligible relative to the
    /// soundness guarantee.
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
            assert!(
                self.method_producer.is_some(),
                "MonoInstance invariant violated: method instance for {:?} must carry an exact producer",
                self.fn_name,
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
            assert!(
                self.method_producer.is_none(),
                "MonoInstance invariant violated: top-level instance for {:?} must not carry a method producer",
                self.fn_name,
            );
        }

        let method_const_values: Vec<_> = self
            .method_args
            .iter()
            .filter_map(|arg| match arg {
                GenericArg::Const(value) => Some(value),
                GenericArg::Type(_) => None,
            })
            .collect();
        let binding_values: Vec<_> = self
            .const_bindings
            .iter()
            .map(|binding| &binding.value)
            .collect();
        assert_eq!(
            method_const_values, binding_values,
            "MonoInstance invariant violated: const-valued method_args for {:?} must exactly match ordered const_bindings values",
            self.fn_name,
        );
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
    /// A transient inference type recorded from a non-generic caller: the bound
    /// type carries an unbound inference var (a bare `Tag::Var` OR a composite
    /// such as `[Var]`/`Option<Var>`) at eager-record time but resolves to its
    /// concrete root by the deferred phase. `resolve_deferred_var_subst`
    /// re-resolves `idx` against the now-fully-linked pool
    /// (`substitute_in_pool` follows every interior var link to its concrete
    /// leaf); if any interior var is still unbound, the publish aborts (retry).
    DeferredType { idx: Idx },
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
    /// The exact source body that contains the call.
    pub caller: DeferredMonoCaller,
    /// The generic function being called.
    pub callee: Name,
    /// The callee's scheme var IDs (in declaration order).
    pub callee_scheme_var_ids: Vec<u32>,
    /// Maps callee scheme var ID → binding (caller scheme var position or
    /// concrete type). This semantic mapping avoids dependence on pool
    /// union-find state.
    pub var_subst: Vec<(u32, DeferredVarBinding)>,
    /// Ordered provider binders owned by the callee signature.
    ///
    /// Their bindings live in `var_subst` beside ordinary scheme binders so
    /// one deferred publication substitutes the complete source body. The
    /// order is the provider ABI order and therefore also the capability
    /// specialization identity order.
    pub capability_var_ids: Vec<u32>,
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
    /// `TypedModule.mono_dispatch_map` and follows the same path
    /// with eager-path entries.
    pub call_expr_id: ExprId,
}
