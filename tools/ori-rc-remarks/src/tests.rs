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
fn ingest_parses_header_fields() {
    let stream = ingest(SAMPLE).unwrap_or_else(|e| panic!("ingest failed: {e}"));
    let Some(header) = &stream.header else {
        panic!("expected a stream header");
    };
    assert_eq!(header.schema_version, 1, "schema_version");
    assert_eq!(header.compiler_sha, "000de0232", "compiler_sha");
    assert_eq!(header.source_file, "survivor.ori", "source_file");
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
    assert_eq!(err.line(), 3, "error line is the malformed line (1-based)");
}

/// A schema-2 header: the legacy path-label field is absent.
const HEADER_V2: &str = concat!(
    r#"{"record":"header","schema_version":2,"compiler_sha":"000de0232","source_file":"survivor.ori"}"#,
    "\n",
);

/// A header declaring a version this analyzer does not understand.
const HEADER_UNSUPPORTED: &str = concat!(
    r#"{"record":"header","schema_version":999,"compiler_sha":"000de0232","source_file":"survivor.ori"}"#,
    "\n",
);

#[test]
fn ingest_accepts_legacy_schema_v1() {
    // A v1 stream still carries the retired path-label field. Ingest ignores
    // the extra field rather than rejecting the stream.
    let stream = ingest(SAMPLE).unwrap_or_else(|e| panic!("v1 ingest failed: {e}"));
    let Some(header) = &stream.header else {
        panic!("expected a stream header");
    };
    assert_eq!(header.schema_version, 1, "schema_version");
    assert_eq!(stream.remarks.len(), 1, "v1 remarks still ingest");
}

#[test]
fn ingest_accepts_schema_v2() {
    let stream = ingest(HEADER_V2).unwrap_or_else(|e| panic!("v2 ingest failed: {e}"));
    let Some(header) = &stream.header else {
        panic!("expected a stream header");
    };
    assert_eq!(header.schema_version, 2, "schema_version");
}

#[test]
fn ingest_rejects_unsupported_schema_version() {
    // Negative pin: an unknown schema is REJECTED, never analyzed under the
    // current version's semantics. Analysis of a stream whose shape is not
    // understood yields a confident wrong verdict, which is worse than an error.
    let Err(err) = ingest(HEADER_UNSUPPORTED) else {
        panic!("an unsupported schema version must be rejected, not analyzed");
    };
    assert_eq!(err.line(), 1, "the header line is the rejection site");

    let rendered = err.to_string();
    assert!(
        rendered.contains("999"),
        "the message names the version found\nmessage: {rendered}"
    );
    assert!(
        rendered.contains('1') && rendered.contains('2'),
        "the message names the supported versions\nmessage: {rendered}"
    );
    assert!(
        rendered.contains("rebuild") || rendered.contains("regenerate"),
        "the message names an action the reader can take\nmessage: {rendered}"
    );
}

#[test]
fn ingest_refuses_a_remark_with_no_preceding_header() {
    // This assertion was INVERTED deliberately. It previously admitted a
    // headerless stream and let the analysis rest on an assumed generation,
    // which is the version gate reached by OMITTING a header instead of
    // declaring a bad one -- the same fail-open, one spelling out.
    // "No version claim" is not a weaker claim than a wrong one; it is an
    // unchecked one, so it refuses.
    let headerless = SAMPLE
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("SAMPLE has a remark line"));
    let Err(err) = ingest(headerless) else {
        panic!("an unversioned remark must be refused, not analyzed on an assumption");
    };
    assert_eq!(err.line(), 1, "refusal names the offending line");
}

#[test]
fn ingest_accepts_a_header_only_stream() {
    // The refusal above is scoped to REMARKS. A header with no remarks is a
    // legitimate empty capture and must still ingest, or the gate would turn
    // "nothing to report" into an error.
    let header_only = SAMPLE
        .lines()
        .next()
        .unwrap_or_else(|| panic!("SAMPLE has a header line"));
    let stream =
        ingest(header_only).unwrap_or_else(|e| panic!("header-only ingest failed: {e}"));
    assert!(stream.header.is_some(), "header claimed");
    assert!(stream.remarks.is_empty(), "no remarks");
}

#[test]
fn ingest_rejects_unsupported_version_before_reading_remarks() {
    // The gate sits at the ingest boundary, so no consumer (summary, stats,
    // view, diff) can reach remark data from an unsupported stream.
    let with_remark = format!(
        "{HEADER_UNSUPPORTED}{}",
        SAMPLE.lines().nth(1).unwrap_or_else(|| panic!("SAMPLE has a remark line"))
    );
    let Err(_) = ingest(&with_remark) else {
        panic!("an unsupported schema must be rejected even when remarks parse");
    };
}
