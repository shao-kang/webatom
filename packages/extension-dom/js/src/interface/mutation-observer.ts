// https://dom.spec.whatwg.org/#mutationobserver

import type { Node } from './node';

// ── Types ──────────────────────────────────────────────────────────────────

type MutationRecordType = 'childList' | 'attributes' | 'characterData';
type MutationCallback = (records: MutationRecord[], observer: MutationObserver) => void;

export interface MutationObserverInit {
  childList?: boolean;
  attributes?: boolean;
  characterData?: boolean;
  subtree?: boolean;
  attributeOldValue?: boolean;
  characterDataOldValue?: boolean;
  attributeFilter?: string[];
}

export type ObservationEntry = {
  observer: MutationObserver;
  options: MutationObserverInit;
};

// ── Microtask flush ────────────────────────────────────────────────────────

let subtreeObserverCount = 0;
const pendingObservers: MutationObserver[] = [];
let flushScheduled = false;

function scheduleMicrotaskFlush(): void {
  if (flushScheduled) return;
  flushScheduled = true;
  Promise.resolve().then(flushAll);
}

function flushAll(): void {
  flushScheduled = false;
  const batch = pendingObservers.splice(0);
  for (const obs of batch) {
    const records = obs.takeRecords();
    if (records.length > 0) obs._callback(records, obs);
  }
}

// ── MutationRecord ─────────────────────────────────────────────────────────

export class MutationRecord {
  readonly type: MutationRecordType;
  readonly target: Node;
  readonly addedNodes: Node[];
  readonly removedNodes: Node[];
  readonly previousSibling: Node | null;
  readonly nextSibling: Node | null;
  readonly attributeName: string | null;
  readonly attributeNamespace: string | null = null;
  readonly oldValue: string | null;

  constructor(init: {
    type: MutationRecordType;
    target: Node;
    addedNodes?: Node[];
    removedNodes?: Node[];
    previousSibling?: Node | null;
    nextSibling?: Node | null;
    attributeName?: string | null;
    oldValue?: string | null;
  }) {
    this.type            = init.type;
    this.target          = init.target;
    this.addedNodes      = init.addedNodes   ?? [];
    this.removedNodes    = init.removedNodes ?? [];
    this.previousSibling = init.previousSibling ?? null;
    this.nextSibling     = init.nextSibling     ?? null;
    this.attributeName   = init.attributeName   ?? null;
    this.oldValue        = init.oldValue        ?? null;
  }
}

// ── MutationObserver ───────────────────────────────────────────────────────

export class MutationObserver {
  _callback: MutationCallback;
  private _observations: Map<Node, MutationObserverInit> = new Map();
  private _records: MutationRecord[] = [];

  constructor(callback: MutationCallback) {
    this._callback = callback;
  }

  observe(target: Node, options: MutationObserverInit = {}): void {
    // Per spec: implicitly enable attributes/characterData when related option is set
    if ((options.attributeOldValue != null || options.attributeFilter != null) && options.attributes == null) {
      options = { ...options, attributes: true };
    }
    if (options.characterDataOldValue != null && options.characterData == null) {
      options = { ...options, characterData: true };
    }
    if (!options.childList && !options.attributes && !options.characterData) {
      throw new TypeError('MutationObserver.observe: at least one of childList, attributes, or characterData must be true');
    }

    const nodeEntries = (target as any)._mutationObservers as ObservationEntry[] | undefined;
    const existingIdx = nodeEntries?.findIndex(e => e.observer === this) ?? -1;

    if (existingIdx >= 0) {
      // Update existing registration, maintain subtreeObserverCount
      const oldOpts = nodeEntries![existingIdx].options;
      if (oldOpts.subtree && !options.subtree) subtreeObserverCount--;
      else if (!oldOpts.subtree && options.subtree) subtreeObserverCount++;
      nodeEntries![existingIdx].options = options;
    } else {
      const entry: ObservationEntry = { observer: this, options };
      if (nodeEntries) {
        nodeEntries.push(entry);
      } else {
        (target as any)._mutationObservers = [entry];
      }
      if (options.subtree) subtreeObserverCount++;
    }

    this._observations.set(target, options);
  }

  disconnect(): void {
    for (const [target, options] of this._observations) {
      if (options.subtree) subtreeObserverCount--;
      const list = (target as any)._mutationObservers as ObservationEntry[] | undefined;
      if (list) {
        const idx = list.findIndex(e => e.observer === this);
        if (idx >= 0) list.splice(idx, 1);
        if (list.length === 0) delete (target as any)._mutationObservers;
      }
    }
    this._observations.clear();
    this._records = [];
  }

  takeRecords(): MutationRecord[] {
    const records = this._records;
    this._records = [];
    return records;
  }

  _enqueue(record: MutationRecord): void {
    this._records.push(record);
    if (!pendingObservers.includes(this)) pendingObservers.push(this);
    scheduleMicrotaskFlush();
  }
}

// ── Internal dispatch ──────────────────────────────────────────────────────

function forEachMatchingObserver(
  target: Node,
  type: MutationRecordType,
  cb: (observer: MutationObserver, options: MutationObserverInit) => void,
): void {
  const direct = (target as any)._mutationObservers as ObservationEntry[] | undefined;
  if (direct) {
    for (const { observer, options } of direct) {
      if (typeMatches(options, type)) cb(observer, options);
    }
  }
  if (subtreeObserverCount > 0) {
    let anc = (target as any).parentNode as Node | null;
    while (anc) {
      const entries = (anc as any)._mutationObservers as ObservationEntry[] | undefined;
      if (entries) {
        for (const { observer, options } of entries) {
          if (options.subtree && typeMatches(options, type)) cb(observer, options);
        }
      }
      anc = (anc as any).parentNode as Node | null;
    }
  }
}

function typeMatches(options: MutationObserverInit, type: MutationRecordType): boolean {
  return (type === 'childList'      && !!options.childList)
      || (type === 'attributes'     && !!options.attributes)
      || (type === 'characterData'  && !!options.characterData);
}

// ── Public notification functions (called by Node / Element) ───────────────

export function notifyChildList(
  target: Node,
  addedNodes: Node[],
  removedNodes: Node[],
  previousSibling: Node | null,
  nextSibling: Node | null,
): void {
  if (!(target as any)._mutationObservers && subtreeObserverCount === 0) return;
  // childList records carry no per-observer variation: one shared record
  const record = new MutationRecord({ type: 'childList', target, addedNodes, removedNodes, previousSibling, nextSibling });
  forEachMatchingObserver(target, 'childList', (observer) => observer._enqueue(record));
}

export function notifyAttribute(
  target: Node,
  attributeName: string,
  oldValue: string | null,
): void {
  if (!(target as any)._mutationObservers && subtreeObserverCount === 0) return;
  forEachMatchingObserver(target, 'attributes', (observer, options) => {
    if (options.attributeFilter && !options.attributeFilter.includes(attributeName)) return;
    observer._enqueue(new MutationRecord({
      type: 'attributes',
      target,
      attributeName,
      oldValue: options.attributeOldValue ? oldValue : null,
    }));
  });
}

export function notifyCharacterData(
  target: Node,
  oldValue: string | null,
): void {
  if (!(target as any)._mutationObservers && subtreeObserverCount === 0) return;
  forEachMatchingObserver(target, 'characterData', (observer, options) => {
    observer._enqueue(new MutationRecord({
      type: 'characterData',
      target,
      oldValue: options.characterDataOldValue ? oldValue : null,
    }));
  });
}
