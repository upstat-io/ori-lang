# Journey 6: "I am a match"

**Code**:
```ori
type Status = Pending | Running | Completed;
type Result2 = Success(value: int) | Failure(code: int);

@to_code (s: Status) -> int = match s { Pending -> 0, Running -> 1, Completed -> 2 }
@extract (r: Result2) -> int = match r { Success(v) -> v, Failure(c) -> c }

@main () -> int = {
    let a = extract(r: Success(42));     // = 42
    let b = extract(r: Failure(-1));     // = -1
    let c = to_code(s: Pending);         // = 0
    a + b + c                            // = 41
}
```
**Source**: 690 bytes, **Expected Result**: 41 (= 42 + (-1) + 0)
**Actual**: Eval = 41 (correct), AOT = 41 (correct)

---

## Transformation Timeline

### Stage 1-2: Lexer
```
User:    690 bytes → 154 tokens (7 comments, 0 errors)
Prelude: 10,331 bytes → 1,516 tokens (unchanged)
```

### Stage 3: Parser
```
User:    154 tokens → 3 functions, 2 types, 28 expressions, 0 errors
```

### Stage 5: Canonicalizer
```
User:    3 functions, 28 source_exprs → 31 canon_nodes, 3 roots, 6 constants, 2 decision_trees
```
- First time `decision_trees > 0` for user code — match expressions generate decision trees
- 10.7% expansion (28→31)

### Stage 6a: Eval Path
```
Total eval_can calls:  31
```
- 1:1 with canon nodes — very efficient

### Stage 6b: LLVM Path

#### Type Layouts
```llvm
%ori.Status = type { i64 }              ; tag only (unit variants): 8 bytes
%ori.Result2 = type { i64, [1 x i64] }  ; tag + payload array: 16 bytes
```

#### Generated LLVM IR (key functions)
```llvm
; Unit variant match → select chains (branch-free!)
define fastcc i64 @_ori_to_code(%ori.Status %0) #0 {
bb0:
  %proj.0 = extractvalue %ori.Status %0, 0    ; extract tag
  %eq = icmp eq i64 %proj.0, 0                ; Pending?
  %sel = select i1 %eq, i64 0, i64 2          ; yes→0, else→2 (Completed default)
  %eq1 = icmp eq i64 %proj.0, 1               ; Running?
  %sel2 = select i1 %eq1, i64 1, i64 %sel     ; yes→1, else→previous
  br label %bb1
bb1:
  %v2 = phi i64 [ %sel2, %bb0 ]               ; single-predecessor phi (redundant)
  ret i64 %v2
}

; Payload variant match → switch + extract
define fastcc i64 @_ori_extract(%ori.Result2 %0) #0 {
bb0:
  %proj.0 = extractvalue %ori.Result2 %0, 0   ; extract tag
  switch i64 %proj.0, label %bb4 [
    i64 0, label %bb2                          ; Success
    i64 1, label %bb3                          ; Failure
  ]
bb1:
  %v2 = phi i64 [ %proj.1, %bb2 ], [ %proj.14, %bb3 ]
  ret i64 %v2
bb2:                                           ; Success arm
  %proj.alloca = alloca %ori.Result2, align 8  ; store aggregate back to memory
  store %ori.Result2 %0, ptr %proj.alloca, align 4
  %proj.payload = getelementptr inbounds %ori.Result2, ptr %proj.alloca, i32 0, i32 1
  %proj.1.gep = getelementptr inbounds i64, ptr %proj.payload, i64 0
  %proj.1 = load i64, ptr %proj.1.gep, align 4
  br label %bb1
bb3:                                           ; Failure arm (IDENTICAL to bb2!)
  %proj.alloca1 = alloca %ori.Result2, align 8
  store %ori.Result2 %0, ptr %proj.alloca1, align 4
  %proj.payload2 = getelementptr inbounds %ori.Result2, ptr %proj.alloca1, i32 0, i32 1
  %proj.1.gep3 = getelementptr inbounds i64, ptr %proj.payload2, i64 0
  %proj.14 = load i64, ptr %proj.1.gep3, align 4
  br label %bb1
bb4:
  unreachable                                  ; exhaustive match → unreachable default
}
```

#### Key Observations
1. **`select` for unit variant matching** — `to_code` compiles to branch-free `select` chains. Excellent optimization — no branch misprediction.
2. **`switch` for payload variant dispatch** — `extract` uses `switch` on the tag for O(1) dispatch. Correct.
3. **`unreachable` for exhaustive default** — the `bb4: unreachable` block handles the impossible default case. Correct.
4. **Tagged union representation**: `{ i64, [1 x i64] }` — tag + fixed-size payload array. All variants share the same payload size (max).
5. **Redundant alloca-store-load for payload extraction** — Both `bb2` (Success) and `bb3` (Failure) store the aggregate to an alloca, GEP to the payload, and load. This is because `extractvalue` can't extract from an array at a dynamic index — but here the index IS known (field 0 of the payload). Could use `extractvalue` on the array.
6. **bb2 and bb3 are IDENTICAL** — Both branches extract the same field from the same offset. The codegen doesn't realize that Success and Failure have payloads at identical offsets. Could be merged.
7. **Single-predecessor phi** — `to_code`'s `bb1` has a phi with only one predecessor (`bb0`). This is a no-op phi that could be eliminated.
8. **Verbose variant construction in main** — Creating `Success(42)`: 12 instructions (alloca, store tag, GEP, store value, load tag, insertvalue, load payload, insertvalue). Could be 2 `insertvalue` instructions.
9. **Sum types passed BY VALUE** — Status (8 bytes) and Result2 (16 bytes) are passed directly, not by reference. Correct — they're at or below the 16-byte threshold.
10. **CONFIRMED M5**: `align 4` on all loads
11. **CONFIRMED M3**: Dead branches after calls in `_ori_main`
12. **Zero runtime declarations** — sum type operations are pure

---

## Issues Found

### CRITICAL
None.

### HIGH
None.

### MEDIUM

**M7 (NEW): Verbose variant construction — alloca+store+load roundtrip**
- Creating `Success(42)` takes 12 instructions instead of 2 `insertvalue` instructions
- The codegen writes to memory then reads it back to produce an SSA aggregate
- Same pattern as M6 (full struct load) but worse — it writes then immediately reads

**M8 (NEW): Identical match arms not deduplicated**
- `extract`'s Success (bb2) and Failure (bb3) arms compile to identical code
- Both store the aggregate, GEP to payload[0], load it — same offset for both variants
- Could be a single block with the switch falling through

**CONFIRMED M5**: `align 4` on all i64/array loads
**CONFIRMED M3**: Dead branches in main

### LOW

**L4 (NEW): Single-predecessor phi nodes in match codegen**
- `to_code`'s bb1 has `phi i64 [ %sel2, %bb0 ]` — only one incoming edge
- This is always equal to `%sel2` — no phi needed
- LLVM's optimizer removes this, but it's codegen noise

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | 41 | 41 |
| Unit variant match | Works | Works (select chains — excellent) |
| Payload variant match | Works | Works (switch dispatch) |
| Exhaustive checking | N/A | unreachable default — correct |
