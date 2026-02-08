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

use std::cell::Cell;

use ori_ir::{ExprArena, ExprId, Function, Name, StringInterner, TestDef};
use ori_types::{FunctionSig, Idx, Pool};
use rustc_hash::FxHashMap;
use tracing::{debug, trace, warn};

use crate::aot::mangle::Mangler;

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
    /// Symbol mangler for generating unique LLVM symbol names.
    mangler: Mangler,
    /// Module path for name mangling (e.g., "", "math", "data/utils").
    module_path: &'a str,
    /// Declared functions: `Name` → (`FunctionId`, ABI).
    functions: FxHashMap<Name, (FunctionId, FunctionAbi)>,
    /// Type-qualified method lookup: `(type_name, method_name)` → (`FunctionId`, ABI).
    ///
    /// Allows same-name methods on different types (e.g., `Point.distance` and
    /// `Line.distance`) to coexist without collision. Populated by `compile_impls`.
    method_functions: FxHashMap<(Name, Name), (FunctionId, FunctionAbi)>,
    /// Maps receiver type `Idx` → type `Name` for method dispatch.
    ///
    /// Used by `ExprLowerer` to resolve `expr_type(receiver)` to a type name
    /// for lookup in `method_functions`. Populated by `compile_impls` using
    /// `FunctionSig.param_types[0]` (the self parameter type).
    type_idx_to_name: FxHashMap<Idx, Name>,
    /// Module-wide lambda counter for unique lambda function names.
    lambda_counter: Cell<u32>,
}

impl<'a, 'scx: 'ctx, 'ctx, 'tcx> FunctionCompiler<'a, 'scx, 'ctx, 'tcx> {
    /// Create a new function compiler.
    ///
    /// `module_path` determines name mangling: `""` for the root module,
    /// `"math"` or `"data/utils"` for nested modules. All LLVM symbols
    /// are mangled (e.g., `add` → `_ori_add`, `math.add` → `_ori_math$add`).
    pub fn new(
        builder: &'a mut IrBuilder<'scx, 'ctx>,
        type_info: &'a TypeInfoStore<'tcx>,
        type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
        interner: &'a StringInterner,
        pool: &'tcx Pool,
        module_path: &'a str,
    ) -> Self {
        Self {
            builder,
            type_info,
            type_resolver,
            interner,
            pool,
            mangler: Mangler::new(),
            module_path,
            functions: FxHashMap::default(),
            method_functions: FxHashMap::default(),
            type_idx_to_name: FxHashMap::default(),
            lambda_counter: Cell::new(0),
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
    ///
    /// The LLVM symbol uses the mangled name (e.g., `_ori_add`), while the
    /// `functions` map key uses the interned `Name` for internal lookups.
    fn declare_function(&mut self, name: Name, sig: &FunctionSig) {
        let name_str = self.interner.lookup(name);
        let symbol = self.mangler.mangle_function(self.module_path, name_str);
        self.declare_function_with_symbol(name, &symbol, sig);
    }

    /// Declare a function with an explicit LLVM symbol name.
    ///
    /// Shared implementation for `declare_function` (top-level) and
    /// `declare_impl_method` (impl block methods with type-qualified names).
    fn declare_function_with_symbol(&mut self, name: Name, symbol: &str, sig: &FunctionSig) {
        let name_str = self.interner.lookup(name);
        let abi = compute_function_abi(sig, self.type_info);

        debug!(
            name = name_str,
            symbol,
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

        // Declare the LLVM function using the mangled symbol name
        let func_id = match &abi.return_abi.passing {
            ReturnPassing::Direct => {
                self.builder
                    .declare_function(symbol, &llvm_param_types, return_llvm_id)
            }
            ReturnPassing::Sret { .. } | ReturnPassing::Void => self
                .builder
                .declare_void_function(symbol, &llvm_param_types),
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
            &self.method_functions,
            &self.type_idx_to_name,
            &self.lambda_counter,
            self.module_path,
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
            let wrapper_name = self
                .mangler
                .mangle_function(self.module_path, &format!("test_{test_name_str}"));

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
                &self.method_functions,
                &self.type_idx_to_name,
                &self.lambda_counter,
                self.module_path,
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
    /// Impl methods use type-qualified mangled names: `_ori_[<module>$]<type>$<method>`.
    /// This ensures different types can define methods with the same name without
    /// LLVM symbol collision (e.g., `Point.distance` → `_ori_Point$distance`).
    ///
    /// Methods are inserted into both:
    /// - `functions` (bare `method.name` key, for backward compat)
    /// - `method_functions` (`(type_name, method_name)` key, for type-qualified dispatch)
    ///
    /// `type_idx_to_name` is also populated to map `sig.param_types[0]` (the self
    /// parameter type) to the type name, enabling receiver type → type name resolution
    /// during method call lowering.
    pub fn compile_impls(
        &mut self,
        impls: &[ori_ir::ImplDef],
        impl_sigs: &[(Name, FunctionSig)],
        arena: &ExprArena,
        expr_types: &[Idx],
    ) {
        // Consume impl_sigs positionally — the type checker pushes sigs in the
        // same iteration order: `for impl_def { for method { register_impl_sig } }`.
        // A flat HashMap keyed by method Name would lose entries when two types
        // define same-name methods (e.g., Point.distance vs Line.distance).
        let mut sig_iter = impl_sigs.iter();

        for impl_def in impls {
            // Resolve the type name from self_path for mangling
            let type_name = if let Some(first) = impl_def.self_path.first() {
                self.interner.lookup(*first).to_owned()
            } else {
                String::new()
            };

            for method in &impl_def.methods {
                let Some((sig_name, sig)) = sig_iter.next() else {
                    trace!(
                        name = %self.interner.lookup(method.name),
                        "no type signature for impl method — exhausted sig iterator"
                    );
                    continue;
                };

                debug_assert_eq!(
                    *sig_name, method.name,
                    "impl sig/method name mismatch: sig has {:?}, method has {:?}",
                    sig_name, method.name
                );

                if sig.is_generic() {
                    continue;
                }

                // Use type-qualified mangled name for LLVM symbol
                let method_str = self.interner.lookup(method.name);
                let symbol = if type_name.is_empty() {
                    self.mangler.mangle_function(self.module_path, method_str)
                } else {
                    self.mangler
                        .mangle_method(self.module_path, &type_name, method_str)
                };
                self.declare_function_with_symbol(method.name, &symbol, sig);

                let Some(&(func_id, ref abi)) = self.functions.get(&method.name) else {
                    continue;
                };
                let abi = abi.clone();

                // Populate type-qualified method map for dispatch
                if let Some(&type_name_name) = impl_def.self_path.first() {
                    self.method_functions
                        .insert((type_name_name, method.name), (func_id, abi.clone()));

                    // Map the self type Idx → type Name for receiver resolution
                    if let Some(&self_type_idx) = sig.param_types.first() {
                        self.type_idx_to_name.insert(self_type_idx, type_name_name);
                    }
                }

                self.define_function_body(
                    method.name,
                    func_id,
                    &abi,
                    method.body,
                    arena,
                    expr_types,
                );
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
    // AOT Entry Points
    // -----------------------------------------------------------------------

    /// Generate a C-compatible `main()` wrapper that calls the Ori `@main` function.
    ///
    /// The wrapper bridges the C calling convention (`ccc`) to Ori's internal
    /// calling convention (`fastcc`). Four `@main` signatures are supported:
    ///
    /// | Ori signature               | C wrapper                                    |
    /// |-----------------------------|----------------------------------------------|
    /// | `@main () -> void`          | `define i32 @main() { call @_ori_main(); ret 0 }` |
    /// | `@main () -> int`           | `define i32 @main() { trunc call @_ori_main() }` |
    /// | `@main (args) -> void`      | `define i32 @main(i32, ptr) { ... }`         |
    /// | `@main (args) -> int`       | `define i32 @main(i32, ptr) { ... }`         |
    ///
    /// Must be called after `declare_all()` + `define_all()` so the `@main`
    /// function is already compiled. Returns `false` if no `@main` was found.
    pub fn generate_main_wrapper(&mut self, main_name: Name, main_sig: &FunctionSig) -> bool {
        let Some(&(ori_main_id, ref abi)) = self.functions.get(&main_name) else {
            debug!("no @main function declared — skipping entry point wrapper");
            return false;
        };
        let abi = abi.clone();

        let has_args = !main_sig.param_types.is_empty();
        let returns_int = main_sig.return_type == Idx::INT;

        debug!(
            has_args,
            returns_int, "generating C main() entry point wrapper"
        );

        // C main signature: i32 @main() or i32 @main(i32 %argc, ptr %argv)
        let i32_ty = self.builder.i32_type();
        let c_main_params = if has_args {
            let ptr_ty = self.builder.ptr_type();
            vec![i32_ty, ptr_ty]
        } else {
            vec![]
        };

        let c_main_id = self
            .builder
            .declare_function("main", &c_main_params, i32_ty);
        self.builder.set_ccc(c_main_id);

        let entry = self.builder.append_block(c_main_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(c_main_id);

        // Build args for calling the Ori @main function
        let call_args = if has_args {
            // Call ori_args_from_argv(arg_count, arg_values) → Ori [str]
            let arg_count = self.builder.get_param(c_main_id, 0);
            let arg_values = self.builder.get_param(c_main_id, 1);

            let ptr_ty = self.builder.ptr_type();
            let str_llvm_ty = self.type_resolver.resolve(Idx::STR);
            let str_ty_id = self.builder.register_type(str_llvm_ty);
            let args_fn = self.builder.get_or_declare_function(
                "ori_args_from_argv",
                &[i32_ty, ptr_ty],
                str_ty_id,
            );
            let args_val = self.builder.call(args_fn, &[arg_count, arg_values], "args");
            if let Some(val) = args_val {
                vec![val]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        // Call the Ori @main function
        match &abi.return_abi.passing {
            super::abi::ReturnPassing::Direct => {
                let result = self
                    .builder
                    .call(ori_main_id, &call_args, "ori_main_result");
                if returns_int {
                    // Truncate i64 → i32 for C exit code
                    if let Some(val) = result {
                        let exit_code = self.builder.trunc(val, i32_ty, "exit_code");
                        self.builder.ret(exit_code);
                    } else {
                        let zero = self.builder.const_i32(0);
                        self.builder.ret(zero);
                    }
                } else {
                    let zero = self.builder.const_i32(0);
                    self.builder.ret(zero);
                }
            }
            super::abi::ReturnPassing::Void | super::abi::ReturnPassing::Sret { .. } => {
                self.builder.call(ori_main_id, &call_args, "");
                let zero = self.builder.const_i32(0);
                self.builder.ret(zero);
            }
        }

        true
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

    /// Borrow the type-qualified method map.
    pub fn method_function_map(&self) -> &FxHashMap<(Name, Name), (FunctionId, FunctionAbi)> {
        &self.method_functions
    }

    /// Borrow the type index → type name mapping.
    pub fn type_idx_to_name_map(&self) -> &FxHashMap<Idx, Name> {
        &self.type_idx_to_name
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

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool, "");
        fc.declare_function(func_name, &sig);

        let (_func_id, abi) = fc.get_function(func_name).unwrap();
        assert_eq!(abi.params.len(), 2);
        assert_eq!(abi.return_abi.passing, ReturnPassing::Direct);
        assert_eq!(abi.call_conv, CallConv::Fast);

        // Function is declared with mangled name _ori_add
        assert!(scx.llmod.get_function("_ori_add").is_some());
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

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool, "");
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

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool, "");
        fc.declare_function(func_name, &sig);

        let (_, abi) = fc.get_function(func_name).unwrap();
        assert!(matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }));

        // Must drop borrowers of scx before accessing scx directly
        drop(fc);
        drop(builder);
        drop(resolver);

        // Function is declared with mangled name _ori_get_list
        let llvm_fn = scx.llmod.get_function("_ori_get_list").unwrap();
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

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool, "");
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

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool, "");
        fc.declare_all(&[func], &[sig]);

        assert!(fc.get_function(func_name).is_none());

        // Must drop borrowers of scx before accessing scx directly
        drop(fc);
        drop(builder);
        drop(resolver);
        // Generic functions are not declared at all (neither mangled nor unmangled)
        assert!(scx.llmod.get_function("identity").is_none());
        assert!(scx.llmod.get_function("_ori_identity").is_none());
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

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool, "");
        fc.declare_function(add_name, &sig_add);
        fc.declare_function(sub_name, &sig_sub);

        assert_eq!(fc.function_map().len(), 2);
        assert!(fc.function_map().contains_key(&add_name));
        assert!(fc.function_map().contains_key(&sub_name));
    }

    #[test]
    fn compile_impls_populates_method_functions_map() {
        use ori_ir::{GenericParamRange, ImplDef, ImplMethod, ParsedType, ParsedTypeRange, Span};

        let interner = StringInterner::new();
        let point_name = interner.intern("Point");
        let line_name = interner.intern("Line");

        let mut pool = Pool::new();
        // Create named type Idx values for receiver types
        let point_idx = pool.named(point_name);
        let line_idx = pool.named(line_name);

        let ctx = Context::create();
        let store = TypeInfoStore::new(&pool);
        let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_method_dispatch"));
        let resolver = TypeLayoutResolver::new(&store, &scx);
        let mut builder = IrBuilder::new(&scx);

        let distance_name = interner.intern("distance");
        let self_name = interner.intern("self");

        // Create two impl blocks with same-name method "distance"
        let impl_point = ImplDef {
            generics: GenericParamRange::EMPTY,
            trait_path: None,
            trait_type_args: ParsedTypeRange::EMPTY,
            self_path: vec![point_name],
            self_ty: ParsedType::Named {
                name: point_name,
                type_args: ParsedTypeRange::EMPTY,
            },
            where_clauses: vec![],
            methods: vec![ImplMethod {
                name: distance_name,
                params: ori_ir::ParamRange::EMPTY,
                return_ty: ParsedType::Primitive(ori_ir::TypeId::FLOAT),
                body: ExprId::INVALID,
                span: Span::new(0, 0),
            }],
            assoc_types: vec![],
            span: Span::new(0, 0),
        };

        let impl_line = ImplDef {
            generics: GenericParamRange::EMPTY,
            trait_path: None,
            trait_type_args: ParsedTypeRange::EMPTY,
            self_path: vec![line_name],
            self_ty: ParsedType::Named {
                name: line_name,
                type_args: ParsedTypeRange::EMPTY,
            },
            where_clauses: vec![],
            methods: vec![ImplMethod {
                name: distance_name,
                params: ori_ir::ParamRange::EMPTY,
                return_ty: ParsedType::Primitive(ori_ir::TypeId::FLOAT),
                body: ExprId::INVALID,
                span: Span::new(0, 0),
            }],
            assoc_types: vec![],
            span: Span::new(0, 0),
        };

        // Signatures: distance(self: Point) -> float, distance(self: Line) -> float
        let sig_point = make_sig(
            distance_name,
            vec![self_name],
            vec![point_idx],
            Idx::FLOAT,
            false,
        );
        let sig_line = make_sig(
            distance_name,
            vec![self_name],
            vec![line_idx],
            Idx::FLOAT,
            false,
        );

        let impl_sigs = vec![
            (distance_name, sig_point.clone()),
            (distance_name, sig_line.clone()),
        ];

        let arena = ori_ir::ExprArena::new();
        let expr_types: Vec<Idx> = vec![];

        let mut fc = FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool, "");

        // Compile Point impl first, then Line impl
        // Note: compile_impls processes all impls; same method name → last one
        // overwrites in bare functions map, but BOTH should be in method_functions
        fc.compile_impls(&[impl_point, impl_line], &impl_sigs, &arena, &expr_types);

        // The bare functions map has only the LAST one (Line.distance overwrites Point.distance)
        assert!(fc.function_map().contains_key(&distance_name));

        // The type-qualified method map has BOTH
        assert!(
            fc.method_function_map()
                .contains_key(&(point_name, distance_name)),
            "method_functions should contain (Point, distance)"
        );
        assert!(
            fc.method_function_map()
                .contains_key(&(line_name, distance_name)),
            "method_functions should contain (Line, distance)"
        );

        // The type Idx → Name map should have both types
        assert_eq!(
            fc.type_idx_to_name_map().get(&point_idx),
            Some(&point_name),
            "type_idx_to_name should map Point Idx → Point Name"
        );
        assert_eq!(
            fc.type_idx_to_name_map().get(&line_idx),
            Some(&line_name),
            "type_idx_to_name should map Line Idx → Line Name"
        );

        // The two entries in method_functions should have DIFFERENT FunctionIds
        // (because they are different LLVM functions with different mangled names)
        let (point_func_id, _) = fc
            .method_function_map()
            .get(&(point_name, distance_name))
            .unwrap();
        let (line_func_id, _) = fc
            .method_function_map()
            .get(&(line_name, distance_name))
            .unwrap();
        assert_ne!(
            point_func_id, line_func_id,
            "Point.distance and Line.distance should have different FunctionIds"
        );

        // Must drop borrowers before accessing scx
        drop(fc);
        drop(builder);
        drop(resolver);

        // Verify mangled LLVM symbols exist
        assert!(
            scx.llmod.get_function("_ori_Point$distance").is_some(),
            "LLVM module should have _ori_Point$distance"
        );
        assert!(
            scx.llmod.get_function("_ori_Line$distance").is_some(),
            "LLVM module should have _ori_Line$distance"
        );
    }

    #[test]
    fn module_path_appears_in_mangled_name() {
        let pool = Pool::new();
        let ctx = Context::create();
        let interner = StringInterner::new();
        let store = TypeInfoStore::new(&pool);
        let scx = ManuallyDrop::new(SimpleCx::new(&ctx, "test_module_mangle"));
        let resolver = TypeLayoutResolver::new(&store, &scx);
        let mut builder = IrBuilder::new(&scx);

        let func_name = interner.intern("add");
        let a_name = interner.intern("a");
        let sig = make_sig(func_name, vec![a_name], vec![Idx::INT], Idx::INT, false);

        // Use "math" as module path
        let mut fc =
            FunctionCompiler::new(&mut builder, &store, &resolver, &interner, &pool, "math");
        fc.declare_function(func_name, &sig);

        // Must drop borrowers before accessing scx directly
        drop(fc);
        drop(builder);
        drop(resolver);

        // Mangled as _ori_math$add
        assert!(scx.llmod.get_function("_ori_math$add").is_some());
        // Unmangled name should NOT exist
        assert!(scx.llmod.get_function("add").is_none());
    }
}
