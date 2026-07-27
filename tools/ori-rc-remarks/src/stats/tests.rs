//! Tests for cause-cluster ranking.

use crate::{ingest, Stream};

use super::rank_cause_clusters;

/// Build a remark line with a given lattice dim + proof failure.
fn remark(dim: &str, proof_failure: &str) -> String {
    format!(
        r#"{{"kind":"missed","pass":"aims-burden-elim","name":"rc-inc-not-elided","rc_op":"burden_inc","function":"f","debug_loc":null,"ssa_value":1,"exit_block":null,"cause":{{"proof_failure":"{proof_failure}","lattice_dim":"{dim}","detail":null}},"burden_net":null,"args":[],"cow_mode":null}}"#
    )
}

/// A remark with no cause object at all.
const NO_CAUSE: &str = r#"{"kind":"missed","pass":"p","name":"n","rc_op":"burden_dec","function":"g","debug_loc":null,"ssa_value":2,"exit_block":null,"cause":null,"burden_net":null,"args":[],"cow_mode":null}"#;

/// Every fixture carries a current-generation header; ingest refuses remarks without one.
const HEADER: &str = r#"{"record":"header","schema_version":2,"compiler_sha":"000de0232","source_file":"t.ori"}"#;

fn build(lines: &[String]) -> Stream {
    let body = format!("{HEADER}\n{}", lines.join("\n"));
    ingest(&body).unwrap_or_else(|e| panic!("ingest failed: {e}"))
}

#[test]
fn ranks_largest_cluster_first() {
    // 3 locality/PF_A, 1 uniqueness/PF_B → PF_A cluster ranks first.
    let stream = build(&[
        remark("locality", "PF_A"),
        remark("locality", "PF_A"),
        remark("locality", "PF_A"),
        remark("uniqueness", "PF_B"),
    ]);
    let clusters = rank_cause_clusters(&stream);
    assert_eq!(clusters.len(), 2, "two distinct cause clusters");
    assert_eq!(clusters[0].lattice_dim, "locality", "biggest cluster's dim");
    assert_eq!(clusters[0].proof_failure, "PF_A", "biggest cluster's proof failure");
    assert_eq!(clusters[0].count, 3, "biggest cluster count");
    assert_eq!(clusters[1].count, 1, "smaller cluster count");
}

#[test]
fn no_cause_remarks_cluster_under_none_sentinel() {
    let stream = build(&[NO_CAUSE.to_string(), NO_CAUSE.to_string()]);
    let clusters = rank_cause_clusters(&stream);
    assert_eq!(clusters.len(), 1, "no-cause remarks form one sentinel cluster");
    assert_eq!(clusters[0].lattice_dim, "<none>", "sentinel dim");
    assert_eq!(clusters[0].proof_failure, "<none>", "sentinel proof failure");
    assert_eq!(clusters[0].count, 2, "both no-cause remarks counted");
}

#[test]
fn equal_count_clusters_break_ties_deterministically() {
    // Two 1-count clusters → stable order by (dim, proof_failure) ascending.
    let stream = build(&[remark("uniqueness", "PF_Z"), remark("locality", "PF_A")]);
    let first = rank_cause_clusters(&stream);
    let second = rank_cause_clusters(&stream);
    assert_eq!(first, second, "ranking is deterministic across runs");
    assert_eq!(first[0].lattice_dim, "locality", "tie-break orders dim ascending");
}

#[test]
fn empty_stream_yields_no_clusters() {
    let clusters = rank_cause_clusters(&Stream::default());
    assert!(clusters.is_empty(), "no remarks → no clusters");
}
