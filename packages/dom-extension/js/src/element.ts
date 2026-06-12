import { Node, Text, _registerElement, wrapNode, childIds, ELEMENT_NODE } from './node.js';

// ── classList ──────────────────────────────────────────────────────────────

class DOMTokenList {
  private _el: Element;
  constructor(el: Element) { this._el = el; }

  private _classes(): string[] {
    return (this._el.getAttribute('class') ?? '').split(/\s+/).filter(Boolean);
  }
  private _set(arr: string[]) { this._el.setAttribute('class', arr.join(' ')); }

  get length() { return this._classes().length; }
  item(i: number) { return this._classes()[i] ?? null; }
  contains(c: string) { return this._classes().includes(c); }
  add(...tokens: string[]) {
    const arr = this._classes();
    for (const t of tokens) if (!arr.includes(t)) arr.push(t);
    this._set(arr);
  }
  remove(...tokens: string[]) {
    this._set(this._classes().filter(c => !tokens.includes(c)));
  }
  toggle(c: string, force?: boolean): boolean {
    const has = this.contains(c);
    if (force === true || (!has && force === undefined)) { this.add(c); return true; }
    this.remove(c); return false;
  }
  replace(old: string, next: string) { this.remove(old); this.add(next); }
  toString() { return this._el.getAttribute('class') ?? ''; }
}

// ── style bag ─────────────────────────────────────────────────────────────

class CSSStyleDeclaration {
  private _map = new Map<string, string>();

  setProperty(prop: string, value: string) { this._map.set(prop, value); }
  getPropertyValue(prop: string) { return this._map.get(prop) ?? ''; }
  removeProperty(prop: string) { this._map.delete(prop); }

  // Convenience setters for common properties
  set color(v: string)            { this.setProperty('color', v); }
  get color()                     { return this.getPropertyValue('color'); }
  set display(v: string)          { this.setProperty('display', v); }
  get display()                   { return this.getPropertyValue('display'); }
  set visibility(v: string)       { this.setProperty('visibility', v); }
  set opacity(v: string)          { this.setProperty('opacity', v); }
  set background(v: string)       { this.setProperty('background', v); }
  set backgroundColor(v: string)  { this.setProperty('background-color', v); }
  set fontSize(v: string)         { this.setProperty('font-size', v); }
  set fontWeight(v: string)       { this.setProperty('font-weight', v); }
  set fontFamily(v: string)       { this.setProperty('font-family', v); }
  set margin(v: string)           { this.setProperty('margin', v); }
  set padding(v: string)          { this.setProperty('padding', v); }
  set width(v: string)            { this.setProperty('width', v); }
  get width()                     { return this.getPropertyValue('width'); }
  set height(v: string)           { this.setProperty('height', v); }
  get height()                    { return this.getPropertyValue('height'); }
  set position(v: string)         { this.setProperty('position', v); }
  set top(v: string)              { this.setProperty('top', v); }
  set left(v: string)             { this.setProperty('left', v); }
  set right(v: string)            { this.setProperty('right', v); }
  set bottom(v: string)           { this.setProperty('bottom', v); }
  set zIndex(v: string)           { this.setProperty('z-index', v); }
  set transform(v: string)        { this.setProperty('transform', v); }
  set transition(v: string)       { this.setProperty('transition', v); }
  set overflow(v: string)         { this.setProperty('overflow', v); }
  set cursor(v: string)           { this.setProperty('cursor', v); }
  set border(v: string)           { this.setProperty('border', v); }
  set borderRadius(v: string)     { this.setProperty('border-radius', v); }
  set boxShadow(v: string)        { this.setProperty('box-shadow', v); }
  set textAlign(v: string)        { this.setProperty('text-align', v); }
  set flex(v: string)             { this.setProperty('flex', v); }
  set flexDirection(v: string)    { this.setProperty('flex-direction', v); }
  set alignItems(v: string)       { this.setProperty('align-items', v); }
  set justifyContent(v: string)   { this.setProperty('justify-content', v); }
  set gap(v: string)              { this.setProperty('gap', v); }
}

// ── selector helpers ───────────────────────────────────────────────────────

function matchesSelector(id: number, selector: string): boolean {
  selector = selector.trim();
  // multi-selector: "a, b"
  if (selector.includes(',')) {
    return selector.split(',').some(s => matchesSelector(id, s.trim()));
  }
  const tag = selector.match(/^[a-zA-Z][a-zA-Z0-9-]*/)?.[0];
  if (tag && (__tagName(id) ?? '').toLowerCase() !== tag.toLowerCase()) return false;
  for (const m of selector.matchAll(/#([\w-]+)/g)) {
    if (__getAttribute(id, 'id') !== m[1]) return false;
  }
  const cls = ' ' + (__getAttribute(id, 'class') ?? '') + ' ';
  for (const m of selector.matchAll(/\.([\w-]+)/g)) {
    if (!cls.includes(' ' + m[1] + ' ')) return false;
  }
  return true;
}

function bfsSelect(rootId: number, selector: string, first: true): Element | null;
function bfsSelect(rootId: number, selector: string, first: false): Element[];
function bfsSelect(rootId: number, selector: string, first: boolean): Element | null | Element[] {
  const results: Element[] = [];
  const queue = childIds(rootId);
  while (queue.length) {
    const cur = queue.shift()!;
    if (matchesSelector(cur, selector)) {
      const el = new Element(cur);
      if (first) return el;
      results.push(el);
    }
    queue.push(...childIds(cur));
  }
  return first ? null : results;
}

// ── Element ────────────────────────────────────────────────────────────────

export class Element extends Node {
  private _style = new CSSStyleDeclaration();

  get nodeName() { return this.tagName; }
  get tagName()  { return __tagName(this._id) ?? ''; }

  get id()          { return this.getAttribute('id') ?? ''; }
  set id(v: string) { this.setAttribute('id', v); }

  get className()          { return this.getAttribute('class') ?? ''; }
  set className(v: string) { this.setAttribute('class', v); }

  get classList() { return new DOMTokenList(this); }

  getAttribute(name: string): string | null {
    return __getAttribute(this._id, name) ?? null;
  }
  setAttribute(name: string, value: string): void {
    __setAttribute(this._id, name, String(value));
  }
  removeAttribute(name: string): void {
    __removeAttribute(this._id, name);
  }
  hasAttribute(name: string): boolean {
    return this.getAttribute(name) !== null;
  }
  toggleAttribute(name: string, force?: boolean): boolean {
    const has = this.hasAttribute(name);
    if (force === true || (!has && force === undefined)) {
      this.setAttribute(name, ''); return true;
    }
    this.removeAttribute(name); return false;
  }

  get value()         { return this.getAttribute('value') ?? ''; }
  set value(v: string){ this.setAttribute('value', v); }
  get checked()       { return this.getAttribute('checked') === 'true'; }
  set checked(v: boolean) { this.setAttribute('checked', v ? 'true' : 'false'); }

  override get textContent(): string {
    return this.childNodes.map(n => n.textContent).join('');
  }
  override set textContent(v: string) {
    while (this.firstChild) this.removeChild(this.firstChild);
    if (v) this.appendChild(new Text(__createTextNode(v)));
  }

  get innerHTML(): string { return ''; }   // TODO: serializer
  set innerHTML(_v: string) {}             // TODO: HTML parser

  get outerHTML(): string { return ''; }  // TODO: serializer

  get style() { return this._style; }

  // ── child element helpers ────────────────────────────────────────────────

  get children(): Element[] {
    return this.childNodes.filter(n => n.nodeType === ELEMENT_NODE) as Element[];
  }
  get childElementCount() { return this.children.length; }
  get firstElementChild(): Element | null  { return this.children[0] ?? null; }
  get lastElementChild(): Element | null   { const c = this.children; return c[c.length - 1] ?? null; }

  get nextElementSibling(): Element | null {
    let s = this.nextSibling;
    while (s && s.nodeType !== ELEMENT_NODE) s = s.nextSibling;
    return s as Element | null;
  }
  get previousElementSibling(): Element | null {
    let s = this.previousSibling;
    while (s && s.nodeType !== ELEMENT_NODE) s = s.previousSibling;
    return s as Element | null;
  }

  // ── selectors ────────────────────────────────────────────────────────────

  matches(selector: string): boolean { return matchesSelector(this._id, selector); }
  querySelector(selector: string): Element | null {
    return bfsSelect(this._id, selector, true);
  }
  querySelectorAll(selector: string): Element[] {
    return bfsSelect(this._id, selector, false);
  }
  closest(selector: string): Element | null {
    let cur: Element | null = this;
    while (cur) { if (cur.matches(selector)) return cur; cur = cur.parentElement; }
    return null;
  }

  // ── mutation helpers ─────────────────────────────────────────────────────

  remove() { this.parentNode?.removeChild(this); }

  before(...nodes: (Node | string)[]): void {
    const p = this.parentNode; if (!p) return;
    for (const n of nodes)
      p.insertBefore(typeof n === 'string' ? new Text(__createTextNode(n)) : n, this);
  }
  after(...nodes: (Node | string)[]): void {
    const ref = this.nextSibling;
    const p = this.parentNode; if (!p) return;
    for (const n of nodes)
      p.insertBefore(typeof n === 'string' ? new Text(__createTextNode(n)) : n, ref);
  }
  replaceWith(node: Node | string): void {
    const p = this.parentNode; if (!p) return;
    p.replaceChild(typeof node === 'string' ? new Text(__createTextNode(node)) : node, this);
  }
  append(...nodes: (Node | string)[]): void {
    for (const n of nodes)
      this.appendChild(typeof n === 'string' ? new Text(__createTextNode(n)) : n);
  }
  prepend(...nodes: (Node | string)[]): void {
    const ref = this.firstChild;
    for (const n of nodes)
      this.insertBefore(typeof n === 'string' ? new Text(__createTextNode(n)) : n, ref);
  }

  // ── layout stubs ─────────────────────────────────────────────────────────

  getBoundingClientRect() {
    return { x: 0, y: 0, width: 0, height: 0, top: 0, left: 0, right: 0, bottom: 0 };
  }
  get offsetWidth()  { return 0; }
  get offsetHeight() { return 0; }
  get clientWidth()  { return 0; }
  get clientHeight() { return 0; }
  get offsetTop()    { return 0; }
  get offsetLeft()   { return 0; }
  scrollIntoView()   {}

  // ── interaction stubs ─────────────────────────────────────────────────────

  focus() {}
  blur()  {}
  click() { this.dispatchEvent(new (globalThis as any).Event('click')); }
}

// Register Element so wrapNode can produce Element instances
_registerElement(Element);
