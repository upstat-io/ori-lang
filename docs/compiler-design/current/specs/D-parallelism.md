# D: Parallelism Specification

This document specifies the parallel compilation infrastructure for the V2 compiler.

---

## Thread Pool Architecture

### Rayon Global Pool

```rust
/// Initialize Rayon with custom configuration
pub fn init_thread_pool(config: &ThreadPoolConfig) -> Result<(), ThreadPoolBuildError> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(config.max_threads)
        .stack_size(config.stack_size)
        .thread_name(|i| format!("sigil-worker-{}", i))
        .build_global()
}

/// Thread pool configuration
pub struct ThreadPoolConfig {
    /// Maximum worker threads
    pub max_threads: usize,

    /// Stack size per thread (bytes)
    pub stack_size: usize,

    /// Thread priority (platform-specific)
    pub priority: ThreadPriority,
}

impl Default for ThreadPoolConfig {
    fn default() -> Self {
        Self {
            max_threads: num_cpus::get(),
            stack_size: 4 * 1024 * 1024,  // 4 MB
            priority: ThreadPriority::Normal,
        }
    }
}

impl ThreadPoolConfig {
    /// Configuration for small projects
    pub fn small() -> Self {
        Self {
            max_threads: 2.max(num_cpus::get() / 2),
            ..Default::default()
        }
    }

    /// Configuration for large projects
    pub fn large() -> Self {
        Self {
            max_threads: num_cpus::get(),
            stack_size: 8 * 1024 * 1024,  // 8 MB for deep recursion
            ..Default::default()
        }
    }
}
```

---

## Work-Stealing Scheduler

### Task Queue

```rust
use crossbeam_deque::{Injector, Stealer, Worker};

/// Work-stealing task scheduler
pub struct WorkStealingScheduler<T: Send> {
    /// Global task queue
    injector: Injector<T>,

    /// Per-worker local queues
    workers: Vec<Worker<T>>,

    /// Stealers for cross-worker stealing
    stealers: Vec<Stealer<T>>,

    /// Number of active workers
    active_count: AtomicUsize,
}

impl<T: Send> WorkStealingScheduler<T> {
    pub fn new(num_workers: usize) -> Self {
        let injector = Injector::new();
        let workers: Vec<_> = (0..num_workers)
            .map(|_| Worker::new_fifo())
            .collect();
        let stealers: Vec<_> = workers.iter()
            .map(|w| w.stealer())
            .collect();

        Self {
            injector,
            workers,
            stealers,
            active_count: AtomicUsize::new(0),
        }
    }

    /// Submit task to global queue
    pub fn submit(&self, task: T) {
        self.injector.push(task);
    }

    /// Submit multiple tasks
    pub fn submit_batch(&self, tasks: impl IntoIterator<Item = T>) {
        for task in tasks {
            self.injector.push(task);
        }
    }

    /// Get next task for worker (local → global → steal)
    fn next_task(&self, worker_id: usize) -> Option<T> {
        let worker = &self.workers[worker_id];

        // 1. Try local queue
        if let Some(task) = worker.pop() {
            return Some(task);
        }

        // 2. Try global queue
        loop {
            match self.injector.steal() {
                crossbeam_deque::Steal::Success(task) => return Some(task),
                crossbeam_deque::Steal::Empty => break,
                crossbeam_deque::Steal::Retry => continue,
            }
        }

        // 3. Try stealing from other workers
        self.stealers
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != worker_id)
            .find_map(|(_, stealer)| loop {
                match stealer.steal() {
                    crossbeam_deque::Steal::Success(task) => break Some(task),
                    crossbeam_deque::Steal::Empty => break None,
                    crossbeam_deque::Steal::Retry => continue,
                }
            })
    }
}
```

### Parallel Execution

```rust
impl<T: Send> WorkStealingScheduler<T> {
    /// Run tasks to completion, returning results
    pub fn run<R: Send>(
        &self,
        process: impl Fn(T) -> R + Send + Sync,
    ) -> Vec<R> {
        let results = Arc::new(Mutex::new(Vec::new()));
        let process = Arc::new(process);

        std::thread::scope(|s| {
            for worker_id in 0..self.workers.len() {
                let results = Arc::clone(&results);
                let process = Arc::clone(&process);

                s.spawn(move || {
                    self.active_count.fetch_add(1, Ordering::SeqCst);

                    loop {
                        match self.next_task(worker_id) {
                            Some(task) => {
                                let result = process(task);
                                results.lock().unwrap().push(result);
                            }
                            None => {
                                // Check if all workers are idle
                                let active = self.active_count.fetch_sub(1, Ordering::SeqCst);
                                if active == 1 && self.injector.is_empty() {
                                    // Last worker, all done
                                    break;
                                }

                                // Wait briefly and retry
                                std::thread::yield_now();
                                self.active_count.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    }
                });
            }
        });

        Arc::try_unwrap(results).unwrap().into_inner().unwrap()
    }
}
```

---

## DashMap Usage

### Concurrent Hash Maps

```rust
use dashmap::DashMap;

/// Shared type interner using DashMap
pub struct TypeInterner {
    /// TypeKind → TypeId (for deduplication)
    map: DashMap<TypeKind, TypeId>,

    /// TypeId → TypeKind (for lookup)
    types: RwLock<Vec<TypeKind>>,

    /// Type ranges
    ranges: RwLock<Vec<TypeId>>,
}

impl TypeInterner {
    pub fn new() -> Self {
        Self {
            // Use more shards for high contention
            map: DashMap::with_capacity_and_shard_amount(4096, 32),
            types: RwLock::new(Vec::with_capacity(4096)),
            ranges: RwLock::new(Vec::with_capacity(1024)),
        }
    }

    pub fn intern(&self, kind: TypeKind) -> TypeId {
        // Fast path: already interned
        if let Some(id) = self.map.get(&kind) {
            return *id;
        }

        // Slow path: insert new
        // Use entry API to avoid TOCTOU race
        *self.map.entry(kind.clone()).or_insert_with(|| {
            let mut types = self.types.write();
            let id = TypeId(types.len() as u32);
            types.push(kind);
            id
        })
    }
}
```

### Sharding Strategy

```rust
/// Configure DashMap sharding based on expected contention
pub fn configure_dashmap_shards(contention: ContentionLevel) -> usize {
    match contention {
        // Low contention: fewer shards, less memory
        ContentionLevel::Low => 8,

        // Medium: balance
        ContentionLevel::Medium => 16,

        // High: more shards, less lock contention
        ContentionLevel::High => 32,

        // Very high (e.g., string interner during parallel parse)
        ContentionLevel::VeryHigh => 64,
    }
}

#[derive(Copy, Clone)]
pub enum ContentionLevel {
    Low,
    Medium,
    High,
    VeryHigh,
}
```

---

## Parallel Parsing

### File-Level Parallelism

```rust
/// Parse all files in parallel
pub fn parse_files_parallel(
    db: &dyn Db,
    files: &[SourceFile],
) -> Vec<Module> {
    files
        .par_iter()
        .map(|file| {
            let tokens = tokens(db, *file);
            parsed_module(db, *file)
        })
        .collect()
}
```

### Chunked Lexing for Large Files

```rust
/// Parallel lexer for large files
pub fn lex_large_file(
    source: &str,
    interner: &StringInterner,
    threshold: usize,
) -> TokenList {
    if source.len() < threshold {
        return lex_sequential(source, interner);
    }

    let chunk_size = source.len() / rayon::current_num_threads();
    let boundaries = find_safe_boundaries(source, chunk_size);

    // Lex chunks in parallel
    let chunk_results: Vec<_> = boundaries
        .par_windows(2)
        .map(|window| {
            let start = window[0];
            let end = window[1];
            lex_chunk(&source[start..end], start, interner)
        })
        .collect();

    // Merge results
    merge_token_lists(chunk_results)
}

fn find_safe_boundaries(source: &str, target_size: usize) -> Vec<usize> {
    let mut boundaries = vec![0];
    let mut pos = target_size;

    while pos < source.len() {
        // Find newline near target position
        let safe_pos = find_newline_boundary(source, pos);
        boundaries.push(safe_pos);
        pos = safe_pos + target_size;
    }

    boundaries.push(source.len());
    boundaries
}
```

---

## Parallel Type Checking

### Module Dependency Graph

```rust
/// Build module dependency graph for parallel ordering
pub fn build_dependency_graph(db: &dyn Db) -> DependencyGraph {
    let modules: Vec<_> = db.all_modules();

    let mut graph = DependencyGraph::new();

    for module in &modules {
        let imports = resolved_imports(db, *module);

        for import in &imports.resolved {
            // module depends on import.target
            graph.add_edge(*module, import.target);
        }
    }

    graph
}

/// Compute parallel levels (modules in same level can run concurrently)
pub fn compute_parallel_levels(graph: &DependencyGraph) -> Vec<Vec<Module>> {
    let mut levels = Vec::new();
    let mut processed = FxHashSet::default();
    let mut remaining: FxHashSet<_> = graph.nodes().collect();

    while !remaining.is_empty() {
        // Find modules whose dependencies are all processed
        let ready: Vec<_> = remaining
            .iter()
            .filter(|m| {
                graph.dependencies(*m).all(|dep| processed.contains(&dep))
            })
            .copied()
            .collect();

        if ready.is_empty() {
            // Cycle - handle gracefully
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
```

### Type Check by Level

```rust
/// Type check project with level-based parallelism
pub fn type_check_parallel(db: &dyn Db) -> TypeCheckResult {
    let graph = build_dependency_graph(db);
    let levels = compute_parallel_levels(&graph);

    let mut all_diagnostics = Vec::new();

    for level in levels {
        // All modules in level can be checked in parallel
        let level_results: Vec<_> = level
            .par_iter()
            .map(|module| typed_module(db, *module))
            .collect();

        for result in level_results {
            all_diagnostics.extend(result.diagnostics(db).iter().cloned());
        }
    }

    TypeCheckResult { diagnostics: all_diagnostics }
}
```

---

## Parallel Code Generation

### Function-Level Parallelism

```rust
/// Generate code for all functions in parallel
pub fn codegen_parallel(db: &dyn Db, modules: &[TypedModule]) -> Vec<GeneratedFunction> {
    // Collect all functions
    let functions: Vec<_> = modules
        .iter()
        .flat_map(|m| m.functions(db))
        .collect();

    // Generate in parallel
    functions
        .par_iter()
        .map(|func| generated_function(db, func.func(db)))
        .collect()
}
```

### Template Instantiation

```rust
/// Instantiate pattern templates in parallel
pub fn instantiate_templates_parallel(
    db: &dyn Db,
    usages: &[PatternUsage],
) -> Vec<CCode> {
    let cache = db.pattern_template_cache();

    usages
        .par_iter()
        .map(|usage| {
            let sig = usage.compute_signature(db);
            let template = cache.get_or_compile(&sig, || {
                compile_template(db, &sig)
            });
            template.instantiate(&usage.bindings())
        })
        .collect()
}
```

---

## Thread-Local State

### Per-Thread Arenas

```rust
thread_local! {
    /// Thread-local expression arena for parsing
    static PARSE_ARENA: RefCell<ExprArena> = RefCell::new(ExprArena::new());

    /// Thread-local string buffer for codegen
    static CODEGEN_BUFFER: RefCell<String> = RefCell::new(String::with_capacity(64 * 1024));
}

/// Use thread-local arena for parsing
pub fn parse_with_local_arena(
    tokens: &TokenList,
    interner: &StringInterner,
) -> (Module, ExprArena) {
    PARSE_ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        arena.reset();  // Reuse allocation

        let mut parser = Parser::new(tokens, interner, &mut arena);
        let module = parser.parse();

        // Clone out the arena (small cost vs allocation)
        (module, arena.clone())
    })
}
```

---

## Synchronization Primitives

### Lock-Free Progress Tracking

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

/// Lock-free compilation progress
pub struct Progress {
    files_total: AtomicUsize,
    files_parsed: AtomicUsize,
    functions_typed: AtomicUsize,
    functions_generated: AtomicUsize,
}

impl Progress {
    pub fn new(total_files: usize) -> Self {
        Self {
            files_total: AtomicUsize::new(total_files),
            files_parsed: AtomicUsize::new(0),
            functions_typed: AtomicUsize::new(0),
            functions_generated: AtomicUsize::new(0),
        }
    }

    pub fn file_parsed(&self) {
        self.files_parsed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn function_typed(&self) {
        self.functions_typed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> ProgressSnapshot {
        ProgressSnapshot {
            files_total: self.files_total.load(Ordering::Relaxed),
            files_parsed: self.files_parsed.load(Ordering::Relaxed),
            functions_typed: self.functions_typed.load(Ordering::Relaxed),
            functions_generated: self.functions_generated.load(Ordering::Relaxed),
        }
    }
}
```

### Parking Lot Locks

```rust
use parking_lot::{RwLock, Mutex};

/// Use parking_lot for faster locking
pub struct SharedState {
    /// Read-heavy data uses RwLock
    type_cache: RwLock<TypeCache>,

    /// Write-heavy data uses Mutex
    diagnostics: Mutex<Vec<Diagnostic>>,
}
```

---

## Scaling Expectations

| Cores | Parse | Type Check | Codegen | Overall |
|-------|-------|------------|---------|---------|
| 1 | 1.0x | 1.0x | 1.0x | 1.0x |
| 2 | 1.9x | 1.8x | 1.9x | 1.85x |
| 4 | 3.6x | 3.2x | 3.7x | 3.5x |
| 8 | 6.8x | 5.5x | 7.0x | 6.4x |
| 16 | 12x | 8x | 13x | 11x |

*Type checking scales less due to dependency constraints*

### Bottlenecks

1. **String interning** - Mitigated by sharding
2. **Type interning** - Mitigated by DashMap
3. **Module dependencies** - Limited by graph topology
4. **Global diagnostics** - Low contention, acceptable

---

## Configuration Recommendations

### Small Project (<50 files)

```rust
ThreadPoolConfig {
    max_threads: 4,
    stack_size: 2 * 1024 * 1024,
}
```

### Medium Project (50-500 files)

```rust
ThreadPoolConfig {
    max_threads: num_cpus::get(),
    stack_size: 4 * 1024 * 1024,
}
```

### Large Project (>500 files)

```rust
ThreadPoolConfig {
    max_threads: num_cpus::get(),
    stack_size: 8 * 1024 * 1024,
}
```
