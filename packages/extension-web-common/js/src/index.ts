// Stub globals that webidl-conversions accesses at init time but QuickJS lacks.
if (typeof globalThis.SharedArrayBuffer === 'undefined') {
  (globalThis as any).SharedArrayBuffer = class SharedArrayBuffer {};
}
if (typeof (globalThis as any).Buffer === 'undefined') {
  (globalThis as any).Buffer = undefined;
}
if (typeof globalThis.atob === 'undefined') {
  (globalThis as any).atob = undefined;
}

import { URL, URLSearchParams } from 'whatwg-url';

globalThis.URL = URL;
globalThis.URLSearchParams = URLSearchParams;
