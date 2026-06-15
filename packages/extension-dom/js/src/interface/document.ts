// https://dom.spec.whatwg.org/#document

import {
  Node,
  wrapHandleWith,
  registerNodeType,
} from './node.js';

import { DocumentHandle, NodeHandle } from './native';

import {DocumentContext} from './document-context.js'


export class Document extends Node {
  
    _ctx: DocumentContext

    constructor() {
        super()
        this._ctx = new DocumentContext();
    }
    get _docCtx(): DocumentContext {
        return this._ctx
    }

  override get ownerDocument(): null {
    return null;
  }

  // ── Document tree ────────────────────────────────────────────────────────

  get documentElement(): Node | null {
    return this._wrap(this._docCtx.documentElement());
  }

  // ── Node creation ────────────────────────────────────────────────────────

  createElement(tagName: string): Node {
    return wrapHandleWith(this._docCtx, this._docCtx.createElement(tagName))!;
  }

  createTextNode(data: string): Node {
    return wrapHandleWith(this._docCtx, this._docCtx.createTextNode(data))!;
  }

  createComment(data: string): Node {
    return wrapHandleWith(this._docCtx, this._docCtx.createComment(data))!;
  }

  // ── Query ────────────────────────────────────────────────────────────────

//   getElementById(id: string): Node | null {
//     return this._findById(this._docCtx.firstChild(this._handle), id);
//   }

//   private _findById(h: NodeHandle | null, id: string): Node | null {
//     while (h) {
//       if (this._docCtx.nodeType(h) === Node.ELEMENT_NODE) {
//         if (this._docCtx.getAttribute(h, 'id') === id) {
//           return this._wrap(h);
//         }
//         const found = this._findById(this._docCtx.firstChild(h), id);
//         if (found) return found;
//       }
//       h = this._docCtx.nextSibling(h);
//     }
//     return null;
//   }
}

registerNodeType(Node.DOCUMENT_NODE, () => {
  throw new DOMException('Document nodes must be constructed directly', 'NotSupportedError');
});
