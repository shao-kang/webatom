// https://dom.spec.whatwg.org/#document

import { Node } from './node';
import type { NodeHandle } from './native';
import { DocumentContext } from './document-context';

export class Document extends Node {
  constructor() {
    const ctx = new DocumentContext();
    const docNode = ctx.documentNode();
    super(ctx, docNode);
    // Register the document node so wrapHandleWith returns this instance
    ctx._attachRoot(this);
  }

  override get ownerDocument(): null {
    return null;
  }

  // ── Document tree ────────────────────────────────────────────────────────

  get documentElement(): Node | null {
    return this._ctx.documentElement();
  }

  get body(): Node | null {
    return this._ctx.body();
  }

  get head(): Node | null {
    return this._ctx.head();
  }

  // ── Node creation ────────────────────────────────────────────────────────

  createElement(tagName: string): Node {
    return this._ctx.wrap(this._ctx.createElement(tagName))!;
  }

  createTextNode(data: string): Node {
    return this._ctx.wrap(this._ctx.createTextNode(data))!;
  }

  createComment(data: string): Node {
    return this._ctx.wrap(this._ctx.createComment(data))!;
  }

  // ── Query ────────────────────────────────────────────────────────────────

  getElementById(id: string): Node | null {
    return this._findById(this._ctx._docHandle.firstChild(this._handle), id);
  }

  private _findById(h: NodeHandle | null, id: string): Node | null {
    while (h) {
      if (this._ctx.nodeType(h) === Node.ELEMENT_NODE) {
        if (this._ctx.getAttribute(h, 'id') === id) {
          return this._ctx.wrap(h);
        }
        const found = this._findById(this._ctx._docHandle.firstChild(h), id);
        if (found) return found;
      }
      h = this._ctx._docHandle.nextSibling(h);
    }
    return null;
  }
}
