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
			this._ctx._nodes.delete(child);
			child = next;
		}
		if (value) {
			const textHandle = this._ctx.createTextNode(value);
			this._ctx.appendChild(this._handle, textHandle);
			const textNode = this._ctx.wrap(textHandle);
			this._ctx._nodes.add(textNode);
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
		this._ctx._nodes.add(node);
		return node;
	}
	removeChild(child) {
		this._ctx.removeChild(this._handle, child._handle);
		this._ctx._nodes.delete(child);
		return child;
	}
	insertBefore(node, refChild) {
		if (refChild === null) this._ctx.appendChild(this._handle, node._handle);
		else this._ctx.insertBefore(this._handle, node._handle, refChild._handle);
		this._ctx._nodes.add(node);
		return node;
	}
	replaceChild(node, child) {
		this._ctx.replaceChild(this._handle, node._handle, child._handle);
		this._ctx._nodes.add(node);
		this._ctx._nodes.delete(child);
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
function registerNodeType(nodeType, factory) {
	nodeRegistry.set(nodeType, factory);
}
function getNodeFactory(nodeType) {
	return nodeRegistry.get(nodeType);
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
	}
	removeChild(parent, child) {
		this._docHandle.removeChild(parent, child);
	}
	insertBefore(parent, newNode, ref) {
		this._docHandle.insertBefore(parent, newNode, ref);
	}
	replaceChild(parent, newNode, old) {
		this._docHandle.replaceChild(parent, newNode, old);
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
		ctx._handleNodeMap.set(docNode, this);
		ctx._nodes.add(this);
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
			this._ctx._nodes.add(this._ctx.wrap(handle));
		} else this.appendChild(n);
	}
	prepend(...nodes) {
		const ref = this._ctx.firstChild(this._handle);
		for (const n of nodes) if (typeof n === "string") {
			const handle = this._ctx.createTextNode(n);
			if (ref) this._ctx.insertBefore(this._handle, handle, ref._handle);
			else this._ctx.appendChild(this._handle, handle);
			this._ctx._nodes.add(this._ctx.wrap(handle));
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
			this._ctx._nodes.add(this._ctx.wrap(handle));
		} else {
			this._ctx.insertBefore(parent._handle, n._handle, this._handle);
			this._ctx._nodes.add(n);
		}
		parent.removeChild(this);
	}
	get style() {
		return { getPropertyValue: () => "" };
	}
};
registerNodeType(Node.ELEMENT_NODE, (ctx, handle) => new Element(ctx, handle));
//#endregion
//#region src/window.ts
const windowDefs = {
	Document,
	document: new Document(),
	Node,
	Element,
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
