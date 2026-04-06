//! Operator and punctuation scanning.
//!
//! Each function handles one or more related ASCII operator characters,
//! consuming lookahead bytes to disambiguate multi-character operators
//! (e.g., `=` vs `==` vs `=>`).

use crate::tag::{RawTag, RawToken};

impl super::RawScanner<'_> {
    /// Single-byte token: advance one byte and emit the given tag.
    pub(super) fn single(&mut self, start: u32, tag: RawTag) -> RawToken {
        self.cursor.advance();
        RawToken {
            tag,
            len: self.cursor.pos() - start,
        }
    }

    /// Advance past an operator char; if `=` follows, consume it and emit
    /// the compound tag, otherwise emit the single-char tag.
    #[inline]
    fn compound_eq(&mut self, start: u32, single: RawTag, compound: RawTag) -> RawToken {
        self.cursor.advance();
        if self.cursor.current() == b'=' {
            self.cursor.advance();
            RawToken {
                tag: compound,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: single,
                len: self.cursor.pos() - start,
            }
        }
    }

    pub(super) fn plus(&mut self, start: u32) -> RawToken {
        self.compound_eq(start, RawTag::Plus, RawTag::PlusEq)
    }

    pub(super) fn minus_or_arrow(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '-'
        match self.cursor.current() {
            b'>' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::Arrow,
                    len: self.cursor.pos() - start,
                }
            }
            b'=' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::MinusEq,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: RawTag::Minus,
                len: self.cursor.pos() - start,
            },
        }
    }

    pub(super) fn star(&mut self, start: u32) -> RawToken {
        self.compound_eq(start, RawTag::Star, RawTag::StarEq)
    }

    pub(super) fn percent(&mut self, start: u32) -> RawToken {
        self.compound_eq(start, RawTag::Percent, RawTag::PercentEq)
    }

    pub(super) fn caret(&mut self, start: u32) -> RawToken {
        self.compound_eq(start, RawTag::Caret, RawTag::CaretEq)
    }

    pub(super) fn at(&mut self, start: u32) -> RawToken {
        self.compound_eq(start, RawTag::At, RawTag::AtEq)
    }

    pub(super) fn equal(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '='
        match self.cursor.current() {
            b'=' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::EqualEqual,
                    len: self.cursor.pos() - start,
                }
            }
            b'>' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::FatArrow,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: RawTag::Equal,
                len: self.cursor.pos() - start,
            },
        }
    }

    pub(super) fn bang(&mut self, start: u32) -> RawToken {
        self.compound_eq(start, RawTag::Bang, RawTag::BangEqual)
    }

    pub(super) fn less(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '<'
        match self.cursor.current() {
            b'=' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::LessEqual,
                    len: self.cursor.pos() - start,
                }
            }
            b'<' => {
                self.cursor.advance();
                // Check for <<= (shift-left-assign)
                if self.cursor.current() == b'=' {
                    self.cursor.advance();
                    RawToken {
                        tag: RawTag::ShlEq,
                        len: self.cursor.pos() - start,
                    }
                } else {
                    RawToken {
                        tag: RawTag::Shl,
                        len: self.cursor.pos() - start,
                    }
                }
            }
            _ => RawToken {
                tag: RawTag::Less,
                len: self.cursor.pos() - start,
            },
        }
    }

    pub(super) fn dot(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '.'
        if self.cursor.current() == b'.' {
            self.cursor.advance(); // consume second '.'
            if self.cursor.current() == b'=' {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::DotDotEqual,
                    len: self.cursor.pos() - start,
                }
            } else if self.cursor.current() == b'.' {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::DotDotDot,
                    len: self.cursor.pos() - start,
                }
            } else {
                RawToken {
                    tag: RawTag::DotDot,
                    len: self.cursor.pos() - start,
                }
            }
        } else {
            RawToken {
                tag: RawTag::Dot,
                len: self.cursor.pos() - start,
            }
        }
    }

    pub(super) fn question(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '?'
        if self.cursor.current() == b'?' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::QuestionQuestion,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Question,
                len: self.cursor.pos() - start,
            }
        }
    }

    pub(super) fn pipe(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '|'
        match self.cursor.current() {
            b'|' => {
                self.cursor.advance();
                // Check for ||=
                if self.cursor.current() == b'=' {
                    self.cursor.advance();
                    RawToken {
                        tag: RawTag::PipePipeEq,
                        len: self.cursor.pos() - start,
                    }
                } else {
                    RawToken {
                        tag: RawTag::PipePipe,
                        len: self.cursor.pos() - start,
                    }
                }
            }
            b'=' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::PipeEq,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: RawTag::Pipe,
                len: self.cursor.pos() - start,
            },
        }
    }

    pub(super) fn ampersand(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '&'
        match self.cursor.current() {
            b'&' => {
                self.cursor.advance();
                // Check for &&=
                if self.cursor.current() == b'=' {
                    self.cursor.advance();
                    RawToken {
                        tag: RawTag::AmpersandAmpersandEq,
                        len: self.cursor.pos() - start,
                    }
                } else {
                    RawToken {
                        tag: RawTag::AmpersandAmpersand,
                        len: self.cursor.pos() - start,
                    }
                }
            }
            b'=' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::AmpersandEq,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: RawTag::Ampersand,
                len: self.cursor.pos() - start,
            },
        }
    }

    pub(super) fn colon(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume ':'

        // Inside template interpolation at top-level → format spec separator
        if let Some(depth) = self.template_depth.last() {
            if depth.is_top_level() {
                return self.format_spec(start);
            }
        }

        if self.cursor.current() == b':' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::ColonColon,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Colon,
                len: self.cursor.pos() - start,
            }
        }
    }

    /// Scan a format spec after `:` in a template interpolation.
    ///
    /// Consumes everything between `:` (already consumed) and `}` (not consumed).
    /// The `}` will be handled by the normal `right_brace` → `template_middle_or_tail`
    /// path on the next call to `next_token()`.
    fn format_spec(&mut self, start: u32) -> RawToken {
        // Scan forward until `}` at brace depth 0.
        // Track nested `{}`  in the spec (unlikely but safe).
        let mut brace_depth: u32 = 0;
        loop {
            match self.cursor.current() {
                b'}' if brace_depth == 0 => {
                    // Don't consume the `}` — it triggers template_middle_or_tail
                    return RawToken {
                        tag: RawTag::FormatSpec,
                        len: self.cursor.pos() - start,
                    };
                }
                b'}' => {
                    brace_depth -= 1;
                    self.cursor.advance();
                }
                b'{' => {
                    brace_depth += 1;
                    self.cursor.advance();
                }
                0 if self.cursor.is_eof() => {
                    // Unterminated — return what we have
                    return RawToken {
                        tag: RawTag::FormatSpec,
                        len: self.cursor.pos() - start,
                    };
                }
                _ => {
                    self.cursor.advance();
                }
            }
        }
    }

    pub(super) fn hash(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '#'
        match self.cursor.current() {
            b'[' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::HashBracket,
                    len: self.cursor.pos() - start,
                }
            }
            b'!' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::HashBang,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: RawTag::Hash,
                len: self.cursor.pos() - start,
            },
        }
    }
}
