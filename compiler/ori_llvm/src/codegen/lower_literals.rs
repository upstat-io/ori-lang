//! Literal and identifier lowering for V2 codegen.
//!
//! Handles the simplest expression kinds: constants, variables, and
//! function references. These produce values without control flow or
//! type-dependent dispatch.

use ori_ir::canon::{CanId, ConstantId};
use ori_ir::{DurationUnit, Name, SizeUnit};
use ori_types::Idx;

use super::expr_lowerer::ExprLowerer;
use super::scope::ScopeBinding;
use super::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Numeric / primitive literals
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Int(n)` → i64 constant (or i8 for byte-typed expressions).
    pub(crate) fn lower_int(&mut self, n: i64) -> ValueId {
        self.builder.const_i64(n)
    }

    /// Lower an integer literal with type awareness.
    ///
    /// Integer literals can represent both `int` (i64) and `byte` (i8) depending
    /// on their resolved type. Using the wrong width causes store/load mismatches
    /// that corrupt stack memory.
    pub(crate) fn lower_int_typed(&mut self, n: i64, id: CanId) -> ValueId {
        let ty = self.expr_type(id);
        if ty == Idx::BYTE {
            self.builder.const_i8(n as i8)
        } else {
            self.builder.const_i64(n)
        }
    }

    /// Lower `ExprKind::Float(bits)` → f64 constant.
    ///
    /// Floats are stored as `u64` bits in the AST for `Hash` compatibility.
    pub(crate) fn lower_float(&mut self, bits: u64) -> ValueId {
        self.builder.const_f64(f64::from_bits(bits))
    }

    /// Lower `ExprKind::Bool(b)` → i1 constant.
    pub(crate) fn lower_bool(&mut self, b: bool) -> ValueId {
        self.builder.const_bool(b)
    }

    /// Lower `ExprKind::Char(c)` → i32 constant (Unicode scalar value).
    pub(crate) fn lower_char(&mut self, c: char) -> ValueId {
        self.builder.const_i32(c as i32)
    }

    /// Lower `ExprKind::Unit` → i64(0).
    ///
    /// LLVM void cannot be stored, passed, or phi'd, so Ori represents
    /// unit as `i64(0)` — a valid SSA value that can participate in merges.
    pub(crate) fn lower_unit(&mut self) -> ValueId {
        self.builder.const_i64(0)
    }

    // -----------------------------------------------------------------------
    // Duration / Size literals
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Duration { value, unit }` → i64 (nanoseconds).
    ///
    /// Duration is decimal sugar only — the unit multiplier converts at
    /// compile time (e.g., `5s` → `5_000_000_000ns`).
    pub(crate) fn lower_duration(&mut self, value: u64, unit: DurationUnit) -> ValueId {
        let nanos = unit.to_nanos(value);
        self.builder.const_i64(nanos)
    }

    /// Lower `ExprKind::Size { value, unit }` → i64 (bytes).
    ///
    /// Size is decimal sugar only (SI powers of 1000) — the unit multiplier
    /// converts at compile time (e.g., `4kb` → `4000`).
    pub(crate) fn lower_size(&mut self, value: u64, unit: SizeUnit) -> ValueId {
        let bytes = unit.to_bytes(value);
        self.builder.const_i64(bytes as i64)
    }

    // -----------------------------------------------------------------------
    // String literals
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::String(name)` or `ExprKind::TemplateFull(name)`.
    ///
    /// Creates an Ori string value `{i64 len, ptr data}` from a global
    /// string constant. The string data is null-terminated for C interop
    /// but the length field does not include the null terminator.
    pub(crate) fn lower_string(&mut self, name: Name) -> Option<ValueId> {
        let s = self.resolve_name(name).to_owned();
        let len = s.len();
        let len_val = self.builder.const_i64(len as i64);
        let ptr_val = self.builder.build_global_string_ptr(&s, "str.data");

        // Build {i64 len, ptr data} struct
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let result = self
            .builder
            .build_struct(str_ty, &[len_val, ptr_val], "str");
        Some(result)
    }

    // -----------------------------------------------------------------------
    // Identifiers and references
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::Ident(name)` — variable lookup.
    ///
    /// Resolution order:
    /// 1. Scope (local/immutable/mutable bindings)
    /// 2. Declared functions map (V2 mangled names — user-defined functions)
    /// 3. LLVM module lookup (runtime functions with unmangled `ori_*` names)
    pub(crate) fn lower_ident(&mut self, name: Name, expr_id: CanId) -> Option<ValueId> {
        match self.scope.lookup(name) {
            Some(ScopeBinding::Immutable(val)) => Some(val),
            Some(ScopeBinding::Mutable { ptr, ty }) => {
                let name_str = self.resolve_name(name).to_owned();
                let val = self.builder.load(ty, ptr, &name_str);
                Some(val)
            }
            None => {
                // Check declared functions map first (V2 mangled names)
                if let Some(&(func_id, _)) = self.functions.get(&name) {
                    return self.wrap_function_as_value(func_id, expr_id);
                }

                // Fall back to LLVM module lookup (runtime functions, etc.)
                let name_str = self.resolve_name(name).to_owned();
                if let Some(func) = self.builder.scx().llmod.get_function(&name_str) {
                    let _func_id = self.builder.intern_function(func);
                    let ptr_val = func.as_global_value().as_pointer_value();
                    let fn_ptr_id = self.builder.intern_value(ptr_val.into());

                    // Check if this identifier has function type — if so, wrap
                    // in a fat-pointer closure { fn_ptr, null_env }
                    let ident_type = self.expr_type(expr_id);
                    let type_info = self.type_info.get(ident_type);
                    if matches!(type_info, super::type_info::TypeInfo::Function { .. }) {
                        let null_env = self.builder.const_null_ptr();
                        let closure_ty = self.builder.closure_type();
                        let fat_ptr =
                            self.builder
                                .build_struct(closure_ty, &[fn_ptr_id, null_env], "fn_ref");
                        Some(fat_ptr)
                    } else {
                        Some(fn_ptr_id)
                    }
                } else {
                    tracing::warn!(name = %name_str, "unresolved identifier in codegen");
                    None
                }
            }
        }
    }

    /// Wrap a declared function as a value, applying fat-pointer closure
    /// wrapping if the expression has function type.
    fn wrap_function_as_value(
        &mut self,
        func_id: super::value_id::FunctionId,
        expr_id: CanId,
    ) -> Option<ValueId> {
        let fn_val = self.builder.get_function_value(func_id);
        let ptr_val = fn_val.as_global_value().as_pointer_value();
        let fn_ptr_id = self.builder.intern_value(ptr_val.into());

        let ident_type = self.expr_type(expr_id);
        let type_info = self.type_info.get(ident_type);
        if matches!(type_info, super::type_info::TypeInfo::Function { .. }) {
            let null_env = self.builder.const_null_ptr();
            let closure_ty = self.builder.closure_type();
            let fat_ptr = self
                .builder
                .build_struct(closure_ty, &[fn_ptr_id, null_env], "fn_ref");
            Some(fat_ptr)
        } else {
            Some(fn_ptr_id)
        }
    }

    /// Lower `CanExpr::Const(name)` — compile-time constant reference.
    ///
    /// Constants are bound in the scope like immutable variables.
    /// Falls back to identifier lookup.
    pub(crate) fn lower_const(&mut self, name: Name, expr_id: CanId) -> Option<ValueId> {
        self.lower_ident(name, expr_id)
    }

    /// Lower `ExprKind::FunctionRef(name)` — `@name` function reference.
    ///
    /// Looks up the function in the declared functions map first (mangled names),
    /// then falls back to LLVM module lookup for runtime functions.
    /// Wraps the result in a fat-pointer closure `{ fn_ptr, null }`.
    pub(crate) fn lower_function_ref(&mut self, name: Name) -> Option<ValueId> {
        // Check declared functions map first (V2 mangled names)
        if let Some(&(func_id, _)) = self.functions.get(&name) {
            let fn_val = self.builder.get_function_value(func_id);
            let ptr_val = fn_val.as_global_value().as_pointer_value();
            let fn_ptr_id = self.builder.intern_value(ptr_val.into());
            let null_env = self.builder.const_null_ptr();
            let closure_ty = self.builder.closure_type();
            let fat_ptr = self
                .builder
                .build_struct(closure_ty, &[fn_ptr_id, null_env], "fn_ref");
            return Some(fat_ptr);
        }

        // Fall back to LLVM module lookup (runtime functions)
        let name_str = self.resolve_name(name);
        if let Some(func) = self.builder.scx().llmod.get_function(name_str) {
            let ptr_val = func.as_global_value().as_pointer_value();
            let fn_ptr_id = self.builder.intern_value(ptr_val.into());
            let null_env = self.builder.const_null_ptr();
            let closure_ty = self.builder.closure_type();
            let fat_ptr = self
                .builder
                .build_struct(closure_ty, &[fn_ptr_id, null_env], "fn_ref");
            Some(fat_ptr)
        } else {
            tracing::warn!(name = name_str, "unresolved function reference");
            None
        }
    }

    // -----------------------------------------------------------------------
    // Compile-time constants
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::Constant(id)` — emit a compile-time constant value.
    ///
    /// Reads the pre-computed value from the `ConstantPool` and emits the
    /// appropriate LLVM constant.
    pub(crate) fn lower_constant(
        &mut self,
        const_id: ConstantId,
        expr_id: CanId,
    ) -> Option<ValueId> {
        use ori_ir::canon::ConstValue;
        let val = self.canon.constants.get(const_id);
        match val {
            ConstValue::Int(n) => Some(self.lower_int_typed(*n, expr_id)),
            ConstValue::Float(bits) => Some(self.lower_float(*bits)),
            ConstValue::Bool(b) => Some(self.lower_bool(*b)),
            ConstValue::Str(name) => self.lower_string(*name),
            ConstValue::Char(c) => Some(self.lower_char(*c)),
            ConstValue::Unit => Some(self.lower_unit()),
            ConstValue::Duration { value, .. } | ConstValue::Size { value, .. } => {
                // Duration and Size are stored as i64 at the LLVM level
                Some(self.lower_int(*value as i64))
            }
        }
    }
}
