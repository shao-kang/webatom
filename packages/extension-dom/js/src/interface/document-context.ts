// Bridge between DocumentHandle (Rust primitives) and the JS Node layer.
// Creates and owns the single DocumentHandle for its document.

import { DocumentHandle, NodeHandle } from './native';
import { Node } from './node';
import { getNodeFactory } from '@/html/index';

export class DocumentContext {
  // ── Handle → Node wrapping ───────────────────────────────────────────────

  wrap(handle: NodeHandle | null): Node | null {
    if (!handle) return null;
    const existing = this._handleNodeMap.get(handle);
    if (existing) return existing;
    const type = this._docHandle.nodeType(handle);
    const factory = getNodeFactory(type);
    const node = factory ? factory(this, handle) : new Node(this, handle);
    this._handleNodeMap.set(handle, node);
    if (this._docHandle.parentNode(handle) !== null) {
      this._nodes.add(node);
    }
    return node;
  }


  readonly _docHandle: DocumentHandle;
  readonly _nodes: Set<Node> = new Set();
  readonly _handleNodeMap: WeakMap<NodeHandle, Node> = new WeakMap();

  constructor() {
    this._docHandle = new DocumentHandle();
    this._docHandle.onEvent((event) => {
      console.log(JSON.stringify(event));
    })
  }

  _attachRoot(doc: Node): void {
    this._handleNodeMap.set(this._docHandle.documentNode(), doc);
    this._nodes.add(doc);
    this._initNodes();
  }

  _initNodes(): void {
    this._traverseSubtree(this._docHandle.documentNode());
  }

  _traverseSubtree(h: NodeHandle): void {
    const node = this.wrap(h);
    if (node) this._nodes.add(node);
    let child = this._docHandle.firstChild(h);
    while (child) {
      this._traverseSubtree(child);
      child = this._docHandle.nextSibling(child);
    }
  }

  _releaseSubtree(h: NodeHandle): void {
    const node = this._handleNodeMap.get(h);
    if (node) this._nodes.delete(node);
    let child = this._docHandle.firstChild(h);
    while (child) {
      this._releaseSubtree(child);
      child = this._docHandle.nextSibling(child);
    }
  }

  // ── Scalar queries (no wrapping) ─────────────────────────────────────────

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

  // ── Tree traversal (wraps result to Node | null) ─────────────────────────

  parentNode(node: NodeHandle): Node | null {
    return this.wrap(this._docHandle.parentNode(node));
  }

  firstChild(node: NodeHandle): Node | null {
    return this.wrap(this._docHandle.firstChild(node));
  }

  lastChild(node: NodeHandle): Node | null {
    return this.wrap(this._docHandle.lastChild(node));
  }

  nextSibling(node: NodeHandle): Node | null {
    return this.wrap(this._docHandle.nextSibling(node));
  }

  previousSibling(node: NodeHandle): Node | null {
    return this.wrap(this._docHandle.previousSibling(node));
  }

  // ── Document structure (wraps result to Node | null) ─────────────────────

  documentElement(): Node | null {
    return this.wrap(this._docHandle.documentElement());
  }

  body(): Node | null {
    return this.wrap(this._docHandle.body());
  }

  head(): Node | null {
    return this.wrap(this._docHandle.head());
  }

  // ── Tree mutation ────────────────────────────────────────────────────────

  appendChild(parent: NodeHandle, child: NodeHandle): void {
    this._docHandle.appendChild(parent, child);
    const parentNode = this._handleNodeMap.get(parent);
    if (parentNode && this._nodes.has(parentNode)) {
      this._traverseSubtree(child);
    }
  }

  removeChild(parent: NodeHandle, child: NodeHandle): void {
    this._docHandle.removeChild(parent, child);
    const childNode = this._handleNodeMap.get(child);
    if (childNode && this._nodes.has(childNode)) {
      this._releaseSubtree(child);
    }
  }

  insertBefore(parent: NodeHandle, newNode: NodeHandle, ref: NodeHandle): void {
    this._docHandle.insertBefore(parent, newNode, ref);
    const parentNode = this._handleNodeMap.get(parent);
    if (parentNode && this._nodes.has(parentNode)) {
      this._traverseSubtree(newNode);
    }
  }

  replaceChild(parent: NodeHandle, newNode: NodeHandle, old: NodeHandle): void {
    this._docHandle.replaceChild(parent, newNode, old);
    const parentNode = this._handleNodeMap.get(parent);
    if (parentNode && this._nodes.has(parentNode)) {
      this._traverseSubtree(newNode);
    }
    const oldNode = this._handleNodeMap.get(old);
    if (oldNode && this._nodes.has(oldNode)) {
      this._releaseSubtree(old);
    }
  }

  // ── Node creation (returns NodeHandle — detached, not in _nodes) ──────────

  createElement(tagName: string): NodeHandle {
    return this._docHandle.createElement(tagName);
  }

  createTextNode(data: string): NodeHandle {
    return this._docHandle.createTextNode(data);
  }

  createComment(data: string): NodeHandle {
    return this._docHandle.createComment(data);
  }

  // ── Event bridge ─────────────────────────────────────────────────────────

  /** Find the JS Node for a given webAtom numeric node id. */
  // nodeById(id: number): Node | null {
  //   const handle = this._docHandle.nodeById(id);
  //   if (!handle) return null;
  //   return this.wrap(handle);
  // }

  /**
   * Register a callback that receives resolved Events.
   * `target` is already a wrapped `Node` (resolved from `targetId`), or null for non-node events.
   */
  // onEvent(cb: (evt: { type: string; target: Node | null; key?: string; modifiers?: number; x?: number; y?: number; width?: number; height?: number }) => void): void {
  //   this._docHandle.onEvent((raw) => {
  //     const target = raw.targetId != null ? this.nodeById(raw.targetId) : null;
  //     cb({ type: raw.type, target, key: raw.key, modifiers: raw.modifiers, x: raw.x, y: raw.y, width: raw.width, height: raw.height });
  //   });
  // }

  // ── Bootstrap (Document constructor only) ────────────────────────────────

  documentNode(): NodeHandle {
    return this._docHandle.documentNode();
  }
}
