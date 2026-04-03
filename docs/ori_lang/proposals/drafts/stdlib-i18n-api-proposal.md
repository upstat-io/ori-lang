# Proposal: std.i18n — Internationalization Library

**Status:** Draft
**Created:** 2026-04-02
**Author:** Eric (with AI assistance)
**Affects:** Standard library, `library/std/i18n/`
**Depends on:** stdlib-text-api-proposal.md (approved) — shares Unicode foundation
**Prior art:** ICU4X (Unicode Consortium), Go `x/text`, C# `System.Globalization`, Java `java.text`, CLDR

---

## Summary

This proposal defines `std.i18n` — a standard library package for locale-aware internationalization operations: collation (locale-sensitive sorting), number formatting, date/time formatting, currency formatting, plural rules, and message formatting. Backed by CLDR (Unicode Common Locale Data Repository) data.

---

## Motivation

`std.text` (approved) provides Unicode algorithms that are **locale-independent**: grapheme segmentation, normalization, case folding, display width. These work identically regardless of the user's language.

**Locale-dependent** operations are a separate concern:

- **Collation**: "ä" sorts with "a" in German but after "z" in Swedish
- **Number formatting**: 1,234.56 (English) vs 1.234,56 (German) vs 1 234,56 (French)
- **Date formatting**: 04/02/2026 (US) vs 02/04/2026 (UK) vs 2026年4月2日 (Japan)
- **Currency**: $1,234 vs 1.234 € vs ¥1,234
- **Plural rules**: "1 file" vs "2 files" (English) vs "1 файл" / "2 файла" / "5 файлов" (Russian — 3 forms)

These require CLDR data (~500KB minimum for common locales, ~5MB for full coverage) and are architecturally distinct from `std.text`'s pure Unicode algorithms.

---

## Scope

### In Scope

- **Collation**: Locale-sensitive string comparison and sorting (UTS #10)
- **Number formatting**: Decimal, percent, scientific, compact (CLDR patterns)
- **Date/time formatting**: Date, time, datetime with locale patterns (CLDR)
- **Currency formatting**: Currency symbol placement, grouping (CLDR)
- **Plural rules**: Cardinal and ordinal plural forms (CLDR)
- **Message formatting**: ICU MessageFormat-style parameterized messages
- **List formatting**: "A, B, and C" vs "A, B und C" (CLDR)
- **Locale type**: BCP 47 language tags, locale matching

### Out of Scope

- Calendar systems beyond Gregorian (Islamic, Hebrew, etc.) — future version
- Transliteration (Cyrillic → Latin, etc.) — future version
- Bidirectional text — covered by `std.text.unicode.bidi`
- Text segmentation — covered by `std.text.unicode.segmentation`

---

## API Sketch

```ori
use std.i18n { Locale, collation_sort, format_number, format_date }
use std.time { DateTime }

let locale = Locale.from_tag("de-DE")

// Locale-aware sorting
let sorted = collation_sort(["Zürich", "Aachen", "Österreich"], locale:)
// → ["Aachen", "Österreich", "Zürich"]  (Ö sorts with O in German)

// Number formatting
format_number(1234.56, locale:)  // → "1.234,56"

// Date formatting
let now = DateTime.now_utc()
format_date(now, locale:, style: DateStyle.Long)  // → "2. April 2026"

// Plural-aware messages
format_message("{count, plural, one {# file} other {# files}}", count: 5, locale:)
// → "5 files"
```

---

## Data Strategy

### CLDR Data Packaging

Options to evaluate:
1. **Ship full CLDR data** (~5MB) as part of `std.i18n` package
2. **Ship common locales** (~500KB for top 20 locales) with on-demand download for others
3. **Data-at-build-time** — `ori build` downloads locale data based on `ori.toml` configuration
4. **ICU4X backend** — delegate to ICU4X which has optimized, tree-shakeable locale data

### Capability

Locale-sensitive operations may need to detect the system locale:

```ori
@system_locale () -> Locale uses Env
// Reads LC_ALL / LANG environment variables
```

Pure operations that take an explicit `locale:` parameter need no capability.

---

## Detailed Design

*To be expanded during full proposal development.*

---

## Open Questions

1. Should collation be in `std.i18n.collation` or `std.text.collation`? (Recommendation: `std.i18n` since it needs CLDR data)
2. ICU4X as FFI backend vs pure Ori + CLDR data files?
3. How to handle locale data distribution? Ship with package or download on demand?
4. Should `std.i18n` re-export locale-independent `std.text` functions for convenience?
5. What is the minimum viable locale set to ship in the package?
