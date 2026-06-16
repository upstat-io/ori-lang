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
const RESOLVED_CALL_FIXTURE: &str =
    include_str!("fixtures/emit_scip/resolved_call_occurrences.ori");
const AMBIGUOUS_METHOD_FIXTURE: &str = include_str!("fixtures/emit_scip/ambiguous_method.ori");
const NON_INDEXED_CALLS_FIXTURE: &str = include_str!("fixtures/emit_scip/non_indexed_calls.ori");
// Multi-line bodies so a Definition occurrence's identifier-token `range`
// (single-line) is crisply distinct from its body-spanning `enclosing_range`.
const CALLER_CALLEE_FIXTURE: &str = include_str!("fixtures/emit_scip/caller_callee.ori");
// Inherent + trait-impl `@render` on `Widget` both mint `Widget#render().` at
// distinct source sites — the dedup-distinct-site Definition-occurrence pin.
const SAME_SYMBOL_METHODS_FIXTURE: &str =
    include_str!("fixtures/emit_scip/same_symbol_methods.ori");

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

/// All occurrences of the single emitted document (every role).
fn occurrences(index: &Index) -> &[scip::types::Occurrence] {
    &document(index).occurrences
}

/// The REFERENCE-role (`ReadAccess`) occurrences only. The emitter now also emits
/// Definition-role occurrences per definition; the reference pins
/// assert on call-site references, so they filter to ReadAccess-role first.
fn reference_occurrences(index: &Index) -> Vec<&scip::types::Occurrence> {
    occurrences(index)
        .iter()
        .filter(|o| o.symbol_roles == ROLE_READ_ACCESS)
        .collect()
}

/// The (sorted) SCIP symbol strings of every REFERENCE occurrence (the
/// reference-only set — Definition occurrences are excluded).
fn occurrence_symbols(index: &Index) -> Vec<String> {
    let mut syms: Vec<String> = reference_occurrences(index)
        .iter()
        .map(|o| o.symbol.clone())
        .collect();
    syms.sort();
    syms
}

/// Count of REFERENCE occurrences carrying an exact symbol string.
fn occurrence_count(index: &Index, symbol: &str) -> usize {
    reference_occurrences(index)
        .iter()
        .filter(|o| o.symbol == symbol)
        .count()
}

// SCIP SymbolRole bits: Definition=1, ReadAccess=8 (a call READS the callable).
const ROLE_DEFINITION: i32 = 1;
const ROLE_READ_ACCESS: i32 = 8;

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

/// Regression pin: two top-level `greet` definitions in DIFFERENT source files
/// must mint distinct symbol strings (disambiguated by the module component),
/// so the exact-equality occurrence->definition join never collides on a
/// cross-file homonym.
#[test]
fn emit_scip_same_name_in_different_files_mints_distinct_symbols() {
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

// Default-impl method cell — `def impl Trait { @m }` methods are indexed

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

// Extension method cell — `extend Type { @m }` methods are indexed

#[test]
fn emit_scip_extend_emits_method_keyed_on_target_type() {
    // Regression pin: an extension method is keyed on its target type and is
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

// Reference-occurrence cells — trait-dispatch-resolved targets (accuracy core)

/// Positive semantic pin: a trait-method call emits an `Occurrence` whose
/// `symbol` is the CONCRETE impl method's SCIP symbol (`Rectangle#describe().`),
/// NOT the bare method name — the receiver's type-checker-resolved type
/// (`Rectangle`) selects the target, and the minted string is byte-identical to
/// the definition side so the occurrence->definition join lands on equality.
/// A free-function call emits the resolved `area().` symbol.
#[test]
fn emit_scip_method_call_occurrence_references_resolved_impl_symbol() {
    let index = emit_and_decode("resolved_call_occurrences.ori", RESOLVED_CALL_FIXTURE);

    let method_symbol = "ori . resolved_call_occurrences . Rectangle#describe().";
    let fn_symbol = "ori . resolved_call_occurrences . area().";

    let occ_syms = occurrence_symbols(&index);
    assert!(
        occ_syms.contains(&method_symbol.to_string()),
        "trait-method call occurrence references the concrete impl symbol \
         (resolved target, not bare `describe`): {occ_syms:?}"
    );
    assert!(
        occ_syms.contains(&fn_symbol.to_string()),
        "free-function call occurrence references the resolved function symbol: {occ_syms:?}"
    );

    // The emitted occurrence symbol is byte-identical to the definition the def
    // side minted for the same impl method — the join is exact-string equality.
    let def_syms = symbol_strings(&index);
    assert!(
        def_syms.contains(&method_symbol.to_string()),
        "the resolved occurrence symbol also appears as a definition (join lands): {def_syms:?}"
    );

    // Role bits: a reference carries ReadAccess and never Definition.
    let method_occ = reference_occurrences(&index)
        .into_iter()
        .find(|o| o.symbol == method_symbol)
        .unwrap_or_else(|| panic!("method-call occurrence present"));
    assert_eq!(
        method_occ.symbol_roles, ROLE_READ_ACCESS,
        "reference occurrence carries the ReadAccess role bit"
    );
    assert_eq!(
        method_occ.symbol_roles & ROLE_DEFINITION,
        0,
        "a reference occurrence must NOT carry the Definition role bit"
    );

    // The range carries the EXACT 0-based [start_line, start_col, end_line,
    // end_col] of the `r.describe()` call on fixture line 15 (0-based 14), at the
    // 4-space indent. Value-level pin (not just len==4): catches a line/col field
    // swap (start_line would be 4 not 14), a column off-by-one, and a start/end
    // swap (end_col must exceed start_col).
    assert_eq!(
        method_occ.range,
        vec![14, 4, 14, 16],
        "method-call occurrence range is the exact span of `r.describe()`"
    );

    // The free-function occurrence range is the callee-IDENTIFIER token (`area`)
    // on fixture line 14 (0-based 13), NOT the whole call expression — pins the
    // `free_call_occurrence` range == expr_span(func) contract.
    let area_occ = reference_occurrences(&index)
        .into_iter()
        .find(|o| o.symbol == fn_symbol)
        .unwrap_or_else(|| panic!("free-function-call occurrence present"));
    assert_eq!(
        area_occ.range,
        vec![13, 13, 13, 17],
        "free-function occurrence range is the exact span of the `area` callee token"
    );
}

/// Negative pin: two distinct types each declare a same-named method `speak`.
/// Each call site must resolve to its OWN receiver's impl symbol — the
/// `c.speak()` call references `Cat#speak().` and the `d.speak()` call
/// references `Dog#speak().`. A wrong-target resolver (e.g. one that keys on the
/// method name alone, or collapses to the first matching impl) would emit
/// `Cat#speak().` twice and zero `Dog#speak().`; this pin rejects that.
#[test]
fn emit_scip_ambiguous_method_name_resolves_to_correct_receiver_impl() {
    let index = emit_and_decode("ambiguous_method.ori", AMBIGUOUS_METHOD_FIXTURE);

    let cat_symbol = "ori . ambiguous_method . Cat#speak().";
    let dog_symbol = "ori . ambiguous_method . Dog#speak().";

    let occ_syms = occurrence_symbols(&index);
    assert!(
        occ_syms.contains(&cat_symbol.to_string()),
        "the `c.speak()` call resolves to Cat's impl: {occ_syms:?}"
    );
    assert!(
        occ_syms.contains(&dog_symbol.to_string()),
        "the `d.speak()` call resolves to Dog's impl (NOT collapsed to Cat): {occ_syms:?}"
    );

    // Exactly one occurrence per receiver type — proves neither call site was
    // mis-resolved to the other type's impl symbol.
    assert_eq!(
        occurrence_count(&index, cat_symbol),
        1,
        "exactly one Cat#speak() occurrence (the cat call site)"
    );
    assert_eq!(
        occurrence_count(&index, dog_symbol),
        1,
        "exactly one Dog#speak() occurrence (the dog call site)"
    );
}

/// Negative pin for the "absent beats wrong-target" contract — the two
/// occurrence None-drop paths must mint ZERO occurrences:
/// - a builtin-receiver method call (`items.len()` on `[int]`) — the receiver
///   has no user nominal type, so `resolved_receiver_type_name` returns `None`
///   and NO `<builtin>#len().` occurrence is minted (a dangling/wrong-target
///   occurrence would corrupt the join);
/// - a prelude free-function call (`len(collection:)`) — not in this module's
///   `local_fns`, so no occurrence is minted (prelude has no in-module def).
///
/// The occurrence walk still RAN (the local `helper(n:)` call mints its
/// occurrence), so the empty builtin/prelude result is an active exclusion, not
/// a dead walk. A resolver that keyed on the method name alone, or minted for
/// every callee, would leak a builtin-method or prelude-function occurrence;
/// this pin rejects that — the set is EXACTLY the one local call.
#[test]
fn emit_scip_builtin_and_prelude_calls_mint_no_occurrence() {
    let index = emit_and_decode("non_indexed_calls.ori", NON_INDEXED_CALLS_FIXTURE);

    assert_eq!(
        occurrence_symbols(&index),
        vec!["ori . non_indexed_calls . helper().".to_string()],
        "only the local free-function call mints an occurrence; the builtin-receiver \
         method call and the prelude free-function call mint none (absent beats wrong-target)"
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

// Definition-occurrence cells — the caller-attribution signal.
//
// The spec-correct SCIP convention (rust-analyzer / scip-clang / scip-java): a
// `Definition`-role occurrence's `range` is the definition's IDENTIFIER TOKEN
// (the name span), and the full definition extent (body included) lives in
// `enclosing_range`. The importer keys caller-attribution containment on
// `enclosing_range`, so the emitter MUST emit these per definition.

/// True iff `outer` (a `[sl, sc, el, ec]` range) fully contains `inner`.
fn range_contains(outer: &[i32], inner: &[i32]) -> bool {
    (outer[0], outer[1]) <= (inner[0], inner[1]) && (inner[2], inner[3]) <= (outer[2], outer[3])
}

/// The single `Definition`-role occurrence (`symbol_roles == SymbolRole::Definition`)
/// carrying `symbol`. Panics if absent (the pin: the emitter must emit it).
fn definition_occurrence<'a>(index: &'a Index, symbol: &str) -> &'a scip::types::Occurrence {
    occurrences(index)
        .iter()
        .find(|o| o.symbol == symbol && o.symbol_roles == ROLE_DEFINITION)
        .unwrap_or_else(|| panic!("a Definition-role occurrence for `{symbol}` is emitted"))
}

/// Positive L12 pin: on a two-function fixture (one calls the other), each
/// definition emits exactly one `Definition`-role occurrence whose `range` is
/// the identifier-token span and whose `enclosing_range` spans the full
/// (multi-line) definition extent. Regression pin: pre-fix the emitter emitted
/// NO Definition-role occurrences and never set `enclosing_range`.
#[test]
fn emit_scip_emits_definition_occurrence_with_identifier_range_and_body_enclosing_range() {
    let index = emit_and_decode("caller_callee.ori", CALLER_CALLEE_FIXTURE);

    let helper_sym = "ori . caller_callee . helper().";
    let run_sym = "ori . caller_callee . run().";

    // Exactly one Definition-role occurrence per emitted definition symbol.
    for sym in [helper_sym, run_sym] {
        let count = occurrences(&index)
            .iter()
            .filter(|o| o.symbol == sym && o.symbol_roles == ROLE_DEFINITION)
            .count();
        assert_eq!(
            count, 1,
            "exactly one Definition-role occurrence for `{sym}`"
        );
    }

    // `helper`: identifier token `helper` is cols 1..7 on line 0 (after `@`);
    // the body-spanning enclosing_range starts at the `@` (line 0, col 0) and
    // ends on the closing-brace line 2, fully containing the identifier range.
    let helper = definition_occurrence(&index, helper_sym);
    assert_eq!(
        helper.range,
        vec![0, 1, 0, 7],
        "Definition `range` is the `helper` identifier token, not the body"
    );
    assert_eq!(
        (helper.enclosing_range[0], helper.enclosing_range[1]),
        (0, 0),
        "enclosing_range starts at the definition's first line, column 0"
    );
    assert_eq!(
        helper.enclosing_range[2], 2,
        "enclosing_range ends on the closing-brace line (body-spanning)"
    );
    assert!(
        range_contains(&helper.enclosing_range, &helper.range),
        "enclosing_range contains the identifier-token range"
    );

    // `run`: identifier token `run` is cols 1..4 on line 4; enclosing_range
    // spans lines 4..6.
    let run = definition_occurrence(&index, run_sym);
    assert_eq!(
        run.range,
        vec![4, 1, 4, 4],
        "Definition `range` is the `run` identifier token, not the body"
    );
    assert_eq!((run.enclosing_range[0], run.enclosing_range[1]), (4, 0));
    assert_eq!(run.enclosing_range[2], 6);
    assert!(range_contains(&run.enclosing_range, &run.range));
}

/// Negative pin: the rejected pragmatic shape gave the Definition occurrence a
/// body-spanning `range`. The spec-correct convention keeps `range` as the
/// single-line identifier token and puts the body span in `enclosing_range` —
/// the two are never equal, and `range` never straddles lines. Regression pin:
/// pre-fix no Definition-role occurrence was emitted at all; also fails-closed
/// if a later implementation ships the body-spanning-`range` shortcut.
#[test]
fn emit_scip_definition_occurrence_range_is_identifier_token_not_body_spanning() {
    let index = emit_and_decode("caller_callee.ori", CALLER_CALLEE_FIXTURE);

    let defs: Vec<&scip::types::Occurrence> = occurrences(&index)
        .iter()
        .filter(|o| o.symbol_roles == ROLE_DEFINITION)
        .collect();
    assert!(
        !defs.is_empty(),
        "the emitter emits at least one Definition-role occurrence"
    );
    for o in defs {
        assert_ne!(
            o.range, o.enclosing_range,
            "Definition `range` (identifier token) is distinct from `enclosing_range` (body): {}",
            o.symbol
        );
        // The identifier-token range is single-line; a body-spanning range would
        // straddle lines (start_line != end_line) — the rejected shape.
        assert_eq!(
            o.range[0], o.range[2],
            "Definition `range` is the single-line identifier token, never body-spanning: {}",
            o.symbol
        );
        // enclosing_range proves the body span by straddling lines.
        assert!(
            o.enclosing_range[2] > o.enclosing_range[0],
            "enclosing_range spans the multi-line definition body: {}",
            o.symbol
        );
    }
}

/// Nesting pin: a method-in-impl emits its OWN `Definition`-role occurrence with
/// its own method-extent `enclosing_range`, distinct from the enclosing type's
/// Definition occurrence. Regression pin: pre-fix no Definition-role occurrences
/// were emitted.
#[test]
fn emit_scip_method_and_type_each_emit_distinct_definition_occurrences() {
    let index = emit_and_decode("impl.ori", IMPL_FIXTURE);

    let type_sym = "ori . impl . Point#";
    let method_sym = "ori . impl . Point#get_x().";

    // The type's Definition occurrence: `Point` identifier token on line 0
    // (`type ` is cols 0..5, `Point` cols 5..10).
    let type_def = definition_occurrence(&index, type_sym);
    assert_eq!(
        type_def.range,
        vec![0, 5, 0, 10],
        "type Definition `range` is the `Point` identifier token"
    );
    assert_eq!(
        (type_def.enclosing_range[0], type_def.enclosing_range[1]),
        (0, 0),
        "type enclosing_range starts at the `type Point` declaration"
    );
    assert!(range_contains(&type_def.enclosing_range, &type_def.range));

    // The method's OWN Definition occurrence: `get_x` identifier token on line 3
    // (4-space indent + `@`, so cols 5..10), with a method-extent enclosing_range
    // beginning on line 3 — distinct from the type's.
    let method_def = definition_occurrence(&index, method_sym);
    assert_eq!(
        method_def.range,
        vec![3, 5, 3, 10],
        "method Definition `range` is the `get_x` identifier token"
    );
    assert_eq!(
        method_def.enclosing_range[0], 3,
        "method enclosing_range is the method extent (line 3), not the type's"
    );
    assert!(range_contains(
        &method_def.enclosing_range,
        &method_def.range
    ));

    assert_ne!(method_def.range, type_def.range);
    assert_ne!(
        method_def.enclosing_range, type_def.enclosing_range,
        "method and type each carry a distinct enclosing_range"
    );
}

/// Coverage pin for non-function / non-method definition KINDS: `collect_defs`
/// mints `type_def` / `trait_def` / variant beyond fn + struct + method, and EACH
/// must emit a Definition-role occurrence with an identifier-token `range` +
/// body `enclosing_range`. Drives the multi-entity fixture (trait `Drawable`,
/// enum `Shape` + variant `Circle`). Regression pin: pre-fix no Definition
/// occurrences were emitted.
#[test]
fn emit_scip_trait_and_enum_definition_kinds_emit_definition_occurrences() {
    let index = emit_and_decode("multi.ori", MULTI_FIXTURE);

    // Trait declaration: `Drawable` identifier token on line 6 (`trait ` is cols
    // 0..6, `Drawable` cols 6..14), enclosing_range spans the trait body.
    let trait_def = definition_occurrence(&index, "ori . multi . Drawable#");
    assert_eq!(
        trait_def.range,
        vec![6, 6, 6, 14],
        "trait Definition `range` is the `Drawable` identifier token"
    );
    assert!(
        range_contains(&trait_def.enclosing_range, &trait_def.range),
        "trait enclosing_range contains its identifier token"
    );
    assert!(
        trait_def.enclosing_range[2] > trait_def.enclosing_range[0],
        "trait enclosing_range spans the multi-line trait body"
    );

    // Enum type declaration: `Shape` identifier token on line 4 (`type ` cols
    // 0..5, `Shape` cols 5..10).
    let enum_def = definition_occurrence(&index, "ori . multi . Shape#");
    assert_eq!(
        enum_def.range,
        vec![4, 5, 4, 10],
        "enum Definition `range` is the `Shape` identifier token"
    );
    assert!(range_contains(&enum_def.enclosing_range, &enum_def.range));
    assert_ne!(
        enum_def.range, enum_def.enclosing_range,
        "enum identifier-token range is distinct from its enclosing range"
    );

    // An enum VARIANT also emits a Definition occurrence: `Circle` on line 4.
    let variant_def = definition_occurrence(&index, "ori . multi . Shape#Circle.");
    assert_eq!(
        variant_def.range,
        vec![4, 13, 4, 19],
        "variant Definition `range` is the `Circle` identifier token"
    );
    assert!(range_contains(
        &variant_def.enclosing_range,
        &variant_def.range
    ));
}

/// Dedup-distinct-site pin: an inherent `@render` and a trait-impl `@render` on
/// `Widget` mint the SAME `Widget#render().` symbol at two distinct source
/// sites. The symbol table dedups to ONE `SymbolInformation`, but EACH site
/// MUST emit its own Definition-role occurrence (guards `mod.rs` `defs.dedup_by`
/// from collapsing per-site occurrences). Regression pin: pre-fix no Definition
/// occurrences were emitted at all.
#[test]
fn emit_scip_same_symbol_methods_at_distinct_sites_each_emit_a_definition_occurrence() {
    let index = emit_and_decode("same_symbol_methods.ori", SAME_SYMBOL_METHODS_FIXTURE);

    let widget_render = "ori . same_symbol_methods . Widget#render().";

    // Exactly ONE SymbolInformation for the deduped symbol.
    assert_eq!(
        symbol_strings(&index)
            .iter()
            .filter(|s| *s == widget_render)
            .count(),
        1,
        "the symbol table carries exactly one `Widget#render().` SymbolInformation (deduped)"
    );

    // But TWO Definition-role occurrences — one per distinct source site.
    let defs: Vec<&scip::types::Occurrence> = occurrences(&index)
        .iter()
        .filter(|o| o.symbol == widget_render && o.symbol_roles == ROLE_DEFINITION)
        .collect();
    assert_eq!(
        defs.len(),
        2,
        "each distinct `@render` site (inherent + trait-impl) emits its own \
         Definition occurrence — the dedup must NOT collapse per-site occurrences"
    );

    // The two sites carry DISTINCT enclosing ranges (different source extents).
    assert_ne!(
        defs[0].enclosing_range, defs[1].enclosing_range,
        "the two Definition occurrences span distinct source extents"
    );
    for o in &defs {
        assert_eq!(
            o.range[0], o.range[2],
            "each Definition `range` is the single-line `render` identifier token"
        );
        assert!(range_contains(&o.enclosing_range, &o.range));
    }
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
