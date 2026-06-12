import { EventTarget } from './event_target.js';

export const ELEMENT_NODE          = 1;
export const TEXT_NODE             = 3;
export const CDATA_SECTION_NODE    = 4;
export const COMMENT_NODE          = 8;
export const DOCUMENT_NODE         = 9;
export const DOCUMENT_FRAGMENT_NODE = 11;

// Populated by element.ts to avoid circular import
let _ElementClass: new (id: number) => Node = null!;
export function _registerElement(cls: new (id: number) => Node) { _ElementClass = cls; }

export function wrapNode(id: number): Node {
  const type = __nodeType(id);
  if (type === ELEMENT_NODE)  return new _ElementClass(id);
  if (type === TEXT_NODE)     return new Text(id);
  if (type === COMMENT_NODE)  return new Comment(id);
  return new Node(id);
}

export function childIds(id: number): number[] {
  const result: number[] = [];
  let cur = __firstChild(id);
  while (cur != null) { result.push(cur); cur = __nextSibling(cur); }
  return result;
}

export class Node extends EventTarget {
  readonly _id: number;

  static readonly ELEMENT_NODE           = ELEMENT_NODE;
  static readonly TEXT_NODE              = TEXT_NODE;
  static readonly COMMENT_NODE           = COMMENT_NODE;
  static readonly DOCUMENT_NODE          = DOCUMENT_NODE;
  static readonly DOCUMENT_FRAGMENT_NODE = DOCUMENT_FRAGMENT_NODE;

  constructor(id: number) { super(); this._id = id; }

  get nodeType(): number  { return __nodeType(this._id) ?? 0; }
  get nodeName(): string  { return '#unknown'; }

  get nodeValue(): string | null { return __nodeValue(this._id) ?? null; }
  set nodeValue(v: string | null) { if (v != null) __setNodeValue(this._id, v); }

  get parentNode(): Node | null {
    const pid = __parentNode(this._id);
    return pid != null ? wrapNode(pid) : null;
  }
  get parentElement(): import('./element.js').Element | null {
    const p = this.parentNode;
    return p?.nodeType === ELEMENT_NODE ? p as any : null;
  }

  get firstChild(): Node | null {
    const id = __firstChild(this._id); return id != null ? wrapNode(id) : null;
  }
  get lastChild(): Node | null {
    const id = __lastChild(this._id); return id != null ? wrapNode(id) : null;
  }
  get nextSibling(): Node | null {
    const id = __nextSibling(this._id); return id != null ? wrapNode(id) : null;
  }
  get previousSibling(): Node | null {
    const id = __previousSibling(this._id); return id != null ? wrapNode(id) : null;
  }

  get childNodes(): Node[] {
    return childIds(this._id).map(wrapNode);
  }

  get textContent(): string {
    const t = this.nodeType;
    if (t === TEXT_NODE || t === COMMENT_NODE) return this.nodeValue ?? '';
    return childIds(this._id)
      .map(id => wrapNode(id).textContent)
      .join('');
  }
  set textContent(value: string) {
    while (this.firstChild) this.removeChild(this.firstChild);
    if (value) this.appendChild(new Text(__createTextNode(value)));
  }

  hasChildNodes(): boolean { return __firstChild(this._id) != null; }

  appendChild<T extends Node>(child: T): T {
    __appendChild(this._id, child._id);
    return child;
  }
  removeChild<T extends Node>(child: T): T {
    __removeChild(this._id, child._id);
    return child;
  }
  insertBefore<T extends Node>(newNode: T, ref: Node | null): T {
    if (!ref) return this.appendChild(newNode);
    __insertBefore(this._id, newNode._id, ref._id);
    return newNode;
  }
  replaceChild<T extends Node>(newChild: Node, oldChild: T): T {
    this.insertBefore(newChild, oldChild);
    return this.removeChild(oldChild);
  }

  contains(other: Node | null): boolean {
    let cur: Node | null = other;
    while (cur) { if (cur._id === this._id) return true; cur = cur.parentNode; }
    return false;
  }
}

export class Text extends Node {
  get nodeName() { return '#text'; }
  get data(): string { return this.nodeValue ?? ''; }
  set data(v: string) { this.nodeValue = v; }
}

export class Comment extends Node {
  get nodeName() { return '#comment'; }
  get data(): string { return this.nodeValue ?? ''; }
  set data(v: string) { this.nodeValue = v; }
}
