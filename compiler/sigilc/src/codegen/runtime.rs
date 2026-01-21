// C runtime code emission for Sigil
// Emits the runtime helpers needed by generated C code (String type, conversions)

use super::CodeGen;

impl CodeGen {
    pub(super) fn emit_runtime(&mut self) {
        self.emit_line("// Runtime helpers");
        self.emit_line("typedef struct { char* data; size_t len; } String;");
        self.emit_line("");
        self.emit_line("String str_new(const char* s) {");
        self.indent();
        self.emit_line("String str = { strdup(s), strlen(s) };");
        self.emit_line("return str;");
        self.dedent();
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("String str_concat(String a, String b) {");
        self.indent();
        self.emit_line("size_t len = a.len + b.len;");
        self.emit_line("char* data = malloc(len + 1);");
        self.emit_line("memcpy(data, a.data, a.len);");
        self.emit_line("memcpy(data + a.len, b.data, b.len);");
        self.emit_line("data[len] = '\\0';");
        self.emit_line("String str = { data, len };");
        self.emit_line("return str;");
        self.dedent();
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("String int_to_str(int64_t n) {");
        self.indent();
        self.emit_line("char buf[32];");
        self.emit_line("snprintf(buf, sizeof(buf), \"%ld\", n);");
        self.emit_line("return str_new(buf);");
        self.dedent();
        self.emit_line("}");
        self.emit_line("");
    }
}
