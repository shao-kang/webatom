// Type declarations for Rust primitives injected by quickweb

declare function __createElement(tag: string): number;
declare function __createTextNode(data: string): number;
declare function __createComment(data: string): number;
declare function __documentElement(): number;

declare function __appendChild(parentId: number, childId: number): void;
declare function __removeChild(parentId: number, childId: number): void;
declare function __insertBefore(parentId: number, newId: number, refId: number): void;

declare function __parentNode(id: number): number | undefined;
declare function __firstChild(id: number): number | undefined;
declare function __lastChild(id: number): number | undefined;
declare function __nextSibling(id: number): number | undefined;
declare function __previousSibling(id: number): number | undefined;
declare function __nodeType(id: number): number | undefined;

declare function __tagName(id: number): string | undefined;
declare function __nodeValue(id: number): string | undefined;
declare function __setNodeValue(id: number, value: string): void;

declare function __getAttribute(id: number, name: string): string | undefined;
declare function __setAttribute(id: number, name: string, value: string): void;
declare function __removeAttribute(id: number, name: string): void;
