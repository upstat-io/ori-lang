//! Literal and identifier lowering for V2 codegen.
//!
//! Handles the simplest expression kinds: constants, variables, and
//! function references. These produce values without control flow or
//! type-dependent dispatch.

use ori_ir::{DurationUnit, Name, SizeUnit, TemplatePartRange};

use super::expr_lowerer::ExprLowerer;
use super::scope::ScopeBinding;
use super::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Numeric / primitive literals
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Int(n)` → i64 constant.
    pub(crate) fn lower_int(&mut self, n: i64) -> ValueId {
        self.builder.const_i64(n)
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

    /// Lower `ExprKind::Ident(name)` — variable lookup.
    ///
    /// Immutable bindings return the SSA value directly (no memory traffic).
    /// Mutable bindings load from the alloca pointer.
    pub(crate) fn lower_ident(&mut self, name: Name) -> Option<ValueId> {
        match self.scope.lookup(name) {
            Some(ScopeBinding::Immutable(val)) => Some(val),
            Some(ScopeBinding::Mutable { ptr, ty }) => {
                let name_str = self.resolve_name(name).to_owned();
                let val = self.builder.load(ty, ptr, &name_str);
                Some(val)
            }
            None => {
                // May be a function reference or unit variant — check LLVM module
                let name_str = self.resolve_name(name).to_owned();
                if let Some(func) = self.builder.scx().llmod.get_function(&name_str) {
                    let _func_id = self.builder.intern_function(func);
                    // Return function pointer as a value
                    let ptr_val = func.as_global_value().as_pointer_value();
                    Some(self.builder.intern_value(ptr_val.into()))
                } else {
                    tracing::warn!(name = %name_str, "unresolved identifier in codegen");
                    None
                }
            }
        }
    }

    /// Lower `ExprKind::Const(name)` — compile-time constant reference.
    ///
    /// Constants are bound in the scope like immutable variables.
    /// Falls back to identifier lookup.
    pub(crate) fn lower_const(&mut self, name: Name) -> Option<ValueId> {
        self.lower_ident(name)
    }

    /// Lower `ExprKind::FunctionRef(name)` — `@name` function reference.
    ///
    /// Looks up the function in the LLVM module and returns its pointer.
    pub(crate) fn lower_function_ref(&mut self, name: Name) -> Option<ValueId> {
        let name_str = self.resolve_name(name);
        if let Some(func) = self.builder.scx().llmod.get_function(name_str) {
            let ptr_val = func.as_global_value().as_pointer_value();
            Some(self.builder.intern_value(ptr_val.into()))
        } else {
            tracing::warn!(name = name_str, "unresolved function reference");
            None
        }
    }

    /// Lower `ExprKind::HashLength` — `#` (collection length in index context).
    ///
    /// Not yet implemented; requires context about which collection is
    /// being indexed.
    #[allow(clippy::unused_self)] // Will use self when hash length is implemented
    pub(crate) fn lower_hash_length(&mut self) -> Option<ValueId> {
        tracing::warn!("hash length (#) not yet implemented in V2 codegen");
        None
    }

    // -----------------------------------------------------------------------
    // Template literals
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::TemplateLiteral { head, parts }` — string interpolation.
    ///
    /// Requires runtime string concatenation and `str_from_*` conversions.
    /// Currently a stub that returns the head string.
    pub(crate) fn lower_template_literal(
        &mut self,
        head: Name,
        _parts: TemplatePartRange,
    ) -> Option<ValueId> {
        tracing::warn!("template literal interpolation not yet implemented in V2 codegen");
        // Return the head as a plain string for now
        self.lower_string(head)
    }
}
