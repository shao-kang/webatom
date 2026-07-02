// Bridge between DocumentHandle (Rust primitives) and the JS Node layer.
// Creates and owns the single DocumentHandle for its document.

import { DocumentHandle, NodeHandle } from './native';
import { Node } from './node';
import { getNodeFactory } from '@/html/index';
import { Event as DomEvent, MouseEvent, KeyboardEvent, FocusEvent } from './event';

export class DocumentContext {
  readonly _docHandle: DocumentHandle;
  readonly _nodes: Set<Node> = new Set();
  /** node id → WeakRef<NodeHandle>: ensures at most one live NodeHandle per node */
  readonly _nodeHandles: Map<number, WeakRef<NodeHandle>> = new Map();
  readonly _handleNodeMap: WeakMap<NodeHandle, Node> = new WeakMap();

  constructor() {
    this._docHandle = new DocumentHandle();
    this._docHandle.onEvent((event) => {
      this.onEvent(event);
    });
  }

  onEvent(raw: {
    type: string;
    nodeId?: number;
    x?: number; y?: number;
    button?: number;
    key?: string; modifiers?: number;
    width?: number; height?: number;
    deltaX?: number; deltaY?: number;
  }) {
    const { type } = raw;

    // Resolve target node (null for document-level events like resize/scroll)
    const targetNode = raw.nodeId != null ? this._wrapId(raw.nodeId) : null;

    let domEvent: DomEvent;
    switch (type) {
      case 'click':
      case 'dblclick':
      case 'mousedown':
      case 'mouseup':
      case 'mousemove':
        domEvent = new MouseEvent(type, {
          bubbles: true, cancelable: true,
          clientX: raw.x ?? 0, clientY: raw.y ?? 0,
          button: raw.button ?? 0,
        });
        break;
      case 'keydown':
      case 'keyup':
        domEvent = new KeyboardEvent(type, {
          bubbles: true, cancelable: true,
          key: raw.key ?? '',
        });
        break;
      case 'focus':
        domEvent = new FocusEvent('focus', { bubbles: false, cancelable: false });
        break;
      case 'blur':
        domEvent = new FocusEvent('blur', { bubbles: false, cancelable: false });
        break;
      default:
        domEvent = new DomEvent(type, { bubbles: false, cancelable: false });
    }

    // Dispatch: capture → target → bubble (handled by EventTarget.dispatchEvent)
    const dispatchTarget = targetNode ?? this._wrapId(this._docHandle.documentNode());
    dispatchTarget?.dispatchEvent(domEvent);
  }

  // ── Internal: id ↔ NodeHandle ─────────────────────────────────────────────

  private _idToHandle(id: number | null | undefined): NodeHandle | null {
    if (id === null || id === undefined) return null;
    const ref = this._nodeHandles.get(id);
    if (ref) {
      const handle = ref.deref();
      if (handle) return handle;
      this._nodeHandles.delete(id);
    }
    const handle = this._docHandle.acquireHandle(id);
    if (handle) this._nodeHandles.set(id, new WeakRef(handle));
    return handle;
  }

  // ── Handle → Node wrapping ────────────────────────────────────────────────

  wrap(handle: NodeHandle | null): Node | null {
    if (!handle) return null;
    const existing = this._handleNodeMap.get(handle);
    if (existing) return existing;
    const type = this._docHandle.nodeType(handle.nodeId);
    const factory = getNodeFactory(type);
    const node = factory ? factory(this, handle) : new Node(this, handle);
    this._handleNodeMap.set(handle, node);
    if (this._docHandle.parentNode(handle.nodeId) !== null) {
      this._nodes.add(node);
    }
    return node;
  }

  _wrapId(id: number | null): Node | null {
    return this.wrap(this._idToHandle(id));
  }

  // ── Bootstrap ─────────────────────────────────────────────────────────────

  _attachRoot(doc: Node): void {
    this._handleNodeMap.set(doc._handle, doc);
    this._nodes.add(doc);
    this._initNodes();
  }

  _initNodes(): void {
    const rootHandle = this._idToHandle(this._docHandle.documentNode());
    if (rootHandle) this._traverseSubtree(rootHandle);
  }

  _traverseSubtree(h: NodeHandle): void {
    const node = this.wrap(h);
    if (node) this._nodes.add(node);
    let children = this._docHandle.childNodes(h.nodeId);
    children.forEach((child) => {
      const childHandle = this._idToHandle(child);
      if (childHandle) this._traverseSubtree(childHandle);
    })
  }

  _releaseSubtree(h: NodeHandle): void {
    const node = this._handleNodeMap.get(h);
    if (node) this._nodes.delete(node);
    let childId = this._docHandle.firstChild(h.nodeId);
    while (childId !== null) {
      const childHandle = this._idToHandle(childId);
      if (childHandle) this._releaseSubtree(childHandle);
      childId = this._docHandle.nextSibling(childId);
    }
  }

  // ── Scalar queries (no wrapping) ──────────────────────────────────────────

  nodeType(node: NodeHandle): number {
    return this._docHandle.nodeType(node.nodeId);
  }

  tagName(node: NodeHandle): string | null {
    return this._docHandle.tagName(node.nodeId);
  }

  nodeValue(node: NodeHandle): string | null {
    return this._docHandle.nodeValue(node.nodeId);
  }

  setNodeValue(node: NodeHandle, value: string | null): void {
    this._docHandle.setNodeValue(node.nodeId, value);
  }

  getAttribute(node: NodeHandle, name: string): string | null {
    return this._docHandle.getAttribute(node.nodeId, name);
  }

  setAttribute(node: NodeHandle, name: string, value: string): void {
    this._docHandle.setAttribute(node.nodeId, name, value);
  }

  removeAttribute(node: NodeHandle, name: string): void {
    this._docHandle.removeAttribute(node.nodeId, name);
  }

  hasAttribute(node: NodeHandle, name: string): boolean {
    return this._docHandle.hasAttribute(node.nodeId, name);
  }

  attributes(node: NodeHandle): [string, string][] {
    return this._docHandle.attributes(node.nodeId);
  }

  // ── Tree traversal (wraps result to Node | null) ──────────────────────────

  parentNode(node: NodeHandle): Node | null {
    return this._wrapId(this._docHandle.parentNode(node.nodeId));
  }

  firstChild(node: NodeHandle): Node | null {
    return this._wrapId(this._docHandle.firstChild(node.nodeId));
  }

  lastChild(node: NodeHandle): Node | null {
    return this._wrapId(this._docHandle.lastChild(node.nodeId));
  }

  nextSibling(node: NodeHandle): Node | null {
    return this._wrapId(this._docHandle.nextSibling(node.nodeId));
  }

  previousSibling(node: NodeHandle): Node | null {
    return this._wrapId(this._docHandle.previousSibling(node.nodeId));
  }

  // ── Document structure (wraps result to Node | null) ──────────────────────

  documentElement(): Node | null {
    return this._wrapId(this._docHandle.documentElement());
  }

  body(): Node | null {
    return this._wrapId(this._docHandle.body());
  }

  head(): Node | null {
    return this._wrapId(this._docHandle.head());
  }

  querySelector(scope: NodeHandle, selector: string): Node | null {
    const id = this._docHandle.querySelector(scope.nodeId, selector);
    return this._wrapId(id);
  }

  querySelectorAll(scope: NodeHandle, selector: string): Node[] {
    return (this._docHandle.querySelectorAll(scope.nodeId, selector) as number[])
      .flatMap((id) => { const n = this._wrapId(id); return n ? [n] : []; });
  }

  // ── Tree mutation ─────────────────────────────────────────────────────────

  appendChild(parent: NodeHandle, child: NodeHandle): void {
    this._docHandle.appendChild(parent.nodeId, child.nodeId);
    const parentNode = this._handleNodeMap.get(parent);
    if (parentNode && this._nodes.has(parentNode)) {
      this._traverseSubtree(child);
    }
  }

  removeChild(parent: NodeHandle, child: NodeHandle): void {
    this._docHandle.removeChild(parent.nodeId, child.nodeId);
    const childNode = this._handleNodeMap.get(child);
    if (childNode && this._nodes.has(childNode)) {
      this._releaseSubtree(child);
    }
  }

  insertBefore(parent: NodeHandle, newNode: NodeHandle, ref: NodeHandle): void {
    this._docHandle.insertBefore(parent.nodeId, newNode.nodeId, ref.nodeId);
    const parentNode = this._handleNodeMap.get(parent);
    if (parentNode && this._nodes.has(parentNode)) {
      this._traverseSubtree(newNode);
    }
  }

  replaceChild(parent: NodeHandle, newNode: NodeHandle, old: NodeHandle): void {
    this._docHandle.replaceChild(parent.nodeId, newNode.nodeId, old.nodeId);
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
    return this._idToHandle(this._docHandle.createElement(tagName))!;
  }

  createTextNode(data: string): NodeHandle {
    return this._idToHandle(this._docHandle.createTextNode(data))!;
  }

  createComment(data: string): NodeHandle {
    return this._idToHandle(this._docHandle.createComment(data))!;
  }

  // ── Bootstrap (Document constructor only) ─────────────────────────────────

  documentNode(): NodeHandle {
    return this._idToHandle(this._docHandle.documentNode())!;
  }
}
