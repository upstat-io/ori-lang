//! L12 production-path pins for `ori emit-scip` (full SCIP definition index).
//!
//! Spawns the REAL `ori` binary against temp `.ori` fixtures and decodes the
//! emitted `index.scip` back through the `scip` protobuf crate, asserting the
//! `SymbolInformation` set, the exact (globally-stable) SCIP symbol strings,
//! and the kinds. The CLI-handler layer is otherwise unit-untested, so these
//! end-to-end pins are the load-bearing coverage for the emit pipe.
//!
//! Each fixture is emitted with the temp dir as the working directory and a
//! relative path argument, so the minted module identity is the project-relative
//! path (deterministic + machine-independent), not an absolute one.

use std::path::Path;
use std::process::Command;

use protobuf::Message;
use scip::types::symbol_information::Kind;
use scip::types::{Index, SymbolInformation};

// One fixture per (entity-kind x pattern) matrix cell.
const SINGLE_FN_FIXTURE: &str = include_str!("fixtures/emit_scip/greet.ori");
const TWO_FN_FIXTURE: &str = include_str!("fixtures/emit_scip/two_fn.ori");
const STRUCT_FIXTURE: &str = include_str!("fixtures/emit_scip/no_fn.ori");
const SUM_FIXTURE: &str = include_str!("fixtures/emit_scip/sum_type.ori");
const TRAITS_FIXTURE: &str = include_str!("fixtures/emit_scip/traits.ori");
const IMPL_FIXTURE: &str = include_str!("fixtures/emit_scip/impl_methods.ori");
const TRAIT_IMPL_FIXTURE: &str = include_str!("fixtures/emit_scip/trait_impl.ori");
const DEF_IMPL_FIXTURE: &str = include_str!("fixtures/emit_scip/def_impl.ori");
const EXTEND_FIXTURE: &str = include_str!("fixtures/emit_scip/extend_methods.ori");
const MULTI_FIXTURE: &str = include_str!("fixtures/emit_scip/multi.ori");
const EMPTY_FIXTURE: &str = include_str!("fixtures/emit_scip/empty.ori");
const TYPE_ERROR_FIXTURE: &str = include_str!("fixtures/emit_scip/type_error.ori");

/// Write `content` to `dir/name`, creating any parent directories the relative
/// `name` implies (e.g. `mod_a/greet.ori`).
fn write_fixture(dir: &tempfile::TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("failed to create dir: {e}"));
    }
    std::fs::write(&path, content).unwrap_or_else(|e| panic!("failed to write fixture: {e}"));
    path
}

/// Run `ori emit-scip <fixture_arg> -o <output_arg>` with `cwd` as the working
/// directory, so the emitted symbols carry a project-relative module identity.
fn run_emit_scip(cwd: &Path, fixture_arg: &str, output_arg: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ori"))
        .current_dir(cwd)
        .arg("emit-scip")
        .arg(fixture_arg)
        .arg("-o")
        .arg(output_arg)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn ori emit-scip: {e}"))
}

/// Drive the real CLI on `content` written to relative `name`, decoding the
/// emitted `index.scip` back through the `scip` protobuf crate. Asserts a clean
/// exit. `name` becomes the project-relative module identity (minus `.ori`).
fn emit_and_decode(name: &str, content: &str) -> Index {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    write_fixture(&dir, name, content);

    let out = run_emit_scip(dir.path(), name, "index.scip");
    assert!(
        out.status.success(),
        "emit-scip exited non-zero: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let bytes = std::fs::read(dir.path().join("index.scip"))
        .unwrap_or_else(|e| panic!("index.scip written: {e}"));
    Index::parse_from_bytes(&bytes).unwrap_or_else(|e| panic!("index.scip is valid protobuf: {e}"))
}

/// The (sorted) SCIP symbol strings of the single emitted document.
fn symbol_strings(index: &Index) -> Vec<String> {
    document(index)
        .symbols
        .iter()
        .map(|s| s.symbol.clone())
        .collect()
}

fn document(index: &Index) -> &scip::types::Document {
    index
        .documents
        .first()
        .unwrap_or_else(|| panic!("index carries exactly one document"))
}

/// The `SymbolInformation` for an exact symbol string.
fn find<'a>(index: &'a Index, symbol: &str) -> &'a SymbolInformation {
    document(index)
        .symbols
        .iter()
        .find(|s| s.symbol == symbol)
        .unwrap_or_else(|| panic!("symbol `{symbol}` present in index"))
}

fn kind(info: &SymbolInformation) -> Kind {
    info.kind.enum_value_or_default()
}

// Function cell

#[test]
fn emit_scip_function_mints_method_descriptor() {
    // Stability pin: the `greet.ori` fixture at the project root mints exactly
    // this module-qualified symbol shape. The `greet` package component is the
    // source module identity (path minus `.ori`).
    let index = emit_and_decode("greet.ori", SINGLE_FN_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec!["ori . greet . greet().".to_string()],
        "a single function emits exactly one module-qualified Method-descriptor symbol"
    );
    let greet = find(&index, "ori . greet . greet().");
    assert_eq!(greet.display_name, "greet");
    assert_eq!(kind(greet), Kind::Function);
}

#[test]
fn emit_scip_nested_module_path_mints_slash_joined_module_component() {
    // Stability pin for a subdirectory path: the module component is the
    // project-relative path (forward-slash normalized, minus `.ori`).
    let index = emit_and_decode("pkg/util.ori", SINGLE_FN_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec!["ori . pkg/util . greet().".to_string()],
        "the module component is the forward-slash-joined project-relative path"
    );
}

#[test]
fn emit_scip_same_name_in_different_files_mints_distinct_symbols() {
    // F2 regression pin: two top-level `greet` definitions in DIFFERENT source
    // files must mint distinct symbol strings (disambiguated by the module
    // component), so the exact-equality occurrence->definition join never
    // collides on a cross-file homonym.
    let index_a = emit_and_decode("mod_a/greet.ori", SINGLE_FN_FIXTURE);
    let index_b = emit_and_decode("mod_b/greet.ori", SINGLE_FN_FIXTURE);

    let syms_a = symbol_strings(&index_a);
    let syms_b = symbol_strings(&index_b);

    assert_eq!(syms_a, vec!["ori . mod_a/greet . greet().".to_string()]);
    assert_eq!(syms_b, vec!["ori . mod_b/greet . greet().".to_string()]);
    assert_ne!(
        syms_a, syms_b,
        "same-named functions in different files must not collide"
    );
}

#[test]
fn emit_scip_emits_every_function_not_just_the_first() {
    // Negative pin against the walking-skeleton behavior (first-function-only).
    let index = emit_and_decode("two.ori", TWO_FN_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec![
            "ori . two . add().".to_string(),
            "ori . two . zebra().".to_string(),
        ],
        "both functions are indexed, sorted by symbol string"
    );
}

// Struct type + fields cell

#[test]
fn emit_scip_struct_emits_type_and_fields() {
    let index = emit_and_decode("point.ori", STRUCT_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec![
            "ori . point . Point#".to_string(),
            "ori . point . Point#x.".to_string(),
            "ori . point . Point#y.".to_string(),
        ],
    );
    assert_eq!(kind(find(&index, "ori . point . Point#")), Kind::Struct);
    let field_x = find(&index, "ori . point . Point#x.");
    assert_eq!(field_x.display_name, "x");
    assert_eq!(kind(field_x), Kind::Field);
}

// Sum type + variants + variant fields cell

#[test]
fn emit_scip_sum_emits_enum_variants_and_variant_fields() {
    let index = emit_and_decode("color.ori", SUM_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec![
            "ori . color . Color#".to_string(),
            "ori . color . Color#Blue.".to_string(),
            "ori . color . Color#Custom.".to_string(),
            "ori . color . Color#Custom.b.".to_string(),
            "ori . color . Color#Custom.g.".to_string(),
            "ori . color . Color#Custom.r.".to_string(),
            "ori . color . Color#Green.".to_string(),
            "ori . color . Color#Red.".to_string(),
        ],
    );
    assert_eq!(kind(find(&index, "ori . color . Color#")), Kind::Enum);
    assert_eq!(
        kind(find(&index, "ori . color . Color#Red.")),
        Kind::EnumMember
    );
    let variant_field = find(&index, "ori . color . Color#Custom.r.");
    assert_eq!(variant_field.display_name, "r");
    assert_eq!(kind(variant_field), Kind::Field);
}

// Trait + trait-method cell

#[test]
fn emit_scip_trait_emits_trait_and_methods() {
    let index = emit_and_decode("traits.ori", TRAITS_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec![
            "ori . traits . Describable#".to_string(),
            "ori . traits . Describable#describe().".to_string(),
            "ori . traits . Measurable#".to_string(),
            "ori . traits . Measurable#measure().".to_string(),
        ],
    );
    assert_eq!(
        kind(find(&index, "ori . traits . Describable#")),
        Kind::Trait
    );
    assert_eq!(
        kind(find(&index, "ori . traits . Describable#describe().")),
        Kind::Method
    );
}

// Inherent impl method cell

#[test]
fn emit_scip_inherent_impl_emits_methods() {
    let index = emit_and_decode("impl.ori", IMPL_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec![
            "ori . impl . Point#".to_string(),
            "ori . impl . Point#get_x().".to_string(),
            "ori . impl . Point#sum().".to_string(),
            "ori . impl . Point#x.".to_string(),
            "ori . impl . Point#y.".to_string(),
        ],
    );
    let method = find(&index, "ori . impl . Point#get_x().");
    assert_eq!(method.display_name, "get_x");
    assert_eq!(kind(method), Kind::Method);
}

// Trait-impl method cell — trait-method and impl-method are DISTINCT symbols

#[test]
fn emit_scip_trait_impl_emits_distinct_method_symbols() {
    let index = emit_and_decode("trait_impl.ori", TRAIT_IMPL_FIXTURE);
    let syms = symbol_strings(&index);
    // The trait's `describe` declaration and the impl's `describe` definition
    // mint distinct symbols keyed on their respective parent (trait vs type).
    assert!(
        syms.contains(&"ori . trait_impl . Describable#describe().".to_string()),
        "trait-method symbol present: {syms:?}"
    );
    assert!(
        syms.contains(&"ori . trait_impl . Rectangle#describe().".to_string()),
        "impl-method symbol present: {syms:?}"
    );
    assert_eq!(
        kind(find(&index, "ori . trait_impl . Rectangle#describe().")),
        Kind::Method
    );
}

// Default-impl method cell (F3) — `def impl Trait { @m }` methods are indexed

#[test]
fn emit_scip_def_impl_emits_method_keyed_on_trait() {
    // The def-impl method is keyed on the trait it implements; the symbol is
    // present in the index (mirrors the trait-declared method symbol — one
    // logical definition, deduped on the symbol string).
    let index = emit_and_decode("def_impl.ori", DEF_IMPL_FIXTURE);
    let syms = symbol_strings(&index);
    assert!(
        syms.contains(&"ori . def_impl . Greeter#greeting().".to_string()),
        "def-impl method symbol present: {syms:?}"
    );
    assert_eq!(
        kind(find(&index, "ori . def_impl . Greeter#greeting().")),
        Kind::Method
    );
}

// Extension method cell (F3) — `extend Type { @m }` methods are indexed

#[test]
fn emit_scip_extend_emits_method_keyed_on_target_type() {
    // F3 regression pin: an extension method is keyed on its target type and is
    // NOT minted by any other walk (no trait, the struct walk emits only the
    // type + fields), so its presence proves the `extends` walk runs.
    let index = emit_and_decode("extend_methods.ori", EXTEND_FIXTURE);
    let syms = symbol_strings(&index);
    assert!(
        syms.contains(&"ori . extend_methods . Widget#doubled().".to_string()),
        "extension method symbol present: {syms:?}"
    );
    let method = find(&index, "ori . extend_methods . Widget#doubled().");
    assert_eq!(method.display_name, "doubled");
    assert_eq!(kind(method), Kind::Method);
}

// Multi-entity cell — every kind in one file, full deterministic set

#[test]
fn emit_scip_multi_entity_emits_full_sorted_set() {
    let index = emit_and_decode("multi.ori", MULTI_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec![
            "ori . multi . Drawable#".to_string(),
            "ori . multi . Drawable#draw().".to_string(),
            "ori . multi . Point#".to_string(),
            "ori . multi . Point#draw().".to_string(),
            "ori . multi . Point#magnitude().".to_string(),
            "ori . multi . Point#x.".to_string(),
            "ori . multi . Point#y.".to_string(),
            "ori . multi . Shape#".to_string(),
            "ori . multi . Shape#Circle.".to_string(),
            "ori . multi . Shape#Circle.radius.".to_string(),
            "ori . multi . Shape#Square.".to_string(),
            "ori . multi . area().".to_string(),
        ],
        "full definition inventory, deterministically sorted by symbol string"
    );
}

// Negative / edge cells

#[test]
fn emit_scip_function_less_file_emits_zero_symbol_index() {
    let index = emit_and_decode("empty.ori", EMPTY_FIXTURE);
    // The deliverable: a structurally-valid 0-symbol index (NOT a missing file
    // or a non-zero exit). The document is still present.
    assert_eq!(document(&index).symbols.len(), 0);
}

#[test]
fn emit_scip_rejects_type_error_file() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    write_fixture(&dir, "bad.ori", TYPE_ERROR_FIXTURE);

    let out = run_emit_scip(dir.path(), "bad.ori", "index.scip");
    assert!(
        !out.status.success(),
        "emit-scip must exit non-zero on a type-error file"
    );
    assert!(
        !dir.path().join("index.scip").exists(),
        "no index.scip is written when the frontend errors"
    );
}

// Metadata + determinism pins

#[test]
fn emit_scip_index_carries_tool_metadata() {
    let index = emit_and_decode("greet.ori", SINGLE_FN_FIXTURE);
    let tool = &index.metadata.tool_info;
    assert_eq!(tool.name, "oric", "tool name identifies the emitter");
    assert!(
        !index.metadata.project_root.is_empty(),
        "project_root is recorded"
    );
    assert!(
        document(&index).relative_path.ends_with("greet.ori"),
        "document records the source path"
    );
}

#[test]
fn emit_scip_is_byte_deterministic() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    write_fixture(&dir, "multi.ori", MULTI_FIXTURE);

    assert!(run_emit_scip(dir.path(), "multi.ori", "first.scip")
        .status
        .success());
    assert!(run_emit_scip(dir.path(), "multi.ori", "second.scip")
        .status
        .success());

    let a =
        std::fs::read(dir.path().join("first.scip")).unwrap_or_else(|e| panic!("read first: {e}"));
    let b = std::fs::read(dir.path().join("second.scip"))
        .unwrap_or_else(|e| panic!("read second: {e}"));
    assert_eq!(a, b, "two emissions of the same source are byte-identical");
}
