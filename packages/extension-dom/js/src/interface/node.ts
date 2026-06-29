// https://dom.spec.whatwg.org/#node

import { EventTarget } from './event-target';
export type { NodeHandle } from './native';
import type { NodeHandle } from './native';
import type { DocumentContext } from './document-context';

// ── Constants ──────────────────────────────────────────────────────────────

const NODE_CONSTANTS = {
  ELEMENT_NODE:                1,
  ATTRIBUTE_NODE:              2,
  TEXT_NODE:                   3,
  CDATA_SECTION_NODE:          4,
  PROCESSING_INSTRUCTION_NODE: 7,
  COMMENT_NODE:                8,
  DOCUMENT_NODE:               9,
  DOCUMENT_TYPE_NODE:          10,
  DOCUMENT_FRAGMENT_NODE:      11,
} as const;

const POSITION_CONSTANTS = {
  DOCUMENT_POSITION_DISCONNECTED:            0x01,
  DOCUMENT_POSITION_PRECEDING:               0x02,
  DOCUMENT_POSITION_FOLLOWING:               0x04,
  DOCUMENT_POSITION_CONTAINS:               0x08,
  DOCUMENT_POSITION_CONTAINED_BY:           0x10,
  DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC: 0x20,
} as const;

// ── Node ───────────────────────────────────────────────────────────────────

class Node extends EventTarget {
  static readonly ELEMENT_NODE                  = NODE_CONSTANTS.ELEMENT_NODE;
  static readonly ATTRIBUTE_NODE                = NODE_CONSTANTS.ATTRIBUTE_NODE;
  static readonly TEXT_NODE                     = NODE_CONSTANTS.TEXT_NODE;
  static readonly CDATA_SECTION_NODE            = NODE_CONSTANTS.CDATA_SECTION_NODE;
  static readonly PROCESSING_INSTRUCTION_NODE   = NODE_CONSTANTS.PROCESSING_INSTRUCTION_NODE;
  static readonly COMMENT_NODE                  = NODE_CONSTANTS.COMMENT_NODE;
  static readonly DOCUMENT_NODE                 = NODE_CONSTANTS.DOCUMENT_NODE;
  static readonly DOCUMENT_TYPE_NODE            = NODE_CONSTANTS.DOCUMENT_TYPE_NODE;
  static readonly DOCUMENT_FRAGMENT_NODE        = NODE_CONSTANTS.DOCUMENT_FRAGMENT_NODE;
  static readonly DOCUMENT_POSITION_DISCONNECTED            = POSITION_CONSTANTS.DOCUMENT_POSITION_DISCONNECTED;
  static readonly DOCUMENT_POSITION_PRECEDING               = POSITION_CONSTANTS.DOCUMENT_POSITION_PRECEDING;
  static readonly DOCUMENT_POSITION_FOLLOWING               = POSITION_CONSTANTS.DOCUMENT_POSITION_FOLLOWING;
  static readonly DOCUMENT_POSITION_CONTAINS                = POSITION_CONSTANTS.DOCUMENT_POSITION_CONTAINS;
  static readonly DOCUMENT_POSITION_CONTAINED_BY            = POSITION_CONSTANTS.DOCUMENT_POSITION_CONTAINED_BY;
  static readonly DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC = POSITION_CONSTANTS.DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC;

  _ctx: DocumentContext;
  _handle: NodeHandle;

  constructor(ctx: DocumentContext, handle: NodeHandle) {
    super();
    this._ctx = ctx;
    this._handle = handle;
  }

  override _getParent() {
    return this._ctx.parentNode(this._handle);
  }

  // ── nodeType / nodeName ──────────────────────────────────────────────────

  get nodeType(): number {
    return this._ctx.nodeType(this._handle);
  }

  get nodeName(): string {
    switch (this.nodeType) {
      case Node.ELEMENT_NODE:                return this._ctx.tagName(this._handle)?.toUpperCase() ?? '';
      case Node.TEXT_NODE:                   return '#text';
      case Node.CDATA_SECTION_NODE:          return '#cdata-section';
      case Node.PROCESSING_INSTRUCTION_NODE: return this._ctx.nodeValue(this._handle) ?? '';
      case Node.COMMENT_NODE:                return '#comment';
      case Node.DOCUMENT_NODE:               return '#document';
      case Node.DOCUMENT_FRAGMENT_NODE:      return '#document-fragment';
      default:                               return '';
    }
  }

  // ── Tree traversal ───────────────────────────────────────────────────────

  get parentNode(): Node | null      { return this._ctx.parentNode(this._handle); }
  get parentElement(): Node | null {
    const p = this._ctx.parentNode(this._handle);
    return p?.nodeType === Node.ELEMENT_NODE ? p : null;
  }
  get firstChild(): Node | null      { return this._ctx.firstChild(this._handle); }
  get lastChild(): Node | null       { return this._ctx.lastChild(this._handle); }
  get nextSibling(): Node | null     { return this._ctx.nextSibling(this._handle); }
  get previousSibling(): Node | null { return this._ctx.previousSibling(this._handle); }

  // ── Child list ───────────────────────────────────────────────────────────

  get childNodes(): Node[] {
    const result: Node[] = [];
    let child = this._ctx.firstChild(this._handle);
    while (child) {
      result.push(child);
      child = this._ctx.nextSibling(child._handle);
    }
    return result;
  }

  hasChildNodes(): boolean {
    return this._ctx.firstChild(this._handle) !== null;
  }

  // ── nodeValue / textContent ──────────────────────────────────────────────

  get nodeValue(): string | null {
    return this._ctx.nodeValue(this._handle);
  }

  set nodeValue(value: string | null) {
    this._ctx.setNodeValue(this._handle, value);
  }

  get textContent(): string | null {
    const t = this.nodeType;
    if (t === Node.DOCUMENT_NODE || t === Node.DOCUMENT_TYPE_NODE) return null;
    if (t === Node.TEXT_NODE || t === Node.CDATA_SECTION_NODE ||
        t === Node.COMMENT_NODE || t === Node.PROCESSING_INSTRUCTION_NODE) {
      return this._ctx.nodeValue(this._handle);
    }
    return this._collectText(this._handle);
  }

  set textContent(value: string | null) {
    let child = this._ctx.firstChild(this._handle);
    while (child) {
      const next = this._ctx.nextSibling(child._handle);
      this._ctx.removeChild(this._handle, child._handle);
      child = next;
    }
    if (value) {
      const textHandle = this._ctx.createTextNode(value);
      this._ctx.appendChild(this._handle, textHandle);
    }
  }

  private _collectText(h: NodeHandle): string {
    let text = '';
    let child = this._ctx.firstChild(h);
    while (child) {
      const t = child.nodeType;
      text += (t === Node.TEXT_NODE || t === Node.CDATA_SECTION_NODE)
        ? (child.nodeValue ?? '')
        : this._collectText(child._handle);
      child = this._ctx.nextSibling(child._handle);
    }
    return text;
  }

  // ── Tree mutation ────────────────────────────────────────────────────────

  appendChild<T extends Node>(node: T): T {
    this._ctx.appendChild(this._handle, node._handle);
    return node;
  }

  removeChild<T extends Node>(child: T): T {
    this._ctx.removeChild(this._handle, child._handle);
    return child;
  }

  insertBefore<T extends Node>(node: T, refChild: Node | null): T {
    if (refChild === null) {
      this._ctx.appendChild(this._handle, node._handle);
    } else {
      this._ctx.insertBefore(this._handle, node._handle, refChild._handle);
    }
    return node;
  }

  replaceChild<T extends Node>(node: Node, child: T): T {
    this._ctx.replaceChild(this._handle, node._handle, child._handle);
    return child;
  }

  // ── Utility ──────────────────────────────────────────────────────────────

  getRootNode(_options?: GetRootNodeOptions): Node {
    let root: Node = this;
    let p = this.parentNode;
    while (p) { root = p; p = p.parentNode; }
    return root;
  }

  contains(other: Node | null): boolean {
    let n: Node | null = other;
    while (n) {
      if (n._handle === this._handle) return true;
      n = n.parentNode;
    }
    return false;
  }

  isSameNode(other: Node | null): boolean {
    return other !== null && other._handle === this._handle;
  }

  isEqualNode(other: Node | null): boolean {
    if (!other) return false;
    if (other._handle === this._handle) return true;
    if (this.nodeType !== other.nodeType || this.nodeName !== other.nodeName ||
        this.nodeValue !== other.nodeValue) return false;
    const a = this.childNodes, b = other.childNodes;
    if (a.length !== b.length) return false;
    return a.every((c, i) => c.isEqualNode(b[i]));
  }

  normalize(): void {
    let child = this.firstChild;
    while (child) {
      if (child.nodeType === Node.TEXT_NODE) {
        const next = child.nextSibling;
        if (!child.nodeValue) {
          this.removeChild(child);
          child = next;
          continue;
        }
        if (next?.nodeType === Node.TEXT_NODE) {
          child.nodeValue = (child.nodeValue ?? '') + (next.nodeValue ?? '');
          this.removeChild(next);
          continue;
        }
      } else {
        child.normalize();
      }
      child = child.nextSibling;
    }
  }

  compareDocumentPosition(other: Node): number {
    if (other._handle === this._handle) return 0;
    if (this.contains(other))
      return Node.DOCUMENT_POSITION_CONTAINED_BY | Node.DOCUMENT_POSITION_FOLLOWING;
    if (other.contains(this))
      return Node.DOCUMENT_POSITION_CONTAINS | Node.DOCUMENT_POSITION_PRECEDING;
    return Node.DOCUMENT_POSITION_DISCONNECTED;
  }

  cloneNode(_deep = false): Node {
    throw new DOMException('cloneNode not yet implemented', 'NotSupportedError');
  }

  lookupPrefix(_namespace: string | null): string | null { return null; }
  lookupNamespaceURI(_prefix: string | null): string | null { return null; }
  isDefaultNamespace(_namespace: string | null): boolean { return false; }

  get ownerDocument(): Node | null { return null; }
  get isConnected(): boolean { return false; }
  get baseURI(): string { return ''; }
}

for (const [key, value] of [...Object.entries(NODE_CONSTANTS), ...Object.entries(POSITION_CONSTANTS)]) {
  Object.defineProperty(Node.prototype, key, { value, writable: false, enumerable: true, configurable: false });
}

export { Node };
