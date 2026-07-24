//! Ingest tests for the RC-remark JSONL stream.

use super::{ingest, Stream};

/// A representative two-line stream: header + one missed remark, matching the
/// exact wire shape `oric --emit-rc-remarks` produces.
const SAMPLE: &str = concat!(
    r#"{"record":"header","schema_version":1,"compiler_sha":"000de0232","source_file":"survivor.ori","burden_path":true}"#,
    "\n",
    r#"{"kind":"missed","pass":"aims-burden-elim","name":"rc-inc-not-elided","rc_op":"burden_inc","function":"main","debug_loc":null,"ssa_value":3,"exit_block":null,"cause":{"proof_failure":"burden_inc_kept_by_whole_var_disposition","lattice_dim":"locality","detail":"consumption=Affine locality=FunctionLocal cardinality=Once"},"burden_net":null,"args":["def_kind=let-literal","var_repr=fat-value","span=515:565"],"cow_mode":null}"#,
    "\n",
);

fn lookup(stream: &Stream) -> &super::Remark {
    let Some(remark) = stream.remarks.first() else {
        panic!("expected >=1 ingested remark");
    };
    remark
}

#[test]
fn ingest_parses_header_burden_path_true() {
    let stream = ingest(SAMPLE).unwrap_or_else(|e| panic!("ingest failed: {e}"));
    let Some(header) = &stream.header else {
        panic!("expected a stream header");
    };
    assert_eq!(header.schema_version, 1, "schema_version");
    assert_eq!(header.compiler_sha, "000de0232", "compiler_sha");
    assert_eq!(header.source_file, "survivor.ori", "source_file");
    assert!(header.burden_path, "burden_path must parse true");
}

#[test]
fn ingest_parses_missed_remark_fields() {
    let stream = ingest(SAMPLE).unwrap_or_else(|e| panic!("ingest failed: {e}"));
    assert_eq!(stream.remarks.len(), 1, "exactly one remark");
    let remark = lookup(&stream);
    assert_eq!(remark.kind, "missed", "kind");
    assert_eq!(remark.rc_op, "burden_inc", "rc_op");
    assert_eq!(remark.function.as_deref(), Some("main"), "function attribution");
    assert_eq!(remark.ssa_value, Some(3), "ssa_value");
    assert!(remark.debug_loc.is_none(), "synthetic op has no debug_loc");
}

#[test]
fn ingest_parses_cause_and_args() {
    let stream = ingest(SAMPLE).unwrap_or_else(|e| panic!("ingest failed: {e}"));
    let remark = lookup(&stream);
    let Some(cause) = &remark.cause else {
        panic!("expected a cause");
    };
    assert_eq!(cause.lattice_dim.as_deref(), Some("locality"), "lattice_dim");
    assert!(
        remark.args.iter().any(|a| a.starts_with("span=")),
        "carried span arg must survive ingest: {:?}",
        remark.args
    );
}

#[test]
fn ingest_skips_blank_lines() {
    let with_blanks = format!("\n{SAMPLE}\n\n");
    let stream = ingest(&with_blanks).unwrap_or_else(|e| panic!("ingest failed: {e}"));
    assert!(stream.header.is_some(), "header survives blank-line padding");
    assert_eq!(stream.remarks.len(), 1, "blank lines do not add remarks");
}

#[test]
fn ingest_empty_stream_yields_empty() {
    let stream = ingest("").unwrap_or_else(|e| panic!("ingest failed: {e}"));
    assert!(stream.header.is_none(), "empty stream has no header");
    assert!(stream.remarks.is_empty(), "empty stream has no remarks");
}

#[test]
fn ingest_reports_line_on_malformed_json() {
    // Negative pin: a malformed third line reports line 3, not a silent skip.
    let malformed = format!("{SAMPLE}{{not json\n");
    let Err(err) = ingest(&malformed) else {
        panic!("malformed JSON must error");
    };
    assert_eq!(err.line, 3, "error line is the malformed line (1-based)");
}
