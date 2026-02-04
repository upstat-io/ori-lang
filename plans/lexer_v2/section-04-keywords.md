---
section: "04"
title: Keyword & Operator Handling
status: not-started
goal: Optimal keyword recognition and operator metadata
sections:
  - id: "04.1"
    title: Perfect Hash Keywords
    status: not-started
  - id: "04.2"
    title: Compile-time Collision Detection
    status: not-started
  - id: "04.3"
    title: Operator Precedence Table
    status: not-started
  - id: "04.4"
    title: Context-Sensitive Keywords
    status: not-started
  - id: "04.5"
    title: Token Gluing/Breaking
    status: not-started
---

# Section 04: Keyword & Operator Handling

**Status:** 📋 Planned
**Goal:** Optimal keyword recognition and operator metadata
**Source:** Go (`cmd/compile/internal/syntax/scanner.go`), Rust (`rustc_parse`)

---

## Background

### Go's Perfect Hash (O(1) Keyword Lookup)

```go
func hash(s []byte) uint {
    return (uint(s[0])<<4 ^ uint(s[1]) + uint(len(s))) & 63
}

var keywordMap [1 << 6]token // 64 slots

func init() {
    for tok := _Break; tok <= _Var; tok++ {
        h := hash([]byte(tok.String()))
        if keywordMap[h] != 0 {
            panic("imperfect hash")
        }
        keywordMap[h] = tok
    }
}
```

### Rust's Token Gluing

Rust lexes `>` and `>` separately, then glues them into `>>` in the parser. This allows `Vec<Vec<i32>>` to work correctly.

---

## 04.1 Perfect Hash Keywords

**Goal:** O(1) keyword lookup with zero runtime overhead

### Tasks

- [ ] Enumerate all Ori keywords
  ```rust
  const KEYWORDS: &[(&str, Tag)] = &[
      // Core keywords
      ("let", Tag::KwLet),
      ("fn", Tag::KwFn),
      ("if", Tag::KwIf),
      ("else", Tag::KwElse),
      ("match", Tag::KwMatch),
      ("for", Tag::KwFor),
      ("while", Tag::KwWhile),
      ("loop", Tag::KwLoop),
      ("break", Tag::KwBreak),
      ("continue", Tag::KwContinue),

      // Type keywords
      ("type", Tag::KwType),
      ("trait", Tag::KwTrait),
      ("impl", Tag::KwImpl),
      ("where", Tag::KwWhere),

      // Module keywords
      ("use", Tag::KwUse),
      ("mod", Tag::KwMod),
      ("pub", Tag::KwPub),

      // Value keywords
      ("true", Tag::KwTrue),
      ("false", Tag::KwFalse),
      ("nil", Tag::KwNil),
      ("self", Tag::KwSelf_),

      // Logical keywords
      ("and", Tag::KwAnd),
      ("or", Tag::KwOr),
      ("not", Tag::KwNot),
      ("in", Tag::KwIn),
      ("as", Tag::KwAs),

      // Verification keywords
      ("tests", Tag::KwTests),
      ("pre_check", Tag::KwPrecondition),
      ("post_check", Tag::KwPostcondition),

      // Effect keywords
      ("uses", Tag::KwUses),
      ("with", Tag::KwWith),

      // Async keywords
      ("async", Tag::KwAsync),
      ("await", Tag::KwAwait),

      // Mutability
      ("mut", Tag::KwMut),
  ];
  ```

- [ ] Design perfect hash function
  ```rust
  /// Perfect hash for Ori keywords
  /// Formula: ((first << 4) ^ second + len) & (TABLE_SIZE - 1)
  const fn keyword_hash(s: &[u8]) -> usize {
      if s.len() < 2 {
          return TABLE_SIZE; // Invalid slot
      }
      let first = s[0] as usize;
      let second = s[1] as usize;
      ((first << 4) ^ second + s.len()) & (TABLE_SIZE - 1)
  }

  const TABLE_SIZE: usize = 64; // Power of 2, fits all keywords
  ```

- [ ] Generate lookup table at compile time
  ```rust
  /// Keyword lookup table - generated at compile time
  const KEYWORD_TABLE: [Option<Tag>; TABLE_SIZE] = {
      let mut table = [None; TABLE_SIZE];
      let mut i = 0;
      while i < KEYWORDS.len() {
          let (kw, tag) = KEYWORDS[i];
          let hash = keyword_hash(kw.as_bytes());
          // Note: collision detection happens separately
          table[hash] = Some(tag);
          i += 1;
      }
      table
  };

  /// Keyword strings for verification (parallel to KEYWORD_TABLE)
  const KEYWORD_STRINGS: [&str; TABLE_SIZE] = {
      let mut strings = [""; TABLE_SIZE];
      let mut i = 0;
      while i < KEYWORDS.len() {
          let (kw, _) = KEYWORDS[i];
          let hash = keyword_hash(kw.as_bytes());
          strings[hash] = kw;
          i += 1;
      }
      strings
  };
  ```

- [ ] Implement lookup function
  ```rust
  /// Look up a keyword by identifier text
  #[inline]
  pub fn keyword_lookup(ident: &[u8]) -> Option<Tag> {
      if ident.len() < 2 || ident.len() > 12 {
          return None; // No keywords shorter than 2 or longer than 12
      }

      let hash = keyword_hash(ident);
      if hash >= TABLE_SIZE {
          return None;
      }

      KEYWORD_TABLE[hash].filter(|_| {
          // Verify it's the right keyword (handles hash collisions)
          KEYWORD_STRINGS[hash].as_bytes() == ident
      })
  }
  ```

---

## 04.2 Compile-time Collision Detection

**Goal:** Guarantee hash function correctness at compile time

### Tasks

- [ ] Add compile-time collision check
  ```rust
  const _COLLISION_CHECK: () = {
      let mut seen = [false; TABLE_SIZE];
      let mut i = 0;
      while i < KEYWORDS.len() {
          let (kw, _) = KEYWORDS[i];
          let hash = keyword_hash(kw.as_bytes());

          if hash >= TABLE_SIZE {
              panic!("Hash out of bounds");
          }
          if seen[hash] {
              panic!("Perfect hash collision detected!");
          }
          seen[hash] = true;
          i += 1;
      }
  };
  ```

- [ ] Add runtime tests for completeness
  ```rust
  #[test]
  fn all_keywords_recognized() {
      for (kw, expected_tag) in KEYWORDS {
          let result = keyword_lookup(kw.as_bytes());
          assert_eq!(
              result, Some(*expected_tag),
              "Keyword '{}' not recognized", kw
          );
      }
  }

  #[test]
  fn non_keywords_not_recognized() {
      let non_keywords = [
          "foo", "bar", "baz",         // Common identifiers
          "Let", "FN", "IF",           // Wrong case
          "lets", "fns", "iff",        // Similar but not keywords
          "a", "x", "ab",              // Short identifiers
          "verylongidentifier",        // Long identifier
      ];

      for nk in non_keywords {
          assert!(
              keyword_lookup(nk.as_bytes()).is_none(),
              "False positive: '{}' recognized as keyword", nk
          );
      }
  }
  ```

- [ ] Document hash function parameters
  ```rust
  /// # Perfect Hash Function Design
  ///
  /// The hash function `((first << 4) ^ second + len) & 63` was chosen because:
  ///
  /// 1. **Uses first two characters**: Keywords have diverse first chars
  /// 2. **Incorporates length**: Disambiguates same-prefix keywords
  /// 3. **64 slots**: Sufficient for ~40 keywords with no collisions
  /// 4. **Power-of-2 mask**: Fast modulo via bitwise AND
  ///
  /// If adding new keywords causes collision, try:
  /// - Different shift amount (try 3, 5, 6)
  /// - Different character positions (try 0+2 instead of 0+1)
  /// - Larger table size (128 slots)
  ```

---

## 04.3 Operator Precedence Table

**Goal:** Pre-computed operator metadata for parser

### Tasks

- [ ] Define operator info structure
  ```rust
  /// Operator metadata
  #[derive(Clone, Copy, Debug)]
  pub struct OperatorInfo {
      pub precedence: u8,
      pub associativity: Associativity,
      pub is_comparison: bool,
      pub is_assignment: bool,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum Associativity {
      Left,
      Right,
      None, // Non-associative (comparisons)
  }

  impl OperatorInfo {
      pub const NONE: Self = Self {
          precedence: 0,
          associativity: Associativity::Left,
          is_comparison: false,
          is_assignment: false,
      };
  }
  ```

- [ ] Create precedence table
  ```rust
  /// Operator precedence table indexed by Tag
  /// Precedence levels (higher binds tighter):
  ///   1: Assignment (=, +=, etc.)
  ///   2: Or (or)
  ///   3: And (and)
  ///   4: Not (not) - prefix
  ///   5: Comparison (==, !=, <, >, <=, >=)
  ///   6: Bitwise Or (|)
  ///   7: Bitwise Xor (^)
  ///   8: Bitwise And (&)
  ///   9: Shift (<<, >>)
  ///  10: Range (.., ..=)
  ///  11: Addition (+, -)
  ///  12: Multiplication (*, /, %)
  ///  13: Unary (-, !, ~) - prefix
  ///  14: Exponentiation (**)
  ///  15: Call, Index, Field access

  const OPERATOR_TABLE: [OperatorInfo; 256] = {
      let mut table = [OperatorInfo::NONE; 256];

      // Assignment (level 1, right associative)
      table[Tag::Eq as usize] = OperatorInfo {
          precedence: 1,
          associativity: Associativity::Right,
          is_comparison: false,
          is_assignment: true,
      };
      table[Tag::PlusEq as usize] = OperatorInfo {
          precedence: 1,
          associativity: Associativity::Right,
          is_comparison: false,
          is_assignment: true,
      };
      // ... other compound assignments

      // Logical Or (level 2, left associative)
      table[Tag::KwOr as usize] = OperatorInfo {
          precedence: 2,
          associativity: Associativity::Left,
          is_comparison: false,
          is_assignment: false,
      };

      // Logical And (level 3, left associative)
      table[Tag::KwAnd as usize] = OperatorInfo {
          precedence: 3,
          associativity: Associativity::Left,
          is_comparison: false,
          is_assignment: false,
      };

      // Comparison (level 5, non-associative)
      table[Tag::EqEq as usize] = OperatorInfo {
          precedence: 5,
          associativity: Associativity::None,
          is_comparison: true,
          is_assignment: false,
      };
      table[Tag::BangEq as usize] = OperatorInfo {
          precedence: 5,
          associativity: Associativity::None,
          is_comparison: true,
          is_assignment: false,
      };
      table[Tag::Lt as usize] = OperatorInfo {
          precedence: 5,
          associativity: Associativity::None,
          is_comparison: true,
          is_assignment: false,
      };
      // ... other comparisons

      // Addition (level 11, left associative)
      table[Tag::Plus as usize] = OperatorInfo {
          precedence: 11,
          associativity: Associativity::Left,
          is_comparison: false,
          is_assignment: false,
      };
      table[Tag::Minus as usize] = OperatorInfo {
          precedence: 11,
          associativity: Associativity::Left,
          is_comparison: false,
          is_assignment: false,
      };

      // Multiplication (level 12, left associative)
      table[Tag::Star as usize] = OperatorInfo {
          precedence: 12,
          associativity: Associativity::Left,
          is_comparison: false,
          is_assignment: false,
      };
      table[Tag::Slash as usize] = OperatorInfo {
          precedence: 12,
          associativity: Associativity::Left,
          is_comparison: false,
          is_assignment: false,
      };
      table[Tag::Percent as usize] = OperatorInfo {
          precedence: 12,
          associativity: Associativity::Left,
          is_comparison: false,
          is_assignment: false,
      };

      table
  };

  impl Tag {
      /// Get operator info for this tag
      #[inline]
      pub fn operator_info(self) -> OperatorInfo {
          OPERATOR_TABLE[self as usize]
      }

      /// Get precedence (0 if not an operator)
      #[inline]
      pub fn precedence(self) -> u8 {
          self.operator_info().precedence
      }

      /// Is this tag an operator?
      #[inline]
      pub fn is_operator(self) -> bool {
          self.operator_info().precedence > 0
      }
  }
  ```

---

## 04.4 Context-Sensitive Keywords

**Goal:** Handle keywords that are only keywords in certain contexts

### Tasks

- [ ] Identify context-sensitive keywords
  ```rust
  /// Keywords that are only keywords when followed by specific tokens
  /// These are treated as identifiers by default
  const CONTEXTUAL_KEYWORDS: &[(&str, Tag, ContextRequirement)] = &[
      // Pattern keywords - only keyword when followed by `(`
      ("timeout", Tag::KwTimeout, ContextRequirement::FollowedByParen),
      ("parallel", Tag::KwParallel, ContextRequirement::FollowedByParen),
      ("cache", Tag::KwCache, ContextRequirement::FollowedByParen),
      ("spawn", Tag::KwSpawn, ContextRequirement::FollowedByParen),
      ("recurse", Tag::KwRecurse, ContextRequirement::FollowedByParen),
  ];

  enum ContextRequirement {
      Always,           // Always a keyword
      FollowedByParen,  // Only keyword when next token is (
      InPattern,        // Only keyword in pattern context
  }
  ```

- [ ] Implement context-sensitive lookup
  ```rust
  /// Look up keyword with context awareness
  pub fn keyword_lookup_contextual(
      ident: &[u8],
      next_char: Option<u8>,
  ) -> Option<Tag> {
      // First try normal keyword lookup
      if let Some(tag) = keyword_lookup(ident) {
          return Some(tag);
      }

      // Then try contextual keywords
      for (kw, tag, req) in CONTEXTUAL_KEYWORDS {
          if kw.as_bytes() == ident {
              match req {
                  ContextRequirement::Always => return Some(*tag),
                  ContextRequirement::FollowedByParen => {
                      if next_char == Some(b'(') {
                          return Some(*tag);
                      }
                  }
                  ContextRequirement::InPattern => {
                      // Parser must handle this
                      return None;
                  }
              }
          }
      }

      None
  }
  ```

- [ ] Document behavior
  ```rust
  /// # Context-Sensitive Keywords
  ///
  /// Some identifiers are only keywords in specific contexts:
  ///
  /// ```ori
  /// let timeout = 100          // `timeout` is identifier
  /// timeout(100) { ... }       // `timeout` is keyword
  ///
  /// let cache = HashMap::new() // `cache` is identifier
  /// cache(key) { ... }         // `cache` is keyword
  /// ```
  ///
  /// The lexer uses 1-char lookahead to determine if `(` follows.
  ```

---

## 04.5 Token Gluing/Breaking

**Goal:** Handle compound operators that conflict with generics

### Tasks

- [ ] Implement token gluing in parser (Rust pattern)
  ```rust
  /// The parser can glue adjacent tokens into compound operators
  impl Cursor<'_> {
      /// Check if current and next tokens form `>>`
      pub fn is_shift_right(&self) -> bool {
          self.check(Tag::Gt)
              && self.peek_tag() == Tag::Gt
              && self.are_adjacent()
      }

      /// Check if current and next tokens form `>=`
      pub fn is_greater_equal(&self) -> bool {
          self.check(Tag::Gt)
              && self.peek_tag() == Tag::Eq
              && self.are_adjacent()
      }

      /// Consume a glued `>>` operator
      pub fn consume_shift_right(&mut self) -> Span {
          assert!(self.is_shift_right());
          let start = self.current_span().start;
          self.advance(); // First >
          let end = self.current_span().end;
          self.advance(); // Second >
          Span::new(start, end)
      }
  }
  ```

- [ ] Document generic closing scenarios
  ```rust
  /// # Token Gluing for Generics
  ///
  /// When parsing generics like `Result<Result<T, E>, E>`, the closing
  /// `>>` must be treated as two separate `>` tokens, not as shift-right.
  ///
  /// The lexer always produces separate `>` tokens. The parser glues them
  /// into `>>` only in expression contexts, not in type contexts.
  ///
  /// ```ori
  /// let x = a >> b           // Parsed as shift-right
  /// let y: Result<T, E>      // Parsed as generic close
  /// let z: Vec<Vec<i32>>     // Parsed as two generic closes
  /// ```
  ```

- [ ] Add tests for edge cases
  ```rust
  #[test]
  fn shift_right_vs_generic_close() {
      // Expression context: >> is shift
      let tokens = tokenize("a >> b");
      // Parser should glue these

      // Type context: >> is two closes
      let tokens = tokenize("Vec<Vec<i32>>");
      // Parser should NOT glue these
  }

  #[test]
  fn greater_equal_vs_generic() {
      // Expression: >= is comparison
      let tokens = tokenize("a >= b");

      // Type with default: > = is two tokens
      let tokens = tokenize("Option<T = Default>");
  }
  ```

---

## 04.6 Completion Checklist

- [ ] Perfect hash function implemented
- [ ] Compile-time collision detection
- [ ] All keywords recognized
- [ ] Operator precedence table complete
- [ ] Context-sensitive keywords handled
- [ ] Token gluing documented and tested
- [ ] Parser integration complete

**Exit Criteria:**
- O(1) keyword lookup verified
- All operators have correct precedence
- Generic syntax works correctly
- No regression in existing tests
