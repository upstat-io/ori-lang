# Lexer V2 Index

> **Plan Status: COMPLETE** (February 2026) — All 10 sections done. V2 is the default and only lexer.

> **Maintenance Notice:** Update this index when adding/modifying sections.
>
> **Conventions:** Cross-references to `plans/v2-conventions.md` are noted as (§*N*) throughout.

## How to Use
1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Architecture & Source Buffer
**File:** `section-01-architecture.md` | **Status:** Done

```
two-layer, architecture, crate, separation, rustc_lexer
ori_lexer_core, ori_lexer, crate boundary, reusable
low-level tokenizer, high-level processor, compiler integration
stable API, pure function, no dependencies, non_exhaustive
LSP, formatter, syntax highlighter, external tools
RawTag (core), TokenTag (ori_ir), crate mapping
source buffer, cursor, input, byte buffer
sentinel-terminated, null-terminated, zero byte
cache-line alignment, 64-byte alignment, SIMD-ready
BOM detection, UTF-8 BOM, byte order mark
v2-conventions §1 (TokenIdx), §2 (RawTag/TokenTag), §10 (two-layer)
```

### Section 02: Raw Scanner
**File:** `section-02-raw-scanner.md` | **Status:** Done

```
raw scanner, tokenizer, state machine, DFA
hand-written lexer, replace logos, manual scanning
RawTag, token tag, discriminant, character dispatch
character iteration, byte scanning, ASCII fast path
operator scanning, punctuation, delimiters
comment scanning, line comment, whitespace skip
string scanning, char literal, numeric literal
template literal, backtick, interpolation, brace depth
TemplateHead, TemplateMiddle, TemplateTail, TemplateComplete
newline handling, trivia, newline-significant
QuestionQuestion (??), DotDotDot (...), Dollar ($), Div (keyword)
HashBang (#!), file_attribute, conditional compilation
Rust rustc_lexer, Zig labeled switch, Go scanner
v2-conventions §2 (Tag enums), §10 (two-layer pattern)
```

### Section 03: Token Cooking & Interning
**File:** `section-03-token-cooking.md` | **Status:** Done (all cooking operations complete; IS_DOC deferred to Section 09 by design; 20 context-sensitive keyword integration tests)

```
token cooking, token conversion, raw to rich
RawTag to TokenTag, crate boundary mapping
string interning, Name, StringInterner, intern
escape sequences, unescape, string processing
only \n \t \r \0 \\ \" \' \` (no hex/unicode escapes)
template literal escapes: {{ }} for literal braces
numeric parsing, integer parsing, float parsing
duration literal, size literal, unit suffix, decimal duration
context-sensitive keywords, soft keywords
span construction, position tracking, byte offset
TokenFlags computation, SPACE_BEFORE, NEWLINE_BEFORE
detached doc comment, warning, doc classification
Rust cooking layer, two-phase, phase separation
v2-conventions §5 (LexError), §6 (LexOutput), §7 (shared types)
```

### Section 04: TokenList Compatibility & Tag Alignment
**File:** `section-04-soa-storage.md` | **Status:** Done

```
TokenList, tag alignment, discriminant, compatibility
RawTag, TokenKind, tag mapping, push path
TemplateHead, TemplateMiddle, TemplateTail, TemplateComplete
dual-enum elimination, RawToken removal, convert.rs removal
logos removal, dependency cleanup, code reduction
TokenSet, bitset, tag stability, tag numbering
existing SoA, parallel tag array, unchanged structure
```

### Section 05: SWAR & Fast Paths
**File:** `section-05-swar-fast-paths.md` | **Status:** Done (SWAR + memchr implemented; profiling showed byte loop faster than SWAR for typical whitespace; memchr used for strings/comments)

```
SWAR, SIMD within a register, bit manipulation
whitespace scanning, fast whitespace, 8 bytes
memchr, fast search, byte search, string terminator
ASCII fast path, sentinel, branchless, hot loop
comment body scanning, fast skip, eat_until
identifier scanning, fast identifier, ASCII check
Roc fast_eat_whitespace, Rust memchr, Go sentinel
```

### Section 06: Keyword Recognition
**File:** `section-06-keyword-recognition.md` | **Status:** Done (6 pattern keywords context-sensitive via `(` lookahead)

```
keyword, reserved word, keyword lookup, keyword hash
perfect hash, compile-time hash, static map
length-bucketed, PHF, minimal perfect hash
keyword vs identifier, classification, recognition
reserved keywords: as, break, continue, def, div, do, else,
extend, extension, extern, false, for, if, impl, in, let, loop, match,
pub, self, Self, suspend, tests, then, trait, true,
type, unsafe, use, uses, void, where, with, yield
reserved (future): asm, inline, static, union, view
context-sensitive keywords (pattern position):
cache, catch, parallel, recurse, run, spawn, timeout, try
context-sensitive (other): by (after range), max (fixed lists)
context-sensitive (imports): without (before "def")
context-sensitive (type names): bool, byte, float, int, str
built-in names: Ok, Err, Some, None, print, panic, todo, unreachable
Go perfect hash, Zig StaticStringMap, TypeScript textToKeyword
```

### Section 07: Diagnostics & Error Recovery
**File:** `section-07-diagnostics.md` | **Status:** Done (WHERE+WHAT+WHY+HOW error shape; Unicode confusables; cross-language habit detection; detached doc warnings; oric diagnostic pipeline integration)

```
error handling, diagnostics, error recovery
LexError, WHERE+WHAT+WHY+HOW, structured errors
error message, context-aware, whatIsNext
cross-language detection, JavaScript habits
unterminated string, invalid escape, bad character
unicode confusable, smart quotes, curly quotes
detached doc comment, warning, doc distance
error accumulation, continue on error, resilient
Elm error messages, Gleam proactive detection
Rust unicode_chars, TypeScript conflict markers
v2-conventions §5 (Error Shape)
```

### Section 08: Parser Integration & Migration
**File:** `section-08-parser-integration.md` | **Status:** Done (migration complete; template literals done; cursor flags + IS_DOC done; V2 is default lexer; 8,779 tests pass)

```
parser integration, cursor, parser cursor
tag-based dispatch, token dispatch, tag constant
migration, backward compatibility, API stability
skip_newlines, synchronize, recovery set
TokenSet, discriminant_index, bitset
greater-than splitting, shift synthesis
re-scanning, context-sensitive, rescan
template literal, template interpolation, format spec
TemplateFull, TemplateLiteral, TemplatePart, FormatSpec
parse_template_literal, backtick, string interpolation
TypeScript reScanGreaterToken, speculation
```

### Section 09: Salsa & IDE Integration
**File:** `section-09-salsa-ide.md` | **Status:** Complete (position-independent Hash/Eq, LexOutput Salsa traits, DocMember replaces DocParam/DocField, tokens_with_metadata() query, early cutoff verified)

```
Salsa, incremental compilation, query, caching
early cutoff, token hash, recomputation skip
lex_with_comments, formatter, comment metadata
ModuleExtra, blank lines, newline positions
CommentList, doc comment, comment classification
IDE, language server, syntax highlighting
v2-conventions §6 (Phase Output), §8 (Salsa Compatibility)
```

### Section 10: Benchmarking & Performance Validation
**File:** `section-10-benchmarking.md` | **Status:** Done (V2 final: ~238-242 MiB/s; ~0.83x V1; 65% improvement from initial; 3 callgrind campaigns; 8,779 tests pass)

```
benchmark, performance, throughput, regression
bytes per second, tokens per second, latency
259-292 MiB/s baseline, V2 final ~238-242 MiB/s
lexer benchmark, scaling, file size
profiling, callgrind, instruction count, cross-crate inline
SWAR counterproductive, byte loop whitespace, eat_whitespace
baseline, comparison, before/after
test-all, clippy-all, conformance
```

---

## Quick Reference

| ID | Title | File | Tier |
|----|-------|------|------|
| 01 | Architecture & Source Buffer | `section-01-architecture.md` | 0 |
| 02 | Raw Scanner | `section-02-raw-scanner.md` | 0 |
| 03 | Token Cooking & Interning | `section-03-token-cooking.md` | 0 |
| 04 | TokenList Compatibility & Tag Alignment | `section-04-soa-storage.md` | 1 |
| 05 | SWAR & Fast Paths | `section-05-swar-fast-paths.md` | 1 (done) |
| 06 | Keyword Recognition | `section-06-keyword-recognition.md` | 1 |
| 07 | Diagnostics & Error Recovery | `section-07-diagnostics.md` | 2 |
| 08 | Parser Integration & Migration | `section-08-parser-integration.md` | 3 (done) |
| 09 | Salsa & IDE Integration | `section-09-salsa-ide.md` | 3 (complete) |
| 10 | Benchmarking & Performance Validation | `section-10-benchmarking.md` | 4 (done) |

---

## Related Plans

| Plan | Relationship |
|------|--------------|
| `plans/parser_v2/` (completed) | Parser V2 has been implemented and plan removed; lexer V2 feeds into the existing parser |
| `plans/types_v2/` (completed) | **Independent** -- operates on AST, not tokens; types V2 has been implemented and plan removed |
| `plans/roadmap/` | Overall language roadmap |
| `plans/ori_lsp/` | Uses tokens for syntax highlighting |
| `plans/v2-conventions.md` | **Cross-system conventions** -- shared patterns for all V2 systems |

### Cross-System Cohesion

Lexer V2 and Types V2 are **independent in dependency** but share design conventions:
- Different compiler phases (lexing vs type checking)
- Different crates (`ori_lexer` vs `ori_types`)
- Communicate only via `ori_ir` shared types (`Span`, `Name`, `TokenKind`, `ModuleExtra`)
- Can be developed in parallel

Both systems follow the same structural patterns -- index types, tag enums, SoA accessors, flag types, error shapes -- codified in `plans/v2-conventions.md`. This consistency means knowledge transfers between systems: understanding how `TokenList.tag(idx)` works in the existing token infrastructure immediately tells you how the lexer V2 tag-based access will work.
