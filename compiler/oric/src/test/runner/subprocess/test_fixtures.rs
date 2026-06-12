//! Shared IO fixtures for the subprocess + stream test pins.

/// Reader that yields one payload, then fails every subsequent read.
pub(crate) struct ErrorAfterFirstRead {
    payload: Vec<u8>,
    sent: bool,
}

impl ErrorAfterFirstRead {
    pub(crate) fn new(payload: &[u8]) -> Self {
        ErrorAfterFirstRead {
            payload: payload.to_vec(),
            sent: false,
        }
    }
}

impl std::io::Read for ErrorAfterFirstRead {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.sent {
            return Err(std::io::Error::other("synthetic read failure"));
        }
        self.sent = true;
        let n = self.payload.len().min(buf.len());
        buf[..n].copy_from_slice(&self.payload[..n]);
        Ok(n)
    }
}
