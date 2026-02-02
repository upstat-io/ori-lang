//! Object file emission tests.
//!
//! Tests for the `ObjectEmitter` and `EmitError` types in `ori_llvm::aot::object`.
//! These tests validate:
//! - Output format extensions and descriptions
//! - Object emitter creation and configuration
//! - Emission to files and memory
//! - Error handling and display

#[cfg(feature = "llvm")]
mod tests {
    use std::error::Error;
    use std::path::Path;

    use ori_llvm::aot::object::{EmitError, ObjectEmitter, OutputFormat};
    use ori_llvm::aot::target::TargetError;
    use ori_llvm::inkwell::context::Context;

    #[test]
    fn test_output_format_extension() {
        assert_eq!(OutputFormat::Object.extension(), "o");
        assert_eq!(OutputFormat::Assembly.extension(), "s");
        assert_eq!(OutputFormat::Bitcode.extension(), "bc");
        assert_eq!(OutputFormat::LlvmIr.extension(), "ll");
    }

    #[test]
    fn test_output_format_description() {
        assert_eq!(OutputFormat::Object.description(), "native object file");
        assert_eq!(OutputFormat::Assembly.description(), "assembly text");
        assert_eq!(OutputFormat::Bitcode.description(), "LLVM bitcode");
        assert_eq!(OutputFormat::LlvmIr.description(), "LLVM IR text");
    }

    #[test]
    fn test_object_emitter_native() {
        // May fail on systems without proper LLVM setup
        if let Ok(emitter) = ObjectEmitter::native() {
            assert!(!emitter.config().triple().is_empty());
        }
    }

    #[test]
    fn test_object_emitter_configure_module() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test");

            let result = emitter.configure_module(&module);
            assert!(result.is_ok());

            // Module should have triple set
            let triple = module.get_triple();
            assert!(!triple.as_str().to_string_lossy().is_empty());
        }
    }

    #[test]
    fn test_emit_llvm_ir() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_ir");

            // Create a simple function
            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("test_func", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(42, false)))
                .unwrap();

            // Emit to temp file
            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join("test_emit.ll");

            let result = emitter.emit_llvm_ir(&module, &path);
            assert!(result.is_ok(), "emit_llvm_ir failed: {result:?}");

            // Verify file exists and contains expected content
            let ir_text = std::fs::read_to_string(&path).unwrap();
            assert!(ir_text.contains("test_func"));
            assert!(ir_text.contains("ret i64 42"));

            // Cleanup
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_emit_bitcode() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_bc");

            // Create a simple function
            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("bc_func", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(100, false)))
                .unwrap();

            // Emit to temp file
            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join("test_emit.bc");

            let result = emitter.emit_bitcode(&module, &path);
            assert!(result.is_ok(), "emit_bitcode failed: {result:?}");

            // Verify file exists and has content
            let metadata = std::fs::metadata(&path).unwrap();
            assert!(metadata.len() > 0);

            // Cleanup
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_emit_object() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_obj");

            // Configure module for target
            emitter.configure_module(&module).unwrap();

            // Create a simple function
            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("obj_func", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(200, false)))
                .unwrap();

            // Emit to temp file
            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join("test_emit.o");

            let result = emitter.emit_object(&module, &path);
            assert!(result.is_ok(), "emit_object failed: {result:?}");

            // Verify file exists and has content
            let metadata = std::fs::metadata(&path).unwrap();
            assert!(metadata.len() > 0);

            // Cleanup
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_emit_assembly() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_asm");

            // Configure module for target
            emitter.configure_module(&module).unwrap();

            // Create a simple function
            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("asm_func", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(300, false)))
                .unwrap();

            // Emit to temp file
            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join("test_emit.s");

            let result = emitter.emit_assembly(&module, &path);
            assert!(result.is_ok(), "emit_assembly failed: {result:?}");

            // Verify file exists and contains assembly
            let asm_text = std::fs::read_to_string(&path).unwrap();
            assert!(asm_text.contains("asm_func") || asm_text.contains("_asm_func"));

            // Cleanup
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_emit_object_to_memory() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_mem");

            // Configure module for target
            emitter.configure_module(&module).unwrap();

            // Create a simple function
            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("mem_func", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(400, false)))
                .unwrap();

            let result = emitter.emit_object_to_memory(&module);
            assert!(result.is_ok(), "emit_object_to_memory failed: {result:?}");

            let bytes = result.unwrap();
            assert!(!bytes.is_empty());
        }
    }

    #[test]
    fn test_emit_invalid_path() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_invalid");

            // Try to emit to a non-existent directory
            let path = Path::new("/nonexistent/directory/file.o");
            let result = emitter.emit_object(&module, path);

            assert!(matches!(result, Err(EmitError::InvalidPath { .. })));
        }
    }

    #[test]
    fn test_emit_error_display() {
        let err = EmitError::ObjectEmission {
            path: "test.o".to_string(),
            message: "LLVM error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to emit object file 'test.o': LLVM error"
        );

        let err = EmitError::InvalidPath {
            path: "/bad/path".to_string(),
            reason: "does not exist".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "invalid output path '/bad/path': does not exist"
        );
    }

    #[test]
    fn test_object_emitter_debug() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let debug_str = format!("{emitter:?}");
            assert!(debug_str.contains("ObjectEmitter"));
            assert!(debug_str.contains("target"));
        }
    }

    #[test]
    fn test_object_emitter_machine() {
        if let Ok(emitter) = ObjectEmitter::native() {
            // Verify we can access the machine and it has expected properties
            let _machine = emitter.machine();
            // Just verify it doesn't panic - TargetMachine has limited introspection
        }
    }

    #[test]
    fn test_emit_error_display_all_variants() {
        // TargetMachine error
        let target_err = TargetError::UnsupportedTarget {
            triple: "test".to_string(),
            supported: vec!["x86_64-unknown-linux-gnu"],
        };
        let err = EmitError::TargetMachine(target_err);
        let display = err.to_string();
        assert!(display.contains("failed to create target machine"));
        assert!(display.contains("test"));

        // ModuleConfiguration error
        let config_err = TargetError::InitializationFailed("test init".to_string());
        let err = EmitError::ModuleConfiguration(config_err);
        let display = err.to_string();
        assert!(display.contains("failed to configure module"));

        // AssemblyEmission error
        let err = EmitError::AssemblyEmission {
            path: "test.s".to_string(),
            message: "assembly error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to emit assembly file 'test.s': assembly error"
        );

        // BitcodeEmission error
        let err = EmitError::BitcodeEmission {
            path: "test.bc".to_string(),
            message: "bitcode error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to emit bitcode file 'test.bc': bitcode error"
        );

        // LlvmIrEmission error
        let err = EmitError::LlvmIrEmission {
            path: "test.ll".to_string(),
            message: "ir error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to emit LLVM IR file 'test.ll': ir error"
        );
    }

    #[test]
    fn test_emit_error_source() {
        // TargetMachine has a source
        let target_err = TargetError::InitializationFailed("test".to_string());
        let err = EmitError::TargetMachine(target_err);
        assert!(err.source().is_some());

        // ModuleConfiguration has a source
        let config_err = TargetError::InitializationFailed("test".to_string());
        let err = EmitError::ModuleConfiguration(config_err);
        assert!(err.source().is_some());

        // ObjectEmission has no source
        let err = EmitError::ObjectEmission {
            path: "test.o".to_string(),
            message: "error".to_string(),
        };
        assert!(err.source().is_none());

        // InvalidPath has no source
        let err = EmitError::InvalidPath {
            path: "/bad".to_string(),
            reason: "test".to_string(),
        };
        assert!(err.source().is_none());
    }

    #[test]
    fn test_emit_error_from_target_error() {
        let target_err = TargetError::UnsupportedTarget {
            triple: "bad-target".to_string(),
            supported: vec!["x86_64-unknown-linux-gnu"],
        };
        let emit_err: EmitError = target_err.into();
        assert!(matches!(emit_err, EmitError::TargetMachine(_)));
    }

    #[test]
    fn test_emit_bitcode_invalid_path() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_bc");
            emitter.configure_module(&module).unwrap();

            // Try to emit to a path with non-existent parent
            let bad_path = std::path::Path::new("/nonexistent_dir_12345/test.bc");
            let result = emitter.emit_bitcode(&module, bad_path);
            assert!(result.is_err());
            if let Err(EmitError::InvalidPath { path, reason }) = result {
                assert!(path.contains("nonexistent"));
                assert!(reason.contains("parent"));
            }
        }
    }

    #[test]
    fn test_emit_llvm_ir_invalid_path() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_ir");
            emitter.configure_module(&module).unwrap();

            // Try to emit to a path with non-existent parent
            let bad_path = std::path::Path::new("/nonexistent_dir_12345/test.ll");
            let result = emitter.emit_llvm_ir(&module, bad_path);
            assert!(result.is_err());
            if let Err(EmitError::InvalidPath { path, reason }) = result {
                assert!(path.contains("nonexistent"));
                assert!(reason.contains("parent"));
            }
        }
    }

    #[test]
    fn test_emit_dispatch_bitcode() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_dispatch_bc");
            emitter.configure_module(&module).unwrap();

            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("dispatch_bc", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(1, false)))
                .unwrap();

            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join("test_dispatch.bc");

            // Use the dispatch emit method with Bitcode format
            let result = emitter.emit(&module, &path, OutputFormat::Bitcode);
            assert!(result.is_ok());

            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_emit_dispatch_llvm_ir() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_dispatch_ir");
            emitter.configure_module(&module).unwrap();

            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("dispatch_ir", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(2, false)))
                .unwrap();

            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join("test_dispatch.ll");

            // Use the dispatch emit method with LlvmIr format
            let result = emitter.emit(&module, &path, OutputFormat::LlvmIr);
            assert!(result.is_ok());

            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_emit_assembly_to_memory() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_asm_mem");
            emitter.configure_module(&module).unwrap();

            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("asm_mem_func", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(99, false)))
                .unwrap();

            let result = emitter.emit_assembly_to_memory(&module);
            assert!(result.is_ok());
            let asm_bytes = result.unwrap();
            assert!(!asm_bytes.is_empty());
        }
    }
}
