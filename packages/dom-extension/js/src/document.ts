import { Element } from './element.js';
import { Event, EventTarget } from './event_target.js';
import { Text, Comment } from './node.js';

// ── Initialize HTML document structure ────────────────────────────────────
// Rust creates a bare Document root (nodeType=9).
// We build html > head + body in JS so browser APIs work.

const _docRootId = __documentElement();
const _htmlId    = __createElement("html");
const _headId    = __createElement("head");
const _bodyId    = __createElement("body");
__appendChild(_docRootId, _htmlId);
__appendChild(_htmlId, _headId);
__appendChild(_htmlId, _bodyId);

const _listeners = new EventTarget();

// ── Document object ───────────────────────────────────────────────────────

export const document = {
  nodeType: 9,
  nodeName: '#document',

  // documentElement is the <html> element, not the Document root node
  get documentElement(): Element { return new Element(_htmlId); },
  get head(): Element             { return new Element(_headId); },
  get body(): Element             { return new Element(_bodyId); },
  get doctype(): null             { return null; },

  get title(): string {
    const t = this.head.querySelector('title');
    return t ? t.textContent : '';
  },
  set title(v: string) {
    let t = this.head.querySelector('title');
    if (!t) { t = this.createElement('title'); this.head.appendChild(t); }
    t.textContent = v;
  },

  createElement(tag: string): Element {
    return new Element(__createElement(tag));
  },
  createElementNS(_ns: string | null, tag: string): Element {
    return this.createElement(tag);
  },
  createTextNode(data: string): Text {
    return new Text(__createTextNode(String(data)));
  },
  createComment(data: string): Comment {
    return new Comment(__createComment(String(data)));
  },
  createDocumentFragment(): Element {
    return this.createElement('template');
  },
  createEvent(_type: string): Event {
    return new Event('');
  },

  getElementById(id: string): Element | null {
    return this.documentElement.querySelector('#' + id);
  },
  querySelector(selector: string): Element | null {
    return this.documentElement.querySelector(selector);
  },
  querySelectorAll(selector: string): Element[] {
    return this.documentElement.querySelectorAll(selector);
  },

  get readyState() { return 'complete'; },
  get URL()        { return ''; },
  get documentURI(){ return ''; },
  get baseURI()    { return ''; },
  get charset()    { return 'UTF-8'; },
  get compatMode() { return 'CSS1Compat'; },
  get cookie()     { return ''; },
  set cookie(_v: string) {},

  hasFocus()  { return true; },

  addEventListener(type: string, handler: (e: Event) => void) {
    _listeners.addEventListener(type, handler);
  },
  removeEventListener(type: string, handler: (e: Event) => void) {
    _listeners.removeEventListener(type, handler);
  },
  dispatchEvent(event: Event): boolean {
    return _listeners.dispatchEvent(event);
  },
};
