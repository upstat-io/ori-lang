//! Native Lean 4 emitter per
//! `the SMT / Lean 4 emission strategy`
//! Option C.
//!
//! Consumes a parsed `ProofFile` from `crate::ast` and renders a
//! Lean 4 source string the Lean 4 toolchain accepts unmodified (the
//! emitter's correctness contract is "produces well-formed Lean 4
//! syntax that Lean 4's parser would accept" — semantic equivalence to
//! the Ori claim is BLOCKED on Mathlib integration which is out of
//! scope for the §01A bootstrap per
//! `the Lean 4 bootstrap proofs`
//! Implementation Items).
//!
//! Foundational-axiom discipline per
//! `the foundational-axiom policy`:
//!
//! - Constructive logic by default — NO `Classical.em`, NO
//! `Classical.choice`, NO `funext`, NO `propext`, NO proof
//! irrelevance, NO Markov's Principle.
//! - The emitted file's comment header documents this discipline and
//! forbids classical axioms absent a matched commit per
//! the foundational-axiom policy §Permitted Extensions.
//!
//! Cross-validation contract per
//! `the Lean 4 cross-validation strategy`:
//!
//! - `aims-proof-checker emit-lean4 <proof-file> > <output>.lean` is
//! the SSOT path; manual hand-mirroring at `lean4-mirror/` is BANNED
//! per §01A Banned section.
//! - `lean --run <output>.lean` exits 0 on parity-green; divergence
//! routes through `cross_validation_divergence` per §01A's
//! FAIL-branch enum.
//!
//! Scaffold-time semantic gap (documented per
//! `the Lean 4 bootstrap proofs`
//! Implementation Items): the AimsState 7-tuple model expressed in
//! Ori's canonical notation does not yet have a Mathlib companion; the
//! emitter emits a constructive `AimsState` placeholder structure and
//! declares each theorem with body `True := by trivial`. The Ori
//! soundness-property source is preserved verbatim above the
//! declaration as an audit-trail comment block. Substantive
//! translation lands in subsequent commits.

use crate::ast::{ProofFile, ProofObligation, Theorem};

/// Render `proof` into a Lean 4 source string.
///
/// Output shape:
///
/// - File header documenting the SSOT, the constructive-axiom
/// discipline per the foundational-axiom policy, and the namespace declaration.
/// - One placeholder Lean 4 `inductive` / `structure` block declaring
/// the seven AimsState dimensions per `Annex E §AIMS` sec-1.1-sec-1.7.
/// - One `theorem` declaration per input theorem; each carries:
/// * a verbatim comment block containing the Ori soundness-property
/// source (audit trail);
/// * a `theorem <CATEGORY>_<SUFFIX>_<sanitized_name> : True := by trivial`
/// placeholder declaration so the file type-checks under Lean 4's
/// parser; substantive translation lands in subsequent commits;
/// * a `-- TODO` comment naming the substantive-translation gap.
///
/// The returned string is plain UTF-8; lines are joined with `\n` and
/// the file ends with a single trailing `\n` per Lean 4 convention.
pub fn emit(proof: &ProofFile) -> String {
    let mut out = String::new();

    emit_header(&mut out, proof);
    emit_aims_state_model(&mut out);

    for theorem in &proof.theorems {
        emit_theorem(&mut out, proof, theorem);
    }

    out.push_str("end AimsBootstrap\n");
    // `lean --run` (per §01A success_criterion #4) requires a `main`
    // function. The theorem declarations above are type-checked during
    // `lean --run`'s elaboration phase; the no-op `main` below satisfies
    // the interpreter's `unknown declaration 'main'` requirement without
    // introducing executable semantics that diverge from the Ori claim.
    out.push('\n');
    out.push_str("def main : IO Unit := pure ()\n");
    out
}

/// Emit the file-top header documenting SSOT + constructive-axiom
/// discipline + namespace.
fn emit_header(out: &mut String, proof: &ProofFile) {
    out.push_str("-- AIMS-Proof bootstrap translation\n");
    out.push_str(
        "-- SSOT: aims-proof/checker/src/emit/lean4.rs (the SMT / Lean 4 emission strategy Option C)\n",
    );
    out.push_str(
        "-- Constructive-by-default per the foundational-axiom policy; classical escalation\n",
    );
    out.push_str(
        "-- requires matched commit per the foundational-axiom policy §Permitted Extensions.\n",
    );
    out.push_str(
        "-- BANNED: Classical.em, Classical.choice, funext, propext, proof\n",
    );
    out.push_str(
        "-- irrelevance, Markov's Principle — absent a matched extension entry\n",
    );
    out.push_str(
        "-- in the foundational-axiom policy.\n",
    );
    out.push_str(&format!(
        "-- Translated from {}\n",
        proof.source_path.display()
    ));
    out.push('\n');
    out.push_str("namespace AimsBootstrap\n");
    out.push('\n');
}

/// Emit the AimsState 7-tuple placeholder model per `Annex E §AIMS`
/// sec-1.1-sec-1.7. The Lean 4 declarations are constructive inductive
/// types + a structure record; no Mathlib import; no Classical
/// namespace.
fn emit_aims_state_model(out: &mut String) {
    out.push_str("-- AimsState 7-tuple placeholder per Annex E §AIMS.1 through sec-1.7.\n");
    out.push_str("-- Each dimension is a finite constructive inductive carrier per\n");
    out.push_str("-- the foundational-axiom policy sec-Per-Engine-Constructive-Proof-Shape.\n");
    out.push_str("\n");
    out.push_str("inductive AccessClass\n");
    out.push_str(" | Borrowed\n");
    out.push_str(" | Owned\n");
    out.push_str("\n");
    out.push_str("inductive Consumption\n");
    out.push_str(" | Dead\n");
    out.push_str(" | Linear\n");
    out.push_str(" | Affine\n");
    out.push_str(" | Unrestricted\n");
    out.push_str("\n");
    out.push_str("inductive Cardinality\n");
    out.push_str(" | Absent\n");
    out.push_str(" | One\n");
    out.push_str(" | Many\n");
    out.push_str("\n");
    out.push_str("inductive Uniqueness\n");
    out.push_str(" | Unique\n");
    out.push_str(" | MaybeShared\n");
    out.push_str(" | Shared\n");
    out.push_str("\n");
    out.push_str("inductive Locality\n");
    out.push_str(" | BlockLocal\n");
    out.push_str(" | FunctionLocal\n");
    out.push_str(" | ArgEscaping\n");
    out.push_str(" | HeapEscaping\n");
    out.push_str(" | Unknown\n");
    out.push_str("\n");
    out.push_str("inductive Shape\n");
    out.push_str(" | NonReusable\n");
    out.push_str(" | ReusableStruct\n");
    out.push_str(" | ReusableEnumVariant\n");
    out.push_str(" | CollectionBuffer\n");
    out.push_str(" | ContextHole\n");
    out.push_str("\n");
    out.push_str("structure EffectClass where\n");
    out.push_str(" may_alloc : Bool := false\n");
    out.push_str(" may_share : Bool := false\n");
    out.push_str(" may_throw : Bool := false\n");
    out.push_str("\n");
    out.push_str("structure AimsState where\n");
    out.push_str(" access : AccessClass\n");
    out.push_str(" consumption : Consumption\n");
    out.push_str(" cardinality : Cardinality\n");
    out.push_str(" uniqueness : Uniqueness\n");
    out.push_str(" locality : Locality\n");
    out.push_str(" shape : Shape\n");
    out.push_str(" effect : EffectClass\n");
    out.push_str("\n");
}

/// Emit one theorem declaration carrying the Ori source as a comment
/// block plus a `True := by trivial` placeholder body.
fn emit_theorem(out: &mut String, proof: &ProofFile, theorem: &Theorem) {
    out.push_str(&format!(
        "-- Translated from {}:{}\n",
        proof.source_path.display(),
        theorem.id.canonical()
    ));
    out.push_str("-- Theorem name (verbatim from canonical-notation source):\n");
    out.push_str(&format!("-- {}\n", theorem.name));

    out.push_str("-- Preconditions (verbatim from canonical-notation source):\n");
    if theorem.preconditions.items.is_empty() {
        out.push_str("-- (none declared)\n");
    } else {
        for item in &theorem.preconditions.items {
            for line in item.lines() {
                out.push_str(&format!("-- - {}\n", line));
            }
        }
    }

    out.push_str("-- Soundness property (verbatim from canonical-notation source):\n");
    for line in theorem.soundness.source.lines() {
        out.push_str(&format!("-- {}\n", line));
    }

    out.push_str("-- Proof obligation (verbatim from canonical-notation source):\n");
    match &theorem.obligation {
        ProofObligation::Sorry => {
            out.push_str("-- sorry (per the canonical proof notation sec-Theorem-Statement-Format)\n");
        }
        ProofObligation::Steps(steps) => {
            if steps.is_empty() {
                out.push_str("-- (no steps; obligation pending)\n");
            } else {
                for step in steps {
                    for line in step.source.lines() {
                        out.push_str(&format!("-- {}\n", line));
                    }
                }
            }
        }
    }

    out.push_str(&format!(
        "-- TODO(BUG-XX-NNN): substantive translation pending Mathlib AimsState model;\n"
    ));
    out.push_str(
        "-- placeholder body is `True := by trivial` so Lean 4's parser accepts\n",
    );
    out.push_str(
        "-- the file. Semantic equivalence to the Ori claim is out of scope for\n",
    );
    out.push_str(
        "-- the §01A bootstrap per the SMT / Lean 4 emission strategy Option C consequences.\n",
    );

    let lean_name = lean4_theorem_name(theorem);
    out.push_str(&format!("theorem {} : True := by trivial\n", lean_name));
    out.push('\n');
}

/// Sanitize a theorem id + name into a Lean 4 identifier.
///
/// Lean 4 identifiers allow letters, digits, underscores, and a few
/// special characters; this emitter restricts the output alphabet to
/// `[A-Za-z0-9_]` for maximum portability across Lean 4 versions. The
/// category prefix and suffix come from the parsed `TheoremId`; the
/// theorem name is sanitized by replacing every non-identifier
/// character with `_` and collapsing consecutive underscores.
fn lean4_theorem_name(theorem: &Theorem) -> String {
    let prefix = theorem.id.category.prefix();
    let suffix = &theorem.id.suffix;
    let sanitized = sanitize_for_lean4_identifier(&theorem.name);
    if sanitized.is_empty() {
        format!("{}_{}", prefix, suffix)
    } else {
        format!("{}_{}_{}", prefix, suffix, sanitized)
    }
}

/// Replace every char outside `[A-Za-z0-9_]` with `_` and collapse
/// consecutive underscores. Trim leading/trailing underscores. Truncate
/// to 80 chars so the emitted identifier stays readable.
fn sanitize_for_lean4_identifier(input: &str) -> String {
    let mut out = String::new();
    let mut prev_underscore = false;
    for c in input.chars() {
        let ok = c.is_ascii_alphanumeric() || c == '_';
        if ok {
            out.push(c);
            prev_underscore = c == '_';
        } else if !prev_underscore && !out.is_empty() {
            out.push('_');
            prev_underscore = true;
        }
    }
    // Trim leading/trailing underscores.
    let trimmed: String = out.trim_matches('_').to_string();
    if trimmed.len() <= 80 {
        trimmed
    } else {
        trimmed.chars().take(80).collect::<String>().trim_end_matches('_').to_string()
    }
}

/// Convenience helper for the CLI: emit the Lean 4 source for the
/// theorem-set in `proof`. Pure wrapper over [`emit`]; retained as a
/// distinct public entry point so future emitter feature flags (per
/// the SMT / Lean 4 emission strategy Option C consequences) compose without breaking the
/// CLI surface.
pub fn emit_lean4(proof: &ProofFile) -> String {
    emit(proof)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        Category, ExpectedOutcome, Preconditions, ProofFile, ProofObligation, ProofStep,
        SoundnessProperty, Theorem, TheoremId,
    };
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        // CARGO_MANIFEST_DIR -> aims-proof/checker. One pop reaches aims-proof/.
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.pop();
        p
    }

    fn synthetic_proof() -> ProofFile {
        ProofFile {
            source_path: PathBuf::from("synthetic.proof"),
            theorems: vec![Theorem {
                id: TheoremId {
                    category: Category::Lattice,
                    suffix: "1".to_string(),
                },
                name: "Lattice join commutativity".to_string(),
                preconditions: Preconditions {
                    items: vec!["a, b are AimsState elements".to_string()],
                },
                soundness: SoundnessProperty {
                    source: "Forall a, b in AimsState. join(a, b) = join(b, a)".to_string(),
                },
                obligation: ProofObligation::Steps(vec![ProofStep {
                    source: "Per-dimension max commutativity".to_string(),
                }]),
                expected: Some(ExpectedOutcome {
                    status: "unimplemented_engine_shape".to_string(),
                    reason: "test fixture".to_string(),
                }),
            }],
        }
    }

    #[test]
    fn emit_produces_non_empty_output_for_synthetic_proof() {
        let proof = synthetic_proof();
        let lean = emit(&proof);
        assert!(!lean.is_empty(), "emit must produce non-empty output");
    }

    #[test]
    fn emit_output_starts_with_canonical_header() {
        let proof = synthetic_proof();
        let lean = emit(&proof);
        assert!(
            lean.starts_with("-- AIMS-Proof bootstrap translation\n"),
            "emit must start with the canonical AIMS-Proof header; got: {:?}",
            lean.lines().next()
        );
    }

    #[test]
    fn emit_carries_ssot_pointer() {
        let proof = synthetic_proof();
        let lean = emit(&proof);
        assert!(
            lean.contains("aims-proof/checker/src/emit/lean4.rs"),
            "emit must cite the SSOT path in the header"
        );
    }

    #[test]
    fn emit_documents_constructive_axiom_discipline() {
        let proof = synthetic_proof();
        let lean = emit(&proof);
        for ban in &["Classical.em", "Classical.choice", "funext"] {
            assert!(
                lean.contains(ban),
                "header must enumerate banned axiom {:?} per the foundational-axiom policy",
                ban
            );
        }
    }

    #[test]
    fn emit_declares_namespace_and_closes_it() {
        let proof = synthetic_proof();
        let lean = emit(&proof);
        assert!(
            lean.contains("namespace AimsBootstrap\n"),
            "namespace declaration missing"
        );
        assert!(
            lean.contains("end AimsBootstrap\n"),
            "namespace close missing"
        );
    }

    #[test]
    fn emit_appends_main_for_lean_run_compatibility() {
        let proof = synthetic_proof();
        let lean = emit(&proof);
        assert!(
            lean.contains("def main : IO Unit := pure ()"),
            "main function for `lean --run` per §01A success_criterion #4 missing"
        );
        assert!(
            lean.trim_end().ends_with("def main : IO Unit := pure ()"),
            "main must be the last declaration so `lean --run` finds it"
        );
    }

    #[test]
    fn emit_declares_seven_dimension_carriers() {
        let proof = synthetic_proof();
        let lean = emit(&proof);
        for carrier in &[
            "inductive AccessClass",
            "inductive Consumption",
            "inductive Cardinality",
            "inductive Uniqueness",
            "inductive Locality",
            "inductive Shape",
            "structure EffectClass",
            "structure AimsState",
        ] {
            assert!(
                lean.contains(carrier),
                "AimsState 7-tuple model missing {:?}",
                carrier
            );
        }
    }

    #[test]
    fn emit_renders_one_theorem_per_input() {
        let mut proof = synthetic_proof();
        // Duplicate the theorem so we can count the declarations.
        proof.theorems.push(proof.theorems[0].clone());
        let lean = emit(&proof);
        let theorem_count = lean
            .lines()
            .filter(|line| line.starts_with("theorem "))
            .count();
        assert_eq!(
            theorem_count, 2,
            "expected 2 theorem declarations; got {}: \n{}",
            theorem_count, lean
        );
    }

    #[test]
    fn emit_theorem_name_sanitizes_to_lean4_identifier() {
        let proof = synthetic_proof();
        let lean = emit(&proof);
        assert!(
            lean.contains("theorem L_1_Lattice_join_commutativity : True := by trivial"),
            "expected sanitized theorem identifier in output; got: {}",
            lean
        );
    }

    #[test]
    fn emit_preserves_soundness_source_verbatim_as_comment() {
        let proof = synthetic_proof();
        let lean = emit(&proof);
        // The soundness source is rendered as a `-- <line>` comment.
        assert!(
            lean.contains("-- Forall a, b in AimsState. join(a, b) = join(b, a)"),
            "expected verbatim soundness source as comment; got:\n{}"
            ,
            lean
        );
    }

    #[test]
    fn emit_smoke_fixture_produces_expected_shape() {
        let path = workspace_root().join("proofs/00-smoke-test/cn-1-bidirectional.proof");
        let proof = crate::parser::parse_proof_file(&path).expect("smoke proof must parse");
        let lean = emit(&proof);
        // Header present.
        assert!(lean.starts_with("-- AIMS-Proof bootstrap translation\n"));
        // Every parsed theorem produces one theorem declaration.
        let theorem_count = lean
            .lines()
            .filter(|line| line.starts_with("theorem "))
            .count();
        assert_eq!(
            theorem_count,
            proof.theorems.len(),
            "expected {} theorem declarations; got {}",
            proof.theorems.len(),
            theorem_count
        );
        // CN-1's category + suffix appears in the sanitized identifier.
        assert!(
            lean.contains("theorem CN_1_"),
            "expected CN-1 theorem identifier in output"
        );
    }

    #[test]
    fn sanitize_collapses_consecutive_underscores() {
        assert_eq!(sanitize_for_lean4_identifier("a b"), "a_b");
        assert_eq!(sanitize_for_lean4_identifier("a..b..c"), "a_b_c");
        assert_eq!(sanitize_for_lean4_identifier(" leading"), "leading");
        assert_eq!(sanitize_for_lean4_identifier("trailing "), "trailing");
        assert_eq!(sanitize_for_lean4_identifier("ABC_123"), "ABC_123");
    }

    #[test]
    fn emit_lean4_wrapper_returns_same_string_as_emit() {
        let proof = synthetic_proof();
        assert_eq!(emit(&proof), emit_lean4(&proof));
    }

    /// Touches the `Category` import to keep the module-level `use`
    /// pointer load-bearing for downstream emitter expansion (per
    /// the SMT / Lean 4 emission strategy Option C consequences: future emitter features will
    /// consume `Category` for axiom-namespace dispatch).
    #[test]
    fn category_import_remains_load_bearing() {
        let _ = Category::Lattice.prefix();
    }
}
