// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { THEME_KEY, getTheme, resetThemeForTests, setTheme } from "./theme";

function systemPrefers(light: boolean) {
  vi.stubGlobal(
    "matchMedia",
    (query: string) =>
      ({
        matches: light && query === "(prefers-color-scheme: light)",
        media: query,
        addEventListener: () => {},
        removeEventListener: () => {},
      }) as unknown as MediaQueryList,
  );
}

beforeEach(() => localStorage.clear());
afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("theme", () => {
  it("is dark by default", () => {
    systemPrefers(false);
    resetThemeForTests();
    expect(getTheme()).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("follows prefers-color-scheme while nothing is chosen", () => {
    systemPrefers(true);
    resetThemeForTests();
    expect(getTheme()).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("remembers an explicit choice over the system", () => {
    systemPrefers(true);
    resetThemeForTests();
    setTheme("dark");
    expect(localStorage.getItem(THEME_KEY)).toBe("dark");
    resetThemeForTests();
    expect(getTheme()).toBe("dark");
  });

  it("works without storage", () => {
    systemPrefers(false);
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    });
    resetThemeForTests();
    setTheme("light");
    expect(getTheme()).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });
});
