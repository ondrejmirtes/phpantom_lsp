//! Target-independent LSP JSON-RPC dispatcher for the wasm builds.
//!
//! Speaks the LSP protocol so the browser can drive PHPantom with
//! `@codemirror/lsp-client` (completion, hover, signature help, …). Routes each
//! request to PHPantom's existing *synchronous* `handle_*` methods —
//! single-threaded, no async runtime, no thread pool.
//!
//! Built for `wasm32-wasip1` (the WASI target works correctly; the bare
//! `wasm32-unknown-unknown` target hits a memory-corruption bug in the
//! completion path). The WASI reactor exports in `wasm_wasi` wrap this.

use crate::Backend;
use serde_json::{json, Value};
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use tower_lsp::lsp_types::{
	CompletionItem, CompletionParams, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
	HoverParams, SignatureHelpParams,
};

// `handle_completion` is `async` only as a trait artifact — it never awaits, so
// a single poll resolves it.
fn now_or_never<F: Future>(future: F) -> Option<F::Output> {
	let mut future = pin!(future);
	let mut cx = Context::from_waker(Waker::noop());
	match future.as_mut().poll(&mut cx) {
		Poll::Ready(value) => Some(value),
		Poll::Pending => None,
	}
}

pub struct LspDispatcher {
	backend: Backend,
}

impl LspDispatcher {
	pub fn new() -> LspDispatcher {
		LspDispatcher {
			backend: Backend::new_headless(),
		}
	}

	/// Handle one LSP JSON-RPC message. Returns a JSON-RPC response string for
	/// requests, or `None` for notifications (which produce no reply).
	pub fn handle(&self, message: &str) -> Option<String> {
		let value: Value = serde_json::from_str(message).ok()?;
		let id = value.get("id").cloned();
		let method = value.get("method").and_then(Value::as_str).unwrap_or("");
		let params = value.get("params").cloned().unwrap_or(Value::Null);

		let outcome: Option<Result<Value, (i64, String)>> = match method {
			"initialize" => Some(Ok(capabilities())),
			"shutdown" => Some(Ok(Value::Null)),
			"initialized" | "exit" | "$/cancelRequest" => None,
			"textDocument/didOpen" => {
				self.did_open(params);
				None
			}
			"textDocument/didChange" => {
				self.did_change(params);
				None
			}
			"textDocument/completion" => Some(self.completion(params)),
			"completionItem/resolve" => Some(self.resolve(params)),
			"textDocument/hover" => Some(self.hover(params)),
			"textDocument/signatureHelp" => Some(self.signature_help(params)),
			_ => {
				if id.is_some() {
					Some(Err((-32601, format!("method not found: {method}"))))
				} else {
					None
				}
			}
		};

		match (id, outcome) {
			(Some(id), Some(Ok(result))) => {
				Some(json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string())
			}
			(Some(id), Some(Err((code, message)))) => Some(
				json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
					.to_string(),
			),
			_ => None,
		}
	}

	// Mirror the server's did_open: store the file content (completion reads it
	// back via get_file_content) and parse/index it.
	fn store(&self, uri: String, text: String) {
		self.backend
			.open_files()
			.write()
			.insert(uri.clone(), Arc::new(text.clone()));
		self.backend.update_ast(&uri, &text);
	}

	fn did_open(&self, params: Value) {
		if let Ok(p) = serde_json::from_value::<DidOpenTextDocumentParams>(params) {
			self.store(p.text_document.uri.to_string(), p.text_document.text);
		}
	}

	fn did_change(&self, params: Value) {
		if let Ok(p) = serde_json::from_value::<DidChangeTextDocumentParams>(params) {
			// Full-sync: the last change carries the entire document.
			if let Some(change) = p.content_changes.into_iter().next_back() {
				self.store(p.text_document.uri.to_string(), change.text);
			}
		}
	}

	fn completion(&self, params: Value) -> Result<Value, (i64, String)> {
		let params: CompletionParams = serde_json::from_value(params).map_err(bad_params)?;
		match now_or_never(self.backend.handle_completion(params)) {
			Some(Ok(Some(response))) => Ok(serde_json::to_value(response).unwrap_or(Value::Null)),
			_ => Ok(Value::Null),
		}
	}

	fn resolve(&self, params: Value) -> Result<Value, (i64, String)> {
		let item: CompletionItem = serde_json::from_value(params).map_err(bad_params)?;
		let resolved = self.backend.handle_completion_resolve(item);
		Ok(serde_json::to_value(resolved).unwrap_or(Value::Null))
	}

	fn hover(&self, params: Value) -> Result<Value, (i64, String)> {
		let params: HoverParams = serde_json::from_value(params).map_err(bad_params)?;
		let uri = params.text_document_position_params.text_document.uri.to_string();
		let position = params.text_document_position_params.position;
		let Some(content) = self.backend.get_file_content(&uri) else {
			return Ok(Value::Null);
		};
		match self.backend.handle_hover(&uri, &content, position) {
			Some(hover) => Ok(serde_json::to_value(hover).unwrap_or(Value::Null)),
			None => Ok(Value::Null),
		}
	}

	fn signature_help(&self, params: Value) -> Result<Value, (i64, String)> {
		let params: SignatureHelpParams = serde_json::from_value(params).map_err(bad_params)?;
		let uri = params.text_document_position_params.text_document.uri.to_string();
		let position = params.text_document_position_params.position;
		let Some(content) = self.backend.get_file_content(&uri) else {
			return Ok(Value::Null);
		};
		match self.backend.handle_signature_help(&uri, &content, position) {
			Some(help) => Ok(serde_json::to_value(help).unwrap_or(Value::Null)),
			None => Ok(Value::Null),
		}
	}
}

fn bad_params(error: serde_json::Error) -> (i64, String) {
	(-32602, error.to_string())
}

fn capabilities() -> Value {
	json!({
		"capabilities": {
			"textDocumentSync": 1,
			"completionProvider": {
				"triggerCharacters": [">", ":", "$", "\\"],
				"resolveProvider": true
			},
			"hoverProvider": true,
			"signatureHelpProvider": { "triggerCharacters": ["(", ","] }
		},
		"serverInfo": { "name": "phpantom-wasm", "version": "0.8.0" }
	})
}
