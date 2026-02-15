//! Common mistake detection for source text.
//!
//! When the lexer produces an `Error` token, these functions examine the
//! actual source text to provide targeted help for patterns from other languages.

use ori_ir::TokenKind;

/// Detect common mistakes from source text.
///
/// This is used when the lexer produces an `Error` token — we look at the
/// actual source text to provide targeted help for patterns from other languages.
///
/// # Arguments
/// * `source_text` - The slice of source that produced the error token
///
/// # Returns
/// A tuple of (short description, detailed help message) if a pattern is recognized
pub fn detect_common_mistake(source_text: &str) -> Option<(&'static str, &'static str)> {
    match source_text {
        // JavaScript/TypeScript triple equals
        "===" => Some((
            "triple equals",
            "Ori uses `==` for equality comparison. There's no `===` because Ori is \
             statically typed — values are always compared with consistent semantics.",
        )),

        // JavaScript/TypeScript strict not-equals
        "!==" => Some((
            "strict not-equals",
            "Ori uses `!=` for inequality. There's no `!==` because Ori's static typing \
             ensures consistent comparison semantics.",
        )),

        // C/Java increment/decrement
        "++" => Some((
            "increment operator",
            "Ori doesn't have `++`. Use `x = x + 1` or a compound assignment pattern.",
        )),
        "--" => Some((
            "decrement operator",
            "Ori doesn't have `--`. Use `x = x - 1` or a compound assignment pattern.",
        )),

        // Pascal/SQL not-equals
        "<>" => Some(("not-equals", "Ori uses `!=` for inequality, not `<>`.")),

        // Assignment operators from other languages
        "+=" | "-=" | "*=" | "/=" | "%=" | "&&=" | "||=" | "??=" => Some((
            "compound assignment",
            "Ori doesn't have compound assignment operators. Use `x = x + y` instead of `x += y`.",
        )),

        // Spread/rest from JavaScript
        "..." if source_text == "..." => Some((
            "spread operator",
            "For rest patterns in lists, use `..rest` (two dots). For struct rest, use `..`.",
        )),

        // These ARE valid in Ori - `??` (nullish coalescing) and `=>` (fat arrow)
        "??" | "=>" => None,

        _ => {
            // Check for common keyword-like identifiers
            check_common_keyword_mistake(source_text)
        }
    }
}

/// Check if a source fragment looks like a common keyword from other languages.
pub fn check_common_keyword_mistake(text: &str) -> Option<(&'static str, &'static str)> {
    match text {
        // OOP keywords
        "class" => Some((
            "class keyword",
            "Ori doesn't have classes. Use `type` for data structures and `trait` for \
             shared behavior. Ori favors composition over inheritance.",
        )),
        "extends" | "extends " => Some((
            "extends keyword",
            "Ori doesn't have inheritance. Use `trait` for shared behavior and \
             composition for combining types.",
        )),
        "implements" => Some((
            "implements keyword",
            "In Ori, use `impl Trait for Type { ... }` to implement a trait for a type.",
        )),
        "interface" => Some((
            "interface keyword",
            "Ori uses `trait` instead of `interface`. Traits define shared behavior \
             that types can implement.",
        )),
        "abstract" => Some((
            "abstract keyword",
            "Ori doesn't have abstract classes. Use traits for polymorphic behavior.",
        )),
        "virtual" | "override" => Some((
            "virtual/override keyword",
            "Ori doesn't have virtual methods or override. Traits provide polymorphism \
             without inheritance hierarchies.",
        )),

        // Control flow from other languages
        "switch" => Some((
            "switch keyword",
            "Ori uses `match` instead of `switch`. Match expressions are exhaustive \
             and support pattern matching.",
        )),
        "case" => Some((
            "case keyword",
            "In Ori's `match`, use `pattern -> expression` instead of `case:`. \
             Example: `match(x, 1 -> \"one\", _ -> \"other\")`.",
        )),
        "default" => Some((
            "default keyword",
            "In Ori's `match`, use `_` (underscore) as the wildcard/default pattern.",
        )),
        "elif" => Some((
            "elif keyword",
            "Ori uses `else if` (two words), not `elif`.",
        )),
        "elsif" => Some((
            "elsif keyword",
            "Ori uses `else if` (two words), not `elsif`.",
        )),
        "elseif" => Some((
            "elseif keyword",
            "Ori uses `else if` (two words, with space), not `elseif`.",
        )),

        // Function keywords
        "function" | "func" | "fn" => Some((
            "function keyword",
            "Ori functions are declared with `@` prefix: `@add (a: int, b: int) -> int = a + b`.",
        )),
        "lambda" => Some((
            "lambda keyword",
            "Ori uses `|args| body` for anonymous functions: `|x| x * 2`.",
        )),

        // Variable keywords
        "var" | "const" => Some((
            "var/const keyword",
            "Ori uses `let` for variable binding. Variables are mutable by default; \
             use `$name` for immutable bindings.",
        )),
        "final" => Some((
            "final keyword",
            "Ori uses `$name` (dollar prefix) for immutable bindings instead of `final`.",
        )),

        // Module keywords
        "import" | "from" => Some((
            "import keyword",
            "Ori uses `use` for imports: `use std::math` or `use std::io::{read, write}`.",
        )),
        "require" => Some((
            "require keyword",
            "Ori uses `use` for imports, not `require`.",
        )),
        "export" | "module" => Some((
            "export/module keyword",
            "In Ori, items are public by default. Use `::` prefix for private items.",
        )),

        // Exception handling
        "throw" | "throws" | "raise" => Some((
            "throw/raise keyword",
            "Ori uses `Result` types for error handling. Return `Err(value)` instead \
             of throwing. Use `?` to propagate errors.",
        )),
        "except" | "catch" if text != "catch" => Some((
            "except keyword",
            "Ori uses `catch { ... }` pattern for error handling, which wraps the \
             result in a `Result` type.",
        )),
        "finally" => Some((
            "finally keyword",
            "Ori doesn't have `finally`. Use RAII patterns or explicit cleanup.",
        )),

        // Null/None
        "null" | "nil" | "NULL" => Some((
            "null keyword",
            "Ori uses `None` (capital N) for absent values in `Option` types. \
             Use `Some(value)` for present values.",
        )),
        "undefined" => Some((
            "undefined keyword",
            "Ori doesn't have `undefined`. Use `Option` types with `Some`/`None`.",
        )),

        // Boolean literals (case sensitivity)
        "True" | "TRUE" | "False" | "FALSE" => Some((
            "boolean literal",
            "Ori booleans are lowercase: `true` and `false`.",
        )),

        // Note: Valid Ori type keywords (int, float, str, bool, char, byte, void)
        // are handled by the wildcard arm below, returning None.

        // Common type names from other languages
        "string" | "String" => Some((
            "string type",
            "Ori uses `str` (lowercase, three letters) for the string type.",
        )),
        "integer" | "Integer" | "Int" => Some((
            "integer type",
            "Ori uses `int` (lowercase, three letters) for the integer type.",
        )),
        "boolean" | "Boolean" | "Bool" => Some((
            "boolean type",
            "Ori uses `bool` (lowercase, four letters) for the boolean type.",
        )),
        "double" | "Double" | "Float" => Some((
            "float type",
            "Ori uses `float` (lowercase) for floating-point numbers.",
        )),
        "Void" => Some((
            "void type",
            "Ori uses `void` (lowercase) for the unit type.",
        )),

        _ => None,
    }
}

/// Get the closing delimiter for an opening delimiter.
pub(crate) fn closing_delimiter(open: &TokenKind) -> TokenKind {
    match open {
        TokenKind::LParen => TokenKind::RParen,
        TokenKind::LBracket => TokenKind::RBracket,
        TokenKind::LBrace => TokenKind::RBrace,
        TokenKind::Lt => TokenKind::Gt,
        _ => TokenKind::Eof, // fallback
    }
}

/// Get a human-readable name for a delimiter type.
pub(crate) fn delimiter_name(open: &TokenKind) -> &'static str {
    match open {
        TokenKind::LParen => "parenthesis",
        TokenKind::LBracket => "bracket",
        TokenKind::LBrace => "brace",
        TokenKind::Lt => "angle bracket",
        _ => "delimiter",
    }
}
