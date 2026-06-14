// https://dom.spec.whatwg.org/#interface-eventtarget

type ListenerEntry = EventListener | EventListenerObject;
type ListenerOptions = boolean | AddEventListenerOptions | undefined;
type ListenerMap = Map<string, Map<ListenerEntry, ListenerOptions>>;

const wm = new WeakMap<object, ListenerMap>();

function dispatch(event: Event, listener: EventListenerObject | EventListener): boolean {
  if (typeof listener === 'function')
    listener.call(event.target, event);
  else
    listener.handleEvent(event);
  return (event as any).stopImmediatePropagation;
}

interface PathEntry {
  currentTarget: object;
  target: object;
}

function invokeListeners(this: Event, { currentTarget, target }: PathEntry): boolean | undefined {
  const map = wm.get(currentTarget);
  if (map && map.has(this.type)) {
    const listeners = map.get(this.type)!;
    const ev = this as any;
    if (currentTarget === target) {
      ev.eventPhase = ev.AT_TARGET;
    } else {
      ev.eventPhase = ev.BUBBLING_PHASE;
    }

    ev.currentTarget = currentTarget;
    ev.target = target;
    for (const [listener, options] of listeners) {
      if (options && typeof options !== 'boolean' && options.once)
        listeners.delete(listener);
      if (dispatch(this, listener))
        break;
    }
    delete ev.currentTarget;
    delete ev.target;
    return ev.cancelBubble as boolean;
  }
}


/**
 * @implements globalThis.EventTarget
 */
class DOMEventTarget {

  constructor() {
    wm.set(this, new Map());
  }

  /**
   * @protected
   */
  _getParent(): DOMEventTarget | null {
    return null;
  }

  addEventListener(
    type: string,
    listener: ListenerEntry | null,
    options?: boolean | AddEventListenerOptions,
  ): void {
    if (!listener) return;
    const map = wm.get(this)!;
    if (!map.has(type))
      map.set(type, new Map());
    map.get(type)!.set(listener, options);
  }

  removeEventListener(
    type: string,
    listener: ListenerEntry | null,
    _options?: boolean | EventListenerOptions,
  ): void {
    if (!listener) return;
    const map = wm.get(this)!;
    if (map.has(type)) {
      const listeners = map.get(type)!;
      if (listeners.delete(listener) && !listeners.size)
        map.delete(type);
    }
  }

  dispatchEvent(event: Event): boolean {
    let node: DOMEventTarget | null = this;
    (event as any).eventPhase = (event as any).CAPTURING_PHASE;

    // intentionally simplified, specs imply way more code: https://dom.spec.whatwg.org/#event-path
    while (node) {
      if ((node as any).dispatchEvent)
        (event as any)._path.push({ currentTarget: node, target: this });
      node = event.bubbles && node._getParent ? node._getParent() : null;
    }
    (event as any)._path.some(invokeListeners, event);
    (event as any)._path = [];
    (event as any).eventPhase = (event as any).NONE;
    return !event.defaultPrevented;
  }

}

export { DOMEventTarget as EventTarget };
