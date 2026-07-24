/// Zeroed, 16-byte-aligned storage for an ABI result.
#[repr(C, align(16))]
pub(crate) struct AbiOutput<const N: usize>([u8; N]);

impl<const N: usize> AbiOutput<N> {
    /// Borrow the initialized test storage as bytes.
    pub(crate) const fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Return a pointer to the ABI result storage.
    pub(crate) const fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    /// Return a mutable pointer to the ABI result storage.
    pub(crate) const fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }
}

impl<const N: usize> Default for AbiOutput<N> {
    fn default() -> Self {
        Self([0; N])
    }
}
