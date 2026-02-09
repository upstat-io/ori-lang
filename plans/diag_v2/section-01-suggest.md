---
section: "01"
title: Edit-Distance Suggestions
status: not-started
goal: Damerau-Levenshtein edit distance module for "did you mean?" suggestions across all phases
sections:
  - id: "01.1"
    title: Core Algorithm
    status: not-started
  - id: "01.2"
    title: Candidate Selection API
    status: not-started
  - id: "01.3"
    title: Integration Points
    status: not-started
  - id: "01.4"
    title: Completion Checklist
    status: not-started
---

# Section 01: Edit-Distance Suggestions

**Status:** Not Started
**Goal:** Add a `suggest` module to `ori_diagnostic` providing Damerau-Levenshtein edit distance for "did you mean?" suggestions. Integrate with all phases that report unknown identifiers, fields, functions, variants, or arguments.

**Reference compilers:**
- **Elm** `compiler/src/Reporting/Suggest.hs` — Damerau-Levenshtein with weighted costs; `nearbyNames` function returns sorted candidates
- **Rust** `compiler/rustc_span/src/lev_distance.rs` — Levenshtein with dynamic threshold `max(1, name.len() / 3)`
- **Go** `src/cmd/compile/internal/types2/errors.go` — Simple Levenshtein for field/method suggestions

**Current state:** `SemanticProblem::UnknownIdentifier` has a `similar: Option<Name>` field, and `TypeErrorKind::UnknownIdent` has `similar: Vec<Name>`. Both are populated by callers doing ad-hoc similarity checks. There is no shared, well-tuned edit-distance implementation.

---

## 01.1 Core Algorithm

Implement Damerau-Levenshtein (not plain Levenshtein) because transpositions are the most common typo pattern (`teh` → `the`, `adn` → `and`).

### Algorithm

```rust
// In ori_diagnostic/src/suggest.rs

/// Compute the Damerau-Levenshtein distance between two strings.
///
/// Supports four operations: insertion, deletion, substitution, and
/// adjacent transposition. Uses the optimal string alignment variant
/// (restricted edit distance) which is O(m*n) time and O(m*n) space
/// but correct for all practical suggestion use cases.
///
/// Returns `None` if the distance exceeds `max_distance` (early exit).
pub fn damerau_levenshtein(a: &str, b: &str, max_distance: usize) -> Option<usize>
```

**Design decisions:**
- **Optimal String Alignment (OSA)** variant, not full Damerau-Levenshtein. OSA is simpler (no alphabet-size array) and sufficient for typo detection. The difference only matters for pathological cases like `CA` → `ABC` where OSA gives 3 vs full DL gives 2 — irrelevant for identifier suggestions.
- **Early exit** when distance exceeds `max_distance` — prunes the matrix computation for clearly dissimilar strings.
- **Case-sensitive** comparison (Ori identifiers are case-sensitive).
- **UTF-8 aware** — operates on `char` boundaries, not bytes.

### Threshold Function

```rust
/// Compute the maximum acceptable edit distance for a given name length.
///
/// Follows Rust's heuristic: max(1, name.len() / 3).
/// Short names (1-3 chars) allow 1 edit; longer names scale linearly.
///
/// This prevents suggesting `x` for `xyz` (distance 2, threshold 1)
/// while allowing `function_nam` for `function_name` (distance 1, threshold 4).
pub fn suggestion_threshold(name_len: usize) -> usize
```

The Rust heuristic `max(1, len/3)` is well-tested across millions of error reports. Elm uses a similar approach but with slightly different constants.

### Weighted Distance (Optional Enhancement)

Elm weights certain edits lower than others:
- First-character case change: cost 0.5 (common: `String` vs `string`)
- Adjacent key transposition: cost 0.5 (keyboard layout aware)

For V2, start with uniform costs. Weighted distance can be added later without API changes.

- [ ] Implement `damerau_levenshtein(a, b, max_distance) -> Option<usize>`
- [ ] Implement `suggestion_threshold(name_len) -> usize`
- [ ] Unit tests: empty strings, identical strings, single edits, transpositions, max distance cutoff
- [ ] Unit tests: Unicode identifiers (e.g., `café` vs `cafe`)

---

## 01.2 Candidate Selection API

### Primary API

```rust
/// Find the best suggestion from a list of candidates.
///
/// Returns the candidate with the lowest edit distance that is within
/// the threshold for the given name. If multiple candidates have the
/// same distance, returns the first one found (stable ordering).
///
/// # Arguments
/// * `name` — The unknown name to find suggestions for
/// * `candidates` — Available names to compare against
/// * `max_results` — Maximum number of suggestions to return (typically 1-3)
///
/// # Returns
/// Sorted `Vec<Suggestion>` (best match first), empty if nothing is close enough.
pub fn find_suggestions(
    name: &str,
    candidates: impl IntoIterator<Item = impl AsRef<str>>,
    max_results: usize,
) -> Vec<SuggestedName>

/// A suggested name with its edit distance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuggestedName {
    pub name: String,
    pub distance: usize,
}
```

### Name-Based API (For Interned Names)

```rust
/// Find suggestions using interned `Name` values.
///
/// Convenience wrapper that resolves names through the interner.
/// Returns `Name` values rather than strings.
pub fn find_name_suggestions(
    name: Name,
    candidates: &[Name],
    interner: &StringInterner,
    max_results: usize,
) -> Vec<SuggestedIdent>

/// A suggested interned identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuggestedIdent {
    pub name: Name,
    pub distance: usize,
}
```

### Filtering Heuristics

Before computing edit distance, apply fast pre-filters (from Elm's `Suggest.hs`):

1. **Length filter**: Skip candidates where `|len(a) - len(b)| > threshold`. This is O(1) and eliminates most candidates.
2. **First-character filter**: Optionally boost candidates sharing the first character (most typos preserve the first letter).
3. **Substring filter**: If the name is a substring of a candidate (or vice versa), include it even if edit distance is high (`log` in `logger`).

```rust
/// Pre-filter candidates by length before computing edit distance.
/// Returns only candidates within the length threshold.
fn length_prefilter<'a>(
    name_len: usize,
    candidates: impl Iterator<Item = &'a str>,
    threshold: usize,
) -> Vec<&'a str>
```

- [ ] Implement `find_suggestions(name, candidates, max_results) -> Vec<SuggestedName>`
- [ ] Implement `find_name_suggestions(name, candidates, interner, max_results) -> Vec<SuggestedIdent>`
- [ ] Implement length pre-filter
- [ ] Unit tests: no candidates, single candidate, multiple matches, ordering
- [ ] Unit tests: threshold edge cases (1-char names, very long names)

---

## 01.3 Integration Points

### Phase-by-Phase Integration

Each phase that reports "unknown X" errors should use the suggest module:

| Phase | Error | Candidate Source | Current State |
|-------|-------|-----------------|---------------|
| Semantic | `UnknownIdentifier` | Scope bindings + imports | Has `similar: Option<Name>` — replace with suggest |
| Semantic | `UnknownFunction` | Function definitions | No suggestion — add |
| Semantic | `ImportedItemNotFound` | Module exports | No suggestion — add |
| Type checker | `UnknownIdent` | Type scope + functions | Has `similar: Vec<Name>` — replace with suggest |
| Type checker | `UndefinedField` | Struct fields | Has `available: Vec<Name>` — add distance ranking |
| Parser | `UnknownPatternArg` | Valid pattern arguments | Has `valid_args` — add distance ranking |
| Parser | `RequiresNamedArgs` | Named argument list | No suggestion — add |

### Integration Pattern

Each call site follows the same pattern:

```rust
// Before (ad-hoc):
SemanticProblem::UnknownIdentifier {
    span,
    name,
    similar: scope.find_similar(name), // Ad-hoc comparison
}

// After (using suggest module):
use ori_diagnostic::suggest::find_name_suggestions;

let suggestions = find_name_suggestions(
    name,
    &scope.all_visible_names(),
    interner,
    3, // max 3 suggestions
);

SemanticProblem::UnknownIdentifier {
    span,
    name,
    suggestions, // Vec<SuggestedIdent> — ranked by distance
}
```

### Rendering Integration

The reporting layer formats suggestions into human-readable and machine-readable form:

```rust
// In oric/src/reporting/semantic.rs
if suggestions.len() == 1 {
    diagnostic = diagnostic.with_suggestion(
        format!("did you mean `{}`?", interner.lookup(suggestions[0].name))
    );
} else if !suggestions.is_empty() {
    let names: Vec<_> = suggestions.iter()
        .map(|s| format!("`{}`", interner.lookup(s.name)))
        .collect();
    diagnostic = diagnostic.with_note(
        format!("similar names: {}", names.join(", "))
    );
}
```

### Migration Plan

1. Add `suggest` module to `ori_diagnostic`
2. Update `SemanticProblem::UnknownIdentifier` to use `Vec<SuggestedIdent>` instead of `Option<Name>`
3. Update `TypeErrorKind::UnknownIdent` to rank by distance
4. Update `TypeErrorKind::UndefinedField` to rank `available` by distance
5. Update `ParseProblem::UnknownPatternArg` to rank `valid_args` by distance
6. Update all renderers to format suggestions consistently

- [ ] Update `SemanticProblem::UnknownIdentifier` field type
- [ ] Update `TypeErrorKind::UnknownIdent` to use suggest
- [ ] Update `TypeErrorKind::UndefinedField` to rank by distance
- [ ] Update `ParseProblem::UnknownPatternArg` to rank by distance
- [ ] Update all corresponding renderers
- [ ] Ensure all tests pass after migration

---

## 01.4 Completion Checklist

- [ ] `ori_diagnostic/src/suggest.rs` module created
- [ ] `damerau_levenshtein` function with early exit
- [ ] `suggestion_threshold` function
- [ ] `find_suggestions` and `find_name_suggestions` API
- [ ] Length pre-filter optimization
- [ ] All phases integrated (semantic, type checker, parser)
- [ ] Renderers format suggestions consistently
- [ ] Tests: algorithm correctness (15+ test cases)
- [ ] Tests: API usage (10+ test cases)
- [ ] Tests: integration (5+ end-to-end test cases in `tests/spec/`)
- [ ] `./test-all.sh` passes

**Exit Criteria:** Every "unknown X" error in every phase includes ranked edit-distance suggestions when similar names exist. The suggest module is a standalone, well-tested utility with no phase dependencies.
