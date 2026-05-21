// Run the phpantom_lsp WASI binary under Node's built-in WASI, with an
// in-memory-style preopened project dir. Proves the real analysis pipeline
// (mago parse + phpantom type analysis + diagnostics) runs in wasm.
import { WASI } from 'node:wasi';
import { readFileSync } from 'node:fs';
import { argv } from 'node:process';

const wasmPath = argv[2];
const subcommand = argv.slice(3);

const wasi = new WASI({
  version: 'preview1',
  args: ['phpantom_lsp', ...subcommand],
  env: {},
  preopens: { '/work': new URL('./work', import.meta.url).pathname },
  returnOnExit: true,
});

const bytes = readFileSync(wasmPath);
console.log(`wasm size: ${(bytes.length / (1024 * 1024)).toFixed(1)} MiB`);
const t0 = performance.now();
const module = await WebAssembly.compile(bytes);
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
const compileMs = (performance.now() - t0).toFixed(0);
console.log(`compiled+instantiated in ${compileMs} ms`);

const t1 = performance.now();
const code = wasi.start(instance);
const runMs = (performance.now() - t1).toFixed(0);
console.log(`\n--- wasi exit code: ${code} (ran in ${runMs} ms) ---`);
