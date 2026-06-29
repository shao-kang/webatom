// https://html.spec.whatwg.org/multipage/dom.html#htmlelement

import { Element } from '@/interface/element';
import type { DocumentContext } from '@/interface/document-context';
import type { NodeHandle } from '@/interface/native';

export class HTMLElement extends Element {
  constructor(ctx: DocumentContext, handle: NodeHandle) {
    super(ctx, handle);
  }

  get hidden(): boolean { return this.hasAttribute('hidden'); }
  set hidden(value: boolean) {
    if (value) this.setAttribute('hidden', '');
    else this.removeAttribute('hidden');
  }

  get title(): string { return this.getAttribute('title') ?? ''; }
  set title(value: string) { this.setAttribute('title', value); }

  get lang(): string { return this.getAttribute('lang') ?? ''; }
  set lang(value: string) { this.setAttribute('lang', value); }

  get dir(): string { return this.getAttribute('dir') ?? ''; }
  set dir(value: string) { this.setAttribute('dir', value); }

  get tabIndex(): number { return Number(this.getAttribute('tabindex') ?? -1); }
  set tabIndex(value: number) { this.setAttribute('tabindex', String(value)); }

  get draggable(): boolean { return this.getAttribute('draggable') === 'true'; }
  set draggable(value: boolean) { this.setAttribute('draggable', String(value)); }

  get contentEditable(): string { return this.getAttribute('contenteditable') ?? 'inherit'; }
  set contentEditable(value: string) { this.setAttribute('contenteditable', value); }
  get isContentEditable(): boolean { return this.contentEditable === 'true'; }

  get innerText(): string { return this.textContent ?? ''; }
  set innerText(value: string) { this.textContent = value; }

  get innerHTML(): string { return ''; }
  set innerHTML(value: string) {
    if (value === '') this.textContent = '';
  }
  get outerHTML(): string { return ''; }

  get offsetWidth(): number { return 0; }
  get offsetHeight(): number { return 0; }
  get offsetTop(): number { return 0; }
  get offsetLeft(): number { return 0; }
  get clientWidth(): number { return 0; }
  get clientHeight(): number { return 0; }
  get scrollWidth(): number { return 0; }
  get scrollHeight(): number { return 0; }
  get scrollTop(): number { return 0; }
  set scrollTop(_v: number) {}
  get scrollLeft(): number { return 0; }
  set scrollLeft(_v: number) {}

  getBoundingClientRect(): DOMRect {
    return { x: 0, y: 0, width: 0, height: 0, top: 0, right: 0, bottom: 0, left: 0, toJSON: () => ({}) } as DOMRect;
  }

  focus(_options?: FocusOptions): void {}
  blur(): void {}
  click(): void { this.dispatchEvent(new Event('click', { bubbles: true })); }

  get dataset(): DOMStringMap { return {} as DOMStringMap; }

  get spellcheck(): boolean { return this.getAttribute('spellcheck') !== 'false'; }
  set spellcheck(v: boolean) { this.setAttribute('spellcheck', String(v)); }

  get autocapitalize(): string { return this.getAttribute('autocapitalize') ?? ''; }
  set autocapitalize(v: string) { this.setAttribute('autocapitalize', v); }
}
