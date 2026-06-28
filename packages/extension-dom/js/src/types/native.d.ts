


// Shared primitive types used by both node.ts and document.ts.
// This file has NO imports from other interface files, breaking any potential
// circular type dependency.

// Native class registered by Rust — opaque, carries no public API
declare module "webatom_ext_native:dom" {
  export class NodeHandle {}

  // All node operations go through DocumentHandle — the Rust-native facade.
  export class DocumentHandle {
    nodeType(node: NodeHandle): number;
    tagName(node: NodeHandle): string | null;
    nodeValue(node: NodeHandle): string | null;
    setNodeValue(node: NodeHandle, value: string | null): void;
    parentNode(node: NodeHandle): NodeHandle | null;
    firstChild(node: NodeHandle): NodeHandle | null;
    lastChild(node: NodeHandle): NodeHandle | null;
    nextSibling(node: NodeHandle): NodeHandle | null;
    previousSibling(node: NodeHandle): NodeHandle | null;
    appendChild(parent: NodeHandle, child: NodeHandle): void;
    removeChild(parent: NodeHandle, child: NodeHandle): void;
    insertBefore(parent: NodeHandle, newNode: NodeHandle, ref: NodeHandle): void;
    replaceChild(parent: NodeHandle, newNode: NodeHandle, old: NodeHandle): void;
    documentNode(): NodeHandle;
    documentElement(): NodeHandle | null;
    body(): NodeHandle | null;
    head(): NodeHandle | null;
    createElement(tagName: string): NodeHandle;
    createComment(data: string): NodeHandle;
    createTextNode(data: string): NodeHandle;
    getAttribute(node: NodeHandle, name: string): string | null;
    setAttribute(node: NodeHandle, name: string, value: string): void;
    removeAttribute(node: NodeHandle, name: string): void;
    hasAttribute(node: NodeHandle, name: string): boolean;
    attributes(node: NodeHandle): [string, string][];
  }
}
