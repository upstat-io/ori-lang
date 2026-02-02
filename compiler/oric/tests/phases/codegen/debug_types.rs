//! Debug info type tests.
//!
//! Tests for composite debug types: struct, enum, pointer, array, typedef,
//! and Ori-specific types like string, Option, Result, and list.

#[cfg(feature = "llvm")]
mod tests {
    use ori_llvm::aot::debug::{DebugInfoBuilder, DebugInfoConfig, DebugLevel, FieldInfo};
    use ori_llvm::inkwell::context::Context;

    #[test]
    fn test_debug_struct_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().unwrap().as_type();
        let float_ty = builder.float_type().unwrap().as_type();

        // Create Point { x: int, y: float }
        let fields = [
            FieldInfo {
                name: "x",
                ty: int_ty,
                size_bits: 64,
                offset_bits: 0,
                line: 1,
            },
            FieldInfo {
                name: "y",
                ty: float_ty,
                size_bits: 64,
                offset_bits: 64,
                line: 2,
            },
        ];

        let _struct_ty = builder.create_struct_type("Point", 1, 128, 64, &fields);

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_enum_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let byte_ty = builder.byte_type().unwrap().as_type();

        // Create Color enum
        let _enum_ty = builder.create_enum_type(
            "Color",
            1,
            8,
            8,
            &[("Red", 0), ("Green", 1), ("Blue", 2)],
            byte_ty,
        );

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_pointer_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().unwrap().as_type();
        let _ptr_ty = builder.create_pointer_type("*int", int_ty, 64);

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_array_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().unwrap().as_type();
        // [int; 10] - array of 10 ints, each 64 bits = 640 bits total
        let _array_ty = builder.create_array_type(int_ty, 10, 640, 64);

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_typedef() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().unwrap().as_type();
        // type UserId = int
        let _typedef = builder.create_typedef("UserId", int_ty, 1, 64);

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_string_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let _str_ty = builder.string_type().unwrap();

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_option_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().unwrap().as_type();
        let _option_ty = builder.option_type(int_ty, 64).unwrap();

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_result_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().unwrap().as_type();
        let str_ty = builder.string_type().unwrap().as_type();
        let _result_ty = builder.result_type(int_ty, 64, str_ty, 128).unwrap();

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_list_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().unwrap().as_type();
        let _list_ty = builder.list_type(int_ty).unwrap();

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_string_type_detailed() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        // Call string_type and verify it produces a valid composite type
        let str_ty = builder.string_type().unwrap();
        assert!(!str_ty.as_mut_ptr().is_null());

        // Create string_type again to test caching behavior
        let str_ty2 = builder.string_type().unwrap();
        assert!(!str_ty2.as_mut_ptr().is_null());

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_option_type_detailed() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().unwrap().as_type();

        // Create Option<int> with explicit size
        let option_ty = builder.option_type(int_ty, 64).unwrap();
        assert!(!option_ty.as_mut_ptr().is_null());

        // Create Option<bool> with smaller payload
        let bool_ty = builder.bool_type().unwrap().as_type();
        let option_bool = builder.option_type(bool_ty, 8).unwrap();
        assert!(!option_bool.as_mut_ptr().is_null());

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_result_type_detailed() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().unwrap().as_type();
        let str_ty = builder.string_type().unwrap().as_type();

        // Create Result<int, str>
        let result_ty = builder.result_type(int_ty, 64, str_ty, 128).unwrap();
        assert!(!result_ty.as_mut_ptr().is_null());

        // Create Result<bool, int> with smaller ok type
        let bool_ty = builder.bool_type().unwrap().as_type();
        let result_small = builder.result_type(bool_ty, 8, int_ty, 64).unwrap();
        assert!(!result_small.as_mut_ptr().is_null());

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_list_type_detailed() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().unwrap().as_type();
        let float_ty = builder.float_type().unwrap().as_type();

        // Create [int]
        let list_int = builder.list_type(int_ty).unwrap();
        assert!(!list_int.as_mut_ptr().is_null());

        // Create [float]
        let list_float = builder.list_type(float_ty).unwrap();
        assert!(!list_float.as_mut_ptr().is_null());

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_struct_with_many_fields() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().unwrap().as_type();
        let float_ty = builder.float_type().unwrap().as_type();
        let bool_ty = builder.bool_type().unwrap().as_type();
        let str_ty = builder.string_type().unwrap().as_type();

        // Create a struct with many fields
        let fields = [
            FieldInfo {
                name: "id",
                ty: int_ty,
                size_bits: 64,
                offset_bits: 0,
                line: 1,
            },
            FieldInfo {
                name: "value",
                ty: float_ty,
                size_bits: 64,
                offset_bits: 64,
                line: 2,
            },
            FieldInfo {
                name: "active",
                ty: bool_ty,
                size_bits: 8,
                offset_bits: 128,
                line: 3,
            },
            FieldInfo {
                name: "name",
                ty: str_ty,
                size_bits: 128,
                offset_bits: 192,
                line: 4,
            },
        ];

        let _struct_ty = builder.create_struct_type("Record", 1, 320, 64, &fields);

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_enum_many_variants() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let byte_ty = builder.byte_type().unwrap().as_type();

        // Create an enum with many variants
        let _enum_ty = builder.create_enum_type(
            "Status",
            1,
            8,
            8,
            &[
                ("Pending", 0),
                ("Running", 1),
                ("Success", 2),
                ("Failed", 3),
                ("Cancelled", 4),
            ],
            byte_ty,
        );

        builder.finalize();
        assert!(module.verify().is_ok());
    }
}
