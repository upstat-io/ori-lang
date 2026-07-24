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
        self.scan_template_body(start, RawTag::TemplateComplete, RawTag::TemplateHead)
    }

    pub(super) fn template_middle_or_tail(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume closing '}'
        self.scan_template_body(start, RawTag::TemplateTail, RawTag::TemplateMiddle)
    }

    /// Scan template content until the closing backtick or the next `{`
    /// interpolation. `closing_tag` is emitted at the closing `` ` `` (the
    /// segment ran to the end of the template); `interp_tag` is emitted at an
    /// unescaped `{` (an interpolation opens, depth pushed). Shared by the
    /// opening (`template_literal`) and continuation (`template_middle_or_tail`)
    /// scans — the only difference between them is which pair of tags they pass.
    #[inline]
    fn scan_template_body(
        &mut self,
        start: u32,
        closing_tag: RawTag,
        interp_tag: RawTag,
    ) -> RawToken {
        loop {
            // SIMD-accelerated skip past ordinary template content
            let b = self.cursor.skip_to_template_delim();
            match b {
                b'`' => {
                    self.cursor.advance();
                    return RawToken {
                        tag: closing_tag,
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
                        tag: interp_tag,
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
                    self.skip_escape_body();
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
}
