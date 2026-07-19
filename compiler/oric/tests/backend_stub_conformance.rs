//! Plan-wide north-star conformance gate for the `CodegenBackend` boundary
//! (`plans/backend-boundary`).
//!
//! Implements a genuinely trivial second `CodegenBackend` — a no-op
//! `Artifact` producer — using ONLY `oric`'s public API + `ori_repr`'s
//! public `CodegenBackend`/`RealizedProgram` surface, never `oric`'s
//! internal `commands::backend` module. Proves the trait is implementable
//! by an outside crate, selectable through the same closed-enum dispatch
//! shape `oric::commands::backend::BackendChoice` uses, with zero further
//! `oric` driver changes. See `.claude/rules/compiler.md §Dispatch`.

#![cfg(all(feature = "llvm", feature = "backend_stub_conformance"))]

use ori_repr::{BackendError, CodegenBackend, NarrowingPolicy, RealizedProgram};
use oric::query::{parsed, typed, typed_pool};
use oric::{CompilerDb, Db, SourceFile};

/// A trivial outside-crate `CodegenBackend`: produces `()` — no LLVM, no
/// `oric` internals, zero further driver changes required to compile it.
struct NoOpBackend;

impl<'ctx> CodegenBackend<'ctx> for NoOpBackend {
    type Artifact = ();

    fn compile<'p>(
        &self,
        _program: &RealizedProgram<'ctx, 'p>,
    ) -> Result<Self::Artifact, BackendError> {
        Ok(())
    }
}

/// A local closed-enum selector mirroring `oric::commands::backend::BackendChoice`'s
/// shape — proves the trait supports enum dispatch (`.claude/rules/compiler.md
/// §Dispatch`) from an outside crate, not just direct method calls.
enum StubBackendChoice {
    NoOp(NoOpBackend),
}

impl StubBackendChoice {
    fn compile(&self, program: &RealizedProgram<'_, '_>) -> Result<(), BackendError> {
        match self {
            StubBackendChoice::NoOp(backend) => backend.compile(program),
        }
    }
}

#[test]
fn backend_stub_conformance() {
    let db = CompilerDb::new();
    let file = SourceFile::new(
        &db,
        "backend_stub_conformance.ori".into(),
        "@f () -> int = 42;".into(),
    );

    let parse_result = parsed(&db, file);
    assert!(
        !parse_result.has_errors(),
        "conformance fixture must parse cleanly: {:?}",
        parse_result.errors
    );

    let type_result = typed(&db, file);
    assert!(
        !type_result.has_errors(),
        "conformance fixture must typecheck cleanly: {:?}",
        type_result.errors()
    );

    let Some(pool) = typed_pool(&db, file) else {
        panic!("typed pool available after clean typecheck")
    };
    let interner = db.interner();
    let canon = ori_canon::lower_module(
        &parse_result.module,
        &parse_result.arena,
        &type_result,
        &pool,
        interner,
    );

    let program = RealizedProgram {
        pool: &pool,
        type_result: &type_result,
        canon: &canon,
        source_path: "backend_stub_conformance.ori",
        module_name: "backend_stub_conformance",
        symbol_prefix: "",
        target_triple: None,
        narrowing_policy: NarrowingPolicy::Aggressive,
        imported_type_metadata: &[],
        imported_collection_surfaces: &[],
    };

    let backend = StubBackendChoice::NoOp(NoOpBackend);
    match backend.compile(&program) {
        Ok(()) => {}
        Err(e) => {
            panic!("trivial outside CodegenBackend must compile a well-typed RealizedProgram: {e}")
        }
    }
}
