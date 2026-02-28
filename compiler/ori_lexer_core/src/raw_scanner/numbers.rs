//! Numeric literal scanning (decimal, hex, binary, float, duration/size suffixes).
//!
//! Entry point: [`number()`](super::RawScanner::number), dispatched from
//! `next_token()` when the current byte is `b'0'..=b'9'`.

use crate::tag::{RawTag, RawToken};

impl super::RawScanner<'_> {
    #[inline]
    pub(super) fn number(&mut self, start: u32) -> RawToken {
        let first = self.cursor.current();
        self.cursor.advance();

        // Check for hex prefix: 0x or 0X
        if first == b'0' && matches!(self.cursor.current(), b'x' | b'X') {
            return self.hex_number(start);
        }

        // Check for binary prefix: 0b or 0B followed by binary digit or underscore.
        // Without the peek, `0b` (0 bytes size literal) would be misclassified.
        if first == b'0'
            && matches!(self.cursor.current(), b'b' | b'B')
            && matches!(self.cursor.peek(), b'0' | b'1' | b'_')
        {
            return self.bin_number(start);
        }

        // Decimal digits and underscores
        self.eat_decimal_digits();

        // Check for float (dot followed by digit — not `..` range)
        if self.cursor.current() == b'.' && self.cursor.peek().is_ascii_digit() {
            self.cursor.advance(); // consume '.'
            self.eat_decimal_digits();
            self.eat_exponent();
            return self.check_suffix(start, true);
        }

        // Check for exponent without dot (e.g., 1e5)
        if matches!(self.cursor.current(), b'e' | b'E') {
            self.eat_exponent();
            return self.check_suffix(start, true);
        }

        // Integer — check for duration/size suffix
        self.check_suffix(start, false)
    }

    fn hex_number(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume 'x' or 'X'
        self.cursor
            .eat_while(|b| b.is_ascii_hexdigit() || b == b'_');
        RawToken {
            tag: RawTag::HexInt,
            len: self.cursor.pos() - start,
        }
    }

    fn bin_number(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume 'b' or 'B'
        self.cursor
            .eat_while(|b| b == b'0' || b == b'1' || b == b'_');
        RawToken {
            tag: RawTag::BinInt,
            len: self.cursor.pos() - start,
        }
    }

    fn eat_decimal_digits(&mut self) {
        self.cursor.eat_while(|b| b.is_ascii_digit() || b == b'_');
    }

    fn eat_exponent(&mut self) {
        if matches!(self.cursor.current(), b'e' | b'E') {
            self.cursor.advance();
            if matches!(self.cursor.current(), b'+' | b'-') {
                self.cursor.advance();
            }
            self.eat_decimal_digits();
        }
    }

    /// Check for duration/size suffix after a numeric literal.
    /// `is_float` indicates whether a decimal point was consumed.
    fn check_suffix(&mut self, start: u32, is_float: bool) -> RawToken {
        let default_tag = if is_float { RawTag::Float } else { RawTag::Int };

        match self.cursor.current() {
            // ns, us — 2-char duration suffixes
            b'n' | b'u'
                if self.cursor.peek() == b's' && !super::is_ident_continue(self.cursor.peek2()) =>
            {
                self.cursor.advance_n(2);
                RawToken {
                    tag: RawTag::Duration,
                    len: self.cursor.pos() - start,
                }
            }
            // m, ms, mb — minutes / milliseconds / megabytes
            b'm' => match self.cursor.peek() {
                b's' if !super::is_ident_continue(self.cursor.peek2()) => {
                    self.cursor.advance_n(2);
                    RawToken {
                        tag: RawTag::Duration,
                        len: self.cursor.pos() - start,
                    }
                }
                b'b' if !super::is_ident_continue(self.cursor.peek2()) => {
                    self.cursor.advance_n(2);
                    RawToken {
                        tag: RawTag::Size,
                        len: self.cursor.pos() - start,
                    }
                }
                next if !super::is_ident_continue(next) => {
                    self.cursor.advance();
                    RawToken {
                        tag: RawTag::Duration,
                        len: self.cursor.pos() - start,
                    }
                }
                _ => RawToken {
                    tag: default_tag,
                    len: self.cursor.pos() - start,
                },
            },
            // s, h — 1-char duration suffixes
            b's' | b'h' if !super::is_ident_continue(self.cursor.peek()) => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::Duration,
                    len: self.cursor.pos() - start,
                }
            }
            // b — bytes (1-char size suffix)
            b'b' if !super::is_ident_continue(self.cursor.peek()) => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::Size,
                    len: self.cursor.pos() - start,
                }
            }
            // kb, gb, tb — 2-char size suffixes
            b'k' | b'g' | b't'
                if self.cursor.peek() == b'b' && !super::is_ident_continue(self.cursor.peek2()) =>
            {
                self.cursor.advance_n(2);
                RawToken {
                    tag: RawTag::Size,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: default_tag,
                len: self.cursor.pos() - start,
            },
        }
    }
}
