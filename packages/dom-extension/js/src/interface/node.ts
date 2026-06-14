// https://dom.spec.whatwg.org/#node

import { EventTarget } from './event-target.js';

// Native class registered by Rust — only carries the opaque node id
declare class NodeHandle {
  readonly _id: number;
}

// Minimal Rust primitives — everything else is implemented in JS
declare const __web_atom_dom: {
  nodeType(id: number): number;
  tagName(id: number): string;
  nodeValue(id: number): string | null;
  setNodeValue(id: number, value: string | null): void;
  parentNode(id: number): NodeHandle | null;
  firstChild(id: number): NodeHandle | null;
  nextSibling(id: number): NodeHandle | null;
  appendChild(parentId: number, childId: number): void;
  removeChild(parentId: number, childId: number): void;
  insertBefore(parentId: number, newId: number, refId: number): void;
  createTextNode(data: string): NodeHandle;
};

// Subclasses register factories here so wrapHandle returns the right subclass.
// Key = nodeType constant value.
type NodeFactory = (handle: NodeHandle) => Node;
const nodeRegistry = new Map<number, NodeFactory>();

export function registerNodeType(nodeType: number, factory: NodeFactory): void {
  nodeRegistry.set(nodeType, factory);
}

export function wrapHandle(handle: NodeHandle | null): Node | null {
  if (handle === null) return null;
  const type = __web_atom_dom.nodeType(handle._id);
  const factory = nodeRegistry.get(type);
  return factory ? factory(handle) : new Node(handle);
}

// Node type constants (also placed on the prototype below for instance access)
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

class Node extends EventTarget {
  // Static constants — access via Node.ELEMENT_NODE etc.
  static readonly ELEMENT_NODE                 = NODE_CONSTANTS.ELEMENT_NODE;
  static readonly ATTRIBUTE_NODE               = NODE_CONSTANTS.ATTRIBUTE_NODE;
  static readonly TEXT_NODE                    = NODE_CONSTANTS.TEXT_NODE;
  static readonly CDATA_SECTION_NODE           = NODE_CONSTANTS.CDATA_SECTION_NODE;
  static readonly PROCESSING_INSTRUCTION_NODE  = NODE_CONSTANTS.PROCESSING_INSTRUCTION_NODE;
  static readonly COMMENT_NODE                 = NODE_CONSTANTS.COMMENT_NODE;
  static readonly DOCUMENT_NODE               = NODE_CONSTANTS.DOCUMENT_NODE;
  static readonly DOCUMENT_TYPE_NODE           = NODE_CONSTANTS.DOCUMENT_TYPE_NODE;
  static readonly DOCUMENT_FRAGMENT_NODE       = NODE_CONSTANTS.DOCUMENT_FRAGMENT_NODE;
  static readonly DOCUMENT_POSITION_DISCONNECTED            = POSITION_CONSTANTS.DOCUMENT_POSITION_DISCONNECTED;
  static readonly DOCUMENT_POSITION_PRECEDING               = POSITION_CONSTANTS.DOCUMENT_POSITION_PRECEDING;
  static readonly DOCUMENT_POSITION_FOLLOWING               = POSITION_CONSTANTS.DOCUMENT_POSITION_FOLLOWING;
  static readonly DOCUMENT_POSITION_CONTAINS               = POSITION_CONSTANTS.DOCUMENT_POSITION_CONTAINS;
  static readonly DOCUMENT_POSITION_CONTAINED_BY           = POSITION_CONSTANTS.DOCUMENT_POSITION_CONTAINED_BY;
  static readonly DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC = POSITION_CONSTANTS.DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC;

  protected _handle: NodeHandle;

  constructor(handle: NodeHandle) {
    super();
    this._handle = handle;
  }

  get _id(): number {
    return this._handle._id;
  }

  // --- nodeType / nodeName ---

  get nodeType(): number {
    return __web_atom_dom.nodeType(this._id);
  }

  get nodeName(): string {
    switch (this.nodeType) {
      case Node.ELEMENT_NODE:                return __web_atom_dom.tagName(this._id).toUpperCase();
      case Node.TEXT_NODE:                   return '#text';
      case Node.CDATA_SECTION_NODE:          return '#cdata-section';
      case Node.PROCESSING_INSTRUCTION_NODE: return __web_atom_dom.nodeValue(this._id) ?? '';
      case Node.COMMENT_NODE:               return '#comment';
      case Node.DOCUMENT_NODE:              return '#document';
      case Node.DOCUMENT_FRAGMENT_NODE:     return '#document-fragment';
      default:                              return '';
    }
  }

  // --- Tree traversal ---

  get parentNode(): Node | null {
    return wrapHandle(__web_atom_dom.parentNode(this._id));
  }

  get parentElement(): Node | null {
    const p = __web_atom_dom.parentNode(this._id);
    if (!p) return null;
    return __web_atom_dom.nodeType(p._id) === Node.ELEMENT_NODE ? wrapHandle(p) : null;
  }

  get firstChild(): Node | null {
    return wrapHandle(__web_atom_dom.firstChild(this._id));
  }

  get lastChild(): Node | null {
    let h = __web_atom_dom.firstChild(this._id);
    if (!h) return null;
    let next: NodeHandle | null;
    while ((next = __web_atom_dom.nextSibling(h._id)) !== null) h = next;
    return wrapHandle(h);
  }

  get nextSibling(): Node | null {
    return wrapHandle(__web_atom_dom.nextSibling(this._id));
  }

  get previousSibling(): Node | null {
    const parent = __web_atom_dom.parentNode(this._id);
    if (!parent) return null;
    let h = __web_atom_dom.firstChild(parent._id);
    if (!h || h._id === this._id) return null;
    let prev = h;
    let next: NodeHandle | null;
    while ((next = __web_atom_dom.nextSibling(prev._id)) !== null) {
      if (next._id === this._id) return wrapHandle(prev);
      prev = next;
    }
    return null;
  }

  // --- Child list ---

  get childNodes(): Node[] {
    const result: Node[] = [];
    let h = __web_atom_dom.firstChild(this._id);
    while (h) {
      result.push(wrapHandle(h)!);
      h = __web_atom_dom.nextSibling(h._id);
    }
    return result;
  }

  hasChildNodes(): boolean {
    return __web_atom_dom.firstChild(this._id) !== null;
  }

  // --- nodeValue / textContent ---

  get nodeValue(): string | null {
    return __web_atom_dom.nodeValue(this._id);
  }

  set nodeValue(value: string | null) {
    __web_atom_dom.setNodeValue(this._id, value);
  }

  get textContent(): string | null {
    const t = this.nodeType;
    if (t === Node.DOCUMENT_NODE || t === Node.DOCUMENT_TYPE_NODE) return null;
    if (t === Node.TEXT_NODE || t === Node.CDATA_SECTION_NODE ||
        t === Node.COMMENT_NODE || t === Node.PROCESSING_INSTRUCTION_NODE) {
      return __web_atom_dom.nodeValue(this._id);
    }
    return this._collectText(this._id);
  }

  set textContent(value: string | null) {
    let h = __web_atom_dom.firstChild(this._id);
    while (h) {
      const next = __web_atom_dom.nextSibling(h._id);
      __web_atom_dom.removeChild(this._id, h._id);
      h = next;
    }
    if (value) {
      const text = __web_atom_dom.createTextNode(value);
      __web_atom_dom.appendChild(this._id, text._id);
    }
  }

  private _collectText(id: number): string {
    let text = '';
    let h = __web_atom_dom.firstChild(id);
    while (h) {
      const t = __web_atom_dom.nodeType(h._id);
      text += (t === Node.TEXT_NODE || t === Node.CDATA_SECTION_NODE)
        ? (__web_atom_dom.nodeValue(h._id) ?? '')
        : this._collectText(h._id);
      h = __web_atom_dom.nextSibling(h._id);
    }
    return text;
  }

  // --- Tree mutation ---

  appendChild<T extends Node>(node: T): T {
    __web_atom_dom.appendChild(this._id, node._id);
    return node;
  }

  removeChild<T extends Node>(child: T): T {
    __web_atom_dom.removeChild(this._id, child._id);
    return child;
  }

  insertBefore<T extends Node>(node: T, refChild: Node | null): T {
    if (refChild === null) {
      __web_atom_dom.appendChild(this._id, node._id);
    } else {
      __web_atom_dom.insertBefore(this._id, node._id, refChild._id);
    }
    return node;
  }

  replaceChild<T extends Node>(node: Node, child: T): T {
    __web_atom_dom.insertBefore(this._id, node._id, child._id);
    __web_atom_dom.removeChild(this._id, child._id);
    return child;
  }

  // --- Utility ---

  getRootNode(_options?: GetRootNodeOptions): Node {
    let root: Node = this;
    let p = this.parentNode;
    while (p) { root = p; p = p.parentNode; }
    return root;
  }

  contains(other: Node | null): boolean {
    let n: Node | null = other;
    while (n) {
      if (n._id === this._id) return true;
      n = n.parentNode;
    }
    return false;
  }

  isSameNode(other: Node | null): boolean {
    return other !== null && other._id === this._id;
  }

  isEqualNode(other: Node | null): boolean {
    if (!other) return false;
    if (this._id === other._id) return true;
    if (this.nodeType !== other.nodeType || this.nodeName !== other.nodeName ||
        this.nodeValue !== other.nodeValue) return false;
    const a = this.childNodes;
    const b = other.childNodes;
    if (a.length !== b.length) return false;
    return a.every((child, i) => child.isEqualNode(b[i]));
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
    if (this._id === other._id) return 0;
    if (this.contains(other))
      return Node.DOCUMENT_POSITION_CONTAINED_BY | Node.DOCUMENT_POSITION_FOLLOWING;
    if (other.contains(this))
      return Node.DOCUMENT_POSITION_CONTAINS | Node.DOCUMENT_POSITION_PRECEDING;
    return Node.DOCUMENT_POSITION_DISCONNECTED;
  }

  cloneNode(_deep = false): Node {
    throw new DOMException('cloneNode not yet implemented', 'NotSupportedError');
  }

  // Namespace methods — no namespace support in current primitives
  lookupPrefix(_namespace: string | null): string | null { return null; }
  lookupNamespaceURI(_prefix: string | null): string | null { return null; }
  isDefaultNamespace(_namespace: string | null): boolean { return false; }

  // To be refined by Document/Element
  get ownerDocument(): Node | null { return null; }
  get isConnected(): boolean { return false; }
  get baseURI(): string { return ''; }
}

// Place constants on the prototype so `node.ELEMENT_NODE` works (W3C compat)
for (const [key, value] of [...Object.entries(NODE_CONSTANTS), ...Object.entries(POSITION_CONSTANTS)]) {
  Object.defineProperty(Node.prototype, key, { value, writable: false, enumerable: true, configurable: false });
}

export { Node };
