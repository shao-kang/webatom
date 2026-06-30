// Shared primitive types used by both node.ts and document.ts.
// This file has NO imports from other interface files, breaking any potential
// circular type dependency.

// Native class registered by Rust — opaque token; only created via DocumentHandle.acquireHandle()
declare module "webatom_ext_native:dom" {
  export class NodeHandle {
    readonly nodeId: number;
  }

  // All node operations go through DocumentHandle — the Rust-native facade.
  // Node parameters and return values are plain numeric IDs (usize).
  export class DocumentHandle {
    /** Acquire a NodeHandle for the given id. Returns null if the node already has a live handle. */
    acquireHandle(id: number): NodeHandle | null;

    nodeType(nodeId: number): number;
    tagName(nodeId: number): string | null;
    nodeValue(nodeId: number): string | null;
    setNodeValue(nodeId: number, value: string | null): void;
    parentNode(nodeId: number): number | null;
  childNodes(nodeId: number): number[];
    firstChild(nodeId: number): number | null;
    lastChild(nodeId: number): number | null;
    nextSibling(nodeId: number): number | null;
    previousSibling(nodeId: number): number | null;
    appendChild(parentId: number, childId: number): void;
    removeChild(parentId: number, childId: number): void;
    insertBefore(parentId: number, newId: number, beforeId: number): void;
    replaceChild(parentId: number, newId: number, oldId: number): void;
    documentNode(): number;
    documentElement(): number | null;
    body(): number | null;
    head(): number | null;
    createElement(tagName: string): number;
    createComment(data: string): number;
    createTextNode(data: string): number;
    getAttribute(nodeId: number, name: string): string | null;
    setAttribute(nodeId: number, name: string, value: string): void;
    removeAttribute(nodeId: number, name: string): void;
    hasAttribute(nodeId: number, name: string): boolean;
    attributes(nodeId: number): [string, string][];

    /** Register a callback invoked (via MacroTask) whenever an Event arrives.
     *  Node-targeted events carry `targetNodeId: number` instead of a handle. */
    onEvent(callback: (raw: {
      type: string;
      targetNodeId?: number;
      key?: string;
      modifiers?: number;
      x?: number;
      y?: number;
      button?: number;
      width?: number;
      height?: number;
      deltaX?: number;
      deltaY?: number;
    }) => void): void;
  }
}
