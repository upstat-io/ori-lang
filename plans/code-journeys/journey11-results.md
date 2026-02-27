# Journey 11: "I am a derived trait"

**Code**:
```ori
#[derive(Eq)]
type Point = { x: int, y: int }

#[derive(Eq)]
type Color = Red | Green | Blue;

#[derive(Eq)]
type Shape = Circle(radius: int) | Rect(w: int, h: int);

@check_struct_eq () -> int = {
    let p1 = Point { x: 10, y: 20 };
    let p2 = Point { x: 10, y: 20 };
    let p3 = Point { x: 10, y: 30 };
    let same = if p1 == p2 then 3 else 0;
    let diff = if p1 != p3 then 4 else 0;
    same + diff
    // = 3 + 4 = 7
}

@check_sum_eq () -> int = {
    let c1 = Red;
    let c2 = Red;
    let c3 = Blue;
    let unit_same = if c1 == c2 then 5 else 0;
    let unit_diff = if c1 != c3 then 6 else 0;
    unit_same + unit_diff
    // = 5 + 6 = 11
}

@check_nested () -> int = {
    let s1 = Circle(radius: 10);
    let s2 = Circle(radius: 10);
    let s3 = Rect(w: 5, h: 8);
    let payload_same = if s1 == s2 then 7 else 0;
    let payload_diff = if s1 != s3 then 8 else 0;
    payload_same + payload_diff
    // = 7 + 8 = 15
}

@main () -> int = {
    let a = check_struct_eq();   // = 7
    let b = check_sum_eq();      // = 11
    let c = check_nested();      // = 15
    a + b + c                    // = 33
}
```
**Source**: 1319 bytes, **Expected Result**: 33 (= 7 + 11 + 15)
**Actual**: Eval = 33 (correct), **AOT = 18 (WRONG — missing 15 from check_nested)**

**CRITICAL**: Payload sum type `==`/`!=` silently produces wrong results in AOT. No `Shape$eq` function generated — falls through to raw `icmp` which can't compare structs, silently returns `false`.

---

## Transformation Timeline

### Stages
```
Lexer:   1319 bytes → 344 tokens (10 comments, 0 errors)
Parser:  344 tokens → 4 functions, 3 types, 85 expressions, 0 errors
TypeCk:  4 functions registered, signatures collected, bodies checked — no mono instances
Canon:   4 functions, 85 source_exprs → 100 canon_nodes, 4 roots, 6 constants, 0 decision_trees
Eval:    105 eval_can calls
```
- 17.6% canon expansion (85→100) — moderate, from struct/variant construction + if/then/else desugaring
- 105 eval_can calls — derived method dispatch (eq comparisons) adds overhead

### Stage 6b: LLVM Path

#### Type Representations
```llvm
%ori.Point = type { i64, i64 }          ; 16 bytes — struct, passed by value
%ori.Color = type { i64 }               ; 8 bytes — unit-only sum type, just a tag
%ori.Shape = type { i64, [2 x i64] }    ; 24 bytes — payload sum type: tag + 2-slot payload
```

#### Generated LLVM IR (formatted, key sections)
```llvm
; --- check_struct_eq: calls Point$eq, branch on result ---
define fastcc i64 @_ori_check_struct_eq() #0 {       ; nounwind
bb0:
  ; Point constants passed by value (16 bytes, fits in registers)
  %eq_trait = call fastcc i1 @"_ori_Point$eq"(
    %ori.Point { i64 10, i64 20 }, %ori.Point { i64 10, i64 20 })
  br i1 %eq_trait, label %bb1, label %bb2
  ; ... phi: same = 3 or 0 ...
  %eq_trait1 = call fastcc i1 @"_ori_Point$eq"(
    %ori.Point { i64 10, i64 20 }, %ori.Point { i64 10, i64 30 })
  %neq = xor i1 %eq_trait1, true    ; != is !(==) — correct
  ; ... phi: diff = 4 or 0 ...
  ret i64 %add                       ; same + diff
}

; --- check_sum_eq: calls Color$eq ---
define fastcc i64 @_ori_check_sum_eq() #0 {
bb0:
  ; Variant construction for Color: alloca+store tag+load (CONFIRMED M7)
  %variant = alloca %ori.Color, align 8
  %variant.tag = getelementptr inbounds %ori.Color, ptr %variant, i32 0, i32 0
  store i64 0, ptr %variant.tag, align 4    ; ← CONFIRMED M5: align 4
  ; ... load and insertvalue to build Color value ...
  %eq_trait = call fastcc i1 @"_ori_Color$eq"(%ori.Color %variant.s0, %ori.Color %variant.s05)
  ; ... branch, phi, neq pattern (same as struct) ...
  ret i64 %add
}

; --- check_nested: *** NO Shape$eq call — hardcoded `false` ***
define fastcc i64 @_ori_check_nested() #0 {
bb0:
  ; Variant construction for Shape: alloca+store tag+payload+load
  ; ... Circle(radius: 10), Circle(radius: 10), Rect(w: 5, h: 8) ...

  br i1 false, label %bb1, label %bb2   ; ← CRITICAL: hardcoded false!
  ; ... phi: same = 7 or 0 → always 0 ...

  br i1 false, label %bb4, label %bb5   ; ← CRITICAL: hardcoded false!
  ; ... phi: diff = 8 or 0 → always 0 ...

  ret i64 %add                          ; 0 + 0 = 0
}

; --- main: all calls, no invoke (no ARC types) ---
define i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_check_struct_eq()
  br label %bb1                          ; ← CONFIRMED M3
bb1:
  %call1 = call fastcc i64 @_ori_check_sum_eq()
  br label %bb3                          ; ← CONFIRMED M3
bb3:
  %call2 = call fastcc i64 @_ori_check_nested()
  br label %bb5                          ; ← CONFIRMED M3
bb5:
  %add = add i64 %call, %call1
  %add3 = add i64 %add, %call2
  ret i64 %add3
}

; --- Point$eq: field-by-field comparison with early exit ---
define fastcc i1 @"_ori_Point$eq"(%ori.Point %0, %ori.Point %1) {
entry:
  %self.x = extractvalue %ori.Point %0, 0
  %other.x = extractvalue %ori.Point %1, 0
  %eq.x = icmp eq i64 %self.x, %other.x
  br i1 %eq.x, label %eq.check.1, label %eq.false
eq.true:
  ret i1 true
eq.false:
  ret i1 false
eq.check.1:
  %self.y = extractvalue %ori.Point %0, 1
  %other.y = extractvalue %ori.Point %1, 1
  %eq.y = icmp eq i64 %self.y, %other.y
  br i1 %eq.y, label %eq.true, label %eq.false
}

; --- Color$eq: tag-only comparison ---
define fastcc i1 @"_ori_Color$eq"(%ori.Color %0, %ori.Color %1) {
entry:
  %eq.tag.self = extractvalue %ori.Color %0, 0
  %eq.tag.other = extractvalue %ori.Color %1, 0
  %eq.tags = icmp eq i64 %eq.tag.self, %eq.tag.other
  ret i1 %eq.tags
}

; --- No Shape$eq function exists! ---
```

#### Key Observations
1. **Point$eq: exemplary codegen** — field-by-field `icmp eq i64` with early-exit chain. Constants inlined (`%ori.Point { i64 10, i64 20 }`). Pass-by-value since Point fits in 16 bytes.
2. **Color$eq: correct minimal codegen** — single `icmp eq i64` on tag values. Unit-only sum types reduce to pure tag comparison.
3. **MISSING Shape$eq** — No derived `$eq` function is generated for payload sum types. The codegen falls through to raw `icmp` on `%ori.Shape` (a struct), which triggers the ERROR log and substitutes `false`.
4. **Silent wrong answer** — `check_nested()` has `br i1 false, label %bb1, label %bb2` hardcoded TWICE. Both `==` and `!=` always return false. This is the worst kind of bug — no crash, no error visible at runtime, just wrong results.
5. **`!=` is `xor i1 %eq, true`** — Correct negation pattern. But since the underlying `eq` returns `false`, the `!=` result is `xor false, true = true`... wait, but in check_nested, there's no `xor` — the hardcoded `false` replaces both operations.
6. **0 runtime declarations** — No ARC types in this program. All types are value types (no strings, no lists). The IR is pure computation.
7. **7 functions defined** — check_struct_eq, check_sum_eq, check_nested, _ori_main, Point$eq, Color$eq, main wrapper.
8. **nounwind on everything** — Correct! No ARC, no allocations, no panic paths. Pure value manipulation.
9. **CONFIRMED M5**: `align 4` on variant tag stores (should be `align 8` for i64).
10. **CONFIRMED M7**: Verbose variant construction via alloca+store+load for Color and Shape variants.
11. **CONFIRMED M3**: Dead `br label` after every function call in main (3 instances).
12. **Point constants inlined** — `%ori.Point { i64 10, i64 20 }` as function arguments. No alloca needed.

---

## Issues Found

### CRITICAL
**C3 (NEW): Derived `Eq` for payload sum types — `$eq` function not generated, silent wrong results**
- `#[derive(Eq)]` on `type Shape = Circle(radius: int) | Rect(w: int, h: int)` does NOT generate a `Shape$eq` function
- Comparison falls through to raw LLVM `icmp` on struct type `%ori.Shape { i64, [2 x i64] }` which cannot compare aggregate types
- The codegen ERROR handler silently returns `false` instead of failing — substitutes `br i1 false` in the IR
- **Impact**: ALL payload sum type equality is silently wrong in AOT. `==` always returns false, `!=` always returns false (both comparisons fail identically).
- **Root cause**: `derive_codegen.rs` generates `$eq` for product types (structs) and unit-only sum types, but not for sum types with payload fields. The codegen needs to: (1) compare tags first, (2) if tags match, compare payload fields based on variant.
- **Eval**: Works correctly — the evaluator handles derived Eq for all type forms.

### MEDIUM
**CONFIRMED M3**: Dead branches after calls (3 instances in main)
**CONFIRMED M5**: `align 4` on i64 stores in variant construction (Color tags, Shape tags + payloads)
**CONFIRMED M7**: alloca+store+load roundtrip for variant construction (Color and Shape)

### What Works Exceptionally Well
- **Point$eq: textbook derived equality** — field-by-field with early exit, constants inlined, pass-by-value
- **Color$eq: minimal tag comparison** — 4 instructions total for unit-only sum type equality
- **`!=` as `xor i1 %eq, true`** — simple, correct negation (when eq function exists)
- **No ARC overhead** — pure value types produce zero runtime declarations
- **nounwind analysis correct** — all functions correctly marked nounwind (no panic paths)
- **Type representations clean** — Point (2×i64), Color (1×i64 tag), Shape (i64 tag + [2 x i64] payload)

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | **33** | **18 (WRONG)** |
| check_struct_eq (Point) | 7 ✓ | 7 ✓ |
| check_sum_eq (Color) | 11 ✓ | 11 ✓ |
| check_nested (Shape) | **15 ✓** | **0 ✗ (WRONG)** |
| Point$eq | Derived dispatch | Generated `$eq` function ✓ |
| Color$eq | Derived dispatch | Generated `$eq` function ✓ |
| Shape$eq | Derived dispatch | **NOT GENERATED** ✗ |
| Runtime declarations | 0 | 0 |
| Functions generated | N/A | 7 (missing Shape$eq) |
