# Phase 6: Formatter (Weeks 21-22)

## Goal

Build a high-performance CST-based formatter:
- Lossless concrete syntax tree
- Parallel file formatting
- Hash-based incremental formatting
- Strict Sigil formatting rules

**Deliverable:** `sigil fmt` command with <50ms single file, <1s for 100 files.

---

## Week 21: CST Architecture

### Objective

Build lossless CST that preserves all source information for formatting.

### Concrete Syntax Tree

```rust
/// Concrete syntax tree node
#[derive(Clone)]
pub enum CstNode {
    /// Terminal: actual token from source
    Token(Token),

    /// Trivia: whitespace, comments
    Trivia(Trivia),

    /// Non-terminal: named production with children
    Node {
        kind: CstKind,
        children: Vec<CstNode>,
    },
}

/// CST node kinds
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum CstKind {
    // Top-level
    SourceFile,
    Function,
    Type,
    Config,
    Import,
    Test,

    // Expressions
    Binary,
    Unary,
    Call,
    Index,
    Field,
    If,
    Match,
    For,
    Loop,
    Block,
    Lambda,
    Let,
    Pattern,

    // Types
    TypeExpr,
    TypeParam,
    TypeArg,

    // Patterns
    PatternArgs,
    PatternArg,

    // Misc
    ParamList,
    ArgList,
    MatchArm,
    ImportItem,
}

/// Trivia (whitespace and comments)
#[derive(Clone)]
pub enum Trivia {
    Whitespace(String),
    Newline,
    LineComment(String),
    DocComment(String),
}

/// Token with position info
#[derive(Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub span: Span,
}
```

### CST Builder

```rust
/// CST parser that preserves all trivia
pub struct CstParser<'src> {
    source: &'src str,
    tokens: Vec<Token>,
    trivia: Vec<Trivia>,
    position: usize,
}

impl<'src> CstParser<'src> {
    pub fn parse_file(&mut self) -> CstNode {
        let mut children = Vec::new();

        // Leading trivia
        children.extend(self.consume_trivia());

        // Items
        while !self.at_end() {
            children.push(self.parse_item());
            children.extend(self.consume_trivia());
        }

        CstNode::Node {
            kind: CstKind::SourceFile,
            children,
        }
    }

    fn parse_function(&mut self) -> CstNode {
        let mut children = Vec::new();

        // Visibility (optional)
        if self.at(TokenKind::Pub) {
            children.push(self.consume_token());
            children.extend(self.consume_trivia());
        }

        // @
        children.push(self.expect_token(TokenKind::At));
        children.extend(self.consume_trivia());

        // Name
        children.push(self.expect_token(TokenKind::Ident));
        children.extend(self.consume_trivia());

        // Type parameters (optional)
        if self.at(TokenKind::Lt) {
            children.push(self.parse_type_params());
            children.extend(self.consume_trivia());
        }

        // Parameters
        children.push(self.parse_params());
        children.extend(self.consume_trivia());

        // Return type
        children.push(self.expect_token(TokenKind::Arrow));
        children.extend(self.consume_trivia());
        children.push(self.parse_type());
        children.extend(self.consume_trivia());

        // Capabilities (optional)
        if self.at(TokenKind::Uses) {
            children.push(self.consume_token());
            children.extend(self.consume_trivia());
            children.push(self.parse_capability_list());
            children.extend(self.consume_trivia());
        }

        // =
        children.push(self.expect_token(TokenKind::Eq));
        children.extend(self.consume_trivia());

        // Body
        children.push(self.parse_expr());

        CstNode::Node {
            kind: CstKind::Function,
            children,
        }
    }

    fn consume_trivia(&mut self) -> Vec<CstNode> {
        let mut result = Vec::new();

        while let Some(trivia) = self.try_consume_trivia() {
            result.push(CstNode::Trivia(trivia));
        }

        result
    }
}
```

### CST to Source

```rust
impl CstNode {
    /// Reconstruct exact source (for validation)
    pub fn to_source(&self) -> String {
        let mut output = String::new();
        self.write_source(&mut output);
        output
    }

    fn write_source(&self, output: &mut String) {
        match self {
            CstNode::Token(token) => {
                output.push_str(&token.text);
            }
            CstNode::Trivia(trivia) => {
                match trivia {
                    Trivia::Whitespace(ws) => output.push_str(ws),
                    Trivia::Newline => output.push('\n'),
                    Trivia::LineComment(c) => {
                        output.push_str("//");
                        output.push_str(c);
                    }
                    Trivia::DocComment(c) => {
                        output.push_str("// ");
                        output.push_str(c);
                    }
                }
            }
            CstNode::Node { children, .. } => {
                for child in children {
                    child.write_source(output);
                }
            }
        }
    }
}
```

---

## Week 21 (continued): Formatting Engine

### Formatting Rules

```rust
/// Sigil formatting configuration (zero-config, but documented)
pub const FORMAT_CONFIG: FormatConfig = FormatConfig {
    indent_width: 4,
    max_line_width: 100,
    trailing_commas: true,
    space_around_binary_ops: true,
    space_around_arrows: true,
    space_after_colon: true,
    space_after_comma: true,
    blank_line_between_functions: true,
    max_consecutive_blank_lines: 1,
};

/// Intermediate representation for formatting decisions
pub enum FormatIR {
    /// Literal text
    Text(String),

    /// Hard line break (always breaks)
    HardLine,

    /// Soft line break (breaks if needed to fit)
    SoftLine,

    /// Indented group
    Indent(Box<FormatIR>),

    /// Group that may be broken
    Group(Vec<FormatIR>),

    /// Concatenation
    Concat(Vec<FormatIR>),

    /// If-break: different content when broken vs flat
    IfBreak {
        broken: Box<FormatIR>,
        flat: Box<FormatIR>,
    },
}
```

### Formatter Implementation

```rust
/// Format CST to IR
pub struct Formatter {
    config: FormatConfig,
}

impl Formatter {
    pub fn format_file(&self, cst: &CstNode) -> FormatIR {
        match cst {
            CstNode::Node { kind: CstKind::SourceFile, children } => {
                self.format_source_file(children)
            }
            _ => panic!("expected source file"),
        }
    }

    fn format_source_file(&self, children: &[CstNode]) -> FormatIR {
        let mut items = Vec::new();
        let mut prev_was_item = false;

        for child in children {
            match child {
                CstNode::Node { kind, .. } => {
                    // Blank line between top-level items
                    if prev_was_item {
                        items.push(FormatIR::HardLine);
                    }

                    items.push(self.format_item(child));
                    prev_was_item = true;
                }
                CstNode::Trivia(Trivia::DocComment(comment)) => {
                    items.push(FormatIR::Text(format!("// {}", comment.trim())));
                    items.push(FormatIR::HardLine);
                }
                CstNode::Trivia(Trivia::LineComment(comment)) => {
                    items.push(FormatIR::Text(format!("//{}", comment)));
                    items.push(FormatIR::HardLine);
                }
                _ => {}  // Skip whitespace - we regenerate it
            }
        }

        FormatIR::Concat(items)
    }

    fn format_function(&self, children: &[CstNode]) -> FormatIR {
        let mut parts = Vec::new();

        // Extract components from CST children
        let (visibility, name, params, ret_type, body) = self.extract_function_parts(children);

        // Visibility
        if let Some(vis) = visibility {
            parts.push(FormatIR::Text("pub ".into()));
        }

        // @ and name
        parts.push(FormatIR::Text(format!("@{}", name)));

        // Parameters
        parts.push(self.format_params(params));

        // Return type
        parts.push(FormatIR::Text(" -> ".into()));
        parts.push(self.format_type(ret_type));

        // =
        parts.push(FormatIR::Text(" = ".into()));

        // Body (may need breaking)
        parts.push(self.format_expr(body));

        FormatIR::Concat(parts)
    }

    fn format_params(&self, params: &CstNode) -> FormatIR {
        let param_list = self.extract_params(params);

        if param_list.is_empty() {
            return FormatIR::Text("()".into());
        }

        if param_list.len() == 1 && self.fits_inline(&param_list[0]) {
            // Single param, fits on line
            let formatted = self.format_param(&param_list[0]);
            return FormatIR::Concat(vec![
                FormatIR::Text("(".into()),
                formatted,
                FormatIR::Text(")".into()),
            ]);
        }

        // Multi-param or long param: may break
        let formatted_params: Vec<_> = param_list.iter()
            .map(|p| self.format_param(p))
            .collect();

        FormatIR::Group(vec![
            FormatIR::Text("(".into()),
            FormatIR::IfBreak {
                broken: Box::new(FormatIR::Concat(vec![
                    FormatIR::HardLine,
                    FormatIR::Indent(Box::new(
                        self.join_with_separator(&formatted_params, FormatIR::Concat(vec![
                            FormatIR::Text(",".into()),
                            FormatIR::HardLine,
                        ]))
                    )),
                    FormatIR::Text(",".into()),  // Trailing comma
                    FormatIR::HardLine,
                ])),
                flat: Box::new(
                    self.join_with_separator(&formatted_params, FormatIR::Text(", ".into()))
                ),
            },
            FormatIR::Text(")".into()),
        ])
    }

    fn format_pattern(&self, kind: CstKind, args: &[CstNode]) -> FormatIR {
        let pattern_name = self.pattern_name(kind);
        let formatted_args = self.format_pattern_args(args);

        // Pattern args always break if 2+ properties
        if args.len() >= 2 {
            FormatIR::Concat(vec![
                FormatIR::Text(format!("{}(", pattern_name)),
                FormatIR::HardLine,
                FormatIR::Indent(Box::new(formatted_args)),
                FormatIR::HardLine,
                FormatIR::Text(")".into()),
            ])
        } else {
            FormatIR::Concat(vec![
                FormatIR::Text(format!("{}(", pattern_name)),
                formatted_args,
                FormatIR::Text(")".into()),
            ])
        }
    }
}
```

### IR to String

```rust
/// Render format IR to string
pub struct Printer {
    config: FormatConfig,
    output: String,
    current_line_width: usize,
    indent_level: usize,
}

impl Printer {
    pub fn print(&mut self, ir: &FormatIR) -> String {
        self.print_ir(ir, false);
        std::mem::take(&mut self.output)
    }

    fn print_ir(&mut self, ir: &FormatIR, broken: bool) {
        match ir {
            FormatIR::Text(text) => {
                self.output.push_str(text);
                self.current_line_width += text.len();
            }

            FormatIR::HardLine => {
                self.output.push('\n');
                self.output.push_str(&" ".repeat(self.indent_level * self.config.indent_width));
                self.current_line_width = self.indent_level * self.config.indent_width;
            }

            FormatIR::SoftLine => {
                if broken {
                    self.print_ir(&FormatIR::HardLine, broken);
                } else {
                    self.output.push(' ');
                    self.current_line_width += 1;
                }
            }

            FormatIR::Indent(inner) => {
                self.indent_level += 1;
                self.print_ir(inner, broken);
                self.indent_level -= 1;
            }

            FormatIR::Group(children) => {
                // Try flat first
                let flat_output = self.try_flat(children);

                if flat_output.len() + self.current_line_width <= self.config.max_line_width {
                    self.output.push_str(&flat_output);
                    self.current_line_width += flat_output.len();
                } else {
                    // Must break
                    for child in children {
                        self.print_ir(child, true);
                    }
                }
            }

            FormatIR::Concat(children) => {
                for child in children {
                    self.print_ir(child, broken);
                }
            }

            FormatIR::IfBreak { broken: b, flat: f } => {
                if broken {
                    self.print_ir(b, true);
                } else {
                    self.print_ir(f, false);
                }
            }
        }
    }
}
```

---

## Week 22: Parallel & Incremental Formatting

### Parallel File Formatting

```rust
/// Format all files in parallel
pub fn format_project(
    db: &dyn Db,
    files: &[SourceFile],
    check_only: bool,
) -> FormatResult {
    let results: Vec<_> = files
        .par_iter()
        .map(|file| format_file(db, *file, check_only))
        .collect();

    let changed: Vec<_> = results.iter()
        .filter(|r| r.changed)
        .map(|r| r.path.clone())
        .collect();

    let errors: Vec<_> = results.iter()
        .filter_map(|r| r.error.as_ref())
        .cloned()
        .collect();

    FormatResult {
        files_checked: files.len(),
        files_changed: changed.len(),
        changed_paths: changed,
        errors,
    }
}

fn format_file(db: &dyn Db, file: SourceFile, check_only: bool) -> FileFormatResult {
    let path = file.path(db);
    let source = file.text(db);

    // Parse to CST
    let cst = parse_cst(source);

    // Format
    let formatter = Formatter::new();
    let ir = formatter.format_file(&cst);

    let mut printer = Printer::new();
    let formatted = printer.print(&ir);

    let changed = formatted != source;

    if changed && !check_only {
        // Write back
        std::fs::write(&path, &formatted).ok();
    }

    FileFormatResult {
        path: path.to_path_buf(),
        changed,
        error: None,
    }
}
```

### Hash-Based Incremental Formatting

```rust
/// Cache of file content hashes for incremental formatting
pub struct FormatCache {
    /// Path → (content_hash, formatted_hash)
    cache: DashMap<PathBuf, (u64, u64)>,
    cache_path: PathBuf,
}

impl FormatCache {
    pub fn load(project_root: &Path) -> Self {
        let cache_path = project_root.join(".sigil/format-cache");

        let cache = if cache_path.exists() {
            let data = std::fs::read(&cache_path).unwrap_or_default();
            bincode::deserialize(&data).unwrap_or_default()
        } else {
            DashMap::new()
        };

        Self { cache, cache_path }
    }

    pub fn save(&self) {
        if let Ok(data) = bincode::serialize(&self.cache) {
            let _ = std::fs::write(&self.cache_path, data);
        }
    }

    /// Check if file needs formatting
    pub fn needs_format(&self, path: &Path, content: &str) -> bool {
        let content_hash = hash_content(content);

        match self.cache.get(path) {
            Some(entry) => {
                let (cached_content, _) = *entry;
                cached_content != content_hash
            }
            None => true,
        }
    }

    /// Update cache after formatting
    pub fn update(&self, path: &Path, original: &str, formatted: &str) {
        let content_hash = hash_content(original);
        let formatted_hash = hash_content(formatted);
        self.cache.insert(path.to_path_buf(), (content_hash, formatted_hash));
    }
}

fn hash_content(content: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    use rustc_hash::FxHasher;

    let mut hasher = FxHasher::default();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Format with caching
pub fn format_project_cached(
    db: &dyn Db,
    files: &[SourceFile],
    check_only: bool,
) -> FormatResult {
    let cache = FormatCache::load(db.project_root());

    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            let path = file.path(db);
            let content = file.text(db);

            // Skip if cached and unchanged
            if !cache.needs_format(&path, &content) {
                return None;
            }

            Some(format_file_with_cache(db, *file, check_only, &cache))
        })
        .collect();

    cache.save();

    FormatResult::from_results(results)
}
```

### CLI Integration

```rust
/// Format command implementation
pub fn cmd_format(args: FormatArgs) -> Result<()> {
    let db = CompilerDb::new();

    // Find files to format
    let files = if args.paths.is_empty() {
        db.all_sigil_files()
    } else {
        args.paths.iter()
            .flat_map(|p| find_sigil_files(p))
            .map(|p| db.source_file(p))
            .collect()
    };

    let start = Instant::now();

    let result = if args.no_cache {
        format_project(&db, &files, args.check)
    } else {
        format_project_cached(&db, &files, args.check)
    };

    let elapsed = start.elapsed();

    if args.check {
        if result.files_changed > 0 {
            eprintln!(
                "{} file(s) would be reformatted",
                result.files_changed
            );
            for path in &result.changed_paths {
                eprintln!("  {}", path.display());
            }
            std::process::exit(1);
        } else {
            eprintln!("All {} files formatted correctly", result.files_checked);
        }
    } else {
        if result.files_changed > 0 {
            eprintln!(
                "Formatted {} file(s) in {:?}",
                result.files_changed,
                elapsed
            );
        } else {
            eprintln!(
                "Checked {} file(s) in {:?}, no changes needed",
                result.files_checked,
                elapsed
            );
        }
    }

    Ok(())
}
```

---

## Formatting Rules Reference

### Indentation
- 4 spaces, never tabs
- Continued lines aligned to opening delimiter or indented 4 spaces

### Line Length
- 100 characters hard limit
- Break before binary operators
- Break after opening delimiters for multi-line

### Spacing
```sigil
// Binary operators: space around
a + b
x == y

// Arrows: space around
(x: int) -> int
x -> x + 1

// Colons: space after
name: Type
.key: value

// Commas: space after
f(a, b, c)

// Parens/brackets: no space inside
f(x)
[1, 2, 3]

// Comments: space after //
// This is a comment
```

### Breaking Rules
```sigil
// Pattern args 2+ properties: always stack
map(
    .over: items,
    .transform: x -> x * 2,
)

// Long function signatures: break at arrow
@very_long_function_name (param1: Type1, param2: Type2)
    -> ReturnType = body

// Long binary expressions: break before operator
very_long_expression
    + another_long_expression
    + yet_another_one
```

### Blank Lines
```sigil
// One after import block
use std.math { sqrt }

@function1 () -> int = 1

// One between functions
@function2 () -> int = 2

// No consecutive blank lines
// No trailing blank lines
// No leading blank lines in blocks
```

---

## Performance Targets

| Metric | Target | Strategy |
|--------|--------|----------|
| Single file (1KB) | <10ms | Direct formatting |
| Single file (10KB) | <50ms | Direct formatting |
| 100 files | <1s | Parallel + caching |
| 1000 files | <5s | Parallel + caching |
| Incremental (1 changed) | <100ms | Hash-based skip |

---

## Phase 6 Deliverables Checklist

### Week 21: CST Architecture
- [ ] `CstNode` enum with all variants
- [ ] `CstParser` preserving trivia
- [ ] CST to source reconstruction
- [ ] Roundtrip tests (parse → source = original)

### Week 21: Formatting Engine
- [ ] `FormatIR` intermediate representation
- [ ] `Formatter` CST to IR
- [ ] `Printer` IR to string
- [ ] All formatting rules implemented

### Week 22: Parallel & Incremental
- [ ] `format_project` parallel formatting
- [ ] `FormatCache` hash-based caching
- [ ] `format_project_cached` incremental
- [ ] CLI `sigil fmt` command
- [ ] CLI `sigil fmt --check` for CI

### Tests
- [ ] Formatting rule tests
- [ ] Roundtrip tests
- [ ] Performance benchmarks
- [ ] Idempotency tests (format(format(x)) == format(x))

---

## V2 Complete

With Phase 6 complete, the V2 compiler is feature-complete:

1. **Foundation** - Interning, flat AST, Salsa ✓
2. **Type System** - Type interning, inference ✓
3. **Patterns** - Templates, fusion ✓
4. **Parallelism** - Work-stealing, parallel pipeline ✓
5. **Advanced** - Test-gating, LSP ✓
6. **Formatter** - CST-based, parallel ✓

Proceed to [H: Migration](../appendices/H-migration.md) for V1→V2 rollout strategy.
