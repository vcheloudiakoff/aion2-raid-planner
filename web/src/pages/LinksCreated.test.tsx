// @vitest-environment happy-dom
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router";
import { resetLangForTests } from "../i18n";
import { byText, click, mount, type Mounted } from "../test/dom";
import { ORGANIZER, created } from "../test/fixtures";
import { LinksCreated } from "./LinksCreated";

const ORIGIN = "https://raid.example.org";
let m: Mounted;

function render(stored = true) {
  m = mount(
    <MemoryRouter initialEntries={["/new"]}>
      <Routes>
        <Route path="/new" element={<LinksCreated created={created} stored={stored} origin={ORIGIN} />} />
        <Route path="/o/:secret" element={<p>organizer page</p>} />
      </Routes>
    </MemoryRouter>,
  );
}

function setNavigator(name: "clipboard" | "share", value: unknown) {
  Object.defineProperty(navigator, name, { value, configurable: true, writable: true });
}

const openButton = () => byText(m.container, "Ouvrir mon raid") as HTMLButtonElement;
const chip = (section: string) =>
  m.container.querySelector<HTMLButtonElement>(`[data-testid="${section}"] button[aria-label^="Copier"]`)!;

async function tap(el: Element) {
  await act(async () => {
    (el as HTMLElement).click();
    for (let i = 0; i < 5; i++) await Promise.resolve();
  });
}

beforeEach(() => {
  vi.spyOn(navigator, "language", "get").mockReturnValue("fr-FR");
  resetLangForTests();
  setNavigator("clipboard", { writeText: vi.fn(async () => {}) });
  setNavigator("share", undefined);
});

afterEach(() => {
  m.unmount();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("links created", () => {
  it("shows the three chips in order: organizer, direct, public, with full URLs", () => {
    render();
    const sections = [...m.container.querySelectorAll("section")].map((s) => s.dataset.testid);
    expect(sections).toEqual(["link-organizer", "link-join", "link-public"]);
    expect(chip("link-organizer").textContent).toBe(`${ORIGIN}/o/${ORGANIZER}`);
    expect(chip("link-join").textContent).toBe(`${ORIGIN}${created.links.join}`);
    expect(chip("link-public").textContent).toBe(`${ORIGIN}${created.links.public}`);
    expect(byText(m.container, "Garde ce lien : c'est le seul moyen de gérer ton raid.")).toBeTruthy();
    expect(m.container.textContent).not.toMatch(/Partager/);
    expect(m.container.textContent).not.toMatch(/Ce navigateur ne gardera pas/);
  });

  it("keeps Open my raid disabled until the organizer link is copied", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    render();
    expect(openButton().disabled).toBe(true);
    expect(byText(m.container, "Copie ou partage ton lien d'organisateur, ou coche « J'ai gardé mon lien ».")).toBeTruthy();
    // Copier un autre lien ne suffit pas.
    await tap(chip("link-join"));
    expect(openButton().disabled).toBe(true);
    await tap(chip("link-organizer"));
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(`${ORIGIN}/o/${ORGANIZER}`);
    expect(openButton().disabled).toBe(false);
    expect(byText(m.container, "Copié")).toBeTruthy();
    act(() => vi.advanceTimersByTime(1500));
    expect(m.container.textContent).not.toMatch(/Copié/);
  });

  it("selects the text when the clipboard fails, without unlocking", async () => {
    setNavigator("clipboard", { writeText: vi.fn(async () => Promise.reject(new Error("denied"))) });
    render();
    await tap(chip("link-organizer"));
    expect(openButton().disabled).toBe(true);
    expect(m.container.textContent).not.toMatch(/Copié/);
    expect(window.getSelection()?.toString()).toBe(`${ORIGIN}/o/${ORGANIZER}`);
  });

  it("unlocks after a share, with only the URL shared", async () => {
    const share = vi.fn(async () => {});
    setNavigator("share", share);
    render();
    await tap(byText(m.container, "Partager"));
    expect(share).toHaveBeenCalledWith({ url: `${ORIGIN}/o/${ORGANIZER}` });
    expect(openButton().disabled).toBe(false);
  });

  it("stays locked when the share is cancelled", async () => {
    setNavigator("share", vi.fn(async () => Promise.reject(new DOMException("cancel", "AbortError"))));
    render();
    await tap(byText(m.container, "Partager"));
    expect(openButton().disabled).toBe(true);
  });

  it("unlocks with the I kept my link box, then opens the organizer page", () => {
    render();
    const box = m.container.querySelector<HTMLInputElement>('input[type="checkbox"]')!;
    click(box);
    expect(openButton().disabled).toBe(false);
    click(openButton());
    expect(byText(m.container, "organizer page")).toBeTruthy();
    expect(m.container.textContent).not.toContain(ORGANIZER);
  });

  it("warns when the browser cannot keep the link", () => {
    render(false);
    expect(byText(m.container, "Ce navigateur ne gardera pas le lien : copie-le maintenant.")).toBeTruthy();
  });
});
