//! String literal compilation.

use inkwell::values::BasicValueEnum;
use ori_ir::Name;

use crate::builder::Builder;

impl<'a, 'll, 'tcx> Builder<'a, 'll, 'tcx> {
    /// Compile a string literal.
    ///
    /// Creates a global constant string and returns a pointer to it.
    /// Strings are represented as { i64 len, i8* data } structs.
    pub(crate) fn compile_string(&self, name: Name) -> Option<BasicValueEnum<'ll>> {
        let string_content = self.cx().interner.lookup(name);

        // Create a unique global name for this string based on a hash
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        string_content.hash(&mut hasher);
        let global_name = format!(".str.{:x}", hasher.finish());

        // Check if we already have this string as a global
        if let Some(global) = self.cx().llmod().get_global(&global_name) {
            // Return pointer to existing string data
            let ptr = global.as_pointer_value();

            // Create string struct { len, data_ptr }
            let len = self.cx().scx.type_i64().const_int(string_content.len() as u64, false);
            let string_struct = self.cx().string_type();

            let struct_val = self.build_struct(
                string_struct,
                &[len.into(), ptr.into()],
                "str",
            );

            return Some(struct_val.into());
        }

        // Create a null-terminated string constant
        let string_bytes: Vec<u8> = string_content.bytes().chain(std::iter::once(0)).collect();
        let string_const = self.cx().llcx().const_string(&string_bytes, false);

        // Create global variable for the string data
        let global = self.cx().llmod().add_global(string_const.get_type(), None, &global_name);
        global.set_linkage(inkwell::module::Linkage::Private);
        global.set_constant(true);
        global.set_initializer(&string_const);

        // Get pointer to the string data
        let ptr = global.as_pointer_value();

        // Create string struct { len, data_ptr }
        let len = self.cx().scx.type_i64().const_int(string_content.len() as u64, false);
        let string_struct = self.cx().string_type();

        let struct_val = self.build_struct(
            string_struct,
            &[len.into(), ptr.into()],
            "str",
        );

        Some(struct_val.into())
    }
}
