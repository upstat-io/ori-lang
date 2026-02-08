//! Two-pass function compilation for V2 codegen.
//!
//! `FunctionCompiler` implements the declare-then-define pattern:
//!
//! 1. **Phase 1 (declare)**: Walk all functions, compute `FunctionAbi` from
//!    `ori_types::FunctionSig`, declare LLVM functions with correct types,
//!    calling conventions, and attributes (sret, noalias).
//!
//! 2. **Phase 2 (define)**: Walk all functions again, create `ExprLowerer`
//!    for each, bind parameters to scope, lower body expression, emit return.
//!
//! This replaces `ModuleCompiler::compile_function_with_sig()` and
//! `compile_test()` with ABI-driven compilation that gets calling conventions
//! and sret handling correct from the start.

use ori_ir::{ExprArena, ExprId, Function, Name, StringInterner, TestDef};
use ori_types::{FunctionSig, Idx, Pool};
use rustc_hash::FxHashMap;
use tracing::{debug, trace, warn};

use super::abi::{compute_function_abi, CallConv, FunctionAbi, ParamPassing, ReturnPassing};
use super::expr_lowerer::ExprLowerer;
use super::ir_builder::IrBuilder;
use super::scope::Scope;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::FunctionId;

// ---------------------------------------------------------------------------
// FunctionCompiler
// ---------------------------------------------------------------------------

/// Two-pass function compiler.
///
/// Holds the mapping from function `Name` → `(FunctionId, FunctionAbi)`,
/// enabling call sites to look up the callee's ABI for correct argument
/// passing (direct vs. sret).
pub struct FunctionCompiler<'a, 'scx, 'ctx, 'tcx> {
    builder: &'a mut IrBuilder<'scx, 'ctx>,
    type_info: &'a TypeInfoStore<'tcx>,
    type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
    interner: &'a StringInterner,
    pool: &'tcx Pool,
    /// Declared functions: `Name` → (`FunctionId`, ABI).
    functions: FxHashMap<Name, (FunctionId, FunctionAbi)>,
}

impl<'a, 'scx: 'ctx, 'ctx, 'tcx> FunctionCompiler<'a, 'scx, 'ctx, 'tcx> {
    /// Create a new function compiler.
    pub fn new(
        builder: &'a mut IrBuilder<'scx, 'ctx>,
        type_info: &'a TypeInfoStore<'tcx>,
        type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
        interner: &'a StringInterner,
        pool: &'tcx Pool,
    ) -> Self {
        Self {
            builder,
            type_info,
            type_resolver,
            interner,
            pool,
            functions: FxHashMap::default(),
        }
    }

    // -----------------------------------------------------------------------
    // Phase 1: Declare
    // -----------------------------------------------------------------------

    /// Declare all module functions from type checker signatures.
    ///
    /// Iterates over `module.functions` paired with their `FunctionSig` from the
    /// type checker. Generic functions are skipped (they require monomorphization).
    pub fn declare_all(&mut self, module_functions: &[Function], function_sigs: &[FunctionSig]) {
        for (func, sig) in module_functions.iter().zip(function_sigs.iter()) {
            // Skip generic functions
            if sig.is_generic() {
                trace!(
                    name = %self.interner.lookup(func.name),
                    "skipping generic function declaration"
                );
                continue;
            }

            self.declare_function(func.name, sig);
        }
    }

    /// Declare a single function from its type checker signature.
    fn declare_function(&mut self, name: Name, sig: &FunctionSig) {
        let name_str = self.interner.lookup(name);
        let abi = compute_function_abi(sig, self.type_info);

        debug!(
            name = name_str,
            params = abi.params.len(),
            call_conv = ?abi.call_conv,
            return_passing = ?abi.return_abi.passing,
            "declaring function"
        );

        // Build LLVM parameter types
        let mut llvm_param_types = Vec::with_capacity(abi.params.len() + 1);

        // If sret, the first LLVM param is the hidden return pointer
        let return_llvm_type = self.type_resolver.resolve(abi.return_abi.ty);
        let return_llvm_id = self.builder.register_type(return_llvm_type);

        if matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }) {
            // Hidden sret pointer as first param
            llvm_param_types.push(self.builder.ptr_type());
        }

        // User-visible parameters
        for param in &abi.params {
            match &param.passing {
                ParamPassing::Direct => {
                    let ty = self.type_resolver.resolve(param.ty);
                    llvm_param_types.push(self.builder.register_type(ty));
                }
                ParamPassing::Indirect { .. } => {
                    // Passed as pointer
                    llvm_param_types.push(self.builder.ptr_type());
                }
                ParamPassing::Void => {
                    // Not physically passed — skip
                }
            }
        }

        // Declare the LLVM function
        let func_id = match &abi.return_abi.passing {
            ReturnPassing::Direct => {
                self.builder
                    .declare_function(name_str, &llvm_param_types, return_llvm_id)
            }
            ReturnPassing::Sret { .. } | ReturnPassing::Void => self
                .builder
                .declare_void_function(name_str, &llvm_param_types),
        };

        // Set calling convention
        match abi.call_conv {
            CallConv::Fast => self.builder.set_fastcc(func_id),
            CallConv::C => self.builder.set_ccc(func_id),
        }

        // Apply sret attributes
        if let ReturnPassing::Sret { .. } = &abi.return_abi.passing {
            self.builder.add_sret_attribute(func_id, 0, return_llvm_id);
            self.builder.add_noalias_attribute(func_id, 0);
        }

        self.functions.insert(name, (func_id, abi));
    }

    // -----------------------------------------------------------------------
    // Phase 2: Define
    // -----------------------------------------------------------------------

    /// Define all module function bodies.
    ///
    /// Must be called after `declare_all()`. For each non-generic function,
    /// creates an `ExprLowerer`, binds parameters, lowers the body, and emits
    /// the return instruction.
    pub fn define_all(
        &mut self,
        module_functions: &[Function],
        function_sigs: &[FunctionSig],
        arena: &ExprArena,
        expr_types: &[Idx],
    ) {
        for (func, sig) in module_functions.iter().zip(function_sigs.iter()) {
            if sig.is_generic() {
                continue;
            }

            // Look up previously declared function
            let Some(&(func_id, ref abi)) = self.functions.get(&func.name) else {
                warn!(
                    name = %self.interner.lookup(func.name),
                    "function not declared — skipping definition"
                );
                continue;
            };
            let abi = abi.clone();

            self.define_function_body(func.name, func_id, &abi, func.body, arena, expr_types);
        }
    }

    /// Define a single function body.
    fn define_function_body(
        &mut self,
        name: Name,
        func_id: FunctionId,
        abi: &FunctionAbi,
        body: ExprId,
        arena: &ExprArena,
        expr_types: &[Idx],
    ) {
        let name_str = self.interner.lookup(name);
        debug!(name = name_str, "defining function body");

        // Create entry block
        let entry_block = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry_block);
        self.builder.set_current_function(func_id);

        // Bind parameters to scope
        let scope = self.bind_parameters(func_id, abi);

        // Lower the body expression
        let mut lowerer = ExprLowerer::new(
            self.builder,
            self.type_info,
            self.type_resolver,
            scope,
            arena,
            expr_types,
            self.interner,
            self.pool,
            func_id,
            &self.functions,
        );

        let result = lowerer.lower(body);

        // Check if the block is already terminated (e.g., by panic, break, unreachable)
        if let Some(block) = self.builder.current_block() {
            if self.builder.block_has_terminator(block) {
                return;
            }
        }

        // Emit return instruction based on ABI
        match &abi.return_abi.passing {
            ReturnPassing::Sret { .. } => {
                // Store result through the hidden sret pointer, then ret void
                if let Some(val) = result {
                    let sret_ptr = self.builder.get_param(func_id, 0);
                    self.builder.store(val, sret_ptr);
                }
                self.builder.ret_void();
            }
            ReturnPassing::Direct => {
                if let Some(val) = result {
                    self.builder.ret(val);
                } else {
                    // No value produced — shouldn't happen for Direct return, but be safe
                    warn!(name = name_str, "direct return function produced no value");
                    self.builder.ret_void();
                }
            }
            ReturnPassing::Void => {
                self.builder.ret_void();
            }
        }
    }

    /// Bind function parameters to a `Scope`, accounting for sret offset.
    fn bind_parameters(&mut self, func_id: FunctionId, abi: &FunctionAbi) -> Scope {
        let mut scope = Scope::new();
        let has_sret = matches!(abi.return_abi.passing, ReturnPassing::Sret { .. });
        let param_offset: u32 = u32::from(has_sret);

        let mut llvm_param_idx = param_offset;
        for param in &abi.params {
            match &param.passing {
                ParamPassing::Direct | ParamPassing::Indirect { .. } => {
                    let param_val = self.builder.get_param(func_id, llvm_param_idx);
                    let name_str = self.interner.lookup(param.name);
                    self.builder.set_value_name(param_val, name_str);

                    scope.bind_immutable(param.name, param_val);
                    llvm_param_idx += 1;
                }
                ParamPassing::Void => {
                    // Void parameters aren't physically passed — no LLVM param to bind
                }
            }
        }

        scope
    }

    // -----------------------------------------------------------------------
    // Tests and Impls
    // -----------------------------------------------------------------------

    /// Compile test definitions as void → void wrapper functions.
    ///
    /// Returns a map of `test_name → wrapper_function_name` for the JIT to call.
    pub fn compile_tests(
        &mut self,
        tests: &[&TestDef],
        arena: &ExprArena,
        expr_types: &[Idx],
    ) -> FxHashMap<Name, String> {
        let mut test_wrappers = FxHashMap::default();

        for test in tests {
            let test_name_str = self.interner.lookup(test.name);
            let wrapper_name = format!("__test_{test_name_str}");

            debug!(name = test_name_str, wrapper = %wrapper_name, "compiling test");

            // Declare void → void wrapper
            let func_id = self.builder.declare_void_function(&wrapper_name, &[]);
            self.builder.set_fastcc(func_id);

            // Define body
            let entry_block = self.builder.append_block(func_id, "entry");
            self.builder.position_at_end(entry_block);
            self.builder.set_current_function(func_id);

            let mut lowerer = ExprLowerer::new(
                self.builder,
                self.type_info,
                self.type_resolver,
                Scope::new(),
                arena,
                expr_types,
                self.interner,
                self.pool,
                func_id,
                &self.functions,
            );

            lowerer.lower(test.body);

            // Ensure terminator
            if let Some(block) = self.builder.current_block() {
                if !self.builder.block_has_terminator(block) {
                    self.builder.ret_void();
                }
            }

            test_wrappers.insert(test.name, wrapper_name);
        }

        test_wrappers
    }

    /// Compile impl block methods.
    ///
    /// Impl methods are compiled as regular functions. The method name is used
    /// directly (mangling is a future step in 04.5).
    pub fn compile_impls(
        &mut self,
        impls: &[ori_ir::ImplDef],
        impl_sigs: &[(Name, FunctionSig)],
        arena: &ExprArena,
        expr_types: &[Idx],
    ) {
        // Build a lookup map for impl method sigs
        let sig_map: FxHashMap<Name, &FunctionSig> =
            impl_sigs.iter().map(|(name, sig)| (*name, sig)).collect();

        for impl_def in impls {
            for method in &impl_def.methods {
                if let Some(sig) = sig_map.get(&method.name) {
                    if sig.is_generic() {
                        continue;
                    }

                    self.declare_function(method.name, sig);
                    let Some(&(func_id, ref abi)) = self.functions.get(&method.name) else {
                        continue;
                    };
                    let abi = abi.clone();

                    self.define_function_body(
                        method.name,
                        func_id,
                        &abi,
                        method.body,
                        arena,
                        expr_types,
                    );
                } else {
                    // No sig available — use a simple void function as placeholder
                    trace!(
                        name = %self.interner.lookup(method.name),
                        "no type signature for impl method — skipping"
                    );
                }
            }
        }
    }

    /// Declare external imported functions (for multi-module AOT compilation).
    pub fn declare_imports(&mut self, imports: &[(Name, FunctionSig)]) {
        for (name, sig) in imports {
            self.declare_function(*name, sig);
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Look up a declared function by name.
    pub fn get_function(&self, name: Name) -> Option<&(FunctionId, FunctionAbi)> {
        self.functions.get(&name)
    }

    /// Borrow the function map (for call-site ABI lookup).
    pub fn function_map(&self) -> &FxHashMap<Name, (FunctionId, FunctionAbi)> {
        &self.functions
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
    use crate::context::SimpleCx;
    use inkwell::context::Context;
    use ori_ir::Name;
    use ori_types::{Idx, Pool};
    use std::mem::ManuallyDrop;

    /// Create a basic FunctionSig for testing.
    fn make_sig(
        name: Name,
        param_names: Vec<Name>,
        param_types: Vec<Idx>,
        return_type: Idx,
        is_main: bool,
    ) -> FunctionSig {
        let required_params = param_types.len();
        FunctionSig {
            name,
            type_params: vec![],
            param_names,
            param_types,
            return_type,
            capabilities: vec![],
            is_public: false,
            is_test: false,
            is_main,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            required_params,
        }
    }

    // Note: SimpleCx has a Drop impl (LLVM module), which interacts with the
    // drop checker when other locals borrow `&scx`. We use ManuallyDrop to
    // suppress the drop checker's conservative analysis. The LLVM context
    // outlives all these locals (it owns the actual memory), so this is safe.

    #[test]
    fn declare_simple_function() {
        let pool = Pool::new();
        let ctx = Context::create();
        let interner = StringInterner::new();
        let store = TypeInfoStore::new(&pool);
        let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_declare"));
        let resolver = TypeLayoutResolver::new(&store, &scx);
        let mut builder = IrBuilder::new(&scx);

        let func_name = interner.intern("add");
        let a_name = interner.intern("a");
        let b_name = interner.intern("b");

        let sig = make_sig(
            func_name,
            vec![a_name, b_name],
            vec![Idx::INT, Idx::INT],
            Idx::INT,
            false,
        );

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool);
        fc.declare_function(func_name, &sig);

        let (_func_id, abi) = fc.get_function(func_name).unwrap();
        assert_eq!(abi.params.len(), 2);
        assert_eq!(abi.return_abi.passing, ReturnPassing::Direct);
        assert_eq!(abi.call_conv, CallConv::Fast);

        assert!(scx.llmod.get_function("add").is_some());
    }

    #[test]
    fn declare_void_function() {
        let pool = Pool::new();
        let ctx = Context::create();
        let interner = StringInterner::new();
        let store = TypeInfoStore::new(&pool);
        let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_void"));
        let resolver = TypeLayoutResolver::new(&store, &scx);
        let mut builder = IrBuilder::new(&scx);

        let func_name = interner.intern("do_thing");
        let sig = make_sig(func_name, vec![], vec![], Idx::UNIT, false);

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool);
        fc.declare_function(func_name, &sig);

        let (_, abi) = fc.get_function(func_name).unwrap();
        assert_eq!(abi.return_abi.passing, ReturnPassing::Void);
    }

    #[test]
    fn declare_sret_function() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);
        let ctx = Context::create();
        let interner = StringInterner::new();
        let store = TypeInfoStore::new(&pool);
        let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_sret"));
        let resolver = TypeLayoutResolver::new(&store, &scx);
        let mut builder = IrBuilder::new(&scx);

        let func_name = interner.intern("get_list");
        let sig = make_sig(func_name, vec![], vec![], list_int, false);

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool);
        fc.declare_function(func_name, &sig);

        let (_, abi) = fc.get_function(func_name).unwrap();
        assert!(matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }));

        // Must drop borrowers of scx before accessing scx directly
        drop(fc);
        drop(builder);
        drop(resolver);

        let llvm_fn = scx.llmod.get_function("get_list").unwrap();
        assert!(llvm_fn.get_type().get_return_type().is_none());
        assert_eq!(llvm_fn.count_params(), 1);
    }

    #[test]
    fn declare_main_uses_c_calling_convention() {
        let pool = Pool::new();
        let ctx = Context::create();
        let interner = StringInterner::new();
        let store = TypeInfoStore::new(&pool);
        let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_main_cc"));
        let resolver = TypeLayoutResolver::new(&store, &scx);
        let mut builder = IrBuilder::new(&scx);

        let func_name = interner.intern("main");
        let sig = make_sig(func_name, vec![], vec![], Idx::UNIT, true);

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool);
        fc.declare_function(func_name, &sig);

        let (_, abi) = fc.get_function(func_name).unwrap();
        assert_eq!(abi.call_conv, CallConv::C);
    }

    #[test]
    fn generic_functions_are_skipped() {
        let pool = Pool::new();
        let ctx = Context::create();
        let interner = StringInterner::new();
        let store = TypeInfoStore::new(&pool);
        let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_generic_skip"));
        let resolver = TypeLayoutResolver::new(&store, &scx);
        let mut builder = IrBuilder::new(&scx);

        let func_name = interner.intern("identity");
        let t_name = interner.intern("T");
        let sig = FunctionSig {
            name: func_name,
            type_params: vec![t_name],
            param_names: vec![],
            param_types: vec![],
            return_type: Idx::UNIT,
            capabilities: vec![],
            is_public: false,
            is_test: false,
            is_main: false,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            required_params: 0,
        };

        let func = Function {
            name: func_name,
            generics: ori_ir::GenericParamRange::EMPTY,
            params: ori_ir::ParamRange::EMPTY,
            return_ty: None,
            capabilities: vec![],
            where_clauses: vec![],
            guard: None,
            body: ExprId::INVALID,
            span: ori_ir::Span::new(0, 0),
            visibility: ori_ir::Visibility::Private,
        };

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool);
        fc.declare_all(&[func], &[sig]);

        assert!(fc.get_function(func_name).is_none());

        // Must drop borrowers of scx before accessing scx directly
        drop(fc);
        drop(builder);
        drop(resolver);
        assert!(scx.llmod.get_function("identity").is_none());
    }

    #[test]
    fn function_map_returns_all_declared() {
        let pool = Pool::new();
        let ctx = Context::create();
        let interner = StringInterner::new();
        let store = TypeInfoStore::new(&pool);
        let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_map"));
        let resolver = TypeLayoutResolver::new(&store, &scx);
        let mut builder = IrBuilder::new(&scx);

        let add_name = interner.intern("add");
        let sub_name = interner.intern("sub");
        let a_name = interner.intern("a");
        let b_name = interner.intern("b");

        let sig_add = make_sig(
            add_name,
            vec![a_name, b_name],
            vec![Idx::INT, Idx::INT],
            Idx::INT,
            false,
        );
        let sig_sub = make_sig(
            sub_name,
            vec![a_name, b_name],
            vec![Idx::INT, Idx::INT],
            Idx::INT,
            false,
        );

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool);
        fc.declare_function(add_name, &sig_add);
        fc.declare_function(sub_name, &sig_sub);

        assert_eq!(fc.function_map().len(), 2);
        assert!(fc.function_map().contains_key(&add_name));
        assert!(fc.function_map().contains_key(&sub_name));
    }
}
