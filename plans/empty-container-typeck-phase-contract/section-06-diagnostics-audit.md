---
section: "06"
title: "Diagnostics + Spec-Test Audit"
status: in-progress

reviewed: true
goal: >
  Resolve the 35-file × 386-diagnostic E2005 ledger captured by §03.N by adding
  explicit type annotations (empty-literal class) and explicit lambda-parameter
  annotations (lambda-parameter class) so the full test suite returns to green
  after §03's validator is live and §04's codegen assertions are in place. Also
  verify the E2005 diagnostic wording is actionable and keep `diagnostics/state.sh`
  and §03's ledger synchronized as files come out of the known-failing list.
success_criteria:
  - "E2005 diagnostic message reads: 'cannot infer the type of this empty list; add a type annotation like `let x: [int] = []`' — verifiable via the E2005 message test in `check/validators/tests.rs`."
  - "After the per-file annotation pass, `diagnostics/state.sh show --json | jq '.test_suite.known_failing_files | length'` returns `0` (the 35-file ledger from §03.N is empty) AND `diagnostics/state.sh show --json | jq '.test_suite.totals.failed'` returns `0`."
  - "`timeout 150 ./test-all.sh` is green (debug build); then `timeout 150 cargo test --release -p ori_types` and `timeout 150 cargo test --release -p ori_llvm` are green (release build)."
  - "Every file in §03.N's `tests/spec/` (31 files) and `tests/compiler/` (4 files) Known Failing ledger has a checkbox in §06.2 / §06.2B marked `[x]` with the annotation form applied; file count in the ledger matches — no drift between `- [ ]` anchors and the §03.N table."
  - "Lambda-parameter annotations preserve observable behavior: `timeout 150 diagnostics/dual-exec-verify.sh` reports interpreter/LLVM parity on every spec test touched in §06.2B."
  - "No test in `tests/spec/types/empty_literals/` (§03.BUG-FIXES `[Never]` defaulting corpus, 21 files) regresses — `timeout 150 cargo st tests/spec/types/empty_literals/` green before AND after §06 commits; §06 never mechanically annotates files in this directory because they cover the defaulting path."
  - "`#compile_fail(code: \"E2005\")` is NOT added to any file in §03.N's ledger — that tag belongs to §05's NEW negative-pin corpus at `tests/spec/types/collections/empty_list/`; §06 only adds concrete annotations to legacy files so they compile clean."
  - "`diagnostics/state.sh refresh --full --by section-06` runs at the end of §06.4 and writes updated totals (`known_failing_count == 0`, `known_failing_files == []`, `failure_class == null`) to `.claude/state/known-state.json`; §03.N Known Failing Tests table is updated in the same commit to reference the cleared state."
depends_on: ["01", "02", "03", "04", "05"]
third_party_review:
  status: resolved
  updated: 2026-04-20
  notes: "3 rounds completed: R0+R1 produced 3 verified findings all fixed inline (commits 52aa673d, 7caff2b9); R2 clean — codex's R2 findings dropped at §4 verification as hallucinated out-of-scope §08 content, gemini R2 clean."
sections:
  - id: "06.1"
    title: "E2005 diagnostic wording + suggestion text"
    status: not-started
  - id: "06.2"
    title: "Annotation sweep — empty-literal class (tests/spec/, tests/compiler/, tests/valgrind/)"
    status: not-started
  - id: "06.2B"
    title: "Annotation sweep — lambda-parameter class (35-file ledger subset with .map/.filter/.fold)"
    status: not-started
  - id: "06.3"
    title: "Annotation sweep — library/std/"
    status: not-started
  - id: "06.4"
    title: "Regression verification + state.sh + §03.N ledger sync"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: complete


  - id: "06.N"
    title: "Completion Checklist"
    status: in-progress

---

## Intelligence Reconnaissance

Queries run 2026-04-17 (preserved) + 2026-04-20 (editor re-scope):

- `scripts/intel-query.sh --human file-symbols "ori_diagnostic" --repo ori` — inventory `ori_diagnostic` crate symbols (error builder, suggestion API) before auditing E2005 wording.
- `scripts/intel-query.sh --human callers "AmbiguousType" --repo ori` — find all `E2005` construction sites to confirm the message string is set in exactly one place.
- `scripts/intel-query.sh --human similar "empty list type annotation suggestion" --repo rust,elm --limit 5` — prior art for actionable empty-collection type-inference suggestions (Rust `E0282` "type annotations needed", Elm explicit annotation prompts).
- `scripts/intel-query.sh --human symbol-plans "AmbiguousType" --repo ori` — cross-reference which plans/sections/bugs reference E2005; used to confirm §05 owns the canonical corpus and §06 is the audit-only consumer.
- `scripts/intel-query.sh --human file-symbols "default_unbound_vars_from_empty_literals" --repo ori` — confirm the `[Never]`-defaulting pre-pass surface before deciding which legacy files to annotate vs leave to defaulting.

Results summary (≤500 chars) [ori]: `AmbiguousType` (E2005) constructed in `ori_types/src/type_error/check_error/`; message string lives in the `message.rs` mapping. `default_unbound_vars_from_empty_literals` lives in `infer/mod.rs` with call sites in `check/bodies/{functions,impls,tests}.rs` + `infer/body_finalize/mod.rs` — the defaulting corpus at `tests/spec/types/empty_literals/` (21 files) MUST NOT be touched by §06 mechanical annotation. [rust]: E0282 "type annotations needed, cannot infer type" uses the exact imperative pattern. [elm]: `Type.Error` suggestions show concrete annotated form.

---

# Section 06: Diagnostics + Spec-Test Audit

**Status:** Not Started
**Goal:** Clear §03.N's 35-file × 386-diagnostic Known Failing ledger by adding
annotations to legacy spec-test files, verify E2005 wording is actionable, and
sync `diagnostics/state.sh` + §03.N's ledger when the sweep completes. This is
the "clean-up after the fix" section; it does NOT add new negative pins or
`#compile_fail` tests (§05 owns those) and does NOT add new defaulting tests
(§03.BUG-FIXES owns the `tests/spec/types/empty_literals/` corpus).

**Depends on:** §01 (Value Restriction), §02 (Validator Module), §03 (Bodies-Pass
Integration + BUG-FIXES defaulting), §04 (Codegen Assertions), §05 (Test Matrix
— canonical negative-pin corpus and semantic pins land first so §06 never adds
redundant `#compile_fail` coverage).

**Sequencing rule:** §06 MUST NOT start until §03's validator is live on all four
body-group passes AND §05's canonical empty-literal corpus exists at
`tests/spec/types/collections/empty_list/` AND §04's codegen assertions have
landed their primary seam. §03 already emits `E2005` across the 35-file ledger;
starting §06 before §05 lands risks §06 accidentally owning `#compile_fail`
coverage that belongs to the canonical corpus. Starting §06 before §04 lands
means the "no `unresolved type variable at codegen`" success criterion from
`00-overview.md` cannot be verified alongside the annotation sweep.

**Two failure classes, one section:** Per §03.N Known Failing Tests table, the
ledger has TWO distinct `E2005` classes that §06 resolves with DIFFERENT
annotation shapes:

| Class | Example | Annotation form | §06 subsection |
|-------|---------|-----------------|----------------|
| Empty-literal, no constraining use | `let x = []`, `[].iter()`, `[].len()`, `{}.keys()`, `[] + [1, 2, 3]` | Add `let x: [T] = []` / `{str: int} = {}` binding annotation | §06.2 |
| Lambda-parameter inference in method chains | `list.map(x -> x.method())`, `list.filter(x -> ...)` — receiver's element type does not propagate bidirectionally into the closure body | Annotate the closure parameter AND the return type (spec `grammar.ebnf §lambda` `typed_lambda` rule requires both when any param is annotated): `.map((d: Duration) -> int = d.minutes())` | §06.2B |

Skipping §06.2B (only sweeping `\[\s*\]`) leaves ~26 lambda-parameter files in
the ledger — the comprehensive `rg '\[\s*\]'` regex does NOT match lambda
parameters. The §03.N table names both classes; §06 MUST resolve both.

---

## 06.1 E2005 Diagnostic Wording

**Target message:** `"cannot infer the type of this empty list; add a type annotation like `let x: [int] = []`"`

Per `impl-hygiene.md §Error Handling — Diagnostics`:
- All errors have spans (the empty-list expression span)
- Imperative suggestions ("add a type annotation")
- No "unexpected X" without "expected Y because Z"

**Known limitation from §03.N close-out:** `validate_body_types` currently emits
E2005 with "expression" as the context label regardless of where the unresolved
`Tag::Var` appears (parameter, return type, body expression). §06.1 refines the
message to distinguish empty-list sites from lambda-parameter sites.

- [ ] Add a dispatch in the E2005 message builder (see `ori_types/src/type_error/check_error/message.rs`) that selects message text based on the expression's `ExprKind` at the error site:
  - `ExprKind::List([])` / `ExprKind::ListWithSpread([])` → `"cannot infer the type of this empty list; add a type annotation like \`let x: [int] = []\`"` (per `00-overview.md:106` Design Principle 4: specialized E2005 wording targets lists only; `Map`/`MapWithSpread`/`Set` empty-literal sites stay on the generic fallback to preserve the overview's list-only scope).
  - `ExprKind::Lambda { params, .. }` where a parameter has unresolved `Tag::Var` → `"cannot infer the type of this closure parameter; add a full typed-lambda annotation like \`(x: int) -> ReturnT = body\`"` (per spec `grammar.ebnf:550-553` typed_lambda rule — param type + return type + `=` body all required together; shorthand forms like `(x: int) -> body` are parse errors).
  - All other positions → preserve the current generic wording.
- [ ] Span discipline: the primary span SHALL point to the `[]` or `{}` literal (empty-literal class) or to the parameter token (lambda-parameter class), NOT to the enclosing `let` binding or method call.
- [ ] **Matrix test** (TDD): extend `compiler/ori_types/src/check/validators/tests.rs` with two new cells exercising the two message forms — `test_e2005_message_for_empty_list` and `test_e2005_message_for_lambda_param` — asserting the exact message string AND the span byte range.
- [ ] **Negative pin**: add `test_e2005_message_falls_back_to_generic_for_signature_var` so a fresh `Tag::Var` in a `FunctionSig.param_types[0]` position (unannotated parameter, not a lambda inside a body) still gets the original generic wording — regression guard against over-eager lambda-message dispatch.

**Verification:** The E2005 message tests in `check/validators/tests.rs` assert
the exact message string AND the span byte range. The diagnostic span points to
the empty list literal `[]` (or closure parameter token), not to the `let`
binding or the `push` call.

---

## 06.2 Annotation Sweep — Empty-Literal Class

**Scope:** Legacy spec-test files whose E2005 failure is an empty-literal
(`[]`, `{}`, `Set<T>()` when surface syntax permits) without a constraining use.

**Discovery commands (use these to build the working set; DO NOT use them as the
success criterion — the ledger in §03.N is authoritative):**

```bash
# Empty list literals — let-binding, argument, operator/concat, for..in,
# return-from-block, nested, receiver-chain, comment-filled `[/* empty */]`,
# and multiline `[\n]` forms.
rg '\[\s*\]' tests/spec/ tests/compiler/ tests/valgrind/ library/ --glob '*.ori'

# Empty map literals — also trigger E2005 per §02 validator rejecting ANY
# unresolved Tag::Var, not only list element vars.
rg '\{\s*\}' tests/spec/ tests/compiler/ tests/valgrind/ library/ --glob '*.ori'

# Set<T>() — if surface syntax admits bare `Set<T>()` without element
# constraints; grep both forms.
rg 'Set\s*<[^>]*>\s*\(\s*\)' tests/spec/ tests/compiler/ tests/valgrind/ library/ --glob '*.ori'
```

**Manual inspection required.** The regex false-positives on:
- Positions where context supplies type: `get_count(items: [])` has `[int]` inferred from the parameter type (`tests/spec/extensions/list_methods.ori:38-50`) — these compile clean today without annotation.
- Pattern-match arms: `match e { [] -> ... }` — pattern type is constrained top-down by the scrutinee type, no E2005 emitted. Patterns are not expressions for validator purposes.
- Empty map literal in block-value position vs empty block: `{}` at block end IS the empty map (Spec Clause 11 / 14.4); empty block is `{ ; }`.
- Multiline `[\n]` empties and `[/* empty */]` comment-filled forms — captured by `\[\s*\]` but require line inspection to confirm.

**`[Never]` defaulting protection — do NOT annotate these:** Files under
`tests/spec/types/empty_literals/` (21 files, owned by §03.BUG-FIXES) exercise
the `default_unbound_vars_from_empty_literals` pre-pass that defaults
unconstrained empty literals to `Idx::NEVER`. Mechanical annotation would
destroy the defaulting coverage. §06.2 SHALL exclude this directory from the
annotation sweep — `rg ... --glob '!tests/spec/types/empty_literals/**'` or
manual filter.

### 06.2 — Per-file annotation tasks (empty-literal class subset of §03.N ledger)

For each file: inspect every hit with the discovery commands above, decide
annotation target per the fix rules below, apply the annotation, run
`timeout 150 cargo st <path>` to confirm local green, then check the box.

**Fix rules (in priority order):**
1. If context supplies type (argument position with declared parameter type, return position with declared return type, `Check(T)` propagation target) → no annotation needed; regex false-positive, skip.
2. If the test SHOULD compile cleanly → add `let x: [T] = []` / `let x: {K: V} = {}` annotation with `T` / `K`/`V` taken from the downstream use (e.g., `.push(value: 10)` → `[int]`; `.insert(key: "a", value: 1)` → `{str: int}`).
3. If the test documents a compile failure in another dimension → preserve its existing `#compile_fail(...)` attribute (multi-error case); document that E2005 also fires as an inline comment referencing this section.
4. **NEVER add `#compile_fail(code: "E2005")`** to any file in §03.N's ledger — that tag is reserved for §05's new negative-pin corpus at `tests/spec/types/collections/empty_list/`. `#compile_fail` is file-level per §05 (`section-05-test-matrix.md:779-781`), so converting a mixed-behavior file in place would destroy its existing positive-pin coverage.

Sourced from §03.N Known Failing Tests table (`section-03-bodies-pass-integration.md:1243-1284`). One checkbox per file. Mark `[x]` as each file's annotation lands.

#### tests/spec/ (empty-literal-class subset — verified by §03.N table plus manual triage of §06.2B overlap)

- [ ] `tests/spec/capabilities/propagation.ori` — annotate any `let x = []` or `{}` sites; verify `cargo st tests/spec/capabilities/propagation.ori` green.
- [ ] `tests/spec/declarations/stdlib/testing_assert_eq.ori` — typically uses empty-literal patterns in assert helpers; annotate where E2005 fires.
- [ ] `tests/spec/declarations/test_variant_match.ori` — sum-variant construction with empty-payload branches.
- [ ] `tests/spec/declarations/traits.ori` — trait-body empty literals (if any).
- [ ] `tests/spec/expressions/field_access.ori` — empty literal in field-access context.
- [ ] `tests/spec/imports/generic_import.ori` — empty literal crossing the import boundary.
- [ ] `tests/spec/inference/generics.ori` — empty literal exercising generic parameter inference.
- [ ] `tests/spec/inference/unification.ori` — empty literal in unification-edge tests.
- [ ] `tests/spec/lexical/delimiters.ori` — `[].len()` expression-position form (§03.N line 134; BLOAT note: this file is 577 lines — do NOT split, unrelated bloat, only add annotations).
- [ ] `tests/spec/lexical/keywords.ori` — `[].len()` expression-position form (§03.N line 135; BLOAT note: 399 lines — only add annotations).
- [ ] `tests/spec/lexical/operators.ori` — operator-position empty-literal forms.
- [ ] `tests/spec/patterns/data.ori` — empty-pattern + empty-literal interaction.
- [ ] `tests/spec/patterns/match.ori` — confirm any `[]` match-arm hits are pattern-position (no annotation needed) vs scrutinee-position (annotate).
- [ ] `tests/spec/traits/core/comparable.ori` — empty literal in comparable-trait tests.
- [ ] `tests/spec/traits/core/compound_equals.ori` — empty literal in compound-equals tests.
- [ ] `tests/spec/traits/core/compound_hash.ori` — empty literal in compound-hash tests.
- [ ] `tests/spec/traits/core/option.ori` — empty literal inside `Option<[T]>` / `Some([])` forms.
- [ ] `tests/spec/traits/debug/join.ori` — empty-list join.
- [ ] `tests/spec/traits/into/str_to_error.ori` — empty literal in `Into` conversion tests.
- [ ] `tests/spec/traits/traceable/definition.ori` — empty literal in traceable-trait definition tests.
- [ ] `tests/spec/traits/traceable/result_delegation.ori` — empty literal in traceable-result delegation tests.
- [ ] `tests/spec/types/duration_size_default.ori` — empty literal in duration/size default tests.
- [ ] `tests/spec/types/enum/niche/niche_cross_feature.ori` — empty literal in niche-encoded sum types.
- [ ] `tests/spec/types/enum/niche/option_str.ori` — empty literal in `Option<str>` niche tests.
- [ ] `tests/spec/types/existential.ori` — empty literal in `impl Trait` position.
- [ ] `tests/spec/types/never.ori` — empty literal in `Never` defaulting tests (careful: some of these may legitimately exercise `[Never]` defaulting — leave untouched if `tests/spec/types/empty_literals/` would own the equivalent coverage).
- [ ] `tests/spec/types/option/ok_or.ori` — empty literal in `Option::ok_or` tests.

#### tests/compiler/ (4 files — all empty-literal class per §03.N)

- [ ] `tests/compiler/typeck/collections.ori` — empty-collection typeck tests; annotate per fix-rule 2.
- [ ] `tests/compiler/typeck/control_flow.ori` — empty literal in control-flow typeck.
- [ ] `tests/compiler/typeck/generics.ori` — empty literal in generics typeck.
- [ ] `tests/compiler/typeck/let_bindings.ori` — empty literal in let-binding typeck (overlaps with §01 Value Restriction coverage; annotate where E2005 fires post-§03).

#### tests/valgrind/ (edge case — only if §03 ledger confirms failure)

Per §03.N line 1282, `tests/valgrind/` currently has 0 files in the ledger
because all empty-literal occurrences there have constraining uses. The
`rg '\[\s*\]'` discovery command still hits `tests/valgrind/cow/cow_list_concat.ori`
(`[] + [1, 2, 3]` operator position), which is constrained by `[int]` from the
other operand — no annotation needed today. Include as a verification-only
entry; if §03 validator surfaces a NEW valgrind failure during §06.4 regression,
add the file here.

- [ ] Verify `timeout 150 cargo st tests/valgrind/cow/cow_list_concat.ori` green with no `E2005` after §06.2 sweep. If it fails, add annotation at the `[] + ...` site and file `/add-bug` on §06.2's discovery regex as a tooling gap.

### Close §06.2

- [ ] All boxes above marked `[x]`; 35-file ledger's empty-literal-class entries cleared.
- [ ] `timeout 150 cargo st tests/spec/` green for every touched file (pair-wise spot check: `cargo st <file>` for 5 randomly-picked files AND the full suite in §06.4).
- [ ] Update §03.N Known Failing Tests table inline: strike through each completed file with a `[REMEDIATED IN §06.2]` tag; do NOT delete entries — `/continue-roadmap` scanner reads the historical table for provenance.
- [ ] `tests/spec/types/empty_literals/` corpus still green — `timeout 150 cargo st tests/spec/types/empty_literals/` identical pass/fail counts pre- and post-§06.2.

---

## 06.2B Annotation Sweep — Lambda-Parameter Class

**Scope:** The ~20 files in §03.N's ledger whose E2005 cause is a lambda
parameter whose type cannot be inferred from the method-call receiver — the
`list.map(x -> x.method())` family. The empty-literal `rg` does NOT match these;
§06.2 without this subsection leaves the class untouched.

**Fix shape:** Annotate the closure parameter AND the return type inline. Per `docs/ori_lang/v2026/spec/grammar.ebnf` lines 550-553, Ori has exactly TWO lambda forms: `simple_lambda = lambda_params "->" expression` (bare identifiers, no annotations) and `typed_lambda = "(" typed_param_list ")" "->" type "=" expression` (typed params REQUIRE a return type AND `=` body — no mid-form exists). Annotating just the parameter without the return type is a parse error (E1xxx). `docs/ori_lang/v2026/spec/14-expressions.md:149` pins the four valid shapes:

```ori
// Before (fails with E2005 on `d` closure parameter after §03 validator fires):
durations.map(d -> d.minutes())

// After — typed_lambda form (param type + return type + `=` body all required together):
durations.map((d: Duration) -> int = d.minutes())
```

**Discovery command** (narrows to §03.N's ledger subset with lambda-parameter
method chains):

```bash
# Files from the 35-file ledger that contain `.map(`, `.filter(`, `.fold(`,
# `.find(`, `.any(`, `.all(`, `.for_each(`, `.flat_map(` with a bare-name
# lambda parameter `x ->` or `(x) ->` (no annotation).
for f in $(grep -l '' /dev/null \
    tests/spec/capabilities/propagation.ori \
    tests/spec/declarations/stdlib/testing_assert_eq.ori \
    tests/spec/declarations/test_variant_match.ori \
    tests/spec/declarations/traits.ori \
    tests/spec/expressions/field_access.ori \
    tests/spec/imports/generic_import.ori \
    tests/spec/inference/generics.ori \
    tests/spec/inference/unification.ori \
    tests/spec/lexical/delimiters.ori \
    tests/spec/lexical/keywords.ori \
    tests/spec/lexical/operators.ori \
    tests/spec/patterns/data.ori \
    tests/spec/patterns/match.ori \
    tests/spec/traits/core/comparable.ori \
    tests/spec/traits/core/compound_equals.ori \
    tests/spec/traits/core/compound_hash.ori \
    tests/spec/traits/core/option.ori \
    tests/spec/traits/debug/join.ori \
    tests/spec/traits/into/str_to_error.ori \
    tests/spec/traits/iterator/methods.ori \
    tests/spec/traits/traceable/definition.ori \
    tests/spec/traits/traceable/result_delegation.ori \
    tests/spec/types/duration_size_default.ori \
    tests/spec/types/enum/niche/niche_cross_feature.ori \
    tests/spec/types/enum/niche/option_str.ori \
    tests/spec/types/existential.ori \
    tests/spec/types/never.ori \
    tests/spec/types/option/map.ori \
    tests/spec/types/option/ok_or.ori \
    tests/spec/types/primitives.ori \
    tests/spec/types/result/map.ori \
    tests/compiler/typeck/collections.ori \
    tests/compiler/typeck/control_flow.ori \
    tests/compiler/typeck/generics.ori \
    tests/compiler/typeck/let_bindings.ori ; do
  rg -l '\.(map|filter|fold|find|any|all|for_each|flat_map|flatten|take|skip|reduce)\(' "$f" && echo "    ^ inspect for bare-name closure params"
done
```

**Representative lambda-parameter hits from §03.N (concrete annotation work):**

- [ ] `tests/spec/types/primitives.ori:~1584` — `[Duration...].map(d -> d.minutes())` → `.map((d: Duration) -> int = d.minutes())` (typed_lambda form — param type + return type + `=` all required per spec grammar).
- [ ] `tests/spec/types/option/map.ori` — `Option::map` closure param annotation.
- [ ] `tests/spec/types/result/map.ori` — `Result::map` closure param annotation.
- [ ] `tests/spec/traits/core/option.ori` — closure params in `Option` trait tests.
- [ ] `tests/spec/traits/core/comparable.ori` — closure params in `Comparable` trait tests.
- [ ] `tests/spec/traits/core/compound_equals.ori` — closure params in compound-equality tests.
- [ ] `tests/spec/traits/core/compound_hash.ori` — closure params in compound-hash tests.
- [ ] `tests/spec/traits/iterator/methods.ori` — closure params across `.map` / `.filter` / `.fold` in iterator-method tests.
- [ ] `tests/spec/traits/traceable/definition.ori` — closure params in traceable definition tests.
- [ ] `tests/spec/traits/traceable/result_delegation.ori` — closure params in traceable-result tests.
- [ ] `tests/spec/inference/generics.ori` — closure param in generic inference edge tests.
- [ ] `tests/spec/inference/unification.ori` — closure param in unification-edge tests.

Note: several files appear in BOTH §06.2 (empty-literal class) AND §06.2B
(lambda-parameter class). For those, annotate BOTH classes before marking the
`[x]` in §06.2B; §06.2's checkbox and §06.2B's checkbox each gate on that file
being green with both classes of annotations applied. Duplication is tolerated
in the checklist for traceability — a file with hits in both classes needs two
editorial passes.

**Dual-execution parity requirement:** Annotating a lambda parameter changes the
AST surface seen by typeck but MUST NOT change observable semantics. Run
`timeout 150 diagnostics/dual-exec-verify.sh --json | jq '.per_test[] | select(.parity_status != "match")'`
after §06.2B commits; output MUST be empty (no divergences on any touched file).
If divergence appears, the annotation is wrong — revert and file `/add-bug` on
the specific pattern.

### Close §06.2B

- [ ] All boxes above marked `[x]`; lambda-parameter class cleared from the §03.N ledger.
- [ ] `diagnostics/dual-exec-verify.sh` shows zero parity divergences on every touched file.
- [ ] `timeout 150 cargo st tests/spec/traits/` + `tests/spec/types/` + `tests/spec/inference/` all green.

---

## 06.3 Annotation Sweep — library/std/

Stdlib empty-literal exposure: `rg '\[\s*\]' library/std/ --glob '*.ori'` +
`rg '\{\s*\}' library/std/ --glob '*.ori'` + `rg '\.(map|filter|fold|find)\(' library/std/ --glob '*.ori'`.

Per §03.N line 1282, `library/` currently has 0 failing files in the ledger
because all stdlib empty-literal occurrences have constraining uses or flow
through the end-of-body defaulting pre-pass. Include §06.3 as a verification
step that confirms this continues to hold after §06.2 + §06.2B edits — stdlib
is compiled through the same typeck pipeline, so a regression here would
cascade to every user.

- [ ] Run the three discovery commands above; inspect each hit manually.
- [ ] For each empty-literal hit: confirm type context (call site, end-of-body defaulting, explicit annotation) resolves the element type; if not, add `let x: [T] = []` at the binding. Expected result: no new annotations needed (per §03.N baseline).
- [ ] For each lambda-parameter hit: confirm bidirectional propagation resolves the parameter; if not, add the full typed_lambda form `(x: T) -> R = body` (spec grammar requires param type + return type + `=` body together — the `(x: T) ->` shorthand does not parse).
- [ ] `timeout 150 cargo st library/std/` green (if a spec runner covers stdlib) AND `timeout 150 cargo t -p ori_std` green (if `ori_std` has its own Rust test surface).
- [ ] Document findings in §06.N: if §06.3 added any annotations, list them; if it added zero, record "stdlib baseline preserved — no annotations needed".

---

## 06.4 Regression Verification + state.sh + §03.N Ledger Sync

This is the load-bearing success-criteria subsection. `cargo st <path>` returns
exit 0 for non-existent paths, so §05's existing warning (`section-05-test-matrix.md:18,1212`)
applies here too — do NOT rely on a per-path `cargo st` as the sole regression
gate. Use `test-all.sh` (authoritative) PLUS explicit file count assertions on
`diagnostics/state.sh`.

### 06.4.1 Authoritative regression run

- [ ] `timeout 150 ./test-all.sh` green (debug build). Record the actual pass/fail/skipped numbers; compare against the `diagnostics/state.sh show --json` baseline from §03.N (`passed: 16374, failed: 844, skipped: 160`). Expected post-§06: `passed: 17218, failed: 0, skipped: 160` (+844 passed, −844 failed, skipped unchanged).
- [ ] `timeout 150 cargo test --release -p ori_types` green (release build).
- [ ] `timeout 150 cargo test --release -p ori_llvm` green (release build).
- [ ] `timeout 150 ./clippy-all.sh` clean.
- [ ] `timeout 150 ./llvm-test.sh` green.

### 06.4.2 Directory-scoped spot checks

These are informational and non-authoritative (see `cargo st` false-pass
warning above); they localize failures if §06.4.1 regresses.

- [ ] `timeout 150 cargo st tests/spec/` — green (no `E2005` across the directory).
- [ ] `timeout 150 cargo st tests/spec/types/empty_literals/` — green, identical pass/fail counts to §03.BUG-FIXES baseline (21 files, 0 failures). Defaulting corpus untouched.
- [ ] `timeout 150 cargo st tests/spec/types/collections/empty_list/` — green, positive+negative pins from §05 all pass (§05's canonical corpus, untouched by §06).
- [ ] `timeout 150 cargo st tests/spec/traits/iterator/` — green (heavy lambda-parameter-class test coverage; §06.2B completeness check).
- [ ] `timeout 150 cargo st tests/compiler/typeck/` — green (the 4-file subset of §03.N).

### 06.4.3 state.sh + §03.N ledger sync

The `known_failing_files` list at `.claude/state/known-state.json` is NOT
auto-populated from `test-all.sh` — per `diagnostics/state.sh:400-401` it
reflects plan intent and requires explicit editing. §06.4 MUST update both.

- [ ] Run `timeout 150 diagnostics/state.sh refresh --full --by section-06`. This updates test totals from the actual `test-all.sh` run.
- [ ] Edit `.claude/state/known-state.json` directly (or via a state.sh subcommand if one ships): set `known_failing_files: []`, `known_failing_count: 0`, `diagnostic_count: 0`, `failure_class: null`, `remediation: []`. Commit alongside the §06 annotation commit(s).
- [ ] Edit `plans/empty-container-typeck-phase-contract/section-03-bodies-pass-integration.md` Known Failing Tests section (`~:1227-1299`): prepend a dated note `> **RESOLVED 2026-04-XX by §06**: All 35 files remediated via §06.2 (empty-literal class) + §06.2B (lambda-parameter class). state.sh cache now shows 0 known-failing files. Historical table preserved below for audit.` Strike through but do NOT delete the file list — `/continue-roadmap` scanner and TPR audit trails reference it.
- [ ] Confirm `diagnostics/state.sh check` reports `status: green` (or equivalent clean state indicator).

### 06.4.4 §04 + §07 handoff

- [ ] Confirm §04's `unresolved type variable at codegen` diagnostic does NOT fire on any file in `tests/spec/` — run `diagnostics/detect-tag-var-at-codegen.sh tests/spec/` (or equivalent per §04 design) with zero hits. If §04 is still in progress when §06 closes, defer this check to §04's completion checklist and cross-link.
- [ ] Prepare the §07 handoff: §06 touched spec + stdlib files; §07's annotation-cleanup pass (`section-07-closeout.md §07.2` — `rg 'BUG-04-074|empty-container-typeck' tests/ library/`) MUST return zero hits from §06 edits. §06 annotations are permanent (they're the spec-compliant form); any `§06.*` / `BUG-04-074` markers §06 temporarily embedded for tracking MUST be stripped before §07 audits.

---

## 06.R Third Party Review Findings

Round 2 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini). Findings
addressed in prior revisions.

### [[TPR-06-001-codex]] [HIGH] Broaden the annotation sweep to all empty-list call patterns

**Location:** `plans/empty-container-typeck-phase-contract/section-06-diagnostics-audit.md:70`
**Reviewer:** Codex | **Status:** Fixed (R2) — superseded by §06.2 re-scope in R5

**Evidence:** Section 06 originally instructed only `rg 'let .* = \[\]' tests/spec/` and
listed two known hits under `tests/spec/collections/cow/double_ended*.ori` — but those
files do not exist. A broader repo scan found current empty-list sites that this grep
misses, including direct-receiver forms in `tests/spec/lexical/delimiters.ori:151`,
`tests/spec/lexical/keywords.ori:222`, `tests/spec/traits/iterator/double_ended.ori:167`,
`tests/spec/traits/iterator/double_ended_methods.ori:35`, and
`tests/spec/collections/cow/matrix_map_set.ori:94`.

**Fix:** Replaced the narrow `let x = []` grep with a two-command sweep: one for
`let.*=\s*\[\s*\]` (let-binding forms, tolerates whitespace variation) and one for
`\[\]\.` (expression-position bare `[]` receiver chains). Updated the known-hit list to
real file paths under `tests/spec/traits/iterator/` and other locations discovered by
the sweep.

### [[TPR-06-002-gemini]] [LOW] Improve unannotated empty list sweep regex to handle whitespace

**Location:** `plans/empty-container-typeck-phase-contract/section-06-diagnostics-audit.md:70`
**Reviewer:** Gemini | **Status:** Fixed

**Evidence:** The original regex `rg 'let .* = \[\]'` strictly requires exactly one space
around `=` and no spaces inside `[]`. Valid Ori code like `let x=[]` or `let y = [ ]`
would be silently missed.

**Fix:** Changed to `rg 'let.*=\s*\[\s*\]'` (allows any whitespace around `=` and inside
`[]`). Also updated the success_criteria regex in the frontmatter to match.

Round 3 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini). Findings addressed.

### [[TPR-06-R3-001-codex+gemini]] [HIGH] Broaden sweep beyond let-bindings and receiver chains

**Location:** `plans/empty-container-typeck-phase-contract/section-06-diagnostics-audit.md:§06.2`
**Reviewers:** Codex + Gemini | **Status:** Fixed (R3) — superseded by §06.2 re-scope in R5

**Evidence:** The two-command sweep (`let.*=\s*\[\s*\]` + `\[\]\.`) was added in Round 2 to
catch let-binding forms and receiver chains. Both reviewers independently verified via repo
grep that this still misses empty-list usage in argument position (`foo(items: [])`), operator/
concatenation position (`[] + [1, 2, 3]`), `for...in []` position, and return-from-block.

**Fix:** Replaced the two-command sweep with a single comprehensive `rg '\[\s*\]' tests/spec/ library/ --glob '*.ori'` that covers all syntactic positions.

Round 4 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini).

### [[TPR-06-R4-001-codex]] [MEDIUM] Comprehensive sweep misses tests/valgrind/ operator-position hit

**Location:** `plans/empty-container-typeck-phase-contract/section-06-diagnostics-audit.md:77`
**Reviewer:** Codex | **Status:** Fixed

**Fix:** Added `tests/valgrind/` to the comprehensive sweep.

### [[TPR-06-R4-002-gemini]] [LOW] Empty list pattern-match arms described as "irrefutable" — incorrect

**Location:** `plans/empty-container-typeck-phase-contract/section-06-diagnostics-audit.md:93,104`
**Reviewer:** Gemini | **Status:** Fixed

**Fix:** Pattern-match arms are refutable; E2005 exemption comes from scrutinee-constrained top-down typing, not irrefutability. Clarified the prose.

### Round 5 — /review-plan Step 4 blind-spots (dual-source /tp-help, 2026-04-20) — addressed by editor

The /review-plan Step 4 /tp-help run surfaced 10 blind spots + 5 architectural
risks + 5 cross-cutting concerns on which codex + gemini converged. The editor
applied the following structural changes to §06 in Step 5 (this round). Recording
them here to preserve the rationale and ensure §06 TPR Round 5 doesn't re-flag
them.

#### [[TPR-06-R5-001-codex+gemini]] [HIGH] Scope mismatch: §06.2 regex misses 26 lambda-parameter files + empty maps/sets

**Resolution:** Split annotation sweep into §06.2 (empty-literal class, 35-file
subset) and §06.2B (lambda-parameter class, ~12-file subset). Added empty-map
and empty-set discovery commands to §06.2. Per-file checkbox tracking for every
file in §03.N's 35-file ledger.

#### [[TPR-06-R5-002-codex+gemini]] [HIGH] Premature #compile_fail before §03 validator wires guarantees failures

**Resolution:** Explicit sequencing rule at top of section ("§06 MUST NOT start
until §03's validator is live…"). §03 is already `status: complete` per
`index.md`, so this is satisfied; the rule documents the dependency for future
plan re-execution.

#### [[TPR-06-R5-003-codex+gemini]] [HIGH] §06.N blob-checkbox tracking desyncs with state.sh + §03's 35-file ledger

**Resolution:** Replaced single `§06.N` blob checkboxes with per-file tasks in
§06.2 and §06.2B. §06.4.3 adds explicit `state.sh refresh` + `.claude/state/known-state.json`
edit + §03.N ledger strike-through steps.

#### [[TPR-06-R5-004-codex+gemini]] [MEDIUM] `#compile_fail` is file-level — cannot be applied to mixed suites in place

**Resolution:** Fix rule #4 under §06.2 explicitly forbids adding `#compile_fail(code: "E2005")`
to any file in the ledger. That tag belongs exclusively to §05's new negative-pin
corpus at `tests/spec/types/collections/empty_list/`. §06 only adds concrete
annotations (`let x: [T] = []`).

#### [[TPR-06-R5-005-gemini]] [HIGH] Mechanical annotation destroys [Never]-defaulting coverage from §03.BUG-FIXES

**Resolution:** `[Never]` defaulting protection note in §06.2 scope header;
`tests/spec/types/empty_literals/` (21 files) explicitly excluded from the
annotation sweep. §06.4.2 spot-checks that directory's pass counts pre- and
post-§06 to confirm no regression.

#### [[TPR-06-R5-006-codex]] [MEDIUM] §06.N `cargo st <path>` false-passes on non-existent paths

**Resolution:** §06.4 splits into 06.4.1 (authoritative `test-all.sh`
regression), 06.4.2 (informational directory spot checks — documented as
non-authoritative per §05's existing warning), and 06.4.3 (state.sh + §03.N
ledger sync with explicit count assertions). The file-count assertion in
success_criteria (`known_failing_count == 0`) catches silent regressions that
`cargo st` misses.

#### [[TPR-06-R5-007-codex]] [MEDIUM] E2005 wording pins empty-list-specific message; §03 failing class includes other E2005 sources

**Resolution:** §06.1 now dispatches the message by `ExprKind` — empty-list
sites get the empty-list-specific message; lambda-parameter sites get a
parameter-specific message; all other positions preserve the generic wording.
Negative pin `test_e2005_message_falls_back_to_generic_for_signature_var` guards
against over-dispatch.

#### [[TPR-06-R5-008-codex+gemini]] [MEDIUM] §03.N's 35-file ledger + state.sh cache + §06 annotation are three sources that drift

**Resolution:** §06.4.3 makes the sync explicit and load-bearing:
`state.sh refresh --full`, then manual edit of `known-state.json`, then strike-
through update of §03.N's table — all in the same commit. success_criteria
asserts the post-§06 state (`known_failing_count == 0`) as a machine-checkable
anchor.

#### [[TPR-06-R5-009-codex]] [LOW] §05 owns canonical empty-literal corpus; §06 adds `#compile_fail` = redundant coverage

**Resolution:** Scope note at top of section clarifies §06 does NOT add new
`#compile_fail` tests — that's §05's job. Fix rule #4 enforces. success_criteria
asserts `#compile_fail(code: "E2005")` is NOT added to any file in §03.N's
ledger.

#### [[TPR-06-R5-010-codex+gemini]] [MEDIUM] §07 close-out expects cleanup across §06 spec + stdlib edits; handoff contract missing

**Resolution:** §06.4.4 adds the explicit §04 + §07 handoff: enumerates what
§06 hands off (stripped plan markers, permanent spec-compliant annotations) vs
what §07 inherits (the `BUG-04-074|empty-container-typeck` regex sweep over
tests/ and library/).

### Round 6 — /review-plan Step 6 TPR convergence (dual-source codex + gemini, 2026-04-20) — CONVERGED

3 rounds dispatched (R0+R1+R2); 3 verified findings total, all fixed inline.
Gemini converged clean by R1. Codex raised 4 additional claims across R1+R2; 2
verified (both fixed), 4 dropped at §4 verification as hallucinated/misapplied.

- [x] `[TPR-06-R0-001-codex][high]` §06:N/A — "seven `- [ ]` items re: error codes". Fixed (dropped at verification): quoted evidence absent from §06 per grep — codex hallucinated different-plan content. No code change.
- [x] `[TPR-06-R0-002-codex][medium]` §06:frontmatter — "no blocker annotation". Fixed (dropped at verification): §06 frontmatter has canonical `depends_on: [01..05]`; §07 has `depends_on: [...06...]`. Rule was misapplied — `<!-- blocked-by -->` is item-level. No code change.
- [x] `[TPR-06-R0-003-gemini][high]` §06:99,259,311 — "Invalid lambda syntax `(d: Duration) -> d.minutes()` violates spec grammar.ebnf §550-553". Fixed in `52aa673d`: all 3 instances use spec-compliant typed_lambda `(d: Duration) -> int = d.minutes()`.
- [x] `[TPR-06-R0-004-gemini][low]` §06:396 — "Stale state.sh:400-401 line ref". Fixed (dropped at verification): verified `diagnostics/state.sh:400` IS the manual-ledger note (2-line range correct). No code change.
- [x] `[TPR-06-R1-001-codex][medium]` §06:123,360 — "Residual typed-lambda shorthand in diagnostic message + checklist prose". Fixed in `7caff2b9`: diagnostic now `(x: int) -> ReturnT = body`; checklist prose now `(x: T) -> R = body` with explicit spec-grammar note.
- [x] `[TPR-06-R1-002-codex][medium]` §06:122 — "E2005 wording widened to Map violates overview Design Principle 4 (list-only scope)". Fixed in `7caff2b9`: ExprKind dispatch restricted to List/ListWithSpread; Map/Set sites fall back to generic.
- [x] `[TPR-06-R1-INFO-gemini][informational]` §06:259 — Confirms Round 0 lambda fix valid per spec grammar. No action (advisory).
- [x] `[TPR-06-R2-001-codex][high]` §08:N/A — "fabricated spec clause 8.4 lambda codegen". Fixed (dropped at verification): phrase doesn't exist in §08 (grep count 0); also out-of-scope for §06 review. No code change.
- [x] `[TPR-06-R2-002-codex][medium]` §08:N/A — "missing dual-execution parity success criterion". Fixed (dropped at verification): §08 has 3 explicit dual-exec-verify success criteria at lines 405, 697, 797. No code change.
- [x] `[TPR-06-R2-INFO-gemini][informational]` §06:259 — Re-confirms typed_lambda fix. No action (advisory).

**Convergence verdict:** Clean exit after R2. Gemini `status: clean` both R1 and R2. Codex R2 returned `status: findings` but all claims dropped at §4 verification (fabricated content about out-of-scope §08). Functionally converged — zero remaining verified findings. `reviewed: true` to be flipped by /review-plan Step 7+8.

---

## 06.N Completion Checklist

- [ ] **06.1 complete** — E2005 message wording finalized; ExprKind-dispatched message + span-specific tests pass in `check/validators/tests.rs`.
- [ ] **06.2 complete** — empty-literal class cleared; every checkbox in §06.2 marked `[x]`; `tests/spec/types/empty_literals/` untouched and still green.
- [ ] **06.2B complete** — lambda-parameter class cleared; every checkbox in §06.2B marked `[x]`; `diagnostics/dual-exec-verify.sh` shows zero parity divergences.
- [ ] **06.3 complete** — library/std/ sweep done; findings documented (expected: zero annotations needed, baseline preserved).
- [ ] **06.4.1 complete** — `timeout 150 ./test-all.sh` green; delta vs §03.N baseline = +844 passed, −844 failed.
- [ ] **06.4.2 complete** — all five directory spot checks green.
- [ ] **06.4.3 complete** — `diagnostics/state.sh show --json` reports `known_failing_count: 0`, `known_failing_files: []`; §03.N Known Failing Tests table strike-through update landed.
- [ ] **06.4.4 complete** — §04's codegen diagnostic returns zero hits on `tests/spec/`; §07 handoff checklist satisfied (no `BUG-04-074|empty-container-typeck` markers in spec/stdlib edits).
- [x] `/tpr-review` passed on this section — Round 6 converged clean after 3 dual-source rounds (commits `52aa673d`, `7caff2b9`); all 3 verified findings fixed inline; remaining codex claims dropped at §4 verification. Details in §06.R Round 6 block.
- [ ] `/impl-hygiene-review` passed — no prose violations in this file; no mechanical annotations in `tests/spec/types/empty_literals/`.
- [ ] `/improve-tooling` sweep — any regex or discovery-command limitations hit during §06.2 / §06.2B surfaced as tooling improvements (expected: `state.sh` probably wants an explicit `clear-known-failing --plan <name>` subcommand; file `/improve-tooling` at close-out).
- [ ] **Plan sync** — this section's frontmatter `status` → `complete`; `00-overview.md` Quick Reference table entry updated to `Complete`; `index.md` section 06 status updated; `00-overview.md` Mission Success Criteria checkboxes for E2005/annotation/state.sh-cache-clear satisfied.

**Exit criteria:** Full test suite green (`test-all.sh`, release, clippy, llvm-test). No
unannotated empty lists AND no unannotated closure parameters in test or stdlib
code that would surprise users after the fix lands. `diagnostics/state.sh` cache
reports zero known-failing files. §03.N ledger is updated in place to reflect
remediation. §04's codegen-time assertion never fires on any program in
`tests/spec/`. Section 07 may begin.
