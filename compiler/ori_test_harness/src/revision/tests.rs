use super::*;
use crate::directive::{Directive, DirectiveLine};

fn make_directive(line: usize, revision: Option<&str>, directive: Directive) -> DirectiveLine {
    DirectiveLine {
        line_number: line,
        revision: revision.map(String::from),
        directive,
    }
}

// Positive

#[test]
fn test_no_revisions_directive_returns_single_default() {
    let directives = vec![make_directive(
        1,
        None,
        Directive::Check {
            pattern: "foo".into(),
        },
    )];
    let revisions = expand_revisions(&directives);
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].name, "");
}

#[test]
fn test_revisions_directive_expands_to_one_config_per_name() {
    let directives = vec![make_directive(
        1,
        None,
        Directive::Revisions {
            names: vec!["debug".into(), "release".into()],
        },
    )];
    let revisions = expand_revisions(&directives);
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].name, "debug");
    assert_eq!(revisions[1].name, "release");
}

#[test]
fn test_revision_specific_compile_flags_applied_to_correct_revision() {
    let directives = vec![
        make_directive(
            1,
            None,
            Directive::Revisions {
                names: vec!["debug".into(), "release".into()],
            },
        ),
        make_directive(
            2,
            Some("release"),
            Directive::CompileFlags {
                flags: vec!["--release".into()],
            },
        ),
    ];
    let revisions = expand_revisions(&directives);
    assert!(revisions[0].compile_flags.is_empty()); // debug: no flags
    assert_eq!(revisions[1].compile_flags, vec!["--release"]); // release: has flags
}

#[test]
fn test_filter_directives_returns_ungated_plus_matching_revision() {
    let directives = vec![
        make_directive(
            1,
            None,
            Directive::Check {
                pattern: "a".into(),
            },
        ),
        make_directive(
            2,
            Some("release"),
            Directive::Check {
                pattern: "b".into(),
            },
        ),
        make_directive(
            3,
            Some("debug"),
            Directive::Check {
                pattern: "c".into(),
            },
        ),
    ];
    let filtered = filter_directives_for_revision(&directives, "release");
    assert_eq!(filtered.len(), 2); // ungated "a" + release "b"
    assert_eq!(filtered[0].line_number, 1);
    assert_eq!(filtered[1].line_number, 2);
}

// Negative

#[test]
fn test_filter_directives_excludes_other_revision_directives() {
    let directives = vec![
        make_directive(
            1,
            Some("debug"),
            Directive::Check {
                pattern: "only-debug".into(),
            },
        ),
        make_directive(
            2,
            Some("release"),
            Directive::Check {
                pattern: "only-release".into(),
            },
        ),
    ];
    let filtered = filter_directives_for_revision(&directives, "debug");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].line_number, 1); // only the debug one
}
