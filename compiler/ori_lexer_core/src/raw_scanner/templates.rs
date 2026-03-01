//! Template literal scanning.
//!
//! Template literals use backtick delimiters (`` ` ``) and support
//! interpolation via `{expr}`. Scanning uses SIMD-accelerated
//! `skip_to_template_delim()` to skip past ordinary template content.
//!
//! Token sequence for `` `hello {name}!` ``:
//! `TemplateHead` → (expression tokens) → `TemplateTail`

use crate::tag::{RawTag, RawToken};

use super::InterpolationDepth;

impl super::RawScanner<'_> {
    pub(super) fn template_literal(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume opening '`'
        loop {
            // SIMD-accelerated skip past ordinary template content
            let b = self.cursor.skip_to_template_delim();
            match b {
                b'`' => {
                    self.cursor.advance();
                    return RawToken {
                        tag: RawTag::TemplateComplete,
                        len: self.cursor.pos() - start,
                    };
                }
                b'{' => {
                    if self.cursor.peek() == b'{' {
                        // Escaped brace `{{`
                        self.cursor.advance();
                        self.cursor.advance();
                        continue;
                    }
                    self.cursor.advance(); // consume '{'
                    self.template_depth.push(InterpolationDepth::default());
                    return RawToken {
                        tag: RawTag::TemplateHead,
                        len: self.cursor.pos() - start,
                    };
                }
                b'}' => {
                    if self.cursor.peek() == b'}' {
                        // Escaped brace `}}`
                        self.cursor.advance();
                        self.cursor.advance();
                        continue;
                    }
                    // Lone `}` in template text — consume it
                    self.cursor.advance();
                }
                b'\\' => {
                    self.cursor.advance(); // consume '\'
                    if self.cursor.current() != 0 || !self.cursor.is_eof() {
                        self.cursor.advance(); // skip escaped char
                    }
                }
                b'\n' | b'\r' => {
                    // Templates can span multiple lines
                    self.cursor.advance();
                }
                0 => {
                    if self.cursor.is_eof() {
                        return RawToken {
                            tag: RawTag::UnterminatedTemplate,
                            len: self.cursor.pos() - start,
                        };
                    }
                    self.cursor.advance(); // interior null
                }
                _ => unreachable!("skip_to_template_delim returned unexpected byte"),
            }
        }
    }

    pub(super) fn template_middle_or_tail(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume closing '}'
        loop {
            // SIMD-accelerated skip past ordinary template content
            let b = self.cursor.skip_to_template_delim();
            match b {
                b'`' => {
                    self.cursor.advance();
                    return RawToken {
                        tag: RawTag::TemplateTail,
                        len: self.cursor.pos() - start,
                    };
                }
                b'{' => {
                    if self.cursor.peek() == b'{' {
                        self.cursor.advance();
                        self.cursor.advance();
                        continue;
                    }
                    self.cursor.advance(); // consume '{'
                    self.template_depth.push(InterpolationDepth::default());
                    return RawToken {
                        tag: RawTag::TemplateMiddle,
                        len: self.cursor.pos() - start,
                    };
                }
                b'}' => {
                    if self.cursor.peek() == b'}' {
                        self.cursor.advance();
                        self.cursor.advance();
                        continue;
                    }
                    self.cursor.advance();
                }
                b'\\' => {
                    self.cursor.advance();
                    if self.cursor.current() != 0 || !self.cursor.is_eof() {
                        self.cursor.advance();
                    }
                }
                b'\n' | b'\r' => {
                    self.cursor.advance();
                }
                0 => {
                    if self.cursor.is_eof() {
                        return RawToken {
                            tag: RawTag::UnterminatedTemplate,
                            len: self.cursor.pos() - start,
                        };
                    }
                    self.cursor.advance();
                }
                _ => unreachable!("skip_to_template_delim returned unexpected byte"),
            }
        }
    }
}
