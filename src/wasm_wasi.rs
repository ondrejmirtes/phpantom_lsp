//! WASI reactor exports for the browser.
//!
//! The wasm is instantiated once (a reactor — `_initialize`, no `_start`) and
//! then driven through `lsp_handle`, which takes an LSP JSON-RPC message and
//! returns a response. Strings are marshalled manually through linear memory
//! (`lsp_alloc`/`lsp_dealloc`) since we don't use wasm-bindgen on this target.
//!
//! The dispatcher persists across calls in a thread-local (single-threaded).

use crate::lsp_dispatch::LspDispatcher;
use std::cell::{Cell, RefCell};

thread_local! {
	static DISPATCHER: RefCell<LspDispatcher> = RefCell::new(LspDispatcher::new());
	// Length of the buffer returned by the last `lsp_handle` call. Kept
	// separate so `lsp_handle` can return a plain pointer (u32) instead of a
	// packed u64 (which would surface as a BigInt in JS).
	static LAST_LEN: Cell<usize> = const { Cell::new(0) };
}

/// Length in bytes of the buffer returned by the most recent `lsp_handle`.
#[unsafe(no_mangle)]
pub extern "C" fn lsp_response_len() -> usize {
	LAST_LEN.with(|c| c.get())
}

/// Allocate `len` bytes in wasm linear memory; returns the pointer. The host
/// fills it (e.g. a UTF-8 JSON-RPC message) and passes it to `lsp_handle`.
#[unsafe(no_mangle)]
pub extern "C" fn lsp_alloc(len: usize) -> *mut u8 {
	let mut buf = Vec::<u8>::with_capacity(len);
	let ptr = buf.as_mut_ptr();
	std::mem::forget(buf);
	ptr
}

/// Free a buffer previously returned by `lsp_alloc` or `lsp_handle`.
///
/// # Safety
/// `ptr`/`len` must come from a prior `lsp_alloc`/`lsp_handle` call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsp_dealloc(ptr: *mut u8, len: usize) {
	if !ptr.is_null() && len != 0 {
		drop(unsafe { Vec::from_raw_parts(ptr, 0, len) });
	}
}

/// Handle one LSP JSON-RPC message located at `ptr`/`len`.
///
/// Returns a pointer to the response bytes (whose length is read via
/// `lsp_response_len`); the host reads them then frees with `lsp_dealloc`.
/// Returns null when there is no response (a notification).
///
/// # Safety
/// `ptr`/`len` must describe a valid UTF-8 buffer from `lsp_alloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsp_handle(ptr: *const u8, len: usize) -> *mut u8 {
	let message = unsafe { std::slice::from_raw_parts(ptr, len) };
	let message = String::from_utf8_lossy(message);

	let response = DISPATCHER.with(|d| d.borrow().handle(&message));

	match response {
		None => {
			LAST_LEN.with(|c| c.set(0));
			std::ptr::null_mut()
		}
		Some(text) => {
			let mut bytes = text.into_bytes().into_boxed_slice();
			let out_ptr = bytes.as_mut_ptr();
			LAST_LEN.with(|c| c.set(bytes.len()));
			std::mem::forget(bytes);
			out_ptr
		}
	}
}
