//! L12 production-path pins for `ori emit-scip` (full SCIP definition index).
//!
//! Spawns the REAL `ori` binary against temp `.ori` fixtures and decodes the
//! emitted `index.scip` back through the `scip` protobuf crate, asserting the
//! `SymbolInformation` set, the exact (globally-stable) SCIP symbol strings,
//! and the kinds. The CLI-handler layer is otherwise unit-untested, so these
//! end-to-end pins are the load-bearing coverage for the emit pipe.

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
const MULTI_FIXTURE: &str = include_str!("fixtures/emit_scip/multi.ori");
const EMPTY_FIXTURE: &str = include_str!("fixtures/emit_scip/empty.ori");
const TYPE_ERROR_FIXTURE: &str = include_str!("fixtures/emit_scip/type_error.ori");

fn write_fixture(dir: &tempfile::TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap_or_else(|e| panic!("failed to write fixture: {e}"));
    path
}

fn run_emit_scip(fixture: &Path, output: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ori"))
        .arg("emit-scip")
        .arg(fixture)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn ori emit-scip: {e}"))
}

/// Drive the real CLI on `content` and decode the emitted `index.scip` back
/// through the `scip` protobuf crate. Asserts a clean exit.
fn emit_and_decode(name: &str, content: &str) -> Index {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, name, content);
    let output = dir.path().join("index.scip");

    let out = run_emit_scip(&fixture, &output);
    assert!(
        out.status.success(),
        "emit-scip exited non-zero: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let bytes = std::fs::read(&output).unwrap_or_else(|e| panic!("index.scip written: {e}"));
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

// ---------------------------------------------------------------------------
// Function cell
// ---------------------------------------------------------------------------

#[test]
fn emit_scip_function_mints_method_descriptor() {
    let index = emit_and_decode("greet.ori", SINGLE_FN_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec!["ori . . . greet().".to_string()],
        "a single function emits exactly one Method-descriptor symbol"
    );
    let greet = find(&index, "ori . . . greet().");
    assert_eq!(greet.display_name, "greet");
    assert_eq!(kind(greet), Kind::Function);
}

#[test]
fn emit_scip_emits_every_function_not_just_the_first() {
    // Negative pin against the walking-skeleton behavior (first-function-only).
    let index = emit_and_decode("two.ori", TWO_FN_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec![
            "ori . . . add().".to_string(),
            "ori . . . zebra().".to_string(),
        ],
        "both functions are indexed, sorted by symbol string"
    );
}

// ---------------------------------------------------------------------------
// Struct type + fields cell
// ---------------------------------------------------------------------------

#[test]
fn emit_scip_struct_emits_type_and_fields() {
    let index = emit_and_decode("point.ori", STRUCT_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec![
            "ori . . . Point#".to_string(),
            "ori . . . Point#x.".to_string(),
            "ori . . . Point#y.".to_string(),
        ],
    );
    assert_eq!(kind(find(&index, "ori . . . Point#")), Kind::Struct);
    let field_x = find(&index, "ori . . . Point#x.");
    assert_eq!(field_x.display_name, "x");
    assert_eq!(kind(field_x), Kind::Field);
}

// ---------------------------------------------------------------------------
// Sum type + variants + variant fields cell
// ---------------------------------------------------------------------------

#[test]
fn emit_scip_sum_emits_enum_variants_and_variant_fields() {
    let index = emit_and_decode("color.ori", SUM_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec![
            "ori . . . Color#".to_string(),
            "ori . . . Color#Blue.".to_string(),
            "ori . . . Color#Custom.".to_string(),
            "ori . . . Color#Custom.b.".to_string(),
            "ori . . . Color#Custom.g.".to_string(),
            "ori . . . Color#Custom.r.".to_string(),
            "ori . . . Color#Green.".to_string(),
            "ori . . . Color#Red.".to_string(),
        ],
    );
    assert_eq!(kind(find(&index, "ori . . . Color#")), Kind::Enum);
    assert_eq!(kind(find(&index, "ori . . . Color#Red.")), Kind::EnumMember);
    let variant_field = find(&index, "ori . . . Color#Custom.r.");
    assert_eq!(variant_field.display_name, "r");
    assert_eq!(kind(variant_field), Kind::Field);
}

// ---------------------------------------------------------------------------
// Trait + trait-method cell
// ---------------------------------------------------------------------------

#[test]
fn emit_scip_trait_emits_trait_and_methods() {
    let index = emit_and_decode("traits.ori", TRAITS_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec![
            "ori . . . Describable#".to_string(),
            "ori . . . Describable#describe().".to_string(),
            "ori . . . Measurable#".to_string(),
            "ori . . . Measurable#measure().".to_string(),
        ],
    );
    assert_eq!(kind(find(&index, "ori . . . Describable#")), Kind::Trait);
    assert_eq!(
        kind(find(&index, "ori . . . Describable#describe().")),
        Kind::Method
    );
}

// ---------------------------------------------------------------------------
// Inherent impl method cell
// ---------------------------------------------------------------------------

#[test]
fn emit_scip_inherent_impl_emits_methods() {
    let index = emit_and_decode("impl.ori", IMPL_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec![
            "ori . . . Point#".to_string(),
            "ori . . . Point#get_x().".to_string(),
            "ori . . . Point#sum().".to_string(),
            "ori . . . Point#x.".to_string(),
            "ori . . . Point#y.".to_string(),
        ],
    );
    let method = find(&index, "ori . . . Point#get_x().");
    assert_eq!(method.display_name, "get_x");
    assert_eq!(kind(method), Kind::Method);
}

// ---------------------------------------------------------------------------
// Trait-impl method cell — trait-method and impl-method are DISTINCT symbols
// ---------------------------------------------------------------------------

#[test]
fn emit_scip_trait_impl_emits_distinct_method_symbols() {
    let index = emit_and_decode("trait_impl.ori", TRAIT_IMPL_FIXTURE);
    let syms = symbol_strings(&index);
    // The trait's `describe` declaration and the impl's `describe` definition
    // mint distinct symbols keyed on their respective parent (trait vs type).
    assert!(
        syms.contains(&"ori . . . Describable#describe().".to_string()),
        "trait-method symbol present: {syms:?}"
    );
    assert!(
        syms.contains(&"ori . . . Rectangle#describe().".to_string()),
        "impl-method symbol present: {syms:?}"
    );
    assert_eq!(
        kind(find(&index, "ori . . . Rectangle#describe().")),
        Kind::Method
    );
}

// ---------------------------------------------------------------------------
// Multi-entity cell — every kind in one file, full deterministic set
// ---------------------------------------------------------------------------

#[test]
fn emit_scip_multi_entity_emits_full_sorted_set() {
    let index = emit_and_decode("multi.ori", MULTI_FIXTURE);
    assert_eq!(
        symbol_strings(&index),
        vec![
            "ori . . . Drawable#".to_string(),
            "ori . . . Drawable#draw().".to_string(),
            "ori . . . Point#".to_string(),
            "ori . . . Point#draw().".to_string(),
            "ori . . . Point#magnitude().".to_string(),
            "ori . . . Point#x.".to_string(),
            "ori . . . Point#y.".to_string(),
            "ori . . . Shape#".to_string(),
            "ori . . . Shape#Circle.".to_string(),
            "ori . . . Shape#Circle.radius.".to_string(),
            "ori . . . Shape#Square.".to_string(),
            "ori . . . area().".to_string(),
        ],
        "full definition inventory, deterministically sorted by symbol string"
    );
}

// ---------------------------------------------------------------------------
// Negative / edge cells
// ---------------------------------------------------------------------------

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
    let fixture = write_fixture(&dir, "bad.ori", TYPE_ERROR_FIXTURE);
    let output = dir.path().join("index.scip");

    let out = run_emit_scip(&fixture, &output);
    assert!(
        !out.status.success(),
        "emit-scip must exit non-zero on a type-error file"
    );
    assert!(
        !output.exists(),
        "no index.scip is written when the frontend errors"
    );
}

// ---------------------------------------------------------------------------
// Metadata + determinism pins
// ---------------------------------------------------------------------------

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
    let fixture = write_fixture(&dir, "multi.ori", MULTI_FIXTURE);
    let first = dir.path().join("first.scip");
    let second = dir.path().join("second.scip");

    assert!(run_emit_scip(&fixture, &first).status.success());
    assert!(run_emit_scip(&fixture, &second).status.success());

    let a = std::fs::read(&first).unwrap_or_else(|e| panic!("read first: {e}"));
    let b = std::fs::read(&second).unwrap_or_else(|e| panic!("read second: {e}"));
    assert_eq!(a, b, "two emissions of the same source are byte-identical");
}
