// https://dom.spec.whatwg.org/#interface-eventtarget

import { Event } from './event';

interface InternalListener {
  callback: ((...args: any[]) => any) | { handleEvent: (...args: any[]) => any };
  capture:  boolean;
  once:     boolean;
  passive:  boolean;
}

function invokeListeners(target: EventTarget, event: Event, capture: boolean): void {
  const list = target._listeners.get(event.type);
  if (!list) return;
  // Snapshot to handle once-removal and re-entrancy
  for (const entry of [...list]) {
    if (event._immediatePropagationStopped) break;
    if (entry.capture !== capture) continue;
    if (entry.once) {
      const idx = list.indexOf(entry);
      if (idx !== -1) list.splice(idx, 1);
    }
    if (typeof entry.callback === 'function') {
      entry.callback.call(target, event);
    } else {
      entry.callback.handleEvent.call(target, event);
    }
  }
}

export class EventTarget {
  /** @internal — listener storage; public only for cross-file helper access */
  _listeners: Map<string, InternalListener[]> = new Map();

  /**
   * Override in subclasses to provide the bubble target (parent node, shadow host, …).
   * Return null to stop propagation at this target.
   */
  _getParent(): EventTarget | null {
    return null;
  }

  addEventListener(type: string, callback: any, options?: any): void {
    if (!callback) return;
    const capture = typeof options === 'boolean' ? options : (options?.capture ?? false);
    const once    = typeof options === 'object'  ? (options?.once    ?? false) : false;
    const passive = typeof options === 'object'  ? (options?.passive ?? false) : false;

    if (!this._listeners.has(type)) this._listeners.set(type, []);
    const list = this._listeners.get(type)!;
    // Deduplicate: same callback + same capture flag = same slot
    if (!list.some(l => l.callback === callback && l.capture === capture)) {
      list.push({ callback, capture, once, passive });
    }
  }

  removeEventListener(type: string, callback: any, options?: any): void {
    if (!callback) return;
    const capture = typeof options === 'boolean' ? options : (options?.capture ?? false);
    const list = this._listeners.get(type);
    if (!list) return;
    const idx = list.findIndex(l => l.callback === callback && l.capture === capture);
    if (idx !== -1) list.splice(idx, 1);
  }

  dispatchEvent(event: Event): boolean {
    // Build path from this node up to root: [target, parent, …, root]
    const path: EventTarget[] = [];
    let node: EventTarget | null = this;
    while (node) {
      path.push(node);
      node = node._getParent();
    }

    event.target = this;

    // ── Capture phase: root → parent of target ───────────────────────────────
    for (let i = path.length - 1; i > 0; i--) {
      if (event._propagationStopped) break;
      event.currentTarget = path[i];
      event.eventPhase    = Event.CAPTURING_PHASE;
      invokeListeners(path[i], event, true);
    }

    // ── At-target phase: capture and bubble listeners both fire ───────────────
    if (!event._propagationStopped) {
      event.currentTarget = this;
      event.eventPhase    = Event.AT_TARGET;
      invokeListeners(this, event, true);
      if (!event._immediatePropagationStopped) {
        invokeListeners(this, event, false);
      }
    }

    // ── Bubble phase: parent → root ───────────────────────────────────────────
    if (event.bubbles) {
      for (let i = 1; i < path.length; i++) {
        if (event._propagationStopped) break;
        event.currentTarget = path[i];
        event.eventPhase    = Event.BUBBLING_PHASE;
        invokeListeners(path[i], event, false);
      }
    }

    event.currentTarget = null;
    event.eventPhase    = Event.NONE;
    return !event.defaultPrevented;
  }
}
