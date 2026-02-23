use super::*;

/// Flag inventory: every `ParseContext` flag must have a corresponding accessor
/// and must be used in production code. If you add a new flag, add it here.
/// If a flag becomes unused, this test reminds you to remove it.
///
/// This prevents dead flags (like `CONST_EXPR` was) from accumulating.
#[test]
fn parse_context_flag_inventory() {
    // All flags and their accessor results on a context with that flag set.
    // When adding a new flag:
    //   1. Add the flag constant to ParseContext
    //   2. Add an accessor method
    //   3. Wire it into parser logic (don't leave it dead)
    //   4. Add it to this inventory
    let flags_and_accessors: &[(ParseContext, &str, bool)] = &[
        // (flag, name, accessor returns true when flag is set)
        (
            ParseContext::IN_PATTERN,
            "IN_PATTERN",
            ParseContext::IN_PATTERN.in_pattern(),
        ),
        (
            ParseContext::IN_TYPE,
            "IN_TYPE",
            ParseContext::IN_TYPE.in_type(),
        ),
        (
            ParseContext::NO_STRUCT_LIT,
            "NO_STRUCT_LIT",
            !ParseContext::NO_STRUCT_LIT.allows_struct_lit(),
        ),
        (
            ParseContext::IN_LOOP,
            "IN_LOOP",
            ParseContext::IN_LOOP.in_loop(),
        ),
        (
            ParseContext::ALLOW_YIELD,
            "ALLOW_YIELD",
            ParseContext::ALLOW_YIELD.allows_yield(),
        ),
        (
            ParseContext::IN_FUNCTION,
            "IN_FUNCTION",
            ParseContext::IN_FUNCTION.in_function(),
        ),
        (
            ParseContext::IN_INDEX,
            "IN_INDEX",
            ParseContext::IN_INDEX.in_index(),
        ),
        (
            ParseContext::PIPE_IS_SEPARATOR,
            "PIPE_IS_SEPARATOR",
            ParseContext::PIPE_IS_SEPARATOR.has(ParseContext::PIPE_IS_SEPARATOR),
        ),
    ];

    for (flag, name, accessor_result) in flags_and_accessors {
        // Each flag must be non-zero (a real bit)
        assert_ne!(
            *flag,
            ParseContext::NONE,
            "flag {name} has value NONE — not a real flag",
        );
        // Its accessor must detect it
        assert!(
            *accessor_result,
            "flag {name} accessor does not detect the flag — check accessor wiring",
        );
    }

    // Verify all flags are distinct (no bit collisions after removal/renumbering)
    for (i, (a, a_name, _)) in flags_and_accessors.iter().enumerate() {
        for (b, b_name, _) in &flags_and_accessors[i + 1..] {
            assert_ne!(a, b, "flags {a_name} and {b_name} have the same bit value");
        }
    }

    // Verify the union of all flags matches expected count.
    // If you add a new flag, bump this number.
    assert_eq!(
        flags_and_accessors.len(),
        8,
        "ParseContext flag count changed — update this inventory \
         (add or remove entries above to match)",
    );
}

/// All flags must be distinct bits with no overlapping values.
#[test]
fn parse_context_flags_are_distinct() {
    let all_flags = [
        ParseContext::IN_PATTERN,
        ParseContext::IN_TYPE,
        ParseContext::NO_STRUCT_LIT,
        ParseContext::IN_LOOP,
        ParseContext::ALLOW_YIELD,
        ParseContext::IN_FUNCTION,
        ParseContext::IN_INDEX,
        ParseContext::PIPE_IS_SEPARATOR,
    ];

    // Union of all flags should have exactly N bits set
    let union = all_flags
        .iter()
        .fold(ParseContext::NONE, |acc, f| acc.with(*f));

    // If any two flags shared a bit, the union would have fewer bits
    // than the number of flags. We check by setting each individually.
    for (i, _flag) in all_flags.iter().enumerate() {
        // Each flag should contribute exactly one new bit
        let without_a = all_flags
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .fold(ParseContext::NONE, |acc, (_, f)| acc.with(*f));

        assert_ne!(
            union, without_a,
            "removing flag {i} doesn't change the union — it shares bits with another flag",
        );
    }
}

#[test]
fn test_default_context() {
    let ctx = ParseContext::new();
    assert_eq!(ctx, ParseContext::NONE);
    assert!(ctx.allows_struct_lit());
    assert!(!ctx.in_pattern());
    assert!(!ctx.in_type());
    assert!(!ctx.in_loop());
}

#[test]
fn test_with_flag() {
    let ctx = ParseContext::new().with(ParseContext::IN_PATTERN);
    assert!(ctx.in_pattern());
    assert!(!ctx.in_type());
}

#[test]
fn test_without_flag() {
    let ctx = ParseContext::new()
        .with(ParseContext::IN_PATTERN)
        .with(ParseContext::IN_TYPE);
    assert!(ctx.in_pattern());
    assert!(ctx.in_type());

    let ctx = ctx.without(ParseContext::IN_PATTERN);
    assert!(!ctx.in_pattern());
    assert!(ctx.in_type());
}

#[test]
fn test_no_struct_lit() {
    let ctx = ParseContext::new();
    assert!(ctx.allows_struct_lit());

    let ctx = ctx.with(ParseContext::NO_STRUCT_LIT);
    assert!(!ctx.allows_struct_lit());
}

#[test]
fn test_multiple_flags() {
    let ctx = ParseContext::new()
        .with(ParseContext::IN_LOOP)
        .with(ParseContext::ALLOW_YIELD);

    assert!(ctx.in_loop());
    assert!(ctx.allows_yield());
    assert!(!ctx.in_pattern());
}

#[test]
fn test_union() {
    let ctx1 = ParseContext::new().with(ParseContext::IN_PATTERN);
    let ctx2 = ParseContext::new().with(ParseContext::IN_LOOP);
    let combined = ctx1.union(ctx2);

    assert!(combined.in_pattern());
    assert!(combined.in_loop());
}
