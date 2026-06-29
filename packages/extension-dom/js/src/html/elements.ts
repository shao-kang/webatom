// Concrete HTML element classes. Importing this module registers all tag factories.

import { HTMLElement } from './html-element';
import type { DocumentContext } from '@/interface/document-context';
import type { NodeHandle } from '@/interface/native';
import { registerTagType } from './index';

// ── Structural ────────────────────────────────────────────────────────────

export class HTMLDivElement extends HTMLElement {}
export class HTMLSpanElement extends HTMLElement {}
export class HTMLParagraphElement extends HTMLElement {}
export class HTMLHeadingElement extends HTMLElement {}
export class HTMLUListElement extends HTMLElement {}
export class HTMLPreElement extends HTMLElement {}
export class HTMLHRElement extends HTMLElement {}
export class HTMLBRElement extends HTMLElement {}
export class HTMLBodyElement extends HTMLElement {}
export class HTMLHeadElement extends HTMLElement {}
export class HTMLHtmlElement extends HTMLElement {}

export class HTMLOListElement extends HTMLElement {
  get reversed(): boolean { return this.hasAttribute('reversed'); }
  set reversed(v: boolean) { v ? this.setAttribute('reversed', '') : this.removeAttribute('reversed'); }
  get start(): number { return Number(this.getAttribute('start') ?? 1); }
  set start(v: number) { this.setAttribute('start', String(v)); }
  get type(): string { return this.getAttribute('type') ?? ''; }
  set type(v: string) { this.setAttribute('type', v); }
}

export class HTMLLIElement extends HTMLElement {
  get value(): number { return Number(this.getAttribute('value') ?? 0); }
  set value(v: number) { this.setAttribute('value', String(v)); }
}

export class HTMLQuoteElement extends HTMLElement {
  get cite(): string { return this.getAttribute('cite') ?? ''; }
  set cite(v: string) { this.setAttribute('cite', v); }
}

// ── Anchor ────────────────────────────────────────────────────────────────

export class HTMLAnchorElement extends HTMLElement {
  get href(): string { return this.getAttribute('href') ?? ''; }
  set href(v: string) { this.setAttribute('href', v); }
  get target(): string { return this.getAttribute('target') ?? ''; }
  set target(v: string) { this.setAttribute('target', v); }
  get rel(): string { return this.getAttribute('rel') ?? ''; }
  set rel(v: string) { this.setAttribute('rel', v); }
  get download(): string { return this.getAttribute('download') ?? ''; }
  set download(v: string) { this.setAttribute('download', v); }
  get text(): string { return this.textContent ?? ''; }
  get hash(): string { try { return new URL(this.href).hash; } catch { return ''; } }
  get hostname(): string { try { return new URL(this.href).hostname; } catch { return ''; } }
  get pathname(): string { try { return new URL(this.href).pathname; } catch { return ''; } }
}

// ── Image ─────────────────────────────────────────────────────────────────

export class HTMLImageElement extends HTMLElement {
  get src(): string { return this.getAttribute('src') ?? ''; }
  set src(v: string) { this.setAttribute('src', v); }
  get alt(): string { return this.getAttribute('alt') ?? ''; }
  set alt(v: string) { this.setAttribute('alt', v); }
  get width(): number { return Number(this.getAttribute('width') ?? 0); }
  set width(v: number) { this.setAttribute('width', String(v)); }
  get height(): number { return Number(this.getAttribute('height') ?? 0); }
  set height(v: number) { this.setAttribute('height', String(v)); }
  get naturalWidth(): number { return 0; }
  get naturalHeight(): number { return 0; }
  get complete(): boolean { return true; }
  get loading(): string { return this.getAttribute('loading') ?? 'eager'; }
  set loading(v: string) { this.setAttribute('loading', v); }
}

// ── Input ─────────────────────────────────────────────────────────────────

export class HTMLInputElement extends HTMLElement {
  get type(): string { return this.getAttribute('type') ?? 'text'; }
  set type(v: string) { this.setAttribute('type', v); }
  get name(): string { return this.getAttribute('name') ?? ''; }
  set name(v: string) { this.setAttribute('name', v); }
  get value(): string { return this.getAttribute('value') ?? ''; }
  set value(v: string) { this.setAttribute('value', v); }
  get defaultValue(): string { return this.getAttribute('value') ?? ''; }
  get checked(): boolean { return this.hasAttribute('checked'); }
  set checked(v: boolean) { v ? this.setAttribute('checked', '') : this.removeAttribute('checked'); }
  get defaultChecked(): boolean { return this.hasAttribute('checked'); }
  get disabled(): boolean { return this.hasAttribute('disabled'); }
  set disabled(v: boolean) { v ? this.setAttribute('disabled', '') : this.removeAttribute('disabled'); }
  get readOnly(): boolean { return this.hasAttribute('readonly'); }
  set readOnly(v: boolean) { v ? this.setAttribute('readonly', '') : this.removeAttribute('readonly'); }
  get required(): boolean { return this.hasAttribute('required'); }
  set required(v: boolean) { v ? this.setAttribute('required', '') : this.removeAttribute('required'); }
  get placeholder(): string { return this.getAttribute('placeholder') ?? ''; }
  set placeholder(v: string) { this.setAttribute('placeholder', v); }
  get min(): string { return this.getAttribute('min') ?? ''; }
  set min(v: string) { this.setAttribute('min', v); }
  get max(): string { return this.getAttribute('max') ?? ''; }
  set max(v: string) { this.setAttribute('max', v); }
  get step(): string { return this.getAttribute('step') ?? ''; }
  set step(v: string) { this.setAttribute('step', v); }
  get multiple(): boolean { return this.hasAttribute('multiple'); }
  set multiple(v: boolean) { v ? this.setAttribute('multiple', '') : this.removeAttribute('multiple'); }
  get maxLength(): number { return Number(this.getAttribute('maxlength') ?? -1); }
  set maxLength(v: number) { this.setAttribute('maxlength', String(v)); }
  get minLength(): number { return Number(this.getAttribute('minlength') ?? -1); }
  set minLength(v: number) { this.setAttribute('minlength', String(v)); }
  get size(): number { return Number(this.getAttribute('size') ?? 20); }
  set size(v: number) { this.setAttribute('size', String(v)); }
  get pattern(): string { return this.getAttribute('pattern') ?? ''; }
  set pattern(v: string) { this.setAttribute('pattern', v); }
  get accept(): string { return this.getAttribute('accept') ?? ''; }
  set accept(v: string) { this.setAttribute('accept', v); }
  get autofocus(): boolean { return this.hasAttribute('autofocus'); }
  set autofocus(v: boolean) { v ? this.setAttribute('autofocus', '') : this.removeAttribute('autofocus'); }
  select(): void {}
  checkValidity(): boolean { return true; }
  reportValidity(): boolean { return true; }
  setCustomValidity(_error: string): void {}
}

// ── Button ────────────────────────────────────────────────────────────────

export class HTMLButtonElement extends HTMLElement {
  get type(): string { return this.getAttribute('type') ?? 'submit'; }
  set type(v: string) { this.setAttribute('type', v); }
  get name(): string { return this.getAttribute('name') ?? ''; }
  set name(v: string) { this.setAttribute('name', v); }
  get value(): string { return this.getAttribute('value') ?? ''; }
  set value(v: string) { this.setAttribute('value', v); }
  get disabled(): boolean { return this.hasAttribute('disabled'); }
  set disabled(v: boolean) { v ? this.setAttribute('disabled', '') : this.removeAttribute('disabled'); }
  get autofocus(): boolean { return this.hasAttribute('autofocus'); }
  set autofocus(v: boolean) { v ? this.setAttribute('autofocus', '') : this.removeAttribute('autofocus'); }
  checkValidity(): boolean { return true; }
}

// ── Form ──────────────────────────────────────────────────────────────────

export class HTMLFormElement extends HTMLElement {
  get action(): string { return this.getAttribute('action') ?? ''; }
  set action(v: string) { this.setAttribute('action', v); }
  get method(): string { return this.getAttribute('method') ?? 'get'; }
  set method(v: string) { this.setAttribute('method', v); }
  get enctype(): string { return this.getAttribute('enctype') ?? 'application/x-www-form-urlencoded'; }
  set enctype(v: string) { this.setAttribute('enctype', v); }
  get name(): string { return this.getAttribute('name') ?? ''; }
  set name(v: string) { this.setAttribute('name', v); }
  get noValidate(): boolean { return this.hasAttribute('novalidate'); }
  set noValidate(v: boolean) { v ? this.setAttribute('novalidate', '') : this.removeAttribute('novalidate'); }
  get target(): string { return this.getAttribute('target') ?? ''; }
  set target(v: string) { this.setAttribute('target', v); }
  submit(): void { this.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); }
  reset(): void { this.dispatchEvent(new Event('reset', { bubbles: true, cancelable: true })); }
  checkValidity(): boolean { return true; }
  reportValidity(): boolean { return true; }
}

// ── TextArea ──────────────────────────────────────────────────────────────

export class HTMLTextAreaElement extends HTMLElement {
  get name(): string { return this.getAttribute('name') ?? ''; }
  set name(v: string) { this.setAttribute('name', v); }
  get value(): string { return this.textContent ?? ''; }
  set value(v: string) { this.textContent = v; }
  get rows(): number { return Number(this.getAttribute('rows') ?? 2); }
  set rows(v: number) { this.setAttribute('rows', String(v)); }
  get cols(): number { return Number(this.getAttribute('cols') ?? 20); }
  set cols(v: number) { this.setAttribute('cols', String(v)); }
  get disabled(): boolean { return this.hasAttribute('disabled'); }
  set disabled(v: boolean) { v ? this.setAttribute('disabled', '') : this.removeAttribute('disabled'); }
  get readOnly(): boolean { return this.hasAttribute('readonly'); }
  set readOnly(v: boolean) { v ? this.setAttribute('readonly', '') : this.removeAttribute('readonly'); }
  get required(): boolean { return this.hasAttribute('required'); }
  set required(v: boolean) { v ? this.setAttribute('required', '') : this.removeAttribute('required'); }
  get placeholder(): string { return this.getAttribute('placeholder') ?? ''; }
  set placeholder(v: string) { this.setAttribute('placeholder', v); }
  get maxLength(): number { return Number(this.getAttribute('maxlength') ?? -1); }
  set maxLength(v: number) { this.setAttribute('maxlength', String(v)); }
  get minLength(): number { return Number(this.getAttribute('minlength') ?? -1); }
  set minLength(v: number) { this.setAttribute('minlength', String(v)); }
  get wrap(): string { return this.getAttribute('wrap') ?? 'soft'; }
  set wrap(v: string) { this.setAttribute('wrap', v); }
  get defaultValue(): string { return this.textContent ?? ''; }
  select(): void {}
  checkValidity(): boolean { return true; }
  reportValidity(): boolean { return true; }
  setCustomValidity(_error: string): void {}
}

// ── Select / Option ───────────────────────────────────────────────────────

export class HTMLSelectElement extends HTMLElement {
  get name(): string { return this.getAttribute('name') ?? ''; }
  set name(v: string) { this.setAttribute('name', v); }
  get disabled(): boolean { return this.hasAttribute('disabled'); }
  set disabled(v: boolean) { v ? this.setAttribute('disabled', '') : this.removeAttribute('disabled'); }
  get multiple(): boolean { return this.hasAttribute('multiple'); }
  set multiple(v: boolean) { v ? this.setAttribute('multiple', '') : this.removeAttribute('multiple'); }
  get size(): number { return Number(this.getAttribute('size') ?? 0); }
  set size(v: number) { this.setAttribute('size', String(v)); }
  get required(): boolean { return this.hasAttribute('required'); }
  set required(v: boolean) { v ? this.setAttribute('required', '') : this.removeAttribute('required'); }
  get autofocus(): boolean { return this.hasAttribute('autofocus'); }
  set autofocus(v: boolean) { v ? this.setAttribute('autofocus', '') : this.removeAttribute('autofocus'); }
  get length(): number { return this.children.filter(c => c.localName === 'option').length; }
  get selectedIndex(): number { return -1; }
  get value(): string { return ''; }
  set value(_v: string) {}
  checkValidity(): boolean { return true; }
  reportValidity(): boolean { return true; }
}

export class HTMLOptionElement extends HTMLElement {
  get value(): string { return this.getAttribute('value') ?? this.textContent ?? ''; }
  set value(v: string) { this.setAttribute('value', v); }
  get label(): string { return this.getAttribute('label') ?? this.textContent ?? ''; }
  set label(v: string) { this.setAttribute('label', v); }
  get selected(): boolean { return this.hasAttribute('selected'); }
  set selected(v: boolean) { v ? this.setAttribute('selected', '') : this.removeAttribute('selected'); }
  get defaultSelected(): boolean { return this.hasAttribute('selected'); }
  get disabled(): boolean { return this.hasAttribute('disabled'); }
  set disabled(v: boolean) { v ? this.setAttribute('disabled', '') : this.removeAttribute('disabled'); }
  get text(): string { return this.textContent ?? ''; }
  get index(): number { return 0; }
}

export class HTMLOptGroupElement extends HTMLElement {
  get label(): string { return this.getAttribute('label') ?? ''; }
  set label(v: string) { this.setAttribute('label', v); }
  get disabled(): boolean { return this.hasAttribute('disabled'); }
  set disabled(v: boolean) { v ? this.setAttribute('disabled', '') : this.removeAttribute('disabled'); }
}

export class HTMLLabelElement extends HTMLElement {
  get htmlFor(): string { return this.getAttribute('for') ?? ''; }
  set htmlFor(v: string) { this.setAttribute('for', v); }
}

// ── Metadata ──────────────────────────────────────────────────────────────

export class HTMLScriptElement extends HTMLElement {
  get src(): string { return this.getAttribute('src') ?? ''; }
  set src(v: string) { this.setAttribute('src', v); }
  get type(): string { return this.getAttribute('type') ?? ''; }
  set type(v: string) { this.setAttribute('type', v); }
  get async(): boolean { return this.hasAttribute('async'); }
  set async(v: boolean) { v ? this.setAttribute('async', '') : this.removeAttribute('async'); }
  get defer(): boolean { return this.hasAttribute('defer'); }
  set defer(v: boolean) { v ? this.setAttribute('defer', '') : this.removeAttribute('defer'); }
  get noModule(): boolean { return this.hasAttribute('nomodule'); }
  set noModule(v: boolean) { v ? this.setAttribute('nomodule', '') : this.removeAttribute('nomodule'); }
  get crossOrigin(): string | null { return this.getAttribute('crossorigin'); }
  set crossOrigin(v: string | null) {
    v === null ? this.removeAttribute('crossorigin') : this.setAttribute('crossorigin', v);
  }
  get text(): string { return this.textContent ?? ''; }
  set text(v: string) { this.textContent = v; }
  get charset(): string { return this.getAttribute('charset') ?? ''; }
  set charset(v: string) { this.setAttribute('charset', v); }
}

export class HTMLLinkElement extends HTMLElement {
  get href(): string { return this.getAttribute('href') ?? ''; }
  set href(v: string) { this.setAttribute('href', v); }
  get rel(): string { return this.getAttribute('rel') ?? ''; }
  set rel(v: string) { this.setAttribute('rel', v); }
  get type(): string { return this.getAttribute('type') ?? ''; }
  set type(v: string) { this.setAttribute('type', v); }
  get media(): string { return this.getAttribute('media') ?? ''; }
  set media(v: string) { this.setAttribute('media', v); }
  get as(): string { return this.getAttribute('as') ?? ''; }
  set as(v: string) { this.setAttribute('as', v); }
  get crossOrigin(): string | null { return this.getAttribute('crossorigin'); }
  set crossOrigin(v: string | null) {
    v === null ? this.removeAttribute('crossorigin') : this.setAttribute('crossorigin', v);
  }
  get disabled(): boolean { return this.hasAttribute('disabled'); }
  set disabled(v: boolean) { v ? this.setAttribute('disabled', '') : this.removeAttribute('disabled'); }
}

export class HTMLMetaElement extends HTMLElement {
  get name(): string { return this.getAttribute('name') ?? ''; }
  set name(v: string) { this.setAttribute('name', v); }
  get content(): string { return this.getAttribute('content') ?? ''; }
  set content(v: string) { this.setAttribute('content', v); }
  get httpEquiv(): string { return this.getAttribute('http-equiv') ?? ''; }
  set httpEquiv(v: string) { this.setAttribute('http-equiv', v); }
  get charset(): string { return this.getAttribute('charset') ?? ''; }
  set charset(v: string) { this.setAttribute('charset', v); }
}

export class HTMLTitleElement extends HTMLElement {
  get text(): string { return this.textContent ?? ''; }
  set text(v: string) { this.textContent = v; }
}

export class HTMLStyleElement extends HTMLElement {
  get media(): string { return this.getAttribute('media') ?? ''; }
  set media(v: string) { this.setAttribute('media', v); }
  get type(): string { return this.getAttribute('type') ?? ''; }
  set type(v: string) { this.setAttribute('type', v); }
}

// ── Table ─────────────────────────────────────────────────────────────────

export class HTMLTableElement extends HTMLElement {
  get rows(): HTMLElement[] {
    const rows: HTMLElement[] = [];
    for (const child of this.children) {
      if (['thead','tbody','tfoot'].includes(child.localName)) {
        rows.push(...(child as HTMLElement).children.filter(c => c.localName === 'tr') as HTMLElement[]);
      } else if (child.localName === 'tr') {
        rows.push(child as HTMLElement);
      }
    }
    return rows;
  }
  get caption(): HTMLElement | null {
    return (this.children.find(c => c.localName === 'caption') as HTMLElement) ?? null;
  }
}

export class HTMLTableSectionElement extends HTMLElement {
  get rows(): HTMLElement[] {
    return this.children.filter(c => c.localName === 'tr') as HTMLElement[];
  }
}

export class HTMLTableRowElement extends HTMLElement {
  get cells(): HTMLElement[] {
    return this.children.filter(c => c.localName === 'td' || c.localName === 'th') as HTMLElement[];
  }
  get rowIndex(): number { return -1; }
  get sectionRowIndex(): number { return -1; }
}

export class HTMLTableCellElement extends HTMLElement {
  get colSpan(): number { return Number(this.getAttribute('colspan') ?? 1); }
  set colSpan(v: number) { this.setAttribute('colspan', String(v)); }
  get rowSpan(): number { return Number(this.getAttribute('rowspan') ?? 1); }
  set rowSpan(v: number) { this.setAttribute('rowspan', String(v)); }
  get headers(): string { return this.getAttribute('headers') ?? ''; }
  set headers(v: string) { this.setAttribute('headers', v); }
  get abbr(): string { return this.getAttribute('abbr') ?? ''; }
  set abbr(v: string) { this.setAttribute('abbr', v); }
  get cellIndex(): number { return -1; }
}

export class HTMLTableColElement extends HTMLElement {
  get span(): number { return Number(this.getAttribute('span') ?? 1); }
  set span(v: number) { this.setAttribute('span', String(v)); }
}

export class HTMLTableCaptionElement extends HTMLElement {}

// ── Media ─────────────────────────────────────────────────────────────────

export class HTMLMediaElement extends HTMLElement {
  get src(): string { return this.getAttribute('src') ?? ''; }
  set src(v: string) { this.setAttribute('src', v); }
  get currentTime(): number { return 0; }
  set currentTime(_v: number) {}
  get duration(): number { return NaN; }
  get paused(): boolean { return true; }
  get ended(): boolean { return false; }
  get seeking(): boolean { return false; }
  get muted(): boolean { return this.hasAttribute('muted'); }
  set muted(v: boolean) { v ? this.setAttribute('muted', '') : this.removeAttribute('muted'); }
  get volume(): number { return 1; }
  set volume(_v: number) {}
  get autoplay(): boolean { return this.hasAttribute('autoplay'); }
  set autoplay(v: boolean) { v ? this.setAttribute('autoplay', '') : this.removeAttribute('autoplay'); }
  get loop(): boolean { return this.hasAttribute('loop'); }
  set loop(v: boolean) { v ? this.setAttribute('loop', '') : this.removeAttribute('loop'); }
  get controls(): boolean { return this.hasAttribute('controls'); }
  set controls(v: boolean) { v ? this.setAttribute('controls', '') : this.removeAttribute('controls'); }
  get readyState(): number { return 0; }
  get networkState(): number { return 0; }
  get playbackRate(): number { return 1; }
  set playbackRate(_v: number) {}
  play(): Promise<void> { return Promise.resolve(); }
  pause(): void {}
  load(): void {}
  canPlayType(_type: string): CanPlayTypeResult { return ''; }
}

export class HTMLVideoElement extends HTMLMediaElement {
  get width(): number { return Number(this.getAttribute('width') ?? 0); }
  set width(v: number) { this.setAttribute('width', String(v)); }
  get height(): number { return Number(this.getAttribute('height') ?? 0); }
  set height(v: number) { this.setAttribute('height', String(v)); }
  get videoWidth(): number { return 0; }
  get videoHeight(): number { return 0; }
  get poster(): string { return this.getAttribute('poster') ?? ''; }
  set poster(v: string) { this.setAttribute('poster', v); }
}

export class HTMLAudioElement extends HTMLMediaElement {}

// ── Canvas ────────────────────────────────────────────────────────────────

export class HTMLCanvasElement extends HTMLElement {
  get width(): number { return Number(this.getAttribute('width') ?? 300); }
  set width(v: number) { this.setAttribute('width', String(v)); }
  get height(): number { return Number(this.getAttribute('height') ?? 150); }
  set height(v: number) { this.setAttribute('height', String(v)); }
  getContext(_contextId: string): null { return null; }
  toDataURL(_type?: string): string { return ''; }
}

// ── IFrame ────────────────────────────────────────────────────────────────

export class HTMLIFrameElement extends HTMLElement {
  get src(): string { return this.getAttribute('src') ?? ''; }
  set src(v: string) { this.setAttribute('src', v); }
  get width(): string { return this.getAttribute('width') ?? ''; }
  set width(v: string) { this.setAttribute('width', v); }
  get height(): string { return this.getAttribute('height') ?? ''; }
  set height(v: string) { this.setAttribute('height', v); }
  get name(): string { return this.getAttribute('name') ?? ''; }
  set name(v: string) { this.setAttribute('name', v); }
  get allow(): string { return this.getAttribute('allow') ?? ''; }
  set allow(v: string) { this.setAttribute('allow', v); }
  get sandbox(): string { return this.getAttribute('sandbox') ?? ''; }
  get contentDocument(): null { return null; }
  get contentWindow(): null { return null; }
}

// ── Misc ──────────────────────────────────────────────────────────────────

export class HTMLProgressElement extends HTMLElement {
  get value(): number { return Number(this.getAttribute('value') ?? 0); }
  set value(v: number) { this.setAttribute('value', String(v)); }
  get max(): number { return Number(this.getAttribute('max') ?? 1); }
  set max(v: number) { this.setAttribute('max', String(v)); }
  get position(): number { const m = this.max; return m > 0 ? this.value / m : -1; }
}

export class HTMLMeterElement extends HTMLElement {
  get value(): number { return Number(this.getAttribute('value') ?? 0); }
  set value(v: number) { this.setAttribute('value', String(v)); }
  get min(): number { return Number(this.getAttribute('min') ?? 0); }
  set min(v: number) { this.setAttribute('min', String(v)); }
  get max(): number { return Number(this.getAttribute('max') ?? 1); }
  set max(v: number) { this.setAttribute('max', String(v)); }
  get low(): number { return Number(this.getAttribute('low') ?? this.min); }
  set low(v: number) { this.setAttribute('low', String(v)); }
  get high(): number { return Number(this.getAttribute('high') ?? this.max); }
  set high(v: number) { this.setAttribute('high', String(v)); }
  get optimum(): number { return Number(this.getAttribute('optimum') ?? (this.min + this.max) / 2); }
  set optimum(v: number) { this.setAttribute('optimum', String(v)); }
}

export class HTMLDetailsElement extends HTMLElement {
  get open(): boolean { return this.hasAttribute('open'); }
  set open(v: boolean) { v ? this.setAttribute('open', '') : this.removeAttribute('open'); }
}

export class HTMLDialogElement extends HTMLElement {
  private _returnValue = '';
  get open(): boolean { return this.hasAttribute('open'); }
  get returnValue(): string { return this._returnValue; }
  set returnValue(v: string) { this._returnValue = v; }
  show(): void { this.setAttribute('open', ''); }
  showModal(): void { this.setAttribute('open', ''); }
  close(returnValue?: string): void {
    if (returnValue !== undefined) this._returnValue = returnValue;
    this.removeAttribute('open');
  }
}

// ── Bulk registration ─────────────────────────────────────────────────────

type Ctor = new (ctx: DocumentContext, handle: NodeHandle) => HTMLElement;

const TAG_MAP: [string[], Ctor][] = [
  [['div'], HTMLDivElement],
  [['span'], HTMLSpanElement],
  [['p'], HTMLParagraphElement],
  [['h1','h2','h3','h4','h5','h6'], HTMLHeadingElement],
  [['ul'], HTMLUListElement],
  [['ol'], HTMLOListElement],
  [['li'], HTMLLIElement],
  [['pre'], HTMLPreElement],
  [['blockquote','q'], HTMLQuoteElement],
  [['hr'], HTMLHRElement],
  [['br'], HTMLBRElement],
  [['body'], HTMLBodyElement],
  [['head'], HTMLHeadElement],
  [['html'], HTMLHtmlElement],
  [['a'], HTMLAnchorElement],
  [['img'], HTMLImageElement],
  [['input'], HTMLInputElement],
  [['button'], HTMLButtonElement],
  [['form'], HTMLFormElement],
  [['textarea'], HTMLTextAreaElement],
  [['select'], HTMLSelectElement],
  [['option'], HTMLOptionElement],
  [['optgroup'], HTMLOptGroupElement],
  [['label'], HTMLLabelElement],
  [['script'], HTMLScriptElement],
  [['link'], HTMLLinkElement],
  [['meta'], HTMLMetaElement],
  [['title'], HTMLTitleElement],
  [['style'], HTMLStyleElement],
  [['table'], HTMLTableElement],
  [['thead','tbody','tfoot'], HTMLTableSectionElement],
  [['tr'], HTMLTableRowElement],
  [['td','th'], HTMLTableCellElement],
  [['col','colgroup'], HTMLTableColElement],
  [['caption'], HTMLTableCaptionElement],
  [['video'], HTMLVideoElement],
  [['audio'], HTMLAudioElement],
  [['canvas'], HTMLCanvasElement],
  [['iframe'], HTMLIFrameElement],
  [['progress'], HTMLProgressElement],
  [['meter'], HTMLMeterElement],
  [['details'], HTMLDetailsElement],
  [['dialog'], HTMLDialogElement],
  [['section','article','header','footer','main','nav','aside',
     'figure','figcaption','summary','address',
     'em','strong','small','sub','sup','mark','i','b','u','s','del','ins',
     'code','kbd','samp','var','cite','abbr','dfn','time','data','wbr',
     'dl','dt','dd','fieldset','legend','output','datalist',
     'picture','source','track','map','area','template','slot','noscript'], HTMLElement],
];

for (const [tags, Ctor] of TAG_MAP) {
  for (const tag of tags) {
    registerTagType(tag, (ctx, handle) => new Ctor(ctx, handle));
  }
}

export { HTMLElement } from './html-element';
