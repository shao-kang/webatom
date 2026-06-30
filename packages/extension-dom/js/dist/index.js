import { DocumentHandle } from "webatom_ext_native:dom";
//#region src/interface/event.ts
var Event$1 = class {
	static {
		this.NONE = 0;
	}
	static {
		this.CAPTURING_PHASE = 1;
	}
	static {
		this.AT_TARGET = 2;
	}
	static {
		this.BUBBLING_PHASE = 3;
	}
	constructor(type, init) {
		this.NONE = 0;
		this.CAPTURING_PHASE = 1;
		this.AT_TARGET = 2;
		this.BUBBLING_PHASE = 3;
		this.isTrusted = false;
		this.target = null;
		this.currentTarget = null;
		this.eventPhase = 0;
		this.defaultPrevented = false;
		this._propagationStopped = false;
		this._immediatePropagationStopped = false;
		this.type = type;
		this.bubbles = init?.bubbles ?? false;
		this.cancelable = init?.cancelable ?? false;
		this.composed = init?.composed ?? false;
		this.timeStamp = Date.now();
	}
	stopPropagation() {
		this._propagationStopped = true;
	}
	stopImmediatePropagation() {
		this._propagationStopped = true;
		this._immediatePropagationStopped = true;
	}
	preventDefault() {
		if (this.cancelable) this.defaultPrevented = true;
	}
	composedPath() {
		return [];
	}
};
var UIEvent = class extends Event$1 {
	constructor(type, init) {
		super(type, init);
		this.detail = init?.detail ?? 0;
	}
};
var KeyboardEvent = class extends UIEvent {
	static {
		this.DOM_KEY_LOCATION_STANDARD = 0;
	}
	static {
		this.DOM_KEY_LOCATION_LEFT = 1;
	}
	static {
		this.DOM_KEY_LOCATION_RIGHT = 2;
	}
	static {
		this.DOM_KEY_LOCATION_NUMPAD = 3;
	}
	constructor(type, init) {
		super(type, init);
		this.location = 0;
		this.key = init?.key ?? "";
		this.code = init?.code ?? "";
		this.altKey = init?.altKey ?? false;
		this.ctrlKey = init?.ctrlKey ?? false;
		this.shiftKey = init?.shiftKey ?? false;
		this.metaKey = init?.metaKey ?? false;
		this.repeat = init?.repeat ?? false;
	}
	getModifierState(key) {
		switch (key) {
			case "Alt": return this.altKey;
			case "Control": return this.ctrlKey;
			case "Shift": return this.shiftKey;
			case "Meta": return this.metaKey;
			default: return false;
		}
	}
};
var MouseEvent = class extends UIEvent {
	constructor(type, init) {
		super(type, init);
		this.clientX = init?.clientX ?? 0;
		this.clientY = init?.clientY ?? 0;
		this.pageX = init?.pageX ?? init?.clientX ?? 0;
		this.pageY = init?.pageY ?? init?.clientY ?? 0;
		this.screenX = init?.screenX ?? 0;
		this.screenY = init?.screenY ?? 0;
		this.button = init?.button ?? 0;
		this.buttons = init?.buttons ?? 0;
		this.altKey = init?.altKey ?? false;
		this.ctrlKey = init?.ctrlKey ?? false;
		this.shiftKey = init?.shiftKey ?? false;
		this.metaKey = init?.metaKey ?? false;
		this.relatedTarget = init?.relatedTarget ?? null;
	}
};
var FocusEvent = class extends UIEvent {
	constructor(type, init) {
		super(type, init);
		this.relatedTarget = init?.relatedTarget ?? null;
	}
};
var InputEvent = class extends Event$1 {
	constructor(type, init) {
		super(type, init);
		this.data = init?.data ?? null;
		this.inputType = init?.inputType ?? "";
	}
};
//#endregion
//#region src/interface/event-target.ts
function invokeListeners(target, event, capture) {
	const list = target._listeners.get(event.type);
	if (!list) return;
	for (const entry of [...list]) {
		if (event._immediatePropagationStopped) break;
		if (entry.capture !== capture) continue;
		if (entry.once) {
			const idx = list.indexOf(entry);
			if (idx !== -1) list.splice(idx, 1);
		}
		if (typeof entry.callback === "function") entry.callback.call(target, event);
		else entry.callback.handleEvent.call(target, event);
	}
}
var EventTarget = class {
	constructor() {
		this._listeners = /* @__PURE__ */ new Map();
	}
	/**
	* Override in subclasses to provide the bubble target (parent node, shadow host, …).
	* Return null to stop propagation at this target.
	*/
	_getParent() {
		return null;
	}
	addEventListener(type, callback, options) {
		if (!callback) return;
		const capture = typeof options === "boolean" ? options : options?.capture ?? false;
		const once = typeof options === "object" ? options?.once ?? false : false;
		const passive = typeof options === "object" ? options?.passive ?? false : false;
		if (!this._listeners.has(type)) this._listeners.set(type, []);
		const list = this._listeners.get(type);
		if (!list.some((l) => l.callback === callback && l.capture === capture)) list.push({
			callback,
			capture,
			once,
			passive
		});
	}
	removeEventListener(type, callback, options) {
		if (!callback) return;
		const capture = typeof options === "boolean" ? options : options?.capture ?? false;
		const list = this._listeners.get(type);
		if (!list) return;
		const idx = list.findIndex((l) => l.callback === callback && l.capture === capture);
		if (idx !== -1) list.splice(idx, 1);
	}
	dispatchEvent(event) {
		const path = [];
		let node = this;
		while (node) {
			path.push(node);
			node = node._getParent();
		}
		event.target = this;
		for (let i = path.length - 1; i > 0; i--) {
			if (event._propagationStopped) break;
			event.currentTarget = path[i];
			event.eventPhase = Event$1.CAPTURING_PHASE;
			invokeListeners(path[i], event, true);
		}
		if (!event._propagationStopped) {
			event.currentTarget = this;
			event.eventPhase = Event$1.AT_TARGET;
			invokeListeners(this, event, true);
			if (!event._immediatePropagationStopped) invokeListeners(this, event, false);
		}
		if (event.bubbles) for (let i = 1; i < path.length; i++) {
			if (event._propagationStopped) break;
			event.currentTarget = path[i];
			event.eventPhase = Event$1.BUBBLING_PHASE;
			invokeListeners(path[i], event, false);
		}
		event.currentTarget = null;
		event.eventPhase = Event$1.NONE;
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
var Node = class Node extends EventTarget {
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
	_getParent() {
		return this._ctx.parentNode(this._handle);
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
	constructor() {
		this._nodes = /* @__PURE__ */ new Set();
		this._nodeHandles = /* @__PURE__ */ new Map();
		this._handleNodeMap = /* @__PURE__ */ new WeakMap();
		this._docHandle = new DocumentHandle();
		this._docHandle.onEvent((event) => {
			console.log(JSON.stringify(event));
		});
	}
	_idToHandle(id) {
		if (id === null || id === void 0) return null;
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
	wrap(handle) {
		if (!handle) return null;
		const existing = this._handleNodeMap.get(handle);
		if (existing) return existing;
		const factory = getNodeFactory(this._docHandle.nodeType(handle.nodeId));
		const node = factory ? factory(this, handle) : new Node(this, handle);
		this._handleNodeMap.set(handle, node);
		if (this._docHandle.parentNode(handle.nodeId) !== null) this._nodes.add(node);
		return node;
	}
	_wrapId(id) {
		return this.wrap(this._idToHandle(id));
	}
	_attachRoot(doc) {
		this._handleNodeMap.set(doc._handle, doc);
		this._nodes.add(doc);
		this._initNodes();
	}
	_initNodes() {
		const rootHandle = this._idToHandle(this._docHandle.documentNode());
		if (rootHandle) this._traverseSubtree(rootHandle);
	}
	_traverseSubtree(h) {
		const node = this.wrap(h);
		if (node) this._nodes.add(node);
		this._docHandle.childNodes(h.nodeId).forEach((child) => {
			const childHandle = this._idToHandle(child);
			if (childHandle) this._traverseSubtree(childHandle);
		});
	}
	_releaseSubtree(h) {
		const node = this._handleNodeMap.get(h);
		if (node) this._nodes.delete(node);
		let childId = this._docHandle.firstChild(h.nodeId);
		while (childId !== null) {
			const childHandle = this._idToHandle(childId);
			if (childHandle) this._releaseSubtree(childHandle);
			childId = this._docHandle.nextSibling(childId);
		}
	}
	nodeType(node) {
		return this._docHandle.nodeType(node.nodeId);
	}
	tagName(node) {
		return this._docHandle.tagName(node.nodeId);
	}
	nodeValue(node) {
		return this._docHandle.nodeValue(node.nodeId);
	}
	setNodeValue(node, value) {
		this._docHandle.setNodeValue(node.nodeId, value);
	}
	getAttribute(node, name) {
		return this._docHandle.getAttribute(node.nodeId, name);
	}
	setAttribute(node, name, value) {
		this._docHandle.setAttribute(node.nodeId, name, value);
	}
	removeAttribute(node, name) {
		this._docHandle.removeAttribute(node.nodeId, name);
	}
	hasAttribute(node, name) {
		return this._docHandle.hasAttribute(node.nodeId, name);
	}
	attributes(node) {
		return this._docHandle.attributes(node.nodeId);
	}
	parentNode(node) {
		return this._wrapId(this._docHandle.parentNode(node.nodeId));
	}
	firstChild(node) {
		return this._wrapId(this._docHandle.firstChild(node.nodeId));
	}
	lastChild(node) {
		return this._wrapId(this._docHandle.lastChild(node.nodeId));
	}
	nextSibling(node) {
		return this._wrapId(this._docHandle.nextSibling(node.nodeId));
	}
	previousSibling(node) {
		return this._wrapId(this._docHandle.previousSibling(node.nodeId));
	}
	documentElement() {
		return this._wrapId(this._docHandle.documentElement());
	}
	body() {
		return this._wrapId(this._docHandle.body());
	}
	head() {
		return this._wrapId(this._docHandle.head());
	}
	appendChild(parent, child) {
		this._docHandle.appendChild(parent.nodeId, child.nodeId);
		const parentNode = this._handleNodeMap.get(parent);
		if (parentNode && this._nodes.has(parentNode)) this._traverseSubtree(child);
	}
	removeChild(parent, child) {
		this._docHandle.removeChild(parent.nodeId, child.nodeId);
		const childNode = this._handleNodeMap.get(child);
		if (childNode && this._nodes.has(childNode)) this._releaseSubtree(child);
	}
	insertBefore(parent, newNode, ref) {
		this._docHandle.insertBefore(parent.nodeId, newNode.nodeId, ref.nodeId);
		const parentNode = this._handleNodeMap.get(parent);
		if (parentNode && this._nodes.has(parentNode)) this._traverseSubtree(newNode);
	}
	replaceChild(parent, newNode, old) {
		this._docHandle.replaceChild(parent.nodeId, newNode.nodeId, old.nodeId);
		const parentNode = this._handleNodeMap.get(parent);
		if (parentNode && this._nodes.has(parentNode)) this._traverseSubtree(newNode);
		const oldNode = this._handleNodeMap.get(old);
		if (oldNode && this._nodes.has(oldNode)) this._releaseSubtree(old);
	}
	createElement(tagName) {
		return this._idToHandle(this._docHandle.createElement(tagName));
	}
	createTextNode(data) {
		return this._idToHandle(this._docHandle.createTextNode(data));
	}
	createComment(data) {
		return this._idToHandle(this._docHandle.createComment(data));
	}
	documentNode() {
		return this._idToHandle(this._docHandle.documentNode());
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
const location = {
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
};
const navigator = {
	userAgent: "quickweb/0.1",
	language: "en",
	languages: ["en"],
	onLine: true,
	cookieEnabled: false,
	platform: "rquickjs"
};
const history = {
	length: 1,
	scrollRestoration: "auto",
	pushState(_state, _title, _url) {},
	replaceState(_state, _title, _url) {},
	go(_delta) {},
	back() {},
	forward() {}
};
const screen = {
	width: 1280,
	height: 720,
	availWidth: 1280,
	availHeight: 720,
	colorDepth: 24,
	pixelDepth: 24
};
const _winEvTarget = new EventTarget();
const windowDefs = {
	document: new Document(),
	location,
	navigator,
	history,
	screen,
	innerWidth: 1280,
	innerHeight: 720,
	outerWidth: 1280,
	outerHeight: 720,
	devicePixelRatio: 1,
	scrollX: 0,
	scrollY: 0,
	pageXOffset: 0,
	pageYOffset: 0,
	Event: Event$1,
	UIEvent,
	KeyboardEvent,
	MouseEvent,
	FocusEvent,
	InputEvent,
	EventTarget,
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
	addEventListener(type, cb, opts) {
		_winEvTarget.addEventListener(type, cb, opts);
	},
	removeEventListener(type, cb, opts) {
		_winEvTarget.removeEventListener(type, cb, opts);
	},
	dispatchEvent(event) {
		return _winEvTarget.dispatchEvent(event);
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
