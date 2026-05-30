//! locality-conformance — §17 locality shipped-conformance cross-walk.
//!
//! Reads aims-proof/checker/shipped-conformance-manifest.json + build-time
//! probe consts (`LATTICE_VARIANT_COUNT`, `MAY_ESCAPE_IS_STORED`) and emits
//! a per-manifest-rule verdict block to stdout (canonical JSON). Per
//! `the locality shipped-conformance gate`
//! SC1 + SC5.
//!
//! Verdict rule (per rule):
//! variants == 5 AND !may_escape_stored -> `pass`
//! variants == 4 OR may_escape_stored -> `divergent-pending`
//!
//! Manifest-free divergence (a SHIPPED rule path drifts AWAY from the
//! manifest set without §17 update) -> fatal `divergent`. Detection
//! today: if probe surface itself parses but yields an unexpected
//! (variants, stored) tuple outside {(4,true), (4,false), (5,true),
//! (5,false)}. Build.rs panics on truly absent surface; that is the
//! `probe_compile_failure` path owned by the calling shell runner.
//!
//! Exit codes:
//! 0 — all manifest rules emit `pass`
//! 0 — all manifest rules emit `divergent-pending` (non-fatal under
//! Lazy-Migration Policy per §17)
//! 2 — any rule emits fatal `divergent`
//! 3 — manifest read / parse failure (manifest_invalid)
//!
//! Env overrides (verdict-boundary tests + conftest fixture):
//! ORI_LOCALITY_PROBE_VARIANT_COUNT=<usize> — override variant count
//! ORI_LOCALITY_PROBE_MAY_ESCAPE_STORED=<0|1> — override stored flag

#![deny(unsafe_code)]
#![allow(missing_docs)]

use aims_proof_checker::shipped_lattice_probe::{
    LATTICE_VARIANT_COUNT, MAY_ESCAPE_IS_STORED,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let workspace_root = locate_workspace_root();
    let manifest_path =
        workspace_root.join("aims-proof/checker/shipped-conformance-manifest.json");

    let manifest_src = match fs::read_to_string(&manifest_path) {
        Ok(s) => s,
        Err(e) => {
            print_error_envelope(
                "manifest_invalid",
                &format!("cannot read {}: {}", manifest_path.display(), e),
            );
            return ExitCode::from(3);
        }
    };

    let rules = match parse_manifest_rules(&manifest_src) {
        Ok(r) => r,
        Err(e) => {
            print_error_envelope("manifest_invalid", &e);
            return ExitCode::from(3);
        }
    };

    let (variant_count, may_escape_stored) = read_probe_with_overrides();

    let (verdict, exit_reason, rule_rows) =
        classify(variant_count, may_escape_stored, &rules);

    println!(
        "{}",
        render_envelope(&verdict, &exit_reason, variant_count, may_escape_stored, &rule_rows)
    );

    match verdict.as_str() {
        "pass" | "divergent_pending" => ExitCode::from(0),
        "fatal_divergent" => ExitCode::from(2),
        _ => ExitCode::from(2),
    }
}

#[derive(Debug, Clone)]
struct ManifestRule {
    id: String,
    blocked_by: String,
}

#[derive(Debug, Clone)]
struct RuleRow {
    rule_id: String,
    conformance: String,
}

fn parse_manifest_rules(src: &str) -> Result<Vec<ManifestRule>, String> {
    // Minimal hand-rolled JSON extraction — avoids pulling serde into the
    // trusted kernel surface (per Cargo.toml dependency policy).
    // Extracts each rule's `id` + `blocked_by` from the `rules:` array.
    let rules_start = src
        .find("\"rules\"")
        .ok_or("manifest missing required `rules` array")?;
    let array_start = src[rules_start..]
        .find('[')
        .ok_or("manifest `rules` not an array")?
        + rules_start;

    let mut rules = Vec::new();
    let mut depth = 0_i32;
    let mut obj_start: Option<usize> = None;
    for (i, ch) in src[array_start..].char_indices() {
        let abs = array_start + i;
        match ch {
            '{' => {
                if depth == 0 {
                    obj_start = Some(abs);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = obj_start.take() {
                        let body = &src[start..=abs];
                        let id = extract_field(body, "id")
                            .ok_or("rule entry missing `id`")?;
                        let blocked_by = extract_field(body, "blocked_by")
                            .ok_or("rule entry missing `blocked_by`")?;
                        rules.push(ManifestRule { id, blocked_by });
                    }
                }
            }
            ']' if depth == 0 => break,
            _ => {}
        }
    }
    if rules.is_empty() {
        return Err("manifest `rules` array is empty".into());
    }
    Ok(rules)
}

fn extract_field(obj_body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let key_pos = obj_body.find(&needle)?;
    let after = &obj_body[key_pos + needle.len()..];
    let colon = after.find(':')?;
    let after_colon = &after[colon + 1..];
    let first_quote = after_colon.find('"')?;
    let value_start = first_quote + 1;
    let rest = &after_colon[value_start..];
    let close = rest.find('"')?;
    Some(rest[..close].to_string())
}

fn read_probe_with_overrides() -> (usize, bool) {
    let variant_count = env::var("ORI_LOCALITY_PROBE_VARIANT_COUNT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(LATTICE_VARIANT_COUNT);
    let may_escape_stored = env::var("ORI_LOCALITY_PROBE_MAY_ESCAPE_STORED")
        .ok()
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(MAY_ESCAPE_IS_STORED);
    (variant_count, may_escape_stored)
}

fn classify(
    variant_count: usize,
    may_escape_stored: bool,
    rules: &[ManifestRule],
) -> (String, String, Vec<RuleRow>) {
    // Manifest-free divergence detection: a probe tuple outside the four
    // canonical shapes signals shipped surface drift beyond what §17
    // tracks. Today: only (4,true), (4,false), (5,true), (5,false) are
    // recognized; anything else routes to fatal divergent so §17 amends
    // the manifest or surfaces for manual fix.
    let recognized_shape = matches!(
        (variant_count, may_escape_stored),
        (4, true) | (4, false) | (5, true) | (5, false)
    );

    if !recognized_shape {
        let rows = rules
            .iter()
            .map(|r| RuleRow {
                rule_id: r.id.clone(),
                conformance: "divergent".to_string(),
            })
            .collect();
        return (
            "fatal_divergent".to_string(),
            "locality_conformance_fatal_divergent".to_string(),
            rows,
        );
    }

    if variant_count == 5 && !may_escape_stored {
        let rows = rules
            .iter()
            .map(|r| RuleRow {
                rule_id: r.id.clone(),
                conformance: "pass".to_string(),
            })
            .collect();
        (
            "pass".to_string(),
            "locality_conformance_pass".to_string(),
            rows,
        )
    } else {
        let rows = rules
            .iter()
            .map(|r| RuleRow {
                rule_id: r.id.clone(),
                conformance: format!("divergent-pending(blocked-by: {})", r.blocked_by),
            })
            .collect();
        (
            "divergent_pending".to_string(),
            "locality_conformance_divergent_pending".to_string(),
            rows,
        )
    }
}

fn render_envelope(
    verdict: &str,
    exit_reason: &str,
    variant_count: usize,
    may_escape_stored: bool,
    rule_rows: &[RuleRow],
) -> String {
    let autopilot_routable = matches!(verdict, "pass" | "divergent_pending");
    let mut rows_json = String::new();
    for (i, r) in rule_rows.iter().enumerate() {
        if i > 0 {
            rows_json.push_str(",\n ");
        }
        rows_json.push_str(&format!(
            "{{\"rule_id\": \"{}\", \"prior\": \"unknown\", \"current\": \"{}\"}}",
            r.rule_id,
            r.conformance.replace('"', "\\\"")
        ));
    }
    format!(
        "{{\n \"verdict\": \"{verdict}\",\n \"exit_reason\": \"{exit_reason}\",\n \
         \"autopilot_routable\": {autopilot_routable},\n \"probe_tuple\": \
         {{\"lattice_variant_count\": {variant_count}, \"may_escape_is_stored\": {may_escape_stored}}},\n \
         \"rule_verdicts\": [\n {rows_json}\n ]\n}}"
    )
}

fn print_error_envelope(exit_reason: &str, message: &str) {
    println!(
        "{{\n \"verdict\": \"{exit_reason}\",\n \"exit_reason\": \"{exit_reason}\",\n \
         \"autopilot_routable\": false,\n \"error\": \"{}\"\n}}",
        message.replace('"', "\\\"")
    );
}

fn locate_workspace_root() -> PathBuf {
    if let Ok(cwd) = env::current_dir() {
        let mut cur = cwd;
        loop {
            if cur.join("aims-proof").is_dir() && cur.join("compiler/ori_arc").is_dir() {
                return cur;
            }
            if !cur.pop() {
                break;
            }
        }
    }
    // Fall back to manifest-dir walk.
    let manifest = env!("CARGO_MANIFEST_DIR");
    let mut cur = PathBuf::from(manifest);
    loop {
        if cur.join("aims-proof").is_dir() && cur.join("compiler/ori_arc").is_dir() {
            return cur;
        }
        if !cur.pop() {
            panic!("cannot locate workspace root");
        }
    }
}
