// https://dom.spec.whatwg.org/#element

import { Node } from './node';
import type { DocumentContext } from './document-context';
import type { NodeHandle } from './native';
import { registerNodeType } from '@/html/index';

// Minimal classList shim backed by className string
class DOMTokenList {
  private _el: Element;
  constructor(el: Element) { this._el = el; }

  private _tokens(): string[] {
    return this._el.className.split(/\s+/).filter(Boolean);
  }

  get length(): number { return this._tokens().length; }
  item(index: number): string | null { return this._tokens()[index] ?? null; }
  contains(token: string): boolean { return this._tokens().includes(token); }

  add(...tokens: string[]): void {
    const set = new Set(this._tokens());
    for (const t of tokens) set.add(t);
    this._el.className = [...set].join(' ');
  }

  remove(...tokens: string[]): void {
    const set = new Set(this._tokens());
    for (const t of tokens) set.delete(t);
    this._el.className = [...set].join(' ');
  }

  toggle(token: string, force?: boolean): boolean {
    const has = this.contains(token);
    if (force === undefined ? has : force) {
      this.remove(token); return false;
    } else {
      this.add(token); return true;
    }
  }

  replace(oldToken: string, newToken: string): boolean {
    if (!this.contains(oldToken)) return false;
    this.remove(oldToken);
    this.add(newToken);
    return true;
  }

  toString(): string { return this._el.className; }

  [Symbol.iterator](): Iterator<string> { return this._tokens()[Symbol.iterator](); }
}

// ── Element ────────────────────────────────────────────────────────────────

export class Element extends Node {
  private _classList: DOMTokenList;

  constructor(ctx: DocumentContext, handle: NodeHandle) {
    super(ctx, handle);
    this._classList = new DOMTokenList(this);
  }

  // ── Identity ─────────────────────────────────────────────────────────────

  get tagName(): string {
    return this._ctx.tagName(this._handle)?.toUpperCase() ?? '';
  }

  get localName(): string {
    return this._ctx.tagName(this._handle)?.toLowerCase() ?? '';
  }

  // ── Attributes ───────────────────────────────────────────────────────────

  getAttribute(name: string): string | null {
    return this._ctx.getAttribute(this._handle, name);
  }

  setAttribute(name: string, value: string): void {
    this._ctx.setAttribute(this._handle, name, value);
  }

  removeAttribute(name: string): void {
    this._ctx.removeAttribute(this._handle, name);
  }

  hasAttribute(name: string): boolean {
    return this._ctx.hasAttribute(this._handle, name);
  }

  getAttributeNames(): string[] {
    return this._ctx.attributes(this._handle).map(([name]) => name);
  }

  toggleAttribute(name: string, force?: boolean): boolean {
    const has = this.hasAttribute(name);
    if (force === undefined ? has : force) {
      this.removeAttribute(name); return false;
    } else {
      this.setAttribute(name, ''); return true;
    }
  }

  // ── id / className / classList ────────────────────────────────────────────

  get id(): string { return this.getAttribute('id') ?? ''; }
  set id(value: string) { this.setAttribute('id', value); }

  get className(): string { return this.getAttribute('class') ?? ''; }
  set className(value: string) { this.setAttribute('class', value); }

  get classList(): DOMTokenList { return this._classList; }

  // ── Element tree traversal ────────────────────────────────────────────────

  get children(): Element[] {
    const result: Element[] = [];
    let child = this._ctx.firstChild(this._handle);
    while (child) {
      if (child.nodeType === Node.ELEMENT_NODE) result.push(child as Element);
      child = this._ctx.nextSibling(child._handle);
    }
    return result;
  }

  get childElementCount(): number { return this.children.length; }

  get firstElementChild(): Element | null {
    let child = this._ctx.firstChild(this._handle);
    while (child) {
      if (child.nodeType === Node.ELEMENT_NODE) return child as Element;
      child = this._ctx.nextSibling(child._handle);
    }
    return null;
  }

  get lastElementChild(): Element | null {
    let child = this._ctx.lastChild(this._handle);
    while (child) {
      if (child.nodeType === Node.ELEMENT_NODE) return child as Element;
      child = this._ctx.previousSibling(child._handle);
    }
    return null;
  }

  get nextElementSibling(): Element | null {
    let sib = this._ctx.nextSibling(this._handle);
    while (sib) {
      if (sib.nodeType === Node.ELEMENT_NODE) return sib as Element;
      sib = this._ctx.nextSibling(sib._handle);
    }
    return null;
  }

  get previousElementSibling(): Element | null {
    let sib = this._ctx.previousSibling(this._handle);
    while (sib) {
      if (sib.nodeType === Node.ELEMENT_NODE) return sib as Element;
      sib = this._ctx.previousSibling(sib._handle);
    }
    return null;
  }

  // ── Mutation ─────────────────────────────────────────────────────────────

  append(...nodes: (Node | string)[]): void {
    for (const n of nodes) {
      if (typeof n === 'string') {
        const handle = this._ctx.createTextNode(n);
        this._ctx.appendChild(this._handle, handle);
        this._ctx._nodes.add(this._ctx.wrap(handle)!);
      } else {
        this.appendChild(n);
      }
    }
  }

  prepend(...nodes: (Node | string)[]): void {
    const ref = this._ctx.firstChild(this._handle);
    for (const n of nodes) {
      if (typeof n === 'string') {
        const handle = this._ctx.createTextNode(n);
        if (ref) {
          this._ctx.insertBefore(this._handle, handle, ref._handle);
        } else {
          this._ctx.appendChild(this._handle, handle);
        }
        this._ctx._nodes.add(this._ctx.wrap(handle)!);
      } else {
        if (ref) {
          this.insertBefore(n, ref);
        } else {
          this.appendChild(n);
        }
      }
    }
  }

  remove(): void {
    const parent = this._ctx.parentNode(this._handle);
    if (parent) parent.removeChild(this);
  }

  replaceWith(...nodes: (Node | string)[]): void {
    const parent = this._ctx.parentNode(this._handle);
    if (!parent) return;
    for (const n of nodes) {
      if (typeof n === 'string') {
        const handle = this._ctx.createTextNode(n);
        this._ctx.insertBefore(parent._handle, handle, this._handle);
        this._ctx._nodes.add(this._ctx.wrap(handle)!);
      } else {
        this._ctx.insertBefore(parent._handle, n._handle, this._handle);
        this._ctx._nodes.add(n);
      }
    }
    parent.removeChild(this);
  }

  // ── Style (stub) ──────────────────────────────────────────────────────────

  get style(): CSSStyleDeclaration {
    return { getPropertyValue: () => '' } as unknown as CSSStyleDeclaration;
  }
}

// Register factory so DocumentContext.wrap() returns Element for element nodes
registerNodeType(Node.ELEMENT_NODE, (ctx, handle) => new Element(ctx, handle));
