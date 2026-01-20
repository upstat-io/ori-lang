# Testing & Tooling

Quick-reference guide to compiler testing strategies and language tooling.

---

## Test Organization Patterns

### Directory Structure
```
project/
├── src/
│   ├── lexer/
│   │   ├── mod.rs
│   │   └── tests.rs        # Unit tests alongside
│   └── parser/
│       ├── mod.rs
│       └── tests.rs
├── tests/
│   ├── lexer_tests.rs      # Integration tests
│   ├── parser_tests.rs
│   └── snapshots/          # Snapshot test expectations
└── test_programs/
    ├── valid/              # Should compile
    ├── invalid/            # Should error (with expected error)
    └── run/                # Should compile and run
```

### Test Categories
| Category | Purpose | Example |
|----------|---------|---------|
| Unit | Individual functions | `lexer.next_token()` |
| Integration | Component interaction | Parse → Type check |
| End-to-end | Full compilation | Source → Output |
| Snapshot | Golden file comparison | AST dump, error messages |
| Fuzz | Random input testing | Grammar-based generation |

---

## Unit Testing

### Lexer Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_tokens() {
        assert_tokens("+ - * /", vec![Plus, Minus, Star, Slash, Eof]);
    }

    #[test]
    fn test_numbers() {
        assert_tokens("123", vec![Int(123), Eof]);
        assert_tokens("3.14", vec![Float(3.14), Eof]);
        assert_tokens("0xFF", vec![Int(255), Eof]);
    }

    #[test]
    fn test_strings() {
        assert_tokens(r#""hello""#, vec![String("hello".into()), Eof]);
        assert_tokens(r#""hello\nworld""#, vec![String("hello\nworld".into()), Eof]);
    }

    #[test]
    fn test_keywords() {
        assert_tokens("if else fn let", vec![If, Else, Fn, Let, Eof]);
    }

    fn assert_tokens(input: &str, expected: Vec<TokenKind>) {
        let mut lexer = Lexer::new(input);
        let tokens: Vec<_> = lexer.collect();
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind.clone()).collect();
        assert_eq!(kinds, expected);
    }
}
```

### Parser Tests
```rust
#[test]
fn test_binary_expression() {
    let ast = parse_expr("1 + 2 * 3");
    assert_eq!(
        ast,
        Expr::Binary(
            BinOp::Add,
            Box::new(Expr::Int(1)),
            Box::new(Expr::Binary(
                BinOp::Mul,
                Box::new(Expr::Int(2)),
                Box::new(Expr::Int(3)),
            ))
        )
    );
}

#[test]
fn test_precedence() {
    // Verify operator precedence
    let ast = parse_expr("a + b * c");
    assert!(matches!(ast, Expr::Binary(BinOp::Add, _, _)));

    let ast = parse_expr("a * b + c");
    assert!(matches!(ast, Expr::Binary(BinOp::Add, _, _)));
}
```

### Type Checker Tests
```rust
#[test]
fn test_type_inference() {
    let ty = infer_type("let x = 42");
    assert_eq!(ty, Type::Int);
}

#[test]
fn test_type_mismatch() {
    let result = check("let x: int = \"hello\"");
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("expected `int`"));
}
```

---

## Snapshot Testing

### What is Snapshot Testing?
- Capture complex output to file (snapshot)
- Compare future runs against snapshot
- Review and approve changes interactively
- Great for: AST dumps, error messages, formatted output

### Using Insta (Rust)
```rust
use insta::assert_snapshot;

#[test]
fn test_parse_function() {
    let ast = parse("fn add(a: int, b: int) -> int = a + b");
    assert_snapshot!(format!("{:#?}", ast));
}

#[test]
fn test_error_message() {
    let err = compile_and_get_error("let x: int = \"hello\"");
    assert_snapshot!(err.to_string());
}
```

### Snapshot File
```
// snapshots/test_parse_function.snap
---
source: src/parser/tests.rs
expression: "format!(\"{:#?}\", ast)"
---
FnDef {
    name: "add",
    params: [
        Param { name: "a", ty: Int },
        Param { name: "b", ty: Int },
    ],
    ret_type: Int,
    body: Binary(Add, Ident("a"), Ident("b")),
}
```

### Reviewing Changes
```bash
# Rust/Insta
cargo insta review

# Shows diff, asks approve/reject
# Updates .snap files when approved
```

---

## Integration Testing

### Test File Convention
```
tests/
├── compile/
│   ├── arithmetic.lang     # Test source
│   └── arithmetic.expected # Expected output/behavior
├── error/
│   ├── type_mismatch.lang
│   └── type_mismatch.stderr  # Expected error
└── run/
    ├── hello.lang
    └── hello.stdout          # Expected runtime output
```

### Test Runner
```rust
#[test]
fn run_compile_tests() {
    for entry in fs::read_dir("tests/compile").unwrap() {
        let path = entry.unwrap().path();
        if path.extension() == Some("lang") {
            let source = fs::read_to_string(&path).unwrap();
            let expected = fs::read_to_string(path.with_extension("expected")).unwrap();

            let result = compile(&source);
            assert!(result.is_ok(), "Failed to compile {:?}: {:?}", path, result);
        }
    }
}

#[test]
fn run_error_tests() {
    for entry in fs::read_dir("tests/error").unwrap() {
        let path = entry.unwrap().path();
        if path.extension() == Some("lang") {
            let source = fs::read_to_string(&path).unwrap();
            let expected_err = fs::read_to_string(path.with_extension("stderr")).unwrap();

            let result = compile(&source);
            assert!(result.is_err());
            assert_eq!(result.unwrap_err().to_string().trim(), expected_err.trim());
        }
    }
}
```

### Inline Expected Errors
```rust
// In test file: arithmetic.lang
fn main() {
    let x: int = "hello";
    //           ^^^^^^^ error: expected `int`, found `str`
}

// Parser extracts expected error from comment
fn extract_expected_errors(source: &str) -> Vec<ExpectedError> {
    let mut errors = vec![];
    for (line_num, line) in source.lines().enumerate() {
        if let Some(pos) = line.find("// ") {
            let comment = &line[pos + 3..];
            if let Some(caret_start) = comment.find("^") {
                // Extract error pattern
            }
        }
    }
    errors
}
```

---

## Fuzz Testing

### Grammar-Based Fuzzing
```rust
// Generate valid programs from grammar
fn fuzz_expr() -> Expr {
    match rand::gen_range(0, 5) {
        0 => Expr::Int(rand::gen()),
        1 => Expr::Bool(rand::gen()),
        2 => Expr::Binary(
            rand_binop(),
            Box::new(fuzz_expr()),
            Box::new(fuzz_expr()),
        ),
        3 => Expr::If(
            Box::new(fuzz_expr()),
            Box::new(fuzz_expr()),
            Box::new(fuzz_expr()),
        ),
        _ => Expr::Ident(rand_ident()),
    }
}
```

### LibFuzzer Integration (Rust)
```rust
// In fuzz/fuzz_targets/parse.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Should not panic on any input
        let _ = my_language::parse(s);
    }
});
```

```bash
# Run fuzzer
cargo +nightly fuzz run parse
```

### Property-Based Testing
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn parse_roundtrip(expr in any::<Expr>()) {
        let source = expr.to_source();
        let parsed = parse(&source).unwrap();
        assert_eq!(expr, parsed);
    }

    #[test]
    fn type_check_idempotent(program in valid_program()) {
        let ty1 = infer_type(&program);
        let ty2 = infer_type(&program);
        assert_eq!(ty1, ty2);
    }
}
```

---

## Mandatory Test Coverage

### Sigil-Style Coverage Checking
```rust
// Every function must have at least one test
fn check_test_coverage(module: &Module) -> Vec<UncoveredFunction> {
    let functions: HashSet<_> = module.functions
        .iter()
        .filter(|f| f.name != "main")
        .map(|f| &f.name)
        .collect();

    let tested: HashSet<_> = module.tests
        .iter()
        .flat_map(|t| &t.targets)
        .collect();

    functions.difference(&tested)
        .map(|name| UncoveredFunction { name: name.clone() })
        .collect()
}

// Fail compilation if coverage missing
fn compile(source: &str) -> Result<Module, Error> {
    let module = parse_and_check(source)?;

    let uncovered = check_test_coverage(&module);
    if !uncovered.is_empty() {
        return Err(Error::MissingTests(uncovered));
    }

    Ok(module)
}
```

---

## Parallel Test Execution

### Rust Test Framework
```bash
# Run tests in parallel (default)
cargo test

# Run specific test
cargo test test_name

# Run tests matching pattern
cargo test parser

# Run single-threaded (for debugging)
cargo test -- --test-threads=1
```

### Custom Test Runner
```rust
fn run_tests_parallel(tests: &[TestCase]) -> Vec<TestResult> {
    tests
        .par_iter()  // Rayon parallel iterator
        .map(|test| run_test(test))
        .collect()
}
```

---

## REPL Implementation

### Basic REPL
```rust
fn repl() {
    let mut env = Environment::new();

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }

        let line = line.trim();
        if line.is_empty() { continue; }
        if line == ":quit" || line == ":q" { break; }

        // Special commands
        if line.starts_with(":") {
            handle_command(line, &env);
            continue;
        }

        // Try as expression first, then statement
        match eval_repl_input(line, &mut env) {
            Ok(Some(value)) => println!("{}", value),
            Ok(None) => {}
            Err(e) => eprintln!("{}", e),
        }
    }
}
```

### REPL Commands
```
:help       - Show help
:quit, :q   - Exit REPL
:type <expr> - Show type of expression
:ast <expr>  - Show AST
:env        - Show environment
:clear      - Clear environment
:load <file> - Load and evaluate file
```

### Multi-line Input
```rust
fn read_complete_input() -> String {
    let mut input = String::new();
    let mut brace_count = 0;

    loop {
        let prompt = if input.is_empty() { "> " } else { "... " };
        print!("{}", prompt);
        io::stdout().flush().unwrap();

        let mut line = String::new();
        io::stdin().read_line(&mut line).unwrap();

        brace_count += line.matches('{').count() as i32;
        brace_count -= line.matches('}').count() as i32;

        input.push_str(&line);

        if brace_count <= 0 && (line.trim().is_empty() || !line.trim().ends_with(',')) {
            break;
        }
    }

    input
}
```

---

## Code Formatter

### Formatter Structure
```rust
struct Formatter {
    output: String,
    indent: usize,
    indent_str: &'static str,
}

impl Formatter {
    fn format_module(&mut self, module: &Module) -> String {
        for item in &module.items {
            self.format_item(item);
            self.newline();
        }
        self.output.clone()
    }

    fn format_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Binary(op, left, right) => {
                self.format_expr(left);
                self.write(&format!(" {} ", op));
                self.format_expr(right);
            }
            Expr::Block(stmts) => {
                self.write("{");
                self.indent += 1;
                for stmt in stmts {
                    self.newline();
                    self.format_stmt(stmt);
                }
                self.indent -= 1;
                self.newline();
                self.write("}");
            }
            // ...
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn newline(&mut self) {
        self.output.push('\n');
        for _ in 0..self.indent {
            self.output.push_str(self.indent_str);
        }
    }
}
```

### Idempotent Formatting
```rust
#[test]
fn format_is_idempotent() {
    let source = "fn foo(x:int)->int{x+1}";
    let formatted1 = format(source);
    let formatted2 = format(&formatted1);
    assert_eq!(formatted1, formatted2);
}
```

---

## Language Server Protocol (LSP)

### LSP Overview
- JSON-RPC protocol between editor and language server
- Solves M×N problem (M editors × N languages → M+N)
- Features: completion, hover, go-to-definition, diagnostics, etc.

### Key LSP Messages

#### Initialize
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "capabilities": {...}
  }
}
```

#### Text Document Diagnostics
```json
{
  "jsonrpc": "2.0",
  "method": "textDocument/publishDiagnostics",
  "params": {
    "uri": "file:///path/to/file.lang",
    "diagnostics": [{
      "range": {"start": {"line": 3, "character": 12}, "end": {"line": 3, "character": 19}},
      "severity": 1,
      "message": "type mismatch"
    }]
  }
}
```

#### Completion
```json
// Request
{
  "method": "textDocument/completion",
  "params": {
    "textDocument": {"uri": "file:///..."},
    "position": {"line": 5, "character": 10}
  }
}

// Response
{
  "items": [
    {"label": "println", "kind": 3, "detail": "fn(str) -> void"},
    {"label": "print", "kind": 3, "detail": "fn(str) -> void"}
  ]
}
```

### Basic LSP Server
```rust
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::*;

fn main() {
    let (connection, io_threads) = Connection::stdio();

    // Initialize
    let caps = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncKind::Full),
        completion_provider: Some(CompletionOptions::default()),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        ..Default::default()
    };

    connection.initialize(serde_json::to_value(caps).unwrap());

    // Main loop
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => handle_request(&connection, req),
            Message::Notification(not) => handle_notification(&connection, not),
            _ => {}
        }
    }
}

fn handle_request(conn: &Connection, req: Request) {
    match req.method.as_str() {
        "textDocument/hover" => {
            let params: HoverParams = serde_json::from_value(req.params).unwrap();
            let hover = compute_hover(&params);
            conn.sender.send(Message::Response(Response {
                id: req.id,
                result: Some(serde_json::to_value(hover).unwrap()),
                error: None,
            })).unwrap();
        }
        // ... other methods
    }
}
```

---

## Debug Info Generation

### DWARF Basics
```rust
// Location information for debugger
struct DebugLocation {
    file: String,
    line: u32,
    column: u32,
}

// Variable debug info
struct DebugVariable {
    name: String,
    ty: DebugType,
    location: VariableLocation,  // Register, stack offset, etc.
}

// Generate with LLVM
fn emit_debug_info(builder: &Builder, loc: &DebugLocation) {
    let di_loc = builder.create_debug_location(
        loc.line,
        loc.column,
        current_scope,
        None,
    );
    builder.set_current_debug_location(di_loc);
}
```

### Source Maps (for transpilers)
```rust
struct SourceMap {
    version: u32,
    file: String,
    sources: Vec<String>,
    mappings: String,  // VLQ-encoded
}

// Encode mapping segment
fn encode_vlq(value: i32) -> String {
    // Variable-length quantity encoding
    // ...
}
```

---

## Benchmarking

### Compiler Benchmarks
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_lexer(c: &mut Criterion) {
    let source = include_str!("../benches/large_file.lang");

    c.bench_function("lexer", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(source));
            while lexer.next_token().kind != Eof {}
        })
    });
}

fn bench_parser(c: &mut Criterion) {
    let source = include_str!("../benches/large_file.lang");

    c.bench_function("parser", |b| {
        b.iter(|| parse(black_box(source)))
    });
}

criterion_group!(benches, bench_lexer, bench_parser);
criterion_main!(benches);
```

### Memory Profiling
```bash
# Valgrind for memory usage
valgrind --tool=massif ./compiler input.lang

# Heaptrack (Linux)
heaptrack ./compiler input.lang
heaptrack_gui heaptrack.*.gz
```

---

## Tooling Checklist

### Testing
- [ ] Unit tests for lexer, parser, type checker
- [ ] Integration tests with real programs
- [ ] Snapshot tests for AST and errors
- [ ] Fuzz testing for crash resistance
- [ ] Property-based tests for invariants
- [ ] Parallel test execution
- [ ] Coverage measurement

### Developer Tools
- [ ] REPL with command support
- [ ] Code formatter
- [ ] Language server (LSP)
- [ ] Syntax highlighting grammar
- [ ] Debug info generation

### CI/CD
- [ ] Automated testing on push
- [ ] Benchmark tracking
- [ ] Documentation generation
- [ ] Release automation

---

## Key References
- Fuzzing Book: https://www.fuzzingbook.org/
- LibFuzzer: https://llvm.org/docs/LibFuzzer.html
- Insta (Snapshot Testing): https://insta.rs/
- LSP Specification: https://microsoft.github.io/language-server-protocol/
- Criterion (Benchmarking): https://bheisler.github.io/criterion.rs/book/
