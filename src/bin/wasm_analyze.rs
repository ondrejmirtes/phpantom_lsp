//! Minimal headless single-file analysis entry.
//!
//! Representative of the browser / LSP per-file path (no project indexing,
//! no thread pools): construct a headless `Backend`, parse+index one file
//! with `update_ast`, then collect diagnostics with `collect_slow_diagnostics`.
//! All of these are synchronous, so no async runtime or threads are needed —
//! which is exactly what makes it wasm/WASI-friendly.
//!
//! Reads the PHP file path from argv[1] (defaults to /work/test.php).

use phpantom_lsp::Backend;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/work/test.php".to_string());
    let content = std::fs::read_to_string(&path).expect("read php file");

    let uri = if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    };

    let backend = Backend::new_headless();
    backend.update_ast(&uri, &content);

    let mut out = Vec::new();
    backend.collect_slow_diagnostics(&uri, &content, &mut out);

    println!("diagnostics: {}", out.len());
    for d in &out {
        let line = d.range.start.line + 1;
        let col = d.range.start.character + 1;
        let sev = d
            .severity
            .map(|s| format!("{s:?}"))
            .unwrap_or_else(|| "?".to_string());
        println!("  [{sev}] {path}:{line}:{col}  {}", d.message);
    }
}
