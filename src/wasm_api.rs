//! wasm-bindgen surface for running PHPantom in the browser.
//!
//! A single long-lived `PhpAnalyzer` holds a headless `Backend`. The host
//! (a Web Worker) calls `update()` whenever the document changes, then queries
//! `hover()` / `diagnostics()` on demand. Everything is synchronous and
//! single-threaded — no async runtime, no thread pool — which is what makes it
//! browser-friendly.

use crate::Backend;
use tower_lsp::lsp_types::Position;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct PhpAnalyzer {
    backend: Backend,
}

#[wasm_bindgen]
impl PhpAnalyzer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> PhpAnalyzer {
        console_error_panic_hook::set_once();
        PhpAnalyzer {
            backend: Backend::new_headless(),
        }
    }

    /// Parse and index a document. Call on open and on every change before
    /// querying hover/diagnostics.
    pub fn update(&self, uri: &str, content: &str) {
        self.backend.update_ast(uri, content);
    }

    /// Hover at a zero-based (line, character). Returns the LSP `Hover` as a
    /// JSON string, or `undefined` when there's nothing to show.
    pub fn hover(&self, uri: &str, content: &str, line: u32, character: u32) -> Option<String> {
        let position = Position { line, character };
        let hover = self.backend.handle_hover(uri, content, position)?;
        serde_json::to_string(&hover).ok()
    }

    /// PHPantom's own diagnostics for the file, as a JSON array string of LSP
    /// `Diagnostic`s. (Not wired into the UI by default — PHPStan stays
    /// authoritative for the error panel — but handy for debugging.)
    pub fn diagnostics(&self, uri: &str, content: &str) -> String {
        let mut out = Vec::new();
        self.backend.collect_slow_diagnostics(uri, content, &mut out);
        serde_json::to_string(&out).unwrap_or_else(|_| "[]".to_string())
    }
}
