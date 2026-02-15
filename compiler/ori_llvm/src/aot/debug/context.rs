//! Combined debug info context for a compilation unit.

use std::path::Path;

use inkwell::basic_block::BasicBlock;
use inkwell::context::Context;
use inkwell::debug_info::{
    AsDIScope, DILexicalBlock, DIScope, DISubprogram, DISubroutineType, DIType,
};
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, InstructionValue, PointerValue};
use ori_types::{Idx, Pool};

use super::builder::DebugInfoBuilder;
use super::config::{DebugInfoConfig, DebugLevel};
use super::line_map::LineMap;

/// Combined debug info context for a compilation unit.
///
/// This combines the `DebugInfoBuilder` with a `LineMap` to provide
/// convenient span-based location setting.
pub struct DebugContext<'ctx> {
    /// The debug info builder.
    pub builder: DebugInfoBuilder<'ctx>,
    /// Line map for span-to-location conversion.
    pub line_map: LineMap,
}

impl<'ctx> DebugContext<'ctx> {
    /// Create a new debug context.
    ///
    /// # Arguments
    ///
    /// * `module` - The LLVM module
    /// * `context` - The LLVM context
    /// * `config` - Debug info configuration
    /// * `source_path` - Path to the source file
    /// * `source_text` - The source text (for line map building)
    ///
    /// # Returns
    ///
    /// Returns `None` if debug info is disabled.
    #[must_use]
    pub fn new(
        module: &Module<'ctx>,
        context: &'ctx Context,
        config: DebugInfoConfig,
        source_path: &Path,
        source_text: &str,
    ) -> Option<Self> {
        let builder = DebugInfoBuilder::from_path(module, context, config, source_path)?;
        let line_map = LineMap::new(source_text);
        Some(Self { builder, line_map })
    }

    /// Set debug location from a span's start offset.
    ///
    /// # Arguments
    ///
    /// * `ir_builder` - The LLVM IR builder
    /// * `span_start` - The byte offset of the span start
    /// * `scope` - The debug scope for this location
    pub fn set_location_from_offset(
        &self,
        ir_builder: &inkwell::builder::Builder<'ctx>,
        span_start: u32,
        scope: DIScope<'ctx>,
    ) {
        let (line, col) = self.line_map.offset_to_line_col(span_start);
        self.builder.set_location(ir_builder, line, col, scope);
    }

    /// Set debug location from a span using the current scope.
    pub fn set_location_from_offset_in_current_scope(
        &self,
        ir_builder: &inkwell::builder::Builder<'ctx>,
        span_start: u32,
    ) {
        let (line, col) = self.line_map.offset_to_line_col(span_start);
        self.builder
            .set_location_in_current_scope(ir_builder, line, col);
    }

    /// Get the line and column for a byte offset.
    #[must_use]
    pub fn offset_to_line_col(&self, offset: u32) -> (u32, u32) {
        self.line_map.offset_to_line_col(offset)
    }

    /// Create debug info for a function at a given span offset.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the void type for the return type.
    pub fn create_function_at_offset(
        &self,
        name: &str,
        span_start: u32,
    ) -> Result<DISubprogram<'ctx>, super::config::DebugInfoError> {
        let (line, _col) = self.line_map.offset_to_line_col(span_start);
        self.builder.create_simple_function(name, line)
    }

    /// Create debug info for a function with full signature.
    pub fn create_function_with_type(
        &self,
        name: &str,
        linkage_name: Option<&str>,
        span_start: u32,
        subroutine_type: DISubroutineType<'ctx>,
        is_local: bool,
    ) -> DISubprogram<'ctx> {
        let (line, _col) = self.line_map.offset_to_line_col(span_start);
        self.builder
            .create_function(name, linkage_name, line, subroutine_type, is_local, true)
    }

    /// Create a lexical block at a given span offset.
    pub fn create_lexical_block_at_offset(
        &self,
        scope: DIScope<'ctx>,
        span_start: u32,
    ) -> DILexicalBlock<'ctx> {
        let (line, col) = self.line_map.offset_to_line_col(span_start);
        self.builder.create_lexical_block(scope, line, col)
    }

    /// Push a function scope for the given subprogram.
    pub fn enter_function(&self, subprogram: DISubprogram<'ctx>) {
        self.builder.push_scope(subprogram.as_debug_info_scope());
    }

    /// Pop the current function scope.
    pub fn exit_function(&self) {
        self.builder.pop_scope();
    }

    /// Get the debug info builder.
    #[must_use]
    pub fn di(&self) -> &DebugInfoBuilder<'ctx> {
        &self.builder
    }

    /// Get the debug level.
    #[must_use]
    pub fn level(&self) -> DebugLevel {
        self.builder.level()
    }

    // -- Variable Debug Info Convenience --

    /// Emit `llvm.dbg.declare` for a mutable binding (alloca).
    ///
    /// Creates the auto variable and declare intrinsic in one call.
    /// Uses the current scope and creates a debug location from `span_start`.
    pub fn emit_declare_for_alloca(
        &self,
        alloca: PointerValue<'ctx>,
        name: &str,
        ty: DIType<'ctx>,
        span_start: u32,
        block: BasicBlock<'ctx>,
    ) {
        let (line, col) = self.line_map.offset_to_line_col(span_start);
        let scope = self.builder.current_scope();
        let var = self.builder.create_auto_variable(scope, name, line, ty);
        let loc = self.builder.create_debug_location(line, col, scope);
        self.builder.emit_dbg_declare(alloca, var, loc, block);
    }

    /// Emit `llvm.dbg.value` for an immutable binding (SSA value).
    ///
    /// Creates the auto variable and value intrinsic in one call.
    /// Uses the current scope and creates a debug location from `span_start`.
    ///
    /// `insert_before` is the instruction before which the dbg.value is placed.
    pub fn emit_value_for_binding(
        &self,
        value: BasicValueEnum<'ctx>,
        name: &str,
        ty: DIType<'ctx>,
        span_start: u32,
        insert_before: InstructionValue<'ctx>,
    ) {
        let (line, col) = self.line_map.offset_to_line_col(span_start);
        let scope = self.builder.current_scope();
        let var = self.builder.create_auto_variable(scope, name, line, ty);
        let loc = self.builder.create_debug_location(line, col, scope);
        self.builder.emit_dbg_value(value, var, loc, insert_before);
    }

    /// Emit `llvm.dbg.value` for an immutable binding at the end of a block.
    ///
    /// Like [`emit_value_for_binding`], but appends to the block instead
    /// of requiring an `insert_before` instruction. Useful when emitting
    /// debug info at binding time before any subsequent instructions exist.
    pub fn emit_value_for_binding_at_end(
        &self,
        value: BasicValueEnum<'ctx>,
        name: &str,
        ty: DIType<'ctx>,
        span_start: u32,
        block: BasicBlock<'ctx>,
    ) {
        let (line, col) = self.line_map.offset_to_line_col(span_start);
        let scope = self.builder.current_scope();
        let var = self.builder.create_auto_variable(scope, name, line, ty);
        let loc = self.builder.create_debug_location(line, col, scope);
        self.builder.emit_dbg_value_at_end(value, var, loc, block);
    }

    /// Emit parameter debug info (`DW_TAG_formal_parameter`).
    ///
    /// Creates a `DILocalVariable` with parameter semantics and emits
    /// `llvm.dbg.value` at the end of the given block.
    ///
    /// `arg_no` is 1-based (DWARF convention: first parameter is arg 1).
    pub fn emit_param_debug_info(
        &self,
        value: BasicValueEnum<'ctx>,
        name: &str,
        arg_no: u32,
        ty: DIType<'ctx>,
        block: BasicBlock<'ctx>,
    ) {
        let scope = self.builder.current_scope();
        let loc = self.builder.create_debug_location(0, 0, scope);
        let var = self
            .builder
            .create_parameter_variable(scope, name, arg_no, 0, ty);
        self.builder.emit_dbg_value_at_end(value, var, loc, block);
    }

    /// Resolve an Ori type (`Idx`) to its DWARF debug type (`DIType`).
    ///
    /// Dispatches on the Pool's tag for the given type index, calling
    /// the appropriate `DebugInfoBuilder` type method. Falls back to
    /// `int_type()` with a warning for unmapped types.
    ///
    /// This bridges the gap between the type pool (used everywhere in
    /// codegen) and the debug info builder (which has per-type methods).
    pub fn resolve_debug_type(&self, idx: Idx, pool: &Pool) -> Option<DIType<'ctx>> {
        use ori_types::Tag;

        if idx == Idx::NONE {
            tracing::warn!("resolve_debug_type called with Idx::NONE, using int fallback");
            return self.builder.int_type().ok().map(|t| t.as_type());
        }

        let tag = pool.tag(idx);
        match tag {
            Tag::Int => self.builder.int_type().ok().map(|t| t.as_type()),
            Tag::Float => self.builder.float_type().ok().map(|t| t.as_type()),
            Tag::Bool => self.builder.bool_type().ok().map(|t| t.as_type()),
            Tag::Char => self.builder.char_type().ok().map(|t| t.as_type()),
            Tag::Byte => self.builder.byte_type().ok().map(|t| t.as_type()),
            Tag::Unit | Tag::Never => self.builder.void_type().ok().map(|t| t.as_type()),
            Tag::Str => self.builder.string_type().ok().map(|t| t.as_type()),
            Tag::Duration | Tag::Size => {
                // Duration and Size are represented as i64 (nanoseconds / bytes)
                self.builder.int_type().ok().map(|t| t.as_type())
            }
            Tag::Ordering => self.builder.byte_type().ok().map(|t| t.as_type()),
            Tag::List => {
                // List element type — use int as placeholder for element debug type
                let elem_di = self.builder.int_type().unwrap().as_type();
                self.builder.list_type(elem_di).ok().map(|t| t.as_type())
            }
            Tag::Option => {
                // Option payload — use int as placeholder for payload debug type
                let payload_di = self.builder.int_type().ok().map(|t| t.as_type());
                if let Some(pdi) = payload_di {
                    self.builder.option_type(pdi, 64).ok().map(|t| t.as_type())
                } else {
                    self.builder.int_type().ok().map(|t| t.as_type())
                }
            }
            Tag::Range => {
                // Range is {i64, i64, i1} — show as int for now
                self.builder.int_type().ok().map(|t| t.as_type())
            }
            Tag::Function => {
                // Function pointer — show as ptr
                let void_di = self.builder.void_type().ok().map(|t| t.as_type());
                if let Some(vdi) = void_di {
                    Some(self.builder.create_pointer_type("fn_ptr", vdi, 64))
                } else {
                    self.builder.int_type().ok().map(|t| t.as_type())
                }
            }
            _ => {
                // Unmapped types: Map, Set, Result, Tuple, Struct, Enum, Channel,
                // Named, Applied, Alias, Var, etc.
                tracing::warn!(?tag, "unmapped type for debug info, falling back to int");
                self.builder.int_type().ok().map(|t| t.as_type())
            }
        }
    }

    /// Finalize debug info (must be called before emission).
    pub fn finalize(&self) {
        self.builder.finalize();
    }
}
