// https://dom.spec.whatwg.org/#document

import { Node } from './node';
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
    return this._findById(this._ctx._docHandle.firstChild(this._handle.nodeId), id);
  }

  private _findById(nodeId: number | null, id: string): Node | null {
    while (nodeId !== null) {
      if (this._ctx._docHandle.nodeType(nodeId) === Node.ELEMENT_NODE) {
        if (this._ctx._docHandle.getAttribute(nodeId, 'id') === id) {
          return this._ctx._wrapId(nodeId);
        }
        const found = this._findById(this._ctx._docHandle.firstChild(nodeId), id);
        if (found) return found;
      }
      nodeId = this._ctx._docHandle.nextSibling(nodeId);
    }
    return null;
  }
}
