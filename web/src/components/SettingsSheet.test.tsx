// @vitest-environment happy-dom
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router";
import { LANG_KEY, resetLangForTests } from "../i18n";
import { THEME_KEY, resetThemeForTests } from "../theme";
import { byText, click, key, mount, type Mounted } from "../test/dom";
import { Header } from "./Header";
import { SWIPE_CLOSE_PX } from "./SettingsSheet";

let m: Mounted;

// Le déclencheur n'est pas focalisé avant le clic, comme sur Safari.
function open() {
  const trigger = byText(m.container, "Langue · Thème");
  act(() => (document.activeElement as HTMLElement | null)?.blur());
  click(trigger);
  return trigger;
}
const dialog = () => document.querySelector<HTMLElement>('[role="dialog"]');

beforeEach(() => {
  localStorage.clear();
  vi.spyOn(navigator, "language", "get").mockReturnValue("fr-FR");
  resetLangForTests();
  resetThemeForTests();
  m = mount(
    <MemoryRouter>
      <Header />
    </MemoryRouter>,
  );
});

afterEach(() => {
  m.unmount();
  vi.restoreAllMocks();
  localStorage.clear();
});

describe("settings sheet", () => {
  it("is a modal dialog that takes focus and gives it back on Escape", () => {
    const trigger = open();
    const d = dialog()!;
    expect(d.getAttribute("aria-modal")).toBe("true");
    expect(document.getElementById(d.getAttribute("aria-labelledby")!)?.textContent).toBe("Langue · Thème");
    expect(d.contains(document.activeElement)).toBe(true);
    key(document.activeElement!, "Escape");
    expect(dialog()).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it("closes on Escape even when focus has fallen back to body", () => {
    const trigger = open();
    act(() => (document.activeElement as HTMLElement).blur());
    expect(document.activeElement).toBe(document.body);
    key(document.body, "Escape");
    expect(dialog()).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it("closes with the Close button and returns focus", () => {
    const trigger = open();
    click(byText(dialog()!, "Fermer"));
    expect(dialog()).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it("traps Tab inside the sheet", () => {
    open();
    const buttons = [...dialog()!.querySelectorAll("button")];
    const first = buttons[0];
    const last = buttons[buttons.length - 1];
    act(() => last.focus());
    key(last, "Tab");
    expect(document.activeElement).toBe(first);
    key(first, "Tab", { shiftKey: true });
    expect(document.activeElement).toBe(last);
  });

  it("switches language and theme and remembers both", () => {
    open();
    click(byText(dialog()!, "English"));
    expect(document.documentElement.lang).toBe("en");
    expect(localStorage.getItem(LANG_KEY)).toBe("en");
    expect(byText(dialog()!, "English").getAttribute("aria-pressed")).toBe("true");
    click(byText(dialog()!, "Light"));
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem(THEME_KEY)).toBe("light");
    expect(byText(m.container, "Language · Theme")).toBeTruthy();
  });

  it("closes on a downward swipe of the handle, not on a short one", () => {
    open();
    const handle = () => dialog()!.querySelector('[data-testid="sheet-handle"]')!;
    const swipe = (dy: number) =>
      act(() => {
        const h = handle();
        h.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, pointerId: 1, clientY: 100 }));
        h.dispatchEvent(new PointerEvent("pointermove", { bubbles: true, pointerId: 1, clientY: 100 + dy }));
        h.dispatchEvent(new PointerEvent("pointerup", { bubbles: true, pointerId: 1, clientY: 100 + dy }));
      });
    swipe(SWIPE_CLOSE_PX / 2);
    expect(dialog()).not.toBeNull();
    swipe(SWIPE_CLOSE_PX + 20);
    expect(dialog()).toBeNull();
  });

  it("stays open when a long drag ends in pointercancel", () => {
    open();
    act(() => {
      const h = dialog()!.querySelector('[data-testid="sheet-handle"]')!;
      h.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, pointerId: 1, clientY: 100 }));
      h.dispatchEvent(new PointerEvent("pointermove", { bubbles: true, pointerId: 1, clientY: 100 + SWIPE_CLOSE_PX * 3 }));
      h.dispatchEvent(new PointerEvent("pointercancel", { bubbles: true, pointerId: 1, clientY: 100 + SWIPE_CLOSE_PX * 3 }));
    });
    expect(dialog()).not.toBeNull();
  });
});
