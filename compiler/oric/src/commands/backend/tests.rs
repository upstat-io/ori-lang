use std::path::PathBuf;

use ori_codegen::{BackendError, CodegenInput};
use ori_llvm::inkwell::context::Context;
use ori_repr::NarrowingPolicy;
use oric::{CompilerDb, SourceFile};

use super::{CodegenBackendChoice, EmittedArtifact, EmittedModule, LlvmBackend};
use crate::commands::codegen_pipeline::LlvmCodegenOutput;
use crate::commands::compile_common::{
    check_source, compile_to_llvm_with_imported_monos, compile_to_llvm_with_imports,
    ImportedModuleCompilation, ImportedMonoCompilation,
};
use crate::commands::ImportedSurfaces;

/// Compile-time pin: the closed-enum dispatch's return type is the neutral
/// artifact. Restoring a concrete backend's own output type to this signature
/// stops compiling here.
fn dispatch_returns_neutral_artifact<'ctx>(
    choice: &CodegenBackendChoice<'ctx, '_>,
    program: &CodegenInput<'ctx, '_>,
) -> Result<EmittedArtifact<'ctx>, BackendError> {
    choice.compile(program)
}

/// Compile-time pin: the neutral artifact is a distinct type, not an alias of
/// the LLVM backend's own output. Aliasing the two makes these impls conflict.
trait DistinctFromLlvmOutput {}
impl DistinctFromLlvmOutput for EmittedArtifact<'_> {}
impl DistinctFromLlvmOutput for LlvmCodegenOutput<'_> {}

fn assert_distinct_from_llvm_output<T: DistinctFromLlvmOutput>() {}

#[test]
fn neutral_artifact_is_a_distinct_type_from_the_llvm_backend_output() {
    // The pin is the impl pair above: an alias would make them conflict at
    // compile time. These calls keep both impls reachable.
    assert_distinct_from_llvm_output::<EmittedArtifact<'static>>();
    assert_distinct_from_llvm_output::<LlvmCodegenOutput<'static>>();
}

const MODULE_NAME: &str = "backend_dispatch_neutral_artifact";
const SOURCE: &str = "pub @answer () -> int = 42;\n@main () -> int = answer();\n";

/// The front-end products a `CodegenInput` borrows from, held together so the
/// real backend can be built against them.
struct Fixture {
    db: CompilerDb,
    path: String,
    parse: crate::parser::ParseOutput,
    typed: ori_types::TypeCheckResult,
    pool: std::sync::Arc<ori_types::Pool>,
    canon: ori_ir::canon::SharedCanonResult,
}

fn build_fixture() -> Fixture {
    let db = CompilerDb::new();
    let path = PathBuf::from("backend_dispatch_neutral_artifact.ori");
    let path_str = path.to_string_lossy().into_owned();
    let file = SourceFile::new(&db, path, SOURCE.to_string());

    let Some((parse, typed, pool, canon)) = check_source(&db, file, &path_str) else {
        panic!("fixture must parse, typecheck and canonicalize cleanly")
    };

    Fixture {
        db,
        path: path_str,
        parse,
        typed,
        pool,
        canon,
    }
}

fn assert_carries_module_and_exports(artifact: EmittedArtifact<'_>) {
    assert!(
        artifact
            .exports
            .iter()
            .any(|export| export.source_name == "answer"),
        "neutral artifact must carry the producer-authored callable exports"
    );

    let EmittedModule::Llvm(module) = artifact.module;
    assert!(
        module.verify().is_ok(),
        "neutral artifact must carry a well-formed emitted module"
    );
    assert!(
        module.get_functions().next().is_some(),
        "neutral artifact must carry the emitted module's compiled functions"
    );
}

/// Constructs the production `CodegenBackendChoice::Llvm` — not a test-local
/// mirror selector — and dispatches through it, asserting the returned neutral
/// artifact carries the emitted module and the callable exports.
#[test]
fn real_backend_choice_dispatch_returns_neutral_artifact() {
    let fixture = build_fixture();
    let context = Context::create();

    let program = CodegenInput {
        pool: &fixture.pool,
        type_result: &fixture.typed,
        canon: &fixture.canon,
        source_path: &fixture.path,
        module_name: MODULE_NAME,
        symbol_prefix: MODULE_NAME,
        target_triple: None,
        narrowing_policy: NarrowingPolicy::Aggressive,
        imported_type_metadata: &[],
        imported_collection_surfaces: &[],
    };
    let choice = CodegenBackendChoice::Llvm(LlvmBackend {
        context: &context,
        db: &fixture.db,
        parse_result: &fixture.parse,
        import_sigs: &[],
        imported: ImportedSurfaces {
            imported_mono_fns: &[],
            generic_templates: &[],
            re_interned_canons: &[],
        },
    });

    let dispatched = dispatch_returns_neutral_artifact(&choice, &program);
    match dispatched {
        Ok(artifact) => assert_carries_module_and_exports(artifact),
        Err(e) => panic!("real dispatch must compile a well-typed CodegenInput: {e}"),
    }
}

/// Drives the production driver entry that builds and dispatches the real
/// `CodegenBackendChoice`, asserting the neutral artifact reaches the driver.
#[test]
fn production_compile_entry_returns_neutral_artifact() {
    let fixture = build_fixture();
    let context = Context::create();

    let compiled = compile_to_llvm_with_imports(ImportedModuleCompilation {
        context: &context,
        db: &fixture.db,
        parse: &fixture.parse,
        typed: &fixture.typed,
        pool: &fixture.pool,
        canon: &fixture.canon,
        source_path: &fixture.path,
        module_name: MODULE_NAME,
        imported_functions: &[],
        imported_type_metadata: &[],
        imported_collection_surfaces: &[],
        imported: ImportedSurfaces {
            imported_mono_fns: &[],
            generic_templates: &[],
            re_interned_canons: &[],
        },
        target_triple: None,
        narrowing_policy: NarrowingPolicy::Aggressive,
    });
    match compiled {
        Ok(artifact) => assert_carries_module_and_exports(artifact),
        Err(e) => panic!("production compile entry must succeed on a well-typed module: {e}"),
    }
}

/// Drives the second production driver entry that builds and dispatches the
/// real `CodegenBackendChoice` — the one the single-module build and run
/// commands call — asserting the emitted module reaches the driver with the
/// source's compiled bodies.
#[test]
fn production_mono_compile_entry_returns_the_emitted_module() {
    let fixture = build_fixture();
    let context = Context::create();

    let compiled = compile_to_llvm_with_imported_monos(ImportedMonoCompilation {
        context: &context,
        db: &fixture.db,
        parse: &fixture.parse,
        typed: &fixture.typed,
        pool: &fixture.pool,
        imported: ImportedSurfaces {
            imported_mono_fns: &[],
            generic_templates: &[],
            re_interned_canons: &[],
        },
        canon: &fixture.canon,
        source_path: &fixture.path,
        target_triple: None,
        narrowing_policy: NarrowingPolicy::Aggressive,
    });

    let module = match compiled {
        Ok(module) => module,
        Err(e) => {
            panic!("the single-module compile entry must succeed on a well-typed module: {e}")
        }
    };
    assert!(
        module.verify().is_ok(),
        "the single-module compile entry must return a well-formed emitted module"
    );
    let emitted: Vec<String> = module
        .get_functions()
        .filter(|function| function.count_basic_blocks() > 0)
        .map(|function| function.get_name().to_string_lossy().into_owned())
        .collect();
    for symbol in ["_ori_answer", "_ori_main"] {
        assert!(
            emitted.iter().any(|name| name == symbol),
            "the single-module compile entry must emit `{symbol}`; it emitted {emitted:?}"
        );
    }
}
