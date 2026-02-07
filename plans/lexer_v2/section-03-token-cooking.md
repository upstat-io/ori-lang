---
section: "03"
title: Token Cooking & Interning
status: not-started
goal: "Convert raw scanner output to rich TokenKind values with string interning, escape processing, numeric validation, template literal cooking, and TokenFlags"
sections:
  - id: "03.1"
    title: TokenCooker Architecture
    status: not-started
  - id: "03.2"
    title: Identifier Cooking & Keyword Resolution
    status: not-started
  - id: "03.3"
    title: Context-Sensitive Keyword Resolution
    status: not-started
  - id: "03.4"
    title: String Escape Processing
    status: not-started
  - id: "03.5"
    title: Template Literal Cooking
    status: not-started
  - id: "03.6"
    title: Numeric Literal Validation & Parsing
    status: not-started
  - id: "03.7"
    title: TokenFlags During Cooking
    status: not-started
  - id: "03.8"
    title: Span Construction
    status: not-started
  - id: "03.9"
    title: Error Generation
    status: not-started
  - id: "03.10"
    title: Tests
    status: not-started
---

# Section 03: Token Cooking & Interning

**Status:** :clipboard: Planned
**Goal:** Implement the cooking layer that transforms raw scanner output `(RawTag, len)` into rich `TokenKind` values with interned strings, parsed numeric values, validated escape sequences, resolved keywords, template literal segments, and `TokenFlags` metadata.

> **REFERENCE**: Rust's `rustc_parse::lexer` (cooking phase that adds spans, interning, error reporting to raw tokens); Go's keyword hash + segment capture; Zig's deferred literal validation.
>
> **Conventions:** Follows `plans/v2-conventions.md` -- §2 (Tags), §4 (Flags), §5 (Error Shape), §7 (Shared Types in `ori_ir`), §10 (Two-Layer Pattern)

---

## Design Rationale

The cooking layer is the bridge between the allocation-free raw scanner (`ori_lexer_core`) and the semantically rich `TokenKind` used by the parser (`ori_ir`). Its responsibilities:

1. **Keyword resolution**: Raw `Ident` tokens are checked against a keyword table; matching identifiers become keyword-specific `TokenKind` variants.
2. **Context-sensitive keywords**: ~20 soft keywords (pattern/import/type-conversion contexts) are resolved with 1-character lookahead for `(`.
3. **String interning**: Identifier text and string literal content are interned via `StringInterner` to produce `Name` values.
4. **Escape processing**: String literals have their escape sequences validated and unescaped. Template literals have a separate escape set.
5. **Template literal cooking**: Raw template tokens are cooked into `TemplateHead`/`TemplateMiddle`/`TemplateTail` variants with interned segments and processed escapes.
6. **Numeric parsing**: Integer and float literals are parsed from text to `u64`/`f64` values. Duration and size suffixes are validated. Decimal durations (`0.5s`) and decimal sizes (`1.5kb`) are valid per spec.
7. **Span construction**: Raw `(start, len)` pairs are converted to `Span { start, end }`.
8. **TokenFlags**: Whitespace state is tracked and flags (`SPACE_BEFORE`, `NEWLINE_BEFORE`, `ADJACENT`, etc.) are set on each produced token.
9. **Error generation**: Invalid raw tags (unterminated strings, invalid bytes) are converted to diagnostic-quality `LexError` values following the WHERE+WHAT+WHY+HOW shape (v2-conventions SS5).

### Key Design Decision: Eliminate the Dual-Enum Problem

Currently, `RawToken` (in `ori_lexer`) and `TokenKind` (in `ori_ir`) have near-identical variant sets, connected by a 183-line match in `convert.rs`. The V2 design eliminates this:

- `RawTag` (Section 02) uses `#[repr(u8)]` with discriminant values that **align with** `TokenTag` (in `ori_ir`, per v2-conventions SS2, SS7)
- For non-data-carrying tokens (operators, delimiters, keywords), conversion is a zero-cost cast
- Only data-carrying tokens (identifiers, literals, templates) need actual processing
- The cooker produces `TokenKind` values directly; `TokenTag` is derived via `TokenKind::discriminant_index()`

---

## 03.1 TokenCooker Architecture

- [ ] Define `TokenCooker` struct:
  ```rust
  /// Converts raw scanner output into rich TokenKind values.
  ///
  /// The cooker holds references to the source text (for slicing)
  /// and the string interner (for identifier/string interning).
  /// It processes one raw token at a time, producing a TokenKind.
  /// It also tracks whitespace state to set TokenFlags on each token.
  pub struct TokenCooker<'a> {
      source: &'a [u8],
      interner: &'a StringInterner,
      /// Whitespace tracking for TokenFlags
      had_space: bool,
      had_newline: bool,
      had_trivia: bool,
      at_line_start: bool,
  }

  impl<'a> TokenCooker<'a> {
      /// Check if a line comment is a doc comment.
      /// Grammar lines 43-47: doc comments have markers # ! > after //
      /// The existing comment classification (comments.rs) uses:
      ///   # -> DocDescription, @param -> DocParam, @field -> DocField,
      ///   ! -> DocWarning, > -> DocExample
      /// The grammar defines: doc_marker = "*" | "!" | ">" .
      /// The existing codebase also treats # as a doc marker (for descriptions).
      fn is_doc_comment(&self, text: &str) -> bool {
          let content = text.strip_prefix("//").unwrap_or(text);
          let trimmed = content.trim_start();
          trimmed.starts_with('#') ||         // DocDescription: // #Text
          trimmed.starts_with("@param") ||    // DocParam: // @param name desc
          trimmed.starts_with("@field") ||    // DocField: // @field name desc
          trimmed.starts_with('!') ||         // DocWarning: // !Text
          trimmed.starts_with('>')            // DocExample: // >example()
      }
  }
  ```
- [ ] Implement `TokenCooker::cook(&mut self, raw: RawToken, offset: u32) -> (TokenKind, TokenFlags)`:
  - For operators/delimiters/keywords: direct `RawTag` -> `TokenKind` mapping (ideally a transmute or table lookup)
  - For `RawTag::Ident`: slice source text -> keyword check -> context-sensitive keyword check -> intern if not keyword
  - For `RawTag::String`: slice -> unescape -> intern
  - For `RawTag::Char`: slice -> unescape -> validate single char
  - For `RawTag::Int`/`Float`/`HexInt`: slice -> parse value
  - For `RawTag::Duration`/`Size`: slice -> parse value + unit (including decimal forms like `0.5s`, `1.5kb`)
  - For `RawTag::TemplateHead`/`TemplateMiddle`/`TemplateTail`: slice -> process template escapes -> intern
  - For error tags: -> `TokenKind::Error`
  - Compute `TokenFlags` from tracked whitespace state before processing the token
- [ ] Implement the top-level `lex()` function that combines scanner + cooker:
  ```rust
  pub fn lex(source: &str, interner: &StringInterner) -> LexOutput {
      let buffer = SourceBuffer::new(source);
      let mut scanner = RawScanner::new(buffer.cursor());
      // Capacity heuristic: source.len() / 6 + 1 (v2-conventions SS9)
      // Note: V1 uses source.len() / 4 + 1 (ori_lexer/src/lib.rs)
      let mut tokens = TokenList::with_capacity(source.len() / 6 + 1);
      let mut flags_vec: Vec<TokenFlags> = Vec::with_capacity(source.len() / 6 + 1);
      let mut errors: Vec<LexError> = Vec::new();
      let mut cooker = TokenCooker::new(source.as_bytes(), interner);
      let mut offset = 0u32;

      loop {
          let raw = scanner.next();
          match raw.tag {
              RawTag::Whitespace => {
                  cooker.had_space = true;
                  offset += raw.len;
                  continue;
              }
              RawTag::Newline => {
                  cooker.had_newline = true;
                  cooker.at_line_start = true;
                  // V1 COMPATIBILITY: The V1 lexer produces TokenKind::Newline
                  // tokens in the TokenList. The parser's skip_newlines() method
                  // consumes them. If V2 skips newlines here (not emitting them),
                  // the parser's newline handling must be updated. Choose one:
                  // (a) Emit TokenKind::Newline tokens (V1 compatible)
                  // (b) Skip newlines, rely on NEWLINE_BEFORE flag (V2 new behavior)
                  // For backward compatibility, option (a) is safer:
                  // tokens.push(Token::new(TokenKind::Newline, Span::new(offset, offset + raw.len)));
                  // flags_vec.push(cooker.compute_flags());
                  offset += raw.len;
                  continue;
              }
              RawTag::LineComment => {
                  cooker.had_trivia = true;
                  // Ori has only line comments (// ...). Doc comments are line
                  // comments with doc markers (# ! > @param @field) after the
                  // // prefix (grammar lines 43-47, plus codebase extensions).
                  // The scanner produces RawTag::LineComment for all of them;
                  // the cooker inspects the comment text to classify it.
                  // Classification uses the existing CommentKind enum from
                  // ori_ir/src/comment.rs (DocDescription, DocParam, DocField,
                  // DocWarning, DocExample, Regular).
                  let text = &source[offset as usize..(offset + raw.len) as usize];
                  if cooker.is_doc_comment(text) {
                      let (kind, mut flags) = cooker.cook(raw, offset);
                      flags |= TokenFlags::IS_DOC;
                      let span = Span::new(offset, offset + raw.len);
                      tokens.push(Token::new(kind, span));
                      flags_vec.push(flags);
                  }
                  offset += raw.len;
                  continue;
              }
              RawTag::HashBang => {
                  // File attribute: #!identifier(...)
                  // Grammar line 156: file_attribute = "#!" identifier "(" ... ")"
                  // NOTE: The V1 lexer has no HashBang token. It has HashBracket
                  // (for #[) but not #!. This is a NEW token for V2. A new
                  // TokenKind::HashBang variant must be added to ori_ir/src/token.rs.
                  let (kind, flags) = cooker.cook(raw, offset);
                  let span = Span::new(offset, offset + raw.len);
                  tokens.push(Token::new(kind, span));
                  flags_vec.push(flags);
                  offset += raw.len;
              }
              RawTag::Eof => {
                  let flags = cooker.compute_flags();
                  tokens.push(Token::new(TokenKind::Eof, Span::point(offset)));
                  flags_vec.push(flags);
                  break;
              }
              _ => {
                  let (kind, flags) = cooker.cook(raw, offset);
                  let span = Span::new(offset, offset + raw.len);
                  if flags.contains(TokenFlags::HAS_ERROR) {
                      errors.push(cooker.take_pending_error(span));
                  }
                  tokens.push(Token::new(kind, span));
                  flags_vec.push(flags);
                  offset += raw.len;
              }
          }
      }

      LexOutput { tokens, flags: flags_vec, errors }
  }
  ```
  **V1 COMPATIBILITY NOTE**: The existing `LexOutput` (in `ori_lexer/src/lib.rs`) has a different structure: `{ tokens: TokenList, comments: CommentList, blank_lines: Vec<u32>, newlines: Vec<u32> }`. The V2 `LexOutput` adds `flags` and `errors` fields and replaces the V1 comment/position tracking. The existing `lex_with_comments()` function captures comments into a `CommentList` using `classify_and_normalize_comment()` from `comments.rs`. V2 must either:
  (a) Integrate comment capture into the V2 `LexOutput` (adding `comments`, `blank_lines`, `newlines` fields), or
  (b) Provide a separate `lex_with_comments()` wrapper that adds comment capture on top of the V2 core `lex()`.
  The `TokenList::push()` API is compatible -- it accepts `Token::new(kind, span)` and automatically maintains the parallel `tags` array via `token.kind.discriminant_index()`.

---

## 03.2 Identifier Cooking & Keyword Resolution

- [ ] Slice identifier text from source: `&source[offset..offset + len]`
- [ ] Check against keyword table (Section 06 provides the perfect hash):
  ```rust
  fn cook_identifier(&mut self, text: &str) -> TokenKind {
      if let Some(keyword_kind) = keyword::lookup(text) {
          keyword_kind
      } else if let Some(ctx_kind) = self.try_contextual_keyword(text) {
          ctx_kind
      } else {
          let name = self.interner.intern(text);
          TokenKind::Ident(name)
      }
  }
  ```
- [ ] Reserved keywords (grammar § Keywords) that are always resolved:
  `as`, `break`, `continue`, `def`, `div`, `do`, `else`, `extend`, `extension`, `extern`, `false`, `for`, `if`, `impl`, `in`, `let`, `loop`, `match`, `pub`, `self`, `Self`, `suspend`, `tests`, `then`, `trait`, `true`, `type`, `unsafe`, `use`, `uses`, `void`, `where`, `with`, `yield`
  (34 reserved keywords per grammar)
  **V1 COMPATIBILITY**: The V1 lexer and parser also resolve `async`, `return`, `mut`, `dyn`, and `skip` as dedicated `TokenKind` variants (`Async`, `Return`, `Mut`, `Dyn`, `Skip`). The grammar lists `extern`, `suspend`, and `unsafe` as reserved but the V1 codebase does NOT have `TokenKind` variants for these. The V2 cooker must produce all `TokenKind` variants that the parser dispatches on. Specifically:
  - `async` -> `TokenKind::Async` (V1 has it; parser references it)
  - `return` -> `TokenKind::Return` (V1 has it; expression-based language but keyword exists for error messages)
  - `mut` -> `TokenKind::Mut` (V1 has it; parser references it)
  - `dyn` -> `TokenKind::Dyn` (V1 has it; parser uses it for `dyn Trait` syntax)
  - `skip` -> `TokenKind::Skip` (V1 has it; not in grammar but used in codebase)
  - `extern` -> needs new `TokenKind::Extern` (in grammar but missing from V1)
  - `suspend` -> needs new `TokenKind::Suspend` (in grammar but missing from V1)
  - `unsafe` -> needs new `TokenKind::Unsafe` (in grammar but missing from V1)
  When adding `extern`/`suspend`/`unsafe`, add new `TokenKind` variants and TAG constants to `ori_ir/src/token.rs`.
- [ ] Reserved-future keywords that produce a dedicated error:
  `asm`, `inline`, `static`, `union`, `view`
  **NOTE**: These are reserved for future low-level features. The lexer should resolve them as keywords and set a flag; the parser or type checker produces the "reserved for future use" error.
  (5 reserved-future keywords)
- [ ] Preserve ALL existing `TokenKind` variants that the parser dispatches on (see V1 COMPATIBILITY above). The V2 cooker output must be a drop-in replacement for the V1 `convert_token()` output.
- [ ] Type keywords are always resolved (not context-sensitive): `int` -> `TokenKind::IntType`, `float` -> `TokenKind::FloatType`, `bool` -> `TokenKind::BoolType`, `str` -> `TokenKind::StrType`, `char` -> `TokenKind::CharType`, `byte` -> `TokenKind::ByteType`, `Never` -> `TokenKind::NeverType`
- [ ] Constructor keywords are always resolved: `Ok` -> `TokenKind::Ok`, `Err` -> `TokenKind::Err`, `Some` -> `TokenKind::Some`, `None` -> `TokenKind::None`
- [ ] Built-in I/O keywords are always resolved (V1 behavior): `print` -> `TokenKind::Print`, `panic` -> `TokenKind::Panic`, `todo` -> `TokenKind::Todo`, `unreachable` -> `TokenKind::Unreachable`

---

## 03.3 Context-Sensitive Keyword Resolution

> **Grammar reference**: Lines 61-66 define context-sensitive keywords in patterns, imports, type conversion, and other positions.

Context-sensitive keywords (~20) are identifiers that resolve to keyword `TokenKind` variants only when followed by `(`. This enables their use as regular identifiers in non-call positions. The cooker performs 1-character lookahead past whitespace to detect `(`.

- [ ] Implement context-sensitive keyword check:
  ```rust
  /// Check if identifier is a context-sensitive keyword.
  /// Returns keyword TokenKind only if followed by '(' (call position).
  fn try_contextual_keyword(&self, text: &str, offset: u32, len: u32) -> Option<TokenKind> {
      // Only attempt resolution for known soft keywords
      // Grammar lines 61-62: pattern context-sensitive keywords
      let candidate = match text {
          // Pattern context-sensitive keywords (grammar lines 61-62)
          // NOTE: "for", "match", "with" appear in pattern syntax (grammar
          // lines 454-456) but are reserved keywords (lines 57-59), not context-sensitive.
          // Resolution: Reserved keywords are checked FIRST in cook_identifier,
          // so "for"/"match"/"with" always resolve as reserved keywords.
          //
          // V1 COMPATIBILITY NOTE: In V1, the following pattern keywords are
          // ALWAYS resolved as keywords (not context-sensitive):
          //   cache, catch, parallel, spawn, recurse, run, timeout, try, by
          // The parser's soft_keyword_to_name() handles the reverse mapping
          // (keyword -> identifier name) when these appear in non-keyword position.
          // V2 proposes making some of these context-sensitive in the cooker instead.
          //
          // Keywords with existing TokenKind variants (in V1):
          //   cache, catch, parallel, spawn, recurse, run, timeout, try
          // Keywords WITHOUT existing TokenKind variants (no V1 support):
          //   collect, filter, find, fold, map, nursery, retry, validate
          // These would need new TokenKind variants added to ori_ir/src/token.rs
          // if the cooker should produce dedicated keyword tokens for them.
          // Otherwise they should remain as Ident tokens.
          "collect" | "filter" | "find" | "fold" |
          "map" | "nursery" | "retry" | "validate" => true,
          // Import context (not call-gated, handled separately)
          // "without" — resolved in parser, not here
          // Range context
          // "by" — resolved in parser, not here
          // Fixed-capacity list
          // "max" — resolved in parser, not here
          // Type conversion: NOT context-sensitive in V1.
          // V1 always resolves int/float/str/byte/char as type keywords
          // (IntType, FloatType, StrType, ByteType, CharType).
          // Removing them from context-sensitive and keeping as always-keywords
          // matches V1 behavior. See §03.2 type keyword resolution.
          _ => false,
      };

      if !candidate {
          return None;
      }

      // Lookahead: skip horizontal whitespace (space, tab), check for '('
      // Newlines are NOT skipped — they block soft keyword lookahead.
      // This matches Section 06.2's behavior: `cache\n(key)` resolves as Ident,
      // not as a keyword, because newlines are significant in Ori.
      let after = (offset + len) as usize;
      let next_non_ws = self.source[after..]
          .iter()
          .position(|&b| b != b' ' && b != b'\t')
          .map(|i| self.source[after + i]);

      if next_non_ws == Some(b'(') {
          keyword::lookup_contextual(text)
      } else {
          None
      }
  }
  ```
- [ ] Set `TokenFlags::CONTEXTUAL_KW` on tokens resolved this way
- [ ] Context-sensitive keywords that are NOT call-gated (resolved by parser instead):
  - `without` -- only in import items, before `def`
  - `by` -- only after range expressions
  - `max` -- only in fixed-capacity list contexts
- [ ] **Built-in names** (spec 03-lexical-elements.md "Built-in Names" section):
  The spec lists the following as "reserved in call position (`name(`), usable as variables otherwise":
  `len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`, `assert`, `assert_eq`, `assert_ne`, `compare`, `min`, `max`, `print`, `panic`
  Plus the type conversion names already handled above: `int`, `float`, `str`, `byte`.
  **V1 BEHAVIOR**: The V1 lexer resolves `print`, `panic`, `todo`, and `unreachable` as dedicated `TokenKind` variants (`Print`, `Panic`, `Todo`, `Unreachable`) -- they are always-keywords, not context-sensitive. The parser's `soft_keyword_to_name()` maps `Print` -> `"print"` and `Panic` -> `"panic"` to allow their use as identifiers. The remaining built-in names (`len`, `is_empty`, etc.) are emitted as `TokenKind::Ident` and resolved downstream.
  **V2 DECISION**: If V2 keeps backward compatibility, `print`/`panic`/`todo`/`unreachable` must remain as always-keywords (listed in SS03.2). If V2 demotes them to identifiers, the parser's `soft_keyword_to_name()` and `can_start_expr()` must be updated to handle `Ident` tokens for these names.

---

## 03.4 String Escape Processing

> **Grammar reference**: Line 102 -- `escape = '\' ( '"' | '\' | 'n' | 't' | 'r' | '0' ) .`

- [ ] Reuse/refactor the existing `escape.rs` module. Valid string escapes (per spec):
  - `\"` -> double quote (0x22)
  - `\\` -> backslash (0x5C)
  - `\n` -> newline (0x0A)
  - `\t` -> tab (0x09)
  - `\r` -> carriage return (0x0D)
  - `\0` -> null (0x00)
  **NOTE**: The grammar lists these in the order: `"` `\` `n` `t` `r` `0`. This is the authoritative escape set.
- [ ] **No `\'` escape for strings** -- single quote escape is only valid in character literals (grammar line 127: `char_escape = '\' ( "'" | '\' | 'n' | 't' | 'r' | '0' ) .`)
  **V1 DISCREPANCY**: The V1 `resolve_escape()` in `escape.rs` accepts `\'` for ALL contexts (strings and chars). V2 should split string vs char escape validation per grammar.
- [ ] Any other `\X` sequence is an error: produce `LexError` with suggestion "unknown escape sequence `\\X`; valid escapes are `\\\"`, `\\\\`, `\\n`, `\\t`, `\\r`, `\\0`"
  **V1 DISCREPANCY**: The V1 `unescape_string()` preserves invalid escapes literally (e.g., `\q` -> `\q`). V2 should produce a diagnostic error instead.
- [ ] **Excluded by spec** (do NOT implement):
  - `\xHH` hex escapes -- not in grammar
  - `\u{XXXX}` Unicode escapes -- not in grammar
- [ ] Interning strategy:
  - If string has no escape sequences: `interner.intern(&source[start+1..end-1])` (zero-copy intern of source slice)
  - If string has escapes: build unescaped `String`, then `interner.intern_owned(unescaped)` (avoids double allocation)
- [ ] Character literal cooking:
  **Grammar line 127**: `char_escape = '\' ( "'" | '\' | 'n' | 't' | 'r' | '0' ) .`
  - Valid escapes (in grammar order): `\'` `\\` `\n` `\t` `\r` `\0`
  - Validate exactly one character (or one escape sequence) between quotes
  - Return `TokenKind::Char(char_value)`

---

## 03.5 Template Literal Cooking

> **Grammar reference**: Lines 105-122 define template literals with interpolation, escapes, and format specs.

Template literals use backtick delimiters and support interpolation via `{expression}`. The raw scanner (Section 02) produces `RawTag::TemplateHead`, `RawTag::TemplateMiddle`, and `RawTag::TemplateTail` tokens. The cooker processes each segment.

**V1 STATUS**: Template literals are NOT implemented in the V1 lexer. The following `TokenKind` variants must be added to `ori_ir/src/token.rs` before V2 template cooking can be implemented: `TemplateHead(Name)`, `TemplateMiddle(Name)`, `TemplateTail(Name)`, `TemplateFull(Name)`. These will also need TAG constants and `discriminant_index()` entries.

- [ ] Template escape processing -- valid escapes (per grammar line 107):
  **Grammar line 107**: `template_escape = '\' ( '`' | '\' | 'n' | 't' | 'r' | '0' ) .`
  - `` \` `` -> backtick (0x60)
  - `\\` -> backslash (0x5C)
  - `\n` -> newline (0x0A)
  - `\t` -> tab (0x09)
  - `\r` -> carriage return (0x0D)
  - `\0` -> null (0x00)
  **NOTE**: The grammar lists these in the order: backtick, backslash, n, t, r, 0. This is the authoritative escape set for templates.
  - `\"` is NOT a valid template escape (templates use backticks, not double quotes)
- [ ] Literal brace escaping (grammar line 108):
  - `{{` -> literal `{`
  - `}}` -> literal `}`
- [ ] Cook each template segment type:
  ```rust
  /// Cook a template head segment: `` `text{ ``
  fn cook_template_head(&self, text: &[u8]) -> TokenKind {
      // Strip leading backtick and trailing `{`
      let content = &text[1..text.len() - 1];
      let interned = self.process_template_escapes(content);
      TokenKind::TemplateHead(interned)
  }

  /// Cook a template middle segment: `}text{`
  fn cook_template_middle(&self, text: &[u8]) -> TokenKind {
      // Strip leading `}` and trailing `{`
      let content = &text[1..text.len() - 1];
      let interned = self.process_template_escapes(content);
      TokenKind::TemplateMiddle(interned)
  }

  /// Cook a template tail segment: `` }text` ``
  fn cook_template_tail(&self, text: &[u8]) -> TokenKind {
      // Strip leading `}` and trailing backtick
      let content = &text[1..text.len() - 1];
      let interned = self.process_template_escapes(content);
      TokenKind::TemplateTail(interned)
  }

  /// Template with no interpolations: `` `text` ``
  fn cook_template_no_interp(&self, text: &[u8]) -> TokenKind {
      // Strip backticks
      let content = &text[1..text.len() - 1];
      let interned = self.process_template_escapes(content);
      TokenKind::TemplateFull(interned)
  }
  ```
- [ ] Template escape and brace processing:
  ```rust
  fn process_template_escapes(&self, content: &[u8]) -> Name {
      // Fast path: no escapes or brace escapes
      if !content.iter().any(|&b| b == b'\\' || b == b'{' || b == b'}') {
          return self.interner.intern(std::str::from_utf8(content).unwrap());
      }

      // Slow path: build unescaped string
      let mut buf = String::with_capacity(content.len());
      let mut i = 0;
      while i < content.len() {
          match content[i] {
              b'\\' => {
                  i += 1;
                  match content.get(i) {
                      Some(b'n') => buf.push('\n'),
                      Some(b't') => buf.push('\t'),
                      Some(b'r') => buf.push('\r'),
                      Some(b'0') => buf.push('\0'),
                      Some(b'\\') => buf.push('\\'),
                      Some(b'`') => buf.push('`'),  // backtick is 0x60
                      Some(&other) => {
                          // Error: invalid template escape
                          // Set HAS_ERROR flag, push replacement char
                          buf.push(char::REPLACEMENT_CHARACTER);
                      }
                      None => break,
                  }
                  i += 1;
              }
              b'{' if content.get(i + 1) == Some(&b'{') => {
                  buf.push('{');
                  i += 2;
              }
              b'}' if content.get(i + 1) == Some(&b'}') => {
                  buf.push('}');
                  i += 2;
              }
              _ => {
                  buf.push(content[i] as char);
                  i += 1;
              }
          }
      }
      self.interner.intern_owned(buf)
  }
  ```
- [ ] Format spec handling: raw text between `:` and `}` inside interpolations is captured by the raw scanner but **not parsed by the cooker**. The parser handles format spec parsing (grammar lines 114-122). The cooker only interacts with the text segments outside interpolation boundaries.

---

## 03.6 Numeric Literal Validation & Parsing

> **Grammar reference**: Lines 91-97 (int/float), lines 133-144 (duration/size).

- [ ] Integer parsing:
  - Strip underscores from the scanned text
  - Parse **decimal** and **hex (`0x`)** using `u64` checked arithmetic
  - **No binary (`0b`) or octal (`0o`) per grammar** -- grammar line 91 says `int_literal = decimal_lit | hex_lit .` Only two forms are valid per spec.
  - **V1 DISCREPANCY**: The V1 lexer DOES support binary integers (`0b[01][01_]*` -> `RawToken::BinInt(u64)`, converted to `TokenKind::Int(u64)`). This is beyond what the grammar specifies. V2 should follow the grammar and reject binary literals.
  - Overflow -> `LexError` diagnostic (not a panic)
  - Leading zeros in decimal -> implementation choice (current V1 lexer allows them; spec is silent)
- [ ] Float parsing:
  - Strip underscores
  - Parse via `str::parse::<f64>()` (allocates only if underscores present)
  - Store as `u64` bits (`f64::to_bits()`) for `Hash`/`Eq` on `TokenKind::Float`
- [ ] Duration literals (grammar lines 133-137):
  - Parse value + duration unit (`DurationUnit` enum: `Nanoseconds`, `Microseconds`, `Milliseconds`, `Seconds`, `Minutes`, `Hours` -- these are the existing variant names in `ori_ir/src/token.rs`)
  - Integer duration: `100ms`, `2h` -> `TokenKind::Duration(value, unit)`
  - **Decimal duration: `0.5s`, `1.25ms` -> VALID per spec**. Grammar line 135: `duration_literal = ( int_literal | decimal_duration ) duration_unit`. Grammar line 134 note: "Decimal syntax (e.g., 0.5s) is compile-time sugar computed via integer arithmetic"
  - Decimal durations are stored as `TokenKind::Duration` with the float value encoded as bits, or as a separate `TokenKind::DecimalDuration(bits, unit)` variant -- implementation choice. The key point is: **these are NOT errors**.
  - **V1 DISCREPANCY**: The V1 lexer produces `TokenKind::FloatDurationError` for decimal durations (e.g., `1.5s`). The spec says these are valid. V2 fixes this spec violation. The `FloatDurationError` variant in `TokenKind` can be removed or repurposed once V2 replaces V1.
- [ ] Size literals (grammar lines 140-144):
  - Parse value + size unit (`SizeUnit` enum: `Bytes`, `Kilobytes`, `Megabytes`, `Gigabytes`, `Terabytes` -- these are the existing variant names in `ori_ir/src/token.rs`)
  - Integer size: `64kb`, `1gb` -> `TokenKind::Size(value, unit)`
  - **Decimal size: `0.5kb`, `1.5gb` -> VALID per spec**. Grammar line 142: `size_literal = ( int_literal | decimal_size ) size_unit`. Grammar line 141 note: "Decimal syntax (e.g., 1.5kb) is compile-time sugar computed via integer arithmetic"
  - Same storage approach as decimal durations: **these are NOT errors**.
  - **V1 DISCREPANCY**: The V1 lexer produces `TokenKind::FloatSizeError` for decimal sizes (e.g., `1.5kb`). The spec says these are valid. V2 fixes this spec violation.

---

## 03.7 TokenFlags During Cooking

> **Conventions:** v2-conventions SS4 (`TokenFlags` bitfield, `u8` width, semantic bit ranges)

The cooker tracks whitespace state across raw tokens and sets `TokenFlags` on each produced cooked token. This enables the parser to make whitespace-sensitive decisions without re-scanning.

- [ ] Track whitespace state in `TokenCooker`:
  ```rust
  impl TokenCooker<'_> {
      /// Compute flags for the current token based on tracked whitespace state.
      /// Resets the state after computing.
      fn compute_flags(&mut self) -> TokenFlags {
          let mut flags = TokenFlags::empty();

          if self.had_space {
              flags |= TokenFlags::SPACE_BEFORE;
          }
          if self.had_newline {
              flags |= TokenFlags::NEWLINE_BEFORE;
          }
          if self.had_trivia {
              flags |= TokenFlags::TRIVIA_BEFORE;
          }
          if !self.had_space && !self.had_newline && !self.had_trivia {
              flags |= TokenFlags::ADJACENT;
          }
          if self.at_line_start {
              flags |= TokenFlags::LINE_START;
          }

          // Reset for next token
          self.had_space = false;
          self.had_newline = false;
          self.had_trivia = false;
          self.at_line_start = false;

          flags
      }
  }
  ```
- [ ] The first token in a file gets `LINE_START` set
- [ ] `ADJACENT` is set when no whitespace, newline, or trivia preceded the token. This flag is stored per v2-conventions SS4 but **no space-sensitive parsing logic is built around it yet** -- it is available for future use (e.g., `foo(` call syntax vs `foo (` grouping)
- [ ] `HAS_ERROR` is set on tokens where escape processing or numeric parsing encountered an error
- [ ] `IS_DOC` is set on doc comment tokens
- [ ] `CONTEXTUAL_KW` is set on context-sensitive keywords resolved in SS03.3

---

## 03.8 Span Construction

- [ ] Track cumulative byte offset as tokens are produced
- [ ] Construct `Span { start: u32, end: u32 }` from `(offset, offset + raw_token.len)`
- [ ] Handle files > 4GB gracefully:
  - Saturate offsets at `u32::MAX`
  - Emit a diagnostic for files exceeding the limit
- [ ] `Span::DUMMY` (0..0) for synthetic tokens
- [ ] `Span::point(offset)` for zero-length tokens (EOF)

---

## 03.9 Error Generation

> **Conventions:** v2-conventions SS5 -- errors follow WHERE + WHAT + WHY + HOW shape.

- [ ] Define `LexError` following the canonical error shape:
  ```rust
  #[derive(Clone, Debug, Eq, PartialEq, Hash)]
  pub struct LexError {
      pub span: Span,                      // WHERE
      pub kind: LexErrorKind,              // WHAT
      pub context: LexErrorContext,         // WHY
      pub suggestions: Vec<LexSuggestion>, // HOW
  }
  ```
- [ ] Factory methods with `#[cold]` and `#[must_use]` per conventions:
  ```rust
  impl LexError {
      #[cold]
      pub fn unterminated_string(span: Span) -> Self { /* ... */ }

      #[cold]
      pub fn invalid_escape(span: Span, escape_char: char) -> Self { /* ... */ }

      #[cold]
      pub fn integer_overflow(span: Span) -> Self { /* ... */ }

      #[must_use]
      pub fn with_context(mut self, ctx: LexErrorContext) -> Self {
          self.context = ctx;
          self
      }

      #[must_use]
      pub fn with_suggestion(mut self, suggestion: LexSuggestion) -> Self {
          self.suggestions.push(suggestion);
          self
      }
  }
  ```
- [ ] Error kinds include:
  - `UnterminatedString` / `UnterminatedTemplate`
  - `InvalidEscape { found: char }` -- with suggestion listing valid escapes
  - `IntegerOverflow`
  - `InvalidNumericLiteral`
  - `ReservedFutureKeyword` -- for `asm`, `inline`, `static`, `union`, `view`
  - `InvalidCharLiteral`
  - `EmptyCharLiteral`
  - `MultiCharLiteral`
- [ ] All errors have spans; suggestions use imperative verb phrases ("Replace `\\q` with a valid escape sequence")

---

## 03.10 Tests

- [ ] **Round-trip tests**: For every `.ori` file in the test suite, verify that `lex(source, interner)` produces compatible `TokenList` (same kinds, same spans) as the current logos-based lexer, accounting for known intentional differences (see Exit Criteria)
- [ ] **Keyword resolution**: Test all reserved keywords are correctly identified
- [ ] **Context-sensitive keyword tests**:
  **NOTE**: In V1, `cache`/`catch`/`parallel`/`spawn`/`recurse`/`run`/`timeout`/`try`/`by` are always-keywords with dedicated `TokenKind` variants. The V1 parser handles the reverse (keyword-as-ident) via `soft_keyword_to_name()`. There is NO `TokenKind::Map`, `TokenKind::Filter`, etc. in V1. If V2 adds context-sensitive resolution for pattern keywords that lack `TokenKind` variants (`collect`, `filter`, `find`, `fold`, `map`, `nursery`, `retry`, `validate`), new variants must be added to `ori_ir/src/token.rs` first. Tests should cover:
  - V1-style always-keywords: `cache` -> `TokenKind::Cache` (always, regardless of `(`)
  - `try` -> `TokenKind::Try` (always, regardless of `(`)
  - V2 context-sensitive (if new variants added): `collect(` -> keyword; `collect` alone -> ident
  - Type keywords are always-keywords: `int` -> `TokenKind::IntType` (always, NOT context-sensitive)
  - Whitespace between keyword and `(`: `cache  (` -> still `TokenKind::Cache` (always-keyword)
- [ ] **String escape tests**:
  - All valid escapes produce correct characters (grammar line 102 order): `\"` `\\` `\n` `\t` `\r` `\0`
  - Invalid escapes produce `LexError`: `\a`, `\x41`, `\u{0041}`, `\'` (single quote not valid in strings)
  - Strings with no escapes are interned zero-copy
  - Strings with escapes are interned via `intern_owned`
- [ ] **Template literal tests**:
  - Simple template: `` `hello` `` -> `TemplateFull`
  - Single interpolation: `` `hello {name}` `` -> `TemplateHead` + expr + `TemplateTail`
  - Multiple interpolations: `` `{a} and {b}` `` -> Head + expr + Middle + expr + Tail
  - Template escapes (grammar line 107 order): `` \` \\ \n \t \r \0 `` -> correct characters
  - Brace escapes (grammar line 108): `` `{{` `` -> literal `{`; `` `}}` `` -> literal `}`
  - Format specs: `` `{val:>10.2f}` `` -> head/tail with format spec content passed through
  - Invalid template escape: `` `\"` `` -> error (not a valid template escape; double quote doesn't need escaping in templates)
- [ ] **Numeric parsing tests**:
  - Decimal integers (`42`, `1_000_000`)
  - Hex integers (`0xFF`, `0x1a_2b`, `0x1A_2B`)
  - Float with exponent (`1.5e10`, `3.14`, `2.5E-8`)
  - Duration literals: `100ms`, `2h`, `0.5s`, `1.25ms` -- **all valid** (grammar lines 135-136 explicitly allow decimal_duration)
  - Size literals: `64kb`, `1gb`, `0.5kb`, `1.5gb` -- **all valid** (grammar lines 142-143 explicitly allow decimal_size)
  - Overflow detection for integers
  - **No binary or octal**: `0b1010` and `0o777` should lex as `0` followed by identifier (grammar line 91 only lists decimal and hex)
  - Verify duration/size units match grammar lines 137, 144 exactly
- [ ] **TokenFlags tests**:
  - `SPACE_BEFORE` set after spaces/tabs
  - `NEWLINE_BEFORE` set after newlines
  - `ADJACENT` set when no whitespace precedes
  - `LINE_START` set on first token of each line
  - `CONTEXTUAL_KW` set on context-sensitive keywords
  - `HAS_ERROR` set on tokens with escape/parse errors
  - `IS_DOC` set on doc comments
- [ ] **Span accuracy**: Verify every token's span exactly covers its source text
- [ ] **Operator keyword tests**:
  - `div` resolves to `TokenKind::Div` (integer division operator)
  - `??` resolves to `TokenKind::DoubleQuestion` (null coalescing; note: V1 uses `DoubleQuestion`, not `NullCoalesce`)
  - `...` resolves to `TokenKind::DotDotDot` (spread; note: V1 uses `DotDotDot`, not `Spread`)
  - `as?` is NOT a single token -- the V1 lexer produces `TokenKind::As` followed by `TokenKind::Question`; the parser combines them (there is no `AsFallible` variant in `TokenKind`)

---

## 03.11 Completion Checklist

- [ ] `token_cooker.rs` module added to `ori_lexer`
- [ ] `cook()` method handles all `RawTag` variants
- [ ] Keyword resolution works for all reserved keywords
- [ ] Context-sensitive keyword resolution with lookahead
- [ ] Escape processing for strings: `\"` `\\` `\n` `\t` `\r` `\0` (grammar line 102)
- [ ] Escape processing for characters: `\'` `\\` `\n` `\t` `\r` `\0` (grammar line 127)
- [ ] Template literal cooking with segment interning
- [ ] Template escapes: `` \` `` `\\` `\n` `\t` `\r` `\0` (grammar line 107) and `{{ }}` (grammar line 108)
- [ ] Numeric parsing for decimal, hex (no binary, no octal)
- [ ] Decimal duration/size accepted as valid
- [ ] `TokenFlags` set during cooking
- [ ] `LexError` follows WHERE+WHAT+WHY+HOW shape
- [ ] Round-trip tests pass (V2 produces compatible output to V1, with known intentional differences documented in Exit Criteria)
- [ ] `cargo t -p ori_lexer` passes

**Exit Criteria:** The cooking layer, combined with the raw scanner from Section 02, produces `TokenList` output compatible with the parser for all test files. Known intentional differences from V1:
- Decimal durations/sizes (`0.5s`, `1.5kb`) produce `TokenKind::Duration`/`TokenKind::Size` instead of `FloatDurationError`/`FloatSizeError`
- Binary integers (`0b1010`) are rejected per grammar (V1 accepts them)
- Invalid escape sequences produce `LexError` diagnostics (V1 preserves them literally)
- `\'` in string literals is an error (V1 accepts it)
- Template literals are new (V1 has no template support)
- `TokenFlags` are new (V1 has no flags)
- New keywords (`extern`, `suspend`, `unsafe`, `HashBang`) may be added

All string interning, escape processing, numeric parsing, template cooking, and flag computation is correct. Error messages follow the v2-conventions SS5 shape.
