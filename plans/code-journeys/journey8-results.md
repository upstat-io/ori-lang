# Journey 8: "I am generic"

**Code**:
```ori
type Box<T> = { value: T }
@identity<T> (x: T) -> T = x;
@first<A, B> (a: A, b: B) -> A = a;
@get_value<T> (b: Box<T>) -> T = b.value;

@main () -> int = {
    let a = identity(x: 42);
    let b = first(a: 10, b: 20);
    let c = get_value(b: Box { value: 5 });
    a + b + c    // = 42 + 10 + 5 = 57
}
```
**Source**: 587 bytes, **Expected Result**: 57 (= 42 + 10 + 5)
**Actual**: Eval = 57 (correct), AOT = 57 (correct)

---

## Transformation Timeline

### Stages
```
Lexer:   587 bytes → 139 tokens (7 comments, 0 errors)
Parser:  139 tokens → 4 functions, 1 type, 22 expressions, 0 errors
TypeCk:  3 mono instances recorded: identity<int>, first<int,int>, get_value<int>
         + Applied→Struct resolution: Box<int> → concrete struct
Canon:   4 functions, 22 source_exprs → 24 canon_nodes, 4 roots, 6 constants
Eval:    24 eval_can calls — 1:1 with canon nodes
```

**Key type checker observation**: First time seeing `recorded mono instance` in the tracing — the type checker records that `identity` is called with `[int]`, `first` with `[int, int]`, and `get_value` with `[int]`. Also registers `Applied → Struct resolution` mapping `Box<int>` to a concrete struct.

### Stage 6b: LLVM Path

#### Generated LLVM IR (formatted)
```llvm
%ori.Box = type { i64 }

define i64 @_ori_main() {
bb0:
  %call = call fastcc i64 @"_ori_identity$24m$24int"(i64 42)
  br label %bb1
bb1:
  %call1 = call fastcc i64 @"_ori_first$24m$24int_int"(i64 10, i64 20)
  br label %bb3
bb3:
  %call2 = call fastcc i64 @"_ori_get_value$24m$24int"(%ori.Box { i64 5 })
  br label %bb5
bb5:
  %add = add i64 %call, %call1
  %add3 = add i64 %add, %call2
  ret i64 %add3
}

define fastcc i64 @"_ori_first$24m$24int_int"(i64 %0, i64 %1) #0 {
bb0:
  ret i64 %0      ; just return the first argument!
}

define fastcc i64 @"_ori_get_value$24m$24int"(%ori.Box %0) #0 {
bb0:
  %proj.0 = extractvalue %ori.Box %0, 0
  ret i64 %proj.0  ; extract single field
}

define fastcc i64 @"_ori_identity$24m$24int"(i64 %0) #0 {
bb0:
  ret i64 %0      ; just return the argument!
}
```

#### Key Observations
1. **Full monomorphization** — Generic functions compile to fully specialized code. `identity<int>` → single `ret i64`. Zero overhead.
2. **Name mangling**: `_ori_{name}$24m$24{types}` — `$24` encodes `$` (URL-style), function names quoted in LLVM IR because `$` needs escaping.
3. **`Box<int>` → `%ori.Box = type { i64 }`** — Generic struct correctly specialized. Single i64 field.
4. **Constant struct inlined**: `@"_ori_get_value$24m$24int"(%ori.Box { i64 5 })` — the `Box { value: 5 }` is a constant literal in the call.
5. **Box passed BY VALUE** — 8 bytes, under the 16-byte threshold. Correct.
6. **`first<int,int>` correctly discards `b`** — compiles to `ret i64 %0`, second argument unused.
7. **`_ori_main` missing `nounwind`** — All monomorphized callees have `#0 = { nounwind }`, but `_ori_main` itself lacks the attribute. In J1/J4 it had it. Inconsistency.
8. **Zero runtime declarations** — generic instantiation needs no runtime support.
9. **CONFIRMED M3**: Dead `br label` after each call in main.

---

## Issues Found

### CRITICAL
None.

### HIGH
None.

### MEDIUM
**M10 (NEW): `_ori_main` inconsistently missing `nounwind` attribute**
- In J1/J2/J4, `_ori_main` had `#0 = { nounwind }`
- In J8, it doesn't, despite all callees being `nounwind`
- Suggests the `nounwind` analysis doesn't propagate through monomorphized call sites

**CONFIRMED M3**: Dead branches after calls (3 instances)

### LOW
None new.

### What Works Exceptionally Well
- **Monomorphization**: Zero-overhead generics — specialized functions are minimal
- **Generic struct specialization**: `Box<T>` → `Box<int>` → `{ i64 }` correctly
- **Type inference**: All type arguments inferred from call sites (no explicit `identity<int>(...)` needed)
- **Constant propagation through generics**: `Box { value: 5 }` inlined as constant

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | 57 | 57 |
| Generics | Works (runtime polymorphism) | Works (monomorphization) |
| Generic struct | Works | Works (type specialized) |
| Type inference | Works | Works (types resolved at compile time) |
