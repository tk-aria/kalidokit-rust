/**
 * Extism JavaScript plugin example for dynplug.
 *
 * Build:
 *     extism-js plugin.js -i plugin.d.ts -o plugin.wasm
 *
 * Test:
 *     cargo run -p dynplug --features wasm --example host_wasm_polyglot
 */

function greet() {
  const name = Host.inputString();
  Host.outputString(`Hello from JavaScript, ${name}!`);
}

function add() {
  const str = Host.inputString();
  // Input is 8 raw bytes: two little-endian i32 values
  // Read them via charCodeAt since Host.inputString gives raw bytes as a string
  const buf = new ArrayBuffer(str.length);
  const u8 = new Uint8Array(buf);
  for (let i = 0; i < str.length; i++) {
    u8[i] = str.charCodeAt(i);
  }
  if (u8.length !== 8) {
    throw new Error(`expected 8 bytes, got ${u8.length}`);
  }
  const view = new DataView(buf);
  const a = view.getInt32(0, true);
  const b = view.getInt32(4, true);
  const out = new ArrayBuffer(4);
  new DataView(out).setInt32(0, a + b, true);
  // Output as raw string
  const outBytes = new Uint8Array(out);
  let outStr = "";
  for (let i = 0; i < outBytes.length; i++) {
    outStr += String.fromCharCode(outBytes[i]);
  }
  Host.outputString(outStr);
}

function language() {
  Host.outputString("javascript");
}

module.exports = { greet, add, language };
