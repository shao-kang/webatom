type EventListenerFn = (event: Event) => void;

export class Event {
  readonly type: string;
  readonly bubbles: boolean;
  readonly cancelable: boolean;
  readonly timeStamp: number;
  defaultPrevented = false;
  target: EventTarget | null = null;
  currentTarget: EventTarget | null = null;

  constructor(type: string, options?: EventInit) {
    this.type = type;
    this.bubbles = options?.bubbles ?? false;
    this.cancelable = options?.cancelable ?? false;
    this.timeStamp = Date.now();
  }

  preventDefault() { if (this.cancelable) this.defaultPrevented = true; }
  stopPropagation() {}
  stopImmediatePropagation() {}
}

export class CustomEvent extends Event {
  readonly detail: unknown;
  constructor(type: string, options?: CustomEventInit) {
    super(type, options);
    this.detail = (options as any)?.detail ?? null;
  }
}

export class MouseEvent extends Event {
  readonly clientX: number;
  readonly clientY: number;
  readonly button: number;
  readonly buttons: number;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
  readonly metaKey: boolean;

  constructor(type: string, options?: MouseEventInit) {
    super(type, options);
    this.clientX  = (options as any)?.clientX  ?? 0;
    this.clientY  = (options as any)?.clientY  ?? 0;
    this.button   = (options as any)?.button   ?? 0;
    this.buttons  = (options as any)?.buttons  ?? 0;
    this.ctrlKey  = (options as any)?.ctrlKey  ?? false;
    this.shiftKey = (options as any)?.shiftKey ?? false;
    this.altKey   = (options as any)?.altKey   ?? false;
    this.metaKey  = (options as any)?.metaKey  ?? false;
  }
}

export class KeyboardEvent extends Event {
  readonly key: string;
  readonly code: string;
  readonly keyCode: number;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
  readonly metaKey: boolean;
  readonly repeat: boolean;

  constructor(type: string, options?: KeyboardEventInit) {
    super(type, options);
    this.key      = (options as any)?.key      ?? '';
    this.code     = (options as any)?.code     ?? '';
    this.keyCode  = (options as any)?.keyCode  ?? 0;
    this.ctrlKey  = (options as any)?.ctrlKey  ?? false;
    this.shiftKey = (options as any)?.shiftKey ?? false;
    this.altKey   = (options as any)?.altKey   ?? false;
    this.metaKey  = (options as any)?.metaKey  ?? false;
    this.repeat   = (options as any)?.repeat   ?? false;
  }
}

export class EventTarget {
  private _listeners = new Map<string, EventListenerFn[]>();

  addEventListener(type: string, listener: EventListenerFn): void {
    let list = this._listeners.get(type);
    if (!list) { list = []; this._listeners.set(type, list); }
    if (!list.includes(listener)) list.push(listener);
  }

  removeEventListener(type: string, listener: EventListenerFn): void {
    const list = this._listeners.get(type);
    if (!list) return;
    const idx = list.indexOf(listener);
    if (idx !== -1) list.splice(idx, 1);
  }

  dispatchEvent(event: Event): boolean {
    (event as any).currentTarget = this;
    const list = this._listeners.get(event.type)?.slice() ?? [];
    for (const fn of list) fn(event);
    return !event.defaultPrevented;
  }
}
