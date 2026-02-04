---
section: "02"
title: Lexer Optimizations
status: not-started
goal: Achieve O(1) keyword lookup and pre-computed operator metadata
sections:
  - id: "02.1"
    title: Perfect Hash Keywords
    status: not-started
  - id: "02.2"
    title: Compile-time Collision Detection
    status: not-started
  - id: "02.3"
    title: Precedence Metadata in Tokens
    status: not-started
  - id: "02.4"
    title: Adjacent Token Optimization
    status: not-started
---

# Section 02: Lexer Optimizations

**Status:** 📋 Planned
**Goal:** O(1) keyword recognition and pre-computed operator metadata
**Source:** Go compiler (`src/cmd/compile/internal/syntax/scanner.go`)

---

## Background

Go's parser achieves exceptional speed partly through **perfect hash keyword lookup**:

```go
// Go: O(1) keyword lookup with zero collisions
func hash(s []byte) uint {
    return (uint(s[0])<<4 ^ uint(s[1]) + uint(len(s))) & 63
}
```

Current Ori likely uses:
- Match expression or hash map for keywords
- Runtime computation of operator precedence

This section optimizes both.

---

## 02.1 Perfect Hash Keywords

**Goal:** Single-instruction keyword recognition

### Tasks

- [ ] Analyze Ori's keyword set
  - [ ] List all keywords: `fn`, `let`, `if`, `match`, `type`, `trait`, etc.
  - [ ] Count total keywords (expected: 25-35)
  - [ ] Determine minimum table size (power of 2, typically 64)

- [ ] Design perfect hash function
  ```rust
  const fn keyword_hash(s: &[u8]) -> usize {
      if s.len() < 2 { return 63; }  // Invalid slot
      ((s[0] as usize) << 4 ^ (s[1] as usize) + s.len()) & 63
  }
  ```

- [ ] Generate keyword table at compile time
  ```rust
  const KEYWORD_TABLE: [Option<TokenKind>; 64] = {
      let mut table = [None; 64];
      table[keyword_hash(b"fn")] = Some(TokenKind::Fn);
      table[keyword_hash(b"let")] = Some(TokenKind::Let);
      table[keyword_hash(b"if")] = Some(TokenKind::If);
      // ... all keywords
      table
  };

  // Verification strings for collision detection
  const KEYWORD_STRINGS: [&str; 64] = { /* parallel array */ };
  ```

- [ ] Implement lookup function
  ```rust
  #[inline]
  pub fn lookup_keyword(ident: &str) -> Option<TokenKind> {
      if ident.len() < 2 { return None; }
      let idx = keyword_hash(ident.as_bytes());
      KEYWORD_TABLE.get(idx)
          .copied()
          .flatten()
          .filter(|_| KEYWORD_STRINGS[idx] == ident)
  }
  ```

- [ ] Benchmark: Compare with current implementation

### Validation

The hash function must produce zero collisions. Add a compile-time check:

```rust
const _: () = {
    // Collision detection at compile time
    let mut seen = [false; 64];
    let keywords = [("fn", 2), ("let", 3), ("if", 2), /* ... */];

    let mut i = 0;
    while i < keywords.len() {
        let (kw, len) = keywords[i];
        let hash = keyword_hash(kw.as_bytes());
        if seen[hash] {
            panic!("Perfect hash collision detected!");
        }
        seen[hash] = true;
        i += 1;
    }
};
```

---

## 02.2 Compile-time Collision Detection

**Goal:** Guarantee hash function correctness at compile time

### Tasks

- [ ] Create `build.rs` or const fn validation
  - [ ] Enumerate all keywords
  - [ ] Compute hashes
  - [ ] Assert no collisions

- [ ] Handle hash function adjustments
  - [ ] If collision found, adjust multiplier/shift
  - [ ] Document the chosen parameters

- [ ] Add test for exhaustive keyword coverage
  ```rust
  #[test]
  fn all_keywords_recognized() {
      let keywords = ["fn", "let", "if", "else", "match", /* ... */];
      for kw in keywords {
          assert!(
              lookup_keyword(kw).is_some(),
              "Keyword not recognized: {}", kw
          );
      }
  }
  ```

- [ ] Add test for non-keyword rejection
  ```rust
  #[test]
  fn non_keywords_rejected() {
      let non_keywords = ["foo", "bar", "Function", "IF", "iff"];
      for nk in non_keywords {
          assert!(
              lookup_keyword(nk).is_none(),
              "False positive: {}", nk
          );
      }
  }
  ```

---

## 02.3 Precedence Metadata in Tokens

**Goal:** Pre-compute operator precedence during lexing

### Tasks

- [ ] Extend token representation with precedence
  ```rust
  pub struct OperatorToken {
      pub kind: TokenKind,
      pub span: Span,
      pub precedence: u8,       // Pre-computed
      pub associativity: Assoc, // Left, Right, None
  }
  ```

- [ ] Create precedence lookup table
  ```rust
  const OPERATOR_INFO: [OperatorInfo; 64] = {
      let mut table = [OperatorInfo::NONE; 64];
      table[TokenKind::Plus as usize] = OperatorInfo { prec: 60, assoc: Assoc::Left };
      table[TokenKind::Star as usize] = OperatorInfo { prec: 70, assoc: Assoc::Left };
      table[TokenKind::EqEq as usize] = OperatorInfo { prec: 30, assoc: Assoc::None };
      // ... all operators
      table
  };
  ```

- [ ] Update lexer to set precedence
  ```rust
  fn scan_operator(&mut self) -> Token {
      let kind = self.scan_operator_kind();
      let info = OPERATOR_INFO[kind as usize];
      Token {
          kind,
          span: self.current_span(),
          precedence: info.prec,
          associativity: info.assoc,
      }
  }
  ```

- [ ] Update parser to use pre-computed precedence
  ```rust
  // Before: lookup in parser
  let prec = get_precedence(self.current_token().kind);

  // After: read from token
  let prec = self.current_token().precedence;
  ```

### Benefits

| Operation | Before | After |
|-----------|--------|-------|
| Precedence lookup | Hash/match | Direct field read |
| Binary expression loop | Multiple lookups | Zero lookups |
| Total for 1000 operators | ~1000 lookups | ~1000 reads |

---

## 02.4 Adjacent Token Optimization

**Goal:** Maintain and enhance existing adjacent token handling for compound operators

### Tasks

- [ ] Review existing `is_shift_right()` and `is_greater_equal()` patterns
  - [ ] Location: `compiler/ori_parse/src/cursor.rs`
  - [ ] Document current implementation

- [ ] Ensure consistent handling of:
  - [ ] `>>` (shift right vs nested generics close)
  - [ ] `>=` (greater-equal vs generic close + assign)
  - [ ] `->` (arrow)
  - [ ] `=>` (fat arrow)
  - [ ] `..` (range)
  - [ ] `::` (path separator)

- [ ] Add context-aware disambiguation
  ```rust
  fn scan_greater(&mut self, context: LexContext) -> Token {
      if self.peek() == '>' {
          if context.in_generic_params {
              // Split: return single '>'
              Token::new(TokenKind::Greater, ...)
          } else {
              // Consume both: return '>>'
              self.advance();
              Token::new(TokenKind::ShiftRight, ...)
          }
      } else if self.peek() == '=' {
          // ...
      }
  }
  ```

- [ ] Document all compound operator handling in one place

---

## 02.5 Completion Checklist

- [ ] Perfect hash function with zero collisions
- [ ] Compile-time collision detection
- [ ] All keywords recognized correctly
- [ ] Precedence computed during lexing
- [ ] No performance regression
- [ ] Adjacent token handling documented

**Exit Criteria:**
- Keyword lookup benchmarks show improvement
- All lexer tests pass
- Parser tests pass (precedence integration)
- No hash collisions (verified at compile time)
