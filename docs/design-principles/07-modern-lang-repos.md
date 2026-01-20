# Modern Language Repository Analysis

Deep-dive analysis of Rust, Go, and Zig compiler architectures and patterns.

---

## Repository Overview

| Language | Repo | Key Paths | Focus |
|----------|------|-----------|-------|
| Rust | `rust-lang/rust` | `compiler/rustc_*` | Multi-crate modular design |
| Go | `golang/go` | `src/cmd/compile/internal/` | Single-pass simplicity |
| Zig | `ziglang/zig` | `src/*.zig` | Data-oriented design |

---

## Rust Compiler (rustc)

### Directory Structure
```
rust/
├── compiler/
│   ├── rustc_lexer/      # Standalone lexer library
│   ├── rustc_parse/      # Parser and AST construction
│   ├── rustc_ast/        # AST node definitions
│   ├── rustc_hir/        # High-level IR
│   ├── rustc_middle/     # MIR and type system
│   ├── rustc_mir_*/      # MIR transformations
│   ├── rustc_codegen_*/  # LLVM and other backends
│   ├── rustc_error_*/    # Error handling infrastructure
│   └── rustc_session/    # Compiler session state
├── library/              # Standard library
└── tests/                # Compiler test suites
```

### Key Patterns

#### Crate Organization (~50 crates)
- Each major component is a separate crate
- Clear dependency hierarchy
- `rustc_` prefix for compiler crates
- Enables incremental compilation improvements

#### Two-Stage Lexer
```
rustc_lexer (pure library)
    ↓
rustc_parse::lexer (integrated tokenizer)
```

**rustc_lexer** (`compiler/rustc_lexer/src/lib.rs`):
- Pure, standalone library
- No dependencies on compiler infrastructure
- Returns `(TokenKind, length)` pairs
- Usable by IDEs, tools, etc.

**rustc_parse::lexer**:
- Wraps rustc_lexer
- Adds spans, string interning
- Reports diagnostics
- Edition-aware keyword handling

#### AST Design (`compiler/rustc_ast/src/ast.rs`)
```rust
// Pattern: Item + ItemKind separation
pub struct Item<K = ItemKind> {
    pub id: NodeId,
    pub span: Span,
    pub kind: K,
    pub attrs: AttrVec,
    // ...
}

pub enum ItemKind {
    Fn(Box<Fn>),
    Struct(VariantData, Generics),
    Enum(EnumDef, Generics),
    // ...
}

// Pattern: Expr + ExprKind
pub struct Expr {
    pub id: NodeId,
    pub kind: ExprKind,
    pub span: Span,
    pub attrs: AttrVec,
    // ...
}

pub enum ExprKind {
    Lit(token::Lit),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Unary(UnOp, Box<Expr>),
    If(Box<Expr>, Box<Block>, Option<Box<Expr>>),
    // ...
}
```

#### Visitor Pattern
```rust
// Two visitor traits: immutable and mutable
pub trait Visitor<'ast> {
    fn visit_expr(&mut self, e: &'ast Expr) { walk_expr(self, e) }
    fn visit_stmt(&mut self, s: &'ast Stmt) { walk_stmt(self, s) }
    // ...
}

pub trait MutVisitor {
    fn visit_expr(&mut self, e: &mut Expr) { walk_expr(self, e) }
    // ...
}
```

#### Compilation Pipeline
```
Source → Lexer → Parser → AST
    → AST Validation
    → Name Resolution
    → HIR Lowering → HIR
    → Type Checking
    → MIR Lowering → MIR
    → Borrow Checking
    → Optimization
    → Codegen → LLVM IR
    → LLVM → Object Files
```

#### Query System
- Incremental compilation support
- Memoized computations
- Dependency tracking
- Demand-driven analysis

```rust
// Query definition pattern
queries! {
    query type_of(def_id: DefId) -> Ty<'tcx> {
        // Implementation
    }
}
```

### Key Files to Study
- `compiler/rustc_lexer/src/lib.rs` - Pure lexer
- `compiler/rustc_lexer/src/cursor.rs` - Cursor abstraction
- `compiler/rustc_parse/src/parser/expr.rs` - Expression parsing
- `compiler/rustc_ast/src/ast.rs` - AST definitions
- `compiler/rustc_ast/src/visit.rs` - Visitor pattern

---

## Go Compiler

### Directory Structure
```
go/src/cmd/compile/
├── internal/
│   ├── syntax/       # Lexer, parser, AST
│   │   ├── scanner.go
│   │   ├── parser.go
│   │   ├── nodes.go
│   │   ├── pos.go
│   │   └── walk.go
│   ├── types2/       # Type checker
│   ├── ir/           # Intermediate representation
│   ├── ssa/          # SSA form
│   └── gc/           # Code generation
└── README
```

### Key Patterns

#### Self-Contained Syntax Package
```go
// The scanner, parser, nodes, and tokens are all in one package
// Can be compiled standalone:
// go tool compile scanner.go source.go tokens.go nodes.go
```

#### Simple Node Structure (`syntax/nodes.go`)
```go
// Base interface
type Node interface {
    Pos() Pos
    SetPos(Pos)
    aNode()
}

// Embedded position
type node struct {
    pos Pos
}

// Declarations
type Decl interface {
    Node
    aDecl()
}

type FuncDecl struct {
    Pragma     Pragma
    Recv       *Field      // nil = regular function
    Name       *Name
    TParamList []*Field    // nil = no type params
    Type       *FuncType
    Body       *BlockStmt  // nil = forward decl
    decl
}

// Expressions
type Expr interface {
    Node
    typeInfo
    aExpr()
}

type BinaryExpr struct {
    X    Expr
    Op   Operator
    Y    Expr
    expr
}
```

#### Scanner with Auto-Semicolon
```go
type scanner struct {
    source
    mode   uint
    nlsemi bool // if set '\n' and EOF translate to ';'

    // Current token state
    line, col uint
    tok       token
    lit       string
    kind      LitKind
    op        Operator
}

// Semicolon insertion after these tokens
func (s *scanner) next() {
    nlsemi := s.nlsemi
    s.nlsemi = false
    // ...
    if nlsemi && (s.ch == '\n' || s.ch == -1) {
        s.tok = _Semi
    }
}
```

#### Parser Methods
```go
func (p *parser) got(tok token) bool {
    if p.tok == tok {
        p.next()
        return true
    }
    return false
}

func (p *parser) want(tok token) {
    if !p.got(tok) {
        p.syntaxError("expected " + tokstring(tok))
        p.advance()
    }
}
```

#### Error Recovery
```go
// stopset: statement-starting keywords for synchronization
const stopset uint64 = 1<<_Break | 1<<_Const | 1<<_Continue |
    1<<_For | 1<<_If | 1<<_Return | 1<<_Switch | 1<<_Type | 1<<_Var

func (p *parser) advance(followlist ...token) {
    var followset uint64 = 1 << _EOF
    if len(followlist) > 0 {
        if p.fnest > 0 {
            followset |= stopset
        }
        for _, tok := range followlist {
            followset |= 1 << tok
        }
    }

    for !contains(followset, p.tok) {
        p.next()
        if len(followlist) == 0 {
            break
        }
    }
}
```

#### AST Walker
```go
// Walk traverses an AST in depth-first order
func Walk(n Node, f func(Node) bool) {
    if n != nil {
        if f(n) {
            walkList(n, f)
        }
    }
}

func walkList(node Node, f func(Node) bool) {
    switch n := node.(type) {
    case *File:
        walkDeclList(n.DeclList, f)
    case *FuncDecl:
        Walk(n.Recv, f)
        Walk(n.Name, f)
        // ...
    }
}
```

#### SSA-Based Backend
```
AST → IR Nodes → SSA → Machine Code

src/cmd/compile/internal/ssa/
├── compile.go      # SSA compilation
├── rewrite.go      # SSA rewrites
├── lower.go        # Architecture lowering
└── gen/            # Code generation rules
```

### Key Files to Study
- `src/cmd/compile/internal/syntax/scanner.go` - Lexer
- `src/cmd/compile/internal/syntax/parser.go` - Parser
- `src/cmd/compile/internal/syntax/nodes.go` - AST nodes
- `src/cmd/compile/internal/syntax/walk.go` - AST traversal
- `src/cmd/compile/internal/ssa/` - SSA backend

---

## Zig Compiler

### Directory Structure
```
zig/src/
├── Air.zig          # Analyzed IR (post-Sema)
├── Sema.zig         # Semantic analysis (~38k lines!)
├── AstGen.zig       # AST → ZIR
├── Zir.zig          # Zig IR (pre-Sema)
├── Type.zig         # Type representation
├── Value.zig        # Value representation
├── InternPool.zig   # Interning/deduplication
├── Compilation.zig  # Compilation orchestration
├── codegen/
│   ├── llvm.zig     # LLVM backend
│   ├── x86_64.zig   # Native x86_64
│   └── c/           # C backend
└── link.zig         # Linker integration
```

### Key Patterns

#### IR Pipeline
```
Source
    ↓ (Parser)
AST
    ↓ (AstGen)
ZIR (Zig IR) - Untyped
    ↓ (Sema)
AIR (Analyzed IR) - Typed
    ↓ (Codegen)
LLVM IR / Native / C / WASM
```

#### Semantic Analysis (`Sema.zig`)
```zig
//! Semantic analysis of ZIR instructions.
//! Transforms untyped ZIR instructions into semantically-analyzed AIR.
//! Does type checking, comptime control flow, and safety-check generation.
//! This is the heart of the Zig compiler.

const Sema = @This();

pt: Zcu.PerThread,
gpa: Allocator,
arena: Allocator,
code: Zir,
air_instructions: std.MultiArrayList(Air.Inst) = .{},
air_extra: std.ArrayList(u32) = .empty,
inst_map: InstMap = .{},  // ZIR → AIR mapping
owner: AnalUnit,
func_index: InternPool.Index,
fn_ret_ty: Type,
// ...
```

#### Data-Oriented Design
- Multi-array lists for cache efficiency
- Interned pool for deduplication
- Index-based references instead of pointers
- Arena allocators for phase-scoped memory

```zig
// Index-based references
pub const Index = enum(u32) { ... };

// Multi-array for struct-of-arrays layout
air_instructions: std.MultiArrayList(Air.Inst) = .{},
```

#### Intern Pool
```zig
// Deduplicate types and values
const InternPool = @import("InternPool.zig");

// Types and values are interned
pub const Index = enum(u32) {
    none = std.math.maxInt(u32),
    // Predefined indices for common types
    u8_type, u16_type, u32_type, ...
    bool_type, void_type, ...
};
```

#### Multi-Backend Architecture
```zig
// src/codegen.zig orchestrates backends
pub const Backend = union(enum) {
    llvm: *Llvm,
    c: *C,
    x86_64: *x86_64,
    // ...
};

// Each backend in separate file
// src/codegen/llvm.zig
// src/codegen/c/emit.zig
// src/codegen/x86_64.zig
```

#### Bootstrap Strategy
1. Stage 1: C code → C compiler → bootstrap compiler
2. Stage 2: Zig code → bootstrap → stage2 compiler
3. Stage 3: Zig code → stage2 → final compiler
4. Also: WASM interpreter for cross-compilation

### Key Files to Study
- `src/Sema.zig` - Semantic analysis (core)
- `src/Air.zig` - Analyzed IR
- `src/AstGen.zig` - AST to ZIR
- `src/Type.zig` - Type representation
- `src/codegen/llvm.zig` - LLVM backend

---

## Common Patterns Across All Three

### AST Node Design
| Aspect | Rust | Go | Zig |
|--------|------|----|----|
| Position | `Span` struct | `Pos` uint | Lazy source loc |
| Node ID | `NodeId` | N/A | Index |
| Structure | Enum + Kind | Interface + Struct | Union tags |

### Lexer Architecture
| Aspect | Rust | Go | Zig |
|--------|------|----|----|
| Two-stage | Yes | No | Yes (ZIR) |
| Semicolons | Explicit | Auto-insert | Explicit |
| Keywords | Perfect hash | Perfect hash | Token tags |

### Error Handling
| Aspect | Rust | Go | Zig |
|--------|------|----|----|
| Accumulation | Yes, rich | Limited | Yes |
| Suggestions | Yes | Basic | Yes |
| Error codes | Yes (E0001) | No | No |

### Memory Management
| Aspect | Rust | Go | Zig |
|--------|------|----|----|
| Arena | Per-session | Runtime GC | Explicit arenas |
| Interning | Symbols, strings | N/A | Types, values |
| References | Rc/Arc | GC pointers | Indices |

---

## Extracted Best Practices

### Module Organization
1. Separate lexer from parser
2. AST definitions in dedicated module
3. Visitor patterns for traversal
4. Clear IR pipeline stages

### Performance
1. Use indices instead of pointers where possible
2. Intern common values (strings, types)
3. Arena allocators for phase-scoped data
4. Struct-of-arrays for cache efficiency

### Error Handling
1. Accumulate multiple errors
2. Include source locations
3. Provide actionable suggestions
4. Use error codes for documentation

### Testing
1. Tests alongside source (Go)
2. Snapshot testing for parser output
3. Exhaustive edge case coverage
4. Integration tests with real programs

---

## File Size Reference

| File | Lines | Purpose |
|------|-------|---------|
| Zig `Sema.zig` | ~38,000 | Semantic analysis |
| Go `parser.go` | ~1,800 | Parser |
| Go `scanner.go` | ~900 | Lexer |
| Rust `expr.rs` | ~4,000 | Expression parsing |
| Rust `ast.rs` | ~3,500 | AST definitions |

---

## Key References
- Rust Compiler Dev Guide: https://rustc-dev-guide.rust-lang.org/
- Go Compiler README: https://go.dev/src/cmd/compile/README
- Zig Language Reference: https://ziglang.org/documentation/
