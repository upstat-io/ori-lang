//! String join consumer.

use std::ptr;

use super::super::state::assert_elem_size;
use super::super::ElemBuf;
use super::take_iter;

// Join

/// Join iterator elements into a single string with separator.
///
/// Each element is expected to already be an `OriStr` (24 bytes).
/// The `to_str_fn` trampoline converts non-string elements to strings.
/// If `to_str_fn` is null, elements are assumed to be strings.
///
/// The separator is passed as its 3 struct fields (matching LLVM `{i64, i64, ptr}`
/// layout for `OriStr`). This is SSO-safe because the runtime reconstructs the
/// full `OriStr` union from the raw fields.
///
/// `elem_dec_fn` releases each CONSUMED element after its bytes are copied
/// into the result. Codegen passes it non-null only when it proves every
/// element reaching join is adapter-produced (consumer-owned, RC 1,
/// owned-by-nobody-else); it stays null for source-borrowed chains, whose
/// elements the source buffer's own cleanup releases. The runtime never
/// infers ownership — it only honors the verdict it was handed.
///
/// Writes the resulting `OriStr` to `out_ptr` (sret pattern, 24 bytes).
#[no_mangle]
pub extern "C-unwind" fn ori_iter_join(
    iter: *mut u8,
    sep_field0: i64,
    sep_field1: i64,
    sep_field2: *const u8,
    to_str_fn: Option<extern "C" fn(*mut u8, *const u8, *mut u8)>,
    to_str_env: *mut u8,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    use crate::string::OriStr;

    assert_elem_size(elem_size, "ori_iter_join");

    if out_ptr.is_null() {
        drop(take_iter(iter));
        return;
    }

    if iter.is_null() {
        // Empty iterator — return empty string (SSO empty)
        let empty = OriStr::from_bytes(b"");
        unsafe {
            ptr::copy_nonoverlapping(
                std::ptr::from_ref::<OriStr>(&empty).cast::<u8>(),
                out_ptr,
                std::mem::size_of::<OriStr>(),
            );
        }
        let _ = empty;
        return;
    }

    let Some(mut state) = take_iter(iter) else {
        return;
    };
    // Reconstruct the raw `{len, cap, data}` bits; SSO stores inline bytes in
    // that layout. Its flag occupies pointer MSB position, relying on canonical
    // 64-bit user addresses with a clear high bit.
    let sep_str = OriStr {
        heap: crate::string::OriStrHeap {
            len: sep_field0,
            cap: sep_field1,
            data: sep_field2 as *mut u8,
        },
    };
    // Borrow SSO bytes from the local value or heap bytes from caller-owned RC storage.
    // SAFETY: codegen supplied a valid OriStr layout that outlives this loop.
    let sep = unsafe { sep_str.as_str() };

    let mut result = String::new();
    let mut elem_buf = ElemBuf::new();
    let mut first = true;

    // SAFETY: `state` is live, `elem_buf` exceeds OriStr alignment, and the
    // checked element size bounds every `next` write.
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        if !first {
            result.push_str(sep);
        }

        // Convert element to string via trampoline or direct string read.
        if let Some(to_str) = to_str_fn {
            // `MaybeUninit<OriStr>` gives OriStr's 8-byte alignment, so the
            // trampoline's sret store lands aligned (a bare `[u8; N]` would be
            // align-1 — a misaligned OriStr write is UB).
            let mut str_buf = std::mem::MaybeUninit::<OriStr>::uninit();
            (to_str)(
                to_str_env,
                elem_buf.as_ptr(),
                str_buf.as_mut_ptr().cast::<u8>(),
            );
            // SAFETY: the trampoline wrote a complete OriStr to the aligned
            // buffer; OriStr is Copy (no Drop), so reading it out is sound.
            let s = unsafe { str_buf.assume_init() };
            // SAFETY: `s` is a valid OriStr whose data pointer (heap) or inline
            // bytes (SSO) are valid for the borrow's lifetime (until the next
            // loop iteration overwrites `str_buf`, after `push_str` copies out).
            result.push_str(unsafe { s.as_str() });
            // Copy OriStr temporaries need explicit release; the helper handles
            // SSO and slices before freeing heap storage.
            // SAFETY: `s` is exactly the trampoline-produced value owned here.
            unsafe {
                crate::rc::ori_str_rc_dec(
                    s.heap.data,
                    s.heap.cap,
                    Some(crate::rc::ori_str_drop_buffer),
                );
            }
            // Input-element release is separate from produced-string release;
            // codegen provides a type-matched thunk only for managed elements.
            if let Some(dec) = elem_dec_fn {
                (dec)(elem_buf.as_mut_ptr());
            }
        } else {
            // SAFETY: A string-element chain writes a complete Copy OriStr;
            // unaligned read supports the scratch-buffer layout.
            let s = unsafe { ptr::read_unaligned(elem_buf.as_ptr().cast::<OriStr>()) };
            // SAFETY: `s` is a valid OriStr; its bytes are copied out by
            // push_str before the buffer is reused.
            result.push_str(unsafe { s.as_str() });
            // Only adapter-produced chains supply a release thunk; source-
            // borrowed elements remain owned by source-buffer cleanup.
            if let Some(dec) = elem_dec_fn {
                (dec)(elem_buf.as_mut_ptr());
            }
        }

        first = false;
    }

    let ori_str = OriStr::from_owned(&result);
    unsafe {
        ptr::copy_nonoverlapping(
            std::ptr::from_ref::<OriStr>(&ori_str).cast::<u8>(),
            out_ptr,
            std::mem::size_of::<OriStr>(),
        );
    }
    let _ = ori_str;
}
