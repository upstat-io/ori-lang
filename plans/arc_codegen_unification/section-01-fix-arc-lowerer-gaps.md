# Section 01 — Fix ARC Lowerer Gaps

## File: `compiler/ori_arc/src/lower/expr/mod.rs`

Six `CanExpr` variants currently produce `UnsupportedExpr` or emit unit instead of proper ARC IR.

### 1. FunctionExp (CRITICAL — blocks all print/panic/todo/recurse)

Route through `FunctionExpKind` match. For each kind:
- **Print**: Get "msg" prop, lower expr, emit `Apply` to `ori_print_*` based on type, return unit
- **Panic**: Get "message"/"value" prop, lower expr, emit `Apply` to `ori_panic`/`ori_panic_cstr`, emit `Unreachable`
- **Todo**: Emit `Apply` to `ori_panic_cstr` with "not yet implemented", emit `Unreachable`
- **Unreachable**: Emit `Apply` to `ori_panic_cstr` with "reached unreachable code", emit `Unreachable`
- **Recurse**: Lower all prop args, emit `Apply` to current function name
- **Cache**: Lower "value"/"expr" prop, return result
- **Catch**: Lower "expr"/"value" prop, return result
- **Deferred** (Parallel, Spawn, Timeout, With, Channel*): Keep as `ArcProblem::UnsupportedExpr`

### 2. FunctionRef — emit `PartialApply` with empty captures

`PartialApply { func: name, args: [] }` creates a zero-capture closure.

### 3. HashLength — add field to ArcLowerer

Add `hash_length: Option<ArcVarId>` field. Set during `lower_index()`. Return stored value.

### 4. FormatWith — type-dispatched Apply to `ori_format_*`

Lower inner expr, emit `Apply` to `ori_format_int`/`ori_format_float`/etc. based on inner type.
String spec embedded as `LitValue::String`.

### 5. Await — trivial (evaluate inner)

Replace `UnsupportedExpr` with `self.lower_expr(inner)`.

### 6. WithCapability — trivial (evaluate body)

Replace `UnsupportedExpr` with `self.lower_expr(body)`.
