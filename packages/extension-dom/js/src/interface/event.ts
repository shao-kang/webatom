// https://dom.spec.whatwg.org/#event

export interface EventInit {
  bubbles?: boolean;
  cancelable?: boolean;
  composed?: boolean;
}

export class Event {
  static readonly NONE            = 0;
  static readonly CAPTURING_PHASE = 1;
  static readonly AT_TARGET       = 2;
  static readonly BUBBLING_PHASE  = 3;

  // Instance constants (required by DOM spec on Event instances)
  readonly NONE            = 0;
  readonly CAPTURING_PHASE = 1;
  readonly AT_TARGET       = 2;
  readonly BUBBLING_PHASE  = 3;

  readonly type: string;
  readonly bubbles: boolean;
  readonly cancelable: boolean;
  readonly composed: boolean;
  readonly isTrusted = false;
  readonly timeStamp: number;

  // Mutable during dispatch
  target:         any = null;
  currentTarget:  any = null;
  eventPhase      = 0;
  defaultPrevented = false;

  /** @internal */
  _propagationStopped          = false;
  /** @internal */
  _immediatePropagationStopped = false;

  constructor(type: string, init?: EventInit) {
    this.type       = type;
    this.bubbles    = init?.bubbles    ?? false;
    this.cancelable = init?.cancelable ?? false;
    this.composed   = init?.composed   ?? false;
    this.timeStamp  = Date.now();
  }

  stopPropagation(): void {
    this._propagationStopped = true;
  }

  stopImmediatePropagation(): void {
    this._propagationStopped = true;
    this._immediatePropagationStopped = true;
  }

  preventDefault(): void {
    if (this.cancelable) this.defaultPrevented = true;
  }

  composedPath(): any[] { return []; }
}

// ── CustomEvent ───────────────────────────────────────────────────────────────

export class CustomEvent extends Event {
  readonly detail: unknown;

  constructor(type: string, init?: EventInit & { detail?: unknown }) {
    super(type, init);
    this.detail = init?.detail ?? null;
  }
}

// ── UIEvent ───────────────────────────────────────────────────────────────────

export class UIEvent extends Event {
  readonly detail: number;

  constructor(type: string, init?: EventInit & { detail?: number }) {
    super(type, init);
    this.detail = init?.detail ?? 0;
  }
}

// ── KeyboardEvent ─────────────────────────────────────────────────────────────

export interface KeyboardEventInit extends EventInit {
  key?:      string;
  code?:     string;
  altKey?:   boolean;
  ctrlKey?:  boolean;
  shiftKey?: boolean;
  metaKey?:  boolean;
  repeat?:   boolean;
}

export class KeyboardEvent extends UIEvent {
  static readonly DOM_KEY_LOCATION_STANDARD = 0;
  static readonly DOM_KEY_LOCATION_LEFT     = 1;
  static readonly DOM_KEY_LOCATION_RIGHT    = 2;
  static readonly DOM_KEY_LOCATION_NUMPAD   = 3;

  readonly key:      string;
  readonly code:     string;
  readonly altKey:   boolean;
  readonly ctrlKey:  boolean;
  readonly shiftKey: boolean;
  readonly metaKey:  boolean;
  readonly repeat:   boolean;
  readonly location  = 0;

  constructor(type: string, init?: KeyboardEventInit) {
    super(type, init);
    this.key      = init?.key      ?? '';
    this.code     = init?.code     ?? '';
    this.altKey   = init?.altKey   ?? false;
    this.ctrlKey  = init?.ctrlKey  ?? false;
    this.shiftKey = init?.shiftKey ?? false;
    this.metaKey  = init?.metaKey  ?? false;
    this.repeat   = init?.repeat   ?? false;
  }

  getModifierState(key: string): boolean {
    switch (key) {
      case 'Alt':     return this.altKey;
      case 'Control': return this.ctrlKey;
      case 'Shift':   return this.shiftKey;
      case 'Meta':    return this.metaKey;
      default:        return false;
    }
  }
}

// ── MouseEvent ────────────────────────────────────────────────────────────────

export interface MouseEventInit extends EventInit {
  clientX?: number;
  clientY?: number;
  pageX?:   number;
  pageY?:   number;
  screenX?: number;
  screenY?: number;
  button?:  number;
  buttons?: number;
  altKey?:   boolean;
  ctrlKey?:  boolean;
  shiftKey?: boolean;
  metaKey?:  boolean;
  relatedTarget?: any;
}

export class MouseEvent extends UIEvent {
  readonly clientX: number;
  readonly clientY: number;
  readonly pageX:   number;
  readonly pageY:   number;
  readonly screenX: number;
  readonly screenY: number;
  readonly button:  number;
  readonly buttons: number;
  readonly altKey:   boolean;
  readonly ctrlKey:  boolean;
  readonly shiftKey: boolean;
  readonly metaKey:  boolean;
  readonly relatedTarget: any;

  constructor(type: string, init?: MouseEventInit) {
    super(type, init);
    this.clientX  = init?.clientX  ?? 0;
    this.clientY  = init?.clientY  ?? 0;
    this.pageX    = init?.pageX    ?? init?.clientX ?? 0;
    this.pageY    = init?.pageY    ?? init?.clientY ?? 0;
    this.screenX  = init?.screenX  ?? 0;
    this.screenY  = init?.screenY  ?? 0;
    this.button   = init?.button   ?? 0;
    this.buttons  = init?.buttons  ?? 0;
    this.altKey   = init?.altKey   ?? false;
    this.ctrlKey  = init?.ctrlKey  ?? false;
    this.shiftKey = init?.shiftKey ?? false;
    this.metaKey  = init?.metaKey  ?? false;
    this.relatedTarget = init?.relatedTarget ?? null;
  }
}

// ── FocusEvent ────────────────────────────────────────────────────────────────

export class FocusEvent extends UIEvent {
  readonly relatedTarget: any;

  constructor(type: string, init?: EventInit & { relatedTarget?: any }) {
    super(type, init);
    this.relatedTarget = init?.relatedTarget ?? null;
  }
}

// ── InputEvent ────────────────────────────────────────────────────────────────

export class InputEvent extends Event {
  readonly data:      string | null;
  readonly inputType: string;

  constructor(type: string, init?: EventInit & { data?: string | null; inputType?: string }) {
    super(type, init);
    this.data      = init?.data      ?? null;
    this.inputType = init?.inputType ?? '';
  }
}
