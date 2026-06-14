//#region src/event_target.ts
var Event = class {
	constructor(type, options) {
		this.defaultPrevented = false;
		this.target = null;
		this.currentTarget = null;
		this.type = type;
		this.bubbles = options?.bubbles ?? false;
		this.cancelable = options?.cancelable ?? false;
		this.timeStamp = Date.now();
	}
	preventDefault() {
		if (this.cancelable) this.defaultPrevented = true;
	}
	stopPropagation() {}
	stopImmediatePropagation() {}
};
var CustomEvent = class extends Event {
	constructor(type, options) {
		super(type, options);
		this.detail = options?.detail ?? null;
	}
};
var MouseEvent = class extends Event {
	constructor(type, options) {
		super(type, options);
		this.clientX = options?.clientX ?? 0;
		this.clientY = options?.clientY ?? 0;
		this.button = options?.button ?? 0;
		this.buttons = options?.buttons ?? 0;
		this.ctrlKey = options?.ctrlKey ?? false;
		this.shiftKey = options?.shiftKey ?? false;
		this.altKey = options?.altKey ?? false;
		this.metaKey = options?.metaKey ?? false;
	}
};
var KeyboardEvent = class extends Event {
	constructor(type, options) {
		super(type, options);
		this.key = options?.key ?? "";
		this.code = options?.code ?? "";
		this.keyCode = options?.keyCode ?? 0;
		this.ctrlKey = options?.ctrlKey ?? false;
		this.shiftKey = options?.shiftKey ?? false;
		this.altKey = options?.altKey ?? false;
		this.metaKey = options?.metaKey ?? false;
		this.repeat = options?.repeat ?? false;
	}
};
var EventTarget = class {
	constructor() {
		this._listeners = /* @__PURE__ */ new Map();
	}
	addEventListener(type, listener) {
		let list = this._listeners.get(type);
		if (!list) {
			list = [];
			this._listeners.set(type, list);
		}
		if (!list.includes(listener)) list.push(listener);
	}
	removeEventListener(type, listener) {
		const list = this._listeners.get(type);
		if (!list) return;
		const idx = list.indexOf(listener);
		if (idx !== -1) list.splice(idx, 1);
	}
	dispatchEvent(event) {
		event.currentTarget = this;
		const list = this._listeners.get(event.type)?.slice() ?? [];
		for (const fn of list) fn(event);
		return !event.defaultPrevented;
	}
};
let _ElementClass = null;
function _registerElement(cls) {
	_ElementClass = cls;
}
function wrapNode(id) {
	const type = __nodeType(id);
	if (type === 1) return new _ElementClass(id);
	if (type === 3) return new Text(id);
	if (type === 8) return new Comment(id);
	return new Node(id);
}
function childIds(id) {
	const result = [];
	let cur = __firstChild(id);
	while (cur != null) {
		result.push(cur);
		cur = __nextSibling(cur);
	}
	return result;
}
var Node = class extends EventTarget {
	static {
		this.ELEMENT_NODE = 1;
	}
	static {
		this.TEXT_NODE = 3;
	}
	static {
		this.COMMENT_NODE = 8;
	}
	static {
		this.DOCUMENT_NODE = 9;
	}
	static {
		this.DOCUMENT_FRAGMENT_NODE = 11;
	}
	constructor(id) {
		super();
		this._id = id;
	}
	get nodeType() {
		return __nodeType(this._id) ?? 0;
	}
	get nodeName() {
		return "#unknown";
	}
	get nodeValue() {
		return __nodeValue(this._id) ?? null;
	}
	set nodeValue(v) {
		if (v != null) __setNodeValue(this._id, v);
	}
	get parentNode() {
		const pid = __parentNode(this._id);
		return pid != null ? wrapNode(pid) : null;
	}
	get parentElement() {
		const p = this.parentNode;
		return p?.nodeType === 1 ? p : null;
	}
	get firstChild() {
		const id = __firstChild(this._id);
		return id != null ? wrapNode(id) : null;
	}
	get lastChild() {
		const id = __lastChild(this._id);
		return id != null ? wrapNode(id) : null;
	}
	get nextSibling() {
		const id = __nextSibling(this._id);
		return id != null ? wrapNode(id) : null;
	}
	get previousSibling() {
		const id = __previousSibling(this._id);
		return id != null ? wrapNode(id) : null;
	}
	get childNodes() {
		return childIds(this._id).map(wrapNode);
	}
	get textContent() {
		const t = this.nodeType;
		if (t === 3 || t === 8) return this.nodeValue ?? "";
		return childIds(this._id).map((id) => wrapNode(id).textContent).join("");
	}
	set textContent(value) {
		while (this.firstChild) this.removeChild(this.firstChild);
		if (value) this.appendChild(new Text(__createTextNode(value)));
	}
	hasChildNodes() {
		return __firstChild(this._id) != null;
	}
	appendChild(child) {
		__appendChild(this._id, child._id);
		return child;
	}
	removeChild(child) {
		__removeChild(this._id, child._id);
		return child;
	}
	insertBefore(newNode, ref) {
		if (!ref) return this.appendChild(newNode);
		__insertBefore(this._id, newNode._id, ref._id);
		return newNode;
	}
	replaceChild(newChild, oldChild) {
		this.insertBefore(newChild, oldChild);
		return this.removeChild(oldChild);
	}
	contains(other) {
		let cur = other;
		while (cur) {
			if (cur._id === this._id) return true;
			cur = cur.parentNode;
		}
		return false;
	}
};
var Text = class extends Node {
	get nodeName() {
		return "#text";
	}
	get data() {
		return this.nodeValue ?? "";
	}
	set data(v) {
		this.nodeValue = v;
	}
};
var Comment = class extends Node {
	get nodeName() {
		return "#comment";
	}
	get data() {
		return this.nodeValue ?? "";
	}
	set data(v) {
		this.nodeValue = v;
	}
};
//#endregion
//#region src/element.ts
var DOMTokenList = class {
	constructor(el) {
		this._el = el;
	}
	_classes() {
		return (this._el.getAttribute("class") ?? "").split(/\s+/).filter(Boolean);
	}
	_set(arr) {
		this._el.setAttribute("class", arr.join(" "));
	}
	get length() {
		return this._classes().length;
	}
	item(i) {
		return this._classes()[i] ?? null;
	}
	contains(c) {
		return this._classes().includes(c);
	}
	add(...tokens) {
		const arr = this._classes();
		for (const t of tokens) if (!arr.includes(t)) arr.push(t);
		this._set(arr);
	}
	remove(...tokens) {
		this._set(this._classes().filter((c) => !tokens.includes(c)));
	}
	toggle(c, force) {
		const has = this.contains(c);
		if (force === true || !has && force === void 0) {
			this.add(c);
			return true;
		}
		this.remove(c);
		return false;
	}
	replace(old, next) {
		this.remove(old);
		this.add(next);
	}
	toString() {
		return this._el.getAttribute("class") ?? "";
	}
};
var CSSStyleDeclaration = class {
	constructor() {
		this._map = /* @__PURE__ */ new Map();
	}
	setProperty(prop, value) {
		this._map.set(prop, value);
	}
	getPropertyValue(prop) {
		return this._map.get(prop) ?? "";
	}
	removeProperty(prop) {
		this._map.delete(prop);
	}
	set color(v) {
		this.setProperty("color", v);
	}
	get color() {
		return this.getPropertyValue("color");
	}
	set display(v) {
		this.setProperty("display", v);
	}
	get display() {
		return this.getPropertyValue("display");
	}
	set visibility(v) {
		this.setProperty("visibility", v);
	}
	set opacity(v) {
		this.setProperty("opacity", v);
	}
	set background(v) {
		this.setProperty("background", v);
	}
	set backgroundColor(v) {
		this.setProperty("background-color", v);
	}
	set fontSize(v) {
		this.setProperty("font-size", v);
	}
	set fontWeight(v) {
		this.setProperty("font-weight", v);
	}
	set fontFamily(v) {
		this.setProperty("font-family", v);
	}
	set margin(v) {
		this.setProperty("margin", v);
	}
	set padding(v) {
		this.setProperty("padding", v);
	}
	set width(v) {
		this.setProperty("width", v);
	}
	get width() {
		return this.getPropertyValue("width");
	}
	set height(v) {
		this.setProperty("height", v);
	}
	get height() {
		return this.getPropertyValue("height");
	}
	set position(v) {
		this.setProperty("position", v);
	}
	set top(v) {
		this.setProperty("top", v);
	}
	set left(v) {
		this.setProperty("left", v);
	}
	set right(v) {
		this.setProperty("right", v);
	}
	set bottom(v) {
		this.setProperty("bottom", v);
	}
	set zIndex(v) {
		this.setProperty("z-index", v);
	}
	set transform(v) {
		this.setProperty("transform", v);
	}
	set transition(v) {
		this.setProperty("transition", v);
	}
	set overflow(v) {
		this.setProperty("overflow", v);
	}
	set cursor(v) {
		this.setProperty("cursor", v);
	}
	set border(v) {
		this.setProperty("border", v);
	}
	set borderRadius(v) {
		this.setProperty("border-radius", v);
	}
	set boxShadow(v) {
		this.setProperty("box-shadow", v);
	}
	set textAlign(v) {
		this.setProperty("text-align", v);
	}
	set flex(v) {
		this.setProperty("flex", v);
	}
	set flexDirection(v) {
		this.setProperty("flex-direction", v);
	}
	set alignItems(v) {
		this.setProperty("align-items", v);
	}
	set justifyContent(v) {
		this.setProperty("justify-content", v);
	}
	set gap(v) {
		this.setProperty("gap", v);
	}
};
function matchesSelector(id, selector) {
	selector = selector.trim();
	if (selector.includes(",")) return selector.split(",").some((s) => matchesSelector(id, s.trim()));
	const tag = selector.match(/^[a-zA-Z][a-zA-Z0-9-]*/)?.[0];
	if (tag && (__tagName(id) ?? "").toLowerCase() !== tag.toLowerCase()) return false;
	for (const m of selector.matchAll(/#([\w-]+)/g)) if (__getAttribute(id, "id") !== m[1]) return false;
	const cls = " " + (__getAttribute(id, "class") ?? "") + " ";
	for (const m of selector.matchAll(/\.([\w-]+)/g)) if (!cls.includes(" " + m[1] + " ")) return false;
	return true;
}
function bfsSelect(rootId, selector, first) {
	const results = [];
	const queue = childIds(rootId);
	while (queue.length) {
		const cur = queue.shift();
		if (matchesSelector(cur, selector)) {
			const el = new Element(cur);
			if (first) return el;
			results.push(el);
		}
		queue.push(...childIds(cur));
	}
	return first ? null : results;
}
var Element = class extends Node {
	constructor(..._args) {
		super(..._args);
		this._style = new CSSStyleDeclaration();
	}
	get nodeName() {
		return this.tagName;
	}
	get tagName() {
		return __tagName(this._id) ?? "";
	}
	get id() {
		return this.getAttribute("id") ?? "";
	}
	set id(v) {
		this.setAttribute("id", v);
	}
	get className() {
		return this.getAttribute("class") ?? "";
	}
	set className(v) {
		this.setAttribute("class", v);
	}
	get classList() {
		return new DOMTokenList(this);
	}
	getAttribute(name) {
		return __getAttribute(this._id, name) ?? null;
	}
	setAttribute(name, value) {
		__setAttribute(this._id, name, String(value));
	}
	removeAttribute(name) {
		__removeAttribute(this._id, name);
	}
	hasAttribute(name) {
		return this.getAttribute(name) !== null;
	}
	toggleAttribute(name, force) {
		const has = this.hasAttribute(name);
		if (force === true || !has && force === void 0) {
			this.setAttribute(name, "");
			return true;
		}
		this.removeAttribute(name);
		return false;
	}
	get value() {
		return this.getAttribute("value") ?? "";
	}
	set value(v) {
		this.setAttribute("value", v);
	}
	get checked() {
		return this.getAttribute("checked") === "true";
	}
	set checked(v) {
		this.setAttribute("checked", v ? "true" : "false");
	}
	get textContent() {
		return this.childNodes.map((n) => n.textContent).join("");
	}
	set textContent(v) {
		while (this.firstChild) this.removeChild(this.firstChild);
		if (v) this.appendChild(new Text(__createTextNode(v)));
	}
	get innerHTML() {
		return "";
	}
	set innerHTML(_v) {}
	get outerHTML() {
		return "";
	}
	get style() {
		return this._style;
	}
	get children() {
		return this.childNodes.filter((n) => n.nodeType === 1);
	}
	get childElementCount() {
		return this.children.length;
	}
	get firstElementChild() {
		return this.children[0] ?? null;
	}
	get lastElementChild() {
		const c = this.children;
		return c[c.length - 1] ?? null;
	}
	get nextElementSibling() {
		let s = this.nextSibling;
		while (s && s.nodeType !== 1) s = s.nextSibling;
		return s;
	}
	get previousElementSibling() {
		let s = this.previousSibling;
		while (s && s.nodeType !== 1) s = s.previousSibling;
		return s;
	}
	matches(selector) {
		return matchesSelector(this._id, selector);
	}
	querySelector(selector) {
		return bfsSelect(this._id, selector, true);
	}
	querySelectorAll(selector) {
		return bfsSelect(this._id, selector, false);
	}
	closest(selector) {
		let cur = this;
		while (cur) {
			if (cur.matches(selector)) return cur;
			cur = cur.parentElement;
		}
		return null;
	}
	remove() {
		this.parentNode?.removeChild(this);
	}
	before(...nodes) {
		const p = this.parentNode;
		if (!p) return;
		for (const n of nodes) p.insertBefore(typeof n === "string" ? new Text(__createTextNode(n)) : n, this);
	}
	after(...nodes) {
		const ref = this.nextSibling;
		const p = this.parentNode;
		if (!p) return;
		for (const n of nodes) p.insertBefore(typeof n === "string" ? new Text(__createTextNode(n)) : n, ref);
	}
	replaceWith(node) {
		const p = this.parentNode;
		if (!p) return;
		p.replaceChild(typeof node === "string" ? new Text(__createTextNode(node)) : node, this);
	}
	append(...nodes) {
		for (const n of nodes) this.appendChild(typeof n === "string" ? new Text(__createTextNode(n)) : n);
	}
	prepend(...nodes) {
		const ref = this.firstChild;
		for (const n of nodes) this.insertBefore(typeof n === "string" ? new Text(__createTextNode(n)) : n, ref);
	}
	getBoundingClientRect() {
		return {
			x: 0,
			y: 0,
			width: 0,
			height: 0,
			top: 0,
			left: 0,
			right: 0,
			bottom: 0
		};
	}
	get offsetWidth() {
		return 0;
	}
	get offsetHeight() {
		return 0;
	}
	get clientWidth() {
		return 0;
	}
	get clientHeight() {
		return 0;
	}
	get offsetTop() {
		return 0;
	}
	get offsetLeft() {
		return 0;
	}
	scrollIntoView() {}
	focus() {}
	blur() {}
	click() {
		this.dispatchEvent(new globalThis.Event("click"));
	}
};
_registerElement(Element);
//#endregion
//#region src/document.ts
const _docRootId = __documentElement();
const _htmlId = __createElement("html");
const _headId = __createElement("head");
const _bodyId = __createElement("body");
__appendChild(_docRootId, _htmlId);
__appendChild(_htmlId, _headId);
__appendChild(_htmlId, _bodyId);
const _listeners = new EventTarget();
const document = {
	nodeType: 9,
	nodeName: "#document",
	get documentElement() {
		return new Element(_htmlId);
	},
	get head() {
		return new Element(_headId);
	},
	get body() {
		return new Element(_bodyId);
	},
	get doctype() {
		return null;
	},
	get title() {
		const t = this.head.querySelector("title");
		return t ? t.textContent : "";
	},
	set title(v) {
		let t = this.head.querySelector("title");
		if (!t) {
			t = this.createElement("title");
			this.head.appendChild(t);
		}
		t.textContent = v;
	},
	createElement(tag) {
		return new Element(__createElement(tag));
	},
	createElementNS(_ns, tag) {
		return this.createElement(tag);
	},
	createTextNode(data) {
		return new Text(__createTextNode(String(data)));
	},
	createComment(data) {
		return new Comment(__createComment(String(data)));
	},
	createDocumentFragment() {
		return this.createElement("template");
	},
	createEvent(_type) {
		return new Event("");
	},
	getElementById(id) {
		return this.documentElement.querySelector("#" + id);
	},
	querySelector(selector) {
		return this.documentElement.querySelector(selector);
	},
	querySelectorAll(selector) {
		return this.documentElement.querySelectorAll(selector);
	},
	get readyState() {
		return "complete";
	},
	get URL() {
		return "";
	},
	get documentURI() {
		return "";
	},
	get baseURI() {
		return "";
	},
	get charset() {
		return "UTF-8";
	},
	get compatMode() {
		return "CSS1Compat";
	},
	get cookie() {
		return "";
	},
	set cookie(_v) {},
	hasFocus() {
		return true;
	},
	addEventListener(type, handler) {
		_listeners.addEventListener(type, handler);
	},
	removeEventListener(type, handler) {
		_listeners.removeEventListener(type, handler);
	},
	dispatchEvent(event) {
		return _listeners.dispatchEvent(event);
	}
};
const window = {
	document,
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
	scrollBy(_x, _y) {},
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
	},
	window: null
};
window.window = window;
//#endregion
//#region src/index.ts
Object.assign(globalThis, {
	EventTarget,
	Event,
	CustomEvent,
	MouseEvent,
	KeyboardEvent,
	Node,
	Text,
	Comment,
	Element,
	document,
	window,
	location: window.location,
	navigator: window.navigator,
	history: window.history,
	screen: window.screen,
	setTimeout: window.setTimeout.bind(window),
	clearTimeout: window.clearTimeout.bind(window),
	setInterval: window.setInterval.bind(window),
	clearInterval: window.clearInterval.bind(window),
	requestAnimationFrame: window.requestAnimationFrame.bind(window),
	cancelAnimationFrame: window.cancelAnimationFrame.bind(window),
	getComputedStyle: window.getComputedStyle.bind(window),
	alert: window.alert.bind(window),
	confirm: window.confirm.bind(window),
	prompt: window.prompt.bind(window),
	queueMicrotask: window.queueMicrotask.bind(window)
});
//#endregion
