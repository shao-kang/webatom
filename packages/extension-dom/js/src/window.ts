import { Document } from './interface/document';
// import { Event } from './interface/event_target';

// ── Location ──────────────────────────────────────────────────────────────
const document = new Document()
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

// ── Window object ─────────────────────────────────────────────────────────

const windowDefs: Record<string, unknown> = {
  document,
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

  // Window-level events (no-op: framework-level events not supported)
  addEventListener(_type: string, _handler: unknown): void    {},
  removeEventListener(_type: string, _handler: unknown): void {},
  dispatchEvent(_event: Event): boolean                       { return true; },

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
    return true;
  },
  has(target, key) {
    return key in target || key in globalThis;
  },
});

(globalThis as Record<string, unknown>).window = window;
