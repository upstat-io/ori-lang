# Journey 12: "I am an option"

**Code**:
```ori
@safe_div (a: int, b: int) -> Option<int> =
    if b == 0 then None else Some(a / b);

@unwrap_or (opt: Option<int>, default: int) -> int =
    match opt { Some(v) -> v, None -> default }

@check_some () -> int = {
    let a = safe_div(a: 100, b: 5);
    unwrap_or(opt: a, default: 0)
    // = 20
}

@check_none () -> int = {
    let b = safe_div(a: 100, b: 0);
    unwrap_or(opt: b, default: 5)
    // = 5
}

@check_chain () -> int = {
    let x = unwrap_or(opt: safe_div(a: 80, b: 10), default: 0);
    let y = unwrap_or(opt: safe_div(a: 50, b: 0), default: 0);
    x + y
    // = 8 + 0 = 8
}

@try_div (a: int, b: int, c: int) -> Option<int> = {
    let x = safe_div(a: a, b: b)?;
    safe_div(a: x, b: c)
}

@check_prop () -> int = {
    let ok = unwrap_or(opt: try_div(a: 1000, b: 10, c: 5), default: -1);
    let fail_first = unwrap_or(opt: try_div(a: 1000, b: 0, c: 5), default: -10);
    let fail_second = unwrap_or(opt: try_div(a: 1000, b: 10, c: 0), default: -10);
    ok + fail_first + fail_second
    // = 20 + (-10) + (-10) = 0
}

@main () -> int = {
    let a = check_some();     // = 20
    let b = check_none();     // = 5
    let c = check_chain();    // = 8
    let d = check_prop();     // = 0
    a + b + c + d             // = 33
}
```
**Source**: 1446 bytes, **Expected Result**: 33 (= 20 + 5 + 8 + 0)
**Actual**: Eval = 33 (correct), **AOT = 144 (WRONG — tag inversion in Option match)**

**CRITICAL**: Built-in `Option<T>` match has inverted tag numbering in switch codegen. Construction correctly assigns tag 0 = Some, tag 1 = None, but the match switch maps tag 1 → Some arm, tag 0 → None arm. All match-based Option unwrapping silently returns wrong values. User-defined sum types are NOT affected.

---

## Transformation Timeline

### Stage 1-2: Lexer
```
1446 bytes → 426 tokens (12 comments, 0 errors)
Prelude: 10,331 bytes → 1516 tokens (126 comments)
```
- 3.4 bytes/token ratio (moderate — named arguments add tokens)

### Stage 3: Parser
```
426 tokens → 8 functions, 0 types, 105 expressions, 0 errors
Prelude: 1516 tokens → 9 functions, 39 traits, 46 expressions
```
- 8 functions: safe_div, unwrap_or, check_some, check_none, check_chain, try_div, check_prop, main
- No user-defined types — Option is built-in from prelude

### Stage 4: Type Checker
```
registration: 9 functions (prelude), 8 functions (user), 0 tests, 0 impls
signatures: collected for all functions
body checking: complete (prelude + user)
```
- Option<int> monomorphized from generic `Option<T>` definition in prelude
- `?` operator type-checked correctly (return type must be Option)

### Stage 5: Canonicalizer
```
canon lower_module started (functions=8, source_exprs=105)
canon lower_module complete (canon_nodes=117, roots=8, constants=6, decision_trees=1)
```
- 11.4% canon expansion (105→117) — match desugaring adds nodes
- 1 decision tree (for the match in unwrap_or)
- 6 constants (numeric literals)

### Stage 6a: Eval Path
```
223 eval_can calls
```
- Moderate — Option construction/matching through function calls adds overhead
- All paths produce correct results

### Stage 6b: LLVM Path

#### Type Representation
```llvm
; Option<int> = { tag: i64, payload: i64 }
; Represented as: { i64, i64 }
; Some(42) = { 0, 42 } — tag 0 = first variant (Some)
; None     = { 1, ?? } — tag 1 = second variant (None)
```

#### Generated LLVM IR (formatted, key sections)

```llvm
; --- safe_div: construction is CORRECT ---
define fastcc { i64, i64 } @_ori_safe_div(i64 %0, i64 %1) #0 {
bb0:
  %eq = icmp eq i64 %1, 0
  br i1 %eq, label %bb1, label %bb2

bb1:                                ; b == 0 → None
  %variant = alloca { i64, i64 }, align 8
  store i64 1, ptr %variant.tag, align 4    ; ← tag 1 = None (CORRECT)
  ; NOTE: payload at offset 1 never stored, but loaded anyway (UB)
  %variant.f1 = load i64, ptr %variant.f1.ptr, align 4    ; ← reads uninitialized memory!
  ; ... insertvalue to build SSA value ...
  br label %bb3

bb2:                                ; b != 0 → Some(a / b)
  %div = sdiv i64 %0, %1
  %variant1 = alloca { i64, i64 }, align 8
  store i64 0, ptr %variant.tag2, align 4   ; ← tag 0 = Some (CORRECT)
  store i64 %div, ptr %variant.field, align 4  ; ← payload = quotient
  ; ... insertvalue to build SSA value ...
  br label %bb3

bb3:
  %v10 = phi { i64, i64 } [ %variant.s1, %bb1 ], [ %variant.s18, %bb2 ]
  ret { i64, i64 } %v10
}

; --- unwrap_or: match switch is INVERTED ---
define fastcc i64 @_ori_unwrap_or({ i64, i64 } %0, i64 %1) #0 {
bb0:
  %proj.0 = extractvalue { i64, i64 } %0, 0
  switch i64 %proj.0, label %bb4 [
    i64 1, label %bb2               ; ← tag 1 = Some arm (WRONG! Should be tag 0)
    i64 0, label %bb3               ; ← tag 0 = None arm (WRONG! Should be tag 1)
  ]

bb1:                                ; merge
  %v3 = phi i64 [ %proj.1, %bb2 ], [ %1, %bb3 ]
  ret i64 %v3

bb2:                                ; Some arm (extracts payload)
  %proj.1 = extractvalue { i64, i64 } %0, 1
  br label %bb1

bb3:                                ; None arm (returns default)
  br label %bb1

bb4:
  unreachable
}

; --- try_div: ? propagation uses CORRECT tag check ---
define fastcc { i64, i64 } @_ori_try_div(i64 %0, i64 %1, i64 %2) #0 {
bb0:
  %call = call fastcc { i64, i64 } @_ori_safe_div(i64 %0, i64 %1)
  br label %bb1

bb1:
  %proj.0 = extractvalue { i64, i64 } %call, 0
  %eq = icmp eq i64 %proj.0, 0          ; ← tag == 0 means Some (CORRECT!)
  br i1 %eq, label %bb3, label %bb4     ; tag 0 → continue, else → return None

bb3:                                ; Some path — extract value and continue
  %proj.1 = extractvalue { i64, i64 } %call, 1
  br label %bb5

bb4:                                ; None path — construct None and return early
  store i64 1, ptr %variant.tag, align 4    ; ← tag 1 = None (CORRECT)
  ret { i64, i64 } %variant.s1

bb5:
  %call1 = call fastcc { i64, i64 } @_ori_safe_div(i64 %v11, i64 %2)
  ret { i64, i64 } %call1
}

; --- main: 4 calls, no invoke (pure value types) ---
define i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_check_some()
  br label %bb1                          ; ← CONFIRMED M3
bb1:
  %call1 = call fastcc i64 @_ori_check_none()
  br label %bb3                          ; ← CONFIRMED M3
bb3:
  %call2 = call fastcc i64 @_ori_check_chain()
  br label %bb5                          ; ← CONFIRMED M3
bb5:
  %call3 = call fastcc i64 @_ori_check_prop()
  br label %bb7                          ; ← CONFIRMED M3
bb7:
  %add = add i64 %call, %call1
  %add4 = add i64 %add, %call2
  %add5 = add i64 %add4, %call3
  ret i64 %add5
}
```

#### Key Observations

1. **CRITICAL — Tag inversion in Option match switch**: Construction uses tag 0 = Some, tag 1 = None (correct). But the `match` switch maps tag 1 → Some arm, tag 0 → None arm (INVERTED). This causes all `match`-based Option unwrapping to return wrong values.
2. **User-defined sum types NOT affected**: Testing with `type MyOption = MySome(value: int) | MyNone` — match switch correctly maps tag 0 → MySome, tag 1 → MyNone. The bug is specific to built-in generic `Option<T>`.
3. **`?` propagation uses CORRECT tags**: The `?` operator uses `icmp eq i64 %tag, 0` and branches correctly (tag 0 = Some → continue, else → return None). This is a DIFFERENT codegen path from `match`.
4. **Inconsistency within same module**: `try_div` correctly handles `?` (tag 0 = Some), but `unwrap_or` in the same module has inverted match switch. The module produces correct `?` results that get corrupted by `match`.
5. **None variant reads uninitialized payload**: In `safe_div`'s None branch, the codegen loads BOTH fields from the alloca (including the never-stored payload at offset 1). This is LLVM undefined behavior — reading `poison`.
6. **0 runtime declarations**: Pure value types (Option<int>), no ARC overhead. All functions correctly `nounwind`.
7. **9 functions defined**: safe_div, unwrap_or, check_some, check_none, check_chain, try_div, check_prop, _ori_main, main wrapper.
8. **Option<int> as { i64, i64 }**: Clean 16-byte representation. Tag at index 0, payload at index 1. Fits in registers (no alloca needed for passing).
9. **CONFIRMED M3**: Dead `br label` after every function call in main (4 instances).
10. **CONFIRMED M5**: `align 4` on i64 stores in variant construction.
11. **CONFIRMED M7**: alloca+store+load roundtrip for variant construction (both Some and None).
12. **`?` operator codegen is clean**: Direct tag check + conditional branch. No match switch involved. Much simpler than the match path.

---

## Issues Found

### CRITICAL
**C4 (NEW): Built-in Option<T> match — switch tag numbering inverted, silent wrong results**
- Construction: tag 0 = Some (first variant), tag 1 = None (second variant) — **correct**
- `match` switch codegen: maps tag 1 → Some arm, tag 0 → None arm — **INVERTED**
- `?` propagation codegen: `icmp eq tag, 0` for Some — **correct** (different code path)
- **User-defined sum types are NOT affected** — only built-in generic `Option<T>`
- **Root cause**: The match codegen for monomorphized built-in generic types uses a different variant ordering than the construction codegen. The `?` operator avoids this because it uses a direct `icmp eq` rather than a switch.
- **Impact**: ALL match-based Option processing produces wrong results in AOT. Some(x) is treated as None (returns default), None is treated as Some (reads uninitialized payload). This is the second silent miscompilation finding (after C3 in J11).
- **Eval**: Works correctly — the evaluator handles Option match via its own dispatch.

### MEDIUM
**M14 (NEW): None variant codegen loads uninitialized payload from alloca**
- When constructing None (unit variant of Option), the codegen stores only the tag but then loads ALL fields from the alloca, including the payload at offset 1 which was never written.
- This is technically LLVM undefined behavior (load from uninitialized memory produces `poison`).
- In practice, the JIT often zeroes stack memory, masking the bug. But this is NOT guaranteed.
- **Fix**: Skip loading uninitialized payload fields for unit variants, or zero-initialize the alloca.

**CONFIRMED M3**: Dead branches after calls (4 instances in main)
**CONFIRMED M5**: `align 4` on i64 stores in variant construction
**CONFIRMED M7**: alloca+store+load roundtrip for variant construction (all 4 construction sites)

### What Works Exceptionally Well
- **`?` propagation codegen**: Clean `icmp eq i64 %tag, 0` → branch. Correct tag handling. Simpler than match.
- **Option<int> as { i64, i64 }**: 16 bytes, fits in registers. Passed by value in fastcc.
- **No ARC overhead**: Pure value types, 0 runtime declarations.
- **nounwind on everything**: Correct! No ARC, no allocations, no panic paths.
- **safe_div codegen**: Clean if/else → two variant construction paths → phi merge. Correct sdiv usage.
- **check_prop structure**: Correct 3-call pattern with proper default values.
- **Inter-function Option passing**: { i64, i64 } passed/returned by value in fastcc — efficient register usage.

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | **33** | **144 (WRONG)** |
| check_some (safe_div 100/5) | 20 ✓ | **0 ✗** (tag 0=Some → None arm → default) |
| check_none (safe_div 100/0) | 5 ✓ | **0 or garbage ✗** (tag 1=None → Some arm → uninit payload) |
| check_chain (chained calls) | 8 ✓ | **garbage ✗** (all matches inverted) |
| check_prop (? + match) | 0 ✓ | **garbage ✗** (? correct, match inverted) |
| `?` propagation | Correct | **Correct** (uses icmp, not switch) |
| `match` on Option | Correct | **INVERTED** (switch labels flipped) |
| Runtime declarations | 0 | 0 |
| Functions generated | N/A | 9 |

### Detailed Trace: check_some

```
safe_div(100, 5):
  Construction: tag=0 (Some), payload=20 → returns {0, 20}

unwrap_or({0, 20}, 0):
  Eval:   tag=0 → Some arm → returns payload 20 ✓
  LLVM:   tag=0 → switch i64 0 → bb3 (None arm) → returns default 0 ✗
```

### Comparison: User-Defined vs Built-in Sum Type

```
User-defined:  MySome(42)    → tag 0     switch: i64 0 → bb2 (payload) ✓
Built-in:      Some(42)      → tag 0     switch: i64 1 → bb2 (payload) ✗
```
The construction tag is identical (0 for first variant). The switch label is inverted ONLY for the built-in generic type.
