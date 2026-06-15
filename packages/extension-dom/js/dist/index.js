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
const nodeRegistry = /* @__PURE__ */ new Map();
function registerNodeType(nodeType, factory) {
	nodeRegistry.set(nodeType, factory);
}
function wrapHandleWith(ctx, handle) {
	if (!handle) return null;
	const existing = ctx._handleNodeMap.get(handle);
	if (existing) return existing;
	const type = ctx.nodeType(handle);
	const factory = nodeRegistry.get(type);
	const node = factory ? factory(ctx, handle) : new Node(ctx, handle);
	ctx._handleNodeMap.set(handle, node);
	return node;
}
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
	get _docCtx() {
		return document.__docCtx;
	}
	constructor(handle) {
		super();
		this._handle = handle;
	}
	_wrap(h) {
		return wrapHandleWith(this._docCtx, h);
	}
	get parentNode() {
		return this._wrap(this._docCtx.parentNode(this._handle));
	}
	get parentElement() {
		const h = this._docCtx.parentNode(this._handle);
		if (!h) return null;
		return this._docCtx.nodeType(h) === Node.ELEMENT_NODE ? this._wrap(h) : null;
	}
	get firstChild() {
		return this._wrap(this._docCtx.firstChild(this._handle));
	}
	get lastChild() {
		return this._wrap(this._docCtx.lastChild(this._handle));
	}
	get nextSibling() {
		return this._wrap(this._docCtx.nextSibling(this._handle));
	}
	get previousSibling() {
		return this._wrap(this._docCtx.previousSibling(this._handle));
	}
	get childNodes() {
		const result = [];
		let h = this._docCtx.firstChild(this._handle);
		while (h) {
			result.push(this._wrap(h));
			h = this._docCtx.nextSibling(h);
		}
		return result;
	}
	hasChildNodes() {
		return this._docCtx.firstChild(this._handle) !== null;
	}
	get nodeValue() {
		return this._docCtx.nodeValue(this._handle);
	}
	set nodeValue(value) {
		this._docCtx.setNodeValue(this._handle, value);
	}
	get textContent() {
		const t = this.nodeType;
		if (t === Node.DOCUMENT_NODE || t === Node.DOCUMENT_TYPE_NODE) return null;
		if (t === Node.TEXT_NODE || t === Node.CDATA_SECTION_NODE || t === Node.COMMENT_NODE || t === Node.PROCESSING_INSTRUCTION_NODE) return this._docCtx.nodeValue(this._handle);
		return this._collectText(this._handle);
	}
	set textContent(value) {
		let h = this._docCtx.firstChild(this._handle);
		while (h) {
			const next = this._docCtx.nextSibling(h);
			this._docCtx.removeChild(this._handle, h);
			const child = this._docCtx._handleNodeMap.get(h);
			if (child) this._docCtx._nodes.delete(child);
			h = next;
		}
		if (value) {
			const text = this._docCtx.createTextNode(value);
			this._docCtx.appendChild(this._handle, text);
		}
	}
	_collectText(h) {
		let text = "";
		let child = this._docCtx.firstChild(h);
		while (child) {
			const t = this._docCtx.nodeType(child);
			text += t === Node.TEXT_NODE || t === Node.CDATA_SECTION_NODE ? this._docCtx.nodeValue(child) ?? "" : this._collectText(child);
			child = this._docCtx.nextSibling(child);
		}
		return text;
	}
	appendChild(node) {
		this._docCtx.appendChild(this._handle, node._handle);
		this._docCtx._nodes.add(node);
		return node;
	}
	removeChild(child) {
		this._docCtx.removeChild(this._handle, child._handle);
		this._docCtx._nodes.delete(child);
		return child;
	}
	insertBefore(node, refChild) {
		if (refChild === null) this._docCtx.appendChild(this._handle, node._handle);
		else this._docCtx.insertBefore(this._handle, node._handle, refChild._handle);
		this._docCtx._nodes.add(node);
		return node;
	}
	replaceChild(node, child) {
		this._docCtx.replaceChild(this._handle, node._handle, child._handle);
		this._docCtx._nodes.add(node);
		this._docCtx._nodes.delete(child);
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
//#region src/interface/document-context.ts
var DocumentContext = class {
	constructor() {
		this._nodes = /* @__PURE__ */ new Set();
		this._handleNodeMap = /* @__PURE__ */ new WeakMap();
		this._docHandle = new DocumentHandle();
	}
	getNode(node) {
		return this._handleNodeMap.get(node);
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
	parentNode(node) {
		const handle = this._docHandle.parentNode(node);
		return handle ? this.getNode(handle) : null;
	}
	firstChild(node) {
		const handle = this._docHandle.firstChild(node);
		return handle ? this.getNode(handle) : null;
	}
	lastChild(node) {
		const handle = this._docHandle.lastChild(node);
		return handle ? this.getNode(handle) : null;
	}
	nextSibling(node) {
		const handle = this._docHandle.nextSibling(node);
		return handle ? this.getNode(handle) : null;
	}
	previousSibling(node) {
		const handle = this._docHandle.previousSibling(node);
		return handle ? this.getNode(handle) : null;
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
	documentElement() {
		return this._docHandle.documentElement();
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
};
//#endregion
//#region src/interface/document.ts
var Document = class extends Node {
	constructor() {
		super();
		this._ctx = new DocumentContext();
	}
	get _docCtx() {
		return this._ctx;
	}
	get ownerDocument() {
		return null;
	}
	get documentElement() {
		return this._wrap(this._docCtx.documentElement());
	}
	createElement(tagName) {
		return wrapHandleWith(this._docCtx, this._docCtx.createElement(tagName));
	}
	createTextNode(data) {
		return wrapHandleWith(this._docCtx, this._docCtx.createTextNode(data));
	}
	createComment(data) {
		return wrapHandleWith(this._docCtx, this._docCtx.createComment(data));
	}
};
registerNodeType(Node.DOCUMENT_NODE, () => {
	throw new DOMException("Document nodes must be constructed directly", "NotSupportedError");
});
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
		return true;
	},
	has(target, key) {
		return key in target || key in globalThis;
	}
});
globalThis.window = window;
//#endregion
export { window };
