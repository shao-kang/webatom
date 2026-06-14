// https://dom.spec.whatwg.org/#interface-document

import { Node, wrapHandle, registerNodeType } from './node.js';

declare class NodeHandle {
  readonly _id: number;
}

declare const __web_atom_dom: {
  createDocument(): NodeHandle;
  documentElement(docId: number): NodeHandle | null;
  createElement(tag: string): NodeHandle;
  createTextNode(data: string): NodeHandle;
  nodeType(id: number): number;
  tagName(id: number): string;
  getAttribute(id: number, name: string): string | null;
  firstChild(id: number): NodeHandle | null;
  nextSibling(id: number): NodeHandle | null;
  appendChild(parentId: number, childId: number): void;
};

// ---- Document ----------------------------------------------------------------

class Document extends Node {
  /** Create a new document backed by a fresh Rust-side Document. */
  static create(): Document {
    return new Document(__web_atom_dom.createDocument());
  }

  override get nodeType(): number { return Node.DOCUMENT_NODE; }
  override get nodeName(): string { return '#document'; }
  override get ownerDocument(): Document { return this; }

  // --- Core properties ---

  get documentElement(): Node | null {
    return wrapHandle(__web_atom_dom.documentElement(this._id));
  }

  get doctype(): null { return null; }           // not yet exposed by primitives
  get compatMode(): string { return 'CSS1Compat'; }
  get characterSet(): string { return 'UTF-8'; }
  get contentType(): string { return 'text/html'; }
  get URL(): string { return ''; }
  get documentURI(): string { return ''; }

  // HTMLDocument conveniences
  get head(): Node | null { return this._findByTag(this.documentElement, 'head'); }
  get body(): Node | null { return this._findByTag(this.documentElement, 'body'); }

  get title(): string {
    const titleEl = this._findByTag(this.documentElement, 'title');
    return titleEl?.textContent ?? '';
  }
  set title(value: string) {
    const titleEl = this._findByTag(this.documentElement, 'title');
    if (titleEl) titleEl.textContent = value;
  }

  // --- Factory methods ---

  createElement(tagName: string): Node {
    return wrapHandle(__web_atom_dom.createElement(tagName.toLowerCase()))!;
  }

  createTextNode(data: string): Node {
    return wrapHandle(__web_atom_dom.createTextNode(data))!;
  }

  createComment(_data: string): Node {
    // Comment nodes not yet in primitives — create a text node as placeholder
    throw new DOMException('createComment not yet implemented', 'NotSupportedError');
  }

  createDocumentFragment(): Node {
    throw new DOMException('createDocumentFragment not yet implemented', 'NotSupportedError');
  }

  // --- Tree query ---

  getElementById(id: string): Node | null {
    return this._findById(this.documentElement, id);
  }

  getElementsByTagName(tagName: string): Node[] {
    const results: Node[] = [];
    const name = tagName === '*' ? null : tagName.toUpperCase();
    this._collectByTag(this.documentElement, name, results);
    return results;
  }

  getElementsByClassName(classNames: string): Node[] {
    const classes = classNames.trim().split(/\s+/);
    const results: Node[] = [];
    this._collectByClass(this.documentElement, classes, results);
    return results;
  }

  // querySelector / querySelectorAll — placeholder; requires CSS selector engine
  querySelector(_selector: string): Node | null {
    throw new DOMException('querySelector not yet implemented', 'NotSupportedError');
  }
  querySelectorAll(_selector: string): Node[] {
    throw new DOMException('querySelectorAll not yet implemented', 'NotSupportedError');
  }

  // importNode / adoptNode — primitives don't expose cross-document moves yet
  importNode<T extends Node>(node: T, _deep = false): T { return node; }
  adoptNode<T extends Node>(node: T): T { return node; }

  // --- Private helpers ---

  private _findById(root: Node | null, id: string): Node | null {
    if (!root) return null;
    if (__web_atom_dom.getAttribute(root._id, 'id') === id) return root;
    let h = __web_atom_dom.firstChild(root._id);
    while (h) {
      const found = this._findById(wrapHandle(h), id);
      if (found) return found;
      h = __web_atom_dom.nextSibling(h._id);
    }
    return null;
  }

  private _findByTag(root: Node | null, tag: string): Node | null {
    if (!root) return null;
    if (root.nodeType === Node.ELEMENT_NODE &&
        __web_atom_dom.tagName(root._id).toLowerCase() === tag) return root;
    let h = __web_atom_dom.firstChild(root._id);
    while (h) {
      const found = this._findByTag(wrapHandle(h), tag);
      if (found) return found;
      h = __web_atom_dom.nextSibling(h._id);
    }
    return null;
  }

  private _collectByTag(root: Node | null, upperTag: string | null, out: Node[]): void {
    if (!root) return;
    if (root.nodeType === Node.ELEMENT_NODE) {
      if (upperTag === null || __web_atom_dom.tagName(root._id) === upperTag) out.push(root);
    }
    let h = __web_atom_dom.firstChild(root._id);
    while (h) {
      this._collectByTag(wrapHandle(h), upperTag, out);
      h = __web_atom_dom.nextSibling(h._id);
    }
  }

  private _collectByClass(root: Node | null, classes: string[], out: Node[]): void {
    if (!root) return;
    if (root.nodeType === Node.ELEMENT_NODE) {
      const cls = __web_atom_dom.getAttribute(root._id, 'class') ?? '';
      const set = new Set(cls.trim().split(/\s+/));
      if (classes.every(c => set.has(c))) out.push(root);
    }
    let h = __web_atom_dom.firstChild(root._id);
    while (h) {
      this._collectByClass(wrapHandle(h), classes, out);
      h = __web_atom_dom.nextSibling(h._id);
    }
  }
}

// Register Document as the factory for DOCUMENT_NODE
registerNodeType(Node.DOCUMENT_NODE, handle => new Document(handle));

export { Document };
