import { DocumentHandle } from "webatom_ext_native:dom";
//#region src/interface/event-target.ts
const wm = /* @__PURE__ */ new WeakMap();
function dispatch(event, listener) {
	if (typeof listener === "function") listener.call(event.target, event);
	else listener.handleEvent(event);
	return event.stopImmediatePropagation;
}
function invokeListeners({ currentTarget, target }) {
	const map = wm.get(currentTarget);
	if (map && map.has(this.type)) {
		const listeners = map.get(this.type);
		const ev = this;
		if (currentTarget === target) ev.eventPhase = ev.AT_TARGET;
		else ev.eventPhase = ev.BUBBLING_PHASE;
		ev.currentTarget = currentTarget;
		ev.target = target;
		for (const [listener, options] of listeners) {
			if (options && typeof options !== "boolean" && options.once) listeners.delete(listener);
			if (dispatch(this, listener)) break;
		}
		delete ev.currentTarget;
		delete ev.target;
		return ev.cancelBubble;
	}
}
/**
* @implements globalThis.EventTarget
*/
var DOMEventTarget = class {
	constructor() {
		wm.set(this, /* @__PURE__ */ new Map());
	}
	/**
	* @protected
	*/
	_getParent() {
		return null;
	}
	addEventListener(type, listener, options) {
		if (!listener) return;
		const map = wm.get(this);
		if (!map.has(type)) map.set(type, /* @__PURE__ */ new Map());
		map.get(type).set(listener, options);
	}
	removeEventListener(type, listener, _options) {
		if (!listener) return;
		const map = wm.get(this);
		if (map.has(type)) {
			const listeners = map.get(type);
			if (listeners.delete(listener) && !listeners.size) map.delete(type);
		}
	}
	dispatchEvent(event) {
		let node = this;
		event.eventPhase = event.CAPTURING_PHASE;
		while (node) {
			if (node.dispatchEvent) event._path.push({
				currentTarget: node,
				target: this
			});
			node = event.bubbles && node._getParent ? node._getParent() : null;
		}
		event._path.some(invokeListeners, event);
		event._path = [];
		event.eventPhase = event.NONE;
		return !event.defaultPrevented;
	}
};
//#endregion
//#region src/interface/node.ts
const NODE_CONSTANTS = {
	ELEMENT_NODE: 1,
	ATTRIBUTE_NODE: 2,
	TEXT_NODE: 3,
	CDATA_SECTION_NODE: 4,
	PROCESSING_INSTRUCTION_NODE: 7,
	COMMENT_NODE: 8,
	DOCUMENT_NODE: 9,
	DOCUMENT_TYPE_NODE: 10,
	DOCUMENT_FRAGMENT_NODE: 11
};
const POSITION_CONSTANTS = {
	DOCUMENT_POSITION_DISCONNECTED: 1,
	DOCUMENT_POSITION_PRECEDING: 2,
	DOCUMENT_POSITION_FOLLOWING: 4,
	DOCUMENT_POSITION_CONTAINS: 8,
	DOCUMENT_POSITION_CONTAINED_BY: 16,
	DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC: 32
};
var Node = class Node extends DOMEventTarget {
	static {
		this.ELEMENT_NODE = NODE_CONSTANTS.ELEMENT_NODE;
	}
	static {
		this.ATTRIBUTE_NODE = NODE_CONSTANTS.ATTRIBUTE_NODE;
	}
	static {
		this.TEXT_NODE = NODE_CONSTANTS.TEXT_NODE;
	}
	static {
		this.CDATA_SECTION_NODE = NODE_CONSTANTS.CDATA_SECTION_NODE;
	}
	static {
		this.PROCESSING_INSTRUCTION_NODE = NODE_CONSTANTS.PROCESSING_INSTRUCTION_NODE;
	}
	static {
		this.COMMENT_NODE = NODE_CONSTANTS.COMMENT_NODE;
	}
	static {
		this.DOCUMENT_NODE = NODE_CONSTANTS.DOCUMENT_NODE;
	}
	static {
		this.DOCUMENT_TYPE_NODE = NODE_CONSTANTS.DOCUMENT_TYPE_NODE;
	}
	static {
		this.DOCUMENT_FRAGMENT_NODE = NODE_CONSTANTS.DOCUMENT_FRAGMENT_NODE;
	}
	static {
		this.DOCUMENT_POSITION_DISCONNECTED = POSITION_CONSTANTS.DOCUMENT_POSITION_DISCONNECTED;
	}
	static {
		this.DOCUMENT_POSITION_PRECEDING = POSITION_CONSTANTS.DOCUMENT_POSITION_PRECEDING;
	}
	static {
		this.DOCUMENT_POSITION_FOLLOWING = POSITION_CONSTANTS.DOCUMENT_POSITION_FOLLOWING;
	}
	static {
		this.DOCUMENT_POSITION_CONTAINS = POSITION_CONSTANTS.DOCUMENT_POSITION_CONTAINS;
	}
	static {
		this.DOCUMENT_POSITION_CONTAINED_BY = POSITION_CONSTANTS.DOCUMENT_POSITION_CONTAINED_BY;
	}
	static {
		this.DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC = POSITION_CONSTANTS.DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC;
	}
	constructor(ctx, handle) {
		super();
		this._ctx = ctx;
		this._handle = handle;
	}
	get nodeType() {
		return this._ctx.nodeType(this._handle);
	}
	get nodeName() {
		switch (this.nodeType) {
			case Node.ELEMENT_NODE: return this._ctx.tagName(this._handle)?.toUpperCase() ?? "";
			case Node.TEXT_NODE: return "#text";
			case Node.CDATA_SECTION_NODE: return "#cdata-section";
			case Node.PROCESSING_INSTRUCTION_NODE: return this._ctx.nodeValue(this._handle) ?? "";
			case Node.COMMENT_NODE: return "#comment";
			case Node.DOCUMENT_NODE: return "#document";
			case Node.DOCUMENT_FRAGMENT_NODE: return "#document-fragment";
			default: return "";
		}
	}
	get parentNode() {
		return this._ctx.parentNode(this._handle);
	}
	get parentElement() {
		const p = this._ctx.parentNode(this._handle);
		return p?.nodeType === Node.ELEMENT_NODE ? p : null;
	}
	get firstChild() {
		return this._ctx.firstChild(this._handle);
	}
	get lastChild() {
		return this._ctx.lastChild(this._handle);
	}
	get nextSibling() {
		return this._ctx.nextSibling(this._handle);
	}
	get previousSibling() {
		return this._ctx.previousSibling(this._handle);
	}
	get childNodes() {
		const result = [];
		let child = this._ctx.firstChild(this._handle);
		while (child) {
			result.push(child);
			child = this._ctx.nextSibling(child._handle);
		}
		return result;
	}
	hasChildNodes() {
		return this._ctx.firstChild(this._handle) !== null;
	}
	get nodeValue() {
		return this._ctx.nodeValue(this._handle);
	}
	set nodeValue(value) {
		this._ctx.setNodeValue(this._handle, value);
	}
	get textContent() {
		const t = this.nodeType;
		if (t === Node.DOCUMENT_NODE || t === Node.DOCUMENT_TYPE_NODE) return null;
		if (t === Node.TEXT_NODE || t === Node.CDATA_SECTION_NODE || t === Node.COMMENT_NODE || t === Node.PROCESSING_INSTRUCTION_NODE) return this._ctx.nodeValue(this._handle);
		return this._collectText(this._handle);
	}
	set textContent(value) {
		let child = this._ctx.firstChild(this._handle);
		while (child) {
			const next = this._ctx.nextSibling(child._handle);
			this._ctx.removeChild(this._handle, child._handle);
			child = next;
		}
		if (value) {
			const textHandle = this._ctx.createTextNode(value);
			this._ctx.appendChild(this._handle, textHandle);
		}
	}
	_collectText(h) {
		let text = "";
		let child = this._ctx.firstChild(h);
		while (child) {
			const t = child.nodeType;
			text += t === Node.TEXT_NODE || t === Node.CDATA_SECTION_NODE ? child.nodeValue ?? "" : this._collectText(child._handle);
			child = this._ctx.nextSibling(child._handle);
		}
		return text;
	}
	appendChild(node) {
		this._ctx.appendChild(this._handle, node._handle);
		return node;
	}
	removeChild(child) {
		this._ctx.removeChild(this._handle, child._handle);
		return child;
	}
	insertBefore(node, refChild) {
		if (refChild === null) this._ctx.appendChild(this._handle, node._handle);
		else this._ctx.insertBefore(this._handle, node._handle, refChild._handle);
		return node;
	}
	replaceChild(node, child) {
		this._ctx.replaceChild(this._handle, node._handle, child._handle);
		return child;
	}
	getRootNode(_options) {
		let root = this;
		let p = this.parentNode;
		while (p) {
			root = p;
			p = p.parentNode;
		}
		return root;
	}
	contains(other) {
		let n = other;
		while (n) {
			if (n._handle === this._handle) return true;
			n = n.parentNode;
		}
		return false;
	}
	isSameNode(other) {
		return other !== null && other._handle === this._handle;
	}
	isEqualNode(other) {
		if (!other) return false;
		if (other._handle === this._handle) return true;
		if (this.nodeType !== other.nodeType || this.nodeName !== other.nodeName || this.nodeValue !== other.nodeValue) return false;
		const a = this.childNodes, b = other.childNodes;
		if (a.length !== b.length) return false;
		return a.every((c, i) => c.isEqualNode(b[i]));
	}
	normalize() {
		let child = this.firstChild;
		while (child) {
			if (child.nodeType === Node.TEXT_NODE) {
				const next = child.nextSibling;
				if (!child.nodeValue) {
					this.removeChild(child);
					child = next;
					continue;
				}
				if (next?.nodeType === Node.TEXT_NODE) {
					child.nodeValue = (child.nodeValue ?? "") + (next.nodeValue ?? "");
					this.removeChild(next);
					continue;
				}
			} else child.normalize();
			child = child.nextSibling;
		}
	}
	compareDocumentPosition(other) {
		if (other._handle === this._handle) return 0;
		if (this.contains(other)) return Node.DOCUMENT_POSITION_CONTAINED_BY | Node.DOCUMENT_POSITION_FOLLOWING;
		if (other.contains(this)) return Node.DOCUMENT_POSITION_CONTAINS | Node.DOCUMENT_POSITION_PRECEDING;
		return Node.DOCUMENT_POSITION_DISCONNECTED;
	}
	cloneNode(_deep = false) {
		throw new DOMException("cloneNode not yet implemented", "NotSupportedError");
	}
	lookupPrefix(_namespace) {
		return null;
	}
	lookupNamespaceURI(_prefix) {
		return null;
	}
	isDefaultNamespace(_namespace) {
		return false;
	}
	get ownerDocument() {
		return null;
	}
	get isConnected() {
		return false;
	}
	get baseURI() {
		return "";
	}
};
for (const [key, value] of [...Object.entries(NODE_CONSTANTS), ...Object.entries(POSITION_CONSTANTS)]) Object.defineProperty(Node.prototype, key, {
	value,
	writable: false,
	enumerable: true,
	configurable: false
});
//#endregion
//#region src/html/index.ts
const nodeRegistry = /* @__PURE__ */ new Map();
const tagRegistry = /* @__PURE__ */ new Map();
function registerNodeType(nodeType, factory) {
	nodeRegistry.set(nodeType, factory);
}
function getNodeFactory(nodeType) {
	return nodeRegistry.get(nodeType);
}
function registerTagType(tagName, factory) {
	tagRegistry.set(tagName.toLowerCase(), factory);
}
function getTagFactory(tagName) {
	return tagRegistry.get(tagName.toLowerCase());
}
//#endregion
//#region src/interface/document-context.ts
var DocumentContext = class {
	wrap(handle) {
		if (!handle) return null;
		const existing = this._handleNodeMap.get(handle);
		if (existing) return existing;
		const factory = getNodeFactory(this._docHandle.nodeType(handle));
		const node = factory ? factory(this, handle) : new Node(this, handle);
		this._handleNodeMap.set(handle, node);
		if (this._docHandle.parentNode(handle) !== null) this._nodes.add(node);
		return node;
	}
	constructor() {
		this._nodes = /* @__PURE__ */ new Set();
		this._handleNodeMap = /* @__PURE__ */ new WeakMap();
		this._docHandle = new DocumentHandle();
	}
	_attachRoot(doc) {
		this._handleNodeMap.set(this._docHandle.documentNode(), doc);
		this._nodes.add(doc);
		this._initNodes();
	}
	_initNodes() {
		this._traverseSubtree(this._docHandle.documentNode());
	}
	_traverseSubtree(h) {
		const node = this.wrap(h);
		if (node) this._nodes.add(node);
		let child = this._docHandle.firstChild(h);
		while (child) {
			this._traverseSubtree(child);
			child = this._docHandle.nextSibling(child);
		}
	}
	_releaseSubtree(h) {
		const node = this._handleNodeMap.get(h);
		if (node) this._nodes.delete(node);
		let child = this._docHandle.firstChild(h);
		while (child) {
			this._releaseSubtree(child);
			child = this._docHandle.nextSibling(child);
		}
	}
	nodeType(node) {
		return this._docHandle.nodeType(node);
	}
	tagName(node) {
		return this._docHandle.tagName(node);
	}
	nodeValue(node) {
		return this._docHandle.nodeValue(node);
	}
	setNodeValue(node, value) {
		this._docHandle.setNodeValue(node, value);
	}
	getAttribute(node, name) {
		return this._docHandle.getAttribute(node, name);
	}
	setAttribute(node, name, value) {
		this._docHandle.setAttribute(node, name, value);
	}
	removeAttribute(node, name) {
		this._docHandle.removeAttribute(node, name);
	}
	hasAttribute(node, name) {
		return this._docHandle.hasAttribute(node, name);
	}
	attributes(node) {
		return this._docHandle.attributes(node);
	}
	parentNode(node) {
		return this.wrap(this._docHandle.parentNode(node));
	}
	firstChild(node) {
		return this.wrap(this._docHandle.firstChild(node));
	}
	lastChild(node) {
		return this.wrap(this._docHandle.lastChild(node));
	}
	nextSibling(node) {
		return this.wrap(this._docHandle.nextSibling(node));
	}
	previousSibling(node) {
		return this.wrap(this._docHandle.previousSibling(node));
	}
	documentElement() {
		return this.wrap(this._docHandle.documentElement());
	}
	body() {
		return this.wrap(this._docHandle.body());
	}
	head() {
		return this.wrap(this._docHandle.head());
	}
	appendChild(parent, child) {
		this._docHandle.appendChild(parent, child);
		const parentNode = this._handleNodeMap.get(parent);
		if (parentNode && this._nodes.has(parentNode)) this._traverseSubtree(child);
	}
	removeChild(parent, child) {
		this._docHandle.removeChild(parent, child);
		const childNode = this._handleNodeMap.get(child);
		if (childNode && this._nodes.has(childNode)) this._releaseSubtree(child);
	}
	insertBefore(parent, newNode, ref) {
		this._docHandle.insertBefore(parent, newNode, ref);
		const parentNode = this._handleNodeMap.get(parent);
		if (parentNode && this._nodes.has(parentNode)) this._traverseSubtree(newNode);
	}
	replaceChild(parent, newNode, old) {
		this._docHandle.replaceChild(parent, newNode, old);
		const parentNode = this._handleNodeMap.get(parent);
		if (parentNode && this._nodes.has(parentNode)) this._traverseSubtree(newNode);
		const oldNode = this._handleNodeMap.get(old);
		if (oldNode && this._nodes.has(oldNode)) this._releaseSubtree(old);
	}
	createElement(tagName) {
		return this._docHandle.createElement(tagName);
	}
	createTextNode(data) {
		return this._docHandle.createTextNode(data);
	}
	createComment(data) {
		return this._docHandle.createComment(data);
	}
	documentNode() {
		return this._docHandle.documentNode();
	}
};
//#endregion
//#region src/interface/document.ts
var Document = class extends Node {
	constructor() {
		const ctx = new DocumentContext();
		const docNode = ctx.documentNode();
		super(ctx, docNode);
		ctx._attachRoot(this);
	}
	get ownerDocument() {
		return null;
	}
	get documentElement() {
		return this._ctx.documentElement();
	}
	get body() {
		return this._ctx.body();
	}
	get head() {
		return this._ctx.head();
	}
	createElement(tagName) {
		return this._ctx.wrap(this._ctx.createElement(tagName));
	}
	createTextNode(data) {
		return this._ctx.wrap(this._ctx.createTextNode(data));
	}
	createComment(data) {
		return this._ctx.wrap(this._ctx.createComment(data));
	}
	getElementById(id) {
		return this._findById(this._ctx._docHandle.firstChild(this._handle), id);
	}
	_findById(h, id) {
		while (h) {
			if (this._ctx.nodeType(h) === Node.ELEMENT_NODE) {
				if (this._ctx.getAttribute(h, "id") === id) return this._ctx.wrap(h);
				const found = this._findById(this._ctx._docHandle.firstChild(h), id);
				if (found) return found;
			}
			h = this._ctx._docHandle.nextSibling(h);
		}
		return null;
	}
};
//#endregion
//#region src/interface/element.ts
var DOMTokenList = class {
	constructor(el) {
		this._el = el;
	}
	_tokens() {
		return this._el.className.split(/\s+/).filter(Boolean);
	}
	get length() {
		return this._tokens().length;
	}
	item(index) {
		return this._tokens()[index] ?? null;
	}
	contains(token) {
		return this._tokens().includes(token);
	}
	add(...tokens) {
		const set = new Set(this._tokens());
		for (const t of tokens) set.add(t);
		this._el.className = [...set].join(" ");
	}
	remove(...tokens) {
		const set = new Set(this._tokens());
		for (const t of tokens) set.delete(t);
		this._el.className = [...set].join(" ");
	}
	toggle(token, force) {
		const has = this.contains(token);
		if (force === void 0 ? has : force) {
			this.remove(token);
			return false;
		} else {
			this.add(token);
			return true;
		}
	}
	replace(oldToken, newToken) {
		if (!this.contains(oldToken)) return false;
		this.remove(oldToken);
		this.add(newToken);
		return true;
	}
	toString() {
		return this._el.className;
	}
	[Symbol.iterator]() {
		return this._tokens()[Symbol.iterator]();
	}
};
var Element = class extends Node {
	constructor(ctx, handle) {
		super(ctx, handle);
		this._classList = new DOMTokenList(this);
	}
	get tagName() {
		return this._ctx.tagName(this._handle)?.toUpperCase() ?? "";
	}
	get localName() {
		return this._ctx.tagName(this._handle)?.toLowerCase() ?? "";
	}
	getAttribute(name) {
		return this._ctx.getAttribute(this._handle, name);
	}
	setAttribute(name, value) {
		this._ctx.setAttribute(this._handle, name, value);
	}
	removeAttribute(name) {
		this._ctx.removeAttribute(this._handle, name);
	}
	hasAttribute(name) {
		return this._ctx.hasAttribute(this._handle, name);
	}
	getAttributeNames() {
		return this._ctx.attributes(this._handle).map(([name]) => name);
	}
	toggleAttribute(name, force) {
		const has = this.hasAttribute(name);
		if (force === void 0 ? has : force) {
			this.removeAttribute(name);
			return false;
		} else {
			this.setAttribute(name, "");
			return true;
		}
	}
	get id() {
		return this.getAttribute("id") ?? "";
	}
	set id(value) {
		this.setAttribute("id", value);
	}
	get className() {
		return this.getAttribute("class") ?? "";
	}
	set className(value) {
		this.setAttribute("class", value);
	}
	get classList() {
		return this._classList;
	}
	get children() {
		const result = [];
		let child = this._ctx.firstChild(this._handle);
		while (child) {
			if (child.nodeType === Node.ELEMENT_NODE) result.push(child);
			child = this._ctx.nextSibling(child._handle);
		}
		return result;
	}
	get childElementCount() {
		return this.children.length;
	}
	get firstElementChild() {
		let child = this._ctx.firstChild(this._handle);
		while (child) {
			if (child.nodeType === Node.ELEMENT_NODE) return child;
			child = this._ctx.nextSibling(child._handle);
		}
		return null;
	}
	get lastElementChild() {
		let child = this._ctx.lastChild(this._handle);
		while (child) {
			if (child.nodeType === Node.ELEMENT_NODE) return child;
			child = this._ctx.previousSibling(child._handle);
		}
		return null;
	}
	get nextElementSibling() {
		let sib = this._ctx.nextSibling(this._handle);
		while (sib) {
			if (sib.nodeType === Node.ELEMENT_NODE) return sib;
			sib = this._ctx.nextSibling(sib._handle);
		}
		return null;
	}
	get previousElementSibling() {
		let sib = this._ctx.previousSibling(this._handle);
		while (sib) {
			if (sib.nodeType === Node.ELEMENT_NODE) return sib;
			sib = this._ctx.previousSibling(sib._handle);
		}
		return null;
	}
	append(...nodes) {
		for (const n of nodes) if (typeof n === "string") {
			const handle = this._ctx.createTextNode(n);
			this._ctx.appendChild(this._handle, handle);
		} else this.appendChild(n);
	}
	prepend(...nodes) {
		const ref = this._ctx.firstChild(this._handle);
		for (const n of nodes) if (typeof n === "string") {
			const handle = this._ctx.createTextNode(n);
			if (ref) this._ctx.insertBefore(this._handle, handle, ref._handle);
			else this._ctx.appendChild(this._handle, handle);
		} else if (ref) this.insertBefore(n, ref);
		else this.appendChild(n);
	}
	remove() {
		const parent = this._ctx.parentNode(this._handle);
		if (parent) parent.removeChild(this);
	}
	replaceWith(...nodes) {
		const parent = this._ctx.parentNode(this._handle);
		if (!parent) return;
		for (const n of nodes) if (typeof n === "string") {
			const handle = this._ctx.createTextNode(n);
			this._ctx.insertBefore(parent._handle, handle, this._handle);
		} else this._ctx.insertBefore(parent._handle, n._handle, this._handle);
		parent.removeChild(this);
	}
	get style() {
		return { getPropertyValue: () => "" };
	}
};
registerNodeType(Node.ELEMENT_NODE, (ctx, handle) => {
	const tagFactory = getTagFactory(ctx.tagName(handle)?.toLowerCase() ?? "");
	return tagFactory ? tagFactory(ctx, handle) : new Element(ctx, handle);
});
//#endregion
//#region src/html/html-element.ts
var HTMLElement = class extends Element {
	constructor(ctx, handle) {
		super(ctx, handle);
	}
	get hidden() {
		return this.hasAttribute("hidden");
	}
	set hidden(value) {
		if (value) this.setAttribute("hidden", "");
		else this.removeAttribute("hidden");
	}
	get title() {
		return this.getAttribute("title") ?? "";
	}
	set title(value) {
		this.setAttribute("title", value);
	}
	get lang() {
		return this.getAttribute("lang") ?? "";
	}
	set lang(value) {
		this.setAttribute("lang", value);
	}
	get dir() {
		return this.getAttribute("dir") ?? "";
	}
	set dir(value) {
		this.setAttribute("dir", value);
	}
	get tabIndex() {
		return Number(this.getAttribute("tabindex") ?? -1);
	}
	set tabIndex(value) {
		this.setAttribute("tabindex", String(value));
	}
	get draggable() {
		return this.getAttribute("draggable") === "true";
	}
	set draggable(value) {
		this.setAttribute("draggable", String(value));
	}
	get contentEditable() {
		return this.getAttribute("contenteditable") ?? "inherit";
	}
	set contentEditable(value) {
		this.setAttribute("contenteditable", value);
	}
	get isContentEditable() {
		return this.contentEditable === "true";
	}
	get innerText() {
		return this.textContent ?? "";
	}
	set innerText(value) {
		this.textContent = value;
	}
	get innerHTML() {
		return "";
	}
	set innerHTML(value) {
		if (value === "") this.textContent = "";
	}
	get outerHTML() {
		return "";
	}
	get offsetWidth() {
		return 0;
	}
	get offsetHeight() {
		return 0;
	}
	get offsetTop() {
		return 0;
	}
	get offsetLeft() {
		return 0;
	}
	get clientWidth() {
		return 0;
	}
	get clientHeight() {
		return 0;
	}
	get scrollWidth() {
		return 0;
	}
	get scrollHeight() {
		return 0;
	}
	get scrollTop() {
		return 0;
	}
	set scrollTop(_v) {}
	get scrollLeft() {
		return 0;
	}
	set scrollLeft(_v) {}
	getBoundingClientRect() {
		return {
			x: 0,
			y: 0,
			width: 0,
			height: 0,
			top: 0,
			right: 0,
			bottom: 0,
			left: 0,
			toJSON: () => ({})
		};
	}
	focus(_options) {}
	blur() {}
	click() {
		this.dispatchEvent(new Event("click", { bubbles: true }));
	}
	get dataset() {
		return {};
	}
	get spellcheck() {
		return this.getAttribute("spellcheck") !== "false";
	}
	set spellcheck(v) {
		this.setAttribute("spellcheck", String(v));
	}
	get autocapitalize() {
		return this.getAttribute("autocapitalize") ?? "";
	}
	set autocapitalize(v) {
		this.setAttribute("autocapitalize", v);
	}
};
//#endregion
//#region src/html/elements.ts
var HTMLDivElement = class extends HTMLElement {};
var HTMLSpanElement = class extends HTMLElement {};
var HTMLParagraphElement = class extends HTMLElement {};
var HTMLHeadingElement = class extends HTMLElement {};
var HTMLUListElement = class extends HTMLElement {};
var HTMLPreElement = class extends HTMLElement {};
var HTMLHRElement = class extends HTMLElement {};
var HTMLBRElement = class extends HTMLElement {};
var HTMLBodyElement = class extends HTMLElement {};
var HTMLHeadElement = class extends HTMLElement {};
var HTMLHtmlElement = class extends HTMLElement {};
var HTMLOListElement = class extends HTMLElement {
	get reversed() {
		return this.hasAttribute("reversed");
	}
	set reversed(v) {
		v ? this.setAttribute("reversed", "") : this.removeAttribute("reversed");
	}
	get start() {
		return Number(this.getAttribute("start") ?? 1);
	}
	set start(v) {
		this.setAttribute("start", String(v));
	}
	get type() {
		return this.getAttribute("type") ?? "";
	}
	set type(v) {
		this.setAttribute("type", v);
	}
};
var HTMLLIElement = class extends HTMLElement {
	get value() {
		return Number(this.getAttribute("value") ?? 0);
	}
	set value(v) {
		this.setAttribute("value", String(v));
	}
};
var HTMLQuoteElement = class extends HTMLElement {
	get cite() {
		return this.getAttribute("cite") ?? "";
	}
	set cite(v) {
		this.setAttribute("cite", v);
	}
};
var HTMLAnchorElement = class extends HTMLElement {
	get href() {
		return this.getAttribute("href") ?? "";
	}
	set href(v) {
		this.setAttribute("href", v);
	}
	get target() {
		return this.getAttribute("target") ?? "";
	}
	set target(v) {
		this.setAttribute("target", v);
	}
	get rel() {
		return this.getAttribute("rel") ?? "";
	}
	set rel(v) {
		this.setAttribute("rel", v);
	}
	get download() {
		return this.getAttribute("download") ?? "";
	}
	set download(v) {
		this.setAttribute("download", v);
	}
	get text() {
		return this.textContent ?? "";
	}
	get hash() {
		try {
			return new URL(this.href).hash;
		} catch {
			return "";
		}
	}
	get hostname() {
		try {
			return new URL(this.href).hostname;
		} catch {
			return "";
		}
	}
	get pathname() {
		try {
			return new URL(this.href).pathname;
		} catch {
			return "";
		}
	}
};
var HTMLImageElement = class extends HTMLElement {
	get src() {
		return this.getAttribute("src") ?? "";
	}
	set src(v) {
		this.setAttribute("src", v);
	}
	get alt() {
		return this.getAttribute("alt") ?? "";
	}
	set alt(v) {
		this.setAttribute("alt", v);
	}
	get width() {
		return Number(this.getAttribute("width") ?? 0);
	}
	set width(v) {
		this.setAttribute("width", String(v));
	}
	get height() {
		return Number(this.getAttribute("height") ?? 0);
	}
	set height(v) {
		this.setAttribute("height", String(v));
	}
	get naturalWidth() {
		return 0;
	}
	get naturalHeight() {
		return 0;
	}
	get complete() {
		return true;
	}
	get loading() {
		return this.getAttribute("loading") ?? "eager";
	}
	set loading(v) {
		this.setAttribute("loading", v);
	}
};
var HTMLInputElement = class extends HTMLElement {
	get type() {
		return this.getAttribute("type") ?? "text";
	}
	set type(v) {
		this.setAttribute("type", v);
	}
	get name() {
		return this.getAttribute("name") ?? "";
	}
	set name(v) {
		this.setAttribute("name", v);
	}
	get value() {
		return this.getAttribute("value") ?? "";
	}
	set value(v) {
		this.setAttribute("value", v);
	}
	get defaultValue() {
		return this.getAttribute("value") ?? "";
	}
	get checked() {
		return this.hasAttribute("checked");
	}
	set checked(v) {
		v ? this.setAttribute("checked", "") : this.removeAttribute("checked");
	}
	get defaultChecked() {
		return this.hasAttribute("checked");
	}
	get disabled() {
		return this.hasAttribute("disabled");
	}
	set disabled(v) {
		v ? this.setAttribute("disabled", "") : this.removeAttribute("disabled");
	}
	get readOnly() {
		return this.hasAttribute("readonly");
	}
	set readOnly(v) {
		v ? this.setAttribute("readonly", "") : this.removeAttribute("readonly");
	}
	get required() {
		return this.hasAttribute("required");
	}
	set required(v) {
		v ? this.setAttribute("required", "") : this.removeAttribute("required");
	}
	get placeholder() {
		return this.getAttribute("placeholder") ?? "";
	}
	set placeholder(v) {
		this.setAttribute("placeholder", v);
	}
	get min() {
		return this.getAttribute("min") ?? "";
	}
	set min(v) {
		this.setAttribute("min", v);
	}
	get max() {
		return this.getAttribute("max") ?? "";
	}
	set max(v) {
		this.setAttribute("max", v);
	}
	get step() {
		return this.getAttribute("step") ?? "";
	}
	set step(v) {
		this.setAttribute("step", v);
	}
	get multiple() {
		return this.hasAttribute("multiple");
	}
	set multiple(v) {
		v ? this.setAttribute("multiple", "") : this.removeAttribute("multiple");
	}
	get maxLength() {
		return Number(this.getAttribute("maxlength") ?? -1);
	}
	set maxLength(v) {
		this.setAttribute("maxlength", String(v));
	}
	get minLength() {
		return Number(this.getAttribute("minlength") ?? -1);
	}
	set minLength(v) {
		this.setAttribute("minlength", String(v));
	}
	get size() {
		return Number(this.getAttribute("size") ?? 20);
	}
	set size(v) {
		this.setAttribute("size", String(v));
	}
	get pattern() {
		return this.getAttribute("pattern") ?? "";
	}
	set pattern(v) {
		this.setAttribute("pattern", v);
	}
	get accept() {
		return this.getAttribute("accept") ?? "";
	}
	set accept(v) {
		this.setAttribute("accept", v);
	}
	get autofocus() {
		return this.hasAttribute("autofocus");
	}
	set autofocus(v) {
		v ? this.setAttribute("autofocus", "") : this.removeAttribute("autofocus");
	}
	select() {}
	checkValidity() {
		return true;
	}
	reportValidity() {
		return true;
	}
	setCustomValidity(_error) {}
};
var HTMLButtonElement = class extends HTMLElement {
	get type() {
		return this.getAttribute("type") ?? "submit";
	}
	set type(v) {
		this.setAttribute("type", v);
	}
	get name() {
		return this.getAttribute("name") ?? "";
	}
	set name(v) {
		this.setAttribute("name", v);
	}
	get value() {
		return this.getAttribute("value") ?? "";
	}
	set value(v) {
		this.setAttribute("value", v);
	}
	get disabled() {
		return this.hasAttribute("disabled");
	}
	set disabled(v) {
		v ? this.setAttribute("disabled", "") : this.removeAttribute("disabled");
	}
	get autofocus() {
		return this.hasAttribute("autofocus");
	}
	set autofocus(v) {
		v ? this.setAttribute("autofocus", "") : this.removeAttribute("autofocus");
	}
	checkValidity() {
		return true;
	}
};
var HTMLFormElement = class extends HTMLElement {
	get action() {
		return this.getAttribute("action") ?? "";
	}
	set action(v) {
		this.setAttribute("action", v);
	}
	get method() {
		return this.getAttribute("method") ?? "get";
	}
	set method(v) {
		this.setAttribute("method", v);
	}
	get enctype() {
		return this.getAttribute("enctype") ?? "application/x-www-form-urlencoded";
	}
	set enctype(v) {
		this.setAttribute("enctype", v);
	}
	get name() {
		return this.getAttribute("name") ?? "";
	}
	set name(v) {
		this.setAttribute("name", v);
	}
	get noValidate() {
		return this.hasAttribute("novalidate");
	}
	set noValidate(v) {
		v ? this.setAttribute("novalidate", "") : this.removeAttribute("novalidate");
	}
	get target() {
		return this.getAttribute("target") ?? "";
	}
	set target(v) {
		this.setAttribute("target", v);
	}
	submit() {
		this.dispatchEvent(new Event("submit", {
			bubbles: true,
			cancelable: true
		}));
	}
	reset() {
		this.dispatchEvent(new Event("reset", {
			bubbles: true,
			cancelable: true
		}));
	}
	checkValidity() {
		return true;
	}
	reportValidity() {
		return true;
	}
};
var HTMLTextAreaElement = class extends HTMLElement {
	get name() {
		return this.getAttribute("name") ?? "";
	}
	set name(v) {
		this.setAttribute("name", v);
	}
	get value() {
		return this.textContent ?? "";
	}
	set value(v) {
		this.textContent = v;
	}
	get rows() {
		return Number(this.getAttribute("rows") ?? 2);
	}
	set rows(v) {
		this.setAttribute("rows", String(v));
	}
	get cols() {
		return Number(this.getAttribute("cols") ?? 20);
	}
	set cols(v) {
		this.setAttribute("cols", String(v));
	}
	get disabled() {
		return this.hasAttribute("disabled");
	}
	set disabled(v) {
		v ? this.setAttribute("disabled", "") : this.removeAttribute("disabled");
	}
	get readOnly() {
		return this.hasAttribute("readonly");
	}
	set readOnly(v) {
		v ? this.setAttribute("readonly", "") : this.removeAttribute("readonly");
	}
	get required() {
		return this.hasAttribute("required");
	}
	set required(v) {
		v ? this.setAttribute("required", "") : this.removeAttribute("required");
	}
	get placeholder() {
		return this.getAttribute("placeholder") ?? "";
	}
	set placeholder(v) {
		this.setAttribute("placeholder", v);
	}
	get maxLength() {
		return Number(this.getAttribute("maxlength") ?? -1);
	}
	set maxLength(v) {
		this.setAttribute("maxlength", String(v));
	}
	get minLength() {
		return Number(this.getAttribute("minlength") ?? -1);
	}
	set minLength(v) {
		this.setAttribute("minlength", String(v));
	}
	get wrap() {
		return this.getAttribute("wrap") ?? "soft";
	}
	set wrap(v) {
		this.setAttribute("wrap", v);
	}
	get defaultValue() {
		return this.textContent ?? "";
	}
	select() {}
	checkValidity() {
		return true;
	}
	reportValidity() {
		return true;
	}
	setCustomValidity(_error) {}
};
var HTMLSelectElement = class extends HTMLElement {
	get name() {
		return this.getAttribute("name") ?? "";
	}
	set name(v) {
		this.setAttribute("name", v);
	}
	get disabled() {
		return this.hasAttribute("disabled");
	}
	set disabled(v) {
		v ? this.setAttribute("disabled", "") : this.removeAttribute("disabled");
	}
	get multiple() {
		return this.hasAttribute("multiple");
	}
	set multiple(v) {
		v ? this.setAttribute("multiple", "") : this.removeAttribute("multiple");
	}
	get size() {
		return Number(this.getAttribute("size") ?? 0);
	}
	set size(v) {
		this.setAttribute("size", String(v));
	}
	get required() {
		return this.hasAttribute("required");
	}
	set required(v) {
		v ? this.setAttribute("required", "") : this.removeAttribute("required");
	}
	get autofocus() {
		return this.hasAttribute("autofocus");
	}
	set autofocus(v) {
		v ? this.setAttribute("autofocus", "") : this.removeAttribute("autofocus");
	}
	get length() {
		return this.children.filter((c) => c.localName === "option").length;
	}
	get selectedIndex() {
		return -1;
	}
	get value() {
		return "";
	}
	set value(_v) {}
	checkValidity() {
		return true;
	}
	reportValidity() {
		return true;
	}
};
var HTMLOptionElement = class extends HTMLElement {
	get value() {
		return this.getAttribute("value") ?? this.textContent ?? "";
	}
	set value(v) {
		this.setAttribute("value", v);
	}
	get label() {
		return this.getAttribute("label") ?? this.textContent ?? "";
	}
	set label(v) {
		this.setAttribute("label", v);
	}
	get selected() {
		return this.hasAttribute("selected");
	}
	set selected(v) {
		v ? this.setAttribute("selected", "") : this.removeAttribute("selected");
	}
	get defaultSelected() {
		return this.hasAttribute("selected");
	}
	get disabled() {
		return this.hasAttribute("disabled");
	}
	set disabled(v) {
		v ? this.setAttribute("disabled", "") : this.removeAttribute("disabled");
	}
	get text() {
		return this.textContent ?? "";
	}
	get index() {
		return 0;
	}
};
var HTMLOptGroupElement = class extends HTMLElement {
	get label() {
		return this.getAttribute("label") ?? "";
	}
	set label(v) {
		this.setAttribute("label", v);
	}
	get disabled() {
		return this.hasAttribute("disabled");
	}
	set disabled(v) {
		v ? this.setAttribute("disabled", "") : this.removeAttribute("disabled");
	}
};
var HTMLLabelElement = class extends HTMLElement {
	get htmlFor() {
		return this.getAttribute("for") ?? "";
	}
	set htmlFor(v) {
		this.setAttribute("for", v);
	}
};
var HTMLScriptElement = class extends HTMLElement {
	get src() {
		return this.getAttribute("src") ?? "";
	}
	set src(v) {
		this.setAttribute("src", v);
	}
	get type() {
		return this.getAttribute("type") ?? "";
	}
	set type(v) {
		this.setAttribute("type", v);
	}
	get async() {
		return this.hasAttribute("async");
	}
	set async(v) {
		v ? this.setAttribute("async", "") : this.removeAttribute("async");
	}
	get defer() {
		return this.hasAttribute("defer");
	}
	set defer(v) {
		v ? this.setAttribute("defer", "") : this.removeAttribute("defer");
	}
	get noModule() {
		return this.hasAttribute("nomodule");
	}
	set noModule(v) {
		v ? this.setAttribute("nomodule", "") : this.removeAttribute("nomodule");
	}
	get crossOrigin() {
		return this.getAttribute("crossorigin");
	}
	set crossOrigin(v) {
		v === null ? this.removeAttribute("crossorigin") : this.setAttribute("crossorigin", v);
	}
	get text() {
		return this.textContent ?? "";
	}
	set text(v) {
		this.textContent = v;
	}
	get charset() {
		return this.getAttribute("charset") ?? "";
	}
	set charset(v) {
		this.setAttribute("charset", v);
	}
};
var HTMLLinkElement = class extends HTMLElement {
	get href() {
		return this.getAttribute("href") ?? "";
	}
	set href(v) {
		this.setAttribute("href", v);
	}
	get rel() {
		return this.getAttribute("rel") ?? "";
	}
	set rel(v) {
		this.setAttribute("rel", v);
	}
	get type() {
		return this.getAttribute("type") ?? "";
	}
	set type(v) {
		this.setAttribute("type", v);
	}
	get media() {
		return this.getAttribute("media") ?? "";
	}
	set media(v) {
		this.setAttribute("media", v);
	}
	get as() {
		return this.getAttribute("as") ?? "";
	}
	set as(v) {
		this.setAttribute("as", v);
	}
	get crossOrigin() {
		return this.getAttribute("crossorigin");
	}
	set crossOrigin(v) {
		v === null ? this.removeAttribute("crossorigin") : this.setAttribute("crossorigin", v);
	}
	get disabled() {
		return this.hasAttribute("disabled");
	}
	set disabled(v) {
		v ? this.setAttribute("disabled", "") : this.removeAttribute("disabled");
	}
};
var HTMLMetaElement = class extends HTMLElement {
	get name() {
		return this.getAttribute("name") ?? "";
	}
	set name(v) {
		this.setAttribute("name", v);
	}
	get content() {
		return this.getAttribute("content") ?? "";
	}
	set content(v) {
		this.setAttribute("content", v);
	}
	get httpEquiv() {
		return this.getAttribute("http-equiv") ?? "";
	}
	set httpEquiv(v) {
		this.setAttribute("http-equiv", v);
	}
	get charset() {
		return this.getAttribute("charset") ?? "";
	}
	set charset(v) {
		this.setAttribute("charset", v);
	}
};
var HTMLTitleElement = class extends HTMLElement {
	get text() {
		return this.textContent ?? "";
	}
	set text(v) {
		this.textContent = v;
	}
};
var HTMLStyleElement = class extends HTMLElement {
	get media() {
		return this.getAttribute("media") ?? "";
	}
	set media(v) {
		this.setAttribute("media", v);
	}
	get type() {
		return this.getAttribute("type") ?? "";
	}
	set type(v) {
		this.setAttribute("type", v);
	}
};
var HTMLTableElement = class extends HTMLElement {
	get rows() {
		const rows = [];
		for (const child of this.children) if ([
			"thead",
			"tbody",
			"tfoot"
		].includes(child.localName)) rows.push(...child.children.filter((c) => c.localName === "tr"));
		else if (child.localName === "tr") rows.push(child);
		return rows;
	}
	get caption() {
		return this.children.find((c) => c.localName === "caption") ?? null;
	}
};
var HTMLTableSectionElement = class extends HTMLElement {
	get rows() {
		return this.children.filter((c) => c.localName === "tr");
	}
};
var HTMLTableRowElement = class extends HTMLElement {
	get cells() {
		return this.children.filter((c) => c.localName === "td" || c.localName === "th");
	}
	get rowIndex() {
		return -1;
	}
	get sectionRowIndex() {
		return -1;
	}
};
var HTMLTableCellElement = class extends HTMLElement {
	get colSpan() {
		return Number(this.getAttribute("colspan") ?? 1);
	}
	set colSpan(v) {
		this.setAttribute("colspan", String(v));
	}
	get rowSpan() {
		return Number(this.getAttribute("rowspan") ?? 1);
	}
	set rowSpan(v) {
		this.setAttribute("rowspan", String(v));
	}
	get headers() {
		return this.getAttribute("headers") ?? "";
	}
	set headers(v) {
		this.setAttribute("headers", v);
	}
	get abbr() {
		return this.getAttribute("abbr") ?? "";
	}
	set abbr(v) {
		this.setAttribute("abbr", v);
	}
	get cellIndex() {
		return -1;
	}
};
var HTMLTableColElement = class extends HTMLElement {
	get span() {
		return Number(this.getAttribute("span") ?? 1);
	}
	set span(v) {
		this.setAttribute("span", String(v));
	}
};
var HTMLTableCaptionElement = class extends HTMLElement {};
var HTMLMediaElement = class extends HTMLElement {
	get src() {
		return this.getAttribute("src") ?? "";
	}
	set src(v) {
		this.setAttribute("src", v);
	}
	get currentTime() {
		return 0;
	}
	set currentTime(_v) {}
	get duration() {
		return NaN;
	}
	get paused() {
		return true;
	}
	get ended() {
		return false;
	}
	get seeking() {
		return false;
	}
	get muted() {
		return this.hasAttribute("muted");
	}
	set muted(v) {
		v ? this.setAttribute("muted", "") : this.removeAttribute("muted");
	}
	get volume() {
		return 1;
	}
	set volume(_v) {}
	get autoplay() {
		return this.hasAttribute("autoplay");
	}
	set autoplay(v) {
		v ? this.setAttribute("autoplay", "") : this.removeAttribute("autoplay");
	}
	get loop() {
		return this.hasAttribute("loop");
	}
	set loop(v) {
		v ? this.setAttribute("loop", "") : this.removeAttribute("loop");
	}
	get controls() {
		return this.hasAttribute("controls");
	}
	set controls(v) {
		v ? this.setAttribute("controls", "") : this.removeAttribute("controls");
	}
	get readyState() {
		return 0;
	}
	get networkState() {
		return 0;
	}
	get playbackRate() {
		return 1;
	}
	set playbackRate(_v) {}
	play() {
		return Promise.resolve();
	}
	pause() {}
	load() {}
	canPlayType(_type) {
		return "";
	}
};
var HTMLVideoElement = class extends HTMLMediaElement {
	get width() {
		return Number(this.getAttribute("width") ?? 0);
	}
	set width(v) {
		this.setAttribute("width", String(v));
	}
	get height() {
		return Number(this.getAttribute("height") ?? 0);
	}
	set height(v) {
		this.setAttribute("height", String(v));
	}
	get videoWidth() {
		return 0;
	}
	get videoHeight() {
		return 0;
	}
	get poster() {
		return this.getAttribute("poster") ?? "";
	}
	set poster(v) {
		this.setAttribute("poster", v);
	}
};
var HTMLAudioElement = class extends HTMLMediaElement {};
var HTMLCanvasElement = class extends HTMLElement {
	get width() {
		return Number(this.getAttribute("width") ?? 300);
	}
	set width(v) {
		this.setAttribute("width", String(v));
	}
	get height() {
		return Number(this.getAttribute("height") ?? 150);
	}
	set height(v) {
		this.setAttribute("height", String(v));
	}
	getContext(_contextId) {
		return null;
	}
	toDataURL(_type) {
		return "";
	}
};
var HTMLIFrameElement = class extends HTMLElement {
	get src() {
		return this.getAttribute("src") ?? "";
	}
	set src(v) {
		this.setAttribute("src", v);
	}
	get width() {
		return this.getAttribute("width") ?? "";
	}
	set width(v) {
		this.setAttribute("width", v);
	}
	get height() {
		return this.getAttribute("height") ?? "";
	}
	set height(v) {
		this.setAttribute("height", v);
	}
	get name() {
		return this.getAttribute("name") ?? "";
	}
	set name(v) {
		this.setAttribute("name", v);
	}
	get allow() {
		return this.getAttribute("allow") ?? "";
	}
	set allow(v) {
		this.setAttribute("allow", v);
	}
	get sandbox() {
		return this.getAttribute("sandbox") ?? "";
	}
	get contentDocument() {
		return null;
	}
	get contentWindow() {
		return null;
	}
};
var HTMLProgressElement = class extends HTMLElement {
	get value() {
		return Number(this.getAttribute("value") ?? 0);
	}
	set value(v) {
		this.setAttribute("value", String(v));
	}
	get max() {
		return Number(this.getAttribute("max") ?? 1);
	}
	set max(v) {
		this.setAttribute("max", String(v));
	}
	get position() {
		const m = this.max;
		return m > 0 ? this.value / m : -1;
	}
};
var HTMLMeterElement = class extends HTMLElement {
	get value() {
		return Number(this.getAttribute("value") ?? 0);
	}
	set value(v) {
		this.setAttribute("value", String(v));
	}
	get min() {
		return Number(this.getAttribute("min") ?? 0);
	}
	set min(v) {
		this.setAttribute("min", String(v));
	}
	get max() {
		return Number(this.getAttribute("max") ?? 1);
	}
	set max(v) {
		this.setAttribute("max", String(v));
	}
	get low() {
		return Number(this.getAttribute("low") ?? this.min);
	}
	set low(v) {
		this.setAttribute("low", String(v));
	}
	get high() {
		return Number(this.getAttribute("high") ?? this.max);
	}
	set high(v) {
		this.setAttribute("high", String(v));
	}
	get optimum() {
		return Number(this.getAttribute("optimum") ?? (this.min + this.max) / 2);
	}
	set optimum(v) {
		this.setAttribute("optimum", String(v));
	}
};
var HTMLDetailsElement = class extends HTMLElement {
	get open() {
		return this.hasAttribute("open");
	}
	set open(v) {
		v ? this.setAttribute("open", "") : this.removeAttribute("open");
	}
};
var HTMLDialogElement = class extends HTMLElement {
	constructor(..._args) {
		super(..._args);
		this._returnValue = "";
	}
	get open() {
		return this.hasAttribute("open");
	}
	get returnValue() {
		return this._returnValue;
	}
	set returnValue(v) {
		this._returnValue = v;
	}
	show() {
		this.setAttribute("open", "");
	}
	showModal() {
		this.setAttribute("open", "");
	}
	close(returnValue) {
		if (returnValue !== void 0) this._returnValue = returnValue;
		this.removeAttribute("open");
	}
};
const TAG_MAP = [
	[["div"], HTMLDivElement],
	[["span"], HTMLSpanElement],
	[["p"], HTMLParagraphElement],
	[[
		"h1",
		"h2",
		"h3",
		"h4",
		"h5",
		"h6"
	], HTMLHeadingElement],
	[["ul"], HTMLUListElement],
	[["ol"], HTMLOListElement],
	[["li"], HTMLLIElement],
	[["pre"], HTMLPreElement],
	[["blockquote", "q"], HTMLQuoteElement],
	[["hr"], HTMLHRElement],
	[["br"], HTMLBRElement],
	[["body"], HTMLBodyElement],
	[["head"], HTMLHeadElement],
	[["html"], HTMLHtmlElement],
	[["a"], HTMLAnchorElement],
	[["img"], HTMLImageElement],
	[["input"], HTMLInputElement],
	[["button"], HTMLButtonElement],
	[["form"], HTMLFormElement],
	[["textarea"], HTMLTextAreaElement],
	[["select"], HTMLSelectElement],
	[["option"], HTMLOptionElement],
	[["optgroup"], HTMLOptGroupElement],
	[["label"], HTMLLabelElement],
	[["script"], HTMLScriptElement],
	[["link"], HTMLLinkElement],
	[["meta"], HTMLMetaElement],
	[["title"], HTMLTitleElement],
	[["style"], HTMLStyleElement],
	[["table"], HTMLTableElement],
	[[
		"thead",
		"tbody",
		"tfoot"
	], HTMLTableSectionElement],
	[["tr"], HTMLTableRowElement],
	[["td", "th"], HTMLTableCellElement],
	[["col", "colgroup"], HTMLTableColElement],
	[["caption"], HTMLTableCaptionElement],
	[["video"], HTMLVideoElement],
	[["audio"], HTMLAudioElement],
	[["canvas"], HTMLCanvasElement],
	[["iframe"], HTMLIFrameElement],
	[["progress"], HTMLProgressElement],
	[["meter"], HTMLMeterElement],
	[["details"], HTMLDetailsElement],
	[["dialog"], HTMLDialogElement],
	[[
		"section",
		"article",
		"header",
		"footer",
		"main",
		"nav",
		"aside",
		"figure",
		"figcaption",
		"summary",
		"address",
		"em",
		"strong",
		"small",
		"sub",
		"sup",
		"mark",
		"i",
		"b",
		"u",
		"s",
		"del",
		"ins",
		"code",
		"kbd",
		"samp",
		"var",
		"cite",
		"abbr",
		"dfn",
		"time",
		"data",
		"wbr",
		"dl",
		"dt",
		"dd",
		"fieldset",
		"legend",
		"output",
		"datalist",
		"picture",
		"source",
		"track",
		"map",
		"area",
		"template",
		"slot",
		"noscript"
	], HTMLElement]
];
for (const [tags, Ctor] of TAG_MAP) for (const tag of tags) registerTagType(tag, (ctx, handle) => new Ctor(ctx, handle));
//#endregion
//#region src/window.ts
const windowDefs = {
	document: new Document(),
	location: {
		href: "",
		hostname: "",
		pathname: "/",
		search: "",
		hash: "",
		protocol: "about:",
		origin: "null",
		assign(_url) {},
		replace(_url) {},
		reload() {},
		toString() {
			return this.href;
		}
	},
	navigator: {
		userAgent: "quickweb/0.1",
		language: "en",
		languages: ["en"],
		onLine: true,
		cookieEnabled: false,
		platform: "rquickjs"
	},
	history: {
		length: 1,
		scrollRestoration: "auto",
		pushState(_state, _title, _url) {},
		replaceState(_state, _title, _url) {},
		go(_delta) {},
		back() {},
		forward() {}
	},
	screen: {
		width: 1280,
		height: 720,
		availWidth: 1280,
		availHeight: 720,
		colorDepth: 24,
		pixelDepth: 24
	},
	innerWidth: 1280,
	innerHeight: 720,
	outerWidth: 1280,
	outerHeight: 720,
	devicePixelRatio: 1,
	scrollX: 0,
	scrollY: 0,
	pageXOffset: 0,
	pageYOffset: 0,
	setTimeout(fn, _ms) {
		fn();
		return 0;
	},
	clearTimeout(_id) {},
	setInterval(_fn, _ms) {
		return 0;
	},
	clearInterval(_id) {},
	requestAnimationFrame(fn) {
		fn(0);
		return 0;
	},
	cancelAnimationFrame(_id) {},
	scrollTo(_x, _y) {},
	scroll(_x, _y) {},
	getComputedStyle(_el) {
		return {
			getPropertyValue: () => "",
			display: ""
		};
	},
	addEventListener(_type, _handler) {},
	removeEventListener(_type, _handler) {},
	dispatchEvent(_event) {
		return true;
	},
	alert(_message) {},
	confirm(_message) {
		return false;
	},
	prompt(_message, _default) {
		return null;
	},
	queueMicrotask(fn) {
		Promise.resolve().then(fn);
	}
};
const window = new Proxy(windowDefs, {
	get(target, key) {
		if (key === "window") return window;
		if (key in target) return target[key];
		return globalThis[key];
	},
	set(target, key, value) {
		target[key] = value;
		if (!Object.getOwnPropertyDescriptor(globalThis, key)?.get) globalThis[key] = value;
		return true;
	},
	has(target, key) {
		return key in target || key in globalThis;
	}
});
const _g = globalThis;
for (const key of Object.keys(windowDefs)) {
	if (key in _g) continue;
	Object.defineProperty(_g, key, {
		get() {
			return window[key];
		},
		set(v) {
			window[key] = v;
		},
		configurable: true,
		enumerable: false
	});
}
_g.window = window;
//#endregion
export { window };
