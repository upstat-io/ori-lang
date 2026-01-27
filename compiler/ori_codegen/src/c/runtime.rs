//! C Runtime Type Definitions
//!
//! Defines the C runtime types and functions that Ori programs need:
//!
//! - **Unboxed Option/Result** for primitives (tagged unions, no heap)
//! - **SSO strings** for strings ≤23 bytes
//! - **ARC runtime** for heap-allocated values

use crate::context::CodegenContext;

/// C runtime code generator.
pub struct CRuntime;

impl CRuntime {
    /// Emit runtime type definitions.
    pub fn emit_types(ctx: &mut CodegenContext<'_>) {
        ctx.writeln(
            "// ============================================================================",
        );
        ctx.writeln("// Ori Runtime Types");
        ctx.writeln(
            "// ============================================================================",
        );
        ctx.newline();

        // ARC header for heap-allocated values
        Self::emit_arc_header(ctx);

        // Small String Optimization
        Self::emit_sso_string(ctx);

        // Unboxed Option types for primitives
        Self::emit_option_types(ctx);

        // Unboxed Result types for primitives
        Self::emit_result_types(ctx);

        // List type
        Self::emit_list_type(ctx);

        // Map type (simplified)
        Self::emit_map_type(ctx);
    }

    /// Emit runtime function declarations.
    pub fn emit_functions(ctx: &mut CodegenContext<'_>) {
        ctx.writeln(
            "// ============================================================================",
        );
        ctx.writeln("// Ori Runtime Functions");
        ctx.writeln(
            "// ============================================================================",
        );
        ctx.newline();

        // Runtime init/cleanup
        ctx.writeln("void ori_runtime_init(void);");
        ctx.writeln("void ori_runtime_cleanup(void);");
        ctx.newline();

        // ARC functions
        Self::emit_arc_functions(ctx);

        // String functions
        Self::emit_string_functions(ctx);

        // Option functions
        Self::emit_option_functions(ctx);

        // Result functions
        Self::emit_result_functions(ctx);

        // Panic function
        ctx.writeln("_Noreturn void ori_panic(const char* msg);");
        ctx.newline();

        // Print function
        ctx.writeln("void ori_print(ori_string_t s);");
        ctx.newline();
    }

    /// Emit ARC header struct.
    fn emit_arc_header(ctx: &mut CodegenContext<'_>) {
        ctx.writeln("// ARC header for heap-allocated values");
        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("uint32_t refcount;");
        ctx.writeln("uint32_t size;");
        ctx.dedent();
        ctx.writeln("} ori_arc_header_t;");
        ctx.newline();
    }

    /// Emit SSO string type.
    ///
    /// Strings ≤23 bytes are stored inline (no heap allocation).
    /// Strings >23 bytes use heap allocation with ARC.
    fn emit_sso_string(ctx: &mut CodegenContext<'_>) {
        ctx.writeln("// Small String Optimization (SSO)");
        ctx.writeln("// Strings <= 23 bytes stored inline, no heap allocation");
        ctx.writeln("#define ORI_SSO_CAP 23");
        ctx.newline();

        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("union {");
        ctx.indent();
        ctx.writeln("// Small string: stored inline");
        ctx.writeln("struct {");
        ctx.indent();
        ctx.writeln("char data[ORI_SSO_CAP];");
        ctx.writeln("uint8_t len; // If len <= ORI_SSO_CAP, this is a small string");
        ctx.dedent();
        ctx.writeln("} small;");
        ctx.newline();
        ctx.writeln("// Large string: heap-allocated with ARC");
        ctx.writeln("struct {");
        ctx.indent();
        ctx.writeln("char* data;");
        ctx.writeln("uint64_t len;");
        ctx.writeln("uint8_t _pad[7];");
        ctx.writeln("uint8_t is_large; // High bit set = large string");
        ctx.dedent();
        ctx.writeln("} large;");
        ctx.dedent();
        ctx.writeln("};");
        ctx.dedent();
        ctx.writeln("} ori_string_t;");
        ctx.newline();

        ctx.writeln("#define ORI_STRING_IS_LARGE(s) ((s).large.is_large & 0x80)");
        ctx.writeln(
            "#define ORI_STRING_LEN(s) (ORI_STRING_IS_LARGE(s) ? (s).large.len : (s).small.len)",
        );
        ctx.writeln(
            "#define ORI_STRING_DATA(s) (ORI_STRING_IS_LARGE(s) ? (s).large.data : (s).small.data)",
        );
        ctx.newline();
    }

    /// Emit unboxed Option types for primitives.
    fn emit_option_types(ctx: &mut CodegenContext<'_>) {
        ctx.writeln("// Unboxed Option types for primitives (no heap allocation)");
        ctx.newline();

        // Option<int>
        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("uint8_t tag; // 0 = None, 1 = Some");
        ctx.writeln("int64_t value;");
        ctx.dedent();
        ctx.writeln("} ori_option_int_t;");
        ctx.newline();

        // Option<float>
        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("uint8_t tag;");
        ctx.writeln("double value;");
        ctx.dedent();
        ctx.writeln("} ori_option_float_t;");
        ctx.newline();

        // Option<bool>
        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("uint8_t tag;");
        ctx.writeln("bool value;");
        ctx.dedent();
        ctx.writeln("} ori_option_bool_t;");
        ctx.newline();

        // Option<char>
        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("uint8_t tag;");
        ctx.writeln("uint32_t value; // Unicode codepoint");
        ctx.dedent();
        ctx.writeln("} ori_option_char_t;");
        ctx.newline();

        // Option<byte>
        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("uint8_t tag;");
        ctx.writeln("uint8_t value;");
        ctx.dedent();
        ctx.writeln("} ori_option_byte_t;");
        ctx.newline();

        // Convenience macros
        ctx.writeln("#define ORI_SOME_INT(v) ((ori_option_int_t){1, (v)})");
        ctx.writeln("#define ORI_SOME_FLOAT(v) ((ori_option_float_t){1, (v)})");
        ctx.writeln("#define ORI_SOME_BOOL(v) ((ori_option_bool_t){1, (v)})");
        ctx.writeln("#define ORI_SOME_CHAR(v) ((ori_option_char_t){1, (v)})");
        ctx.writeln("#define ORI_SOME_BYTE(v) ((ori_option_byte_t){1, (v)})");
        ctx.writeln("#define ORI_NONE_INT ((ori_option_int_t){0, 0})");
        ctx.writeln("#define ORI_NONE_FLOAT ((ori_option_float_t){0, 0.0})");
        ctx.writeln("#define ORI_NONE_BOOL ((ori_option_bool_t){0, false})");
        ctx.writeln("#define ORI_NONE_CHAR ((ori_option_char_t){0, 0})");
        ctx.writeln("#define ORI_NONE_BYTE ((ori_option_byte_t){0, 0})");
        ctx.newline();
    }

    /// Emit unboxed Result types for primitives.
    fn emit_result_types(ctx: &mut CodegenContext<'_>) {
        ctx.writeln("// Unboxed Result types for primitives");
        ctx.newline();

        // Result<int, str>
        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("uint8_t tag; // 0 = Ok, 1 = Err");
        ctx.writeln("union {");
        ctx.indent();
        ctx.writeln("int64_t ok;");
        ctx.writeln("ori_string_t err;");
        ctx.dedent();
        ctx.writeln("} value;");
        ctx.dedent();
        ctx.writeln("} ori_result_int_str_t;");
        ctx.newline();

        // Result<float, str>
        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("uint8_t tag;");
        ctx.writeln("union {");
        ctx.indent();
        ctx.writeln("double ok;");
        ctx.writeln("ori_string_t err;");
        ctx.dedent();
        ctx.writeln("} value;");
        ctx.dedent();
        ctx.writeln("} ori_result_float_str_t;");
        ctx.newline();

        // Result<bool, str>
        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("uint8_t tag;");
        ctx.writeln("union {");
        ctx.indent();
        ctx.writeln("bool ok;");
        ctx.writeln("ori_string_t err;");
        ctx.dedent();
        ctx.writeln("} value;");
        ctx.dedent();
        ctx.writeln("} ori_result_bool_str_t;");
        ctx.newline();

        // Result<void, str> (for operations that can fail but return nothing)
        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("uint8_t tag;");
        ctx.writeln("ori_string_t err; // Only used if tag == 1");
        ctx.dedent();
        ctx.writeln("} ori_result_void_str_t;");
        ctx.newline();

        // Convenience macros
        ctx.writeln("#define ORI_OK_INT(v) ((ori_result_int_str_t){0, {.ok = (v)}})");
        ctx.writeln("#define ORI_OK_FLOAT(v) ((ori_result_float_str_t){0, {.ok = (v)}})");
        ctx.writeln("#define ORI_OK_BOOL(v) ((ori_result_bool_str_t){0, {.ok = (v)}})");
        ctx.writeln("#define ORI_OK_VOID ((ori_result_void_str_t){0, {0}})");
        ctx.writeln("#define ORI_ERR_INT(e) ((ori_result_int_str_t){1, {.err = (e)}})");
        ctx.writeln("#define ORI_ERR_FLOAT(e) ((ori_result_float_str_t){1, {.err = (e)}})");
        ctx.writeln("#define ORI_ERR_BOOL(e) ((ori_result_bool_str_t){1, {.err = (e)}})");
        ctx.writeln("#define ORI_ERR_VOID(e) ((ori_result_void_str_t){1, (e)})");
        ctx.newline();
    }

    /// Emit list type (ARC-managed).
    fn emit_list_type(ctx: &mut CodegenContext<'_>) {
        ctx.writeln("// List type (ARC-managed dynamic array)");
        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("ori_arc_header_t* header;");
        ctx.writeln("void* data;");
        ctx.writeln("uint64_t len;");
        ctx.writeln("uint64_t cap;");
        ctx.writeln("uint32_t elem_size;");
        ctx.dedent();
        ctx.writeln("} ori_list_t;");
        ctx.newline();
    }

    /// Emit map type (simplified).
    fn emit_map_type(ctx: &mut CodegenContext<'_>) {
        ctx.writeln("// Map type (ARC-managed hash table)");
        ctx.writeln("typedef struct {");
        ctx.indent();
        ctx.writeln("ori_arc_header_t* header;");
        ctx.writeln("void* buckets;");
        ctx.writeln("uint64_t len;");
        ctx.writeln("uint64_t cap;");
        ctx.dedent();
        ctx.writeln("} ori_map_t;");
        ctx.newline();
    }

    /// Emit ARC function declarations.
    fn emit_arc_functions(ctx: &mut CodegenContext<'_>) {
        ctx.writeln("// ARC memory management");
        ctx.writeln("void* ori_arc_alloc(uint32_t size);");
        ctx.writeln("void* ori_arc_retain(void* ptr);");
        ctx.writeln("void ori_arc_release(void* ptr);");
        ctx.writeln("uint32_t ori_arc_refcount(void* ptr);");
        ctx.newline();
    }

    /// Emit string function declarations.
    fn emit_string_functions(ctx: &mut CodegenContext<'_>) {
        ctx.writeln("// String functions");
        ctx.writeln("ori_string_t ori_string_from_cstr(const char* s);");
        ctx.writeln("ori_string_t ori_string_from_len(const char* s, uint64_t len);");
        ctx.writeln("ori_string_t ori_string_concat(ori_string_t a, ori_string_t b);");
        ctx.writeln("bool ori_string_eq(ori_string_t a, ori_string_t b);");
        ctx.writeln("int ori_string_cmp(ori_string_t a, ori_string_t b);");
        ctx.writeln("uint64_t ori_string_len(ori_string_t s);");
        ctx.writeln("ori_string_t ori_string_clone(ori_string_t s);");
        ctx.writeln("void ori_string_release(ori_string_t s);");
        ctx.newline();
    }

    /// Emit option function declarations.
    fn emit_option_functions(ctx: &mut CodegenContext<'_>) {
        ctx.writeln("// Option functions");
        ctx.writeln("#define ORI_IS_SOME(opt) ((opt).tag == 1)");
        ctx.writeln("#define ORI_IS_NONE(opt) ((opt).tag == 0)");
        ctx.writeln("#define ORI_UNWRAP(opt) ((opt).value)");
        ctx.newline();
    }

    /// Emit result function declarations.
    fn emit_result_functions(ctx: &mut CodegenContext<'_>) {
        ctx.writeln("// Result functions");
        ctx.writeln("#define ORI_IS_OK(res) ((res).tag == 0)");
        ctx.writeln("#define ORI_IS_ERR(res) ((res).tag == 1)");
        ctx.writeln("#define ORI_UNWRAP_OK(res) ((res).value.ok)");
        ctx.writeln("#define ORI_UNWRAP_ERR(res) ((res).value.err)");
        ctx.newline();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::StringInterner;
    use ori_types::TypeInterner;

    #[test]
    fn test_emit_types() {
        let interner = StringInterner::new();
        let type_interner = TypeInterner::new();
        let mut ctx = CodegenContext::new(&interner, &type_interner, &[]);

        CRuntime::emit_types(&mut ctx);
        let output = ctx.take_output();

        assert!(output.contains("ori_string_t"));
        assert!(output.contains("ORI_SSO_CAP"));
        assert!(output.contains("ori_option_int_t"));
        assert!(output.contains("ori_result_int_str_t"));
    }

    #[test]
    fn test_emit_functions() {
        let interner = StringInterner::new();
        let type_interner = TypeInterner::new();
        let mut ctx = CodegenContext::new(&interner, &type_interner, &[]);

        CRuntime::emit_functions(&mut ctx);
        let output = ctx.take_output();

        assert!(output.contains("ori_arc_retain"));
        assert!(output.contains("ori_arc_release"));
        assert!(output.contains("ori_string_from_cstr"));
        assert!(output.contains("ori_panic"));
    }
}
