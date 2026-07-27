//! Tests for the source-annotated survivor view.

use crate::{ingest, Stream};

use super::source_annotated_view;

/// Every fixture carries a current-generation header; ingest refuses remarks without one.
const HEADER: &str = r#"{"record":"header","schema_version":2,"compiler_sha":"000de0232","source_file":"t.ori"}"#;

/// A span-less remark (synthetic op) attributed to `function`.
fn fn_remark(function: &str, ssa: u64) -> String {
    format!(
        r#"{{"kind":"missed","pass":"aims-burden-elim","name":"rc-inc-not-elided","rc_op":"burden_inc","function":"{function}","debug_loc":null,"ssa_value":{ssa},"exit_block":null,"cause":{{"proof_failure":"PF","lattice_dim":"locality","detail":null}},"burden_net":null,"args":[],"cow_mode":null}}"#
    )
}

/// A remark with a resolved `debug_loc`.
fn loc_remark(file: &str, line: u32, column: u32) -> String {
    format!(
        r#"{{"kind":"missed","pass":"p","name":"n","rc_op":"burden_dec","function":"h","debug_loc":{{"file":"{file}","line":{line},"column":{column}}},"ssa_value":7,"exit_block":null,"cause":null,"burden_net":null,"args":[],"cow_mode":null}}"#
    )
}

fn build(lines: &[String]) -> Stream {
    ingest(&format!("{HEADER}\n{}", lines.join("\n")))
        .unwrap_or_else(|e| panic!("ingest failed: {e}"))
}

#[test]
fn span_less_remarks_group_by_function() {
    // 2 in f, 1 in g → two function-level groups, location-sorted (f before g).
    let stream = build(&[fn_remark("f", 1), fn_remark("g", 2), fn_remark("f", 3)]);
    let groups = source_annotated_view(&stream);
    assert_eq!(groups.len(), 2, "two function groups");
    assert_eq!(groups[0].location, "fn f", "groups sorted: f first");
    assert_eq!(groups[0].survivors.len(), 2, "f has two survivors");
    assert_eq!(groups[1].location, "fn g", "g second");
    assert_eq!(groups[1].survivors.len(), 1, "g has one survivor");
}

#[test]
fn resolved_span_uses_file_line_column_location() {
    let stream = build(&[loc_remark("survivor.ori", 12, 5)]);
    let groups = source_annotated_view(&stream);
    assert_eq!(groups.len(), 1, "one location group");
    assert_eq!(
        groups[0].location, "survivor.ori:12:5",
        "resolved span → file:line:column location"
    );
}

#[test]
fn survivor_carries_cause_attribution() {
    let stream = build(&[fn_remark("f", 1)]);
    let groups = source_annotated_view(&stream);
    let survivor = &groups[0].survivors[0];
    assert_eq!(survivor.rc_op, "burden_inc", "rc_op carried");
    assert_eq!(survivor.lattice_dim.as_deref(), Some("locality"), "lattice dim carried");
    assert_eq!(survivor.proof_failure.as_deref(), Some("PF"), "proof failure carried");
}

#[test]
fn view_is_deterministic() {
    let stream = build(&[fn_remark("z", 1), fn_remark("a", 2)]);
    assert_eq!(
        source_annotated_view(&stream),
        source_annotated_view(&stream),
        "view is stable across runs"
    );
}

#[test]
fn empty_stream_yields_no_groups() {
    assert!(
        source_annotated_view(&Stream::default()).is_empty(),
        "no remarks → no groups"
    );
}
