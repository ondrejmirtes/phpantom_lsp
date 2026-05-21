# PHPantom → WebAssembly proof of concept

Goal: run PHPantom's PHP analysis **in the browser** (for the phpstan.org `/try`
editor) by compiling it to WebAssembly, instead of round-tripping to a server.

## What this PoC establishes

- The whole crate compiles to **`wasm32-wasip1`** with only minimal changes:
  - `tokio` is declared per-target (full on native, slim — no `net`/`mio` — on wasm),
  - `main.rs`'s stdio/TCP LSP transport is `cfg`-gated out on wasm.
- WASI gives us the filesystem APIs (`Url::to_file_path`, `std::fs`, `ignore`),
  so the ~23 path-conversion sites and the FS code "just work" — no shimming.
- `src/bin/wasm_analyze.rs` drives the **per-file LSP path** (the browser path):
  `Backend::new_headless()` → `update_ast()` → `collect_slow_diagnostics()`.
  All synchronous: no async runtime, no thread pool.

The CLI `analyze`/`fix` batch path spawns OS worker threads (unsupported on
wasip1) — but that's project indexing, not what the browser needs.

## Build & run

```bash
rustup target add wasm32-wasip1

# size-optimized release build (~11 MiB raw, ~2.2 MiB gzipped)
cargo build --release --bin wasm_analyze --target wasm32-wasip1

# run under Node's built-in WASI (Node 20+)
node --experimental-wasi-unstable-preview1 wasm-poc/run-wasi.mjs \
  target/wasm32-wasip1/release/wasm_analyze.wasm /work/test.php
```

Expected output:

```
diagnostics: 2
  [Warning] /work/test.php:13:10  Method 'goodbye' not found on class 'Greeter'
  [Error]   /work/test.php:12:15  Expected 1 argument, got 0
```

## Next steps toward the browser

1. Replace the CLI bin with a `wasm-bindgen`/JS-callable surface (or keep WASI and
   use a browser WASI shim with an in-memory FS).
2. Drive `Backend` LSP requests (hover, completion, signatureHelp) over a worker.
3. Wire `@codemirror/lsp-client` ↔ the worker; keep PHPStan-on-Lambda authoritative
   for the error panel, use PHPantom only for editor intelligence.
