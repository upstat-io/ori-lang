---
section: "04"
title: Token Representation & Tag Alignment
status: complete
goal: "Define shared token types (TokenTag, TokenIdx, TokenFlags) in ori_ir and ensure V2 pipeline integrates with existing TokenList and tag-based parser dispatch"
sections:
  - id: "04.1"
    title: "TokenTag in ori_ir"
    status: complete
  - id: "04.2"
    title: "TokenIdx(u32) Typed Index"
    status: complete
  - id: "04.3"
    title: "TokenFlags Bitfield"
    status: complete
  - id: "04.4"
    title: Tag Discriminant Alignment
    status: complete
  - id: "04.5"
    title: TokenList Push Path
    status: complete
  - id: "04.6"
    title: Eliminate Dual-Enum Redundancy
    status: complete
  - id: "04.7"
    title: Tests
    status: complete
---

# Section 04: Token Representation & Tag Alignment

**Status:** :white_check_mark: Complete (2026-02-06)
**Goal:** Define the shared token representation types (`TokenTag`, `TokenIdx`, `TokenFlags`) in `ori_ir` and ensure the V2 scanner/cooker pipeline produces `TokenList` output fully compatible with the existing SoA layout and tag-based parser dispatch, while eliminating the current dual-enum redundancy.

> **Conventions:** Follows `plans/v2-conventions.md` -- SS1 (Index Types), SS2 (Tag/Discriminant Enums), SS4 (Flag Types), SS7 (Shared Types in `ori_ir`), SS10 (Two-Layer Pattern)

---

## Design Rationale

### What Already Exists (and works well)

The current `TokenList` in `ori_ir/src/token.rs`:
```rust
pub struct TokenList {
    tokens: Vec<Token>,          // Full Token { kind: TokenKind (16B), span: Span (8B) } = 24B
    tags: Vec<u8>,               // Parallel discriminant array for O(1) dispatch
}
```

The parser cursor in `ori_parse/src/cursor.rs`:
```rust
pub struct Cursor<'a> {
    tokens: &'a TokenList,
    tags: &'a [u8],              // Direct slice into TokenList.tags for hot-path dispatch
    interner: &'a StringInterner,
    pos: usize,
}
```

This partial SoA already powers:
- `OPER_TABLE[128]` -- static Pratt parser lookup table indexed by tag
- `POSTFIX_BITSET` -- two-u64 bitset for postfix token membership
- Direct tag dispatch in `parse_primary()` -- covers ~95% of common cases
- All hot-path token comparisons in the Pratt loop and postfix loop

**The V2 lexer does NOT replace `TokenList` with a full SoA `TokenStorage`.** The existing `TokenList` with its parallel `tags: Vec<u8>` is retained. What this section addresses is defining the shared types that formalize the tag byte contract, adding `TokenFlags` as a parallel array, and eliminating the dual-enum conversion overhead.

### What This Section Addresses

The V2 lexer introduces:
1. **`TokenTag`** (`ori_ir`): A `#[repr(u8)]` enum that formalizes the tag byte semantics currently implicit in `TokenKind::discriminant_index()`. Shared across phases (v2-conventions SS2, SS7).
2. **`TokenIdx`** (`ori_ir`): A typed `u32` index into token storage (v2-conventions SS1).
3. **`TokenFlags`** (`ori_ir`): A `bitflags!` bitfield set during cooking and stored parallel to tokens (v2-conventions SS4).
4. **Tag alignment**: `RawTag` (in `ori_lexer_core`) discriminant values must match `TokenTag` (in `ori_ir`) for non-data-carrying tokens.
5. **Dual-enum elimination**: The current `RawToken` -> `TokenKind` conversion via `convert.rs` is replaced by the cooker producing `TokenKind` directly.

---

## 04.1 TokenTag in `ori_ir`

> **Conventions:** v2-conventions SS2 (Tag/Discriminant Enums), SS7 (Shared Types in `ori_ir`)

`TokenTag` is the cooked token discriminant, defined in `ori_ir` because it is used by the parser, type checker, and other downstream phases. It maps 1:1 from `TokenKind` *variants* (one `TokenTag` variant per `TokenKind` variant), but uses **new discriminant numbering** with semantic range gaps for future expansion. When `TokenTag` is adopted, `TokenKind::discriminant_index()` and all existing `TAG_*` constants must be updated to match the new `TokenTag` values. This is the single highest-risk migration step -- every tag-dispatch site in the parser (`OPER_TABLE`, `POSTFIX_BITSET`, `parse_primary()`, `check_type_keyword()`, `infix_binding_power()`, `match_unary_op()`, `match_function_exp_kind()`, and all cursor methods using `TAG_*` constants) must be updated atomically.

- [x] Define `TokenTag` as `#[repr(u8)]` enum in `ori_ir`:
  ```rust
  /// Cooked token discriminant. Shared across all compiler phases.
  ///
  /// Defined in `ori_ir` (not `ori_lexer`) per v2-conventions §7.
  /// Maps 1:1 from `TokenKind::discriminant_index()`.
  /// All variants < 128 for `TokenSet` (u128 bitset) compatibility.
  ///
  /// This enum must cover every variant in the current `TokenKind` enum
  /// (116 variants) plus new template literal tags. Cross-reference with
  /// `TokenKind::discriminant_index()` TAG_* constants and the grammar
  /// (`docs/ori_lang/0.1-alpha/spec/grammar.ebnf`).
  #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
  #[repr(u8)]
  pub enum TokenTag {
      // === Literals (0-9) ===
      Ident             = 0,
      Int               = 1,
      Float             = 2,
      String            = 3,
      Char              = 4,
      Duration          = 5,
      Size              = 6,
      TemplateHead      = 7,    // NEW: ` ... { (template string start with interpolation)
      TemplateMiddle    = 8,    // NEW: } ... { (template string middle segment)
      TemplateTail      = 9,    // NEW: } ... ` (template string end after interpolation)
      TemplateComplete  = 10,   // NEW: ` ... ` (template string with NO interpolation)

      // === Keywords — reserved (11-39) ===
      // Grammar lines 56-58: break, continue, def, do, else, extern, false,
      // for, if, impl, in, let, loop, match, pub, self, Self, suspend, then,
      // trait, true, type, unsafe, use, uses, void, where, with, yield
      // NOTE: async, mut, return are NOT in the spec's reserved list but exist
      // in current TokenKind for historical reasons. Retained for V2 compatibility.
      KwAsync           = 11,   // PHANTOM: not in grammar; remove in V3
      KwBreak           = 12,
      KwContinue        = 13,
      KwReturn          = 14,   // PHANTOM: not in grammar (Ori has no return keyword); remove in V3
      KwDef             = 15,
      KwDo              = 16,
      KwElse            = 17,
      KwFalse           = 18,
      KwFor             = 19,
      KwIf              = 20,
      KwImpl            = 21,
      KwIn              = 22,
      KwLet             = 23,
      KwLoop            = 24,
      KwMatch           = 25,
      KwMut             = 26,   // PHANTOM: not in grammar; remove in V3
      KwPub             = 27,
      KwSelf_           = 28,   // self
      KwSelfType        = 29,   // Self
      KwThen            = 30,
      KwTrait           = 31,
      KwTrue            = 32,
      KwType            = 33,
      KwUse             = 34,
      KwUses            = 35,
      KwVoid            = 36,
      KwWhere           = 37,
      KwWith            = 38,
      KwYield           = 39,

      // === Keywords — additional (40-49) ===
      KwTests           = 40,
      KwAs              = 41,
      KwDyn             = 42,
      KwExtend          = 43,
      KwExtension       = 44,
      KwSkip            = 45,
      KwSuspend         = 46,   // spec line 57: reserved keyword (NEW: not yet in current TokenKind)
      KwUnsafe          = 47,   // spec line 57: reserved keyword (NEW: not yet in current TokenKind)
      KwDiv             = 48,   // spec line 69: integer division operator keyword
      KwExtern          = 49,   // spec line 57: reserved keyword (NEW: not yet in current TokenKind)

      // === Type keywords (50-56) ===
      KwIntType         = 50,   // int
      KwFloatType       = 51,   // float
      KwBoolType        = 52,   // bool
      KwStrType         = 53,   // str
      KwCharType        = 54,   // char
      KwByteType        = 55,   // byte
      KwNeverType       = 56,   // Never

      // === Result/Option constructors (57-60) ===
      KwOk              = 57,
      KwErr             = 58,
      KwSome            = 59,
      KwNone            = 60,

      // === Pattern keywords (61-73) ===
      KwCache           = 61,
      KwCatch           = 62,
      KwParallel        = 63,
      KwSpawn           = 64,
      KwRecurse         = 65,
      KwRun             = 66,
      KwTimeout         = 67,
      KwTry             = 68,
      KwBy              = 69,   // context-sensitive: range step (0..10 by 2)
      KwPrint           = 70,
      KwPanic           = 71,
      KwTodo            = 72,
      KwUnreachable     = 73,
      // gap 74-79 for future keywords

      // === Punctuation & Delimiters (80-99) ===
      HashBracket       = 80,   // #[
      At                = 81,   // @
      Dollar            = 82,   // $
      Hash              = 83,   // #
      LParen            = 84,   // (
      RParen            = 85,   // )
      LBrace            = 86,   // {
      RBrace            = 87,   // }
      LBracket          = 88,   // [
      RBracket          = 89,   // ]
      Colon             = 90,   // :
      ColonColon        = 91,   // ::
      Comma             = 92,   // ,
      Dot               = 93,   // .
      DotDot            = 94,   // ..
      DotDotEq          = 95,   // ..=
      Spread            = 96,   // ...
      Arrow             = 97,   // ->
      FatArrow          = 98,   // =>
      Underscore        = 99,   // _

      // === Operators (100-121) ===
      Pipe              = 100,  // |
      Question          = 101,  // ?
      QuestionQuestion  = 102,  // ??
      Eq                = 103,  // =
      EqEq              = 104,  // ==
      BangEq            = 105,  // !=
      Lt                = 106,  // <
      LtEq              = 107,  // <=
      ShiftLeft         = 108,  // <<
      Gt                = 109,  // >
      GtEq              = 110,  // >= (parser-synthesized: the lexer never emits this
                                //     directly; the parser detects adjacent `>` `=`
                                //     tokens via `is_greater_equal()` and consumes
                                //     both tokens with `consume_compound()`. No tag
                                //     mutation occurs — the tag array still contains
                                //     two separate tags. These `TokenTag` variants
                                //     exist only for `TokenSet` membership tests.)
      ShiftRight        = 111,  // >> (parser-synthesized: same pattern as GtEq — the
                                //     parser detects adjacent `>` `>` via
                                //     `is_shift_right()` and consumes both tokens.
                                //     No tag mutation — never stored in the tag array.)
      Plus              = 112,  // +
      Minus             = 113,  // -
      Star              = 114,  // *
      Slash             = 115,  // /
      Percent           = 116,  // %
      Bang              = 117,  // !
      Tilde             = 118,  // ~
      Ampersand         = 119,  // &
      AndAnd            = 120,  // &&
      PipePipe          = 121,  // ||
      Caret             = 122,  // ^

      // === Special (123-127) ===
      Newline           = 123,  // significant: implicit statement separator (spec line 32: newline)
      Error             = 124,
      FloatDurationErr  = 125,  // error: float with duration suffix (e.g., 1.5s)
      FloatSizeErr      = 126,  // error: float with size suffix (e.g., 1.5kb)
      Eof               = 127,

      // NOTE: All variants are < 128 for TokenSet (u128 bitset) compatibility.
      // This means all token tags (including error tags) can be used in bitset
      // operations. Error tags are simply excluded from parser "expected" sets
      // by convention, not by discriminant value.
  }

  const _: () = assert!(std::mem::size_of::<TokenTag>() == 1);
  ```

  **Note on TokenSet compatibility:** All variants must be < 128 to work with parser
  `TokenSet` membership tests (`1u128 << tag`). This includes error tokens, even though
  error tokens never appear in parser "expected" sets (by convention). With 122 defined
  variants (including `TemplateComplete` at 10 and both float error tags at 125-126),
  the enum fits within the 0-127 range (max discriminant: `Eof` = 127).

  **Coverage of current TokenKind (116 variants):** Every variant in the current
  `TokenKind` enum has a corresponding `TokenTag` variant, except `Semicolon`
  (see note below). Additionally, 3 new variants (`Suspend`, `Unsafe`, `Extern`)
  are added to `TokenTag` that do NOT exist in the current `TokenKind` -- these
  must be added to `TokenKind` as part of the V2 migration:
  - Data-carrying: `Int`, `Float`, `String`, `Char`, `Duration`, `Size`, `Ident`
  - Keywords (spec-aligned, currently in `TokenKind`): `Break`, `Continue`, `Def`,
    `Do`, `Else`, `False`, `For`, `If`, `Impl`, `In`, `Let`, `Loop`, `Match`,
    `Pub`, `SelfLower` (self), `SelfUpper` (Self), `Then`, `Trait`, `True`, `Type`,
    `Use`, `Uses`, `Void`, `Where`, `With`, `Yield`
  - Keywords (spec-reserved, NOT yet in current `TokenKind` -- must be added):
    `Suspend`, `Unsafe`, `Extern`
  - Keywords (additional/tooling): `Tests`, `As`, `Dyn`, `Extend`, `Extension`, `Skip`
  - Keywords (PHANTOM -- not in spec, retained for V2 compatibility, remove in V3):
    `Async`, `Mut`, `Return`
  - Type keywords: `IntType`, `FloatType`, `BoolType`, `StrType`, `CharType`,
    `ByteType`, `NeverType`
  - Constructors: `Ok`, `Err`, `Some`, `None`
  - Pattern keywords: `Cache`, `Catch`, `Parallel`, `Spawn`, `Recurse`, `Run`,
    `Timeout`, `Try`, `By`, `Print`, `Panic`, `Todo`, `Unreachable`
  - Punctuation: `HashBracket`, `At`, `Dollar`, `Hash`, `LParen`, `RParen`,
    `LBrace`, `RBrace`, `LBracket`, `RBracket`, `Colon`, `DoubleColon`, `Comma`,
    `Dot`, `DotDot`, `DotDotEq`, `DotDotDot` (`...`), `Arrow`, `FatArrow`,
    `Underscore`
  - Operators (spec grammar lines 69-74): `Pipe`, `Question`, `DoubleQuestion`,
    `Eq`, `EqEq`, `NotEq` (`!=`), `Lt`, `LtEq`, `Shl` (`<<`), `Gt`,
    `GtEq` (parser-synthesized: `>=`), `Shr` (parser-synthesized: `>>`),
    `Plus`, `Minus`, `Star`, `Slash`, `Percent`, `Bang`, `Tilde`, `Amp`,
    `AmpAmp`, `PipePipe`, `Caret`, `Div`
  - Special: `Newline`, `Eof`, `Error`, `FloatDurationError`, `FloatSizeError`
  - NEW (template literals): `TemplateHead`, `TemplateMiddle`, `TemplateTail`, `TemplateComplete`
  - REMOVED (from current `TokenKind`): `Semicolon` (see note below)

  **Note on Semicolon:** The current `TokenKind` has a `Semicolon` variant at
  discriminant index 89 (the only `TokenKind` variant without a corresponding
  `TokenTag` variant), but **semicolons are not valid Ori syntax** per the spec
  (grammar lines 78-79 list delimiters; semicolon is absent). The V2 pipeline
  cooker converts `RawTag::Semicolon` to `TokenTag::Error` with a diagnostic
  suggesting removal. There is no `Semicolon` or `Semi` tag in `TokenTag` -- any
  semicolon in source is a parse error. This means the V2 `TokenKind` must also
  drop its `Semicolon` variant (bringing the total from 116 to 115 existing
  variants, plus 3 new spec-reserved keywords and 4 template variants = 122).

  **Note on Phantom Keywords (async, mut, return):** These three keywords exist
  in the current `TokenKind` but are NOT in the grammar's reserved keyword list
  (grammar lines 56-58). They are historical artifacts. The V2 lexer retains them
  for compatibility (to avoid breaking existing tests/error messages), but they
  should be removed in V3. The spec is explicit: Ori is expression-based with NO
  `return` keyword (CLAUDE.md: "NO `return` KEYWORD — every block's value is its
  last expression"). `async` and `mut` are similarly absent from the spec.

- [x] Implement `name() -> &'static str` method for debugging/display:
  ```rust
  impl TokenTag {
      pub fn name(self) -> &'static str {
          match self {
              Self::Ident => "identifier",
              Self::Int => "integer literal",
              Self::Float => "float literal",
              Self::String => "string literal",
              Self::Char => "char literal",
              Self::Duration => "duration literal",
              Self::Size => "size literal",
              Self::TemplateHead => "template head",
              Self::TemplateMiddle => "template middle",
              Self::TemplateTail => "template tail",
              Self::TemplateComplete => "template string (no interpolation)",
              Self::KwBreak => "break",
              Self::KwLet => "let",
              Self::KwIf => "if",
              Self::KwFor => "for",
              Self::KwMatch => "match",
              Self::KwDiv => "div",
              Self::Plus => "+",
              Self::Minus => "-",
              Self::EqEq => "==",
              Self::BangEq => "!=",
              Self::LParen => "(",
              Self::RParen => ")",
              Self::LBrace => "{",
              Self::RBrace => "}",
              Self::LBracket => "[",
              Self::RBracket => "]",
              Self::QuestionQuestion => "??",
              Self::Spread => "...",
              Self::Arrow => "->",
              Self::FatArrow => "=>",
              Self::GtEq => ">=",      // parser-synthesized (adjacent `>` `=`)
              Self::ShiftRight => ">>", // parser-synthesized (adjacent `>` `>`)
              Self::Newline => "newline",
              Self::Error => "<error>",
              Self::FloatDurationErr => "float duration error (e.g., 1.5s)",
              Self::FloatSizeErr => "float size error (e.g., 1.5kb)",
              Self::Eof => "<eof>",
              // ... exhaustive match for all other variants
          }
      }
  }
  ```

- [x] Semantic range layout with gaps for future variants:

  | Range | Category | Count | Notes |
  |-------|----------|-------|-------|
  | 0-10 | Literals | 11 | Ident, Int, Float, String, Char, Duration, Size, Template{Head,Middle,Tail,Complete} |
  | 11-79 | Keywords | ~63 | Reserved (11-39, includes 3 phantom kw), additional (40-49), type kw (50-56), constructors (57-60), pattern kw (61-73); gap at 74-79 |
  | 80-99 | Punctuation/Delimiters | 20 | HashBracket, @, $, #, parens, braces, brackets, :, ::, comma, dots, arrows, _ |
  | 100-122 | Operators | 23 | Pipe, ?, ??, =, ==, !=, <, <=, <<, >, >=*, >>*, +, -, *, /, %, !, ~, &, &&, \|\|, ^ |
  | 123-127 | Special | 5 | Newline, Error, FloatDurationErr, FloatSizeErr, Eof |

  All variants are < 128, ensuring `TokenSet` (u128 bitset) compatibility via `1u128 << tag`.
  Error tags (Error, FloatDurationErr, FloatSizeErr) are excluded from parser "expected" sets
  by convention (never listed in error messages), but their discriminants still fit in u7 range.
  Variants marked with `*` (`GtEq`, `ShiftRight`) are parser-synthesized and never produced by the lexer.

  **Spec Alignment Notes:**
  - Grammar line 72: bit operators include `>>` (right shift). The lexer emits adjacent `>` `>`
    tokens; the parser detects adjacency via `is_shift_right()` and consumes both tokens with
    `consume_compound()`. No tag mutation occurs -- the tag array retains two `Gt` tags.
  - Grammar line 70: comparison operators include `>=`. Same adjacency-detection pattern as `>>`,
    via `is_greater_equal()` and `consume_compound()`.
  - Grammar line 74: `other_op` defines `..=` (inclusive range). The lexer emits this directly.
  - NO compound assignment operators (`+=`, `-=`, `*=`, etc.) in the spec. Not included in TokenTag.
  - NO pipe operator (`|>`) in the spec. The `Pipe` tag is for pattern `|` (or-patterns) and
    bitwise OR, not function composition.

  **CRITICAL: Discriminant Renumbering Migration**

  The proposed `TokenTag` discriminant values are **entirely different** from the current
  `TokenKind::discriminant_index()` values. This is intentional (semantic range layout with
  gaps for future expansion), but requires updating ALL of the following atomically:

  1. **`TokenKind::discriminant_index()`** — all 116 match arms must return new values
  2. **All `TAG_*` constants** on `TokenKind` — must match new `TokenTag` discriminants
  3. **`OPER_TABLE[128]`** in `ori_parse/src/grammar/expr/operators.rs` — 18 entries indexed
     by tag value; all indices change
  4. **`POSTFIX_BITSET`** in `ori_parse/src/grammar/expr/postfix.rs` — 7 tag values used for
     bit positions; all change
  5. **`parse_primary()` match arms** in `ori_parse/src/grammar/expr/primary.rs` — ~15 tag
     dispatch arms
  6. **`check_type_keyword()`** in `ori_parse/src/cursor.rs` — uses range
     `TAG_INT_TYPE..=TAG_NEVER_TYPE`; range bounds change
  7. **`friendly_name_from_index()`** in `ori_ir/src/token.rs` — 115 match arms indexed by
     discriminant
  8. **`parse_type()` match arms** in `ori_parse/src/grammar/ty.rs` — 8 tag dispatch arms
  9. **All `TokenSet` constants** in the parser — bit positions shift
  10. **All cursor helper methods** using `TAG_*` constants — `is_at_end()`, `check_ident()`,
      `is_shift_right()`, `is_greater_equal()`, `next_is_lparen()`, `next_is_colon()`,
      `skip_newlines()`, etc.

  Current → proposed value mapping for key tags (showing the scope of change):

  | Variant | Current `discriminant_index()` | Proposed `TokenTag` |
  |---------|-------------------------------|---------------------|
  | Int | 0 | 1 |
  | Ident | 6 | 0 |
  | Break | 8 | 12 |
  | Let | 19 | 23 |
  | IntType | 42 | 50 |
  | LParen | 70 | 84 |
  | Dot | 79 | 93 |
  | Plus | 99 | 112 |
  | Gt | 96 | 109 |
  | Newline | 111 | 123 |
  | Eof | 112 | 127 |
  | Error | 113 | 124 |

  **Migration strategy:** Update `TokenKind::discriminant_index()` and all `TAG_*` constants
  in a single commit. The `TokenTag` enum defines the source of truth; `TAG_*` constants
  become thin aliases for `TokenTag::Variant as u8`. Run `./test-all.sh` to verify no
  tag-dependent parser code is broken.

---

## 04.2 TokenIdx(u32) Typed Index

> **Conventions:** v2-conventions SS1 (Index Types)

`TokenIdx` is defined in `ori_ir` because the parser and other phases reference tokens by index.

- [x] Define `TokenIdx` in `ori_ir`:
  ```rust
  /// Strongly-typed index into token storage.
  ///
  /// Defined in `ori_ir` (cross-phase) per v2-conventions §1.
  #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
  #[repr(transparent)]
  pub struct TokenIdx(u32);

  impl TokenIdx {
      pub const NONE: Self = Self(u32::MAX);

      #[inline]
      pub const fn from_raw(raw: u32) -> Self { Self(raw) }

      #[inline]
      pub const fn raw(self) -> u32 { self.0 }
  }

  const _: () = assert!(std::mem::size_of::<TokenIdx>() == 4);
  ```

- [x] Sentinel value: `NONE = TokenIdx(u32::MAX)` for "no token" positions (e.g., missing optional tokens in the AST)
- [x] Used by the parser cursor as the primary way to refer back to tokens for span retrieval and error messages

---

## 04.3 TokenFlags Bitfield

> **Conventions:** v2-conventions SS4 (Flag Types -- `bitflags!`, semantic bit ranges, domain-appropriate width)

`TokenFlags` captures per-token metadata set by the cooking layer (Section 03). It is stored parallel to tokens and used by the parser for whitespace-sensitive decisions.

- [x] Define `TokenFlags` in `ori_ir` (or `ori_lexer`, depending on whether parser needs it directly -- since the parser does need it, `ori_ir` is appropriate):
  ```rust
  bitflags::bitflags! {
      /// Per-token metadata flags, set during cooking.
      ///
      /// Width: u8 (8 flags sufficient for token metadata).
      /// See v2-conventions §4 for the pattern; TypeFlags uses u32
      /// because types need ~20+ flags.
      #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
      pub struct TokenFlags: u8 {
          // --- Whitespace flags (bits 0-3) ---

          /// Space or tab preceded this token.
          const SPACE_BEFORE   = 1 << 0;
          /// Newline preceded this token.
          const NEWLINE_BEFORE = 1 << 1;
          /// Comment (trivia) preceded this token.
          const TRIVIA_BEFORE  = 1 << 2;
          /// This token is immediately adjacent to the previous token (no
          /// whitespace, newline, or trivia between them). Stored per
          /// v2-conventions §4 but no space-sensitive parsing logic is
          /// built around it yet.
          const ADJACENT       = 1 << 3;

          // --- Position flags (bits 4-5) ---

          /// This token is the first non-trivia token on its line.
          const LINE_START     = 1 << 4;
          /// This token was resolved as a context-sensitive keyword
          /// (Section 03.3).
          const CONTEXTUAL_KW  = 1 << 5;

          // --- Status flags (bits 6-7) ---

          /// An error was encountered while cooking this token
          /// (e.g., invalid escape, numeric overflow).
          const HAS_ERROR      = 1 << 6;
          /// This token is a doc comment.
          const IS_DOC         = 1 << 7;
      }
  }
  ```

- [x] Set by the cooking layer (Section 03.7): the cooker tracks whitespace state and computes flags for each produced token
- [x] Stored parallel to tokens. Two options for storage location:
  1. **In `TokenList`**: Add `flags: Vec<TokenFlags>` alongside `tokens: Vec<Token>` and `tags: Vec<u8>`
  2. **In `LexOutput`**: Store `flags: Vec<TokenFlags>` in the phase output alongside `TokenList`
  - Option 1 is preferred since the parser cursor needs direct access to flags
- [x] Used by parser for newline significance: `flags.contains(TokenFlags::NEWLINE_BEFORE)` replaces ad-hoc newline detection
- [x] `ADJACENT` flag notes: included per v2-conventions SS4. Future use cases include distinguishing `foo(` (call) from `foo (` (grouping), and `x.y` (field access) from `x . y` (operator). No parsing logic depends on it in the initial V2 implementation.

---

## 04.4 Tag Discriminant Alignment

- [x] Audit the current `TokenKind::discriminant_index()` implementation:
  - What are the current tag values for each variant?
  - Are they stable (derived from enum declaration order)?
  - Does the parser depend on specific numeric values?
- [x] Design `RawTag` (in `ori_lexer_core`) discriminant values to align with `TokenTag` (in `ori_ir`) where possible:
  - For fixed-lexeme tokens (operators, delimiters): `RawTag::Plus as u8 == TokenTag::Plus as u8`
  - For keywords: `RawTag::Ident` in the scanner, but the cooker resolves to keyword `TokenTag` variants whose discriminant is already defined in `TokenTag`
  - For data-carrying tokens (identifiers, literals): the cooker produces the `TokenKind` variant, so alignment is automatic
- [x] Verify `TokenSet` (u128 bitset) compatibility:
  - The parser uses `TokenSet::contains(tag)` with `1u128 << tag`
  - This requires all tag values that appear in expected sets to be < 128
  - All `TokenTag` variants (including error tags) are < 128 by construction (see SS04.1 layout: max is `Eof` = 127)
  - `RawTag` can have additional variants (trivia, error subtypes) in the 128-255 range since those never appear in `TokenSet` -- they are internal to the scanner and consumed by the cooker
- [x] Document the tag numbering contract: tag values are derived from `TokenTag` enum order and must remain stable for `TokenSet` bit positions. Adding new variants should use gap slots, not renumber existing variants.

---

## 04.5 TokenList Push Path

- [x] Verify the V2 cooker output path feeds cleanly into `TokenList::push()`:
  ```rust
  // V2 cooking loop (in Section 03's lex() function):
  let (kind, flags) = cooker.cook(raw, offset);
  let span = Span::new(offset, offset + raw.len);
  tokens.push(Token::new(kind, span));
  flags_vec.push(flags);
  // TokenList::push() automatically derives: tags.push(kind.discriminant_index())
  ```
- [x] Maintain the invariant: `tags[i] == tokens[i].kind.discriminant_index()` for all `i`
  - This invariant is already maintained by `TokenList::push()` which derives the tag byte from the `TokenKind`
  - The V2 cooker produces the same `TokenKind` variants the current converter does, so no change to `push()` is needed
- [x] `TokenFlags` stored in a parallel `flags: Vec<TokenFlags>` on `TokenList`:
  ```rust
  pub struct TokenList {
      tokens: Vec<Token>,
      tags: Vec<u8>,
      flags: Vec<TokenFlags>,  // NEW: parallel to tokens/tags
  }
  ```
  - `TokenList::push()` extended to accept flags, or flags pushed separately by `lex()`
  - The cursor gains a `current_flags() -> TokenFlags` method
- [x] Verify capacity heuristic still works: `source.len() / 6 + 1` (v2-conventions SS9) should remain valid since the V2 scanner produces the same number of tokens (whitespace/comments are still skipped in `lex()` mode)
- [x] Verify `TokenList`'s `Eq`/`Hash` implementations work correctly:
  - Currently compares/hashes only `tokens` (tags are derived)
  - Flags should also be compared/hashed since they carry semantic information (e.g., `CONTEXTUAL_KW` affects how the parser treats the token)

---

## 04.6 Eliminate Dual-Enum Redundancy

- [x] Remove the current `raw_token.rs` module (88-variant `RawToken` enum with logos derive)
- [x] Remove the current `convert.rs` module (183-line `convert_token` match)
- [x] The V2 pipeline replaces both:
  - `RawTag` (Section 02) is internal to `ori_lexer_core` -- never stored in `TokenList`, never exposed beyond the cooker
  - The cooker (Section 03) produces `TokenKind` directly
  - No intermediate enum conversion needed
- [x] Verify the dependency chain is clean:
  - `ori_lexer_core` has NO `ori_*` dependencies (v2-conventions SS10)
  - `ori_lexer` depends on `ori_lexer_core` + `ori_ir` for `TokenKind`, `Token`, `TokenList`, `TokenTag`, `TokenFlags`, `Span`, `Name`, `StringInterner`
  - `ori_lexer` no longer depends on `logos`
  - `RawTag` is private to `ori_lexer_core` (not exported beyond the crate)
- [x] Net code reduction estimate: remove ~300 lines (`raw_token.rs` + `convert.rs`), add ~50 lines (`RawTag` enum, simpler than `RawToken` since no logos attributes)

---

## 04.7 Tests

- [x] **Tag stability test**: Verify that key `TokenTag` discriminant values match their expected numeric values. This catches accidental reordering:
  ```rust
  #[test]
  fn token_tag_discriminants_are_stable() {
      // Literals (0-10)
      assert_eq!(TokenTag::Ident as u8, 0);
      assert_eq!(TokenTag::Int as u8, 1);
      assert_eq!(TokenTag::Float as u8, 2);
      assert_eq!(TokenTag::String as u8, 3);
      assert_eq!(TokenTag::TemplateHead as u8, 7);
      assert_eq!(TokenTag::TemplateMiddle as u8, 8);
      assert_eq!(TokenTag::TemplateTail as u8, 9);
      assert_eq!(TokenTag::TemplateComplete as u8, 10);

      // Keywords — reserved (11-39)
      assert_eq!(TokenTag::KwAsync as u8, 11);  // phantom
      assert_eq!(TokenTag::KwBreak as u8, 12);
      assert_eq!(TokenTag::KwReturn as u8, 14); // phantom
      assert_eq!(TokenTag::KwLet as u8, 23);
      assert_eq!(TokenTag::KwMut as u8, 26);    // phantom

      // Keywords — additional (40-49)
      assert_eq!(TokenTag::KwDiv as u8, 48);

      // Type keywords (50-56)
      assert_eq!(TokenTag::KwIntType as u8, 50);

      // Constructors (57-60)
      assert_eq!(TokenTag::KwOk as u8, 57);

      // Pattern keywords (61-73)
      assert_eq!(TokenTag::KwCache as u8, 61);

      // Punctuation (80-99)
      assert_eq!(TokenTag::HashBracket as u8, 80);
      assert_eq!(TokenTag::LParen as u8, 84);

      // Operators (100-122)
      assert_eq!(TokenTag::Pipe as u8, 100);
      assert_eq!(TokenTag::Plus as u8, 112);

      // Special (123-127)
      assert_eq!(TokenTag::Newline as u8, 123);
      assert_eq!(TokenTag::Error as u8, 124);
      assert_eq!(TokenTag::FloatDurationErr as u8, 125);
      assert_eq!(TokenTag::FloatSizeErr as u8, 126);
      assert_eq!(TokenTag::Eof as u8, 127);
  }
  ```
- [x] **All tags < 128**: Verify every `TokenTag` variant fits in the `TokenSet` bitset (0-127 range):
  ```rust
  #[test]
  fn all_token_tags_fit_in_token_set() {
      // TokenSet uses u128 bitset: 1u128 << tag requires tag < 128
      // This includes error tags (Error, FloatDurationErr, FloatSizeErr)
      // even though they never appear in parser "expected" sets
      for &tag in &[
          TokenTag::Ident, TokenTag::Int, /* ... all variants ... */,
          TokenTag::Error, TokenTag::FloatDurationErr, TokenTag::FloatSizeErr,
          TokenTag::Eof,
      ] {
          assert!((tag as u8) < 128, "{:?} = {} >= 128", tag, tag as u8);
      }
      // Alternatively, if TokenTag implements an ALL constant or iterator:
      // for tag in TokenTag::iter() {
      //     assert!((tag as u8) < 128, "{:?} = {} >= 128", tag, tag as u8);
      // }
  }
  ```
- [x] **TokenIdx size**: `assert!(size_of::<TokenIdx>() == 4)`
- [x] **TokenFlags size**: `assert!(size_of::<TokenFlags>() == 1)`
- [x] **TokenTag size**: `assert!(size_of::<TokenTag>() == 1)`
- [x] **TokenList equivalence**: For every test file, V1 `lex()` and V2 `lex()` produce `TokenList` values where `tokens[i].kind` and `tags[i]` are identical
- [x] **TokenSet compatibility**: Verify `TokenSet` membership tests work with V2-produced tags
- [x] **Push invariant**: Verify `tags[i] == tokens[i].kind.discriminant_index()` holds for all tokens in V2 output
- [x] **Flags parallel invariant**: Verify `flags.len() == tokens.len()` after lexing
- [x] **Tag alignment**: For all non-data-carrying `RawTag` variants (operators, delimiters), verify that the cooker produces a `TokenKind` whose discriminant matches the corresponding `TokenTag` value
- [x] **name() coverage**: Verify `TokenTag::name()` returns a non-empty string for every variant

---

## 04.8 Completion Checklist

- [x] `TokenTag` defined in `ori_ir` with `#[repr(u8)]` and semantic ranges
- [x] `TokenIdx` defined in `ori_ir` as `#[repr(transparent)]` u32 newtype
- [x] `TokenFlags` defined in `ori_ir` as `bitflags!` u8
- [x] `RawTag` discriminant values documented and aligned with `TokenTag`
- [x] All `TokenTag` variants < 128 (TokenSet compatible; 122 defined variants fit in 0-127 range)
- [x] `TokenTag::name()` implemented for all variants
- [x] V2 cooker output feeds cleanly into existing `TokenList::push()`
- [x] `TokenFlags` stored parallel to tokens in `TokenList`
- [x] `raw_token.rs` and `convert.rs` removed
- [x] `logos` dependency removed from `ori_lexer/Cargo.toml`
- [x] Tag stability tests in place
- [x] All tag-dependent parser code updated to new discriminant values and passes tests
- [x] `cargo t -p ori_ir` and `cargo t -p ori_lexer` and `./test-all.sh` pass

**Exit Criteria:** `TokenTag`, `TokenIdx`, and `TokenFlags` are defined in `ori_ir` per v2-conventions. The V2 pipeline produces identical `TokenList` output using the existing SoA structure (with updated discriminant values), with `TokenFlags` added as a parallel array. The `RawToken` -> `TokenKind` dual-enum conversion is eliminated. `TokenKind::discriminant_index()`, all `TAG_*` constants, and all tag-dispatch sites in the parser (`OPER_TABLE`, `POSTFIX_BITSET`, `parse_primary()`, `check_type_keyword()`, `friendly_name_from_index()`, `parse_type()`, and all cursor helper methods) are updated to use the new `TokenTag` discriminant numbering. All discriminant values are < 128 for `TokenSet` compatibility (122 defined variants spanning 0-127 range; 6 reserved gaps for future expansion).

**Spec Alignment Summary:**
- ✅ All operators in grammar lines 69-74 covered (no compound assignment, no pipe operator)
- ✅ All keywords in grammar lines 56-58 covered (plus 3 phantom keywords marked for removal)
- ✅ All delimiters in grammar lines 78-79 covered
- ✅ Template literals supported (4 new tags: Head, Middle, Tail, Complete)
- ✅ Newline token exists (grammar line 32, spec-mandated)
- ✅ GtEq and ShiftRight correctly documented as parser-synthesized (adjacency detection + compound consumption, no tag mutation)
- ✅ Semicolon explicitly noted as parse error (not in spec grammar)
- ⚠️  Phantom keywords (async, mut, return) retained for V2 compatibility, marked for V3 removal
