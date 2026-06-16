// https://dom.spec.whatwg.org/#node

import { EventTarget } from './event-target';
export { NodeHandle } from './native';
import type { NodeHandle } from './native';
import type { DocumentContext } from './document-context';




// Wrap a NodeHandle into the correct Node subclass, reusing existing instances.
export function wrapHandleWith(ctx: DocumentContext, handle: NodeHandle | null): Node | null {
  if (!handle) return null;
  const existing = ctx._handleNodeMap.get(handle);
  if (existing) return existing;
  const type = ctx.nodeType(handle);
  const factory = nodeRegistry.get(type);
  const node = factory ? factory(ctx, handle) : new Node(ctx, handle);
  ctx._handleNodeMap.set(handle, node);
  return node;
}

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
  static readonly DOCUMENT_POSITION_CONTAINS               = POSITION_CONSTANTS.DOCUMENT_POSITION_CONTAINS;
  static readonly DOCUMENT_POSITION_CONTAINED_BY           = POSITION_CONSTANTS.DOCUMENT_POSITION_CONTAINED_BY;
  static readonly DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC = POSITION_CONSTANTS.DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC;

  _handle?: NodeHandle;
  // _docCtx?: DocumentContext;
  get _docCtx(): DocumentContext {
    // @ts-expect-error yuji
    return document._docCtx
  }

  constructor( handle?: NodeHandle) {
    super();
    this._handle = handle;
  }

  protected _wrap(h: NodeHandle | null): Node | null {
    return wrapHandleWith(this._docCtx, h);
  }

  // ── nodeType / nodeName ──────────────────────────────────────────────────

  get nodeType(): number {
    return this._docCtx.nodeType(this._handle!);
  }

  get nodeName(): string {
    switch (this.nodeType) {
      case Node.ELEMENT_NODE:                return this._docCtx.tagName(this._handle!)?.toUpperCase() ?? '';
      case Node.TEXT_NODE:                   return '#text';
      case Node.CDATA_SECTION_NODE:          return '#cdata-section';
      case Node.PROCESSING_INSTRUCTION_NODE: return this._docCtx.nodeValue(this._handle!) ?? '';
      case Node.COMMENT_NODE:                return '#comment';
      case Node.DOCUMENT_NODE:               return '#document';
      case Node.DOCUMENT_FRAGMENT_NODE:      return '#document-fragment';
      default:                               return '';
    }
  }

  // ── Tree traversal ───────────────────────────────────────────────────────

  get parentNode(): Node | null {
    return this._wrap(this._docCtx.parentNode(this._handle));
  }

  get parentElement(): Node | null {
    const h = this._docCtx.parentNode(this._handle);
    if (!h) return null;
    return this._docCtx.nodeType(h) === Node.ELEMENT_NODE ? this._wrap(h) : null;
  }

  get firstChild(): Node | null      { 
    console.log('firstChild', this._handle, this._docCtx)
    const node = this._docCtx.firstChild(this._handle!)
    console.log('firstChild end')
    return node 
  }
  get lastChild(): Node | null       { 
    return this._docCtx.lastChild(this._handle!); 
  }
  get nextSibling(): Node | null     { 
    return this._docCtx.nextSibling(this._handle!); }
  get previousSibling(): Node | null { 
    return this._docCtx.previousSibling(this._handle!); 
  }

  // ── Child list ───────────────────────────────────────────────────────────

  get childNodes(): Node[] {
    const result: Node[] = [];
    let h = this._docCtx.firstChild(this._handle);
    while (h) {
      result.push(this._wrap(h)!);
      h = this._docCtx.nextSibling(h);
    }
    return result;
  }

  hasChildNodes(): boolean {
    return this._docCtx.firstChild(this._handle) !== null;
  }

  // ── nodeValue / textContent ──────────────────────────────────────────────

  get nodeValue(): string | null {
    return this._docCtx.nodeValue(this._handle);
  }

  set nodeValue(value: string | null) {
    this._docCtx.setNodeValue(this._handle, value);
  }

  get textContent(): string | null {
    const t = this.nodeType;
    if (t === Node.DOCUMENT_NODE || t === Node.DOCUMENT_TYPE_NODE) return null;
    if (t === Node.TEXT_NODE || t === Node.CDATA_SECTION_NODE ||
        t === Node.COMMENT_NODE || t === Node.PROCESSING_INSTRUCTION_NODE) {
      return this._docCtx.nodeValue(this._handle);
    }
    return this._collectText(this._handle);
  }

  set textContent(value: string | null) {
    let h = this._docCtx.firstChild(this._handle);
    while (h) {
      const next = this._docCtx.nextSibling(h);
      this._docCtx.removeChild(this._handle, h);
      const child = this._docCtx._handleNodeMap.get(h);
      if (child) this._docCtx._nodes.delete(child);
      h = next;
    }
    if (value) {
      const text = this._docCtx.createTextNode(value);
      this._docCtx.appendChild(this._handle, text);
    }
  }

  private _collectText(h: NodeHandle): string {
    let text = '';
    let child = this._docCtx.firstChild(h);
    while (child) {
      const t = this._docCtx.nodeType(child);
      text += (t === Node.TEXT_NODE || t === Node.CDATA_SECTION_NODE)
        ? (this._docCtx.nodeValue(child) ?? '')
        : this._collectText(child);
      child = this._docCtx.nextSibling(child);
    }
    return text;
  }

  // ── Tree mutation ────────────────────────────────────────────────────────

  appendChild<T extends Node>(node: T): T {
    this._docCtx.appendChild(this._handle, node._handle);
    this._docCtx._nodes.add(node);
    return node;
  }

  removeChild<T extends Node>(child: T): T {
    this._docCtx.removeChild(this._handle, child._handle);
    this._docCtx._nodes.delete(child);
    return child;
  }

  insertBefore<T extends Node>(node: T, refChild: Node | null): T {
    if (refChild === null) {
      this._docCtx.appendChild(this._handle, node._handle);
    } else {
      this._docCtx.insertBefore(this._handle, node._handle, refChild._handle);
    }
    this._docCtx._nodes.add(node);
    return node;
  }

  replaceChild<T extends Node>(node: Node, child: T): T {
    this._docCtx.replaceChild(this._handle, node._handle, child._handle);
    this._docCtx._nodes.add(node);
    this._docCtx._nodes.delete(child);
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

// Copy constants to prototype for instance access (node.ELEMENT_NODE works)
for (const [key, value] of [...Object.entries(NODE_CONSTANTS), ...Object.entries(POSITION_CONSTANTS)]) {
  Object.defineProperty(Node.prototype, key, { value, writable: false, enumerable: true, configurable: false });
}

export { Node };
