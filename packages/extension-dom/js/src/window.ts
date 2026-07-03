import { Document } from './interface/document';
import { Node } from './interface/node';
import { Element } from './interface/element';
import { EventTarget } from './interface/event-target';
import { Event, CustomEvent, UIEvent, KeyboardEvent, MouseEvent, FocusEvent, InputEvent } from './interface/event';
import { MutationObserver, MutationRecord } from './interface/mutation-observer';
import '@/html/elements';

// ── Location ──────────────────────────────────────────────────────────────

interface ILocation {
  href: string;
  hostname: string;
  pathname: string;
  search: string;
  hash: string;
  protocol: string;
  origin: string;
  assign(url: string): void;
  replace(url: string): void;
  reload(): void;
  toString(): string;
}

const location: ILocation = {
  href: '', hostname: '', pathname: '/', search: '', hash: '',
  protocol: 'about:', origin: 'null',
  assign(_url) {},
  replace(_url) {},
  reload() {},
  toString() { return this.href; },
};

// ── Navigator ─────────────────────────────────────────────────────────────

const navigator = {
  userAgent: 'quickweb/0.1',
  language: 'en',
  languages: ['en'] as string[],
  onLine: true,
  cookieEnabled: false,
  platform: 'rquickjs',
};

// ── History ───────────────────────────────────────────────────────────────

const history = {
  length: 1,
  scrollRestoration: 'auto',
  pushState(_state: unknown, _title: string, _url?: string) {},
  replaceState(_state: unknown, _title: string, _url?: string) {},
  go(_delta?: number) {},
  back() {},
  forward() {},
};

// ── Screen ────────────────────────────────────────────────────────────────

const screen = {
  width: 1280, height: 720,
  availWidth: 1280, availHeight: 720,
  colorDepth: 24, pixelDepth: 24,
};

// ── Window-level EventTarget ──────────────────────────────────────────────

const _winEvTarget = new EventTarget();

// ── Window object ─────────────────────────────────────────────────────────

const windowDefs: Record<string, unknown> = {

  document: new Document(),
  location,
  navigator,
  history,
  screen,

  innerWidth: 1280,
  innerHeight: 720,
  outerWidth: 1280,
  outerHeight: 720,
  devicePixelRatio: 1,
  scrollX: 0,
  scrollY: 0,
  pageXOffset: 0,
  pageYOffset: 0,
  Event,
  CustomEvent,
  UIEvent,
  KeyboardEvent,
  MouseEvent,
  FocusEvent,
  InputEvent,
  EventTarget,
  MutationObserver,
  MutationRecord,

  // Timers — synchronous stubs; QuickJS has no event loop
  setTimeout(fn: () => void, _ms?: number): number   { fn(); return 0; },
  clearTimeout(_id: number): void                    {},
  setInterval(_fn: () => void, _ms?: number): number { return 0; },
  clearInterval(_id: number): void                   {},
  requestAnimationFrame(fn: (t: number) => void): number { fn(0); return 0; },
  cancelAnimationFrame(_id: number): void            {},

  // Scroll stubs
  scrollTo(_x?: number | ScrollToOptions, _y?: number): void {},
  scroll(_x?: number | ScrollToOptions, _y?: number): void   {},

  // Style
  getComputedStyle(_el: unknown): CSSStyleDeclaration {
    return { getPropertyValue: () => '', display: '' } as unknown as CSSStyleDeclaration;
  },

  // Window-level events — backed by _winEvTarget
  addEventListener(type: string, cb: any, opts?: any): void    { _winEvTarget.addEventListener(type, cb, opts); },
  removeEventListener(type: string, cb: any, opts?: any): void { _winEvTarget.removeEventListener(type, cb, opts); },
  dispatchEvent(event: any): boolean                           { return _winEvTarget.dispatchEvent(event); },

  // Dialogs
  alert(_message?: string): void              {},
  confirm(_message?: string): boolean         { return false; },
  prompt(_message?: string, _default?: string): string | null { return null; },

  queueMicrotask(fn: () => void): void { Promise.resolve().then(fn); },
};

export const window = new Proxy(windowDefs, {
  get(target, key) {
    if (key === 'window') return window;
    if (key in target) return target[key as string];
    return (globalThis as Record<string | symbol, unknown>)[key];
  },
  set(target, key, value) {
    target[key as string] = value;
    // Skip write-back when globalThis already has an accessor for this key;
    // the existing getter already reads through window, so no sync needed.
    const desc = Object.getOwnPropertyDescriptor(globalThis as object, key as string);
    if (!desc?.get) {
      (globalThis as Record<string | symbol, unknown>)[key] = value;
    }
    return true;
  },
  has(target, key) {
    return key in target || key in globalThis;
  },
});

// 把 windowDefs 的每个 key 以 getter/setter 代理到 globalThis
// 好处：直接写 `document` / `location` 等均通过 window Proxy 读写，行为与浏览器一致
const _g = globalThis as Record<string | symbol, unknown>;
for (const key of Object.keys(windowDefs)) {
  if (key in _g) continue;          // 不覆盖已有的（console、Promise 等）
  Object.defineProperty(_g, key, {
    get() { return window[key]; },
    set(v) { window[key] = v; },
    configurable: true,
    enumerable: false,
  });
}
_g.window = window;


