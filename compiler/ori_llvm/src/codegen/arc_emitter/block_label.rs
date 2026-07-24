//! Stack-backed LLVM block labels.

/// Stack-backed `bb{index}` label used while LLVM copies a block name.
pub(super) struct BlockLabel {
    bytes: [u8; 32],
    len: usize,
}

impl BlockLabel {
    /// Format `index` as a decimal LLVM basic-block label without heap allocation.
    pub(super) fn new(mut index: usize) -> Self {
        const PREFIX_LEN: usize = 2;
        const DIGITS: &[u8; 10] = b"0123456789";

        let mut bytes = [0; 32];
        let mut cursor = bytes.len();
        loop {
            cursor -= 1;
            bytes[cursor] = DIGITS[index % 10];
            index /= 10;
            if index == 0 {
                break;
            }
        }

        let digit_count = bytes.len() - cursor;
        let len = PREFIX_LEN + digit_count;
        bytes.copy_within(cursor.., PREFIX_LEN);
        bytes[..PREFIX_LEN].copy_from_slice(b"bb");
        Self { bytes, len }
    }

    /// Borrow the initialized ASCII prefix of the backing buffer.
    pub(super) fn as_str(&self) -> &str {
        // SAFETY: Every byte in `bytes[..len]` is initialized with ASCII text.
        unsafe { std::str::from_utf8_unchecked(&self.bytes[..self.len]) }
    }
}
