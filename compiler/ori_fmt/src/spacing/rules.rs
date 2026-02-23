//! Declarative spacing rules.
//!
//! All spacing decisions are defined here as static rules.
//! Adding new rules requires only adding entries to `SPACE_RULES`.
//!
//! # Spec Reference
//!
//! - Lines 25-30: Binary operators
//! - Lines 31-35: Delimiters
//! - Lines 36-41: Keywords and punctuation
//! - Lines 42-47: Context-dependent

use super::{SpaceAction, TokenCategory, TokenMatcher};

/// A declarative spacing rule.
///
/// Rules are evaluated in order; the first matching rule determines the action.
#[derive(Clone, Copy, Debug)]
pub struct SpaceRule {
    /// Human-readable name for debugging.
    pub name: &'static str,

    /// Matcher for the left (preceding) token.
    pub left: TokenMatcher,

    /// Matcher for the right (following) token.
    pub right: TokenMatcher,

    /// The spacing action to apply.
    pub action: SpaceAction,

    /// Priority (lower = higher priority, checked first).
    /// Rules with the same priority are checked in definition order.
    pub priority: u8,
}

impl SpaceRule {
    /// Create a new spacing rule.
    #[inline]
    const fn new(
        name: &'static str,
        left: TokenMatcher,
        right: TokenMatcher,
        action: SpaceAction,
    ) -> Self {
        SpaceRule {
            name,
            left,
            right,
            action,
            priority: 50, // Default priority
        }
    }

    /// Create a rule with specific priority.
    #[inline]
    const fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Check if this rule matches the given token pair.
    #[inline]
    pub fn matches(&self, left: TokenCategory, right: TokenCategory) -> bool {
        self.left.matches(left) && self.right.matches(right)
    }
}

// Helper constants for rule definitions

#[allow(
    clippy::enum_glob_use,
    reason = "70+ spacing rules are much more readable with short names"
)]
use TokenCategory::*;
use TokenMatcher::{Any, Category, Exact};

// Common token groups as static slices
static RANGE_OPS: &[TokenCategory] = &[DotDot, DotDotEq];

// All spacing rules

/// All spacing rules in evaluation order.
///
/// Rules are sorted by priority, then by definition order within each priority.
/// Lower priority number = checked first.
///
/// # Rule Categories
///
/// - Priority 10: Empty delimiters (most specific)
/// - Priority 20: Delimiter adjacency
/// - Priority 30: Punctuation rules
/// - Priority 40: Operator rules
/// - Priority 50: Keyword rules (default)
/// - Priority 90: Fallback rules
pub static SPACE_RULES: &[SpaceRule] = &[
    // Priority 10: Empty delimiters (most specific matches)
    // No space inside empty delimiters: (), [], {}
    SpaceRule::new(
        "EmptyParens",
        Exact(LParen),
        Exact(RParen),
        SpaceAction::None,
    )
    .with_priority(10),
    SpaceRule::new(
        "EmptyBrackets",
        Exact(LBracket),
        Exact(RBracket),
        SpaceAction::None,
    )
    .with_priority(10),
    SpaceRule::new(
        "EmptyBraces",
        Exact(LBrace),
        Exact(RBrace),
        SpaceAction::None,
    )
    .with_priority(10),
    // Priority 20: Delimiter rules
    // No space after opening delimiters: (x, [x, {x
    SpaceRule::new("AfterLParen", Exact(LParen), Any, SpaceAction::None).with_priority(20),
    SpaceRule::new("AfterLBracket", Exact(LBracket), Any, SpaceAction::None).with_priority(20),
    // Note: LBrace handled specially for struct literals (space inside)
    // No space before closing delimiters: x), x], x}
    SpaceRule::new("BeforeRParen", Any, Exact(RParen), SpaceAction::None).with_priority(20),
    SpaceRule::new("BeforeRBracket", Any, Exact(RBracket), SpaceAction::None).with_priority(20),
    SpaceRule::new("BeforeRBrace", Any, Exact(RBrace), SpaceAction::None).with_priority(20),
    // Priority 25: Field access and method calls (highest priority for dot)
    // No space around dot: x.y, x.method()
    SpaceRule::new("BeforeDot", Any, Exact(Dot), SpaceAction::None).with_priority(25),
    SpaceRule::new("AfterDot", Exact(Dot), Any, SpaceAction::None).with_priority(25),
    // No space around double colon: Module::item
    SpaceRule::new(
        "BeforeDoubleColon",
        Any,
        Exact(DoubleColon),
        SpaceAction::None,
    )
    .with_priority(25),
    SpaceRule::new(
        "AfterDoubleColon",
        Exact(DoubleColon),
        Any,
        SpaceAction::None,
    )
    .with_priority(25),
    // Priority 30: Punctuation rules
    // Comma: space after, no space before
    SpaceRule::new("AfterComma", Exact(Comma), Any, SpaceAction::Space).with_priority(30),
    SpaceRule::new("BeforeComma", Any, Exact(Comma), SpaceAction::None).with_priority(30),
    // Colon: space after (type annotations), no space before
    SpaceRule::new("AfterColon", Exact(Colon), Any, SpaceAction::Space).with_priority(30),
    SpaceRule::new("BeforeColon", Any, Exact(Colon), SpaceAction::None).with_priority(30),
    // Semicolon: space after, no space before
    SpaceRule::new("AfterSemicolon", Exact(Semicolon), Any, SpaceAction::Space).with_priority(30),
    SpaceRule::new("BeforeSemicolon", Any, Exact(Semicolon), SpaceAction::None).with_priority(30),
    // Question mark (error propagation): no space before
    SpaceRule::new("BeforeQuestion", Any, Exact(Question), SpaceAction::None).with_priority(30),
    // Range operators: no space around
    SpaceRule::new(
        "BeforeRange",
        Any,
        TokenMatcher::OneOf(RANGE_OPS),
        SpaceAction::None,
    )
    .with_priority(30),
    SpaceRule::new(
        "AfterRange",
        TokenMatcher::OneOf(RANGE_OPS),
        Any,
        SpaceAction::None,
    )
    .with_priority(30),
    // Spread operator: no space after ...
    // Note: DotDotDot doesn't exist in TokenKind, it's parsed as DotDot + Dot
    // Priority 35: Special identifier-adjacent rules
    // No space between @ and identifier (function names): @foo
    SpaceRule::new("AtIdent", Exact(At), Exact(Ident), SpaceAction::None).with_priority(35),
    // No space between $ and identifier (constants): $FOO
    SpaceRule::new(
        "DollarIdent",
        Exact(Dollar),
        Exact(Ident),
        SpaceAction::None,
    )
    .with_priority(35),
    // No space after # in attributes: #derive
    SpaceRule::new("HashIdent", Exact(Hash), Exact(Ident), SpaceAction::None).with_priority(35),
    // No space after #[ in attributes: #[derive]
    SpaceRule::new(
        "HashBracketContent",
        Exact(HashBracket),
        Any,
        SpaceAction::None,
    )
    .with_priority(35),
    // No space after #! in file attributes: #!compiler_version
    SpaceRule::new("HashBangContent", Exact(HashBang), Any, SpaceAction::None).with_priority(35),
    // Priority 40: Operator rules
    // Space around binary operators: a + b, x == y
    SpaceRule::new(
        "BeforeBinaryOp",
        Any,
        Category(TokenCategory::is_binary_op),
        SpaceAction::Space,
    )
    .with_priority(40),
    SpaceRule::new(
        "AfterBinaryOp",
        Category(TokenCategory::is_binary_op),
        Any,
        SpaceAction::Space,
    )
    .with_priority(40),
    // Space around assignment: x = 1
    SpaceRule::new("BeforeEq", Any, Exact(Eq), SpaceAction::Space).with_priority(40),
    SpaceRule::new("AfterEq", Exact(Eq), Any, SpaceAction::Space).with_priority(40),
    // Space around compound assignment: x += 1
    SpaceRule::new(
        "BeforeCompoundAssign",
        Any,
        Exact(CompoundAssign),
        SpaceAction::Space,
    )
    .with_priority(40),
    SpaceRule::new(
        "AfterCompoundAssign",
        Exact(CompoundAssign),
        Any,
        SpaceAction::Space,
    )
    .with_priority(40),
    // Space around arrow: (x) -> y
    SpaceRule::new("BeforeArrow", Any, Exact(Arrow), SpaceAction::Space).with_priority(40),
    SpaceRule::new("AfterArrow", Exact(Arrow), Any, SpaceAction::Space).with_priority(40),
    // Space around fat arrow: pattern => expr
    SpaceRule::new("BeforeFatArrow", Any, Exact(FatArrow), SpaceAction::Space).with_priority(40),
    SpaceRule::new("AfterFatArrow", Exact(FatArrow), Any, SpaceAction::Space).with_priority(40),
    // Space around null coalescing: x ?? default
    SpaceRule::new(
        "BeforeDoubleQuestion",
        Any,
        Exact(DoubleQuestion),
        SpaceAction::Space,
    )
    .with_priority(40),
    SpaceRule::new(
        "AfterDoubleQuestion",
        Exact(DoubleQuestion),
        Any,
        SpaceAction::Space,
    )
    .with_priority(40),
    // Priority 45: Unary operators (after binary to not override)
    // No space after unary operators: -x, !y, ~z
    // Note: Context-dependent - only when actually unary (handled by lookup)
    SpaceRule::new(
        "AfterUnaryMinus",
        Exact(Minus),
        Category(TokenCategory::is_literal),
        SpaceAction::None,
    )
    .with_priority(45),
    SpaceRule::new("AfterBang", Exact(Bang), Any, SpaceAction::None).with_priority(45),
    SpaceRule::new("AfterTilde", Exact(Tilde), Any, SpaceAction::None).with_priority(45),
    // Priority 50: Keyword rules
    // Space after pub: pub @foo, pub type
    SpaceRule::new("AfterPub", Exact(Pub), Any, SpaceAction::Space).with_priority(50),
    // Space after let: let x
    SpaceRule::new("AfterLet", Exact(Let), Any, SpaceAction::Space).with_priority(50),
    // Space after type: type Foo
    SpaceRule::new("AfterType", Exact(Type), Any, SpaceAction::Space).with_priority(50),
    // Space after trait: trait Foo
    SpaceRule::new("AfterTrait", Exact(Trait), Any, SpaceAction::Space).with_priority(50),
    // Space after impl: impl Foo
    SpaceRule::new("AfterImpl", Exact(Impl), Any, SpaceAction::Space).with_priority(50),
    // Space after use: use std
    SpaceRule::new("AfterUse", Exact(Use), Any, SpaceAction::Space).with_priority(50),
    // Space after extend: extend Foo
    SpaceRule::new("AfterExtend", Exact(Extend), Any, SpaceAction::Space).with_priority(50),
    // Space after extension: extension std.foo
    SpaceRule::new("AfterExtension", Exact(Extension), Any, SpaceAction::Space).with_priority(50),
    // Space after def: def impl
    SpaceRule::new("AfterDef", Exact(Def), Any, SpaceAction::Space).with_priority(50),
    // Space after if: if condition
    SpaceRule::new("AfterIf", Exact(If), Any, SpaceAction::Space).with_priority(50),
    // Space after then: then result
    SpaceRule::new("AfterThen", Exact(Then), Any, SpaceAction::Space).with_priority(50),
    // Space after else: else result
    SpaceRule::new("AfterElse", Exact(Else), Any, SpaceAction::Space).with_priority(50),
    // Space after for: for x
    SpaceRule::new("AfterFor", Exact(For), Any, SpaceAction::Space).with_priority(50),
    // Space after in: in items
    SpaceRule::new("AfterIn", Exact(In), Any, SpaceAction::Space).with_priority(50),
    // Space before in: x in
    SpaceRule::new("BeforeIn", Any, Exact(In), SpaceAction::Space).with_priority(50),
    // Space after yield: yield x
    SpaceRule::new("AfterYield", Exact(Yield), Any, SpaceAction::Space).with_priority(50),
    // Space after do: do x
    SpaceRule::new("AfterDo", Exact(Do), Any, SpaceAction::Space).with_priority(50),
    // Space after where: where T
    SpaceRule::new("AfterWhere", Exact(Where), Any, SpaceAction::Space).with_priority(50),
    // Space after uses: uses Http
    SpaceRule::new("AfterUses", Exact(Uses), Any, SpaceAction::Space).with_priority(50),
    // Space after with: with Http = Mock
    SpaceRule::new("AfterWith", Exact(With), Any, SpaceAction::Space).with_priority(50),
    // Space around as: x as int, y as? float
    SpaceRule::new("BeforeAs", Any, Exact(As), SpaceAction::Space).with_priority(50),
    SpaceRule::new("AfterAs", Exact(As), Any, SpaceAction::Space).with_priority(50),
    // Space after tests: tests @foo
    SpaceRule::new("AfterTests", Exact(Tests), Any, SpaceAction::Space).with_priority(50),
    // Priority 55: Pattern keywords (run, try, match, etc.)
    // No space between pattern keyword and opening paren: run(, try(
    SpaceRule::new("RunParen", Exact(Run), Exact(LParen), SpaceAction::None).with_priority(55),
    SpaceRule::new("TryParen", Exact(Try), Exact(LParen), SpaceAction::None).with_priority(55),
    SpaceRule::new("MatchParen", Exact(Match), Exact(LParen), SpaceAction::None).with_priority(55),
    SpaceRule::new(
        "RecurseParen",
        Exact(Recurse),
        Exact(LParen),
        SpaceAction::None,
    )
    .with_priority(55),
    SpaceRule::new(
        "ParallelParen",
        Exact(Parallel),
        Exact(LParen),
        SpaceAction::None,
    )
    .with_priority(55),
    SpaceRule::new("SpawnParen", Exact(Spawn), Exact(LParen), SpaceAction::None).with_priority(55),
    SpaceRule::new(
        "TimeoutParen",
        Exact(Timeout),
        Exact(LParen),
        SpaceAction::None,
    )
    .with_priority(55),
    SpaceRule::new("CacheParen", Exact(Cache), Exact(LParen), SpaceAction::None).with_priority(55),
    SpaceRule::new("CatchParen", Exact(Catch), Exact(LParen), SpaceAction::None).with_priority(55),
    SpaceRule::new("PrintParen", Exact(Print), Exact(LParen), SpaceAction::None).with_priority(55),
    SpaceRule::new("PanicParen", Exact(Panic), Exact(LParen), SpaceAction::None).with_priority(55),
    SpaceRule::new("TodoParen", Exact(Todo), Exact(LParen), SpaceAction::None).with_priority(55),
    SpaceRule::new(
        "UnreachableParen",
        Exact(Unreachable),
        Exact(LParen),
        SpaceAction::None,
    )
    .with_priority(55),
    // No space between wrapper keyword and opening paren: Ok(, Err(, Some(
    SpaceRule::new("OkParen", Exact(Ok), Exact(LParen), SpaceAction::None).with_priority(55),
    SpaceRule::new("ErrParen", Exact(Err), Exact(LParen), SpaceAction::None).with_priority(55),
    SpaceRule::new("SomeParen", Exact(Some), Exact(LParen), SpaceAction::None).with_priority(55),
    // No space between loop and opening paren: loop(
    SpaceRule::new("LoopParen", Exact(Loop), Exact(LParen), SpaceAction::None).with_priority(55),
    // Priority 60: Sum type pipe (context-dependent in practice)
    // Space around | in sum types: A | B | C
    SpaceRule::new("BeforePipe", Any, Exact(Pipe), SpaceAction::Space).with_priority(60),
    SpaceRule::new("AfterPipe", Exact(Pipe), Any, SpaceAction::Space).with_priority(60),
    // Priority 70: Generic bounds
    // Space around + in bounds: T: A + B
    SpaceRule::new("BeforeBoundPlus", Any, Exact(Plus), SpaceAction::Space).with_priority(70),
    SpaceRule::new("AfterBoundPlus", Exact(Plus), Any, SpaceAction::Space).with_priority(70),
    // Priority 90: Fallback rules (lowest priority)
    // Default: no space between tokens not covered by other rules
    SpaceRule::new("Default", Any, Any, SpaceAction::None).with_priority(90),
];

/// Get the number of spacing rules.
pub fn rule_count() -> usize {
    SPACE_RULES.len()
}

/// Find the first matching rule for a token pair.
///
/// Rules are checked in priority order, then definition order.
pub fn find_rule(left: TokenCategory, right: TokenCategory) -> &'static SpaceRule {
    // Rules are already sorted by priority in the static definition
    for rule in SPACE_RULES {
        if rule.matches(left, right) {
            return rule;
        }
    }

    // Should never happen - last rule is (Any, Any)
    unreachable!("No matching spacing rule found for {:?} {:?}", left, right)
}

/// Get the spacing action for a token pair.
#[inline]
pub fn spacing_between(left: TokenCategory, right: TokenCategory) -> SpaceAction {
    find_rule(left, right).action
}
