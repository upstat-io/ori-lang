# Appendix D: Debugging

Debug flags and techniques for the Ori compiler.

## Debug Flags

The `debug.rs` module provides debug flags:

```rust
pub struct DebugFlags(u32);

impl DebugFlags {
    pub const TOKENS: Self = Self(0b0000_0001);
    pub const AST: Self = Self(0b0000_0010);
    pub const TYPES: Self = Self(0b0000_0100);
    pub const EVAL: Self = Self(0b0000_1000);
    pub const IMPORTS: Self = Self(0b0001_0000);
    pub const PATTERNS: Self = Self(0b0010_0000);
    pub const SALSA: Self = Self(0b0100_0000);
    pub const ALL: Self = Self(0b0111_1111);
}
```

## Environment Variable

Enable debugging via `ORI_DEBUG`:

```bash
# Enable all debug output
ORI_DEBUG=all ori run file.ori

# Enable specific flags
ORI_DEBUG=tokens,ast ori run file.ori

# Enable single flag
ORI_DEBUG=types ori run file.ori
```

## Debug Macros

```rust
// In compiler code
debug_tokens!("Tokenized: {:?}", tokens);
debug_ast!("Parsed: {:?}", module);
debug_types!("Inferred type: {:?}", ty);
debug_eval!("Evaluating: {:?}", expr);
```

## Token Debugging

```bash
ORI_DEBUG=tokens ori run file.ori
```

Output:
```
[TOKENS] let x = 42
  Token { kind: Let, span: 0..3 }
  Token { kind: Ident("x"), span: 4..5 }
  Token { kind: Eq, span: 6..7 }
  Token { kind: Int(42), span: 8..10 }
```

## AST Debugging

```bash
ORI_DEBUG=ast ori run file.ori
```

Output:
```
[AST] Module {
  functions: [
    Function {
      name: "main",
      params: [],
      body: ExprId(0),
    }
  ]
}

[AST] ExprArena:
  [0] Let { name: "x", value: ExprId(1), body: ExprId(2) }
  [1] Literal(Int(42))
  [2] Ident("x")
```

## Type Debugging

```bash
ORI_DEBUG=types ori run file.ori
```

Output:
```
[TYPES] Inferring: let x = 42
  x : T0 (fresh)
  42 : Int
  Unify(T0, Int) -> Ok
  x : Int

[TYPES] Expression types:
  ExprId(0): Int
  ExprId(1): Int
  ExprId(2): Int
```

## Evaluation Debugging

```bash
ORI_DEBUG=eval ori run file.ori
```

Output:
```
[EVAL] Evaluating ExprId(0): Let
  Evaluating value ExprId(1): Literal
    -> Value::Int(42)
  Binding x = Int(42)
  Evaluating body ExprId(2): Ident
    Lookup x -> Int(42)
    -> Value::Int(42)
  -> Value::Int(42)
```

## Salsa Debugging

```bash
ORI_DEBUG=salsa ori run file.ori
```

Output:
```
[SALSA] will_execute: tokens(SourceFile(0))
[SALSA] did_execute: tokens(SourceFile(0)) in 1.2ms
[SALSA] will_execute: parsed(SourceFile(0))
[SALSA] did_execute: parsed(SourceFile(0)) in 3.4ms
[SALSA] will_execute: typed(SourceFile(0))
[SALSA] did_execute: typed(SourceFile(0)) in 2.1ms
```

## Import Debugging

```bash
ORI_DEBUG=imports ori run file.ori
```

Output:
```
[IMPORTS] Resolving: './math'
  Base: /home/user/project/src/main.ori
  Resolved: /home/user/project/src/math.ori
  Cache: miss
  Loading...

[IMPORTS] Module './math' exports:
  add: (int, int) -> int
  subtract: (int, int) -> int
```

## Pattern Debugging

```bash
ORI_DEBUG=patterns ori run file.ori
```

Output:
```
[PATTERNS] Evaluating: map
  over: [1, 2, 3]
  transform: <function>

[PATTERNS] map iteration:
  [0] transform(1) -> 2
  [1] transform(2) -> 4
  [2] transform(3) -> 6

[PATTERNS] Result: [2, 4, 6]
```

## Programmatic Debugging

```rust
// Enable in code
debug::set_flags(DebugFlags::TYPES | DebugFlags::EVAL);

// Check if enabled
if debug::is_enabled(DebugFlags::TOKENS) {
    eprintln!("Tokens: {:?}", tokens);
}
```

## IDE Integration

For VS Code debugging, launchjson:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Debug Compiler",
      "type": "lldb",
      "request": "launch",
      "program": "${workspaceFolder}/target/debug/oric",
      "args": ["run", "${file}"],
      "env": {
        "ORI_DEBUG": "all",
        "RUST_BACKTRACE": "1"
      }
    }
  ]
}
```

## Panic Debugging

Enable backtraces:

```bash
RUST_BACKTRACE=1 ori run file.ori
RUST_BACKTRACE=full ori run file.ori
```

## Memory Debugging

Using valgrind:

```bash
valgrind --leak-check=full target/debug/oric run file.ori
```

## Performance Profiling

Using perf:

```bash
perf record target/release/oric run large_file.ori
perf report
```

## Test Debugging

Debug specific test:

```bash
ORI_DEBUG=all cargo test test_type_inference -- --nocapture
```

## Common Debug Scenarios

### "Why is this type wrong?"

```bash
ORI_DEBUG=types ori check file.ori
```

Look for:
- Constraint generation
- Unification steps
- Final substitution

### "Why isn't this imported?"

```bash
ORI_DEBUG=imports ori run file.ori
```

Look for:
- Path resolution
- Export list
- Cache hits/misses

### "Why is Salsa recomputing?"

```bash
ORI_DEBUG=salsa ori run file.ori
```

Look for:
- Which queries run
- Early cutoff misses
- Dependency changes
