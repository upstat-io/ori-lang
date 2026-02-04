---
section: "05"
title: Incremental Parsing
status: not-started
goal: Enable efficient reparsing for IDE scenarios with 70-90% AST reuse
sections:
  - id: "05.1"
    title: Syntax Cursor with Caching
    status: not-started
  - id: "05.2"
    title: Node Reusability Predicates
    status: not-started
  - id: "05.3"
    title: Change Range Propagation
    status: not-started
  - id: "05.4"
    title: Lazy Token Capture
    status: not-started
---

# Section 05: Incremental Parsing

**Status:** 📋 Planned
**Goal:** 70-90% AST reuse on typical edits for IDE responsiveness
**Source:** TypeScript (`src/compiler/parser.ts`), Rust (`compiler/rustc_parse/`)

---

## Background

Full reparsing is too slow for IDE scenarios:
- Keystroke latency target: <50ms
- Large file (10K lines): ~100ms to parse
- Need: Reuse unchanged portions of AST

TypeScript achieves 70-90% reuse through:
1. **Syntax cursor** — Navigate old AST by position
2. **Reusability predicates** — Determine which nodes can be reused
3. **Position caching** — Optimize sequential access patterns

Ori already has incremental infrastructure in `incremental.rs` — this section integrates and enhances it.

---

## 05.1 Syntax Cursor with Caching

**Goal:** Efficient old-AST navigation with sequential access optimization

### Tasks

- [ ] Review existing `IncrementalState` in `incremental.rs`
  - [ ] Current implementation status
  - [ ] Missing features

- [ ] Design `SyntaxCursor` struct
  ```rust
  pub struct SyntaxCursor<'a> {
      /// Reference to old AST declarations
      decls: &'a [DeclRef],

      /// Current position in declarations
      current_idx: usize,

      /// Cache: last queried position (most queries are sequential)
      last_queried_pos: u32,

      /// Statistics for debugging/tuning
      stats: CursorStats,
  }

  #[derive(Default)]
  pub struct CursorStats {
      pub cache_hits: u32,
      pub sequential_hits: u32,
      pub binary_searches: u32,
  }
  ```

- [ ] Implement position-based lookup with caching
  ```rust
  impl<'a> SyntaxCursor<'a> {
      /// Get node at position, optimized for sequential access
      pub fn node_at(&mut self, position: u32) -> Option<&DeclRef> {
          // Cache hit: same position as last query
          if position == self.last_queried_pos {
              self.stats.cache_hits += 1;
              return self.decls.get(self.current_idx);
          }

          // Sequential hit: next declaration in order
          if let Some(next) = self.decls.get(self.current_idx + 1) {
              if next.span.start == position {
                  self.stats.sequential_hits += 1;
                  self.current_idx += 1;
                  self.last_queried_pos = position;
                  return Some(next);
              }
          }

          // Fallback: binary search
          self.stats.binary_searches += 1;
          self.search_for_position(position)
      }

      fn search_for_position(&mut self, position: u32) -> Option<&DeclRef> {
          let idx = self.decls.binary_search_by(|d| {
              d.span.start.cmp(&position)
          }).ok()?;

          self.current_idx = idx;
          self.last_queried_pos = position;
          self.decls.get(idx)
      }
  }
  ```

- [ ] Add performance logging
  ```rust
  impl Drop for SyntaxCursor<'_> {
      fn drop(&mut self) {
          if self.stats.total_queries() > 100 {
              log::debug!(
                  "SyntaxCursor stats: {} cache hits, {} sequential, {} binary",
                  self.stats.cache_hits,
                  self.stats.sequential_hits,
                  self.stats.binary_searches,
              );
          }
      }
  }
  ```

---

## 05.2 Node Reusability Predicates

**Goal:** Determine which AST nodes can be safely reused

### Tasks

- [ ] Define reusability criteria
  ```rust
  pub struct ReusabilityChecker<'a> {
      change_range: TextChangeRange,
      old_source: &'a str,
      new_source: &'a str,
  }

  impl<'a> ReusabilityChecker<'a> {
      /// Check if a node can be reused
      pub fn can_reuse(&self, node: &DeclRef) -> bool {
          // Node must be outside change region
          if node.span.overlaps(&self.change_range.span) {
              return false;
          }

          // Node must be after change region AND position adjustable
          // OR before change region (no adjustment needed)
          true
      }
  }
  ```

- [ ] Implement per-node-kind predicates
  ```rust
  impl ReusabilityChecker<'_> {
      pub fn can_reuse_node(&self, kind: DeclKind, node: &DeclRef) -> bool {
          // Base check
          if !self.can_reuse(node) {
              return false;
          }

          // Kind-specific checks
          match kind {
              // Functions: safe if body unchanged
              DeclKind::Function => true,

              // Imports: safe (no internal references)
              DeclKind::Import => true,

              // Types: safe if definition unchanged
              DeclKind::Type => true,

              // Tests: check target reference still valid
              DeclKind::Test => self.check_test_target_valid(node),

              // Impls: check trait/type references
              DeclKind::Impl => true,

              _ => true,
          }
      }
  }
  ```

- [ ] Add content hashing for verification
  ```rust
  impl DeclRef {
      /// Compute hash of declaration content for change detection
      pub fn content_hash(&self, source: &str) -> u64 {
          use std::hash::{Hash, Hasher};
          use std::collections::hash_map::DefaultHasher;

          let content = &source[self.span.start as usize..self.span.end as usize];
          let mut hasher = DefaultHasher::new();
          content.hash(&mut hasher);
          hasher.finish()
      }
  }

  impl ReusabilityChecker<'_> {
      pub fn can_reuse_with_hash(&self, node: &DeclRef, old_hash: u64) -> bool {
          if !self.can_reuse(node) {
              return false;
          }

          // Verify content unchanged (belt and suspenders)
          let new_hash = node.content_hash(self.new_source);
          old_hash == new_hash
      }
  }
  ```

---

## 05.3 Change Range Propagation

**Goal:** Adjust spans of reused nodes for new positions

### Tasks

- [ ] Review existing `ChangeMarker` in `incremental.rs`

- [ ] Implement span adjustment
  ```rust
  pub struct SpanAdjuster {
      /// Start of change in old text
      change_start: u32,
      /// Length of removed text
      old_length: u32,
      /// Length of inserted text
      new_length: u32,
  }

  impl SpanAdjuster {
      pub fn new(change: &TextChangeRange) -> Self {
          Self {
              change_start: change.span.start,
              old_length: change.span.length(),
              new_length: change.new_length,
          }
      }

      /// Adjust a span from old to new positions
      pub fn adjust(&self, span: Span) -> Span {
          if span.end <= self.change_start {
              // Before change: no adjustment
              span
          } else if span.start >= self.change_start + self.old_length {
              // After change: shift by delta
              let delta = self.new_length as i64 - self.old_length as i64;
              Span {
                  start: (span.start as i64 + delta) as u32,
                  end: (span.end as i64 + delta) as u32,
              }
          } else {
              // Overlaps change: cannot reuse (should be filtered earlier)
              panic!("Cannot adjust span that overlaps change");
          }
      }
  }
  ```

- [ ] Implement recursive span adjustment for AST
  ```rust
  impl AstStorage {
      /// Adjust all spans in reused subtree
      pub fn adjust_spans(&mut self, root: NodeIdx, adjuster: &SpanAdjuster) {
          // Adjust root node's main token
          let token_idx = self.main_tokens[root.0 as usize];
          self.token_spans[token_idx.0 as usize] =
              adjuster.adjust(self.token_spans[token_idx.0 as usize]);

          // Recursively adjust children
          self.for_each_child(root, |child| {
              self.adjust_spans(child, adjuster);
          });
      }
  }
  ```

- [ ] Integrate with incremental parse
  ```rust
  pub fn incremental_parse(
      old_ast: &ParsedModule,
      new_source: &str,
      change: TextChangeRange,
  ) -> ParsedModule {
      let cursor = SyntaxCursor::new(&old_ast.decls);
      let checker = ReusabilityChecker::new(&change, old_ast.source, new_source);
      let adjuster = SpanAdjuster::new(&change);

      let mut new_decls = Vec::new();
      let mut reused_count = 0;

      for old_decl in &old_ast.decls {
          if checker.can_reuse(old_decl) {
              // Reuse with adjusted spans
              let mut decl = old_decl.clone();
              decl.span = adjuster.adjust(decl.span);
              new_decls.push(decl);
              reused_count += 1;
          } else {
              // Reparse this region
              let reparsed = parse_decl_at(new_source, old_decl.span.start);
              new_decls.push(reparsed);
          }
      }

      log::debug!(
          "Incremental parse: {}/{} decls reused ({}%)",
          reused_count,
          old_ast.decls.len(),
          reused_count * 100 / old_ast.decls.len(),
      );

      ParsedModule { decls: new_decls, /* ... */ }
  }
  ```

---

## 05.4 Lazy Token Capture

**Goal:** Defer token stream reconstruction for attributes/patterns

### Tasks

- [ ] Design `LazyTokens` type
  ```rust
  pub enum LazyTokens {
      /// No tokens needed (fast path)
      None,

      /// Tokens computed immediately (when definitely needed)
      Eager(TokenStream),

      /// Defer computation until accessed
      Lazy {
          start_pos: u32,
          end_pos: u32,
          source_hash: u64,  // For validation
      },
  }
  ```

- [ ] Implement lazy reconstruction
  ```rust
  impl LazyTokens {
      pub fn to_tokens(&self, source: &str, lexer: &Lexer) -> TokenStream {
          match self {
              Self::None => TokenStream::empty(),
              Self::Eager(tokens) => tokens.clone(),
              Self::Lazy { start_pos, end_pos, source_hash } => {
                  // Validate source unchanged
                  debug_assert_eq!(
                      *source_hash,
                      hash(&source[*start_pos as usize..*end_pos as usize])
                  );

                  // Reconstruct tokens
                  lexer.tokenize_range(source, *start_pos, *end_pos)
              }
          }
      }
  }
  ```

- [ ] Add to AST nodes that need tokens
  ```rust
  pub struct FunctionDef {
      pub name: Name,
      pub params: Vec<Param>,
      pub body: NodeIdx,

      /// Tokens for attribute processing (lazy)
      pub tokens: LazyTokens,
  }
  ```

- [ ] Implement capture during parsing
  ```rust
  impl Parser<'_> {
      fn capture_tokens_if_needed<T, F>(
          &mut self,
          needs_tokens: bool,
          parser: F,
      ) -> (T, LazyTokens)
      where
          F: FnOnce(&mut Self) -> T,
      {
          if !needs_tokens {
              return (parser(self), LazyTokens::None);
          }

          let start_pos = self.current_position();
          let result = parser(self);
          let end_pos = self.current_position();

          let tokens = LazyTokens::Lazy {
              start_pos,
              end_pos,
              source_hash: self.hash_range(start_pos, end_pos),
          };

          (result, tokens)
      }
  }
  ```

---

## 05.5 Completion Checklist

- [ ] `SyntaxCursor` with caching implemented
- [ ] Reusability predicates for all node kinds
- [ ] Span adjustment working correctly
- [ ] Lazy token capture implemented
- [ ] Integration tests for incremental parsing
- [ ] Performance benchmarks showing 70%+ reuse

**Exit Criteria:**
- Typical edits reuse 70-90% of AST
- Incremental parse < 20ms for common edits
- No correctness regressions
- LSP integration working with incremental parsing
