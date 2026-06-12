// Import element.ts first — its module body calls _registerElement(Element),
// which must happen before any wrapNode call creates Element instances.
import { Element } from './element.js';
import { Event, CustomEvent, MouseEvent, KeyboardEvent, EventTarget } from './event_target.js';
import { Node, Text, Comment } from './node.js';
import { document } from './document.js';
import { window } from './window.js';

Object.assign(globalThis, {
  // DOM classes
  EventTarget, Event, CustomEvent, MouseEvent, KeyboardEvent,
  Node, Text, Comment, Element,

  // Document & window
  document,
  window,

  // Hoist window properties to global scope (browser behaviour)
  location:              window.location,
  navigator:             window.navigator,
  history:               window.history,
  screen:                window.screen,
  setTimeout:            window.setTimeout.bind(window),
  clearTimeout:          window.clearTimeout.bind(window),
  setInterval:           window.setInterval.bind(window),
  clearInterval:         window.clearInterval.bind(window),
  requestAnimationFrame: window.requestAnimationFrame.bind(window),
  cancelAnimationFrame:  window.cancelAnimationFrame.bind(window),
  getComputedStyle:      window.getComputedStyle.bind(window),
  alert:                 window.alert.bind(window),
  confirm:               window.confirm.bind(window),
  prompt:                window.prompt.bind(window),
  queueMicrotask:        window.queueMicrotask.bind(window),
});
