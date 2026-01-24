# Appendix D: Debugging

Debug flags and techniques for the Sigil compiler.

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

Enable debugging via `SIGIL_DEBUG`:

```bash
# Enable all debug output
SIGIL_DEBUG=all sigil run file.si

# Enable specific flags
SIGIL_DEBUG=tokens,ast sigil run file.si

# Enable single flag
SIGIL_DEBUG=types sigil run file.si
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
SIGIL_DEBUG=tokens sigil run file.si
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
SIGIL_DEBUG=ast sigil run file.si
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
SIGIL_DEBUG=types sigil run file.si
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
SIGIL_DEBUG=eval sigil run file.si
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
SIGIL_DEBUG=salsa sigil run file.si
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
SIGIL_DEBUG=imports sigil run file.si
```

Output:
```
[IMPORTS] Resolving: './math'
  Base: /home/user/project/src/main.si
  Resolved: /home/user/project/src/math.si
  Cache: miss
  Loading...

[IMPORTS] Module './math' exports:
  add: (int, int) -> int
  subtract: (int, int) -> int
```

## Pattern Debugging

```bash
SIGIL_DEBUG=patterns sigil run file.si
```

Output:
```
[PATTERNS] Evaluating: map
  .over: [1, 2, 3]
  .transform: <function>

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

For VS Code debugging, launch.json:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Debug Compiler",
      "type": "lldb",
      "request": "launch",
      "program": "${workspaceFolder}/target/debug/sigilc",
      "args": ["run", "${file}"],
      "env": {
        "SIGIL_DEBUG": "all",
        "RUST_BACKTRACE": "1"
      }
    }
  ]
}
```

## Panic Debugging

Enable backtraces:

```bash
RUST_BACKTRACE=1 sigil run file.si
RUST_BACKTRACE=full sigil run file.si
```

## Memory Debugging

Using valgrind:

```bash
valgrind --leak-check=full target/debug/sigilc run file.si
```

## Performance Profiling

Using perf:

```bash
perf record target/release/sigilc run large_file.si
perf report
```

## Test Debugging

Debug specific test:

```bash
SIGIL_DEBUG=all cargo test test_type_inference -- --nocapture
```

## Common Debug Scenarios

### "Why is this type wrong?"

```bash
SIGIL_DEBUG=types sigil check file.si
```

Look for:
- Constraint generation
- Unification steps
- Final substitution

### "Why isn't this imported?"

```bash
SIGIL_DEBUG=imports sigil run file.si
```

Look for:
- Path resolution
- Export list
- Cache hits/misses

### "Why is Salsa recomputing?"

```bash
SIGIL_DEBUG=salsa sigil run file.si
```

Look for:
- Which queries run
- Early cutoff misses
- Dependency changes
