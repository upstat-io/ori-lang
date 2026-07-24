//! Ownership cleanup for iterator states and successful yields.

use super::IterState;

/// Unwind-safe ownership guard for a successful child yield.
///
/// Transforming and discarding adapters leave the guard armed. Identity
/// adapters disarm it when they forward the element and its obligation.
pub(crate) struct YieldGuard {
    source: *mut IterState,
    elem_ptr: *mut u8,
    armed: bool,
}

impl YieldGuard {
    pub(crate) fn new(source: &mut IterState, elem_ptr: *mut u8) -> Self {
        Self {
            source,
            elem_ptr,
            armed: true,
        }
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for YieldGuard {
    fn drop(&mut self) {
        if self.armed {
            // SAFETY: constructed only after `source` successfully initialized
            // `elem_ptr`, and the guard cannot outlive either stack frame.
            unsafe { (*self.source).release_last_yield(self.elem_ptr) };
        }
    }
}

fn dec_owned_buffer(buffer: &mut [u8], elem_size: i64, dec: extern "C" fn(*mut u8)) {
    let element_size = elem_size.max(1) as usize;
    let element_count = buffer.len() / element_size;
    for index in 0..element_count {
        // SAFETY: the buffer holds `element_count` contiguous elements.
        let element = unsafe { buffer.as_mut_ptr().add(index * element_size) };
        dec(element);
    }
}

impl Drop for IterState {
    fn drop(&mut self) {
        match self {
            IterState::List {
                data,
                len,
                cap,
                elem_size,
                owns_data,
                ..
            } => {
                if *owns_data && !data.is_null() && *cap != 0 {
                    crate::ori_buffer_rc_dec(*data, *len, *cap, *elem_size, None);
                }
            }
            IterState::Str {
                data,
                cap,
                owns_data,
                ..
            } => {
                if *owns_data && !data.is_null() {
                    if crate::slice_encoding::is_slice_cap(*cap) {
                        let original = crate::slice_encoding::slice_original_data(*data, *cap);
                        let data_size = crate::ori_rc_data_size(original.cast_const());
                        crate::ori_buffer_rc_dec(original, 0, data_size, 1, None);
                    } else {
                        crate::ori_buffer_rc_dec(*data, 0, *cap, 1, None);
                    }
                }
            }
            IterState::Map {
                data,
                cap,
                len,
                key_size,
                val_size,
                owns_data,
                key_dec_fn,
                val_dec_fn,
                ..
            } => {
                if *owns_data && !data.is_null() {
                    crate::ori_map_buffer_rc_dec(
                        *data,
                        *cap,
                        *len,
                        *key_size,
                        *val_size,
                        *key_dec_fn,
                        *val_dec_fn,
                    );
                }
            }
            IterState::Cycled {
                buffer,
                elem_size,
                elem_dec_fn: Some(dec),
                ..
            } => dec_owned_buffer(buffer, *elem_size, *dec),
            IterState::Reversed {
                elements,
                elem_size,
                elem_dec_fn: Some(dec),
                ..
            } => dec_owned_buffer(elements, *elem_size, *dec),
            IterState::Repeat {
                value,
                elem_size,
                elem_dec_fn: Some(dec),
            } => dec_owned_buffer(value, *elem_size, *dec),
            _ => {}
        }
    }
}

/// Create an empty range iterator (yields nothing).
pub(in crate::iterator) fn empty_range() -> IterState {
    IterState::Range {
        current: 0,
        end: 0,
        step: 1,
        inclusive: false,
    }
}

// SAFETY: function pointers and raw pointers in `IterState` are transferable.
unsafe impl Send for IterState {}
