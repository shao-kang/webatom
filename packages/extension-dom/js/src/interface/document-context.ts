import { DocumentHandle, NodeHandle } from './native';
import { Node } from './node.js';

export class DocumentContext {
  readonly _docHandle: DocumentHandle;
  readonly _nodes: Set<Node> = new Set();
  readonly _handleNodeMap: WeakMap<NodeHandle, Node> = new WeakMap();

  constructor() {
    this._docHandle = new DocumentHandle();
  }
  getNode(node: NodeHandle): Node {
    return this._handleNodeMap.get(node)!;
  }


  // ── Node properties ──────────────────────────────────────────────────────

  nodeType(node: NodeHandle): number {
    return this._docHandle.nodeType(node);
  }

  tagName(node: NodeHandle): string | null {
    return this._docHandle.tagName(node);
  }

  nodeValue(node: NodeHandle): string | null {
    return this._docHandle.nodeValue(node);
  }

  setNodeValue(node: NodeHandle, value: string | null): void {
    this._docHandle.setNodeValue(node, value);
  }

  // ── Tree traversal ───────────────────────────────────────────────────────

  parentNode(node: NodeHandle): Node | null {
    const handle = this._docHandle.parentNode(node);
    return handle ? this.getNode(handle) : null;
  }

  firstChild(node: NodeHandle): Node | null {
    console.log('contextstart')
    const handle = this._docHandle.firstChild(node);
    console.log('context', handle)
    return handle ? this.getNode(handle) : null;
  }

  lastChild(node: NodeHandle): Node | null {

    const handle = this._docHandle.lastChild(node);
    return handle ? this.getNode(handle) : null;
  }

  nextSibling(node: NodeHandle): Node | null {
    const handle = this._docHandle.nextSibling(node);
    return handle ? this.getNode(handle) : null;
  }

  previousSibling(node: NodeHandle): Node | null {
    const handle = this._docHandle.previousSibling(node);
    return handle ? this.getNode(handle) : null;
  }

  // ── Tree mutation ────────────────────────────────────────────────────────

  appendChild(parent: NodeHandle, child: NodeHandle): void {
    this._docHandle.appendChild(parent, child);
    
  }

  removeChild(parent: NodeHandle, child: NodeHandle): void {
    this._docHandle.removeChild(parent, child);
  }

  insertBefore(parent: NodeHandle, newNode: NodeHandle, ref: NodeHandle): void {
    this._docHandle.insertBefore(parent, newNode, ref);
  }

  replaceChild(parent: NodeHandle, newNode: NodeHandle, old: NodeHandle): void {
    this._docHandle.replaceChild(parent, newNode, old);
  }

  // ── Node creation ────────────────────────────────────────────────────────

  createElement(tagName: string): NodeHandle {
    return this._docHandle.createElement(tagName);
  }

  createTextNode(data: string): NodeHandle {
    return this._docHandle.createTextNode(data);
  }

  createComment(data: string): NodeHandle {
    return this._docHandle.createComment(data);
  }

  // ── Document structure ───────────────────────────────────────────────────

  documentNode(): NodeHandle {
    return this._docHandle.documentNode();
  }

  documentElement(): NodeHandle | null {
    return this._docHandle.documentElement();
  }

  body(): NodeHandle | null {
    return this._docHandle.body();
  }

  head(): NodeHandle | null {
    return this._docHandle.head();
  }

  // ── Attributes ───────────────────────────────────────────────────────────

  getAttribute(node: NodeHandle, name: string): string | null {
    return this._docHandle.getAttribute(node, name);
  }

  setAttribute(node: NodeHandle, name: string, value: string): void {
    this._docHandle.setAttribute(node, name, value);
  }

  removeAttribute(node: NodeHandle, name: string): void {
    this._docHandle.removeAttribute(node, name);
  }

  hasAttribute(node: NodeHandle, name: string): boolean {
    return this._docHandle.hasAttribute(node, name);
  }

  attributes(node: NodeHandle): [string, string][] {
    return this._docHandle.attributes(node);
  }
}
