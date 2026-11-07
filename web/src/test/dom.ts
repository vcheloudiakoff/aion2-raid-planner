// Rendu de test minimal, sans bibliothèque : `createRoot` + `act`.
import { act, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

export interface Mounted {
  container: HTMLElement;
  unmount: () => void;
}

export function mount(node: ReactNode): Mounted {
  const container = document.createElement("div");
  document.body.append(container);
  let root: Root;
  act(() => {
    root = createRoot(container);
    root.render(node);
  });
  return {
    container,
    unmount: () => {
      act(() => root.unmount());
      container.remove();
    },
  };
}

export function key(target: Element, k: string, opts: KeyboardEventInit = {}) {
  act(() => {
    target.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...opts }));
  });
}

export function click(target: Element) {
  act(() => {
    (target as HTMLElement).click();
  });
}

export function byText(root: ParentNode, text: string): HTMLElement {
  const el = [...root.querySelectorAll<HTMLElement>("button, h1, h2, p, a, span")].find(
    (e) => e.textContent === text,
  );
  if (!el) throw new Error(`no element with text ${text}`);
  return el;
}

/** Saisie comme au clavier : passe par le setter natif pour que React voie le changement. */
export function fill(el: HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), "value")?.set;
  act(() => {
    setter?.call(el, value);
    el.dispatchEvent(new Event(el instanceof HTMLSelectElement ? "change" : "input", { bubbles: true }));
  });
}
