//! Canonical-notation parser per
//! `the canonical proof notation`.
//!
//! Consumes the human-readable + machine-checked notation defined in
//! the canonical proof notation. Token classes (per the canonical proof notation sec-Notation-Parseability):
//!
//! - Greek letters: empty, absent, top, Phi, omega (Unicode: ∅, ⊘, ⊤, Φ, ω).
//! - Mathematical Greek (Shape variants): R<S>, R<V> (angle brackets enclose
//! subscripts).
//! - Lattice operators: <=, <, ⊔, ⊓, <=>, =>, <->, /\, \/, ¬, :=, |->.
//! - Side-table operators: |-> (map entry).
//! - Tuple delimiters: < ... >.
//! - Record/list delimiters: { ... }, [ ... ].
//! - Identifiers: ASCII [A-Za-z_][A-Za-z0-9_]* for variable names,
//! predicate names, rule labels.
//! - ArcVarId form: %[0-9]+.
//!
//! Whitespace-insensitive within tuples; commas separate dimensions;
//! trailing comma permitted.
//!
//! Theorem block format:
//! ```text
//! Theorem <id>: <name>
//! Preconditions:
//! - <p>
//! Soundness property:
//! <invariant>
//! Proof obligation:
//! <body>
//! Engines dispatched:
//! <engine_name>
//! Expected:
//! status: <s>
//! reason: <r>
//! ```
//!
//! Parser MUST reject unknown lattice symbols with exit_reason
//! `notation_unknown_symbol` per the proof-checker design FAIL-branch closed enum.
//! Parser MUST emit recoverable diagnostics on missing dimension in tuple
//! (under-specified state) with exit_reason `notation_state_arity_mismatch`.
//!
//! Implementation language: hand-rolled lexer + recursive-descent parser
//! over stdlib `&str` — zero external dependency surface per the proof-checker design
//! Kernel Verification Methodology (smaller trusted kernel = easier
//! read-by-review).

use crate::ast::{
    Category, ExpectedOutcome, Preconditions, ProofFile, ProofObligation, ProofStep,
    SoundnessProperty, Theorem, TheoremId,
};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse a canonical-notation proof file from disk into the in-memory AST.
///
/// On success, returns a `ProofFile` containing every theorem block in the
/// file. On failure, returns a `ParseError` carrying the structured diagnostic
/// that routes through the §00 FAIL-branch enum.
pub fn parse_proof_file(path: &Path) -> Result<ProofFile, ParseError> {
    let source = fs::read_to_string(path).map_err(|err| ParseError {
        exit_reason: ParseExitReason::NotationMissingTheoremBlock,
        message: format!("failed to read proof file {}: {}", path.display(), err),
        line: None,
        column: None,
    })?;
    let tokens = tokenize(&source)?;
    let theorems = parse_proof_file_tokens_with_source(&tokens, &source)?;
    Ok(ProofFile {
        source_path: PathBuf::from(path),
        theorems,
    })
}

/// Structured parse-error diagnostic.
///
/// Carries the exit_reason key that routes through the §00 FAIL-branch
/// closed enum per `the design-validation gate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Closed-enum exit_reason value per the canonical proof notation sec-Notation-Parseability.
    ///
    /// Values: `notation_unknown_symbol`, `notation_state_arity_mismatch`,
    /// `notation_missing_theorem_block`, `notation_malformed_tuple`.
    pub exit_reason: ParseExitReason,

    /// Human-readable diagnostic message.
    pub message: String,

    /// 1-indexed line number of the offending token; `None` if not applicable.
    pub line: Option<usize>,

    /// 1-indexed column number of the offending token; `None` if not applicable.
    pub column: Option<usize>,
}

/// Closed enum of parser-level failure modes.
///
/// Each variant maps to a documented exit_reason key in
/// `the canonical proof notation`
/// sec-Notation-Parseability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseExitReason {
    /// Parser encountered a lattice symbol not in the canonical table at
    /// the canonical proof notation sec-Per-Dimension-Symbol-Set.
    NotationUnknownSymbol,

    /// AimsState tuple is under-specified (fewer dimensions than the
    /// current per-dimension symbol set).
    NotationStateArityMismatch,

    /// File contains no theorem blocks or a malformed theorem header.
    NotationMissingTheoremBlock,

    /// Tuple syntax is malformed (e.g., unbalanced delimiters, stray comma).
    NotationMalformedTuple,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.line, self.column) {
            (Some(l), Some(c)) => write!(
                f,
                "{:?} at line {} col {}: {}",
                self.exit_reason, l, c, self.message
            ),
            _ => write!(f, "{:?}: {}", self.exit_reason, self.message),
        }
    }
}

impl std::error::Error for ParseError {}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

/// Token kinds emitted by the tokenizer.
///
/// Indentation handling is line-oriented: each physical line begins with an
/// `Indent(spaces)` token, then any non-blank content tokens for that line,
/// followed by a single `Newline`. Blank lines are dropped. Comments
/// (`# ...` to end of line) are dropped.
///
/// Greek-letter handling per the canonical proof notation sec-Notation-Parseability: the
/// tokenizer accepts BOTH the actual Greek codepoints (∅ ⊘ ⊤ Φ ω) AND the
/// ASCII transliterations the smoke + corpus files currently use ("Phi",
/// "omega", "Dead", "Absent", "Unknown"). Both forms produce a `Greek`
/// token carrying the canonical ASCII spelling so downstream consumers
/// only see one shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// Reserved theorem-block keyword (e.g. `Theorem`, `Preconditions`,
    /// `Soundness`, `Proof`, `obligation`, `property`, `Engines`,
    /// `dispatched`, `Expected`, `Forall`, `forall`, `exists`, `Case`,
    /// `implies`, `in`).
    Keyword(String),
    /// ASCII identifier `[A-Za-z_][A-Za-z0-9_]*`.
    Identifier(String),
    /// Non-negative integer literal.
    Integer(u64),
    /// Double-quoted string literal (escape sequences passed through
    /// verbatim at scaffold-time).
    StringLiteral(String),
    /// Single-char punctuation (`(`, `)`, `[`, `]`, `{`, `}`, `,`, `:`,
    /// `-`, `.`, `;`).
    Punct(char),
    /// Multi-char lattice / logical / map operator. Canonical spellings:
    /// `<=`, `>=`, `=>`, `<=>`, `<->`, `||`, `&&`, `:=`, `|->`, `forall`,
    /// `exists`, `/\`, `\/`.
    LatticeOp(String),
    /// Greek-letter or Greek-equivalent ASCII symbol. The carried string
    /// is the canonical ASCII spelling regardless of input charset
    /// (`empty`, `absent`, `top`, `Phi`, `omega`).
    Greek(String),
    /// ArcVarId form `%N`.
    ArcVarId(u64),
    /// End of one logical line (collapses consecutive blank lines).
    Newline,
    /// Leading-whitespace count for the line that starts at this token.
    /// Emitted at the start of every non-blank line.
    Indent(usize),
    /// End-of-file sentinel — always last token.
    Eof,
}

/// One token plus its 1-indexed source position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionedToken {
    /// Token kind.
    pub token: Token,
    /// 1-indexed line number.
    pub line: usize,
    /// 1-indexed column number (UTF-8 byte column).
    pub column: usize,
}

/// Tokenize `input` into a flat `Vec<PositionedToken>`.
///
/// Always terminates with a `Token::Eof`. Returns `ParseError` on any
/// lexical failure (invalid character, malformed string literal,
/// unterminated multi-char operator).
pub fn tokenize(input: &str) -> Result<Vec<PositionedToken>, ParseError> {
    let mut out: Vec<PositionedToken> = Vec::new();
    let mut line_no: usize = 1;
    let lines: Vec<&str> = input.split('\n').collect();
    for raw_line in lines.iter() {
        // Strip carriage return if present (CRLF tolerance).
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        // Skip comment-only and blank lines entirely.
        let mut content_start = 0;
        let line_bytes = line.as_bytes();
        while content_start < line_bytes.len() && line_bytes[content_start] == b' ' {
            content_start += 1;
        }
        let after_indent = &line[content_start..];
        if after_indent.is_empty() || after_indent.starts_with('#') {
            line_no += 1;
            continue;
        }
        // Emit indent marker once per non-blank line.
        out.push(PositionedToken {
            token: Token::Indent(content_start),
            line: line_no,
            column: 1,
        });
        tokenize_line(after_indent, line_no, content_start + 1, &mut out)?;
        out.push(PositionedToken {
            token: Token::Newline,
            line: line_no,
            column: line.len() + 1,
        });
        line_no += 1;
    }
    out.push(PositionedToken {
        token: Token::Eof,
        line: line_no,
        column: 1,
    });
    Ok(out)
}

/// Tokenize the non-leading-whitespace portion of one line.
fn tokenize_line(
    line: &str,
    line_no: usize,
    col_offset: usize,
    out: &mut Vec<PositionedToken>,
) -> Result<(), ParseError> {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        let col = col_offset + i;
        // Skip intra-line whitespace.
        if b == b' ' || b == b'\t' {
            i += 1;
            continue;
        }
        // Comment to end of line.
        if b == b'#' {
            break;
        }
        // String literal — only when a closing `"` is found on the same
        // logical line. Multi-line / unbalanced quotation marks (which
        // appear in prose-quoted citations in the corpus, e.g. CH-shape
        // line 31-32) degrade to single `Punct('"')` tokens so prose
        // continues to tokenize.
        if b == b'"' {
            if let Some((lit, consumed)) = try_scan_string_literal(&line[i..]) {
                out.push(PositionedToken {
                    token: Token::StringLiteral(lit),
                    line: line_no,
                    column: col,
                });
                i += consumed;
                continue;
            }
            out.push(PositionedToken {
                token: Token::Punct('"'),
                line: line_no,
                column: col,
            });
            i += 1;
            continue;
        }
        // Multi-byte UTF-8 sequence — handle Greek-letter codepoints first.
        if b >= 0x80 {
            let (greek, consumed) = scan_utf8_greek(&line[i..], line_no, col)?;
            out.push(PositionedToken {
                token: Token::Greek(greek),
                line: line_no,
                column: col,
            });
            i += consumed;
            continue;
        }
        // ArcVarId form %N.
        if b == b'%' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            let n: u64 = line[i + 1..j].parse().map_err(|_| ParseError {
                exit_reason: ParseExitReason::NotationUnknownSymbol,
                message: format!("invalid ArcVarId: {}", &line[i..j]),
                line: Some(line_no),
                column: Some(col),
            })?;
            out.push(PositionedToken {
                token: Token::ArcVarId(n),
                line: line_no,
                column: col,
            });
            i = j;
            continue;
        }
        // Integer literal.
        if b.is_ascii_digit() {
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            let n: u64 = line[i..j].parse().map_err(|_| ParseError {
                exit_reason: ParseExitReason::NotationUnknownSymbol,
                message: format!("integer literal out of range: {}", &line[i..j]),
                line: Some(line_no),
                column: Some(col),
            })?;
            out.push(PositionedToken {
                token: Token::Integer(n),
                line: line_no,
                column: col,
            });
            i = j;
            continue;
        }
        // Identifier or keyword.
        if b == b'_' || b.is_ascii_alphabetic() {
            let mut j = i;
            while j < bytes.len()
                && (bytes[j] == b'_' || bytes[j].is_ascii_alphanumeric())
            {
                j += 1;
            }
            let word = &line[i..j];
            let tok = classify_word(word);
            out.push(PositionedToken {
                token: tok,
                line: line_no,
                column: col,
            });
            i = j;
            continue;
        }
        // Multi-char operators (longest match first).
        if let Some((op, consumed)) = scan_multichar_op(&line[i..]) {
            out.push(PositionedToken {
                token: Token::LatticeOp(op.to_string()),
                line: line_no,
                column: col,
            });
            i += consumed;
            continue;
        }
        // Single-char punctuation.
        if is_punct(b as char) {
            out.push(PositionedToken {
                token: Token::Punct(b as char),
                line: line_no,
                column: col,
            });
            i += 1;
            continue;
        }
        return Err(ParseError {
            exit_reason: ParseExitReason::NotationUnknownSymbol,
            message: format!("unrecognized character {:?}", b as char),
            line: Some(line_no),
            column: Some(col),
        });
    }
    Ok(())
}

/// Classify an ASCII word into `Keyword`, `Greek`, or `Identifier`.
fn classify_word(word: &str) -> Token {
    // Theorem-block reserved words.
    let keywords = [
        "Theorem",
        "Preconditions",
        "Soundness",
        "property",
        "Proof",
        "obligation",
        "Engines",
        "dispatched",
        "Expected",
        "Case",
        "case_analysis",
        "lattice",
        "monotonicity",
        "fixpoint",
        "structural_induction",
        "refinement",
        "rc_counting",
        "interprocedural_summary",
        "status",
        "reason",
        "implies",
        "and",
        "or",
        "not",
        "in",
        "is",
        "sorry",
    ];
    if keywords.iter().any(|k| *k == word) {
        return Token::Keyword(word.to_string());
    }
    // Greek-equivalent ASCII transliterations the corpus files use.
    let greek_ascii = [
        "Phi", "omega", "Dead", "Absent", "Unknown", "Linear", "Affine",
        "Unrestricted", "Once", "Many", "Unique", "MaybeShared", "Shared",
        "Borrowed", "Owned", "BlockLocal", "FunctionLocal", "ArgEscaping",
        "HeapEscaping", "NonReusable", "CollectionBuffer", "ContextHole",
        "Forall", "forall", "exists",
    ];
    if greek_ascii.iter().any(|k| *k == word) {
        return Token::Greek(word.to_string());
    }
    Token::Identifier(word.to_string())
}

/// Scan a multi-char ASCII operator at the start of `s`. Returns
/// `(op, consumed)` on match. Compares byte-by-byte so it is safe to
/// call when the byte window may contain UTF-8 multibyte sequences
/// (non-match returns `None` without slicing across a char boundary).
fn scan_multichar_op(s: &str) -> Option<(&'static str, usize)> {
    // Longest-match-first table.
    const TABLE: &[(&str, usize)] = &[
        ("|->", 3),
        ("<=>", 3),
        ("<->", 3),
        ("<=", 2),
        (">=", 2),
        ("=>", 2),
        (":=", 2),
        ("||", 2),
        ("&&", 2),
        ("/\\", 2),
        ("\\/", 2),
    ];
    let bytes = s.as_bytes();
    'outer: for (op, n) in TABLE.iter() {
        if bytes.len() < *n {
            continue;
        }
        let op_bytes = op.as_bytes();
        for k in 0..*n {
            if bytes[k] != op_bytes[k] {
                continue 'outer;
            }
        }
        return Some((op, *n));
    }
    None
}

/// Punctuation characters that emit `Token::Punct(c)`. Includes the
/// ASCII apostrophe (English-prose possessive in theorem-block prose,
/// e.g. "L-6's monotone-transfer") so prose lines tokenize without
/// surfacing `notation_unknown_symbol`.
fn is_punct(c: char) -> bool {
    matches!(
        c,
        '(' | ')' | '[' | ']' | '{' | '}' | ',' | ':' | '.' | ';' | '-' | '+' | '*' | '/' | '<' | '>' | '=' | '|' | '!' | '?' | '\'' | '&' | '@' | '^' | '$' | '~' | '`'
    )
}

/// Scan a double-quoted string literal at the start of `s`. Returns
/// `Some((literal, bytes_consumed))` on success, `None` when no closing
/// quote is found on the same logical line (caller demotes to a Punct).
///
/// Escape handling is intentionally minimal at scaffold-time: only `\"`,
/// `\\`, `\n` are unescaped; everything else passes through. Engine
/// consumers re-parse string bodies when they need richer semantics.
fn try_scan_string_literal(s: &str) -> Option<(String, usize)> {
    debug_assert!(s.starts_with('"'));
    let bytes = s.as_bytes();
    let mut out = String::new();
    let mut i = 1usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            return Some((out, i + 1));
        }
        if b == b'\\' && i + 1 < bytes.len() {
            let nxt = bytes[i + 1];
            match nxt {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'n' => out.push('\n'),
                other => {
                    out.push('\\');
                    out.push(other as char);
                }
            }
            i += 2;
            continue;
        }
        out.push(b as char);
        i += 1;
    }
    None
}

/// Scan a single UTF-8 Greek-codepoint at the start of `s`. Returns
/// `(canonical_ascii_spelling, bytes_consumed)` for the recognized set.
fn scan_utf8_greek(s: &str, line: usize, col: usize) -> Result<(String, usize), ParseError> {
    let first = s.chars().next().ok_or_else(|| ParseError {
        exit_reason: ParseExitReason::NotationUnknownSymbol,
        message: "unexpected end of input after non-ASCII byte".to_string(),
        line: Some(line),
        column: Some(col),
    })?;
    // Map known Greek / lattice codepoints to canonical ASCII spellings.
    let (ascii, _consumed_chars): (&str, usize) = match first {
        '\u{2205}' => ("empty", 1), // ∅
        '\u{2298}' => ("absent", 1), // ⊘
        '\u{22A4}' => ("top", 1), // ⊤
        '\u{03A6}' => ("Phi", 1), // Φ
        '\u{03C9}' => ("omega", 1), // ω
        '\u{2294}' => ("join", 1), // ⊔
        '\u{2293}' => ("meet", 1), // ⊓
        '\u{2264}' => ("le", 1), // ≤
        '\u{2265}' => ("ge", 1), // ≥
        '\u{2227}' => ("and", 1), // ∧
        '\u{2228}' => ("or", 1), // ∨
        '\u{00AC}' => ("not", 1), // ¬
        '\u{27FA}' => ("iff", 1), // ⟺
        '\u{27F9}' => ("implies", 1),// ⟹
        '\u{2194}' => ("biimp", 1), // ↔
        '\u{27E8}' => ("langle", 1), // ⟨
        '\u{27E9}' => ("rangle", 1), // ⟩
        '\u{21A6}' => ("mapsto", 1), // ↦
        '\u{2014}' => ("emdash", 1), // — (treat as decorative; surfaces in titles)
        '\u{2013}' => ("endash", 1), // –
        '\u{00A7}' => ("section", 1),// § (theorem-prose section marker, e.g. "§04A.2")
        '\u{2192}' => ("rarrow", 1), // → (chain-order / call-graph arrow)
        '\u{2260}' => ("ne", 1), // ≠ (not-equal predicate in prose)
        '\u{00D7}' => ("times", 1), // × (cartesian product in prose)
        '\u{03BB}' => ("lambda", 1), // λ (lambda binder in prose)
        '\u{00B2}' => ("sup2", 1), // ² (superscript 2 in citation titles, e.g. "FP²")
        other => {
            // Unknown non-ASCII character — surface a structured error so
            // the FAIL-branch enum can route via notation_unknown_symbol.
            return Err(ParseError {
                exit_reason: ParseExitReason::NotationUnknownSymbol,
                message: format!("unrecognized non-ASCII character U+{:04X}", other as u32),
                line: Some(line),
                column: Some(col),
            });
        }
    };
    Ok((ascii.to_string(), first.len_utf8()))
}

// ---------------------------------------------------------------------------
// Token stream + block parser
// ---------------------------------------------------------------------------

/// Cursor over a flat token stream produced by `tokenize`.
struct Cursor<'a> {
    tokens: &'a [PositionedToken],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(tokens: &'a [PositionedToken]) -> Self {
        Cursor { tokens, pos: 0 }
    }
    fn peek(&self) -> &PositionedToken {
        &self.tokens[self.pos]
    }
    fn peek_at(&self, offset: usize) -> &PositionedToken {
        let idx = (self.pos + offset).min(self.tokens.len() - 1);
        &self.tokens[idx]
    }
    fn bump(&mut self) -> &PositionedToken {
        let t = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }
    fn at_eof(&self) -> bool {
        matches!(self.peek().token, Token::Eof)
    }
    /// Skip Newline tokens (no Indent — caller has just consumed a line).
    fn skip_blank_lines(&mut self) {
        while matches!(self.peek().token, Token::Newline) {
            self.bump();
        }
    }
}

/// Parse a flat token stream into a list of theorem blocks.
///
/// Convenience wrapper around `parse_proof_file_tokens_with_source` that
/// passes an empty source string — callers that need verbatim source-line
/// recovery (e.g. the `Expected: reason:` value) MUST use the
/// `_with_source` variant instead.
pub fn parse_proof_file_tokens(
    tokens: &[PositionedToken],
) -> Result<Vec<Theorem>, ParseError> {
    parse_proof_file_tokens_with_source(tokens, "")
}

/// Parse a flat token stream into a list of theorem blocks, carrying the
/// raw source string for positionally-sensitive value recovery.
///
/// The `Expected:` block's `reason:` value is reconstructed from raw
/// source lines (via the carried `source`) rather than from re-tokenized
/// text — token-stream reconstruction collapses interior whitespace and
/// adds spaces around punctuation, corrupting strings like
/// `scaffold-time; engine implementations pending`.
pub fn parse_proof_file_tokens_with_source(
    tokens: &[PositionedToken],
    source: &str,
) -> Result<Vec<Theorem>, ParseError> {
    let mut cursor = Cursor::new(tokens);
    let mut theorems: Vec<Theorem> = Vec::new();
    cursor.skip_blank_lines();
    while !cursor.at_eof() {
        let theorem = parse_theorem(&mut cursor, source)?;
        theorems.push(theorem);
        cursor.skip_blank_lines();
    }
    if theorems.is_empty() {
        let pt = &tokens[0];
        return Err(ParseError {
            exit_reason: ParseExitReason::NotationMissingTheoremBlock,
            message: "file contains no theorem blocks".to_string(),
            line: Some(pt.line),
            column: Some(pt.column),
        });
    }
    Ok(theorems)
}

/// Parse one full theorem block (header + the four sub-blocks).
fn parse_theorem(cursor: &mut Cursor<'_>, source: &str) -> Result<Theorem, ParseError> {
    // Expect line starting with Indent(0) + Keyword("Theorem").
    let header_line_indent = expect_indent(cursor)?;
    if header_line_indent != 0 {
        let pt = cursor.peek();
        return Err(ParseError {
            exit_reason: ParseExitReason::NotationMissingTheoremBlock,
            message: format!(
                "expected theorem header at column 1; found indent {}",
                header_line_indent
            ),
            line: Some(pt.line),
            column: Some(pt.column),
        });
    }
    let (id, name) = parse_theorem_header(cursor)?;
    // Theorem header is one logical line.
    expect_newline(cursor)?;

    // Initialize sub-block holders. Each sub-block lives at indent 2.
    let mut preconditions: Option<Preconditions> = None;
    let mut soundness: Option<SoundnessProperty> = None;
    let mut obligation: Option<ProofObligation> = None;
    let mut expected_status: Option<String> = None;
    let mut expected_reason: Option<String> = None;

    while !cursor.at_eof() {
        cursor.skip_blank_lines();
        if cursor.at_eof() {
            break;
        }
        let indent = match peek_indent(cursor) {
            Some(n) => n,
            None => break,
        };
        // A new theorem header (indent 0) terminates this block.
        if indent == 0 {
            break;
        }
        if indent != 2 {
            let pt = cursor.peek_at(1);
            return Err(ParseError {
                exit_reason: ParseExitReason::NotationMissingTheoremBlock,
                message: format!(
                    "expected theorem sub-block at indent 2; found indent {}",
                    indent
                ),
                line: Some(pt.line),
                column: Some(pt.column),
            });
        }
        // Consume the Indent(2) marker; inspect first content token to
        // decide which sub-block this is.
        let _ = expect_indent(cursor)?;
        let sub_name = read_sub_block_header(cursor)?;
        expect_newline(cursor)?;
        match sub_name.as_str() {
            "Preconditions" => {
                preconditions = Some(parse_preconditions(cursor)?);
            }
            "Soundness property" => {
                soundness = Some(parse_soundness(cursor)?);
            }
            "Proof obligation" => {
                obligation = Some(parse_proof_obligation(cursor)?);
            }
            "Expected" => {
                let (s, r) = parse_expected(cursor, source)?;
                expected_status = Some(s);
                expected_reason = Some(r);
            }
            other => {
                let pt = cursor.peek();
                return Err(ParseError {
                    exit_reason: ParseExitReason::NotationMissingTheoremBlock,
                    message: format!("unrecognized sub-block {:?}", other),
                    line: Some(pt.line),
                    column: Some(pt.column),
                });
            }
        }
    }

    let preconditions = preconditions.ok_or_else(|| missing_subblock_err(&id, "Preconditions"))?;
    let soundness = soundness.ok_or_else(|| missing_subblock_err(&id, "Soundness property"))?;
    let obligation = obligation.ok_or_else(|| missing_subblock_err(&id, "Proof obligation"))?;

    let expected = match (expected_status, expected_reason) {
        (Some(status), Some(reason)) => Some(ExpectedOutcome { status, reason }),
        (Some(status), None) => Some(ExpectedOutcome { status, reason: String::new() }),
        (None, Some(reason)) => Some(ExpectedOutcome { status: String::new(), reason }),
        (None, None) => None,
    };

    Ok(Theorem {
        id,
        name,
        preconditions,
        soundness,
        obligation,
        expected,
    })
}

/// Read a theorem sub-block header (`Preconditions`, `Soundness property`,
/// `Proof obligation`, `Expected`) up to and including its trailing `:`.
/// Returns the canonical header name (without the colon).
fn read_sub_block_header(cursor: &mut Cursor<'_>) -> Result<String, ParseError> {
    let first = cursor.bump().clone();
    let name = match &first.token {
        Token::Keyword(k) | Token::Identifier(k) => k.clone(),
        Token::Greek(k) => k.clone(),
        other => {
            return Err(ParseError {
                exit_reason: ParseExitReason::NotationMissingTheoremBlock,
                message: format!("expected sub-block name, found {:?}", other),
                line: Some(first.line),
                column: Some(first.column),
            })
        }
    };
    // Two-word headers: `Soundness property`, `Proof obligation`.
    let extended = match name.as_str() {
        "Soundness" => {
            expect_keyword_or_ident(cursor, "property")?;
            "Soundness property".to_string()
        }
        "Proof" => {
            expect_keyword_or_ident(cursor, "obligation")?;
            "Proof obligation".to_string()
        }
        _ => name,
    };
    // Trailing colon.
    expect_punct(cursor, ':')?;
    Ok(extended)
}

// ---------------------------------------------------------------------------
// Theorem header
// ---------------------------------------------------------------------------

/// Parse `Theorem <CAT>-<N>[+ <CAT>-<N> ...]: <name>` and return the
/// FIRST theorem id + the full free-form name string (which may contain
/// additional rule references after the first id).
fn parse_theorem_header(cursor: &mut Cursor<'_>) -> Result<(TheoremId, String), ParseError> {
    expect_keyword(cursor, "Theorem")?;
    let id = parse_theorem_id(cursor)?;
    // Optional `+ <CAT>-<N>` siblings — captured into the name suffix so
    // the AST keeps the first id (consumers route per first prefix) plus
    // a human-readable name carrying every id mentioned.
    let mut sibling_ids: Vec<String> = Vec::new();
    while matches!(cursor.peek().token, Token::Punct('+')) {
        cursor.bump();
        let sib = parse_theorem_id(cursor)?;
        sibling_ids.push(format!("{}-{}", sib.category.prefix(), sib.suffix));
    }
    expect_punct(cursor, ':')?;
    // Collect every token up to end-of-line into a free-form name string.
    let mut name_parts: Vec<String> = Vec::new();
    while !matches!(cursor.peek().token, Token::Newline | Token::Eof) {
        let t = cursor.bump();
        name_parts.push(token_to_source(&t.token));
    }
    let mut name = name_parts.join(" ").trim().to_string();
    if !sibling_ids.is_empty() {
        // Record sibling ids inside the name so the human-readable form
        // matches the source line (e.g. "IA-1 + IA-7 ...").
        let prefix = format!(
            "{}-{}{}",
            id.category.prefix(),
            id.suffix,
            sibling_ids
                .iter()
                .map(|s| format!(" + {}", s))
                .collect::<String>()
        );
        name = format!("{} : {}", prefix, name);
    }
    Ok((id, name))
}

/// Parse a single `<CAT>-<N>[a-zA-Z]?` token sequence into a `TheoremId`.
fn parse_theorem_id(cursor: &mut Cursor<'_>) -> Result<TheoremId, ParseError> {
    let first = cursor.bump().clone();
    let category_str = match &first.token {
        Token::Identifier(s) | Token::Keyword(s) | Token::Greek(s) => s.clone(),
        other => {
            return Err(ParseError {
                exit_reason: ParseExitReason::NotationMissingTheoremBlock,
                message: format!("expected theorem category prefix, found {:?}", other),
                line: Some(first.line),
                column: Some(first.column),
            })
        }
    };
    let category = category_from_prefix(&category_str).ok_or_else(|| ParseError {
        exit_reason: ParseExitReason::NotationMissingTheoremBlock,
        message: format!("unknown theorem category prefix {:?}", category_str),
        line: Some(first.line),
        column: Some(first.column),
    })?;
    expect_punct(cursor, '-')?;
    // First suffix token: Integer (e.g. CN-1, TF-15) or Identifier / Keyword
    // (e.g. CN-Ordering, CN-Fixpoint-OneRound). Integer suffix accepts
    // optional fused letter (e.g. `15a`, `8a` — lexer splits `15a` into
    // Integer(15) + Identifier("a")).
    let first_suffix_tok = cursor.bump().clone();
    let mut suffix = match &first_suffix_tok.token {
        Token::Integer(n) => {
            let mut s = n.to_string();
            if let Token::Identifier(letters) = &cursor.peek().token {
                let next = cursor.peek();
                let expected_col = first_suffix_tok.column + n.to_string().len();
                if next.line == first_suffix_tok.line && next.column == expected_col {
                    s.push_str(letters);
                    cursor.bump();
                }
            }
            s
        }
        Token::Identifier(s) | Token::Keyword(s) | Token::Greek(s) => s.clone(),
        other => {
            return Err(ParseError {
                exit_reason: ParseExitReason::NotationMissingTheoremBlock,
                message: format!(
                    "expected theorem number or identifier suffix, found {:?}",
                    other
                ),
                line: Some(first_suffix_tok.line),
                column: Some(first_suffix_tok.column),
            });
        }
    };
    // Compound suffix chain: `-IDENT[-IDENT...]` (e.g. `CN-4-REMOVED`,
    // `CN-Fixpoint-OneRound`, `CN-6-Extraction-Reconciliation`). Stops at
    // `:` (header terminator) or `+` (multi-id sibling).
    while matches!(cursor.peek().token, Token::Punct('-')) {
        cursor.bump();
        let part_tok = cursor.bump().clone();
        let part = match &part_tok.token {
            Token::Identifier(s) | Token::Keyword(s) | Token::Greek(s) => s.clone(),
            Token::Integer(n) => n.to_string(),
            other => {
                return Err(ParseError {
                    exit_reason: ParseExitReason::NotationMissingTheoremBlock,
                    message: format!(
                        "expected identifier in compound theorem suffix, found {:?}",
                        other
                    ),
                    line: Some(part_tok.line),
                    column: Some(part_tok.column),
                });
            }
        };
        suffix.push('-');
        suffix.push_str(&part);
    }
    Ok(TheoremId { category, suffix })
}

/// Map the ASCII prefix to a `Category` variant.
fn category_from_prefix(s: &str) -> Option<Category> {
    Some(match s {
        "L" => Category::Lattice,
        "CN" => Category::Canonicalization,
        "TF" => Category::TransferFunction,
        "DP" => Category::DecisionPredicate,
        "IC" => Category::InterproceduralContract,
        "IA" => Category::IntraproceduralAnalysis,
        "PL" => Category::Pipeline,
        "RL" => Category::Realization,
        "VF" => Category::VerificationLayer,
        "CH" => Category::CoexistenceHandshake,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Per-sub-block parsers
// ---------------------------------------------------------------------------

/// Parse a `Preconditions:` block: every line at indent 4 is either a
/// fresh bullet starting with `-` or a continuation of the most recent
/// bullet (continuation lines are joined into the bullet string with a
/// single space separator).
fn parse_preconditions(cursor: &mut Cursor<'_>) -> Result<Preconditions, ParseError> {
    let mut items: Vec<String> = Vec::new();
    loop {
        cursor.skip_blank_lines();
        let indent = match peek_indent(cursor) {
            Some(n) => n,
            None => break,
        };
        if indent < 4 {
            break;
        }
        let _ = expect_indent(cursor)?;
        // Bullet?
        if matches!(cursor.peek().token, Token::Punct('-')) {
            cursor.bump();
            let text = read_line_text(cursor);
            items.push(text);
        } else {
            // Continuation of the most recent bullet — extend if any.
            let text = read_line_text(cursor);
            if let Some(last) = items.last_mut() {
                last.push(' ');
                last.push_str(text.trim_start());
            } else {
                items.push(text);
            }
        }
        expect_newline(cursor)?;
    }
    Ok(Preconditions { items })
}

/// Parse a `Soundness property:` block: every line at indent 4+ is joined
/// (continuation-friendly) into the property source string.
fn parse_soundness(cursor: &mut Cursor<'_>) -> Result<SoundnessProperty, ParseError> {
    let body = read_indented_body(cursor, 4);
    Ok(SoundnessProperty { source: body })
}

/// Parse a `Proof obligation:` block. The body is free-form prose; nested
/// `Engines dispatched:` sub-blocks (indent 4) are captured but folded
/// into the body's verbatim source.
///
/// Returns `ProofObligation::Sorry` when the body equals the keyword
/// `sorry` (per the canonical proof notation sec-Theorem-Statement-Format); else
/// `ProofObligation::Steps` carrying a single opaque step whose `source`
/// is the verbatim body.
fn parse_proof_obligation(
    cursor: &mut Cursor<'_>,
) -> Result<ProofObligation, ParseError> {
    let body = read_indented_body(cursor, 4);
    let trimmed = body.trim();
    if trimmed.eq_ignore_ascii_case("sorry") {
        return Ok(ProofObligation::Sorry);
    }
    let steps = vec![ProofStep { source: body }];
    Ok(ProofObligation::Steps(steps))
}

/// Parse an `Expected:` block. Each line at indent 4 has form
/// `key: value` (e.g. `status: unimplemented_engine_shape`). Returns the
/// `status` and `reason` values; missing keys default to empty string.
///
/// Recovers the value verbatim from the raw `source` string (rather than
/// re-tokenizing) so strings like `scaffold-time; engine implementations
/// pending` survive intact — `read_line_text` would split on punctuation
/// and re-join with spaces.
fn parse_expected(
    cursor: &mut Cursor<'_>,
    source: &str,
) -> Result<(String, String), ParseError> {
    let mut status = String::new();
    let mut reason = String::new();
    loop {
        cursor.skip_blank_lines();
        let indent = match peek_indent(cursor) {
            Some(n) => n,
            None => break,
        };
        if indent < 4 {
            break;
        }
        let _ = expect_indent(cursor)?;
        // Read leading key keyword/identifier, then `:`, then the rest of
        // the line as the value.
        let first = cursor.bump().clone();
        let key = match &first.token {
            Token::Keyword(s) | Token::Identifier(s) => s.clone(),
            other => {
                return Err(ParseError {
                    exit_reason: ParseExitReason::NotationMissingTheoremBlock,
                    message: format!("expected expected-block key, found {:?}", other),
                    line: Some(first.line),
                    column: Some(first.column),
                })
            }
        };
        expect_punct(cursor, ':')?;
        let line_no = first.line;
        // Skip tokens on this line; we recover the value from raw source.
        while !matches!(cursor.peek().token, Token::Newline | Token::Eof) {
            cursor.bump();
        }
        expect_newline(cursor)?;
        let value = match raw_value_after_colon(source, line_no) {
            Some(v) => v,
            None => String::new(),
        };
        match key.as_str() {
            "status" => status = value.trim().to_string(),
            "reason" => reason = value.trim().to_string(),
            _ => {
                // Unknown key — preserve at scaffold-time but do not fail.
            }
        }
    }
    Ok((status, reason))
}

/// Extract everything after the first `:` on 1-indexed `line_no` of
/// `source` (verbatim — no whitespace collapsing). Returns `None` when
/// the source is empty (parse_proof_file_tokens convenience wrapper) or
/// the line lacks a `:`.
fn raw_value_after_colon(source: &str, line_no: usize) -> Option<String> {
    if source.is_empty() {
        return None;
    }
    let line = source.split('\n').nth(line_no.checked_sub(1)?)?;
    let stripped = line.strip_suffix('\r').unwrap_or(line);
    let colon = stripped.find(':')?;
    Some(stripped[colon + 1..].to_string())
}

// ---------------------------------------------------------------------------
// Token-stream helpers
// ---------------------------------------------------------------------------

/// Return the leading-whitespace count for the next non-blank line.
/// Returns `None` when the stream is at `Eof` — callers exit their
/// indent-driven loop on `None` rather than treating "no more lines" as
/// "indent satisfies the threshold".
fn peek_indent(cursor: &Cursor<'_>) -> Option<usize> {
    match &cursor.peek().token {
        Token::Indent(n) => Some(*n),
        Token::Eof => None,
        _ => Some(0),
    }
}

/// Consume an `Indent(n)` token; error otherwise.
fn expect_indent(cursor: &mut Cursor<'_>) -> Result<usize, ParseError> {
    let pt = cursor.peek().clone();
    match pt.token {
        Token::Indent(n) => {
            cursor.bump();
            Ok(n)
        }
        Token::Eof => Err(ParseError {
            exit_reason: ParseExitReason::NotationMissingTheoremBlock,
            message: "unexpected end of input; expected indent".to_string(),
            line: Some(pt.line),
            column: Some(pt.column),
        }),
        other => Err(ParseError {
            exit_reason: ParseExitReason::NotationMissingTheoremBlock,
            message: format!("expected line-start indent, found {:?}", other),
            line: Some(pt.line),
            column: Some(pt.column),
        }),
    }
}

/// Consume a `Newline` token; error otherwise.
fn expect_newline(cursor: &mut Cursor<'_>) -> Result<(), ParseError> {
    let pt = cursor.peek().clone();
    match pt.token {
        Token::Newline => {
            cursor.bump();
            Ok(())
        }
        Token::Eof => Ok(()),
        other => Err(ParseError {
            exit_reason: ParseExitReason::NotationMissingTheoremBlock,
            message: format!("expected newline, found {:?}", other),
            line: Some(pt.line),
            column: Some(pt.column),
        }),
    }
}

/// Consume a specific keyword token; error otherwise.
fn expect_keyword(cursor: &mut Cursor<'_>, want: &str) -> Result<(), ParseError> {
    let pt = cursor.peek().clone();
    match pt.token {
        Token::Keyword(ref k) if k == want => {
            cursor.bump();
            Ok(())
        }
        other => Err(ParseError {
            exit_reason: ParseExitReason::NotationMissingTheoremBlock,
            message: format!("expected keyword {:?}, found {:?}", want, other),
            line: Some(pt.line),
            column: Some(pt.column),
        }),
    }
}

/// Consume a keyword OR an identifier whose text equals `want`.
fn expect_keyword_or_ident(cursor: &mut Cursor<'_>, want: &str) -> Result<(), ParseError> {
    let pt = cursor.peek().clone();
    match pt.token {
        Token::Keyword(ref k) | Token::Identifier(ref k) if k == want => {
            cursor.bump();
            Ok(())
        }
        other => Err(ParseError {
            exit_reason: ParseExitReason::NotationMissingTheoremBlock,
            message: format!("expected {:?}, found {:?}", want, other),
            line: Some(pt.line),
            column: Some(pt.column),
        }),
    }
}

/// Consume a punctuation char; error otherwise.
fn expect_punct(cursor: &mut Cursor<'_>, want: char) -> Result<(), ParseError> {
    let pt = cursor.peek().clone();
    match pt.token {
        Token::Punct(c) if c == want => {
            cursor.bump();
            Ok(())
        }
        other => Err(ParseError {
            exit_reason: ParseExitReason::NotationMalformedTuple,
            message: format!("expected {:?}, found {:?}", want, other),
            line: Some(pt.line),
            column: Some(pt.column),
        }),
    }
}

/// Read the remaining tokens on the current line (up to the next
/// `Newline` or `Eof`) and reconstruct them into a single string with
/// single-space separators.
fn read_line_text(cursor: &mut Cursor<'_>) -> String {
    let mut parts: Vec<String> = Vec::new();
    while !matches!(cursor.peek().token, Token::Newline | Token::Eof) {
        let t = cursor.bump();
        parts.push(token_to_source(&t.token));
    }
    parts.join(" ")
}

/// Read every line whose indent is >= `min_indent` into a multi-line
/// string. Each line is re-emitted with its indent (spaces) preserved
/// relative to `min_indent` so the body source is human-readable.
fn read_indented_body(cursor: &mut Cursor<'_>, min_indent: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    loop {
        // Allow blank-line gaps inside a block.
        while matches!(cursor.peek().token, Token::Newline) {
            cursor.bump();
            lines.push(String::new());
        }
        let indent = match peek_indent(cursor) {
            Some(n) => n,
            None => break,
        };
        if indent < min_indent {
            break;
        }
        let n = match expect_indent(cursor) {
            Ok(n) => n,
            Err(_) => break,
        };
        let prefix_spaces = n.saturating_sub(min_indent);
        let text = read_line_text(cursor);
        let _ = expect_newline(cursor);
        let mut line = " ".repeat(prefix_spaces);
        line.push_str(&text);
        lines.push(line);
    }
    // Trim trailing blank-line accumulator.
    while lines.last().map(|s| s.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines.join("\n")
}

/// Lossy reconstruction of a `Token` into its source form. Used when
/// re-emitting free-form lines into the theorem AST source fields.
fn token_to_source(t: &Token) -> String {
    match t {
        Token::Keyword(s) | Token::Identifier(s) | Token::Greek(s) => s.clone(),
        Token::Integer(n) => n.to_string(),
        Token::StringLiteral(s) => format!("\"{}\"", s),
        Token::Punct(c) => c.to_string(),
        Token::LatticeOp(s) => s.clone(),
        Token::ArcVarId(n) => format!("%{}", n),
        Token::Newline => "\n".to_string(),
        Token::Indent(_) | Token::Eof => String::new(),
    }
}

/// Build a `ParseError` for a missing required sub-block under a theorem.
fn missing_subblock_err(id: &TheoremId, sub_block: &str) -> ParseError {
    ParseError {
        exit_reason: ParseExitReason::NotationMissingTheoremBlock,
        message: format!(
            "theorem {}-{} missing required sub-block {:?}",
            id.category.prefix(),
            id.suffix,
            sub_block
        ),
        line: None,
        column: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;

    fn corpus_root() -> PathBuf {
        // CARGO_MANIFEST_DIR points at aims-proof/checker/. Walk up two
        // levels to reach the aims-proof/ workspace root.
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.pop();
        p
    }

    #[test]
    fn tokenize_minimal_theorem_header() {
        let toks = tokenize("Theorem L-1: Join commutativity\n").unwrap();
        // Indent(0), Keyword("Theorem"), Identifier("L"), Punct('-'),
        // Integer(1), Punct(':'), Identifier("Join"), Identifier("commutativity"),
        // Newline, Eof.
        assert!(matches!(toks[0].token, Token::Indent(0)));
        assert!(matches!(toks[1].token, Token::Keyword(ref k) if k == "Theorem"));
        assert!(matches!(toks[2].token, Token::Identifier(ref s) if s == "L"));
        assert!(matches!(toks[3].token, Token::Punct('-')));
        assert!(matches!(toks[4].token, Token::Integer(1)));
        assert!(matches!(toks[5].token, Token::Punct(':')));
        assert!(matches!(toks[6].token, Token::Identifier(ref s) if s == "Join"));
        assert!(matches!(toks[7].token, Token::Identifier(ref s) if s == "commutativity"));
        assert!(matches!(toks[8].token, Token::Newline));
        assert!(matches!(toks.last().unwrap().token, Token::Eof));
    }

    #[test]
    fn tokenize_recognizes_ascii_transliterations() {
        let toks = tokenize(" s = (O, Dead, 1, u, bl, Phi, {})\n").unwrap();
        let kinds: Vec<&Token> = toks.iter().map(|p| &p.token).collect();
        // We expect "Dead", "Phi" to be Greek tokens; "O", "u", "bl" stay Identifier.
        assert!(kinds.iter().any(|t| matches!(t, Token::Greek(s) if s == "Dead")));
        assert!(kinds.iter().any(|t| matches!(t, Token::Greek(s) if s == "Phi")));
        assert!(kinds.iter().any(|t| matches!(t, Token::Identifier(s) if s == "O")));
    }

    #[test]
    fn tokenize_recognizes_utf8_greek_codepoints() {
        // ∅, ⊘, ⊤, Φ, ω all map to canonical ASCII spellings.
        let toks = tokenize("(O, ∅, 1, u, bl, Φ, {ω})\n").unwrap();
        let mut found = vec![];
        for p in &toks {
            if let Token::Greek(s) = &p.token {
                found.push(s.clone());
            }
        }
        assert!(found.iter().any(|s| s == "empty"));
        assert!(found.iter().any(|s| s == "Phi"));
        assert!(found.iter().any(|s| s == "omega"));
    }

    #[test]
    fn tokenize_skips_comments_and_blank_lines() {
        let src = "Theorem L-1: foo\n# this is a comment\n\n Preconditions:\n";
        let toks = tokenize(src).unwrap();
        // No Token::Punct('#') and no Indent on the comment line.
        for p in &toks {
            assert!(!matches!(p.token, Token::Punct('#')));
        }
        // Two non-blank lines means exactly two Indent markers.
        let indents: Vec<&Token> = toks.iter().filter(|p| matches!(p.token, Token::Indent(_))).map(|p| &p.token).collect();
        assert_eq!(indents.len(), 2);
    }

    #[test]
    fn tokenize_multichar_operators() {
        let toks = tokenize("a <= b => c <=> d := e |-> f\n").unwrap();
        let ops: Vec<String> = toks
            .iter()
            .filter_map(|p| match &p.token {
                Token::LatticeOp(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(ops, vec!["<=", "=>", "<=>", ":=", "|->"]);
    }

    #[test]
    fn parse_smoke_cn1() {
        let path = corpus_root().join("proofs/00-smoke-test/cn-1-bidirectional.proof");
        let pf = parse_proof_file(&path).expect("CN-1 smoke proof must parse");
        assert_eq!(pf.theorems.len(), 1);
        let th = &pf.theorems[0];
        assert_eq!(th.id.category, Category::Canonicalization);
        assert_eq!(th.id.suffix, "1");
        assert!(th.name.contains("Dead"));
        assert!(th.preconditions.items.len() >= 3);
        assert!(th.soundness.source.contains("Forall"));
        // Body retained the "Engines dispatched:" nested marker verbatim.
        match &th.obligation {
            ProofObligation::Steps(steps) => {
                assert_eq!(steps.len(), 1);
                assert!(steps[0].source.contains("Engines dispatched"));
            }
            ProofObligation::Sorry => panic!("CN-1 should not be `sorry`"),
        }
    }

    #[test]
    fn parse_l_shape_corpus() {
        let path = corpus_root().join("proofs/00-coverage-corpus/L-shape.proof");
        let pf = parse_proof_file(&path).expect("L-shape corpus must parse");
        assert_eq!(pf.theorems.len(), 1);
        let th = &pf.theorems[0];
        assert_eq!(th.id.category, Category::Lattice);
        assert_eq!(th.id.suffix, "1");
        assert!(th.soundness.source.contains("join"));
    }

    #[test]
    fn parse_all_coverage_corpus_files() {
        // Every shape file in the coverage corpus must parse cleanly.
        let dir = corpus_root().join("proofs/00-coverage-corpus");
        let mut count = 0usize;
        for entry in fs::read_dir(&dir).expect("corpus dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("proof") {
                continue;
            }
            let pf = parse_proof_file(&path)
                .unwrap_or_else(|e| panic!("{} failed to parse: {}", path.display(), e));
            assert!(
                !pf.theorems.is_empty(),
                "{} produced zero theorems",
                path.display()
            );
            count += 1;
        }
        assert!(count >= 10, "expected at least 10 corpus files; saw {}", count);
    }

    #[test]
    fn parse_multi_category_theorem_id() {
        // IA-shape carries `Theorem IA-1 + IA-7:` — first id wins; siblings
        // fold into the human-readable name.
        let path = corpus_root().join("proofs/00-coverage-corpus/IA-shape.proof");
        let pf = parse_proof_file(&path).expect("IA-shape corpus must parse");
        let th = &pf.theorems[0];
        assert_eq!(th.id.category, Category::IntraproceduralAnalysis);
        assert_eq!(th.id.suffix, "1");
        assert!(th.name.contains("IA-7"));
    }

    #[test]
    fn parse_letter_suffix_theorem_id() {
        // Construct a synthetic header inline (TF-15a is not in the corpus
        // but the parser must accept it).
        let src = "Theorem TF-15a: synthetic\n Preconditions:\n - foo\n Soundness property:\n bar\n Proof obligation:\n sorry\n";
        let toks = tokenize(src).unwrap();
        let theorems = parse_proof_file_tokens(&toks).expect("TF-15a header must parse");
        assert_eq!(theorems.len(), 1);
        assert_eq!(theorems[0].id.category, Category::TransferFunction);
        assert_eq!(theorems[0].id.suffix, "15a");
        assert!(matches!(theorems[0].obligation, ProofObligation::Sorry));
    }

    #[test]
    fn parse_compound_suffix_theorem_id_numeric_root() {
        // CN-4-REMOVED: integer root + `-IDENT` chain.
        let src = "Theorem CN-4-REMOVED: anti-monotonicity confirmation\n Preconditions:\n - foo\n Soundness property:\n bar\n Proof obligation:\n sorry\n";
        let toks = tokenize(src).unwrap();
        let theorems = parse_proof_file_tokens(&toks)
            .expect("CN-4-REMOVED header must parse");
        assert_eq!(theorems.len(), 1);
        assert_eq!(theorems[0].id.category, Category::Canonicalization);
        assert_eq!(theorems[0].id.suffix, "4-REMOVED");
    }

    #[test]
    fn parse_compound_suffix_theorem_id_identifier_root() {
        // CN-Fixpoint-OneRound: identifier root + `-IDENT` chain.
        let src = "Theorem CN-Fixpoint-OneRound: 1-round empirical convergence\n Preconditions:\n - foo\n Soundness property:\n bar\n Proof obligation:\n sorry\n";
        let toks = tokenize(src).unwrap();
        let theorems = parse_proof_file_tokens(&toks)
            .expect("CN-Fixpoint-OneRound header must parse");
        assert_eq!(theorems.len(), 1);
        assert_eq!(theorems[0].id.category, Category::Canonicalization);
        assert_eq!(theorems[0].id.suffix, "Fixpoint-OneRound");
    }

    #[test]
    fn parse_identifier_only_theorem_id() {
        // CN-Ordering: identifier root, no chain.
        let src = "Theorem CN-Ordering: CN-8 fires before CN-6\n Preconditions:\n - foo\n Soundness property:\n bar\n Proof obligation:\n sorry\n";
        let toks = tokenize(src).unwrap();
        let theorems = parse_proof_file_tokens(&toks)
            .expect("CN-Ordering header must parse");
        assert_eq!(theorems.len(), 1);
        assert_eq!(theorems[0].id.category, Category::Canonicalization);
        assert_eq!(theorems[0].id.suffix, "Ordering");
    }

    #[test]
    fn parse_three_part_compound_suffix() {
        // CN-6-Extraction-Reconciliation: integer root + two `-IDENT` parts.
        let src = "Theorem CN-6-Extraction-Reconciliation: ReturnContract reconciliation\n Preconditions:\n - foo\n Soundness property:\n bar\n Proof obligation:\n sorry\n";
        let toks = tokenize(src).unwrap();
        let theorems = parse_proof_file_tokens(&toks)
            .expect("CN-6-Extraction-Reconciliation header must parse");
        assert_eq!(theorems.len(), 1);
        assert_eq!(theorems[0].id.category, Category::Canonicalization);
        assert_eq!(theorems[0].id.suffix, "6-Extraction-Reconciliation");
    }

    #[test]
    fn parse_error_on_empty_file() {
        let src = "# only a comment\n";
        let toks = tokenize(src).unwrap();
        let err = parse_proof_file_tokens(&toks).unwrap_err();
        assert_eq!(err.exit_reason, ParseExitReason::NotationMissingTheoremBlock);
    }

    #[test]
    fn parse_error_on_unknown_category() {
        let src = "Theorem XX-1: bad\n Preconditions:\n - foo\n Soundness property:\n bar\n Proof obligation:\n sorry\n";
        let toks = tokenize(src).unwrap();
        let err = parse_proof_file_tokens(&toks).unwrap_err();
        assert_eq!(err.exit_reason, ParseExitReason::NotationMissingTheoremBlock);
        assert!(err.message.contains("category"));
    }

    #[test]
    fn parse_error_on_missing_subblock() {
        // Only Preconditions present — Soundness property and Proof obligation missing.
        let src = "Theorem L-1: foo\n Preconditions:\n - bar\n";
        let toks = tokenize(src).unwrap();
        let err = parse_proof_file_tokens(&toks).unwrap_err();
        assert_eq!(err.exit_reason, ParseExitReason::NotationMissingTheoremBlock);
        assert!(err.message.contains("Soundness property"));
    }

    #[test]
    fn parse_error_on_unrecognized_non_ascii() {
        // U+2603 (snowman) is not in the recognized Greek-codepoint table.
        let src = "Theorem L-1: \u{2603}\n Preconditions:\n - foo\n Soundness property:\n bar\n Proof obligation:\n sorry\n";
        let err = tokenize(src).unwrap_err();
        assert_eq!(err.exit_reason, ParseExitReason::NotationUnknownSymbol);
    }

    #[test]
    fn token_position_tracked() {
        let toks = tokenize("Theorem L-1: foo\n").unwrap();
        // First content token should be on line 1, col 1.
        assert_eq!(toks[1].line, 1);
        assert_eq!(toks[1].column, 1);
    }
}
