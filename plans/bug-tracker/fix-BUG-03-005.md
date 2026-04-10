---
bug: "BUG-03-005"
title: "Map key strings include surrounding type prefixes in interpreter — dual-execution mismatch with AOT"
severity: "high"
status: not-started
goal: "Map iteration and display in the interpreter return user-visible keys without internal type prefixes, matching AOT behavior"
success_criteria:
  - "for (k, v) in {\"hello\": 1} do print(msg: k) prints 'hello' not 's:hello'"
  - "print(msg: {\"hello\": 1}) prints '{hello: 1}' not '{s:hello: 1}'"
  - "All key types (str, int, bool, char, byte) decode correctly in iteration and display"
  - "decode_map_key in ori_eval delegates to Value::from_map_key — SSOT, no duplication"
subsystem: "compiler/ori_patterns/src/value/, compiler/ori_eval/src/methods/"
found: "2026-04-10"
source: "continue-roadmap"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-03-005 — Map key strings include type prefixes in interpreter

**Status:** Not Started
**Severity:** high
**Goal:** Map iteration, display_value (Printable), Display, and debug_value all return user-visible keys without the internal type prefix (e.g., `"s:"`, `"i:"`), matching AOT behavior.

**Success Criteria:**
- [ ] `for (k, v) in {"hello": 1} do print(msg: k)` prints `hello` not `s:hello`
- [ ] `print(msg: {"hello": 1})` prints `{hello: 1}` not `{s:hello: 1}`
- [ ] All key types decode correctly in iteration and display
- [ ] `decode_map_key` in `ori_eval` delegates to `Value::from_map_key` — SSOT

**Context:** Maps in the interpreter store keys as type-prefixed strings (`"s:hello"`, `"i:42"`) via `to_map_key()` for internal collision avoidance. Four code paths leak these internal keys to user-visible output. The `.keys()` and `.entries()` methods correctly decode via `decode_map_key()` in `ori_eval`, but the iterator, display_value, Display, and debug_value paths don't. AOT is correct. Found during diagnostic-tooling-improvements §06.1 fixture creation.

---

## 1. Root Cause Analysis

- **Symptom**: `for (k, v) in {"hello": 1} do print(msg: k.length())` prints 7 (interpreter) vs 5 (AOT). Map display shows `{s:hello: 1}` instead of `{hello: 1}`.
- **Proximate cause**: Four code paths return raw BTreeMap keys without decoding the type prefix.
- **Root cause**: `to_map_key()` adds type prefixes for collision avoidance. The inverse decode function (`decode_map_key`) exists only in `ori_eval`, unreachable from `ori_patterns` where the iterator and display code lives. No canonical inverse exists in `ori_patterns`.
- **Blast radius**: All map iteration (`for...in`), all map display (Printable, Display, Debug). Does NOT affect map indexing (uses `to_map_key` for lookup — correct), `.keys()`, `.entries()` (use `decode_map_key` — correct).
- **Affected files**:
  - `compiler/ori_patterns/src/value/conversions.rs` — add `from_map_key()`, fix `display_value` Map branch
  - `compiler/ori_patterns/src/value/iterator/next.rs` — fix Map iterator `next()`
  - `compiler/ori_patterns/src/value/traits.rs` — fix `Display` for Map
  - `compiler/ori_eval/src/methods/helpers/mod.rs` — fix `debug_value` Map branch to use `from_map_key`
  - `compiler/ori_eval/src/methods/collections.rs` — delegate `decode_map_key` to `Value::from_map_key()`

---

## 1.5 Fix Consensus (via /tp-help)

- **Proposed approach (pre-consensus)**: Add `Value::from_map_key()` to `ori_patterns/value/conversions.rs` as SSOT inverse of `to_map_key()`. Fix all 4 leak paths. Delegate existing `decode_map_key` to eliminate duplication.
- **tp-help run scratch dir**: `/tmp/ori-tpr-Jk44gFNn`

### Round 1
- **Codex summary**: Agrees `from_map_key` in `ori_patterns` is the right SSOT home and dependency direction. Found a 4th leak path (`debug_value` in `helpers/mod.rs:191` using hand-rolled `split_once(':')`). Flagged tuple key encoding as non-bijective (strings containing `;` create ambiguity). Recommends making codec fully reversible before shipping.
- **Gemini summary**: Stalled during composing phase after investigating Set display and IteratorValue structure. No final answer produced.
- **Agreement points**: `from_map_key` location correct; 4th leak path confirmed; SSOT delegation of `decode_map_key` correct.
- **Disagreement points**: Codex wants full codec redesign (length-prefixed encoding) before fixing display paths. Claude considers this a scope escalation — the existing `decode_map_key` approach is production-correct for `.keys()`/`.entries()` and the same approach applies to the other paths.
- **Independent code verification**:
  - `helpers/mod.rs:191` — VERIFIED: hand-rolled `split_once(':')` is LEAK:shadow-home (confirmed by reading the code)
  - Tuple encoding ambiguity — VERIFIED: `to_map_key` for tuples uses `;`-separated prefixed keys with no escaping. `("a;b",)` → `"t:s:a;b;"` and `("a", "b")` → `"t:s:a;s:b;"` — these are actually NOT the same. But `("a:b",)` → `"t:s:a:b;"` could confuse `split_once(':')`. The recursive `from_map_key` approach handles this correctly by taking everything after the FIRST `:` as the value, which works for all non-nested types.
- **Outcome**: Proceed with proposed approach. Codex's codec redesign recommendation is a valid future enhancement but exceeds the scope of this bug fix.

### Final agreed approach
Add `Value::from_map_key()` as the SSOT inverse of `to_map_key()` in `ori_patterns`. Fix all 4 leak paths (iterator next, display_value, Display, debug_value). Delegate `decode_map_key` in `ori_eval` to the new function. Handle all simple key types correctly; for nested types (Some/Ok/Err/Tuple), implement recursive decode where unambiguous. NOTE-level limitation: tuple keys with strings containing the `;` separator may decode imperfectly (same as existing `decode_map_key` behavior — not a regression).

---

## 2. TDD — Test Matrix

### Exact failing case
- [ ] String key iteration: `for (k, v) in {"hello": 1} do assert_eq(actual: k, expected: "hello")`
- [ ] String key length: `for (k, v) in {"hello": 1} do assert_eq(actual: k.length(), expected: 5)`

### Edge cases
- [ ] Empty string key: `{"": 1}` — key is `""` not `"s:"`
- [ ] Key containing colon: `{"a:b": 1}` — key is `"a:b"` not truncated
- [ ] Multiple keys: `{"a": 1, "b": 2}` — both decoded correctly

### Cross-type coverage
- [ ] Int key iteration: `{42: "v"}` — key is `42` (int), not `"i:42"` (string)
- [ ] Bool key iteration: `{true: "v"}` — key is `true` (bool)
- [ ] Char key iteration: map with char key — key is char value
- [ ] Byte key iteration: map with byte key — key is byte value

### Cross-feature interactions
- [ ] Map display (Printable): `print(msg: {"hello": 1})` shows `{hello: 1}`
- [ ] Map with mixed key types: `{42: "a", "hello": "b"}` — both keys decoded correctly
- [ ] Map .keys() consistency: iteration keys match .keys() output

### Semantic pin
- [ ] String key length semantic pin: `k.length() == 5` for key `"hello"` — ONLY passes with decoded keys

### Negative pin
- [ ] No type prefix in iteration output: key string does NOT start with `"s:"`
- [ ] No type prefix in display: map display does NOT contain `"s:"`

### Verify tests fail before fix
- [ ] All new tests fail against current code

---

## 3. Implementation

- [ ] Add `pub fn from_map_key(key: &str) -> Value` to `compiler/ori_patterns/src/value/conversions.rs`
  - Handle: `s:` → Str, `i:` → Int, `f:` → Float, `b:` → Bool, `c:` → Char, `y:` → Byte, `d:` → Duration, `z:` → Size, `o:` → Ordering, `n:` → None, `S:` → Some (recursive), `O:` → Ok (recursive), `E:` → Err (recursive), `t:` → Tuple (split on `;`, recursive)
  - Fallback: unknown prefix → return as string
- [ ] Fix `IteratorValue::Map::next()` in `next.rs:76`: `Value::from_map_key(key)` instead of `Value::string(key.clone())`
- [ ] Fix `display_value` Map branch in `conversions.rs:154`: decode key, format decoded value
- [ ] Fix `Display` Map branch in `traits.rs:98`: decode key, format with type-appropriate quoting
- [ ] Fix `debug_value` Map branch in `helpers/mod.rs:191`: use `Value::from_map_key()` instead of hand-rolled `split_once`
- [ ] Delegate `decode_map_key` in `collections.rs:19` to `Value::from_map_key()`

---

## R. Third Party Review Findings

{Initially empty — populated during Phase 5.}

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix
- [ ] Matrix completeness verified
- [ ] Debug AND release builds pass
- [ ] Interpreter and LLVM produce identical results for all new tests
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `/commit-push` — commit all changes before review
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed
- [ ] Bug entry updated
- [ ] Fix section status updated to `complete`
- [ ] Overview count updated
- [ ] Final `/commit-push`

**Exit Criteria:** `timeout 30 cargo run --bin ori -- run /tmp/map_key_bug.ori` prints `hello` and `5` (not `s:hello` and `7`). Map display shows decoded keys without type prefixes. All key types (str, int, bool, char, byte) decode correctly in iteration, display_value, Display, and debug_value. `timeout 150 ./test-all.sh` green with 0 regressions.
