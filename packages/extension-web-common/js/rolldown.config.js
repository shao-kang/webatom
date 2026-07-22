import { defineConfig } from 'rolldown';

const quickjsStubs = `
if (typeof globalThis.SharedArrayBuffer === 'undefined') {
  function SharedArrayBuffer(len) { this._len = len || 0; }
  Object.defineProperty(SharedArrayBuffer.prototype, 'byteLength', { get() { return this._len; } });
  Object.defineProperty(SharedArrayBuffer.prototype, 'growable',   { get() { return false; } });
  globalThis.SharedArrayBuffer = SharedArrayBuffer;
}
if (typeof globalThis.Buffer === 'undefined') globalThis.Buffer = undefined;
if (typeof globalThis.atob  === 'undefined') globalThis.atob  = undefined;
`;

export default defineConfig({
  input: 'src/index.ts',
  output: {
    file: 'dist/index.js',
    format: 'iife',
    banner: quickjsStubs,
  },
});
