# Phase 4: Parallelism (Weeks 13-16)

## Goal

Build the parallel compilation pipeline:
- Parallel file lexing/parsing
- Parallel module type checking
- Parallel function codegen
- Work-stealing scheduler

**Deliverable:** Full parallel compilation with near-linear speedup.

---

## Week 13: Parallel Parsing

### Objective

Lex and parse all source files concurrently using Rayon.

### File-Level Parallelism

```rust
use rayon::prelude::*;

/// Parse all project files in parallel
pub fn parse_project(db: &dyn Db, files: &[SourceFile]) -> Vec<Module> {
    files
        .par_iter()
        .map(|file| {
            // Each file gets its own token stream
            let tokens = tokens(db, *file);

            // Parse into module
            parsed_module(db, *file)
        })
        .collect()
}
```

### Parallel Lexer with Chunking

```rust
/// Parallel lexer that chunks large files
pub fn lex_parallel(source: &str, interner: &StringInterner) -> TokenList {
    const CHUNK_THRESHOLD: usize = 100_000;  // 100KB

    if source.len() < CHUNK_THRESHOLD {
        // Small file: single-threaded
        return lex_sequential(source, interner);
    }

    // Large file: parallel chunking
    let chunk_size = source.len() / rayon::current_num_threads();
    let chunks = find_safe_chunk_boundaries(source, chunk_size);

    let token_lists: Vec<TokenList> = chunks
        .par_iter()
        .map(|chunk| lex_sequential(chunk.text, interner))
        .collect();

    merge_token_lists(token_lists, &chunks)
}

/// Find chunk boundaries at safe positions (not mid-token)
fn find_safe_chunk_boundaries(source: &str, target_size: usize) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < source.len() {
        let mut end = (start + target_size).min(source.len());

        // Adjust to safe boundary
        if end < source.len() {
            end = find_statement_boundary(&source[start..], end - start) + start;
        }

        chunks.push(Chunk {
            text: &source[start..end],
            offset: start,
        });

        start = end;
    }

    chunks
}

/// Find safe boundary (newline not in string/comment)
fn find_statement_boundary(source: &str, near: usize) -> usize {
    // Sigil has no semicolons - newlines are statement boundaries
    // Find nearest newline that's not in a string literal

    let bytes = source.as_bytes();
    let mut in_string = false;
    let mut pos = near;

    // Search forward for newline
    while pos < source.len() {
        match bytes[pos] {
            b'"' if !in_string => in_string = true,
            b'"' if in_string && bytes.get(pos.wrapping_sub(1)) != Some(&b'\\') => {
                in_string = false;
            }
            b'\n' if !in_string => return pos + 1,
            _ => {}
        }
        pos += 1;
    }

    source.len()
}
```

### Pattern-Aware Parsing

```rust
/// Detect pattern boundaries for parallel pattern parsing
pub fn detect_pattern_boundaries(source: &str) -> Vec<PatternSpan> {
    let mut spans = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Look for pattern keywords followed by '('
        if let Some(keyword_len) = match_pattern_keyword(&bytes[i..]) {
            let start = i;
            i += keyword_len;

            // Skip whitespace
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }

            // Must be followed by '('
            if i < bytes.len() && bytes[i] == b'(' {
                let end = find_matching_paren(bytes, i);
                spans.push(PatternSpan { start, end });
                i = end;
            }
        } else {
            i += 1;
        }
    }

    spans
}

fn match_pattern_keyword(bytes: &[u8]) -> Option<usize> {
    const KEYWORDS: &[&[u8]] = &[
        b"map", b"filter", b"fold", b"find", b"collect",
        b"run", b"try", b"match", b"recurse",
        b"parallel", b"timeout", b"retry", b"cache", b"validate",
    ];

    for keyword in KEYWORDS {
        if bytes.starts_with(keyword) {
            // Check it's a complete word
            let next = bytes.get(keyword.len()).copied();
            if next.map_or(true, |b| !b.is_ascii_alphanumeric() && b != b'_') {
                return Some(keyword.len());
            }
        }
    }

    None
}
```

---

## Weeks 14-15: Parallel Type Checking

### Objective

Type check modules and functions in parallel with work-stealing.

### Dependency Analysis

```rust
/// Build dependency graph for parallel type checking
#[salsa::tracked]
pub fn module_dependency_graph(db: &dyn Db) -> DependencyGraph {
    let modules: Vec<Module> = db.all_modules();

    let mut graph = DependencyGraph::new();

    for module in &modules {
        let imports = resolved_imports(db, *module);

        for import in &imports.imports {
            graph.add_edge(*module, import.module);
        }
    }

    graph
}

impl DependencyGraph {
    /// Get modules in topological order for parallel processing
    pub fn parallel_order(&self) -> Vec<Vec<Module>> {
        let mut levels = Vec::new();
        let mut remaining: FxHashSet<_> = self.nodes.iter().copied().collect();
        let mut processed = FxHashSet::default();

        while !remaining.is_empty() {
            // Find all modules whose dependencies are satisfied
            let ready: Vec<_> = remaining
                .iter()
                .filter(|m| {
                    self.dependencies(*m)
                        .all(|dep| processed.contains(&dep))
                })
                .copied()
                .collect();

            if ready.is_empty() && !remaining.is_empty() {
                // Cycle detected - handle gracefully
                break;
            }

            for m in &ready {
                remaining.remove(m);
                processed.insert(*m);
            }

            levels.push(ready);
        }

        levels
    }
}
```

### Work-Stealing Type Checker

```rust
use crossbeam_deque::{Injector, Stealer, Worker};

/// Task for type checking
pub enum TypeCheckTask {
    Module(Module),
    Function(Function),
    TraitImpl(ImplId),
}

/// Work-stealing pool for type checking
pub struct TypeCheckPool {
    injector: Injector<TypeCheckTask>,
    workers: Vec<Worker<TypeCheckTask>>,
    stealers: Vec<Stealer<TypeCheckTask>>,
}

impl TypeCheckPool {
    pub fn new(num_threads: usize) -> Self {
        let injector = Injector::new();
        let workers: Vec<_> = (0..num_threads).map(|_| Worker::new_fifo()).collect();
        let stealers: Vec<_> = workers.iter().map(|w| w.stealer()).collect();

        Self { injector, workers, stealers }
    }

    /// Add tasks to global queue
    pub fn submit(&self, tasks: impl IntoIterator<Item = TypeCheckTask>) {
        for task in tasks {
            self.injector.push(task);
        }
    }

    /// Run type checking with work-stealing
    pub fn run(&self, db: &dyn Db) -> Vec<TypeCheckResult> {
        let results = Arc::new(Mutex::new(Vec::new()));

        std::thread::scope(|s| {
            for (i, worker) in self.workers.iter().enumerate() {
                let results = Arc::clone(&results);
                let stealers = &self.stealers;
                let injector = &self.injector;

                s.spawn(move || {
                    loop {
                        // Try local queue first
                        let task = worker.pop()
                            // Then try global queue
                            .or_else(|| loop {
                                match injector.steal() {
                                    crossbeam_deque::Steal::Success(t) => break Some(t),
                                    crossbeam_deque::Steal::Empty => break None,
                                    crossbeam_deque::Steal::Retry => {}
                                }
                            })
                            // Then steal from others
                            .or_else(|| {
                                stealers
                                    .iter()
                                    .enumerate()
                                    .filter(|(j, _)| *j != i)
                                    .find_map(|(_, s)| loop {
                                        match s.steal() {
                                            crossbeam_deque::Steal::Success(t) => break Some(t),
                                            crossbeam_deque::Steal::Empty => break None,
                                            crossbeam_deque::Steal::Retry => {}
                                        }
                                    })
                            });

                        match task {
                            Some(task) => {
                                let result = process_task(db, task);
                                results.lock().unwrap().push(result);
                            }
                            None => {
                                // No more work - check if truly done
                                if injector.is_empty() &&
                                   stealers.iter().all(|s| s.is_empty())
                                {
                                    break;
                                }
                                // Spin briefly then retry
                                std::thread::yield_now();
                            }
                        }
                    }
                });
            }
        });

        Arc::try_unwrap(results).unwrap().into_inner().unwrap()
    }
}

fn process_task(db: &dyn Db, task: TypeCheckTask) -> TypeCheckResult {
    match task {
        TypeCheckTask::Function(func) => {
            TypeCheckResult::Function(typed_function(db, func))
        }
        TypeCheckTask::Module(module) => {
            TypeCheckResult::Module(typed_module(db, module))
        }
        TypeCheckTask::TraitImpl(impl_id) => {
            TypeCheckResult::TraitImpl(check_trait_impl(db, impl_id))
        }
    }
}
```

### Level-Based Parallelism

```rust
/// Type check project with level-based parallelism
pub fn type_check_project(db: &dyn Db) -> ProjectTypeCheckResult {
    let graph = module_dependency_graph(db);
    let levels = graph.parallel_order();

    let mut all_results = Vec::new();

    for level in levels {
        // All modules in a level can be checked in parallel
        let level_results: Vec<_> = level
            .par_iter()
            .map(|module| typed_module(db, *module))
            .collect();

        all_results.extend(level_results);
    }

    ProjectTypeCheckResult::from_modules(all_results)
}
```

---

## Week 16: Parallel Codegen

### Objective

Generate C code for functions in parallel.

### Function-Level Parallelism

```rust
/// Generate code for all functions in parallel
pub fn codegen_project(db: &dyn Db, typed_modules: &[TypedModule]) -> GeneratedProject {
    // Collect all functions
    let functions: Vec<_> = typed_modules
        .iter()
        .flat_map(|m| m.functions.iter())
        .collect();

    // Generate code in parallel
    let generated: Vec<GeneratedFunction> = functions
        .par_iter()
        .map(|func| generated_function(db, func.func))
        .collect();

    // Generate module structure (sequential - small)
    let module_code = generate_module_structure(db, typed_modules);

    GeneratedProject {
        functions: generated,
        module_code,
    }
}

/// Salsa query for single function codegen
#[salsa::tracked]
pub fn generated_function(db: &dyn Db, func: Function) -> GeneratedFunction {
    let typed = typed_function(db, func);
    let ctx = CodegenContext::new(db);

    let code = ctx.generate_function(&typed);

    GeneratedFunction {
        func,
        code,
        dependencies: ctx.dependencies(),
    }
}
```

### Template Parallel Instantiation

```rust
/// Instantiate pattern templates in parallel
pub fn instantiate_templates(
    db: &dyn Db,
    pattern_usages: &[PatternUsage],
) -> Vec<InstantiatedTemplate> {
    let cache = db.pattern_template_cache();

    pattern_usages
        .par_iter()
        .map(|usage| {
            let sig = usage.signature();

            // Get or compile template (thread-safe cache)
            let template = cache.get_or_compile(&sig, || {
                compile_template(&sig)
            });

            // Instantiate with concrete arguments
            template.instantiate(&usage.args)
        })
        .collect()
}
```

### Parallel Output Generation

```rust
/// Write generated code to files in parallel
pub fn write_output_parallel(
    generated: &GeneratedProject,
    output_dir: &Path,
) -> io::Result<()> {
    // Create output directory
    fs::create_dir_all(output_dir)?;

    // Write all function files in parallel
    generated.functions
        .par_iter()
        .try_for_each(|func| {
            let path = output_dir.join(format!("{}.c", func.name()));
            fs::write(&path, &func.code.0)
        })?;

    // Write main module file
    let main_path = output_dir.join("main.c");
    fs::write(&main_path, &generated.module_code)?;

    Ok(())
}
```

---

## Thread Pool Configuration

### Adaptive Sizing

```rust
/// Configure thread pool based on workload
pub struct ThreadPoolConfig {
    /// Number of threads for parsing
    pub parse_threads: usize,
    /// Number of threads for type checking
    pub check_threads: usize,
    /// Number of threads for codegen
    pub codegen_threads: usize,
}

impl ThreadPoolConfig {
    pub fn auto() -> Self {
        let cpus = num_cpus::get();

        Self {
            // Parsing is I/O bound - can use more threads
            parse_threads: cpus * 2,
            // Type checking is CPU bound
            check_threads: cpus,
            // Codegen is CPU bound with some memory pressure
            codegen_threads: cpus.max(1) - 1,  // Leave one for main thread
        }
    }

    pub fn for_project_size(file_count: usize, total_loc: usize) -> Self {
        let cpus = num_cpus::get();

        if file_count < 10 || total_loc < 1000 {
            // Small project - minimal parallelism
            Self {
                parse_threads: 2,
                check_threads: 2,
                codegen_threads: 2,
            }
        } else if file_count < 100 || total_loc < 10000 {
            // Medium project
            Self {
                parse_threads: cpus,
                check_threads: cpus,
                codegen_threads: cpus / 2,
            }
        } else {
            // Large project - full parallelism
            Self::auto()
        }
    }
}
```

### Thread-Local Caching

```rust
/// Thread-local state for parallel compilation
thread_local! {
    /// Per-thread expression arena for parsing
    static PARSE_ARENA: RefCell<ExprArena> = RefCell::new(ExprArena::new());

    /// Per-thread codegen buffer
    static CODEGEN_BUFFER: RefCell<String> = RefCell::new(String::with_capacity(64 * 1024));
}

/// Use thread-local arena for parsing
pub fn parse_with_thread_local_arena(tokens: &TokenList, interner: &StringInterner) -> Module {
    PARSE_ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        arena.reset();  // Reuse allocation

        let mut parser = Parser::new(tokens, interner, &mut arena);
        parser.parse_module()
    })
}
```

---

## Synchronization Strategies

### DashMap for Shared State

```rust
use dashmap::DashMap;

/// Thread-safe caches using DashMap
pub struct SharedCaches {
    /// Interned types (read-heavy)
    pub types: DashMap<TypeKind, TypeId>,

    /// Pattern templates (read-heavy after warmup)
    pub templates: DashMap<PatternSignature, Arc<CompiledTemplate>>,

    /// Resolved imports (written once per module)
    pub imports: DashMap<Module, ResolvedImports>,
}

impl SharedCaches {
    pub fn new() -> Self {
        Self {
            types: DashMap::with_capacity_and_shard_amount(4096, 32),
            templates: DashMap::with_capacity_and_shard_amount(1024, 16),
            imports: DashMap::with_capacity_and_shard_amount(256, 8),
        }
    }
}
```

### Lock-Free Progress Tracking

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

/// Track compilation progress without locks
pub struct ProgressTracker {
    total_files: AtomicUsize,
    parsed_files: AtomicUsize,
    typed_functions: AtomicUsize,
    generated_functions: AtomicUsize,
}

impl ProgressTracker {
    pub fn file_parsed(&self) {
        self.parsed_files.fetch_add(1, Ordering::Relaxed);
    }

    pub fn function_typed(&self) {
        self.typed_functions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn function_generated(&self) {
        self.generated_functions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn progress(&self) -> Progress {
        Progress {
            parsed: self.parsed_files.load(Ordering::Relaxed),
            typed: self.typed_functions.load(Ordering::Relaxed),
            generated: self.generated_functions.load(Ordering::Relaxed),
            total: self.total_files.load(Ordering::Relaxed),
        }
    }
}
```

---

## Phase 4 Deliverables Checklist

### Week 13: Parallel Parsing
- [ ] File-level parallel parsing with Rayon
- [ ] Chunk-based parallel lexing for large files
- [ ] Safe chunk boundary detection
- [ ] Pattern boundary detection
- [ ] Thread-local arenas for parsing

### Weeks 14-15: Parallel Type Checking
- [ ] Module dependency graph construction
- [ ] Topological ordering for parallel levels
- [ ] Work-stealing task pool
- [ ] Level-based parallel type checking
- [ ] DashMap for shared caches

### Week 16: Parallel Codegen
- [ ] Function-level parallel codegen
- [ ] Template parallel instantiation
- [ ] Parallel file output
- [ ] Thread pool configuration

### Synchronization
- [ ] DashMap integration for caches
- [ ] Lock-free progress tracking
- [ ] Thread-local state management

### Benchmarks
- [ ] Scaling tests (1-16 cores)
- [ ] Work-stealing efficiency
- [ ] Lock contention analysis
- [ ] Memory overhead per thread

---

## Expected Speedup

| Cores | Parse | Type Check | Codegen | Overall |
|-------|-------|------------|---------|---------|
| 1 | 1x | 1x | 1x | 1x |
| 2 | 1.9x | 1.8x | 1.9x | 1.85x |
| 4 | 3.5x | 3.2x | 3.6x | 3.4x |
| 8 | 6.5x | 5.5x | 6.8x | 6.2x |
| 16 | 11x | 8x | 12x | 10x |

*Type checking has lower scaling due to dependency ordering constraints*

---

## Next Phase

With parallel compilation complete, proceed to [Phase 5: Advanced](07-phase-5-advanced.md) for test-gated invalidation and LSP support.
