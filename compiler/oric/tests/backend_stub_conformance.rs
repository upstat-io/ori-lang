//! Conformance gate for the `ori_codegen::CodegenBackend` boundary.
//!
//! Implements two trivial outside-crate backends — a no-op artifact producer
//! and a failing one — using ONLY `oric`'s public API plus `ori_codegen`'s
//! public `CodegenBackend`/`CodegenInput`/`BackendError` surface, never
//! `oric`'s internal `commands::backend` module. Proves the contract is
//! implementable by an outside crate, selectable through the same closed-enum
//! dispatch shape `oric::commands::backend::CodegenBackendChoice` uses, and
//! that a backend refusal surfaces as a typed `BackendError` through that
//! dispatch rather than a panic — all with zero further `oric` driver changes.

#![cfg(all(feature = "llvm", feature = "backend_stub_conformance"))]

use ori_codegen::{BackendError, CodegenBackend, CodegenInput};
use ori_repr::NarrowingPolicy;
use oric::query::{parsed, typed, typed_pool};
use oric::{CompilerDb, Db, SourceFile};

/// A trivial outside-crate `CodegenBackend`: produces `()` — no LLVM, no
/// `oric` internals, zero further driver changes required to compile it.
struct NoOpBackend;

impl<'ctx> CodegenBackend<'ctx> for NoOpBackend {
    type Artifact = ();

    fn compile<'p>(&self, _input: &CodegenInput<'ctx, 'p>) -> Result<Self::Artifact, BackendError> {
        Ok(())
    }
}

/// The negative counterpart: a backend that refuses every input, pinning that a
/// refusal travels the typed `BackendError` channel with its message intact.
struct RefusingBackend;

impl RefusingBackend {
    const REFUSAL: &'static str = "RefusingBackend declines every CodegenInput";
}

impl<'ctx> CodegenBackend<'ctx> for RefusingBackend {
    type Artifact = ();

    fn compile<'p>(&self, _input: &CodegenInput<'ctx, 'p>) -> Result<Self::Artifact, BackendError> {
        Err(BackendError::from(Self::REFUSAL.to_string()))
    }
}

/// A local closed-enum selector mirroring `oric::commands::backend::CodegenBackendChoice`'s
/// shape — proves the trait supports enum dispatch from an outside crate, not
/// just direct method calls.
enum StubBackendChoice {
    NoOp(NoOpBackend),
    Refusing(RefusingBackend),
}

impl StubBackendChoice {
    fn compile(&self, input: &CodegenInput<'_, '_>) -> Result<(), BackendError> {
        match self {
            StubBackendChoice::NoOp(backend) => backend.compile(input),
            StubBackendChoice::Refusing(backend) => backend.compile(input),
        }
    }
}

/// Everything a fixture `CodegenInput` borrows from, held together so the input
/// can be rebuilt per backend without re-running the front end.
struct Fixture {
    pool: std::sync::Arc<ori_types::Pool>,
    type_result: ori_types::TypeCheckResult,
    canon: ori_ir::canon::CanonResult,
}

fn build_fixture(db: &CompilerDb) -> Fixture {
    let file = SourceFile::new(
        db,
        "backend_stub_conformance.ori".into(),
        "@f () -> int = 42;".into(),
    );

    let parse_result = parsed(db, file);
    assert!(
        !parse_result.has_errors(),
        "conformance fixture must parse cleanly: {:?}",
        parse_result.errors
    );

    let type_result = typed(db, file);
    assert!(
        !type_result.has_errors(),
        "conformance fixture must typecheck cleanly: {:?}",
        type_result.errors()
    );

    let Some(pool) = typed_pool(db, file) else {
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

    Fixture {
        pool,
        type_result,
        canon,
    }
}

fn codegen_input(fixture: &Fixture) -> CodegenInput<'_, '_> {
    CodegenInput {
        pool: &fixture.pool,
        type_result: &fixture.type_result,
        canon: &fixture.canon,
        source_path: "backend_stub_conformance.ori",
        module_name: "backend_stub_conformance",
        symbol_prefix: "",
        target_triple: None,
        narrowing_policy: NarrowingPolicy::Aggressive,
        imported_type_metadata: &[],
        imported_collection_surfaces: &[],
    }
}

#[test]
fn backend_stub_conformance() {
    let db = CompilerDb::new();
    let fixture = build_fixture(&db);
    let input = codegen_input(&fixture);

    let backend = StubBackendChoice::NoOp(NoOpBackend);
    match backend.compile(&input) {
        Ok(()) => {}
        Err(e) => {
            panic!("trivial outside CodegenBackend must compile a well-typed CodegenInput: {e}")
        }
    }
}

#[test]
fn backend_stub_refusal_surfaces_as_backend_error() {
    let db = CompilerDb::new();
    let fixture = build_fixture(&db);
    let input = codegen_input(&fixture);

    let backend = StubBackendChoice::Refusing(RefusingBackend);
    match backend.compile(&input) {
        Ok(()) => panic!("RefusingBackend must not report success"),
        Err(e) => assert_eq!(
            e.to_string(),
            RefusingBackend::REFUSAL,
            "BackendError message must cross the closed-enum dispatch unchanged"
        ),
    }
}
